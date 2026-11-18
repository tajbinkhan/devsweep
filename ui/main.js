const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const dialog = window.__TAURI__.dialog;
const opener = window.__TAURI__.opener;

const $ = (id) => document.getElementById(id);
const { fmtBytes, fmtAge, ageDays, selectable, picksUnused, rangeIds, maxOf } = window.DevSweepLogic;

const state = {
  roots: [],
  items: [],
  selected: new Set(),
  tab: "project",
  sort: { key: "frees", dir: "desc" },
  query: "",
  scanning: false,
  elevated: false,
  canElevate: true,
  platform: "windows",
  keep: [],
  /** Position in the visible list of the last checkbox clicked, for shift-click ranges. */
  anchor: null,
};

/* Platform wording. The backend reports the OS in app_info. */

const isWindows = () => state.platform === "windows";
const fileManager = () => (isWindows() ? "Explorer" : state.platform === "macos" ? "Finder" : "the file manager");
const sep = () => (isWindows() ? "\\" : "/");

function applyPlatformText() {
  if (isWindows()) return;
  $("rule-skip").innerHTML =
    "System folders such as <code>/usr</code> and <code>/opt</code>, <code>~/.config</code>, <code>~/.local</code>, " +
    "<code>~/.cache</code>, Flatpak and Snap apps, editor extensions, Node version managers and <code>.git</code> " +
    "are never scanned, so installed programs are not touched.";
  $("confirm-fine").textContent = "Files are deleted permanently. They do not go to the Trash.";
}

/* Formatting lives in logic.js. */

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
        <span class="drive-name">${esc(d.mount.length > 1 ? d.mount.replace(/[\\/]$/, "") : d.mount)}</span>
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
    const same = (r) => (isWindows() ? r.toLowerCase() === p.toLowerCase() : r === p);
    if (!state.roots.some(same)) state.roots.push(p);
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
    state.anchor = null;
    selectUnused(30, "project", true);
    const notes = [];
    if (res.cancelled) notes.push("Scan was cancelled, so the list is incomplete.");
    if (res.missingRoots.length) notes.push(`Not found: ${res.missingRoots.join(", ")}.`);
    const busy = res.runningTools.filter((t) => t !== "rust-analyzer");
    if (busy.length) notes.push(`Running now: ${res.runningTools.join(", ")}. Folders they use are locked.`);
    showNotice(notes.join(" "));
    showResults();
  } catch (err) {
    showNotice(String(err));
  } finally {
    showProgress(false);
  }
}

function showResults() {
  $("empty").hidden = true;
  $("table-wrap").hidden = false;
  $("footer").hidden = false;
  render();
}

function showNotice(text) {
  $("notice").textContent = text;
  $("notice").hidden = !text;
}

/* Selection */

function selectUnused(days, tab = state.tab, quiet = false) {
  for (const it of state.items) {
    if (picksUnused(it, days, tab)) state.selected.add(it.id);
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

// Ticking one box only touches that row and the totals, so long lists stay quick.
// Shift-click ticks or clears every row between the last box clicked and this one.
$("rows").addEventListener("click", (e) => {
  const cb = e.target.closest("input[data-id]");
  if (!cb) return;
  const rows = visibleItems();
  const at = rows.findIndex((r) => r.id === cb.dataset.id);
  if (e.shiftKey && state.anchor !== null && state.anchor < rows.length) {
    for (const id of rangeIds(rows, state.anchor, at)) {
      if (cb.checked) state.selected.add(id);
      else state.selected.delete(id);
    }
    state.anchor = at;
    render();
    return;
  }
  state.anchor = at;
  if (cb.checked) state.selected.add(cb.dataset.id);
  else state.selected.delete(cb.dataset.id);
  cb.closest("tr").classList.toggle("selected", cb.checked);
  renderSelection(rows);
});

$("rows").addEventListener("click", async (e) => {
  const reveal = e.target.closest("[data-reveal]");
  if (reveal) {
    try { await opener.revealItemInDir(reveal.dataset.reveal); } catch (err) { showNotice(String(err)); }
    return;
  }
  const keep = e.target.closest("[data-keep]");
  if (keep) {
    const path = keep.dataset.keep;
    const next = state.keep.includes(path) ? state.keep.filter((k) => k !== path) : [...state.keep, path];
    await saveKeep(next);
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

/* Keep list */

/** The folder a row's keep button keeps: the whole project for build folders. */
const keepPath = (it) => it.project || it.path;

async function saveKeep(paths) {
  try {
    const keptIds = new Set(await invoke("set_keep", { paths }));
    state.keep = [...new Set(paths)].sort();
    for (const it of state.items) {
      it.kept = keptIds.has(it.id);
      if (it.kept) state.selected.delete(it.id);
    }
  } catch (err) {
    showNotice(String(err));
  }
  renderKeep();
  render();
}

function renderKeep() {
  $("keep-count").textContent = state.keep.length;
  $("keep-empty").hidden = state.keep.length > 0;
  $("keep-list").innerHTML = state.keep
    .map((k) => `<li><span class="f-path">${esc(k)}</span>
      <button class="btn ghost small" type="button" data-unkeep="${esc(k)}">Stop keeping</button></li>`)
    .join("");
}

$("show-keep").addEventListener("click", () => {
  renderKeep();
  $("keep").showModal();
});
$("keep-list").addEventListener("click", (e) => {
  const btn = e.target.closest("[data-unkeep]");
  if (btn) saveKeep(state.keep.filter((k) => k !== btn.dataset.unkeep));
});

/* Export */

$("export").addEventListener("click", async () => {
  const path = await dialog.save({
    title: "Export the list",
    defaultPath: "devsweep-scan.csv",
    filters: [
      { name: "CSV", extensions: ["csv"] },
      { name: "JSON", extensions: ["json"] },
    ],
  });
  if (!path) return;
  try {
    const n = await invoke("export", { path });
    showNotice(`Saved ${n} folder${n === 1 ? "" : "s"} to ${path}.`);
  } catch (err) {
    showNotice(String(err));
  }
});

/* Rendering */

const REVEAL_ICON = `<svg viewBox="0 0 16 16"><path d="M1.5 4.5v8a1 1 0 0 0 1 1h11a1 1 0 0 0 1-1v-6a1 1 0 0 0-1-1H8L6.5 3.5h-4a1 1 0 0 0-1 1z"/></svg>`;
const KEEP_ICON = `<svg viewBox="0 0 16 16"><path d="M4 2.5h8v11l-4-3-4 3z"/></svg>`;

/** Tooltip for the age column: both dates that go into "last used". */
function ageTitle(it) {
  const parts = [];
  if (it.projectActive) parts.push(`Project last changed ${fmtAge(it.projectActive)}`);
  parts.push(`Folder last written ${fmtAge(it.lastWrite)}`);
  return parts.join(". ");
}

function render() {
  const rows = visibleItems();
  const max = Math.max(1, maxOf(rows.map((r) => r.frees)));

  $("rows").innerHTML = rows
    .map((it) => {
      const locked = !selectable(it);
      const sel = state.selected.has(it.id) && !locked;
      const kept = state.keep.includes(keepPath(it));
      const s = SAFETY[it.safety];
      const days = ageDays(it);
      const name = it.group === "project"
        ? `<span>${esc(it.label)}</span><span class="proj">${esc(projectName(it))}</span>`
        : `<span>${esc(it.label)}</span>`;
      const badge = it.kept
        ? `<span class="badge kept" title="On the keep list">Kept</span>`
        : it.inUseBy.length
        ? `<span class="badge locked" title="${esc(it.inUseBy.join(", "))}">In use by ${esc(it.inUseBy[0].split(" ")[0])}${it.inUseBy.length > 1 ? ` +${it.inUseBy.length - 1}` : ""}</span>`
        : `<span class="badge ${s.cls}" title="${esc(it.note || "")}">${s.label}</span>`;
      // Hard links can make a folder look bigger than what deleting it gives back.
      const shared = it.bytes > it.frees * 1.1 && it.bytes - it.frees > 1024 * 1024;
      const sizeTitle = shared
        ? `The files add up to ${fmtBytes(it.bytes)}, but deleting the folder frees ${fmtBytes(it.frees)}. Some files are hard links: counted once per link, or shared with files outside this folder that stay on disk.`
        : `${it.files.toLocaleString()} files`;
      return `<tr class="${sel ? "selected" : ""} ${locked ? "locked" : ""}">
        <td class="col-check"><input type="checkbox" data-id="${esc(it.id)}" ${sel ? "checked" : ""} ${locked ? "disabled" : ""} aria-label="Select ${esc(it.path)}"></td>
        <td title="${esc(sizeTitle)}"><div class="size">${fmtBytes(it.frees)}${shared ? `<span class="size-of">of ${fmtBytes(it.bytes)}</span>` : ""}</div><div class="size-bar"><div style="width:${((it.frees / max) * 100).toFixed(1)}%"></div></div></td>
        <td><div class="folder-name">${name}</div><div class="folder-path" title="${esc(it.path)}">${esc(it.path)}</div></td>
        <td class="eco">${esc(it.ecosystem)}</td>
        <td class="age ${days < 7 ? "recent" : ""}" title="${esc(ageTitle(it))}">${fmtAge(it.lastUsed)}</td>
        <td>${badge}<span class="restore" title="${esc(it.restore)}">${esc(it.restore)}</span></td>
        <td><div class="row-actions">
          <button class="icon-btn ${kept ? "on" : ""}" type="button" data-keep="${esc(keepPath(it))}" aria-pressed="${kept}"
            title="${kept ? "Stop keeping" : "Keep: never offer"} ${esc(keepPath(it))}" aria-label="${kept ? "Stop keeping" : "Keep"} ${esc(keepPath(it))}">${KEEP_ICON}</button>
          <button class="icon-btn" type="button" data-reveal="${esc(it.path)}" title="Show in ${fileManager()}" aria-label="Show ${esc(it.path)} in ${fileManager()}">${REVEAL_ICON}</button>
        </div></td>
      </tr>`;
    })
    .join("");
  $("table-empty").hidden = rows.length > 0;

  for (const g of ["project", "cache"]) {
    const list = state.items.filter((i) => i.group === g);
    $(`count-${g}`).textContent = list.length ? `${list.length} · ${fmtBytes(list.reduce((a, b) => a + b.frees, 0))}` : "0";
  }

  document.querySelectorAll("th.sortable").forEach((th) => {
    if (th.dataset.sort === state.sort.key) th.dataset.dir = state.sort.dir;
    else delete th.dataset.dir;
  });
  renderSelection(rows);
}

/** The select-all box and the footer totals. */
function renderSelection(rows) {
  const pickable = rows.filter(selectable);
  const all = $("check-all");
  const n = pickable.filter((r) => state.selected.has(r.id)).length;
  all.checked = pickable.length > 0 && n === pickable.length;
  all.indeterminate = n > 0 && n < pickable.length;

  const chosen = selectedItems();
  $("sel-count").textContent = chosen.length;
  $("sel-size").textContent = fmtBytes(chosen.reduce((a, b) => a + b.frees, 0));
  $("clean").disabled = chosen.length === 0;
}

function projectName(it) {
  if (!it.project) return "";
  const parts = it.project.split(/[\\/]/).filter(Boolean);
  return parts.slice(-2).join(sep());
}

function selectedItems() {
  return state.items.filter((i) => state.selected.has(i.id) && selectable(i));
}

/* Cleaning */

$("clean").addEventListener("click", () => {
  const chosen = selectedItems();
  if (!chosen.length) return;
  const total = chosen.reduce((a, b) => a + b.frees, 0);
  $("confirm-title").textContent = `Delete ${chosen.length} folder${chosen.length === 1 ? "" : "s"}?`;
  $("confirm-lead").textContent = `This frees up to ${fmtBytes(total)}. Here is how each kind comes back:`;

  const groups = new Map();
  for (const it of chosen) {
    const k = `${it.ecosystem}|${it.label}`;
    const g = groups.get(k) || { it, n: 0, bytes: 0 };
    g.n++;
    g.bytes += it.frees;
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
    lead += ` That is less than the ${fmtBytes(report.countedBytes)} expected, because some files were shared with other folders through hard links, or other programs wrote to the drive meanwhile.`;
  }
  if (report.cancelled) lead += " Stopped early on request.";
  if (failed.length) lead += ` ${failed.length} could not be deleted:`;
  $("report-lead").textContent = lead;
  $("report-fails").innerHTML = failed
    .map((f) => `<li><div class="f-path">${esc(f.path)}</div><div class="f-err">${esc(f.error)}</div></li>`)
    .join("");
  const deniedItems = failed.filter((f) => f.needsAdmin);
  const denied = deniedItems.length;
  const one = denied === 1;
  const offerAdmin = denied > 0 && !state.elevated && state.canElevate;
  const explainOwner = denied > 0 && !state.elevated && !state.canElevate;
  $("admin-note").hidden = !(offerAdmin || explainOwner);
  $("restart-admin").hidden = !offerAdmin;
  if (offerAdmin) {
    $("admin-note").textContent =
      `Windows refused access to ${one ? "this folder" : `${denied} of these folders`}. ` +
      `${one ? "It was" : "They were"} probably created by a program running as administrator. ` +
      `Restart DevSweep as administrator and it will scan again, so you can delete ${one ? "it" : "them"} there.`;
  } else if (explainOwner) {
    const quoted = deniedItems.map((f) => `'${f.path.replace(/'/g, "'\\''")}'`).join(" ");
    $("admin-note").textContent =
      `Permission was denied for ${one ? "this folder" : `${denied} of these folders`}. ` +
      `${one ? "It" : "They"} probably contain${one ? "s" : ""} files owned by root, for example from sudo or a Docker build. ` +
      `To take ownership back, run: sudo chown -R "$USER": ${quoted}  then scan again.`;
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
  state.canElevate = info.canElevate;
  state.platform = info.platform;
  state.keep = await invoke("get_keep");
  renderKeep();
  applyPlatformText();
  $("admin-badge").hidden = !info.elevated;
  if (info.autoScan) return runScan();

  // Show the last finished scan right away. Nothing is selected until the user asks, since
  // sizes and ages may have changed; everything is checked again before deleting anyway.
  const saved = await invoke("last_scan");
  if (saved && saved.items.length && !state.scanning) {
    state.items = saved.items;
    showNotice(`Showing the scan from ${fmtAge(saved.when)}. Scan again for current sizes. Everything is checked again before it is deleted.`);
    showResults();
  }
})();
