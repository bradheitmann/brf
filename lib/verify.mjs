// Render the finished page with JavaScript off at five widths, screenshot each, and
// measure what a reader would trip on. Needs Playwright with Chromium; brf itself has
// no dependencies, so Playwright is found, never installed.
import { mkdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { createRequire } from "node:module";
import { basename, dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { ROOT } from "./templates.mjs";

export const WIDTHS = [360, 390, 768, 1024, 1440];

// Global npm folders are not on Node's lookup path, so they are listed here explicitly.
function globalRoots() {
  const prefix = process.env.npm_config_prefix || process.env.PREFIX || null;
  return [
    prefix && join(prefix, "lib", "node_modules"),
    join(dirname(dirname(process.execPath)), "lib", "node_modules"),
    "/opt/homebrew/lib/node_modules",
    "/usr/local/lib/node_modules",
    "/usr/lib/node_modules",
    join(homedir(), ".npm-global", "lib", "node_modules"),
  ].filter(Boolean);
}

export function findPlaywright(cfg = {}) {
  const starts = [cfg.playwright, process.cwd(), ROOT, ...globalRoots()].filter(Boolean);
  for (const start of starts) {
    try {
      return createRequire(join(resolve(start), "_")).resolve("playwright");
    } catch { /* try the next place */ }
  }
  return null;
}

export async function verify({ file, proofDir = null, cfg = {}, log = () => {} }) {
  const dest = resolve(file);
  const entry = findPlaywright(cfg);
  if (!entry) {
    log("proof     SKIPPED. Playwright was not found. Install it, then tell brf where it is:");
    log("          npm i -g playwright && npx playwright install chromium");
    log("          brf init --output <your folder> --playwright \"$(npm root -g)/playwright\"");
    return { skipped: true, clean: null, report: null };
  }
  const pw = await import(pathToFileURL(entry).href);
  const chromium = pw.chromium ?? pw.default?.chromium;
  if (!chromium) {
    log("proof     SKIPPED. Playwright resolved but exposed no Chromium launcher.");
    return { skipped: true, clean: null, report: null };
  }
  const outDir = resolve(proofDir || join(dirname(dest), "proof"));
  mkdirSync(outDir, { recursive: true });
  const stem = basename(dest).replace(/\.html?$/i, "");
  const report = { file: dest, generated_at: new Date().toISOString(), javascript: "disabled", widths: {} };
  let clean = true;
  let browser;
  try {
    browser = await chromium.launch();
  } catch (err) {
    log(`proof     SKIPPED. Playwright could not start Chromium: ${String(err.message || err).split("\n")[0]}`);
    log("          npx playwright install chromium");
    return { skipped: true, clean: null, report: null };
  }
  try {
    for (const width of WIDTHS) {
      const context = await browser.newContext({ viewport: { width, height: 900 }, javaScriptEnabled: false });
      const page = await context.newPage();
      const external = [];
      page.on("request", (r) => { if (!r.url().startsWith("file:") && !r.url().startsWith("data:")) external.push(r.url()); });
      await page.goto(pathToFileURL(dest).href, { waitUntil: "load" });
      const m = await page.evaluate((vw) => {
        const docWidth = Math.max(document.documentElement.scrollWidth, document.body.scrollWidth);
        let offscreen = 0, invisible = 0, minFont = Infinity;
        for (const el of document.querySelectorAll("body *")) {
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
        for (const el of document.querySelectorAll("a, button, summary, label")) {
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
      }, width);
      const png = join(outDir, `${stem}-${width}.png`);
      await page.screenshot({ path: png, fullPage: true });
      await context.close();
      report.widths[width] = { ...m, external_requests: external, screenshot: png };
      log(`proof     w=${String(width).padStart(4)}  overflow ${m.horizontal_overflow_px}px  offscreen ${m.offscreen_elements}  min font ${m.min_font_px}px  h1 ${m.h1_count}  small taps ${m.small_tap_targets}  external ${external.length}`);
      if (m.horizontal_overflow_px > 0 || m.h1_count !== 1 || m.zero_opacity_elements > 0 || external.length > 0 || (m.min_font_px !== null && m.min_font_px < 12)) clean = false;
    }
  } finally {
    await browser.close();
  }
  report.clean = clean;
  const reportPath = join(outDir, `${stem}-report.json`);
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  log(`report    ${reportPath}`);
  log(clean ? "VERIFY CLEAN. Now read the 390 and 1440 screenshots." : "VERIFY FAILED. Read the numbers above and the screenshots.");
  return { skipped: false, clean, report: reportPath, screenshots: WIDTHS.map((w) => join(outDir, `${stem}-${w}.png`)) };
}
