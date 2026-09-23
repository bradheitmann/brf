// The output folder: current briefs in the root, history in archive/.
//
//   <output>/registry.json
//   <output>/brf_<repo>_<yyyymmdd>.html     one current brief per project
//   <output>/meta_brf_<yyyymmdd>.html       one current meta brief
//   <output>/archive/                       every earlier brief, never deleted
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { readStamp } from "./html.mjs";
import { isBrief, parseName } from "./naming.mjs";

export const ARCHIVE_DIR = "archive";

export function archiveDir(outDir) {
  return join(outDir, ARCHIVE_DIR);
}

export function ensureLayout(outDir) {
  mkdirSync(archiveDir(outDir), { recursive: true });
}

function scan(dir, location) {
  if (!existsSync(dir)) return [];
  const out = [];
  for (const name of readdirSync(dir)) {
    if (name.startsWith(".")) continue;
    const path = join(dir, name);
    let isFile = false;
    try { isFile = statSync(path).isFile(); } catch { continue; }
    if (!isFile) continue;
    const parsed = parseName(name);
    out.push({ name, path, location, parsed });
  }
  return out;
}

export function listFiles(outDir) {
  return [...scan(outDir, "root"), ...scan(archiveDir(outDir), "archive")];
}

// Oldest first: by date, then earlier same-day copies (-v1, -v2) before the plain name.
function order(a, b) {
  if (a.parsed.date !== b.parsed.date) return a.parsed.date < b.parsed.date ? -1 : 1;
  const va = a.parsed.v ?? Infinity, vb = b.parsed.v ?? Infinity;
  if (va !== vb) return va < vb ? -1 : 1;
  return a.location === "archive" ? -1 : 1;
}

// Every brief of one project (key = repo name, or META_KEY for the meta brief), oldest first,
// each with its number.
// A stamped brief carries its own number; an unstamped one takes its position.
export function history(outDir, key) {
  const briefs = listFiles(outDir)
    .filter((f) => isBrief(f.parsed) && f.parsed.key === key)
    .sort(order);
  let last = 0;
  return briefs.map((f, i) => {
    let stamped = null;
    try { stamped = readStamp(readFileSync(f.path, "utf8")).brief; } catch { /* unreadable: treat as unstamped */ }
    const number = stamped ?? Math.max(last + 1, i + 1);
    last = Math.max(last, number);
    return { ...f, number, stamped: stamped !== null };
  });
}

export function nextNumber(outDir, key) {
  const h = history(outDir, key);
  return h.reduce((m, f) => Math.max(m, f.number), 0) + 1;
}

export function currentFiles(outDir, key) {
  return scan(outDir, "root").filter((f) => isBrief(f.parsed) && f.parsed.key === key);
}
