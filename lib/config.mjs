// Where brf keeps its settings, and how it finds the owner's output folder.
//
// Resolution order for every setting: environment variable, then the config file.
//   BRF_OUTPUT_DIR   the folder that receives briefs (required before anything is delivered)
//   BRF_PLAYWRIGHT   a path from which the playwright package can be resolved (optional)
//   BRF_GH_USER      the gh account that can read the owner's repositories (optional)
//   BRF_CONFIG       an alternative config file path (optional)
// The config file lives at $XDG_CONFIG_HOME/brf/config.json, or ~/.config/brf/config.json.
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { BrfError } from "./errors.mjs";

export function expandHome(p) {
  if (!p) return p;
  if (p === "~") return homedir();
  return p.startsWith("~/") ? join(homedir(), p.slice(2)) : p;
}

export function configPath(env = process.env) {
  if (env.BRF_CONFIG) return resolve(expandHome(env.BRF_CONFIG));
  const base = env.XDG_CONFIG_HOME || join(homedir(), ".config");
  return join(base, "brf", "config.json");
}

export function cacheDir(env = process.env) {
  const base = env.XDG_CACHE_HOME || join(homedir(), ".cache");
  return join(base, "brf");
}

export function readConfigFile(path) {
  if (!existsSync(path)) return {};
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (err) {
    throw new BrfError("BAD_CONFIG", `The config file ${path} is not valid JSON: ${err.message}`);
  }
}

export function loadConfig(env = process.env) {
  const path = configPath(env);
  const file = readConfigFile(path);
  const out = env.BRF_OUTPUT_DIR || file.output_dir || null;
  return {
    config_path: path,
    config_exists: existsSync(path),
    output_dir: out ? resolve(expandHome(out)) : null,
    output_dir_source: env.BRF_OUTPUT_DIR ? "BRF_OUTPUT_DIR" : file.output_dir ? "config file" : null,
    playwright: env.BRF_PLAYWRIGHT || file.playwright || null,
    gh_user: env.BRF_GH_USER || file.gh_user || null,
    extra_banned_words: Array.isArray(file.extra_banned_words) ? file.extra_banned_words : [],
    cache_dir: cacheDir(env),
  };
}

export function requireOutputDir(cfg) {
  if (!cfg.output_dir) {
    throw new BrfError(
      "NO_OUTPUT_DIR",
      "No output folder is set. The owner chooses one, then runs:\n  brf init --output <folder>\n(or sets BRF_OUTPUT_DIR for this shell)."
    );
  }
  if (!existsSync(cfg.output_dir)) {
    throw new BrfError(
      "OUTPUT_DIR_MISSING",
      `The output folder ${cfg.output_dir} is not there. It may be on a drive or sync service that is not available yet. Tell the owner; only the owner runs brf init.`
    );
  }
  return cfg.output_dir;
}

// Write JSON through a temporary file so a crash never leaves half a file behind.
export function writeJsonAtomic(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.${process.pid}.tmp`;
  writeFileSync(tmp, `${JSON.stringify(value, null, 2)}\n`);
  renameSync(tmp, path);
}
