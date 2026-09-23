// The registry: the closed list of projects the owner has asked to have briefed.
// It lives in the output folder as registry.json, so every machine that syncs the
// folder shares one list. brf never adds a project on its own; `brf register`
// records the owner's own words as the reason.
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { join, resolve } from "node:path";
import { expandHome, writeJsonAtomic } from "./config.mjs";
import { BrfError } from "./errors.mjs";
import { REPO_NAME_RE } from "./naming.mjs";

export const REGISTRY_FILE = "registry.json";
export const REGISTRY_ABOUT =
  "Projects that get briefs. Add one only when the owner names it and asks for it, with `brf register --reason \"<their words>\"`. Never search the computer for projects.";

export function emptyRegistry() {
  return { brf_registry: 1, about: REGISTRY_ABOUT, projects: [] };
}

export function registryPath(outDir) {
  return join(outDir, REGISTRY_FILE);
}

export function loadRegistry(outDir) {
  const path = registryPath(outDir);
  if (!existsSync(path)) return { path, exists: false, ...emptyRegistry() };
  let data;
  try {
    data = JSON.parse(readFileSync(path, "utf8"));
  } catch (err) {
    throw new BrfError("BAD_REGISTRY", `${path} is not valid JSON: ${err.message}`);
  }
  if (!Array.isArray(data.projects)) throw new BrfError("BAD_REGISTRY", `${path} has no "projects" list.`);
  for (const p of data.projects) {
    if (!p || !REPO_NAME_RE.test(p.repo_name || "")) {
      throw new BrfError("BAD_REGISTRY", `${path} has an entry whose repo_name is not lower-case-hyphenated: ${JSON.stringify(p && p.repo_name)}`);
    }
  }
  return { path, exists: true, ...data };
}

function samePath(a, b) {
  if (!a || !b) return false;
  const norm = (p) => {
    const abs = resolve(expandHome(p));
    try { return realpathSync(abs); } catch { return abs; }
  };
  return norm(a) === norm(b);
}

// The whole repository path of a remote, after the host, lower case: "acme/tide" for
// github.com/acme/tide, "orga/payments/_git/api" for an Azure repo. Scheme, user, host, port
// and SSH host aliases are dropped, so every clone of one remote gives one key.
export function remoteKey(url) {
  if (!url) return null;
  const s = String(url).trim();
  let path;
  const withScheme = s.match(/^[a-z][a-z0-9+.-]*:\/\/(?:[^@/]*@)?[^/]+\/(.*)$/i);
  if (withScheme) path = withScheme[1];
  else {
    const scpLike = s.match(/^(?:[^@/:]+@)?[^/:]+:(.*)$/);
    path = scpLike ? scpLike[1] : s;
  }
  path = path.toLowerCase().replace(/\/+$/, "").replace(/\.git$/, "").replace(/^\/+/, "");
  // Hosts that spell one repository two ways: Azure DevOps (HTTPS "org/project/_git/repo",
  // SSH "v3/org/project/repo") and Bitbucket Server (HTTPS "scm/project/repo", SSH "project/repo").
  const parts = path.split("/");
  if (parts.length >= 3 && (parts[0] === "v3" || parts[0] === "scm")) parts.shift();
  const git = parts.indexOf("_git");
  if (git > 0 && git === parts.length - 2) parts.splice(git, 1);
  return parts.join("/") || null;
}

// Does this registry entry describe this checkout? An entry that records a path or a
// remote must match one of them; a name alone is not enough to be the same project.
export function sameProject(entry, { main_path, remote } = {}) {
  const hasIdentity = Boolean(entry.local_path || entry.remote);
  if (!hasIdentity) return true;
  if (entry.local_path && main_path && samePath(entry.local_path, main_path)) return true;
  if (entry.remote && remote && remoteKey(entry.remote) === remoteKey(remote)) return true;
  return false;
}

// Find the entry for a checkout: by recorded path, then by recorded remote (another clone,
// another computer), then by name when the entry's recorded identity also matches. Returns { project, conflict }: conflict names an entry
// that has this repository's name but belongs to a different checkout.
export function matchCheckout(registry, info) {
  const byPath = registry.projects.find((p) => p.local_path && info.main_path && samePath(p.local_path, info.main_path));
  if (byPath) {
    // Same folder, different repository (another computer, or the folder was reused).
    if (byPath.remote && info.remote && remoteKey(byPath.remote) !== remoteKey(info.remote)) return { project: null, conflict: byPath };
    return { project: byPath, conflict: null };
  }
  const key = remoteKey(info.remote);
  const byRemote = key ? registry.projects.find((p) => p.remote && remoteKey(p.remote) === key) : null;
  if (byRemote) return { project: byRemote, conflict: null };
  const byName = registry.projects.find((p) => p.repo_name === info.repo_name);
  if (!byName) return { project: null, conflict: null };
  if (sameProject(byName, info)) return { project: byName, conflict: null };
  return { project: null, conflict: byName };
}

// Look a project up by the name in a brief's file name.
export function findProject(registry, { repo_name } = {}) {
  return registry.projects.find((p) => p.repo_name === repo_name) || null;
}

export function addProject(outDir, entry) {
  const reason = String(entry.reason || "").trim();
  if (reason.length < 3) {
    throw new BrfError("NO_REASON", 'A project is registered only when the owner asks. Pass their words: --reason "<what they said>".');
  }
  if (!REPO_NAME_RE.test(entry.repo_name || "")) {
    throw new BrfError("BAD_NAME", `"${entry.repo_name}" is not a lower-case-hyphenated repository name.`);
  }
  const reg = loadRegistry(outDir);
  const existing = reg.projects.find(
    (p) =>
      p.repo_name === entry.repo_name ||
      (p.local_path && entry.local_path && samePath(p.local_path, entry.local_path)) ||
      (p.remote && entry.remote && remoteKey(p.remote) === remoteKey(entry.remote))
  );
  if (existing) throw new BrfError("ALREADY_REGISTERED", `This project is already registered as "${existing.repo_name}".`);
  const { path, exists, ...data } = reg;
  const record = {
    repo_name: entry.repo_name,
    everyday_name: entry.everyday_name || entry.repo_name,
    local_path: entry.local_path || null,
    remote: entry.remote || null,
    added: entry.added,
    reason,
  };
  data.projects = [...data.projects, record];
  writeJsonAtomic(path, data);
  return record;
}
