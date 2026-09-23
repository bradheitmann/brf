// File names. One convention, parsed and produced in one place.
//
//   brf_<repo-name>_<yyyymmdd>.html          a project brief
//   meta_brf_<yyyymmdd>.html                 the meta brief across all registered projects
//   ...<yyyymmdd>-v<N>...                    an earlier brief from the same day, in the archive
//   ..._<variant>.<ext>                      another format of the same brief, e.g. _dark.pdf
//
// <repo-name> is lower case, words joined by hyphens. Underscores only separate fields.
export const REPO_NAME_RE = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
// The meta brief's history key. It contains a character no repository name can, so a
// project that happens to be called "meta" never shares numbering or filing with it.
export const META_KEY = "@meta";
const SEG = "[a-z0-9]+(?:-[a-z0-9]+)*";
const PROJECT_RE = new RegExp(`^brf_(${SEG})_(\\d{8})(?:-v(\\d+))?(?:_(${SEG}))?\\.([a-z0-9]+)$`);
const META_RE = new RegExp(`^meta_brf_(\\d{8})(?:-v(\\d+))?(?:_(${SEG}))?\\.([a-z0-9]+)$`);

export function isRealDate(yyyymmdd) {
  if (!/^\d{8}$/.test(yyyymmdd)) return false;
  const y = +yyyymmdd.slice(0, 4), m = +yyyymmdd.slice(4, 6), d = +yyyymmdd.slice(6, 8);
  const dt = new Date(Date.UTC(y, m - 1, d));
  return dt.getUTCFullYear() === y && dt.getUTCMonth() === m - 1 && dt.getUTCDate() === d;
}

export function parseName(base) {
  let m = base.match(PROJECT_RE);
  if (m) {
    const [, repo, date, v, variant, ext] = m;
    if (!isRealDate(date)) return null;
    return { kind: "project", key: repo, repo, date, v: v ? +v : null, variant: variant || null, ext };
  }
  m = base.match(META_RE);
  if (m) {
    const [, date, v, variant, ext] = m;
    if (!isRealDate(date)) return null;
    return { kind: "meta", key: META_KEY, repo: null, date, v: v ? +v : null, variant: variant || null, ext };
  }
  return null;
}

export function formatName({ kind, repo, date, v = null, variant = null, ext = "html" }) {
  const stem = kind === "meta" ? `meta_brf_${date}` : `brf_${repo}_${date}`;
  return `${stem}${v ? `-v${v}` : ""}${variant ? `_${variant}` : ""}.${ext}`;
}

// A brief proper: the HTML page, not another format of it.
export function isBrief(parsed) {
  return Boolean(parsed) && parsed.ext === "html" && !parsed.variant;
}

export function toRepoName(raw) {
  return String(raw || "")
    .toLowerCase()
    .replace(/\.git$/, "")
    .replace(/[\s_.]+/g, "-")
    .replace(/[^a-z0-9-]/g, "")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

export function todayStamp(d = new Date()) {
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}`;
}

export function humanDate(yyyymmdd) {
  const months = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
  return `${+yyyymmdd.slice(6, 8)} ${months[+yyyymmdd.slice(4, 6) - 1]} ${yyyymmdd.slice(0, 4)}`;
}
