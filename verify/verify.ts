// brf render check. `brf build --verify` runs this file with Node (type stripping) or Bun:
//
//   node verify.ts --file <brief.html> --proof-dir <dir> --root <brf root> [--playwright <path>]
//
// It renders the finished page with JavaScript off at five widths, screenshots each, and
// measures what a reader would trip on. It needs Playwright with Chromium; brf has no
// dependencies, so Playwright is found, never installed.
// Exit codes: 0 clean, 1 problems found, 3 skipped (Playwright or Chromium missing), 2 usage.
import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { homedir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

export const WIDTHS = [360, 390, 768, 1024, 1440] as const;

// The small part of Playwright's API this script uses.
interface PwRequest {
  url(): string;
}
interface PwPage {
  on(event: "request", listener: (request: PwRequest) => void): void;
  goto(url: string, options: { waitUntil: "load" }): Promise<unknown>;
  evaluate<R, A>(fn: (arg: A) => R, arg: A): Promise<R>;
  screenshot(options: { path: string; fullPage: boolean }): Promise<unknown>;
}
interface PwContext {
  newPage(): Promise<PwPage>;
  close(): Promise<void>;
}
interface PwBrowser {
  newContext(options: { viewport: { width: number; height: number }; javaScriptEnabled: boolean }): Promise<PwContext>;
  close(): Promise<void>;
}
interface PwChromium {
  launch(): Promise<PwBrowser>;
}
interface PwModule {
  chromium?: PwChromium;
  default?: { chromium?: PwChromium };
}

interface Measures {
  horizontal_overflow_px: number;
  offscreen_elements: number;
  min_font_px: number | null;
  h1_count: number;
  zero_opacity_elements: number;
  small_tap_targets: number;
  page_height_px: number;
}
interface WidthReport extends Measures {
  external_requests: string[];
  screenshot: string;
}
interface Report {
  file: string;
  generated_at: string;
  javascript: "disabled";
  widths: Record<string, WidthReport>;
  clean?: boolean;
}

interface Args {
  file: string;
  proofDir: string;
  root: string;
  playwright: string | null;
}

function parseArgs(argv: readonly string[]): Args | null {
  const flags = new Map<string, string>();
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i];
    const value = argv[i + 1];
    if (key === undefined || !key.startsWith("--") || value === undefined) return null;
    flags.set(key.slice(2), value);
  }
  const file = flags.get("file");
  if (!file) return null;
  return {
    file,
    proofDir: flags.get("proof-dir") ?? join(dirname(resolve(file)), "proof"),
    root: flags.get("root") ?? process.cwd(),
    playwright: flags.get("playwright") ?? null,
  };
}

const log = (s: string): void => {
  process.stdout.write(`${s}\n`);
};

// Global npm folders are not on Node's lookup path, so they are listed here explicitly.
function globalRoots(): string[] {
  const prefix = process.env["npm_config_prefix"] || process.env["PREFIX"] || null;
  const roots: (string | null)[] = [
    prefix && join(prefix, "lib", "node_modules"),
    join(dirname(dirname(process.execPath)), "lib", "node_modules"),
    "/opt/homebrew/lib/node_modules",
    "/usr/local/lib/node_modules",
    "/usr/lib/node_modules",
    join(homedir(), ".npm-global", "lib", "node_modules"),
  ];
  return roots.filter((r): r is string => Boolean(r));
}

export function findPlaywright(playwright: string | null, root: string): string | null {
  const starts = [playwright, process.cwd(), root, ...globalRoots()].filter((s): s is string => Boolean(s));
  for (const start of starts) {
    try {
      return createRequire(join(resolve(start), "_")).resolve("playwright");
    } catch {
      /* try the next place */
    }
  }
  return null;
}

// Runs inside the page, with the page's own DOM.
function measure(vw: number): Measures {
  const docWidth = Math.max(document.documentElement.scrollWidth, document.body.scrollWidth);
  let offscreen = 0;
  let invisible = 0;
  let minFont = Infinity;
  for (const el of Array.from(document.querySelectorAll("body *"))) {
    const r = el.getBoundingClientRect();
    if (r.right > vw + 0.5 || r.left < -0.5) offscreen++;
    const cs = getComputedStyle(el);
    if (el.textContent && el.textContent.trim()) {
      const fs = parseFloat(cs.fontSize);
      if (fs > 0 && fs < minFont) minFont = fs;
    }
    if (cs.opacity === "0") invisible++;
  }
  let smallTaps = 0;
  for (const el of Array.from(document.querySelectorAll("a, button, summary, label"))) {
    const r = el.getBoundingClientRect();
    if (r.height > 0 && r.height < 44) smallTaps++;
  }
  return {
    horizontal_overflow_px: Math.max(0, docWidth - vw),
    offscreen_elements: offscreen,
    min_font_px: Number.isFinite(minFont) ? Math.round(minFont * 100) / 100 : null,
    h1_count: document.querySelectorAll("h1").length,
    zero_opacity_elements: invisible,
    small_tap_targets: smallTaps,
    page_height_px: document.documentElement.scrollHeight,
  };
}

function firstLine(err: unknown): string {
  const text = err instanceof Error ? err.message : String(err);
  return text.split("\n")[0] ?? "";
}

export async function verify(args: Args): Promise<number> {
  const dest = resolve(args.file);
  const entry = findPlaywright(args.playwright, args.root);
  if (!entry) {
    log("proof     SKIPPED. Playwright was not found. Install it, then tell brf where it is:");
    log("          npm i -g playwright && npx playwright install chromium");
    log('          brf init --output <your folder> --playwright "$(npm root -g)/playwright"');
    return 3;
  }
  const pw = (await import(pathToFileURL(entry).href)) as PwModule;
  const chromium = pw.chromium ?? pw.default?.chromium;
  if (!chromium) {
    log("proof     SKIPPED. Playwright resolved but exposed no Chromium launcher.");
    return 3;
  }
  const outDir = resolve(args.proofDir);
  mkdirSync(outDir, { recursive: true });
  const stem = basename(dest).replace(/\.html?$/i, "");
  const report: Report = { file: dest, generated_at: new Date().toISOString(), javascript: "disabled", widths: {} };
  let clean = true;
  let browser: PwBrowser;
  try {
    browser = await chromium.launch();
  } catch (err) {
    log(`proof     SKIPPED. Playwright could not start Chromium: ${firstLine(err)}`);
    log("          npx playwright install chromium");
    return 3;
  }
  try {
    for (const width of WIDTHS) {
      const context = await browser.newContext({ viewport: { width, height: 900 }, javaScriptEnabled: false });
      const page = await context.newPage();
      const external: string[] = [];
      page.on("request", (r) => {
        if (!r.url().startsWith("file:") && !r.url().startsWith("data:")) external.push(r.url());
      });
      await page.goto(pathToFileURL(dest).href, { waitUntil: "load" });
      const m = await page.evaluate(measure, width);
      const png = join(outDir, `${stem}-${width}.png`);
      await page.screenshot({ path: png, fullPage: true });
      await context.close();
      report.widths[String(width)] = { ...m, external_requests: external, screenshot: png };
      log(
        `proof     w=${String(width).padStart(4)}  overflow ${m.horizontal_overflow_px}px  offscreen ${m.offscreen_elements}  min font ${m.min_font_px}px  h1 ${m.h1_count}  small taps ${m.small_tap_targets}  external ${external.length}`,
      );
      if (
        m.horizontal_overflow_px > 0 ||
        m.h1_count !== 1 ||
        m.zero_opacity_elements > 0 ||
        external.length > 0 ||
        (m.min_font_px !== null && m.min_font_px < 12)
      ) {
        clean = false;
      }
    }
  } finally {
    await browser.close();
  }
  report.clean = clean;
  const reportPath = join(outDir, `${stem}-report.json`);
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  log(`report    ${reportPath}`);
  log(clean ? "VERIFY CLEAN. Now read the 390 and 1440 screenshots." : "VERIFY FAILED. Read the numbers above and the screenshots.");
  return clean ? 0 : 1;
}

const args = parseArgs(process.argv.slice(2));
if (!args) {
  process.stderr.write("usage: verify.ts --file <brief.html> [--proof-dir <dir>] [--root <dir>] [--playwright <path>]\n");
  process.exitCode = 2;
} else {
  verify(args).then(
    (code) => {
      process.exitCode = code;
    },
    (err: unknown) => {
      process.stderr.write(`${err instanceof Error ? (err.stack ?? err.message) : String(err)}\n`);
      process.exitCode = 1;
    },
  );
}
