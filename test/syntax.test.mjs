import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { ROOT, tempDir } from "./helpers.mjs";

// Every .mjs file under the given folders, and the ones Node cannot parse.
function unparsable(root, dirs) {
  const files = dirs.flatMap((d) => readdirSync(join(root, d)).filter((f) => f.endsWith(".mjs")).map((f) => join(root, d, f)));
  const bad = files.filter((f) => spawnSync(process.execPath, ["--check", f]).status !== 0);
  return { files, bad };
}

test("every shipped module parses", () => {
  const { files, bad } = unparsable(ROOT, ["bin", "lib", "test"]);
  assert.ok(files.length >= 20, `found only ${files.length} modules`);
  assert.deepEqual(bad, []);
});

test("the scan reports a module that does not parse", () => {
  const root = tempDir();
  mkdirSync(join(root, "lib"));
  writeFileSync(join(root, "lib", "good.mjs"), "export const x = 1;\n");
  writeFileSync(join(root, "lib", "broken.mjs"), "export const x = ;\n");
  const { files, bad } = unparsable(root, ["lib"]);
  assert.equal(files.length, 2);
  assert.deepEqual(bad, [join(root, "lib", "broken.mjs")]);
});
