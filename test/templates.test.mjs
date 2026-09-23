import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { brfVersion, currentTemplates } from "../lib/templates.mjs";
import { ROOT } from "./helpers.mjs";

test("every template declares a version, and the changelog records it", () => {
  const changelog = readFileSync(join(ROOT, "CHANGELOG.md"), "utf8");
  assert.match(changelog, new RegExp(`## brf ${brfVersion().replace(/\./g, "\\.")}\\b`));
  for (const [kind, t] of Object.entries(currentTemplates())) {
    assert.match(t.version || "", /^\d+\.\d+\.\d+$/, kind);
    assert.match(changelog, new RegExp(`${kind} ${t.version.replace(/\./g, "\\.")}`), `${kind} ${t.version} missing from CHANGELOG.md`);
  }
});

test("templates carry the fonts marker and the project template marks its appendices", () => {
  const project = readFileSync(join(ROOT, "templates", "project.html"), "utf8");
  const meta = readFileSync(join(ROOT, "templates", "meta.html"), "utf8");
  for (const t of [project, meta]) assert.ok(t.includes("/* {{FONTS_CSS}} */"));
  assert.ok(project.includes('<section data-brf="appendix">'));
});

function repoFiles(dir = ROOT, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === ".git" || name === "node_modules" || name === "proof") continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) repoFiles(p, out);
    else out.push(p);
  }
  return out;
}

test("nothing in the repository points at a personal machine or address", () => {
  const files = repoFiles();
  assert.ok(files.length >= 30, `found only ${files.length} files`);
  for (const f of files) {
    const text = readFileSync(f, "utf8");
    assert.equal(/\/Users\/[A-Za-z]|\/home\/[a-z]+\/|C:\\Users\\/.test(text), false, `${f} has a personal path`);
    if (f.endsWith("fonts.css")) continue;
    assert.equal(/[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+\.[A-Za-z0-9.-]*[a-z]{2,}/.test(text), false, `${f} has an email address`);
  }
});
