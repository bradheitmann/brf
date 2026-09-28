# Changelog

brf follows semantic versioning. Each template carries its own version in its
`<meta name="brf-template">` tag, and every brief records the brf version and template version it
was built with.

## brf 1.1.0

- Owner templates: `brf init --templates-dir <folder>` (or `BRF_TEMPLATES_DIR`) points brf at your own
  `project.html`, `meta.html` and `fonts.css`, for your own brand. Each file the folder lacks comes from the
  built-in templates. Your templates keep their own `brf-template` tag and version.
- Templates: project 1.0.0 and meta 1.0.0 (unchanged).

## brf 1.0.0

First public release.

- Templates: **project 1.0.0** and **meta 1.0.0**.
- `/brf` skill, the protocol, and the `brf` command line: `init`, `config`, `context`,
  `register`, `snapshot`, `build`, `deliver`, `check`, `version`.
- Briefs are named `brf_<repo-name>_<yyyymmdd>.html`; the meta brief is `meta_brf_<yyyymmdd>.html`.
- Every brief is stamped with its brief number, the brf version and the template version.
- The owner's registry lives in the output folder; only registered projects are briefed.
- Delivery archives the project's previous brief, never overwrites and never deletes; builds
  are refused inside the output folder, the current repository and any registered project.
- `brf build`, `brf deliver` and `brf check` accept only brf's markup allowlist, read the way a
  browser reads HTML, so a brief cannot run code or fetch from the network when opened.
- `brf snapshot` proves the briefed repository did not change: HEAD, branch, staged, unstaged
  and untracked content, ignored paths, refs, stashes, worktrees and HEAD's reflog.
