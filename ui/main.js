const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const dialog = window.__TAURI__.dialog;
const opener = window.__TAURI__.opener;

const $ = (id) => document.getElementById(id);
const DAY = 86400;

const state = {
  roots: [],
  items: [],
  selected: new Set(),
  tab: "project",
  sort: { key: "bytes", dir: "desc" },
  query: "",
  scanning: false,
  elevated: false,
};

/* Formatting */

function fmtBytes(n) {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  // 99.96 rounds to "100.0"; switch to whole numbers once the rounded value reaches 100.
  return `${v >= 99.95 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`;
}

function fmtAge(unixSecs) {
  if (!unixSecs) return "empty";
  const days = Math.floor((Date.now() / 1000 - unixSecs) / DAY);
  if (days < 1) return "today";
  if (days === 1) return "yesterday";
  if (days < 30) return `${days} days ago`;
  if (days < 365) {
    const m = Math.floor(days / 30);
    return `${m} month${m === 1 ? "" : "s"} ago`;
  }
  const y = (days / 365).toFixed(1).replace(/\.0$/, "");
  return `${y} year${y === "1" ? "" : "s"} ago`;
}

function ageDays(item) {
  return item.lastWrite ? (Date.now() / 1000 - item.lastWrite) / DAY : Infinity;
}

const SAFETY = {
  safe: { label: "Rebuilds on its own", cls: "safe" },
  reinstall: { label: "Needs reinstall", cls: "reinstall" },
  caution: { label: "Check first", cls: "caution" },
};

function esc(s) {
  return String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

/* Storage of chosen roots. Per-viewer convenience only. */

function loadRoots() {
  try {
    const saved = JSON.parse(localStorage.getItem("devsweep.roots") || "null");
    if (Array.isArray(saved) && saved.length) return saved;
  } catch {}
  return null;
}
function saveRoots() {
  try { localStorage.setItem("devsweep.roots", JSON.stringify(state.roots)); } catch {}
}

/* Drives */

async function renderDrives() {
  const drives = await invoke("list_drives");
  $("drives").innerHTML = drives
    .map((d) => {
      const used = d.total ? 1 - d.free / d.total : 0;
      const low = d.free / d.total < 0.1;
      return `<div class="drive" title="${esc(d.label || d.mount)}">
        <span class="drive-name">${esc(d.mount.replace(/\\$/, ""))}</span>
        <div class="drive-meter ${low ? "low" : ""}"><div style="width:${(used * 100).toFixed(1)}%"></div></div>
        <span class="drive-free">${fmtBytes(d.free)} free of ${fmtBytes(d.total)}</span>
      </div>`;
    })
    .join("");
}

/* Roots */

function renderRoots() {
  $("roots").innerHTML = state.roots
    .map(
      (r, i) => `<span class="root">${esc(r)}<button type="button" data-remove="${i}" aria-label="Remove ${esc(r)}">
        <svg viewBox="0 0 10 10"><path d="M2 2l6 6M8 2l-6 6"/></svg></button></span>`
    )
    .join("");
  $("scan").disabled = state.roots.length === 0 || state.scanning;
}

$("roots").addEventListener("click", (e) => {
  const btn = e.target.closest("[data-remove]");
  if (!btn || state.scanning) return;
  state.roots.splice(Number(btn.dataset.remove), 1);
  saveRoots();
  renderRoots();
});

$("add-root").addEventListener("click", async () => {
  const picked = await dialog.open({ directory: true, multiple: true, title: "Choose folders to scan" });
  if (!picked) return;
  for (const p of Array.isArray(picked) ? picked : [picked]) {
    if (!state.roots.some((r) => r.toLowerCase() === p.toLowerCase())) state.roots.push(p);
  }
  saveRoots();
  renderRoots();
});

/* Scanning */

function showProgress(on) {
  $("progress").hidden = !on;
  state.scanning = on;
  $("add-root").disabled = on;
  $("include-caches").disabled = on;
  $("scan").textContent = on ? "Scanning" : "Scan";
  renderRoots();
}

listen("scan-progress", ({ payload: p }) => {
  if (!state.scanning) return;
  const fill = $("progress-fill");
  if (p.phase === "folders" && p.toSize === 0) {
    fill.classList.add("indeterminate");
    $("progress-label").textContent = `${p.dirs.toLocaleString()} folders checked, ${p.found} found`;
  } else if (p.phase === "caches") {
    fill.classList.add("indeterminate");
    $("progress-label").textContent = "Measuring package caches";
  } else if (p.phase === "processes") {
    fill.classList.add("indeterminate");
    $("progress-label").textContent = "Checking running programs";
  } else {
    fill.classList.remove("indeterminate");
    fill.style.setProperty("--p", p.toSize ? p.sized / p.toSize : 0);
    $("progress-label").textContent = `Measuring ${p.sized} of ${p.toSize}`;
  }
  $("progress-path").textContent = p.current || "";
});

$("scan").addEventListener("click", runScan);
$("cancel").addEventListener("click", () => invoke("cancel"));

async function runScan() {
  if (state.scanning || !state.roots.length) return;
  showProgress(true);
  $("notice").hidden = true;
  $("progress-fill").style.setProperty("--p", 0);
  $("progress-label").textContent = "Starting";
  $("progress-path").textContent = "";
  try {
    const res = await invoke("scan", { roots: state.roots, includeCaches: $("include-caches").checked });
    state.items = res.items;
    state.selected.clear();
    selectUnused(30, "project", true);
    const notes = [];
    if (res.cancelled) notes.push("Scan was cancelled, so the list is incomplete.");
    if (res.missingRoots.length) notes.push(`Not found: ${res.missingRoots.join(", ")}.`);
    const busy = res.runningTools.filter((t) => t !== "rust-analyzer");
    if (busy.length) notes.push(`Running now: ${res.runningTools.join(", ")}. Folders they use are locked.`);
    showNotice(notes.join(" "));
    $("empty").hidden = true;
    $("table-wrap").hidden = false;
    $("footer").hidden = false;
    render();
  } catch (err) {
    showNotice(String(err));
  } finally {
    showProgress(false);
  }
}

function showNotice(text) {
  $("notice").textContent = text;
  $("notice").hidden = !text;
}

/* Selection */

function selectable(item) {
  return item.inUseBy.length === 0;
}

function selectUnused(days, tab = state.tab, quiet = false) {
  for (const it of state.items) {
    if (it.group !== tab) continue;
    if (!selectable(it) || it.safety === "caution") continue;
    if (days === 0 || ageDays(it) >= days) state.selected.add(it.id);
  }
  if (!quiet) render();
}

$("select-unused").addEventListener("click", () => selectUnused(Number($("unused-days").value)));
$("select-none").addEventListener("click", () => {
  for (const it of visibleItems()) state.selected.delete(it.id);
  render();
});

$("check-all").addEventListener("change", (e) => {
  for (const it of visibleItems()) {
    if (!selectable(it)) continue;
    if (e.target.checked) state.selected.add(it.id);
    else state.selected.delete(it.id);
  }
  render();
});

$("rows").addEventListener("change", (e) => {
  const cb = e.target.closest("input[data-id]");
  if (!cb) return;
  if (cb.checked) state.selected.add(cb.dataset.id);
  else state.selected.delete(cb.dataset.id);
  render();
});

$("rows").addEventListener("click", async (e) => {
  const btn = e.target.closest("[data-reveal]");
  if (btn) {
    try { await opener.revealItemInDir(btn.dataset.reveal); } catch (err) { showNotice(String(err)); }
  }
});

/* Tabs, search, sorting */

document.querySelectorAll(".tab").forEach((t) =>
  t.addEventListener("click", () => {
    state.tab = t.dataset.tab;
    document.querySelectorAll(".tab").forEach((x) => x.setAttribute("aria-selected", String(x === t)));
    render();
  })
);

$("query").addEventListener("input", (e) => {
  state.query = e.target.value.trim().toLowerCase();
  render();
});

document.querySelectorAll("th.sortable").forEach((th) =>
  th.addEventListener("click", () => {
    const key = th.dataset.sort;
    if (state.sort.key === key) state.sort.dir = state.sort.dir === "desc" ? "asc" : "desc";
    else state.sort = { key, dir: key === "path" || key === "ecosystem" ? "asc" : "desc" };
    render();
  })
);

function visibleItems() {
  const q = state.query;
  const list = state.items.filter(
    (it) =>
      it.group === state.tab &&
      (!q || it.path.toLowerCase().includes(q) || it.label.toLowerCase().includes(q) || it.ecosystem.toLowerCase().includes(q))
  );
  const { key, dir } = state.sort;
  const m = dir === "asc" ? 1 : -1;
  list.sort((a, b) => {
    const x = a[key], y = b[key];
    return (typeof x === "string" ? x.localeCompare(y) : x - y) * m;
  });
  return list;
}

/* Rendering */

const REVEAL_ICON = `<svg viewBox="0 0 16 16"><path d="M1.5 4.5v8a1 1 0 0 0 1 1h11a1 1 0 0 0 1-1v-6a1 1 0 0 0-1-1H8L6.5 3.5h-4a1 1 0 0 0-1 1z"/></svg>`;

function render() {
  const rows = visibleItems();
  const max = Math.max(1, ...rows.map((r) => r.bytes));

  $("rows").innerHTML = rows
    .map((it) => {
      const locked = !selectable(it);
      const sel = state.selected.has(it.id) && !locked;
      const s = SAFETY[it.safety];
      const days = ageDays(it);
      const name = it.group === "project"
        ? `<span>${esc(it.label)}</span><span class="proj">${esc(projectName(it))}</span>`
        : `<span>${esc(it.label)}</span>`;
      const badge = locked
        ? `<span class="badge locked" title="${esc(it.inUseBy.join(", "))}">In use by ${esc(it.inUseBy[0].split(" ")[0])}${it.inUseBy.length > 1 ? ` +${it.inUseBy.length - 1}` : ""}</span>`
        : `<span class="badge ${s.cls}" title="${esc(it.note || "")}">${s.label}</span>`;
      return `<tr class="${sel ? "selected" : ""} ${locked ? "locked" : ""}">
        <td class="col-check"><input type="checkbox" data-id="${esc(it.id)}" ${sel ? "checked" : ""} ${locked ? "disabled" : ""} aria-label="Select ${esc(it.path)}"></td>
        <td><div class="size">${fmtBytes(it.bytes)}</div><div class="size-bar"><div style="width:${((it.bytes / max) * 100).toFixed(1)}%"></div></div></td>
        <td><div class="folder-name">${name}</div><div class="folder-path" title="${esc(it.path)}">${esc(it.path)}</div></td>
        <td class="eco">${esc(it.ecosystem)}</td>
        <td class="age ${days < 7 ? "recent" : ""}">${fmtAge(it.lastWrite)}</td>
        <td>${badge}<span class="restore" title="${esc(it.restore)}">${esc(it.restore)}</span></td>
        <td><button class="icon-btn" type="button" data-reveal="${esc(it.path)}" title="Show in Explorer" aria-label="Show ${esc(it.path)} in Explorer">${REVEAL_ICON}</button></td>
      </tr>`;
    })
    .join("");
  $("table-empty").hidden = rows.length > 0;

  for (const g of ["project", "cache"]) {
    const list = state.items.filter((i) => i.group === g);
    $(`count-${g}`).textContent = list.length ? `${list.length} · ${fmtBytes(list.reduce((a, b) => a + b.bytes, 0))}` : "0";
  }

  const pickable = rows.filter(selectable);
  const all = $("check-all");
  const n = pickable.filter((r) => state.selected.has(r.id)).length;
  all.checked = pickable.length > 0 && n === pickable.length;
  all.indeterminate = n > 0 && n < pickable.length;

  document.querySelectorAll("th.sortable").forEach((th) => {
    if (th.dataset.sort === state.sort.key) th.dataset.dir = state.sort.dir;
    else delete th.dataset.dir;
  });

  const chosen = selectedItems();
  $("sel-count").textContent = chosen.length;
  $("sel-size").textContent = fmtBytes(chosen.reduce((a, b) => a + b.bytes, 0));
  $("clean").disabled = chosen.length === 0;
}

function projectName(it) {
  if (!it.project) return "";
  const parts = it.project.split(/[\\/]/).filter(Boolean);
  return parts.slice(-2).join("\\");
}

function selectedItems() {
  return state.items.filter((i) => state.selected.has(i.id) && selectable(i));
}

/* Cleaning */

$("clean").addEventListener("click", () => {
  const chosen = selectedItems();
  if (!chosen.length) return;
  const total = chosen.reduce((a, b) => a + b.bytes, 0);
  $("confirm-title").textContent = `Delete ${chosen.length} folder${chosen.length === 1 ? "" : "s"}?`;
  $("confirm-lead").textContent = `This frees up to ${fmtBytes(total)}. Here is how each kind comes back:`;

  const groups = new Map();
  for (const it of chosen) {
    const k = `${it.ecosystem}|${it.label}`;
    const g = groups.get(k) || { it, n: 0, bytes: 0 };
    g.n++;
    g.bytes += it.bytes;
    groups.set(k, g);
  }
  $("confirm-list").innerHTML = [...groups.values()]
    .sort((a, b) => b.bytes - a.bytes)
    .map(
      (g) => `<li><div class="r-top"><span><strong>${esc(g.it.label)}</strong> (${esc(g.it.ecosystem)}) × ${g.n}</span><span>${fmtBytes(g.bytes)}</span></div>
        <div class="r-cmd">${esc(g.it.restore)}</div></li>`
    )
    .join("");

  const cautious = chosen.filter((i) => i.safety === "caution");
  $("confirm-caution").hidden = cautious.length === 0;
  $("confirm-caution").textContent = cautious.length
    ? `${cautious.length} selected folder${cautious.length === 1 ? " is" : "s are"} marked "Check first". ${cautious[0].note || ""}`
    : "";
  $("confirm-ok").textContent = `Delete ${fmtBytes(total)}`;
  $("confirm").showModal();
});

$("confirm").addEventListener("close", async () => {
  if ($("confirm").returnValue !== "ok") return;
  const ids = selectedItems().map((i) => i.id);
  $("clean-fill").style.setProperty("--p", 0);
  $("clean-path").textContent = "";
  $("working").showModal();
  try {
    const report = await invoke("clean", { ids });
    showReport(report);
  } catch (err) {
    showReport(null, String(err));
  } finally {
    $("working").close();
  }
});

// Esc must not hide the progress dialog while deleting.
$("working").addEventListener("cancel", (e) => e.preventDefault());
$("clean-cancel").addEventListener("click", () => {
  invoke("cancel");
  $("clean-cancel").disabled = true;
  $("clean-cancel").textContent = "Stopping";
});

listen("clean-progress", ({ payload: p }) => {
  $("clean-fill").style.setProperty("--p", p.total ? p.done / p.total : 0);
  $("clean-path").textContent = p.current ? `${p.done + 1} of ${p.total}: ${p.current}` : "Finishing";
});

function showReport(report, error) {
  $("clean-cancel").disabled = false;
  $("clean-cancel").textContent = "Stop after current folder";
  if (!report) {
    $("report-title").textContent = "Could not delete";
    $("report-lead").textContent = error;
    $("report-fails").innerHTML = "";
    $("report").showModal();
    return;
  }
  const ok = report.outcomes.filter((o) => o.ok);
  const failed = report.outcomes.filter((o) => !o.ok);
  const removed = new Set(ok.map((o) => o.id));
  state.items = state.items.filter((i) => !removed.has(i.id));
  for (const id of removed) state.selected.delete(id);

  $("report-title").textContent = failed.length ? "Finished with some problems" : "Done";
  const measured = Math.max(0, report.measuredBytes);
  let lead = `Deleted ${ok.length} folder${ok.length === 1 ? "" : "s"}. Free space went up by ${fmtBytes(measured)}.`;
  if (report.countedBytes > measured * 1.15 && report.countedBytes - measured > 50 * 1024 * 1024) {
    lead += ` That is less than the ${fmtBytes(report.countedBytes)} the folders added up to, because some files were shared with other folders (pnpm and Cargo use hard links).`;
  }
  if (report.cancelled) lead += " Stopped early on request.";
  if (failed.length) lead += ` ${failed.length} could not be deleted:`;
  $("report-lead").textContent = lead;
  $("report-fails").innerHTML = failed
    .map((f) => `<li><div class="f-path">${esc(f.path)}</div><div class="f-err">${esc(f.error)}</div></li>`)
    .join("");
  const denied = failed.filter((f) => f.needsAdmin).length;
  const offerAdmin = denied > 0 && !state.elevated;
  $("admin-note").hidden = !offerAdmin;
  $("restart-admin").hidden = !offerAdmin;
  if (offerAdmin) {
    const one = denied === 1;
    $("admin-note").textContent =
      `Windows refused access to ${one ? "this folder" : `${denied} of these folders`}. ` +
      `${one ? "It was" : "They were"} probably created by a program running as administrator. ` +
      `Restart DevSweep as administrator and it will scan again, so you can delete ${one ? "it" : "them"} there.`;
  }
  $("report").showModal();
  render();
  renderDrives();
}

/* Start */

$("restart-admin").addEventListener("click", async () => {
  try {
    await invoke("restart_as_admin", { scan: true });
  } catch (e) {
    $("admin-note").textContent = String(e);
  }
});

(async function init() {
  state.roots = loadRoots() || (await invoke("default_roots"));
  renderRoots();
  renderDrives();
  const info = await invoke("app_info");
  state.elevated = info.elevated;
  $("admin-badge").hidden = !info.elevated;
  if (info.autoScan) runScan();
})();
