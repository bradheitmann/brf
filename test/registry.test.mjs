import assert from "node:assert/strict";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { writeJsonAtomic } from "../lib/config.mjs";
import { addProject, emptyRegistry, findProject, loadRegistry, matchCheckout, registryPath, remoteKey } from "../lib/registry.mjs";
import { tempDir } from "./helpers.mjs";

// Fake SSH remotes, joined at run time so they are not mistaken for addresses.
const AT = "@";

test("adds a project only with the owner's words, once", () => {
  const out = tempDir();
  writeJsonAtomic(registryPath(out), emptyRegistry());
  assert.throws(() => addProject(out, { repo_name: "tidewater", reason: "" }), /owner asks/);
  addProject(out, { repo_name: "tidewater", everyday_name: "Tidewater", reason: "Brief Tidewater weekly", added: "2026-11-02" });
  assert.throws(() => addProject(out, { repo_name: "tidewater", reason: "again please" }), /already registered/);
  assert.throws(() => addProject(out, { repo_name: "Bad_Name", reason: "please add" }), /lower-case/);
  const reg = loadRegistry(out);
  assert.equal(reg.projects.length, 1);
  assert.equal(reg.projects[0].reason, "Brief Tidewater weekly");
});

test("matches a checkout by its recorded path or remote, never by name alone", () => {
  const out = tempDir();
  const repo = join(out, "checkout");
  const elsewhere = join(out, "elsewhere");
  mkdirSync(repo);
  mkdirSync(elsewhere);
  writeJsonAtomic(registryPath(out), { ...emptyRegistry(), projects: [
    { repo_name: "pinned-name", local_path: repo },
    { repo_name: "tidewater", remote: `git${AT}github.com:acme/tidewater.git` },
    { repo_name: "bare-name" },
  ] });
  const reg = loadRegistry(out);
  assert.equal(findProject(reg, { repo_name: "pinned-name" }).repo_name, "pinned-name");
  assert.equal(matchCheckout(reg, { repo_name: "other", main_path: repo }).project.repo_name, "pinned-name");
  assert.equal(matchCheckout(reg, { repo_name: "pinned-name", main_path: elsewhere }).conflict.repo_name, "pinned-name");
  assert.equal(matchCheckout(reg, { repo_name: "tidewater", main_path: elsewhere, remote: "https://github.com/ACME/tidewater" }).project.repo_name, "tidewater");
  const other = matchCheckout(reg, { repo_name: "tidewater", main_path: elsewhere, remote: "https://github.com/someone-else/tidewater.git" });
  assert.equal(other.project, null);
  assert.equal(other.conflict.repo_name, "tidewater");
  assert.equal(matchCheckout(reg, { repo_name: "bare-name", main_path: elsewhere }).project.repo_name, "bare-name");
  assert.equal(matchCheckout(reg, { repo_name: "nobody", main_path: elsewhere }).project, null);
});

test("remote keys ignore scheme, host alias, case and .git", () => {
  for (const u of [`git${AT}github.com:Acme/Tide.git`, "https://github.com/acme/tide", `ssh://git${AT}github.com/acme/tide.git`, `git${AT}github-work:acme/tide.git`]) {
    assert.equal(remoteKey(u), "acme/tide", u);
  }
});

test("refuses to register the same checkout or remote twice under another name", () => {
  const out = tempDir();
  writeJsonAtomic(registryPath(out), emptyRegistry());
  addProject(out, { repo_name: "harbor-log", local_path: join(out, "harbor"), remote: `git${AT}github.com:acme/harbor-log.git`, reason: "add harbor" });
  assert.throws(() => addProject(out, { repo_name: "harbor-local", local_path: join(out, "harbor"), reason: "again" }), /already registered as "harbor-log"/);
  assert.throws(() => addProject(out, { repo_name: "harbor-2", remote: "https://github.com/acme/harbor-log", reason: "again" }), /already registered/);
});

test("a missing registry is empty, a broken one is an error", () => {
  const out = tempDir();
  assert.equal(loadRegistry(out).exists, false);
  writeJsonAtomic(registryPath(out), { projects: [{ repo_name: "NOPE" }] });
  assert.throws(() => loadRegistry(out), /lower-case-hyphenated/);
});
