import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { build } from "../lib/build.mjs";
import { checkFolder, checkHtml } from "../lib/check.mjs";
import { writeJsonAtomic } from "../lib/config.mjs";
import { deliver } from "../lib/deliver.mjs";
import { ensureLayout } from "../lib/folder.mjs";
import { emptyRegistry, registryPath } from "../lib/registry.mjs";
import { ROOT, tempDir } from "./helpers.mjs";

const EXAMPLE = join(ROOT, "examples", "tidewater.filled.html");

function built() {
  const dir = tempDir();
  const r = build({ input: EXAMPLE, out: join(dir, "brf_tidewater_20261102.html"), example: true, brief: 2 });
  return readFileSync(r.path, "utf8");
}

test("the shipped example passes every check", () => {
  assert.deepEqual(checkHtml(built(), { name: "brf_tidewater_20261102.html" }), []);
});

test("catches scripts, network fetches, em-dashes and filler words", () => {
  const html = built();
  const bad = html
    .replace("</body>", '<script>alert(1)</script><img src="https://example.com/x.png"></body>')
    .replace("Tidewater is live", "Tidewater is honestly live — mostly");
  const problems = checkHtml(bad, { name: "brf_tidewater_20261102.html" }).join("\n");
  assert.match(problems, /script element/);
  assert.match(problems, /<img> element, which brf does not allow/);
  assert.match(problems, /em-dash/);
  assert.match(problems, /banned words: honestly/);
});

test("flags identifiers above the appendices but not inside them", () => {
  const html = built();
  assert.deepEqual(checkHtml(html, { name: "brf_tidewater_20261102.html" }).filter((p) => /above the appendices/.test(p)), []);
  const leaked = html.replace("Tidewater is live", "Tidewater (TW-12, commit 4f2a9c1, #31, src/time.ts, v2.1.0) is live");
  const problems = checkHtml(leaked, { name: "brf_tidewater_20261102.html" }).join("\n");
  for (const what of ["ticket or task id", "commit hash", "pull request", "file path", "version string"]) assert.match(problems, new RegExp(what));
});

test("checks the folder: names, registry, one current brief each", () => {
  const out = tempDir();
  ensureLayout(out);
  writeJsonAtomic(registryPath(out), { ...emptyRegistry(), projects: [{ repo_name: "tidewater" }, { repo_name: "harbor-log" }] });
  const r = build({ input: EXAMPLE, out: join(tempDir(), "brf_tidewater_20261102.html"), outputDir: out });
  deliver({ file: r.path, outputDir: out, cacheDir: tempDir() });
  let result = checkFolder(out, { today: "20261102" });
  assert.deepEqual(result.problems, []);
  assert.match(result.notes.join("\n"), /harbor-log: registered, no current brief/);
  writeFileSync(join(out, "notes.txt"), "x");
  writeFileSync(join(out, "brf_stranger_20261102.html"), "x");
  result = checkFolder(out, { today: "20261102" });
  assert.match(result.problems.join("\n"), /notes\.txt: not a brf file name/);
  assert.match(result.problems.join("\n"), /"stranger" is not in the registry/);
});
