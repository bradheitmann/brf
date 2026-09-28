---
name: brf
description: Writes a re-entry brief (a "brf") for the git repository you are working in, as one phone-readable HTML page delivered to the owner's brf output folder. Use when the user runs /brf, or asks for a brief, briefing or brf of this project. "/brf meta" builds the meta brief across all registered projects instead. Read-only toward the repository; briefs only projects in the owner's registry.
---

# /brf

Write one brief for the repository you are standing in, then deliver it. With the argument
`meta`, write the meta brief across every registered project instead. Nothing else: never brief
another repository, never go looking for projects, never write inside the repository.

`PROTOCOL.md` beside this file is binding. Read it in full before you start. The finished examples
are `examples/tidewater.filled.html` and `examples/meta.filled.html`; read them for tone and length.
A brief is big picture first: a status strip, the big idea, what it unlocks, what is missing for it
to work, related projects, at most one decision and eight weeks of activity, in about 180 words.
Everything else is collapsed.

## The command

In this file, `<skill>` is the directory that contains this `SKILL.md`. Every brf command is

```
node "<skill>/bin/brf.mjs" <command> ...
```

written out in full each time, with `<skill>` replaced by the real path. Do not keep it in a
shell variable: some shells do not split one, and some agents do not keep variables between
commands. It needs Node 20 or newer and git. Playwright with Chromium is needed for the render
check; Mister Clean and the GitHub CLI are optional.

## Rules that stop the job

- `brf context` prints JSON. If it has a `stop` field, stop and tell the owner what it says.
- If it fails with "No output folder is set", tell the owner to choose a folder and run
  `node "<skill>/bin/brf.mjs" init --output <folder>`. Do not choose one for them.
- If it says the output folder "is not there", stop and tell the owner. It may be on a drive or
  sync service that is not available yet. Never run `init` yourself to recreate it.
- Never register a project on your own. A request for a brief, or the owner typing `/brf`, is
  **not** a request to register. Run `brf register` only when the owner explicitly says to add
  this project to their brf registry, and quote their words:
  `node "<skill>/bin/brf.mjs" register --reason "<their words>" --everyday-name "<name>"`.
- If the after snapshot differs from the before snapshot, do not deliver. Tell the owner what
  changed.

## The repository is read-only

Do not write, stage, commit, stash, check out, switch, pull, fetch, install dependencies, or run
builds, generators or formatters inside the repository. Other sessions may be working there. Use
read-only commands only, and run git as `git --no-optional-locks ...` so that even `status` does
not rewrite the index: log, show, status, diff, branch --list, worktree list, stash list. File
reads, `gh` queries and audits in read-only mode are fine. Put every file you make in the `work_dir` from the
context; brf refuses to build inside the repository. Never open the owner's browser. Never print
tokens or key values.

## Project brief

1. `node "<skill>/bin/brf.mjs" context` and read its JSON. Note `output_name`, `brief_number`,
   `prior_briefs`, `work_dir`, `portfolio` and `tools`.
2. `node "<skill>/bin/brf.mjs" snapshot --out "<work_dir>/before.json"`
3. Read every earlier brief in `prior_briefs`, newest first. The newest is the baseline.
4. Ground the facts, then write `<work_dir>/FACTS.md` (PROTOCOL.md, "The job, in order", step 4).
   Put `tools.gh_prefix` in front of `gh` when it is set.
5. Planning records inventory and code hygiene inventory, exactly as PROTOCOL.md says. Use
   Mister Clean only if `tools.mister_clean` is set. All audit output goes to `work_dir`.
   Count commits per week for the last eight weeks:
   `git --no-optional-locks log --all --since="8 weeks ago" --format=%cs`, grouped by the Monday
   of each week, oldest first, zero for empty weeks.
6. Copy the project template named in the context's `templates.project.path` (the owner's own template
   when they set one, otherwise `<skill>/templates/project.html`) to `<work_dir>/<repo_name>.filled.html`
   and fill it in this order: status strip (Stage, Health, Momentum, attention dial and reason), the
   big idea, what it unlocks (up to 3), what is missing to be functional (up to 5, with the count),
   related projects (up to 5), at most one decision (delete the block if none), the eight activity
   weeks; then the collapsed sections: what changed since the last brief, risks, planning records,
   code hygiene, footer. Leave `{{BRIEF_NUMBER}}` as it is; the build writes it.
7. Build and render-check:
   `node "<skill>/bin/brf.mjs" build "<work_dir>/<repo_name>.filled.html" --out "<work_dir>/<output_name>" --verify`
   Read the 390 and 1440 screenshots in `<work_dir>/proof/` with your image viewer tool. Fix
   and rebuild until it prints VERIFY CLEAN. The build prints the main view's word count: cut
   until it is about 180. Exit code 3 means the render check was skipped
   (Playwright or Chromium is missing or would not start).
8. `node "<skill>/bin/brf.mjs" check --file "<work_dir>/<output_name>"` and fix every problem.
9. Reread the main view as the owner on a morning walk. Remove any identifier, jargon or
   sentence that needs a second read. Rebuild and recheck if you changed anything.
10. `node "<skill>/bin/brf.mjs" snapshot --compare "<work_dir>/before.json"` must print
    `"same": true`.
11. `node "<skill>/bin/brf.mjs" deliver "<work_dir>/<output_name>"`
12. Reply with: the delivered path; Stage, Health, Momentum, attention and the big idea as a plain
    list; the decision if there is one; and everything you could not verify, including a skipped
    render check.

## Meta brief (`/brf meta`)

1. `node "<skill>/bin/brf.mjs" context --meta` and read it. It lists every registered project
   and its current brief, and gives `output_name`, `brief_number` and `work_dir`.
2. Read each current brief's main view: the status strip, the big idea, what it unlocks, the
   missing-to-functional count, related projects and the decision. Do not look at the repositories
   themselves. A brief built from an older template has the same facts under other headings.
3. Copy the meta template named in `templates.meta.path` from `context --meta` to `<work_dir>/meta.filled.html` and fill it as
   PROTOCOL.md, "The meta brief", says. Registered projects with no current brief go under
   "Missing or out of date".
4. `node "<skill>/bin/brf.mjs" build "<work_dir>/meta.filled.html" --out "<work_dir>/<output_name>" --verify`,
   read the screenshots, run `check --file`, then `deliver`.
5. Reply with the delivered path and the ranked list, one line per project.

## If a step fails

- A build that says "Refused" lists the slots or repeat markers still to fill. Fill them.
- An error ending in `(BRIEF_NUMBER)` at delivery means another brief of this project landed
  first. Run `context` again and rebuild.
- If the render check was skipped, deliver only if everything else is clean, and say in your
  reply that the page was not render-checked.
