# The brf protocol

Binding rules for the agent that writes a brief. `SKILL.md` is the short procedure; this file is
the reference it points to. Read all of it before writing a word.

A brief is one self-contained HTML page about one project, written for the project's owner and
delivered to the owner's output folder. It is built from the repository's own records: git,
handoffs, management rollups, planning files, CI, and live deployments. It changes nothing in
the repository.

## Who reads this

The owner runs many projects. They open the brief on a phone, often first thing in the morning
on a walk, before they are fully awake. Write for a capable executive **parachuting into the
project for the first time**, just back from a long vacation, now in charge of it. They remember
the project's name and little else.

- **No machine-assigned names in the body.** No task, story, slice or ticket ids, no codenames
  they would have to decode, no commit hashes, pull-request numbers, file paths or version
  strings. Call each thing by what it does: "the check that stops agents skipping reviews," not
  "enforce-no-verify-gate." Identifiers may appear only in the two appendices and the footer,
  always beside a plain description, because the project manager uses them there.
- **Make it an invitation.** The reader should finish wanting to learn more and confident that
  the project manager (the agent that runs this repository day to day) will take care of them.
  "Your way in" says what the project manager will do for them on day one.
- **Say why it matters.** Every brief says why the project exists, what it makes possible in
  general, and what it unlocks for the owner's other projects, naming them by their everyday
  names from the registry.
- **Gentle, warm-up reading.** Short sentences, one idea each, no acronyms without their plain
  meaning, nothing that needs a second read.

## The registry

`registry.json` in the owner's output folder is the closed list of projects that get briefs.
Brief only those. A project is added only when the owner names it and asks, with
`brf register --reason "<their words>"`. A request for a brief is not a request to register.
Never search the computer, a home folder or a code host for projects to add.

A registry entry records the project's path and remote. Another clone of the same remote is the
same project. A checkout whose remote differs from the entry's, even at the recorded path, is
not: `brf context` stops, and `brf build` and `brf deliver` refuse when run inside it. `brf context` stops on an unregistered repository, and `brf build` and
`brf deliver` refuse one.

## What is in brf

| Path | What it is |
|---|---|
| `SKILL.md` | The `/brf` skill: the procedure an agent follows |
| `PROTOCOL.md` | This file |
| `PROMPT.md` | A paste-in prompt for agents that do not load skills |
| `templates/project.html` | The project brief, with `{{SLOTS}}` and repeat blocks |
| `templates/meta.html` | The meta brief across all registered projects |
| `templates/fonts.css` | The embedded faces; the build injects it |
| `examples/tidewater.filled.html` | A finished, filled example about a fictional project. The tone standard. |
| `bin/brf.mjs` | The command line: context, register, snapshot, build, deliver, check |

## The job, in order

1. **Get the context.** `brf context` from inside the repository. It returns the project's name,
   whether it is registered, the output file name, the brief number, every earlier brief with
   its path, a work folder outside the repository, the other registered projects, and which
   tools exist. If it says `stop`, stop and tell the owner why.
2. **Take the before snapshot.** `brf snapshot --out <work_dir>/before.json`. It records HEAD,
   the current branch, the content of staged, unstaged and untracked changes, ignored paths,
   every ref, the stash list, the worktree list, and HEAD's reflog (so a stash-and-pop or a
   switch away and back shows up as `"changed": ["reflog"]`).
3. **Read the earlier briefs.** The newest is the baseline: its decisions, next steps and risks
   must each be accounted for in "Since the last briefing."
4. **Ground the facts.** Read, in this order: git log since the baseline's date, branches,
   worktrees and stashes; every handoff (`HANDOFF*.md`, `CURRENT*.md`, `CURRENT-STATE.md`,
   `docs/handoffs/`, `docs/agent/*/`); every management rollup (`*rollup*`); decision logs; the
   planning corpus; the live deployment or published package; CI results (`gh run list`); who is
   working and who is not. Machine-local files that git ignores count; say they are local. Write
   a fact sheet in the work folder. Anything you cannot verify is marked `[NOT VERIFIED]` in the
   brief, never guessed. Where a handoff or rollup disagrees with git, git wins and the
   disagreement goes in the brief.
5. **Inventory the planning records** ("Planning records inventory" below).
6. **Inventory code hygiene** ("Code hygiene inventory" below). Read-only.
7. **Fill the Project Card first.** It is the common language across every project. Use only the
   closed vocabulary below.
8. **Write "In one breath"** and **"Why it matters"**, naming other registered projects from the
   context's `portfolio` list (read their current briefs if you need to).
9. **Write at most two decisions**, each a real choice for the owner. Anything that is not the
   owner's call goes in "Next 30 days."
10. **Fill the rest:** while you were away, since the last briefing, right now, your way in,
    next 30 days, risks, glossary, the two inventories, footer.
11. **Build and verify.** `brf build <filled> --out <work_dir>/<output_name> --verify`. Read the
    390 and 1440 screenshots with an image viewer tool, never by opening the owner's browser.
    Fix what you see and rebuild until it is clean.
12. **Check.** `brf check --file <built>`. Fix every problem.
13. **Take the after snapshot.** `brf snapshot --compare <work_dir>/before.json` must say
    `"same": true`. If it does not, stop and tell the owner what changed; do not deliver.
14. **Deliver.** `brf deliver <built>`. It files the project's previous brief in `archive/`.
15. **Reply** with the delivered path, the Project Card as a plain list, the first decision, and
    anything you could not verify.

## The Project Card, closed vocabulary

| Field | Allowed values | Notes |
|---|---|---|
| What it is | one plain sentence | a stranger could read it aloud; name who uses it, including AI agents if they do |
| Who depends on it | named people, teams, projects | the stakes |
| Stage | Idea · Building · Usable · Live · Maintaining · Winding down | badge = the latest rung reached; the sentence after it is the qualifier |
| Health | Healthy · Needs attention · Stuck · On fire | plus one sentence saying why |
| Momentum, last 3 weeks | Fast · Steady · Slow · Stopped | plus the plain reason, e.g. "A lot was finished. None of it went out." |
| Bottleneck | You · Team · Outside party · None | who is holding it |
| Your time needed | None · 5 min · 30 min · Half a day | and by when |
| If you do nothing | one sentence | the cost of neglect; this is the prioritisation signal |
| Attention | 1 to 5 | derived from the rows above; one line of reason that states the trade |

The dial is not a mood. A high cost of neglect and many dependants push it up; a large time
demand on the reader pushes it down. Say the trade in the reason line, e.g. "Nothing is on fire,
and thirty minutes of your time publishes three weeks of finished work."

## Voice, binding

- Plain English a sharp person from another company understands cold. Every project-private
  term is avoided, replaced by its plain meaning, or defined inline in six words or fewer the
  first time. The glossary is for the six you could not avoid, no more.
- Outcomes, not mechanics. "Agents can no longer be handed slide-deck work," not "pptx removed
  from the intent array."
- Decisions are choices: the question in plain words, Option A, Option B, a recommendation, what
  waiting costs, the reader's time. Recommendations state the risk ("the risk is low"), never
  erase it ("nothing can break").
- A number appears only where it changes a decision, and it carries its comparison. Every number
  traces to the fact sheet. No rounding up for effect.
- No praise, no process narration, no methodology, no review-round counts, no exclamation marks,
  no em-dashes. Present tense. Periods.
- Avoid filler words that signal automated writing: honest, genuinely, quietly, crucially,
  load-bearing, leverage, seamless, delve, game-changer, world-class, "not just." `brf check`
  flags them; the owner may add more in the config file (`extra_banned_words`).
- Say what is not done and what has not shipped. A brief that reads cleaner than the project is a
  defect.
- Do not repeat a fact across the card, the breath and the decisions. Each fact lands once, where
  it does the most work.

## Length budget

The card, the breath, why it matters and the two decisions are what the reader must get through.
Target under 520 words for that span. Everything below scrolls and may be fuller, but every
sentence still has a job.

| Section | Target |
|---|---|
| Card, all rows plus the dial reason | ≤ 170 words |
| In one breath | 3 to 4 sentences, ≤ 90 words |
| Why it matters | 2 to 3 sentences, ≤ 60 words, plus 2 to 5 projects it helps, one sentence each |
| Each decision, all six parts | ≤ 100 words |
| Each while-you-were-away item | one sentence; say whether it reached users |
| Since the last briefing | every prior decision and next step, one sentence each, ≤ 8 items |
| Right now | ≤ 80 words; say "nobody" when it is nobody |
| Your way in | 2 sentences on the project manager, plus 2 or 3 first moves |
| Next 30 days | 3 to 5 steps, each with what it unlocks |
| Risks | 3 or 4, each as risk, why, and what would reduce it |
| Glossary | ≤ 6 entries, one line each |
| Planning records | ≤ 8 locations; ≤ 8 drift items, the ones that could cause rework first |
| Code hygiene inventory | all ten categories, fixed order, each detail ≤ 40 words |

## Filling a template

- Copy `templates/project.html` into the work folder and fill it there. Never write inside the
  repository being briefed.
- Every `{{SLOT}}` must be filled or the build refuses. Slots are listed at the top of each
  template with a one-line description.
- Repeat blocks sit between `<!-- repeat: NAME -->` and `<!-- /repeat -->`. Duplicate the block
  once per item, fill each copy, then delete both markers. The build refuses a file that still
  has them.
- Keep the `<meta name="brf-template" ...>` tag. The build reads it and stamps the brief.
- Badges: the class is the vocabulary word in kebab case, e.g. `badge--health-needs-attention`.
  Colour follows the word, never the project: green for Healthy, Fast and None; amber for Needs
  attention, Slow, any named bottleneck and Half a day; red for Stuck, On fire and Stopped. The
  full mapping is in the template's header.
- The attention dial: set `data-filled` on the `dial` element to the number; its dots fill.
- The chart: three rows; each width is a percentage of the largest value (largest = 100). The
  line beneath it says what the shape means.
- Logo: paste an SVG mark into `{{LOGO_SVG}}` or leave it empty; empty collapses. Plain shapes
  and paths only: `brf check` refuses images, links, styles and scripts inside the SVG.
- Markup: add no tags, attributes or CSS beyond what the template uses. `brf check` accepts only
  brf's allowlist: the template's elements, links that need a tap (https, http, mailto, #),
  `style` attributes with plain values such as `width:40%`, and CSS whose only `url()` targets
  are embedded data.
- Theme: the `--brf-*` custom properties at the top of the style element hold the palette,
  spacing, radii and type stops. To rebrand, replace the colour half only; the layout does not
  depend on the brand values.
- The build strips every HTML comment, including yours.

## Naming and filing

The output folder is chosen by the owner (`brf init --output <folder>`). Its layout:

```
<output folder>/
  registry.json                  the projects the owner asked for
  brf_<repo-name>_<yyyymmdd>.html   one current brief per project
  meta_brf_<yyyymmdd>.html        one current meta brief
  archive/                        every earlier brief, never deleted
```

- `repo-name` is the code-host repository name in lower case with words joined by hyphens
  (`My_Project` becomes `my-project`); with no remote, the folder name the same way. `brf context`
  works it out; the registry may pin a different name for a path.
- `yyyymmdd` is the date the brief describes.
- An earlier brief from the same day is archived as `..._<yyyymmdd>-v1.html`, `-v2`, in the order
  they were made. The current file keeps the plain name.
- Another format of the same brief adds a variant after the date:
  `brf_my-project_20260131_dark.pdf`.
- `brf deliver` does the filing. It touches only files of the project being delivered, never
  overwrites, and never deletes. `brf build` refuses to write into the output folder, the
  current repository, or any registered project's folder; build in the work folder. Both
  `brf build` and `brf deliver` refuse a page outside the markup allowlist.

## Versions

Every brief carries three numbers, as meta tags and as a line at the foot of the page:

- **Brief number:** the count of briefs for that project, 1 for the first. `brf build` works it
  out from the output folder and `brf deliver` refuses a brief whose number is no longer next.
- **brf version:** the version in `package.json`, semantic versioning.
- **Template version:** from the template's `brf-template` tag. A major bump means filled files
  from the old template no longer build (a slot was renamed or removed); a minor bump adds
  something optional; a patch changes wording or style only. Every change is in `CHANGELOG.md`.

## Incorporating the prior brief

The new brief continues the old one; it does not start over. In "Since the last briefing," list
each decision the prior brief asked for and each of its next steps, and say in one sentence what
happened to it: done, partly done, dropped, or still waiting. Carry forward any prior risk that is
still open. If a prior claim turns out to have been wrong, say so plainly. For a project's first
brief, write "First brief for this project." with one item naming what the reader should know
first.

## Planning records inventory

The reader uses this to see where planning lives and whether the project manager has kept it
true, so nobody rebuilds something that is already built and committed.

1. **Find every location.** Planning corpora (`planning/`, `docs/planning/`, `specs/`, `slices/`,
   `stories/`, `epics/`), status and ledger files (`_STATUS.md`, `CURRENT-STATE.md`,
   `*LEDGER*.md`, task JSON), roadmaps and backlogs (`ROADMAP.md`, `TODO.md`, `BACKLOG.md`),
   machine-local planning that git ignores (say it is local only), and the code host's issues
   and projects.
2. **Count by state** at each location (for example 12 done, 3 active, 5 to do) with the date and
   author of the last change (`git log -1 --format='%cs %an' -- <path>`).
3. **Match to the code.** For work marked done, find the commit or pull request that delivered
   it. For work marked to do or active, check whether it was already built: search commit
   messages for the item's id (`git log --all --grep=<ID>`), look for the files it names, and
   check merged pull requests.
4. **Give each location one verdict:** Current (matches the code), Stale (behind the code but not
   wrong), Contradicts code (says something is not built when it is, or the reverse), Archive
   (historical, not maintained on purpose).
5. **List the drift**, the mismatches that could cause rework first. Lead with the plain name and
   put the identifier after it: "The status-page generator is marked to do (TASK-142). It was
   built and merged on 22 September (pull request #40). Do not rebuild it." If there is none:
   "No drift found."
6. **Say in the summary** whether the project manager has been updating the records and where the
   truth actually lives.

## Code hygiene inventory

An inventory for the reader, not a cleanup. Change nothing.

- **Prove it.** The before and after snapshots must match; say so in the summary.
- **Use Mister Clean if it is installed**: `brf context` reports it, with its version, under
  `tools.mister_clean`. Audit mode only, from the repository root, every output written into the
  work folder:
  `mister-clean audit planning . --json`, `mister-clean audit github-actions . --json`,
  `mister-clean audit repository-boundaries . --json`,
  `mister-clean audit public-safety . --tracked --json`, `mister-clean detect stack .`.
  Never run anything that prepares, fixes or writes. Give each command a time limit; one that
  fails or times out makes its category "Not inspected" with the reason.
- **Read the rest by hand.** Without Mister Clean, read all ten by hand.
- **Ten categories, this order, every time:**
  1. Git state and ownership: uncommitted work, stray branches, stashes, worktrees, who owns what.
  2. Planning records: from the inventory above.
  3. Public safety: keys, tokens, personal data, private paths in tracked files.
  4. Repository boundaries: code or data copied in from other projects.
  5. CI and deploy workflows: failing runs, unpinned third-party actions, broad permissions.
  6. Start-here docs and README: can a newcomer install, run and test from them.
  7. Structure and naming: folders and names that match what they hold.
  8. Dead or duplicate code: unused files, two things doing one job.
  9. Tests: present, passing, covering what matters.
  10. Generated files and dependencies: build output in git, stale or unpinned dependencies.
- **One status word each:** Clean (inspected, nothing found) · Minor (cosmetic, no risk) · Needs
  cleanup (slows the next person or will cause a mistake) · Serious (can leak data, lose work, or
  ship something broken) · Not inspected (say why). Give the count and the worst example with
  its path, in plain words.

## The meta brief

One page across every registered project: `meta_brf_<yyyymmdd>.html`, from
`templates/meta.html`, run with `/brf meta` after the project briefs. It is built from the current
briefs in the output folder only, never from a fresh look at the repositories. `brf context
--meta` lists them. It answers three questions: what is each project in one line, what does each
unlock for the others, and in what order should they get attention.

- **Start here today:** the two or three projects that most deserve the morning, and why.
- **Decisions waiting on you:** every open decision from every current brief, one line each, with
  the time it needs.
- **The sequence:** every registered project, numbered. Order by (1) what others wait on: a
  project that unblocks several others comes before them; (2) the attention score; (3) the cost
  of doing nothing; (4) the reader's time, less first when all else is equal. Each entry:
  everyday name, Stage and Health badges, attention score, one line on what it is, what it
  unlocks for named projects, what it is waiting on, and its brief's file name.
- **How the projects connect:** the dependency chains in plain sentences.
- **Missing or out of date:** registered projects with no current brief, or one older than 14
  days.
- Same voice rules. Deliver it with `brf deliver`; the previous meta brief moves to `archive/`.

## Verifying

`brf build --verify` renders the page with JavaScript off at 360, 390, 768, 1024 and 1440 wide,
screenshots each, and fails on horizontal overflow, a missing or extra top heading, invisible
elements, text under 12px, or any request that leaves the file. It needs Playwright with
Chromium; brf finds it through the config file's `playwright` path, `BRF_PLAYWRIGHT`, or a
`node_modules` above the working folder, or a global npm folder. If none is found it says the
proof was skipped and exits 3, and the reply to the owner must say so too. The owner sets the
path once with `brf init --output <folder> --playwright "$(npm root -g)/playwright"`.

Read the screenshots, not only the numbers. What the numbers cannot catch: a heading doing
another heading's job, two type sizes colliding in one sentence, a table that crushes at phone
width, a chart you need a legend to read.

## What must be true before you deliver

- A stranger from another company understands every sentence.
- No machine-assigned name appears before the appendices.
- The card uses only the closed vocabulary, in the fixed order, with a dial reason that states
  the trade.
- Each decision is a real choice for the owner, with its risk stated.
- Every number traces to a fact you verified. Anything unverified says so.
- It says what has not shipped.
- Every decision and next step from the prior brief is accounted for.
- "Why it matters" names at least two other registered projects it helps, or says plainly that
  it stands alone.
- Every planning location has a verdict, and the drift list names anything already built.
- The hygiene inventory covers all ten categories and says the repository was unchanged.
- `brf build --verify` and `brf check` are clean, and you looked at the phone and laptop
  screenshots. If the render check was skipped (exit 3), everything else here is still true
  and your reply says the page was not render-checked.
