// Checks a finished brief, or the whole output folder, against the protocol's hard rules.
// Problems fail the check. Notes are for the owner and do not.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join } from "node:path";
import { ARCHIVE_DIR, history, listFiles } from "./folder.mjs";
import { decodeEntities, mainBody, readStamp, visibleText } from "./html.mjs";
import { isBrief, parseName, todayStamp } from "./naming.mjs";
import { loadRegistry, REGISTRY_FILE } from "./registry.mjs";

export const DEFAULT_BANNED = [
  "honest", "honestly", "load-bearing", "earns its place", "genuinely", "quietly", "crucially",
  "leverage", "leverages", "leveraged", "seamless", "seamlessly", "delve", "game-changer", "world-class", "not just",
];

const IDENTIFIERS = [
  { what: "ticket or task id", re: /\b[A-Z][A-Z0-9]+-(?:\d+|[A-Z0-9][A-Z0-9-]{2,})\b/g },
  { what: "commit hash", re: /\b(?=[0-9a-f]*[a-f])(?=[0-9a-f]*\d)[0-9a-f]{7,40}\b/g },
  { what: "pull request or issue number", re: /(?<![\w&])#\d{1,5}\b/g },
  { what: "file path", re: /\b[\w.-]+\/[\w./-]*\.(?:md|json|ts|tsx|js|mjs|cjs|py|sh|ya?ml|toml|rs|go|html|css)\b/g },
  { what: "version string", re: /\bv?\d+\.\d+\.\d+\b/g },
];

// The stamp inside a page must agree with its file name: kind, project and date.
export function stampMismatch(stamp, parsed) {
  if (!parsed || !stamp.template) return [];
  const out = [];
  const kind = stamp.template.split(" ")[0];
  if (kind !== parsed.kind) out.push(`is a ${kind} brief but named as a ${parsed.kind} brief`);
  if (parsed.kind === "project" && stamp.repo !== parsed.repo) out.push(`is stamped for "${stamp.repo}" but named for "${parsed.repo}"`);
  const named = `${parsed.date.slice(0, 4)}-${parsed.date.slice(4, 6)}-${parsed.date.slice(6, 8)}`;
  if (stamp.date && stamp.date !== named) out.push(`is stamped ${stamp.date} but named ${named}`);
  return out;
}

function escapeRe(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// ---------------------------------------------------------------------------------------
// Markup allowlist. A brief may contain only the markup brf's templates produce: these tags,
// these attributes, links that need a tap, and CSS whose only url() targets are embedded
// data. Anything else fails, however it is written. The reader below follows the HTML rules
// that matter for that: comments (including "<!-->" and "--!>"), bogus comments ("<!x", "<?x",
// "</3"), text-only elements such as <style> and <title>, SVG content, and HTML whitespace.

const isWS = (c) => c === "\t" || c === "\n" || c === "\f" || c === "\r" || c === " ";
const isLetter = (c) => Boolean(c) && /[a-z]/i.test(c);
const RAW_TEXT = new Set(["script", "style", "xmp", "iframe", "noembed", "noframes", "noscript", "textarea", "title", "plaintext"]);

function readTag(html, start) {
  const n = html.length;
  let j = start;
  while (j < n && !isWS(html[j]) && html[j] !== "/" && html[j] !== ">") j++;
  const name = html.slice(start, j).toLowerCase();
  const attrs = [];
  let selfClosing = false;
  for (;;) {
    while (j < n && (isWS(html[j]) || html[j] === "/")) {
      if (html[j] === "/" && html[j + 1] === ">") selfClosing = true;
      j++;
    }
    if (j >= n) break;
    if (html[j] === ">") { j++; break; }
    let k = j + 1; // the first character of a name may be anything, even "=" or a quote
    while (k < n && !isWS(html[k]) && html[k] !== "/" && html[k] !== ">" && html[k] !== "=") k++;
    const attrName = html.slice(j, k).toLowerCase();
    j = k;
    while (j < n && isWS(html[j])) j++;
    let raw = null;
    if (html[j] === "=") {
      j++;
      while (j < n && isWS(html[j])) j++;
      const q = html[j];
      if (q === '"' || q === "'") {
        const close = html.indexOf(q, j + 1);
        raw = html.slice(j + 1, close < 0 ? n : close);
        j = close < 0 ? n : close + 1;
      } else {
        let e = j;
        while (e < n && !isWS(html[e]) && html[e] !== ">") e++;
        raw = html.slice(j, e);
        j = e;
      }
    }
    attrs.push({ name: attrName, raw: raw ?? "", value: decodeEntities(raw ?? "") });
  }
  return { name, attrs, selfClosing, end: j };
}

function findEndTag(html, name, from) {
  const lower = html.toLowerCase();
  let at = from;
  for (;;) {
    const i = lower.indexOf(`</${name}`, at);
    if (i < 0) return -1;
    const after = html[i + 2 + name.length];
    if (after === undefined || isWS(after) || after === "/" || after === ">") return i;
    at = i + 2;
  }
}

// Start tags (with whether they sit inside SVG or MathML) and the text of each <style> element.
export function scanMarkup(html) {
  const tags = [];
  const styles = [];
  const n = html.length;
  let i = 0;
  let foreign = 0;
  while (i < n) {
    const lt = html.indexOf("<", i);
    if (lt < 0) break;
    const c1 = html[lt + 1];
    if (html.startsWith("<!--", lt)) {
      const j = lt + 4;
      if (html[j] === ">") { i = j + 1; continue; }
      if (html.startsWith("->", j)) { i = j + 2; continue; }
      const a = html.indexOf("-->", j);
      const b = html.indexOf("--!>", j);
      const e = Math.min(a < 0 ? Infinity : a + 3, b < 0 ? Infinity : b + 4);
      i = Number.isFinite(e) ? e : n;
      continue;
    }
    if (c1 === "!" || c1 === "?") {
      const e = html.indexOf(">", lt + 2);
      i = e < 0 ? n : e + 1;
      continue;
    }
    if (c1 === "/") {
      if (isLetter(html[lt + 2])) {
        const t = readTag(html, lt + 2);
        if ((t.name === "svg" || t.name === "math") && foreign > 0) foreign--;
        i = t.end;
      } else {
        const e = html.indexOf(">", lt + 2);
        i = e < 0 ? n : e + 1;
      }
      continue;
    }
    if (isLetter(c1)) {
      const t = readTag(html, lt + 1);
      tags.push({ name: t.name, attrs: t.attrs, foreign: foreign > 0 });
      i = t.end;
      if (t.name === "svg" || t.name === "math") {
        if (!t.selfClosing) foreign++;
        continue;
      }
      if (foreign === 0 && RAW_TEXT.has(t.name)) {
        if (t.name === "plaintext") { i = n; continue; }
        const close = findEndTag(html, t.name, i);
        if (t.name === "style") styles.push(html.slice(i, close < 0 ? n : close));
        i = close < 0 ? n : close;
      }
      continue;
    }
    i = lt + 1;
  }
  return { tags, styles };
}

const HTML_TAGS = new Set([
  "html", "head", "body", "meta", "title", "style", "header", "footer", "main", "section", "article", "aside", "nav",
  "div", "span", "p", "h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol", "li", "dl", "dt", "dd", "table", "thead",
  "tbody", "tfoot", "tr", "th", "td", "caption", "colgroup", "col", "strong", "em", "b", "i", "small", "sub", "sup",
  "abbr", "time", "code", "kbd", "pre", "blockquote", "mark", "s", "u", "br", "wbr", "hr", "figure", "figcaption",
  "details", "summary", "a", "svg",
]);
const SVG_TAGS = new Set(["svg", "g", "path", "circle", "ellipse", "rect", "line", "polyline", "polygon", "title", "desc", "defs", "lineargradient", "radialgradient", "stop", "clippath", "text", "tspan"]);
const GLOBAL_ATTRS = new Set(["class", "id", "lang", "dir", "title", "role", "hidden", "style", "translate"]);
const TAG_ATTRS = {
  html: ["data-style"], meta: ["charset", "name", "content"], a: ["href", "rel", "target"],
  th: ["scope", "colspan", "rowspan"], td: ["colspan", "rowspan"], time: ["datetime"], details: ["open"],
  ol: ["start", "reversed"], col: ["span"], colgroup: ["span"],
};
const SVG_ATTRS = new Set([
  "xmlns", "xmlns:xlink", "viewbox", "width", "height", "fill", "stroke", "stroke-width", "stroke-linecap", "stroke-linejoin",
  "stroke-miterlimit", "stroke-dasharray", "stroke-dashoffset", "stroke-opacity", "fill-opacity", "fill-rule", "clip-rule",
  "clip-path", "d", "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "rx", "ry", "fx", "fy", "dx", "dy", "points",
  "transform", "opacity", "offset", "stop-color", "stop-opacity", "gradientunits", "gradienttransform", "spreadmethod",
  "clippathunits", "font-family", "font-size", "font-weight", "text-anchor", "dominant-baseline", "letter-spacing",
  "preserveaspectratio", "version", "focusable", "shape-rendering", "vector-effect", "paint-order", "xml:space",
  "display", "visibility", "color",
]);
const STYLE_ATTR = /^[a-zA-Z0-9\t\n\f\r :;.%#,-]*$/;
const LINK = /^(?:https?:\/\/[^\s]|mailto:[^\s]|#)/i;

// CSS escapes ("\\72" is "r") decoded, so "u\\72l(" is seen as "url(". Line endings are
// normalised first, as CSS does, so CR LF after an escape counts as one newline.
function decodeCssEscapes(css) {
  return css
    .replace(/\r\n?|\f/g, "\n")
    .replace(/\\([0-9a-fA-F]{1,6})[\t\n\f\r ]?/g, (m, h) => {
      const c = parseInt(h, 16);
      return c > 0 && c <= 0x10ffff && !(c >= 0xd800 && c <= 0xdfff) ? String.fromCodePoint(c) : "\uFFFD";
    })
    .replace(/\\([\s\S])/g, "$1");
}

function cssProblems(css) {
  const out = [];
  // Comments are not removed first: a comment marker inside a CSS string would hide what
  // follows. Looking at everything can only flag more, never less.
  const decoded = decodeCssEscapes(css);
  if (/@import/i.test(decoded)) out.push("its CSS imports another stylesheet");
  if (/image-set\(/i.test(decoded)) out.push("its CSS uses image-set()");
  for (const m of decoded.matchAll(/url\(\s*(["']?)\s*/gi)) {
    if (!/^data:/i.test(decoded.slice(m.index + m[0].length, m.index + m[0].length + 5))) {
      out.push("its CSS fetches from the network (a url() that is not embedded data)");
      break;
    }
  }
  return out;
}

// Everything in a page that is outside brf's markup allowlist.
export function markupProblems(html) {
  return allowlistProblems(scanMarkup(html));
}

// The allowlist itself, applied to a list of start tags and <style> texts.
export function allowlistProblems({ tags, styles }) {
  const found = new Set();
  for (const tag of tags) {
    const allowed = tag.foreign ? SVG_TAGS.has(tag.name) : HTML_TAGS.has(tag.name);
    if (!allowed) {
      found.add(tag.name === "script" ? "has a script element" : `uses a <${tag.name}> element, which brf does not allow`);
      continue;
    }
    const svgish = tag.foreign || tag.name === "svg";
    for (const { name, value, raw } of tag.attrs) {
      if (name.startsWith("on")) { found.add("has an event-handler attribute"); continue; }
      const ok =
        /^(?:aria|data)-[a-z0-9-]+$/.test(name) ||
        GLOBAL_ATTRS.has(name) ||
        (TAG_ATTRS[tag.name] || []).includes(name) ||
        (svgish && SVG_ATTRS.has(name));
      if (!ok) { found.add(`has a "${name}" attribute on <${tag.name}>, which brf does not allow`); continue; }
      // An entity brf cannot decode could spell anything, such as "(" in "url&lpar;".
      if (/&[a-zA-Z][a-zA-Z0-9]*;/.test(value)) { found.add(`has an entity brf cannot read in the "${name}" attribute`); continue; }
      if (name === "style" && !(STYLE_ATTR.test(raw) && STYLE_ATTR.test(value))) found.add("has a style attribute brf does not allow (only plain values like width:40%)");
      // SVG presentation attributes are CSS values, so CSS escapes apply to them too.
      const cssValue = decodeCssEscapes(value);
      if (/url\s*\(/i.test(cssValue) && !/^\s*url\(\s*#[\w-]+\s*\)\s*$/i.test(cssValue)) found.add(`has a url() in the "${name}" attribute`);
      if (name === "href") {
        const compact = value.replace(/[\s\x00-\x1f]/g, "");
        if (!LINK.test(compact) || /&(?:#|[a-z]+;)/i.test(value)) found.add("has a link that is not https, http, mailto or a # anchor");
      }
      if (tag.name === "meta" && name === "content" && /url\s*=/i.test(value)) found.add("has a meta tag that points somewhere");
    }
  }
  for (const css of styles) for (const p of cssProblems(css)) found.add(p);
  return [...found];
}

export function checkHtml(html, { name = "", bannedExtra = [] } = {}) {
  const problems = [...markupProblems(html)];
  const parsed = parseName(name);
  const stamp = readStamp(html);
  if (!stamp.brf_version || !stamp.template || !stamp.brief) problems.push("has no brf version stamp (build it with brf build)");
  problems.push(...stampMismatch(stamp, parsed));
  const text = visibleText(html);
  const dashes = (text.match(/—/g) || []).length;
  if (dashes) problems.push(`has ${dashes} em-dash(es)`);
  const banned = [...DEFAULT_BANNED, ...bannedExtra].filter((w) => new RegExp(`\\b${escapeRe(w)}\\b`, "i").test(text));
  if (banned.length) problems.push(`uses banned words: ${banned.join(", ")}`);
  const body = visibleText(mainBody(html));
  for (const { what, re } of IDENTIFIERS) {
    const hits = [...new Set(body.match(re) || [])];
    if (hits.length) problems.push(`shows a ${what} above the appendices: ${hits.slice(0, 6).join(", ")}`);
  }
  return problems;
}

export function checkFile(path, opts = {}) {
  return checkHtml(readFileSync(path, "utf8"), { ...opts, name: basename(path) });
}

export function checkFolder(outDir, { bannedExtra = [], staleDays = 14, today = todayStamp() } = {}) {
  const problems = [];
  const notes = [];
  const registry = loadRegistry(outDir);
  const registered = new Set(registry.projects.map((p) => p.repo_name));
  if (!registry.exists) problems.push(`${REGISTRY_FILE} is missing; the folder may not be synced yet. Tell the owner; only the owner runs brf init`);

  const current = new Map();
  for (const entry of readdirSync(outDir)) {
    if (entry.startsWith(".") || entry === REGISTRY_FILE) continue;
    const path = join(outDir, entry);
    if (statSync(path).isDirectory()) {
      if (entry !== ARCHIVE_DIR) notes.push(`unexpected folder in the output folder: ${entry}`);
      continue;
    }
    const parsed = parseName(entry);
    if (!parsed) { problems.push(`${entry}: not a brf file name`); continue; }
    if (parsed.v) problems.push(`${entry}: same-day earlier copies belong in ${ARCHIVE_DIR}/`);
    if (parsed.kind === "project" && !registered.has(parsed.repo)) problems.push(`${entry}: "${parsed.repo}" is not in the registry`);
    if (!isBrief(parsed)) continue;
    const list = current.get(parsed.key) || [];
    list.push(entry);
    current.set(parsed.key, list);
    for (const p of checkFile(path, { bannedExtra })) problems.push(`${entry}: ${p}`);
    const h = history(outDir, parsed.key);
    const top = h.reduce((m, f) => Math.max(m, f.number), 0);
    const mine = h.find((f) => f.name === entry && f.location === "root");
    if (mine && mine.number !== top) problems.push(`${entry}: is brief ${mine.number} but the archive holds brief ${top}`);
    const ageDays = Math.round((Date.parse(`${today.slice(0, 4)}-${today.slice(4, 6)}-${today.slice(6, 8)}`) - Date.parse(`${parsed.date.slice(0, 4)}-${parsed.date.slice(4, 6)}-${parsed.date.slice(6, 8)}`)) / 86400000);
    if (ageDays > staleDays) notes.push(`${entry}: ${ageDays} days old`);
  }
  for (const [key, list] of current) if (list.length > 1) problems.push(`${key}: ${list.length} current briefs in the root (${list.join(", ")}); one belongs in ${ARCHIVE_DIR}/`);
  for (const repo of registered) if (!current.has(repo)) notes.push(`${repo}: registered, no current brief`);
  const archived = listFiles(outDir).filter((f) => f.location === "archive");
  const unnamed = archived.filter((f) => !f.parsed).map((f) => f.name);
  if (unnamed.length) notes.push(`${unnamed.length} archive file(s) do not follow the naming: ${unnamed.slice(0, 5).join(", ")}`);
  return { problems, notes, current_count: current.size, registered_count: registered.size };
}
