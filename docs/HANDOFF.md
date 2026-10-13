# brf · handoff note · 2026-10-13

Written by the PM of bradheitmann-web after a site-wide copy review. This repository is public, so this note stays product-level.

## What changed here

Commit `b1ff068` (pushed): in `templates/project.html` and `templates/meta.html`, the section heading "What it unlocks" is now "What depends on it" and the meta-brief row label "Unlocks" is "Depends on it"; "Every project, in order" is "All projects, in order". The CSS class names (`.unlocks`) are unchanged. The protocol vocabulary in `PROTOCOL.md` was not touched; if it names the "unlocks" section, update it to match.

## Why

The example briefs that ship with brf are embedded on bradheitmann.ai, and the owner's copy rules for anything a reader sees there are: plain sentences that say what a thing does; no marketing verbs ("unlock"), no sentence-opening "Every", no slogan pairs, no claims that a sample is "real". The site's example copies (`bradheitmann-web/public/demo/brf/*.html`) also had their sample prose adjusted ("All closed issues point at the change that fixed them", "No emergency. Five minutes fixes the one error…"); those live in the site repo, not here, because they are rendered output, not templates.

## How the site describes brf now

The bradheitmann.ai page gives the install steps from this README: clone, `cargo install`, link the clone into the agent's skills folder (`~/.claude/skills/brf` for Claude Code), `brf init --output <folder>`, then `/brf` inside a project, which writes `brf_<repo>_<date>.html` into that folder. Keep the README's install section accurate and the site page stays accurate.

## Suggestions for this repo's PM (judgement)

- Run the site's copy rules over `README.md`, `PROTOCOL.md` and the template prose: `bradheitmann-web/scripts/copy-rules.ts` has them as checks.
- The template comments instruct agents in the same voice rules ("Prose slots take plain sentences"); the sample content in `templates/*.html` is the first thing a new user reads, so it is worth a plain-language pass of its own.

## Note for the next release (2026-10-13)

`b1ff068` changed the shipped template text without bumping the template version meta tags (still 2.0.0) or CHANGELOG.md, so released 2.0.0 binaries carry the old headings. Bump or annotate before the next release.
