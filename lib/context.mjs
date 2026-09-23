// Everything an agent needs to start a brief, worked out once so it never guesses:
// which project this is, whether the owner registered it, the file name, the brief
// number, the earlier briefs to read, where to keep working files, and what tools exist.
import { execFileSync } from "node:child_process";
import { accessSync, constants, existsSync, mkdirSync, readFileSync, realpathSync, statSync } from "node:fs";
import { delimiter, dirname, join, resolve } from "node:path";
import { requireOutputDir } from "./config.mjs";
import { currentFiles, history, nextNumber } from "./folder.mjs";
import { formatName, humanDate, META_KEY, todayStamp } from "./naming.mjs";
import { loadRegistry, matchCheckout } from "./registry.mjs";
import { repoInfo } from "./repo.mjs";
import { brfVersion, currentTemplates } from "./templates.mjs";

function toolVersion(cmd, args) {
  try {
    return execFileSync(cmd, args, { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], timeout: 15000 }).trim().split("\n")[0];
  } catch {
    return null;
  }
}

export function findOnPath(name, pathVar = process.env.PATH || "") {
  for (const dir of pathVar.split(delimiter).filter(Boolean)) {
    const candidate = join(dir, name);
    try {
      accessSync(candidate, constants.X_OK);
      return candidate;
    } catch { /* keep looking */ }
  }
  return null;
}

export const MISTER_CLEAN_PACKAGE = "@bradheitmann/mister-clean";

// Package managers install small shell scripts that point at the real file. Follow them.
function shimTarget(binPath) {
  try {
    if (statSync(binPath).size > 65536) return null;
    const text = readFileSync(binPath, "utf8");
    const marked = text.match(/#\s*cmd-shim-target=(.+)$/m);
    if (marked) return marked[1].trim();
    const rel = text.match(/\$basedir\/([^"'\s]+\.(?:js|mjs|cjs))/);
    return rel ? resolve(dirname(binPath), rel[1]) : null;
  } catch {
    return null;
  }
}

// The version of an installed command, read only from the package.json of the named package.
export function packageVersionOf(binPath, packageName) {
  for (const start of [binPath, shimTarget(binPath)].filter(Boolean)) {
    try {
      let dir = dirname(realpathSync(start));
      for (let i = 0; i < 6; i++) {
        const pkg = join(dir, "package.json");
        if (existsSync(pkg)) {
          const data = JSON.parse(readFileSync(pkg, "utf8"));
          if (data.name === packageName) return data.version || null;
        }
        dir = dirname(dir);
      }
    } catch { /* try the next start */ }
  }
  return null;
}

export function misterClean(pathVar) {
  const bin = findOnPath("mister-clean", pathVar);
  return bin ? { path: bin, version: packageVersionOf(bin, MISTER_CLEAN_PACKAGE) } : null;
}

function portfolio(outDir, registry) {
  return registry.projects.map((p) => {
    const cur = currentFiles(outDir, p.repo_name)[0];
    return { repo_name: p.repo_name, everyday_name: p.everyday_name, current_brief: cur ? cur.path : null };
  });
}

function priorList(outDir, key) {
  return history(outDir, key).reverse().map((f) => ({ number: f.number, date: f.parsed.date, location: f.location, path: f.path }));
}

export function projectContext({ cfg, cwd = process.cwd(), date = todayStamp() }) {
  const outDir = requireOutputDir(cfg);
  const info = repoInfo(cwd);
  const registry = loadRegistry(outDir);
  const { project, conflict } = matchCheckout(registry, info);
  const repoName = project ? project.repo_name : info.repo_name;
  const work = join(cfg.cache_dir, "work", repoName, date);
  const base = {
    brf_version: brfVersion(),
    templates: Object.fromEntries(Object.entries(currentTemplates()).map(([k, v]) => [k, { version: v.version, path: v.path }])),
    output_dir: outDir,
    registry: registry.path,
    repo: { ...info, repo_name: repoName },
    registered: Boolean(project),
  };
  if (conflict) {
    return {
      ...base,
      stop: `The registry has a project named "${conflict.repo_name}", but it is a different checkout (${conflict.local_path || conflict.remote}). This repository is not registered. Tell the owner; do not brief it.`,
    };
  }
  if (!project) {
    return {
      ...base,
      stop: `"${repoName}" is not in the registry, so it does not get a brief. Tell the owner. Only the owner adds projects: asking for a brief, or typing /brf, is not a request to register one.`,
    };
  }
  mkdirSync(work, { recursive: true });
  const prior = priorList(outDir, repoName);
  return {
    ...base,
    everyday_name: project.everyday_name,
    date,
    date_human: humanDate(date),
    output_name: formatName({ kind: "project", repo: repoName, date }),
    brief_number: nextNumber(outDir, repoName),
    prior_briefs: prior,
    baseline: prior[0] || null,
    work_dir: work,
    portfolio: portfolio(outDir, registry).filter((p) => p.repo_name !== repoName),
    tools: {
      gh: toolVersion("gh", ["--version"]),
      gh_prefix: cfg.gh_user ? `GH_TOKEN=$(gh auth token -u ${cfg.gh_user}) ` : "",
      mister_clean: misterClean(),
    },
  };
}

export function metaContext({ cfg, date = todayStamp() }) {
  const outDir = requireOutputDir(cfg);
  const registry = loadRegistry(outDir);
  const work = join(cfg.cache_dir, "work", "_meta", date);
  mkdirSync(work, { recursive: true });
  const projects = portfolio(outDir, registry);
  return {
    brf_version: brfVersion(),
    templates: Object.fromEntries(Object.entries(currentTemplates()).map(([k, v]) => [k, { version: v.version, path: v.path }])),
    output_dir: outDir,
    registry: registry.path,
    date,
    date_human: humanDate(date),
    output_name: formatName({ kind: "meta", date }),
    brief_number: nextNumber(outDir, META_KEY),
    prior_meta: priorList(outDir, META_KEY),
    projects,
    missing: projects.filter((p) => !p.current_brief).map((p) => p.repo_name),
    work_dir: work,
  };
}
