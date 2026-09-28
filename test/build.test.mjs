import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { build } from "../lib/build.mjs";
import { writeJsonAtomic } from "../lib/config.mjs";
import { readStamp } from "../lib/html.mjs";
import { emptyRegistry, registryPath } from "../lib/registry.mjs";
import { ROOT, tempDir } from "./helpers.mjs";

const EXAMPLE = join(ROOT, "examples", "tidewater.filled.html");

test("builds the example: fonts in, comments out, versions stamped", () => {
  const dir = tempDir();
  const r = build({ input: EXAMPLE, out: join(dir, "brf_tidewater_20261102.html"), example: true, brief: 2 });
  const html = readFileSync(r.path, "utf8");
  assert.equal(html.includes("<!--"), false);
  assert.equal(/\{\{[A-Z0-9_]+\}\}/.test(html), false);
  assert.match(html, /@font-face/);
  const stamp = readStamp(html);
  assert.equal(stamp.brief, 2);
  assert.equal(stamp.repo, "tidewater");
  assert.equal(stamp.template, "project 2.0.0");
  assert.equal(stamp.brf_version, JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8")).version);
  assert.match(html, /Brief 2 of this project · brf \d+\.\d+\.\d+ · project template 2\.0\.0/);
});

test("refuses unfilled slots, a missing template tag, a bad name and a kind mismatch", () => {
  const dir = tempDir();
  const out = join(dir, "brf_tidewater_20261102.html");
  assert.throws(() => build({ input: join(ROOT, "templates", "project.html"), out, example: true }), /still to do/);
  const noTag = join(dir, "no-tag.filled.html");
  writeFileSync(noTag, readFileSync(EXAMPLE, "utf8").replace(/<meta name="brf-template"[^>]*>/, ""));
  assert.throws(() => build({ input: noTag, out, example: true }), /brf-template/);
  assert.throws(() => build({ input: EXAMPLE, out: join(dir, "tidewater.html"), example: true }), /not a brief name/);
  assert.throws(() => build({ input: EXAMPLE, out: join(dir, "meta_brf_20261102.html"), example: true }), /cannot become/);
});

test("refuses an unregistered project and numbers from the output folder", () => {
  const out = tempDir();
  writeJsonAtomic(registryPath(out), emptyRegistry());
  const dir = tempDir();
  assert.throws(() => build({ input: EXAMPLE, out: join(dir, "brf_tidewater_20261102.html"), outputDir: out }), /not in the registry/);
  writeJsonAtomic(registryPath(out), { ...emptyRegistry(), projects: [{ repo_name: "tidewater" }] });
  const r = build({ input: EXAMPLE, out: join(dir, "brf_tidewater_20261102.html"), outputDir: out });
  assert.equal(r.brief, 1);
});

test("refuses to rebuild a finished page", () => {
  const dir = tempDir();
  const first = build({ input: EXAMPLE, out: join(dir, "brf_tidewater_20261102.html"), example: true });
  assert.throws(() => build({ input: first.path, out: join(dir, "brf_tidewater_20261103.html"), example: true }), /already a finished brief/);
});
