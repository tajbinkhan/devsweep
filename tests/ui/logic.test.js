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
