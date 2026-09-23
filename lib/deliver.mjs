// Put a finished brief in the output folder and move that project's previous brief
// into archive/. Only files of the same project are touched. Nothing is overwritten
// and nothing is deleted.
import { constants, copyFileSync, existsSync, mkdirSync, readFileSync, renameSync, rmdirSync, statSync } from "node:fs";
import { basename, join } from "node:path";
import { createHash } from "node:crypto";
import { BrfError } from "./errors.mjs";
import { archiveDir, currentFiles, ensureLayout, nextNumber } from "./folder.mjs";
import { markupProblems, stampMismatch } from "./check.mjs";
import { readStamp } from "./html.mjs";
import { formatName, parseName } from "./naming.mjs";
import { findProject, loadRegistry } from "./registry.mjs";

function sleep(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

// A lock per output folder, kept on this machine, so two agents delivering at once take turns.
export function withLock(cacheDir, outDir, fn, { waitMs = 15000, staleMs = 120000 } = {}) {
  const lock = join(cacheDir, "locks", `${createHash("sha256").update(outDir).digest("hex").slice(0, 16)}.lock`);
  mkdirSync(join(cacheDir, "locks"), { recursive: true });
  const deadline = Date.now() + waitMs;
  for (;;) {
    try {
      mkdirSync(lock);
      break;
    } catch (err) {
      if (err.code !== "EEXIST") throw err;
      try {
        if (Date.now() - statSync(lock).mtimeMs > staleMs) { rmdirSync(lock); continue; }
      } catch { /* the other holder just released it */ }
      if (Date.now() > deadline) throw new BrfError("LOCKED", `Another delivery holds ${lock}. Try again in a minute.`);
      sleep(200);
    }
  }
  try {
    return fn();
  } finally {
    try { rmdirSync(lock); } catch { /* already gone */ }
  }
}

// The archive name for a file leaving the root. A same-day name that is taken, or that
// the incoming brief will use, becomes -v1, -v2 … in the order the briefs were made.
export function archiveNameFor(outDir, parsed, incomingDate) {
  const arch = archiveDir(outDir);
  const plain = formatName({ ...parsed, v: null });
  if (parsed.date !== incomingDate && !existsSync(join(arch, plain))) return plain;
  for (let v = 1; v < 1000; v++) {
    const name = formatName({ ...parsed, v });
    if (!existsSync(join(arch, name))) return name;
  }
  throw new BrfError("ARCHIVE_FULL", `Too many same-day copies of ${plain}.`);
}

export function deliver({ file, outputDir, cacheDir }) {
  const name = basename(file);
  const parsed = parseName(name);
  if (!parsed || parsed.ext !== "html" || parsed.v || parsed.variant) throw new BrfError("BAD_NAME", `"${name}" is not a deliverable brief name.`);
  if (!existsSync(file)) throw new BrfError("NOT_FOUND", `Not found: ${file}`);
  const html = readFileSync(file, "utf8");
  const stamp = readStamp(html);
  if (!stamp.brf_version || !stamp.brief || !stamp.template) throw new BrfError("NOT_BUILT", `${name} has no complete brf stamp. Deliver only what brf build produced.`);
  const markup = markupProblems(html);
  if (markup.length) throw new BrfError("UNSAFE_MARKUP", `${name} ${markup.join("; ")}. Remove it, rebuild, and run brf check.`);
  const mismatch = stampMismatch(stamp, parsed);
  if (mismatch.length) throw new BrfError("STAMP_MISMATCH", `${name} ${mismatch.join("; ")}. Deliver the file under the name brf build gave it.`);
  if (parsed.kind === "project") {
    if (!findProject(loadRegistry(outputDir), { repo_name: parsed.repo })) {
      throw new BrfError("NOT_REGISTERED", `"${parsed.repo}" is not in the registry. Briefs are made only for projects the owner has asked for.`);
    }
  }
  ensureLayout(outputDir);
  return withLock(cacheDir, outputDir, () => {
    const expected = nextNumber(outputDir, parsed.key);
    if (stamp.brief !== expected) {
      throw new BrfError("BRIEF_NUMBER", `${name} is stamped brief ${stamp.brief}, but the next brief for this project is ${expected}. Another brief landed first; rebuild.`);
    }
    const moved = [];
    for (const f of currentFiles(outputDir, parsed.key)) {
      if (f.parsed.date > parsed.date) {
        throw new BrfError("NEWER_EXISTS", `${f.name} describes a later date than ${name}. Refusing to archive a newer brief.`);
      }
      const target = join(archiveDir(outputDir), archiveNameFor(outputDir, f.parsed, parsed.date));
      if (existsSync(target)) throw new BrfError("EXISTS", `${target} already exists.`);
      renameSync(f.path, target);
      moved.push({ from: f.name, to: `archive/${basename(target)}` });
    }
    const dest = join(outputDir, name);
    copyFileSync(file, dest, constants.COPYFILE_EXCL);
    return { delivered: dest, brief: stamp.brief, archived: moved };
  });
}
