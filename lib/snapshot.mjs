// Proof that briefing a repository changed nothing in it.
//
// A snapshot hashes everything an agent could disturb: HEAD and the current branch, staged
// and unstaged changes (their content, not only their paths), the content of untracked files,
// ignored paths, every ref (branches, tags, remote-tracking refs, the stash), the stash list,
// the worktree list, and HEAD's reflog. All of it is read with --no-optional-locks and nothing is written.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readlinkSync, realpathSync } from "node:fs";
import { join } from "node:path";
import { writeJsonAtomic } from "./config.mjs";
import { BrfError } from "./errors.mjs";
import { repoInfo } from "./repo.mjs";

function gitBytes(cwd, args, { allowFail = false } = {}) {
  // log.showSignature would make log-style reads verify every signature: slow, and its text
  // can change when nothing in the repository did.
  const r = spawnSync("git", ["--no-optional-locks", "-c", "log.showSignature=false", ...args], { cwd, maxBuffer: 1024 ** 3 });
  if (r.status !== 0) {
    if (allowFail) return Buffer.alloc(0);
    throw new BrfError("GIT_FAILED", `git ${args.join(" ")} failed; no snapshot taken.`);
  }
  return r.stdout;
}

function untrackedDigest(cwd) {
  const h = createHash("sha256");
  const list = gitBytes(cwd, ["ls-files", "--others", "--exclude-standard", "-z"]).toString("utf8").split("\0").filter(Boolean).sort();
  for (const rel of list) {
    const p = join(cwd, rel);
    h.update(`${rel}\0`);
    try {
      const st = lstatSync(p);
      if (st.isSymbolicLink()) h.update(`link:${readlinkSync(p)}`);
      else if (st.isFile()) h.update(readFileSync(p));
      else h.update(`other:${st.mode}`);
    } catch {
      h.update("unreadable");
    }
    h.update("\0");
  }
  return { digest: h.digest("hex"), count: list.length };
}

export function takeSnapshot(cwd = process.cwd()) {
  const info = repoInfo(cwd);
  const top = info.toplevel;
  const hasHead = Boolean(info.head);
  const parts = {
    head: Buffer.from(info.head || "unborn"),
    branch: gitBytes(top, ["symbolic-ref", "-q", "HEAD"], { allowFail: true }),
    status: gitBytes(top, ["status", "--porcelain=v1", "-z", "--ignored"]),
    staged: gitBytes(top, hasHead ? ["diff", "--cached", "--binary", "HEAD"] : ["diff", "--cached", "--binary"]),
    unstaged: gitBytes(top, ["diff", "--binary"]),
    refs: gitBytes(top, ["for-each-ref", "--format=%(refname) %(objectname)"]),
    stashes: gitBytes(top, ["stash", "list", "--format=%H"], { allowFail: true }),
    worktrees: gitBytes(top, ["worktree", "list", "--porcelain"]),
    // Round trips (switch away and back, stash then pop) leave the tree as it was but write
    // HEAD's reflog, so the reflog is part of the proof.
    reflog: gitBytes(top, ["reflog", "show", "--format=%H%x00%gs", "HEAD"], { allowFail: true }),
  };
  const untracked = untrackedDigest(top);
  const combined = createHash("sha256");
  const digests = {};
  for (const [k, v] of Object.entries(parts)) {
    digests[k] = createHash("sha256").update(v).digest("hex");
    combined.update(`${k}:${digests[k]}\n`);
  }
  digests.untracked_content = untracked.digest;
  combined.update(`untracked_content:${untracked.digest}\n`);
  let repo = top;
  try { repo = realpathSync(top); } catch { /* keep as is */ }
  return {
    brf_snapshot: 1,
    repo,
    head: info.head,
    branch: parts.branch.toString("utf8").trim() || null,
    untracked_files: untracked.count,
    digests,
    digest: combined.digest("hex"),
    taken_at: new Date().toISOString(),
  };
}

export function saveSnapshot(path, snap) {
  writeJsonAtomic(path, snap);
}

export function compareSnapshot(beforePath, cwd = process.cwd()) {
  let before;
  try {
    before = JSON.parse(readFileSync(beforePath, "utf8"));
  } catch (err) {
    throw new BrfError("NO_SNAPSHOT", `Cannot read the earlier snapshot ${beforePath}: ${err.message}`);
  }
  if (before.brf_snapshot !== 1 || !before.digest) throw new BrfError("OLD_SNAPSHOT", `${beforePath} was not made by this version of brf snapshot. Take a new one.`);
  const after = takeSnapshot(cwd);
  if (before.repo !== after.repo) throw new BrfError("OTHER_REPO", `The snapshot was taken in ${before.repo}, not ${after.repo}.`);
  const changed = Object.keys(after.digests).filter((k) => before.digests[k] !== after.digests[k]);
  return { same: changed.length === 0 && before.digest === after.digest, changed, before, after };
}
