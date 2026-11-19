// Pure functions behind the interface: formatting and which rows get selected. They touch no
// DOM, so `node --test "tests/ui/*.test.js"` can check them. In the window they are `window.DevSweepLogic`.
(function (root) {
  const DAY = 86400;

  function fmtBytes(n) {
    if (n < 1024) return `${n} B`;
    const units = ["KB", "MB", "GB", "TB"];
    let v = n / 1024;
    let i = 0;
    while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
    // 99.96 rounds to "100.0"; switch to whole numbers once the rounded value reaches 100.
    return `${v >= 99.95 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`;
  }

  function fmtAge(unixSecs, now = Date.now() / 1000) {
    if (!unixSecs) return "empty";
    const days = Math.floor((now - unixSecs) / DAY);
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

  /** Days since the folder or its project was last used. Empty folders are unused forever. */
  function ageDays(item, now = Date.now() / 1000) {
    return item.lastUsed ? (now - item.lastUsed) / DAY : Infinity;
  }

  /** Whether a row can be ticked at all: not used by a running program, not kept. */
  function selectable(item) {
    return item.inUseBy.length === 0 && !item.kept;
  }

  /** What "Select unused for N days" ticks. "Check first" rows are never ticked for you. */
  function picksUnused(item, days, tab, now = Date.now() / 1000) {
    return (
      item.group === tab &&
      selectable(item) &&
      item.safety !== "caution" &&
      (days === 0 || ageDays(item, now) >= days)
    );
  }

  /** Ids of the selectable rows between two positions in the visible list, both included. */
  function rangeIds(rows, from, to) {
    const [a, b] = from <= to ? [from, to] : [to, from];
    return rows.slice(a, b + 1).filter(selectable).map((r) => r.id);
  }

  /** Cleaned by a docker command. Its `path` is that command, not a folder. */
  function isDocker(item) {
    return String(item.action || "").startsWith("docker_");
  }

  /** A WSL virtual disk that gets compacted, not deleted. */
  function isDisk(item) {
    return item.action === "compact_disk";
  }

  /** "3 folders" when every item is a folder on disk, "3 items" once Docker or a disk is among them. */
  function noun(items, n = items.length) {
    const word = items.every((i) => !isDocker(i) && !isDisk(i)) ? "folder" : "item";
    return `${n} ${word}${n === 1 ? "" : "s"}`;
  }

  /**
   * Whether the free space a clean measured is well short of what it should have freed on
   * the drive. Space Docker freed inside its virtual disk never reaches the drive, so it is
   * left out of the expectation.
   */
  function fellShort(report) {
    const expected = report.countedBytes - (report.vmBytes || 0);
    const measured = Math.max(0, report.measuredBytes);
    return expected > measured * 1.15 && expected - measured > 50 * 1024 * 1024;
  }

  /** The update button's text while the new version downloads. */
  function downloadLabel(downloaded, total) {
    if (!total) return `Downloading ${fmtBytes(downloaded)}`;
    return `Downloading ${Math.min(100, Math.floor((downloaded / total) * 100))}%`;
  }

  /**
   * Whether the update bar shows a release. "Check now" always shows it. Otherwise a version
   * the user skipped, or closed this session, stays out of sight until a newer one comes out.
   */
  function showsUpdate(info, { manual = false, skipped = null, hidden = null } = {}) {
    if (!info) return false;
    return manual || (info.version !== skipped && info.version !== hidden);
  }

  /** Whether a day has passed since the last successful update check. */
  function dueForCheck(lastCheck, now) {
    return !lastCheck || now - lastCheck >= DAY;
  }

  /** The largest value without spreading the array, which fails on very long lists. */
  function maxOf(values) {
    let m = 0;
    for (const v of values) if (v > m) m = v;
    return m;
  }

  const api = { DAY, fmtBytes, fmtAge, ageDays, selectable, picksUnused, rangeIds, maxOf, isDocker, isDisk, noun, fellShort, downloadLabel, showsUpdate, dueForCheck };
  if (typeof module !== "undefined" && module.exports) module.exports = api;
  else root.DevSweepLogic = api;
})(typeof window !== "undefined" ? window : globalThis);
