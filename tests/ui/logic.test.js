// Tests for the interface's selection and formatting rules. Run with `node --test "tests/ui/*.test.js"`.
const test = require("node:test");
const assert = require("node:assert/strict");
const L = require("../../ui/logic.js");

const NOW = 1_800_000_000;
const daysAgo = (d) => NOW - d * L.DAY;
const item = (over = {}) => ({
  id: over.id || "x",
  group: "project",
  safety: "reinstall",
  inUseBy: [],
  kept: false,
  lastUsed: daysAgo(45),
  ...over,
});

test("select unused goes by the last use of the project, not the folder", () => {
  assert.equal(L.picksUnused(item({ lastUsed: daysAgo(45) }), 30, "project", NOW), true);
  assert.equal(L.picksUnused(item({ lastUsed: daysAgo(3), lastWrite: daysAgo(400) }), 30, "project", NOW), false);
  assert.equal(L.picksUnused(item({ lastUsed: daysAgo(3) }), 0, "project", NOW), true, "any age");
  assert.equal(L.picksUnused(item({ lastUsed: 0 }), 90, "project", NOW), true, "empty counts as unused");
});

test("select unused never ticks locked, kept, Check first or other-tab rows", () => {
  assert.equal(L.picksUnused(item({ inUseBy: ["node (PID 3)"] }), 30, "project", NOW), false);
  assert.equal(L.picksUnused(item({ kept: true }), 30, "project", NOW), false);
  assert.equal(L.picksUnused(item({ safety: "caution" }), 30, "project", NOW), false);
  assert.equal(L.picksUnused(item({ group: "cache" }), 30, "project", NOW), false);
});

test("shift-click range covers both ends and skips rows that cannot be ticked", () => {
  const rows = ["a", "b", "c", "d", "e"].map((id) => item({ id, kept: id === "c" }));
  assert.deepEqual(L.rangeIds(rows, 1, 3), ["b", "d"]);
  assert.deepEqual(L.rangeIds(rows, 4, 0), ["a", "b", "d", "e"], "works upwards too");
});

test("formatting", () => {
  assert.equal(L.fmtBytes(512), "512 B");
  assert.equal(L.fmtBytes(1536), "1.5 KB");
  assert.equal(L.fmtBytes(99.96 * 1024 ** 3), "100 GB");
  assert.equal(L.fmtAge(0, NOW), "empty");
  assert.equal(L.fmtAge(daysAgo(1), NOW), "yesterday");
  assert.equal(L.fmtAge(daysAgo(45), NOW), "1 month ago");
  assert.equal(L.fmtAge(daysAgo(365), NOW), "1 year ago");
});

test("maxOf handles lists far longer than the argument limit", () => {
  const big = Array.from({ length: 300_000 }, (_, i) => i);
  assert.equal(L.maxOf(big), 299_999);
  assert.equal(L.maxOf([]), 0);
});

test("Docker and WSL disk rows are told apart from folders", () => {
  const folder = item({ action: "remove_dir" });
  const images = item({ action: "docker_images", group: "cache" });
  const disk = item({ action: "compact_disk", group: "cache" });
  assert.equal(L.isDocker(images), true);
  assert.equal(L.isDocker(folder), false);
  assert.equal(L.isDisk(disk), true);
  assert.equal(L.noun([folder, folder]), "2 folders");
  assert.equal(L.noun([folder]), "1 folder");
  assert.equal(L.noun([folder, images]), "2 items");
  assert.equal(L.noun([disk]), "1 item");
});

test("space freed inside Docker's virtual disk does not count as missing", () => {
  const GB = 1024 ** 3;
  assert.equal(L.fellShort({ countedBytes: 10 * GB, measuredBytes: 2 * GB, vmBytes: 0 }), true);
  assert.equal(L.fellShort({ countedBytes: 10 * GB, measuredBytes: 2 * GB, vmBytes: 8 * GB }), false);
  assert.equal(L.fellShort({ countedBytes: 10 * GB, measuredBytes: 10 * GB }), false, "older reports have no vmBytes");
});

test("update download progress", () => {
  assert.equal(L.downloadLabel(512 * 1024, 2 * 1024 * 1024), "Downloading 25%");
  assert.equal(L.downloadLabel(3 * 1024 * 1024, 2 * 1024 * 1024), "Downloading 100%", "never above 100");
  assert.equal(L.downloadLabel(1536, null), "Downloading 1.5 KB", "size unknown");
});

test("a skipped or closed version stays hidden until a newer one or Check now", () => {
  const v2 = { version: "0.2.0" };
  assert.equal(L.showsUpdate(null), false);
  assert.equal(L.showsUpdate(v2), true);
  assert.equal(L.showsUpdate(v2, { skipped: "0.2.0" }), false);
  assert.equal(L.showsUpdate({ version: "0.3.0" }, { skipped: "0.2.0" }), true, "a newer release shows again");
  assert.equal(L.showsUpdate(v2, { hidden: "0.2.0" }), false, "closed this session");
  assert.equal(L.showsUpdate(v2, { skipped: "0.2.0", manual: true }), true, "Check now always shows it");
});

test("the update check repeats once a day", () => {
  assert.equal(L.dueForCheck(0, NOW), true, "never checked");
  assert.equal(L.dueForCheck(NOW - 23 * 3600, NOW), false);
  assert.equal(L.dueForCheck(NOW - L.DAY, NOW), true);
});

test("a scheduled run reads as one line", () => {
  const run = (over = {}) => ({ when: daysAgo(2), deleted: 0, measuredBytes: 0, failed: [], notes: [], ...over });
  assert.equal(L.runSummary(run(), NOW), "2 days ago: nothing was unused long enough to delete.");
  assert.equal(L.runSummary(run({ deleted: 1, measuredBytes: 3 * 1024 ** 3 }), NOW), "2 days ago: deleted 1 folder, free space went up by 3.0 GB.");
  assert.equal(
    L.runSummary(run({ when: NOW - 60, deleted: 4, measuredBytes: -5, failed: [{ path: "a", error: "b" }] }), NOW),
    "Today: deleted 4 folders, free space went up by 0 B. 1 folder could not be deleted.",
  );
});

test("the schedule button says how often it cleans", () => {
  assert.equal(L.scheduleLabel(null), "Schedule");
  assert.equal(L.scheduleLabel({ enabled: false, every: "week" }), "Schedule");
  assert.equal(L.scheduleLabel({ enabled: true, every: "month" }), "Cleans monthly");
});
