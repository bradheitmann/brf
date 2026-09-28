// Template versions. Each template declares its own version in a meta tag:
//   <meta name="brf-template" content="project 1.0.0">
// The build reads it from the filled file and stamps it into the finished page.
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
export const TEMPLATES_DIR = join(ROOT, "templates");
export const TEMPLATE_FILES = { project: "project.html", meta: "meta.html" };
const TAG_RE = /<meta\s+name="brf-template"\s+content="(project|meta)\s+(\d+)\.(\d+)\.(\d+)"\s*\/?>/;

export function brfVersion() {
  return JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8")).version;
}

export function readTemplateTag(html) {
  const m = html.match(TAG_RE);
  if (!m) return null;
  return { kind: m[1], version: `${m[2]}.${m[3]}.${m[4]}`, major: +m[2], minor: +m[3], patch: +m[4] };
}

// The owner may keep their own templates (their brand, their fonts) outside this repository:
// BRF_TEMPLATES_DIR or the config file's templates_dir. A kind the folder does not provide, and
// fonts.css when it is absent, come from the built-in templates.
export function templatesDir(env = process.env) {
  let dir = env.BRF_TEMPLATES_DIR || null;
  if (!dir) {
    try {
      const home = env.HOME || "";
      const cfgPath = env.BRF_CONFIG || join(env.XDG_CONFIG_HOME || join(home, ".config"), "brf", "config.json");
      if (existsSync(cfgPath)) dir = JSON.parse(readFileSync(cfgPath, "utf8")).templates_dir || null;
      if (dir && dir.startsWith("~/")) dir = join(home, dir.slice(2));
    } catch { dir = null; }
  }
  return dir && existsSync(dir) ? dir : null;
}

export function templatePath(file, env = process.env) {
  const own = templatesDir(env);
  return own && existsSync(join(own, file)) ? join(own, file) : join(TEMPLATES_DIR, file);
}

export function currentTemplates() {
  const out = {};
  for (const [kind, file] of Object.entries(TEMPLATE_FILES)) {
    const path = templatePath(file);
    const tag = readTemplateTag(readFileSync(path, "utf8"));
    out[kind] = { path, version: tag ? tag.version : null, major: tag ? tag.major : null };
  }
  return out;
}

export function compareVersions(a, b) {
  const pa = a.split(".").map(Number), pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) if (pa[i] !== pb[i]) return pa[i] < pb[i] ? -1 : 1;
  return 0;
}
