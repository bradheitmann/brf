import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { build } from "../lib/build.mjs";
import { writeJsonAtomic } from "../lib/config.mjs";
import { deliver } from "../lib/deliver.mjs";
import { ensureLayout, history, nextNumber } from "../lib/folder.mjs";
import { emptyRegistry, registryPath } from "../lib/registry.mjs";
import { ROOT, tempDir } from "./helpers.mjs";

const EXAMPLE = join(ROOT, "examples", "tidewater.filled.html");

function setup() {
  const out = tempDir();
  ensureLayout(out);
  writeJsonAtomic(registryPath(out), { ...emptyRegistry(), projects: [{ repo_name: "tidewater" }, { repo_name: "harbor-log" }] });
  return { out, work: tempDir(), cache: tempDir() };
}

function buildAndDeliver({ out, work, cache }, date) {
  const r = build({ input: EXAMPLE, out: join(work, `brf_tidewater_${date}.html`), outputDir: out });
  return deliver({ file: r.path, outputDir: out, cacheDir: cache });
}

test("files the previous brief in the archive and numbers each brief", () => {
  const ctx = setup();
  const first = buildAndDeliver(ctx, "20261012");
  assert.equal(first.brief, 1);
  assert.deepEqual(first.archived, []);
  const second = buildAndDeliver(ctx, "20261102");
  assert.equal(second.brief, 2);
  assert.deepEqual(second.archived, [{ from: "brf_tidewater_20261012.html", to: "archive/brf_tidewater_20261012.html" }]);
  assert.deepEqual(readdirSync(ctx.out).filter((n) => n.endsWith(".html")), ["brf_tidewater_20261102.html"]);
  assert.equal(nextNumber(ctx.out, "tidewater"), 3);
});

test("a same-day rebrief archives the earlier one as -v1, then -v2", () => {
  const ctx = setup();
  buildAndDeliver(ctx, "20261102");
  const b = buildAndDeliver(ctx, "20261102");
  assert.equal(b.archived[0].to, "archive/brf_tidewater_20261102-v1.html");
  const c = buildAndDeliver(ctx, "20261102");
  assert.equal(c.archived[0].to, "archive/brf_tidewater_20261102-v2.html");
  assert.deepEqual(history(ctx.out, "tidewater").map((f) => f.number), [1, 2, 3]);
});

test("touches no other project's files", () => {
  const ctx = setup();
  writeFileSync(join(ctx.out, "brf_harbor-log_20261001.html"), "<p>other</p>");
  buildAndDeliver(ctx, "20261102");
  buildAndDeliver(ctx, "20261103");
  assert.equal(readFileSync(join(ctx.out, "brf_harbor-log_20261001.html"), "utf8"), "<p>other</p>");
});

test("refuses a stale brief number, an unbuilt file and an older date", () => {
  const ctx = setup();
  const early = build({ input: EXAMPLE, out: join(ctx.work, "brf_tidewater_20261101.html"), outputDir: ctx.out });
  const late = build({ input: EXAMPLE, out: join(ctx.work, "brf_tidewater_20261102.html"), outputDir: ctx.out });
  deliver({ file: late.path, outputDir: ctx.out, cacheDir: ctx.cache });
  assert.throws(() => deliver({ file: early.path, outputDir: ctx.out, cacheDir: ctx.cache }), /next brief/);
  const raw = join(ctx.work, "brf_tidewater_20261104.html");
  writeFileSync(raw, "<html></html>");
  assert.throws(() => deliver({ file: raw, outputDir: ctx.out, cacheDir: ctx.cache }), /no complete brf stamp/);
  const older = build({ input: EXAMPLE, out: join(ctx.work, "brf_tidewater_20261030.html"), outputDir: ctx.out });
  assert.throws(() => deliver({ file: older.path, outputDir: ctx.out, cacheDir: ctx.cache }), /later date/);
  assert.equal(existsSync(join(ctx.out, "brf_tidewater_20261102.html")), true);
});
