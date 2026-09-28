# brf

**brf** (say "brief") gives each of your projects a re-entry brief: one page you can read on your
phone that tells you, as if you had just come back from a long vacation, what the project is,
why it matters, what happened while you were away, and what it needs from you.

Your coding agent writes it from the repository's own records: git history, handoffs, planning
files, CI and live deployments. The agent changes nothing in the repository. The page is a single
HTML file with its fonts inside it and no scripts, so it opens offline from any phone's files app.

## What a brief contains

1. **The Project Card:** what it is, who depends on it, stage, health, momentum, who is holding it
   up, how much of your time it needs, the cost of doing nothing, and an attention score from 1
   to 5. The same closed vocabulary on every project, so briefs compare at a glance.
2. **In one breath**, **why it matters** and what it unlocks for your other projects.
3. **At most two decisions**, each a real choice with a recommendation.
4. While you were away, since the last brief, right now, your way in, the next 30 days, risks,
   and a short glossary.
5. Two appendices for the project manager: a **planning records inventory** that shows where
   plans live and whether they match what is built, and a read-only **code hygiene inventory**.

The **meta brief** is one page across all your projects: where to start today, every decision
waiting on you, and the order to give projects attention, with what each unlocks for the others.

`examples/tidewater.filled.html` is a complete example about a fictional project.

## Install

brf is an agent skill plus a small command line with no dependencies. You need Node 20 or newer
and git.

```sh
git clone https://github.com/bradheitmann/brf.git ~/brf
```

Make `/brf` available to your agent by linking the clone into its skills folder. For Claude Code:

```sh
ln -s ~/brf ~/.claude/skills/brf
```

Agents that do not load skills can use `PROMPT.md` instead.

Optional:

- **Playwright with Chromium** for the render check at five screen widths. Install it, then tell
  brf where it is when you run `init` (below):
  `npm i -g playwright && npx playwright install chromium`, then add
  `--playwright "$(npm root -g)/playwright"` to `brf init`.
- **Mister Clean** for the hygiene audit (`npm i -g @bradheitmann/mister-clean`). Without it the
  agent inspects the ten categories by hand.
- **GitHub CLI** (`gh`) for pull requests, issues and CI runs.

## Choose where briefs go

brf never picks the folder for you. Choose one you can open on your phone, such as a synced
cloud-drive folder, then run:

```sh
node ~/brf/bin/brf.mjs init --output "<your folder>"
```

This creates the folder with an empty `registry.json` and an `archive/` folder, and records the
path in `~/.config/brf/config.json`. On a second computer that syncs the same folder, run the same
command; the existing registry is kept. `BRF_OUTPUT_DIR` overrides the config for one shell.

Other settings in the config file: `playwright` (a path from which Playwright resolves),
`gh_user` (the `gh` account that can read your repositories), `extra_banned_words`, and
`templates_dir`: a folder with your own `project.html`, `meta.html` and `fonts.css` in your own design
system. brf uses each file it finds there instead of its built-in one. Keeping that folder next to your
briefs means every machine that syncs the output folder uses the same design.

## Choose which projects get briefs

Only projects in your registry get briefs. Agents never go looking for projects, and asking for a
brief does not register one. To add a project, say so explicitly from inside that repository;
the agent records your words:

```sh
node ~/brf/bin/brf.mjs register --reason "Brief this one every Monday" --everyday-name "Tidewater"
```

Each entry records the project's folder and remote. If a project moves or its repository is
renamed, update `local_path` and `remote` in `registry.json` yourself; brf refuses a checkout
whose remote no longer matches its entry.

## Use it

Inside a registered repository, ask your agent for `/brf`. It writes one brief for that project
and delivers it. `/brf meta` writes the meta brief from the current project briefs.

The output folder:

```
<your folder>/
  registry.json
  brf_<repo-name>_<yyyymmdd>.html   one current brief per project
  meta_brf_<yyyymmdd>.html          the current meta brief
  archive/                          every earlier brief; nothing is deleted
```

## Versions

Each brief shows, at its foot and in its meta tags, its **brief number** for that project, the
**brf version**, and the **template version** it was built from. Template versions follow semantic
versioning on their own; see `CHANGELOG.md`.

## The command line

| Command | What it does |
|---|---|
| `brf init --output <folder>` | Set up the output folder and point this computer at it |
| `brf context [--meta]` | JSON for the agent: project name, registration, file name, brief number, earlier briefs, work folder |
| `brf register --reason "<words>"` | Add the current repository to the registry |
| `brf snapshot --out <file>` / `--compare <file>` | Record the repository's state and prove it did not change |
| `brf build <filled> --out <work_dir>/<name> [--verify]` | Embed fonts, refuse unfilled slots, stamp versions, render-check |
| `brf deliver <built>` | File the brief; the project's previous brief moves to `archive/` |
| `brf check [--file <brief>]` | Check one brief, or the whole folder, against the protocol |
| `brf version` | Print the brf and template versions |

Run it as `node ~/brf/bin/brf.mjs <command>`, or put `bin/brf.mjs` on your `PATH` as `brf`.

## Privacy

This repository holds templates, the protocol, the skill and the command line. Your briefs, your
registry and your settings stay in your output folder and your config file, never here.

## Development

```sh
npm test
```

Protocol and voice rules: `PROTOCOL.md`. Agent procedure: `SKILL.md`. Changes: `CHANGELOG.md`.

## License

MIT for brf. The embedded fonts (Cormorant Garamond, Inter, JetBrains Mono) are under the SIL Open
Font License 1.1; see `fonts/OFL.txt`.
