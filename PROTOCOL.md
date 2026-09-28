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
project**, just back from a long vacation. They remember the project's name and little else, and
they want the big picture in one screen: what it is for, what it unlocks, what is missing before it
works, and whether anything needs them today.

- **Picture first, few words.** The main view is a status strip, a big idea, short lists, a meter and
  a chart. Everything else is collapsed. If a sentence can be a checklist item or a chip, make it one.
- **No machine-assigned names in the main view.** No task, story, slice or ticket ids, no codenames
  they would have to decode, no commit hashes, pull-request numbers, file paths or version
  strings. Call each thing by what it does: "the check that stops agents skipping reviews," not
  "enforce-no-verify-gate." Identifiers may appear only in the two appendices (planning records and
  code hygiene) and the footer, always beside a plain description.
- **Say why it matters.** The big idea says why the project exists; "What it unlocks" and "Related
  projects" name the owner's other projects by their everyday names from the registry.
- **Gentle reading.** Short sentences, one idea each, no acronyms without their plain meaning,
  nothing that needs a second read.

## The registry

`registry.json` in the owner's output folder is the closed list of projects that get briefs.
Brief only those. A project is added only when the owner names it and asks, with
`brf register --reason "<their words>"`. A request for a brief is not a request to register.
Never search the computer, a home folder or a code host for projects to add.

A registry entry records the project's path and remote. Another clone of the same remote is the
same project. A checkout whose remote differs from the entry's, even at the recorded path, is
not: `brf context` stops, and `brf build` and `brf deliver` refuse when run inside it. `brf context` stops on an unregistered repository, and `brf build` and
`brf deliver` refuse one.

**Projects on another computer.** When a registered project lives on another of the owner's
computers, read it there with read-only commands only (the same list as in `SKILL.md`), bring the
facts back, and build and deliver on this computer. Never change another computer's settings,
services or network configuration, and follow any agent rules that computer's repositories carry.

## What is in brf

| Path | What it is |
|---|---|
| `SKILL.md` | The `/brf` skill: the procedure an agent follows |
| `PROTOCOL.md` | This file |
| `PROMPT.md` | A paste-in prompt for agents that do not load skills |
| `templates/project.html` | The project brief, with `{{SLOTS}}` and repeat blocks |
| `templates/meta.html` | The meta brief across all registered projects |
| `templates/fonts.css` | The embedded faces; the build injects it |
| `examples/tidewater.filled.html` | A filled project brief about a fictional project. The tone and length standard. |
| `examples/meta.filled.html` | A filled meta brief for the same fictional portfolio |
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
3. **Read the earlier briefs.** The newest is the baseline: its decision, its missing items and its
   risks must each be accounted for in "What changed since the last brief."
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
7. **Count the activity.** Commits per week for the last eight weeks, oldest first, on all
   branches: `git --no-optional-locks log --all --since=<8 weeks ago> --format=%cs` and count by
   the Monday of each week. Weeks with none are 0.
8. **Fill the main view** ("The main view" below): the status strip first, then the big idea, what
   it unlocks, what is missing, related projects, at most one decision, activity. Name other
   registered projects from the context's `portfolio` list (read their current briefs if you need to).
9. **Fill the collapsed sections:** what changed since the last brief, risks, the two
   inventories, the footer.
10. **Build and verify.** `brf build <filled> --out <work_dir>/<output_name> --verify`. Read the
    390 and 1440 screenshots with an image viewer tool, never by opening the owner's browser.
    Fix what you see and rebuild until it is clean. The build prints the main view's word count.
11. **Check.** `brf check --file <built>`. Fix every problem.
12. **Take the after snapshot.** `brf snapshot --compare <work_dir>/before.json` must say
    `"same": true`. If it does not, stop and tell the owner what changed; do not deliver.
13. **Deliver.** `brf deliver <built>`. It files the project's previous brief in `archive/`.
14. **Reply** with the delivered path, the status strip and the big idea as a plain list, the
    decision if there is one, and anything you could not verify.

## The main view

Everything outside the collapsed sections. About 180 words in all, labels included; `brf build`
prints the count. In this order:

| Block | What goes in it |
|---|---|
| Header | Logo (optional), the project's everyday name, the date, the brief number (the build writes it) |
| Status strip | Stage, Health and Momentum as badges from the closed lists below; the attention dial, 1 to 5, with one line of reason |
| The big idea | One or two sentences: what the project is for and why it matters |
| What it unlocks | Up to three short items: what it makes possible, and for whom, naming registered projects where true |
| What's missing to be functional | A checklist of up to five things that must exist for the project to do its job, each In place or Missing, and the count drawn as a meter ("3 of 5 in place"). When all are in place, list the ones that matter most, all In place. |
| Related projects | Up to five chips: a registered project and its relation in a word or two (feeds, depends on, shares, replaces, uses, checked by) |
| One decision | At most one: the question, the answer you recommend with its risk, and the owner's time. Leave the block out when nothing needs the owner. |
| Activity | Commits per week for the last eight weeks as bars (from git), and one line on what the shape means |

### Closed vocabulary

| Field | Allowed values | Notes |
|---|---|---|
| Stage | Idea · Building · Usable · Live · Maintaining · Winding down | the latest rung reached |
| Health | Healthy · Needs attention · Stuck · On fire | |
| Momentum, last 3 weeks | Fast · Steady · Slow · Stopped | |
| Attention | 1 to 5 | one line of reason that states the trade |

The dial is not a mood. A high cost of neglect and many dependants push it up; a large demand on
the reader's time pushes it down. Say the trade in the reason line, e.g. "Nothing is on fire, and
five minutes of your time fixes the one error that could hurt paddlers."

The checklist is the answer to "what would it take for this to work?", not a task list. Each item
is a capability or a thing that exists ("Tide tables for next year"), in a few words, in the order
it will be done. The meter and the count come from the same items.

### The collapsed sections

Each is a closed `<details>` block the reader opens only if they want more:

- **What changed since the last brief.** Every decision, missing item and risk from the prior brief
  and what happened to it (done, partly done, dropped, still waiting), plus anything that shipped.
  One sentence each, at most eight. For a first brief: "First brief for this project." and one item
  naming what the reader should know first. If a prior claim was wrong, say so.
- **Risks.** Two to four, each as the risk in one line and what would reduce it (and whether that
  exists yet).
- **Planning records** (an appendix; identifiers allowed). See the inventory below.
- **Code hygiene inventory** (an appendix; identifiers allowed). See the inventory below.

## Voice, binding

- Plain English a sharp person from another company understands cold. Every project-private
  term is avoided or replaced by its plain meaning.
- Outcomes, not mechanics. "Agents can no longer be handed slide-deck work," not "pptx removed
  from the intent array."
- The decision is a real choice for the owner: the question in plain words, the answer you
  recommend, the owner's time. The recommendation states the risk ("the risk is low"), never
  erases it ("nothing can break"). Anything that is not the owner's call is not a decision.
- A number appears only where it changes a decision, and it carries its comparison. Every number
  traces to the fact sheet. No rounding up for effect.
- No praise, no process narration, no methodology, no review-round counts, no exclamation marks,
  no em-dashes. Present tense. Periods.
- Avoid filler words that signal automated writing: honest, genuinely, quietly, crucially,
  load-bearing, leverage, seamless, delve, game-changer, world-class, "not just." `brf check`
  flags them; the owner may add more in the config file (`extra_banned_words`).
- Say what is not done and what has not shipped. A brief that reads cleaner than the project is a
  defect.
- Do not repeat a fact across the blocks. Each fact lands once, where it does the most work.

## Length budget

| Block | Target |
|---|---|
| Main view, all of it, labels included | about 180 words |
| Attention reason | one line, 20 words or fewer |
| The big idea | one or two sentences, 40 words or fewer |
| Each unlock | a few words, plus a short clause naming who it is for |
| Each checklist item | 6 words or fewer |
| The decision | question, one-sentence recommendation, time |
| Activity read | one line |
| What changed | at most 8 items, one sentence each |
| Risks | 2 to 4 |
| Planning records | at most 8 locations; at most 8 drift items, the ones that could cause rework first |
| Code hygiene inventory | all ten categories, fixed order, each detail 40 words or fewer |

## Filling a template

- Copy `templates/project.html` into the work folder and fill it there. Never write inside the
  repository being briefed.
- Every `{{SLOT}}` must be filled or the build refuses. Slots are listed at the top of each
  template with a one-line description. `{{BRIEF_NUMBER}}` is the exception: `brf build` writes it.
- Repeat blocks sit between `<!-- repeat: NAME -->` and `<!-- /repeat -->`. Duplicate the block
  once per item, fill each copy, then delete both markers. The build refuses a file that still
  has them. The decision block may be kept zero times: delete it, markers included.
- Keep the `<meta name="brf-template" ...>` tag. The build reads it and stamps the brief.
- Badges: the class is the vocabulary word in kebab case, e.g. `badge--health-needs-attention`.
  Colour follows the word, never the project: green for Healthy and Fast; amber for Needs
  attention and Slow; red for Stuck, On fire and Stopped. The full mapping is in the template's
  header.
- The attention dial: `ATTENTION_FILLED` is the number; its dots fill.
- The meter: `MISSING_DONE` and `MISSING_TOTAL` are the counts of the checklist; each item's
  class is `check--done` or `check--open` and its state word is "In place" or "Missing", so colour
  never carries the meaning alone.
- The activity chart: exactly eight weeks, oldest first. Each week's `WEEK_LEVEL` is its count
  over the largest count, times 10, rounded (0 to 10); a week above zero that rounds to 0 gets 1.
  The chart is a table, so the numbers are there for anyone who cannot see the bars.
- Logo: paste an SVG mark into `{{LOGO_SVG}}` or leave it empty; empty collapses. Plain shapes
  and paths only: `brf check` refuses images, links, styles and scripts inside the SVG.
- Markup: add no tags, attributes or CSS beyond what the template uses. `brf check` accepts only
  brf's allowlist: the template's elements, links that need a tap (https, http, mailto, #),
  `style` attributes with plain values such as `width:40%`, radio buttons and their labels (for
  the theme switch), and CSS whose only `url()` targets are embedded data.
- Theme: the `--brf-*` custom properties at the top of the style element hold the palette for
  light and for dark. Two hidden radio buttons and one visible label switch between them without
  script (`:root:has(#brf-th-dark:checked)`). Leave them in place. An owner template may offer more
  themes the same way.
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
--meta` lists them.

- **Header:** logo, date, meta brief number, how many projects.
- **The portfolio this morning:** one paragraph, three or four sentences.
- **Start here:** the top three projects for today, each with why in one sentence.
- **Decisions waiting on you:** every open decision from every current brief, one line each, with
  the time it needs.
- **Every project, in order:** a card per project: rank, Stage and Health badges, the attention
  dial, the big idea in one line, what it unlocks, and the missing-to-functional count as a meter,
  with its brief's file name. Order by (1) what others wait on: a project that unblocks several
  comes before them; (2) the attention score; (3) the cost of doing nothing; (4) the reader's
  time, less first when all else is equal.
- **How the projects connect:** dependency chains as project, relation, project, with one short
  sentence each.
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
- The main view is about 180 words and carries no machine-assigned name.
- Stage, Health and Momentum use only the closed vocabulary, and the dial's reason states the trade.
- The big idea says what the project is for and why it matters.
- The checklist says what is missing for the project to work, and the meter matches it.
- There is at most one decision, it is a real choice for the owner, and its risk is stated.
- The activity bars come from git and the counts are right.
- Every number traces to a fact you verified. Anything unverified says so.
- It says what has not shipped.
- Every decision, missing item and risk from the prior brief is accounted for.
- "What it unlocks" or "Related projects" names other registered projects, or the big idea says
  plainly that it stands alone.
- Every planning location has a verdict, and the drift list names anything already built.
- The hygiene inventory covers all ten categories and says the repository was unchanged.
- `brf build --verify` and `brf check` are clean, and you looked at the phone and laptop
  screenshots. If the render check was skipped (exit 3), everything else here is still true
  and your reply says the page was not render-checked.

