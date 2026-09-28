import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { build } from "../lib/build.mjs";
import { templatePath, templatesDir, TEMPLATES_DIR } from "../lib/templates.mjs";
import { ROOT, tempDir } from "./helpers.mjs";

test("an owner templates folder replaces the built-in templates and fonts, file by file", () => {
  const own = tempDir();
  const env = { BRF_TEMPLATES_DIR: own, HOME: tempDir() };
  assert.equal(templatePath("project.html", env), join(TEMPLATES_DIR, "project.html"), "falls back while the folder is empty");
  writeFileSync(join(own, "project.html"), readFileSync(join(ROOT, "templates", "project.html"), "utf8"));
  writeFileSync(join(own, "fonts.css"), "/* owner fonts */");
  assert.equal(templatesDir(env), own);
  assert.equal(templatePath("project.html", env), join(own, "project.html"));
  assert.equal(templatePath("meta.html", env), join(TEMPLATES_DIR, "meta.html"), "a kind the folder lacks stays built in");
  assert.equal(templatePath("fonts.css", env), join(own, "fonts.css"));
});

test("a build embeds the owner's fonts", () => {
  const own = tempDir();
  writeFileSync(join(own, "fonts.css"), "/* owner-fonts-marker */");
  const prev = process.env.BRF_TEMPLATES_DIR;
  process.env.BRF_TEMPLATES_DIR = own;
  try {
    const r = build({ input: join(ROOT, "examples", "tidewater.filled.html"), out: join(tempDir(), "brf_tidewater_20261102.html"), example: true });
    assert.match(readFileSync(r.path, "utf8"), /owner-fonts-marker/);
  } finally {
    if (prev === undefined) delete process.env.BRF_TEMPLATES_DIR; else process.env.BRF_TEMPLATES_DIR = prev;
  }
});
