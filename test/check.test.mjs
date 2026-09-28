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

test("a radio-and-label theme switch passes; other inputs, forms and buttons do not", () => {
  const html = built();
  const name = "brf_tidewater_20261102.html";
  const toggle =
    '<input type="radio" name="t" id="t-light" class="vh" checked aria-label="Light theme">' +
    '<input type="radio" name="t" id="t-dark" class="vh" aria-label="Dark theme">' +
    '<label for="t-dark" class="theme" title="Switch" aria-label="Switch to dark">Light</label>';
  assert.deepEqual(checkHtml(html.replace("</main>", `${toggle}</main>`), { name }), []);
  assert.deepEqual(checkHtml(html.replace("</main>", '<input type="RADIO" name="t" id="x"></main>'), { name }), []);
  const refused = {
    "text input": '<input type="text" name="q">',
    "image input": '<input type="image" src="https://example.com/a.png">',
    "file input": '<input type="file">',
    "submit input": '<input type="submit" formaction="https://example.com">',
    "input without a type": '<input name="q">',
    "two types": '<input type="radio" type="image" src="https://example.com/a.png">',
    "radio with src": '<input type="radio" src="https://example.com/a.png">',
    "radio with a value": '<input type="radio" name="t" value="x">',
    "radio with style": '<input type="radio" style="width:1px">',
    "radio with form": '<input type="radio" form="f">',
    "radio with an event": '<input type="radio" onchange="fetch(1)">',
    "label with an event": '<label for="t" onclick="fetch(1)">x</label>',
    "label with a style": '<label for="t" style="width:1px">x</label>',
    "label with form": '<label for="t" form="f">x</label>',
    form: '<form action="https://example.com"><input type="radio"></form>',
    button: '<button type="button">x</button>',
    select: "<select><option>x</option></select>",
    textarea: "<textarea>x</textarea>",
  };
  for (const [what, snippet] of Object.entries(refused)) {
    assert.ok(checkHtml(html.replace("</main>", `${snippet}</main>`), { name }).length > 0, `${what} was not caught`);
  }
});

test("the build writes the brief number into the page", () => {
  const html = built();
  assert.equal(html.includes("{{BRIEF_NUMBER}}"), false);
  assert.match(html, /Brief 2</);
});

test("the shipped meta example builds and passes every check", () => {
  const dir = tempDir();
  const r = build({ input: join(ROOT, "examples", "meta.filled.html"), out: join(dir, "meta_brf_20261102.html"), example: true });
  assert.deepEqual(checkHtml(readFileSync(r.path, "utf8"), { name: "meta_brf_20261102.html" }), []);
});
