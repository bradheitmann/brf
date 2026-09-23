#!/usr/bin/env node
// brf: re-entry briefs for the projects you own. See README.md and PROTOCOL.md.
import { existsSync, mkdirSync, realpathSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, resolve } from "node:path";
import { build } from "../lib/build.mjs";
import { checkFile, checkFolder } from "../lib/check.mjs";
import { expandHome, loadConfig, readConfigFile, requireOutputDir, writeJsonAtomic } from "../lib/config.mjs";
import { metaContext, projectContext } from "../lib/context.mjs";
import { deliver } from "../lib/deliver.mjs";
import { BrfError } from "../lib/errors.mjs";
import { ensureLayout } from "../lib/folder.mjs";
import { parseName, todayStamp, toRepoName } from "../lib/naming.mjs";
import { addProject, emptyRegistry, loadRegistry, matchCheckout, registryPath } from "../lib/registry.mjs";
import { repoInfo } from "../lib/repo.mjs";
import { compareSnapshot, saveSnapshot, takeSnapshot } from "../lib/snapshot.mjs";
import { brfVersion, currentTemplates } from "../lib/templates.mjs";
import { verify } from "../lib/verify.mjs";

const HELP = `brf ${brfVersion()}: re-entry briefs, one phone-readable page per project.

  brf init --output <folder> [--playwright <path>] [--gh-user <name>] [--force]
      Set up the output folder (registry.json, archive/) and point this machine at it.
  brf config                       Show the settings brf resolved.
  brf context [--meta]             JSON: this repo's name, registration, file name, brief number,
                                   earlier briefs, work folder. --meta for the meta brief.
  brf register --reason "<the owner's words>" [--everyday-name <name>] [--repo-name <name>]
                                   Add this repo to the registry. Only when the owner explicitly
                                   asked to add it; asking for a brief is not that request.
  brf snapshot --out <file>        Record HEAD and a hash of git status (read-only proof).
  brf snapshot --compare <file>    Exit 1 if the repo changed since that snapshot.
  brf build <filled.html> --out <work_dir>/<brf_repo_yyyymmdd.html> [--verify] [--proof-dir <dir>]
            [--brief <n>] [--example]
                                   Embed fonts, refuse unfilled slots, stamp versions, render-check.
                                   Exit 3 means the render check was skipped (Playwright or
                                   Chromium is missing or would not start).
  brf deliver <built.html>         File it in the output folder; the project's previous brief
                                   moves to archive/.
  brf check [--file <brief.html>]  Check one brief, or the whole output folder.
  brf version                      Print the brf and template versions.
`;

function parseArgs(argv) {
  const flags = {};
  const positional = [];
  const bool = new Set(["verify", "example", "meta", "force", "json", "help"]);
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith("--")) {
      const eq = a.indexOf("=");
      if (eq > 2) {
        flags[a.slice(2, eq)] = a.slice(eq + 1);
        continue;
      }
      const key = a.slice(2);
      if (bool.has(key)) flags[key] = true;
      else {
        const v = argv[i + 1];
        if (v === undefined || v.startsWith("--")) throw new BrfError("MISSING_VALUE", `--${key} needs a value.`);
        flags[key] = v;
        i++;
      }
    } else positional.push(a);
  }
  return { flags, positional };
}

// Resolve symlinks and letter case on the nearest existing ancestor, so /var and /private/var,
// or two spellings of one folder on a case-insensitive disk, compare equal.
function realLoose(p) {
  let dir = p;
  const tail = [];
  while (!existsSync(dir) && dirname(dir) !== dir) { tail.unshift(dir.slice(dirname(dir).length + 1)); dir = dirname(dir); }
  return [realpathSync.native(dir), ...tail].join("/");
}
function isInside(path, root) {
  const r = realLoose(root);
  const p = realLoose(path);
  return p === r || p.startsWith(`${r}/`);
}

// Run inside a repository, build and deliver make sure a project brief belongs to this checkout,
// not to a different one that happens to share its name.
function checkoutGuard(file, outputDir) {
  const parsed = parseName(basename(file));
  if (!parsed || parsed.kind !== "project") return;
  let info = null;
  try { info = repoInfo(); } catch { return; }
  const { project, conflict } = matchCheckout(loadRegistry(outputDir), info);
  if (conflict) throw new BrfError("OTHER_CHECKOUT", `This checkout is not the registered "${conflict.repo_name}" (${conflict.local_path || conflict.remote}).`);
  if (project && project.repo_name !== parsed.repo) throw new BrfError("WRONG_PROJECT", `This checkout is registered as "${project.repo_name}", not "${parsed.repo}".`);
  if (!project) throw new BrfError("NOT_REGISTERED", "This checkout is not in the registry.");
}

const print = (x) => process.stdout.write(`${typeof x === "string" ? x : JSON.stringify(x, null, 2)}\n`);
const log = (s) => process.stdout.write(`${s}\n`);

async function main() {
  const [cmd = "help", ...rest] = process.argv.slice(2);
  const { flags, positional } = parseArgs(rest);
  const cfg = loadConfig();

  switch (cmd) {
    case "help":
    case "--help":
    case "-h":
      print(HELP);
      return 0;

    case "version":
    case "--version": {
      const t = currentTemplates();
      print(`brf ${brfVersion()} · project template ${t.project.version} · meta template ${t.meta.version}`);
      return 0;
    }

    case "config":
      print(cfg);
      return 0;

    case "init": {
      if (!flags.output) throw new BrfError("NO_OUTPUT", "Pass --output <folder>, the folder that should receive briefs.");
      const out = resolve(expandHome(flags.output));
      const file = readConfigFile(cfg.config_path);
      if (file.output_dir && resolve(expandHome(file.output_dir)) !== out && !flags.force) {
        throw new BrfError("CONFIG_EXISTS", `This machine already points at ${file.output_dir}. Pass --force to change it.`);
      }
      mkdirSync(out, { recursive: true });
      ensureLayout(out);
      const reg = registryPath(out);
      const createdRegistry = !existsSync(reg);
      if (createdRegistry) writeJsonAtomic(reg, emptyRegistry());
      const next = { ...file, output_dir: out.startsWith(`${homedir()}/`) ? `~/${out.slice(homedir().length + 1)}` : out };
      if (flags.playwright) next.playwright = flags.playwright;
      if (flags["gh-user"]) next.gh_user = flags["gh-user"];
      writeJsonAtomic(cfg.config_path, next);
      print({ output_dir: out, registry: reg, registry_created: createdRegistry, config: cfg.config_path });
      return 0;
    }

    case "context":
      print(flags.meta ? metaContext({ cfg }) : projectContext({ cfg }));
      return 0;

    case "register": {
      const outDir = requireOutputDir(cfg);
      const info = repoInfo();
      const home = homedir();
      const record = addProject(outDir, {
        repo_name: toRepoName(flags["repo-name"] || info.repo_name),
        everyday_name: flags["everyday-name"],
        local_path: info.main_path.startsWith(`${home}/`) ? `~/${info.main_path.slice(home.length + 1)}` : info.main_path,
        remote: info.remote,
        added: `${todayStamp().slice(0, 4)}-${todayStamp().slice(4, 6)}-${todayStamp().slice(6, 8)}`,
        reason: flags.reason,
      });
      print({ registered: record, registry: registryPath(outDir) });
      return 0;
    }

    case "snapshot": {
      if (flags.compare) {
        const r = compareSnapshot(resolve(flags.compare));
        print({
          same: r.same,
          changed: r.changed,
          before: { head: r.before.head, branch: r.before.branch, digest: r.before.digest },
          after: { head: r.after.head, branch: r.after.branch, digest: r.after.digest },
        });
        return r.same ? 0 : 1;
      }
      if (!flags.out) throw new BrfError("NO_OUT", "Pass --out <file> (keep it outside the repository) or --compare <file>.");
      const snap = takeSnapshot();
      const target = resolve(flags.out);
      const here = repoInfo();
      if ([snap.repo, here.toplevel, here.main_path].some((root) => isInside(target, root))) throw new BrfError("INSIDE_REPO", "Write the snapshot outside the repository.");
      saveSnapshot(target, snap);
      print(snap);
      return 0;
    }

    case "build": {
      if (!positional[0]) throw new BrfError("NO_INPUT", "Pass the filled template: brf build <filled.html> --out <work_dir>/<name>");
      if (!flags.out) throw new BrfError("NO_OUT", "Pass --out with a path in your work folder, e.g. --out <work_dir>/brf_my-project_20260131.html");
      const brief = flags.brief !== undefined ? Number(flags.brief) : null;
      const outputDir = flags.example ? null : requireOutputDir(cfg);
      // Builds happen in the work folder. Never inside the repository being briefed, and never
      // straight into the output folder, where only brf deliver may add or move files.
      const out = resolve(flags.out);
      const proofDir = resolve(flags["proof-dir"] || resolve(dirname(out), "proof"));
      // Every checkout brf knows about: this one, its main checkout, and every registered project.
      const repoRoots = [];
      try { const here = repoInfo(); repoRoots.push(here.toplevel, here.main_path); } catch { /* not in a repository */ }
      const registryDir = outputDir || (cfg.output_dir && existsSync(cfg.output_dir) ? cfg.output_dir : null);
      if (registryDir) for (const p of loadRegistry(registryDir).projects) if (p.local_path) repoRoots.push(expandHome(p.local_path));
      for (const [what, path] of [["output", out], ["proof folder", proofDir]]) {
        const inRepo = repoRoots.find((root) => existsSync(root) && isInside(path, root));
        if (inRepo) throw new BrfError("INSIDE_REPO", `The ${what} ${path} is inside a repository (${inRepo}). Build in the work folder from brf context.`);
        if (cfg.output_dir && existsSync(cfg.output_dir) && isInside(path, cfg.output_dir)) {
          throw new BrfError("INSIDE_OUTPUT", `The ${what} ${path} is inside the output folder. Build in the work folder, then brf deliver.`);
        }
      }
      if (!flags.example) checkoutGuard(out, outputDir);
      const result = build({ input: positional[0], out, brief, example: Boolean(flags.example), outputDir, log });
      if (!flags.verify) {
        log("(pass --verify to render it at five widths)");
        return 0;
      }
      const v = await verify({ file: result.path, proofDir, cfg, log });
      if (v.skipped) return 3;
      return v.clean ? 0 : 1;
    }

    case "deliver": {
      if (!positional[0]) throw new BrfError("NO_INPUT", "Pass the built brief: brf deliver <brf_repo_yyyymmdd.html>");
      const outputDir = requireOutputDir(cfg);
      checkoutGuard(resolve(positional[0]), outputDir);
      print(deliver({ file: resolve(positional[0]), outputDir, cacheDir: cfg.cache_dir }));
      return 0;
    }

    case "check": {
      if (flags.file) {
        if (!existsSync(resolve(flags.file))) throw new BrfError("NOT_FOUND", `Not found: ${resolve(flags.file)}`);
        const problems = checkFile(resolve(flags.file), { bannedExtra: cfg.extra_banned_words });
        print(flags.json ? { problems } : problems.length ? problems.map((p) => `PROBLEM ${p}`).join("\n") : "CHECK CLEAN");
        return problems.length ? 1 : 0;
      }
      const outDir = requireOutputDir(cfg);
      const r = checkFolder(outDir, { bannedExtra: cfg.extra_banned_words });
      if (flags.json) print(r);
      else {
        for (const p of r.problems) log(`PROBLEM ${p}`);
        for (const n of r.notes) log(`note    ${n}`);
        log(`${r.current_count} current brief(s), ${r.registered_count} registered project(s)`);
        log(r.problems.length ? "CHECK NOT CLEAN" : "CHECK CLEAN");
      }
      return r.problems.length ? 1 : 0;
    }

    default:
      throw new BrfError("UNKNOWN_COMMAND", `Unknown command "${cmd}". Run brf help.`);
  }
}

// Set the exit code and let Node exit on its own, so piped output is never cut short.
main().then(
  (code) => { process.exitCode = code; },
  (err) => {
    if (err instanceof BrfError) {
      process.stderr.write(`brf: ${err.message} (${err.code})\n`);
      process.exitCode = 2;
      return;
    }
    process.stderr.write(`${err.stack || err}\n`);
    process.exitCode = 1;
  }
);
