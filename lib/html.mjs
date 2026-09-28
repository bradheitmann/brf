// Small, dependency-free helpers for reading finished pages.
export function readMeta(html, name) {
  const re = new RegExp(`<meta\\s+name="${name}"\\s+content="([^"]*)"\\s*/?>`);
  const m = html.match(re);
  return m ? m[1] : null;
}

export function readStamp(html) {
  const brief = readMeta(html, "brf-brief");
  return {
    brf_version: readMeta(html, "brf-version"),
    template: readMeta(html, "brf-template"),
    brief: brief && /^\d+$/.test(brief) ? +brief : null,
    repo: readMeta(html, "brf-repo"),
    date: readMeta(html, "brf-date"),
  };
}

const ENTITIES = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " ", middot: "·", rsquo: "’", lsquo: "‘", ldquo: "“", rdquo: "”", mdash: "—", ndash: "–", hellip: "…" };

export function decodeEntities(s) {
  return s.replace(/&(#[xX][0-9a-fA-F]+|#\d+|[a-zA-Z][a-zA-Z0-9]*);?/g, (all, e) => {
    if (e[0] === "#") {
      const code = e[1] === "x" || e[1] === "X" ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10);
      if (!Number.isFinite(code) || code === 0 || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) return "\uFFFD";
      return String.fromCodePoint(code);
    }
    if (!all.endsWith(";")) return all;
    return ENTITIES[e.toLowerCase()] ?? all;
  });
}

export function visibleText(html) {
  const noHidden = html
    .replace(/<style[\s\S]*?<\/style>/gi, " ")
    .replace(/<script[\s\S]*?<\/script>/gi, " ")
    .replace(/<!--[\s\S]*?-->/g, " ")
    .replace(/<head[\s\S]*?<\/head>/i, " ");
  return decodeEntities(noHidden.replace(/<[^>]+>/g, " ")).replace(/\s+/g, " ").trim();
}

// The part of a page the reader must get through: everything before the appendices
// (marked data-brf="appendix") and before the footer.
export function mainBody(html) {
  const cuts = [html.search(/<section[^>]*data-brf="appendix"/), html.search(/<footer\b/)];
  let cut = Math.min(...cuts.filter((i) => i >= 0));
  if (!Number.isFinite(cut)) cut = html.length;
  let body = html.slice(0, cut);
  // Briefs built before the appendix marker existed: fall back to the heading text.
  if (cuts[0] < 0) {
    const legacy = body.search(/<h2>\s*Planning records\s*<\/h2>/);
    if (legacy >= 0) body = body.slice(0, legacy);
  }
  return body;
}

// Words in the main view: inside <main>, outside every <details> (the collapsed sections) and
// outside tables (chart data). Labels count; bare numbers do not. The protocol's budget is about 180.
export function mainViewWords(html) {
  const m = html.match(/<main\b[^>]*>([\s\S]*?)<\/main>/i);
  if (!m) return null;
  const open = m[1].replace(/<details\b[\s\S]*?<\/details>/gi, " ").replace(/<table\b[\s\S]*?<\/table>/gi, " ");
  const text = visibleText(open);
  return text ? text.split(" ").filter((w) => /\p{L}/u.test(w)).length : 0;
}
