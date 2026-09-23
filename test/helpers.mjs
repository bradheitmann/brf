import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after } from "node:test";
import { fileURLToPath } from "node:url";

export const ROOT = fileURLToPath(new URL("..", import.meta.url));
export const BIN = join(ROOT, "bin", "brf.mjs");

const made = [];
after(() => {
  for (const dir of made) rmSync(dir, { recursive: true, force: true });
});

export function tempDir(prefix = "brf-test-") {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  made.push(dir);
  return dir;
}

export function gitRepo(dir, { remote = null } = {}) {
  mkdirSync(dir, { recursive: true });
  const g = (...args) => execFileSync("git", ["-c", "user.email=brf-test", "-c", "user.name=test", "-c", "commit.gpgsign=false", ...args], { cwd: dir, stdio: "ignore" });
  g("init", "-q");
  if (remote) g("remote", "add", "origin", remote);
  writeFileSync(join(dir, "README.md"), "test\n");
  g("add", "README.md");
  g("commit", "-q", "-m", "init");
  return dir;
}

// Run the CLI with an isolated config and cache.
export function cli(args, { cwd, home, env = {} } = {}) {
  try {
    const stdout = execFileSync(process.execPath, [BIN, ...args], {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, XDG_CONFIG_HOME: join(home, "cfg"), XDG_CACHE_HOME: join(home, "cache"), BRF_OUTPUT_DIR: "", BRF_CONFIG: "", ...env },
    });
    return { code: 0, stdout, stderr: "" };
  } catch (err) {
    return { code: err.status, stdout: err.stdout || "", stderr: err.stderr || "" };
  }
}
