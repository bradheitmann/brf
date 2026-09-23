import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { redactRemote, repoInfo } from "../lib/repo.mjs";
import { compareSnapshot, saveSnapshot, takeSnapshot } from "../lib/snapshot.mjs";
import { gitRepo, tempDir } from "./helpers.mjs";

// Fake credentials, joined at run time so the fixture is not mistaken for an address.
const AT = "@";
test("names the repository from its remote and strips credentials", () => {
  const dir = gitRepo(join(tempDir(), "local-folder"), { remote: `https://someone:secret${AT}github.com/acme/Tide_Water.git` });
  const info = repoInfo(dir);
  assert.equal(info.repo_name, "tide-water");
  assert.equal(info.remote, "https://github.com/acme/Tide_Water.git");
  const ssh = `git${AT}github.com:acme/x.git`;
  assert.equal(redactRemote(ssh), ssh);
});

test("falls back to the folder name without a remote", () => {
  const dir = gitRepo(join(tempDir(), "My_Project"));
  assert.equal(repoInfo(dir).repo_name, "my-project");
});

test("a linked worktree briefs the main project", () => {
  const main = gitRepo(join(tempDir(), "harbor-log"));
  const wt = join(tempDir(), "harbor-log-feature");
  execFileSync("git", ["worktree", "add", "-q", "-b", "feature", wt], { cwd: main, stdio: "ignore" });
  const info = repoInfo(wt);
  assert.equal(info.is_linked_worktree, true);
  assert.equal(info.repo_name, "harbor-log");
});

test("snapshots match when nothing changed and differ when something did", () => {
  const dir = gitRepo(join(tempDir(), "snap"));
  const file = join(tempDir(), "before.json");
  saveSnapshot(file, takeSnapshot(dir));
  assert.equal(compareSnapshot(file, dir).same, true);
  writeFileSync(join(dir, "new.txt"), "x");
  assert.equal(compareSnapshot(file, dir).same, false);
});
