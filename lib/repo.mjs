// Read-only facts about the git repository the agent is standing in.
// Every git call passes --no-optional-locks, so even `git status` never rewrites the index.
import { execFileSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { basename, dirname, isAbsolute, resolve } from "node:path";
import { BrfError } from "./errors.mjs";
import { toRepoName } from "./naming.mjs";

export function git(cwd, args) {
  try {
    return execFileSync("git", ["--no-optional-locks", ...args], {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
      maxBuffer: 256 * 1024 * 1024,
    }).replace(/\n$/, "");
  } catch {
    return null;
  }
}

// Remote URLs can carry a user name or token. Never pass one on.
export function redactRemote(url) {
  if (!url) return url;
  return url.replace(/^([a-z][a-z0-9+.-]*:\/\/)[^@/]+@/i, "$1");
}

export function nameFromRemote(url) {
  if (!url) return null;
  const last = url.replace(/[\\/]+$/, "").split(/[/:]/).pop();
  const name = toRepoName(last);
  return name || null;
}

export function repoInfo(cwd = process.cwd()) {
  const toplevel = git(cwd, ["rev-parse", "--show-toplevel"]);
  if (!toplevel) throw new BrfError("NOT_A_REPO", `${cwd} is not inside a git repository. Run brf from the project you want briefed.`);
  let common = git(toplevel, ["rev-parse", "--git-common-dir"]);
  if (common && !isAbsolute(common)) common = resolve(toplevel, common);
  // In a linked worktree the common dir is the main checkout's .git. Brief the project, not the worktree.
  const main = common && basename(common) === ".git" ? dirname(common) : toplevel;
  const remotes = (git(toplevel, ["remote"]) || "").split("\n").filter(Boolean);
  const remoteName = remotes.includes("origin") ? "origin" : remotes[0] || null;
  const remote = remoteName ? redactRemote(git(toplevel, ["remote", "get-url", remoteName])) : null;
  const fromRemote = nameFromRemote(remote);
  const fromFolder = toRepoName(basename(main));
  let mainPath = main;
  try { mainPath = realpathSync(main); } catch { /* keep the unresolved path */ }
  return {
    toplevel,
    main_path: mainPath,
    is_linked_worktree: resolve(toplevel) !== resolve(main),
    remote,
    repo_name: fromRemote || fromFolder,
    repo_name_source: fromRemote ? `remote ${remoteName}` : "folder name",
    head: git(toplevel, ["rev-parse", "HEAD"]),
    branch: git(toplevel, ["rev-parse", "--abbrev-ref", "HEAD"]),
  };
}
