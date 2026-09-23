import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { cli, gitRepo, ROOT, tempDir } from "./helpers.mjs";

test("prints versions", () => {
  const r = cli(["version"], { cwd: ROOT, home: tempDir() });
  assert.equal(r.code, 0);
  assert.match(r.stdout, /^brf \d+\.\d+\.\d+ · project template \d+\.\d+\.\d+ · meta template \d+\.\d+\.\d+/);
});

test("context: no folder, then unregistered, then registered", () => {
  const home = tempDir();
  const repo = gitRepo(join(tempDir(), "tidewater"));
  let r = cli(["context"], { cwd: repo, home });
  assert.equal(r.code, 2);
  assert.match(r.stderr, /No output folder is set/);

  const out = join(tempDir(), "daily_brf");
  assert.equal(cli(["init", "--output", out], { cwd: repo, home }).code, 0);
  r = cli(["context"], { cwd: repo, home });
  assert.equal(r.code, 0);
  assert.match(JSON.parse(r.stdout).stop, /not in the registry/);

  assert.equal(cli(["register", "--reason", ""], { cwd: repo, home }).code, 2);
  assert.equal(cli(["register", "--reason", "Please brief Tidewater", "--everyday-name", "Tidewater"], { cwd: repo, home }).code, 0);
  const ctx = JSON.parse(cli(["context"], { cwd: repo, home }).stdout);
  assert.equal(ctx.registered, true);
  assert.equal(ctx.brief_number, 1);
  assert.match(ctx.output_name, /^brf_tidewater_\d{8}\.html$/);
  assert.equal(ctx.work_dir.startsWith(repo), false);
  const reg = JSON.parse(readFileSync(join(out, "registry.json"), "utf8"));
  assert.equal(reg.projects[0].reason, "Please brief Tidewater");
});

test("init refuses to repoint a configured computer without --force", () => {
  const home = tempDir();
  assert.equal(cli(["init", "--output", join(tempDir(), "a")], { cwd: ROOT, home }).code, 0);
  assert.equal(cli(["init", "--output", join(tempDir(), "b")], { cwd: ROOT, home }).code, 2);
  assert.equal(cli(["init", "--output", join(tempDir(), "b"), "--force"], { cwd: ROOT, home }).code, 0);
});

test("snapshot refuses to write inside the repository", () => {
  const home = tempDir();
  const repo = gitRepo(join(tempDir(), "snapper"));
  const r = cli(["snapshot", "--out", join(repo, "before.json")], { cwd: repo, home });
  assert.equal(r.code, 2);
  assert.match(r.stderr, /outside the repository/);
});
