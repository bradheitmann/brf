// Turn a filled template into the finished, self-contained page.
//
// In order: read the template tag, check the output name, work out the brief number,
// embed the fonts, refuse anything left unfilled, strip every comment, stamp the
// versions, write the file.
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { BrfError } from "./errors.mjs";
import { nextNumber } from "./folder.mjs";
import { markupProblems } from "./check.mjs";
import { readMeta } from "./html.mjs";
import { parseName } from "./naming.mjs";
import { findProject, loadRegistry } from "./registry.mjs";
import { brfVersion, compareVersions, currentTemplates, readTemplateTag, TEMPLATES_DIR } from "./templates.mjs";

const FONTS_MARKER = "/* {{FONTS_CSS}} */";

export function colophonText({ kind, brief, brf, template }) {
  const what = kind === "meta" ? `Meta brief ${brief}` : `Brief ${brief} of this project`;
  return `${what} · brf ${brf} · ${kind} template ${template}`;
}

export function build({ input, out, brief = null, example = false, outputDir = null, log = () => {} }) {
  const src = resolve(input);
  if (!existsSync(src)) throw new BrfError("NOT_FOUND", `Not found: ${src}`);
  if (!out) throw new BrfError("NO_OUT", "Pass --out with a path in your work folder, e.g. --out <work_dir>/brf_my-project_20260131.html");
  const dest = resolve(out);
  if (dest === src) throw new BrfError("SAME_FILE", "The output would overwrite the input.");

  let html = readFileSync(src, "utf8");
  if (readMeta(html, "brf-version")) throw new BrfError("ALREADY_BUILT", `${basename(src)} is already a finished brief. Build from the filled template.`);

  const tag = readTemplateTag(html);
  if (!tag) throw new BrfError("NO_TEMPLATE_TAG", 'The filled file has no <meta name="brf-template"> tag. Start from a template in templates/ and keep that tag.');
  const current = currentTemplates()[tag.kind];
  if (current.major !== tag.major) {
    throw new BrfError("TEMPLATE_MAJOR", `The filled file uses ${tag.kind} template ${tag.version}; this brf ships ${current.version}. Refill from templates/${basename(current.path)}.`);
  }
  if (compareVersions(tag.version, current.version) < 0) log(`note      ${tag.kind} template ${tag.version} is older than ${current.version}; it still builds`);

  const parsed = parseName(basename(dest));
  if (!parsed || parsed.ext !== "html" || parsed.variant || parsed.v) {
    throw new BrfError(
      "BAD_NAME",
      `"${basename(dest)}" is not a brief name. Use brf_<repo-name>_<yyyymmdd>.html or meta_brf_<yyyymmdd>.html, e.g. <work_dir>/brf_my-project_20260131.html`
    );
  }
  if (parsed.kind !== tag.kind) {
    throw new BrfError("KIND_MISMATCH", `A ${tag.kind} template cannot become ${basename(dest)}. Project briefs are brf_…, the meta brief is meta_brf_….`);
  }

  // Which project, and which brief number.
  if (!example) {
    if (!outputDir) throw new BrfError("NO_OUTPUT_DIR", "No output folder is set, so the brief number and registry cannot be checked. Run brf init, or pass --example for a sample page.");
    if (parsed.kind === "project") {
      const project = findProject(loadRegistry(outputDir), { repo_name: parsed.repo });
      if (!project) {
        throw new BrfError("NOT_REGISTERED", `"${parsed.repo}" is not in the registry. Briefs are made only for projects the owner has asked for (brf register).`);
      }
    }
  }
  let number = brief;
  if (number === null || number === undefined) {
    if (example) number = 1;
    else number = nextNumber(outputDir, parsed.key);
  }
  if (!Number.isInteger(number) || number < 1) throw new BrfError("BAD_BRIEF_NUMBER", `The brief number must be a whole number from 1; got ${number}.`);

  // Fonts.
  const fontsPath = join(TEMPLATES_DIR, "fonts.css");
  if (!html.includes(FONTS_MARKER)) throw new BrfError("NO_FONTS_MARKER", `The fonts marker is gone. Put ${FONTS_MARKER} back inside the style element.`);
  const fontsCss = readFileSync(fontsPath, "utf8");
  html = html.replace(FONTS_MARKER, () => fontsCss);
  log(`fonts     embedded ${fontsCss.length.toLocaleString()} bytes`);

  // Nothing left unfilled.
  const problems = [];
  for (const m of html.matchAll(/\{\{([A-Z0-9_]*)\}\}/g)) problems.push(`unfilled slot  {{${m[1]}}}`);
  for (const m of html.matchAll(/<!--\s*\/?repeat\b[^>]*-->/g)) problems.push(`repeat marker  ${m[0].trim()}`);
  if (problems.length) {
    const counts = new Map();
    for (const p of problems) counts.set(p, (counts.get(p) || 0) + 1);
    const list = [...counts].map(([p, n]) => `  ${p}${n > 1 ? `  (${n}x)` : ""}`).join("\n");
    throw new BrfError("UNFILLED", `Refused. ${problems.length} thing(s) still to do in ${basename(src)}:\n${list}\nFill every slot, expand each repeat block and delete its two markers, then build again.`);
  }

  // Comments carry guidance only; none ships.
  const before = html.length;
  html = html.replace(/<!--[\s\S]*?-->\n?/g, "");
  log(`comments  stripped ${(before - html.length).toLocaleString()} bytes of guidance`);

  // Only brf's markup allowlist may ship: nothing that runs code or reaches the network.
  const markup = markupProblems(html);
  if (markup.length) throw new BrfError("UNSAFE_MARKUP", `Refused. The page ${markup.join("; ")}.`);

  // Stamp.
  const brf = brfVersion();
  const stampMeta = [
    `<meta name="brf-version" content="${brf}">`,
    `<meta name="brf-brief" content="${number}">`,
    parsed.kind === "project" ? `<meta name="brf-repo" content="${parsed.repo}">` : null,
    `<meta name="brf-date" content="${parsed.date.slice(0, 4)}-${parsed.date.slice(4, 6)}-${parsed.date.slice(6, 8)}">`,
  ].filter(Boolean).join("\n");
  html = html.replace(/(<meta\s+name="brf-template"[^>]*>)/, (m) => `${m}\n${stampMeta}`);
  const colophon = `<span data-brf="colophon" style="display:block;margin-top:8px">${colophonText({ kind: parsed.kind, brief: number, brf, template: tag.version })}</span>`;
  const at = html.lastIndexOf("</footer>");
  if (at >= 0) html = `${html.slice(0, at)}${colophon}${html.slice(at)}`;
  else html = html.replace(/<\/main>/, () => `<footer class="footer small">${colophon}</footer></main>`);

  mkdirSync(dirname(dest), { recursive: true });
  writeFileSync(dest, html);
  const bytes = statSync(dest).size;
  log(`built     ${dest}`);
  log(`stamped   ${colophonText({ kind: parsed.kind, brief: number, brf, template: tag.version })}`);
  log(`size      ${(bytes / 1024).toFixed(0)} KB`);
  return { path: dest, name: basename(dest), kind: parsed.kind, repo: parsed.repo, date: parsed.date, brief: number, brf_version: brf, template: `${tag.kind} ${tag.version}` };
}
