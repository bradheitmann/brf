import assert from "node:assert/strict";
import { test } from "node:test";
import { formatName, humanDate, isBrief, isRealDate, parseName, toRepoName } from "../lib/naming.mjs";

test("parses project, meta, same-day and variant names", () => {
  assert.deepEqual(parseName("brf_mister-clean_20260923.html"), { kind: "project", key: "mister-clean", repo: "mister-clean", date: "20260923", v: null, variant: null, ext: "html" });
  assert.equal(parseName("meta_brf_20260923.html").kind, "meta");
  assert.equal(parseName("brf_harbor-log_20260923-v2.html").v, 2);
  const pdf = parseName("brf_tidewater_20260914_dark.pdf");
  assert.equal(pdf.variant, "dark");
  assert.equal(isBrief(pdf), false);
  assert.equal(isBrief(parseName("brf_tidewater_20260914.html")), true);
});

test("rejects names that break the convention", () => {
  for (const bad of ["brf_Tide_20260923.html", "brf_tide_2026-09-23.html", "brf_tide_20260231.html", "2026-09-23-tide-briefing.html", "brf__20260923.html", "brf_tide_water_20260923.html"]) {
    assert.equal(parseName(bad), null, bad);
  }
});

test("formats what it parses", () => {
  for (const n of ["brf_a-b_20260101.html", "meta_brf_20261231-v3.html", "brf_x_20260102_audio.m4a"]) {
    assert.equal(formatName(parseName(n)), n);
  }
});

test("derives repository names", () => {
  assert.equal(toRepoName("Harbor_Log_Service"), "harbor-log-service");
  assert.equal(toRepoName("tide.water.git"), "tide-water");
  assert.equal(toRepoName("--Odd  Name!!"), "odd-name");
});

test("knows real dates", () => {
  assert.equal(isRealDate("20240229"), true);
  assert.equal(isRealDate("20250229"), false);
  assert.equal(humanDate("20261102"), "2 November 2026");
});
