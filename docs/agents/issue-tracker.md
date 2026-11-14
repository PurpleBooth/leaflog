# Issue tracker: git-bug

Issues for this repo live in [git-bug](https://github.com/MichaelMure/git-bug)
(v0.11.0, installed via devbox). Bugs are git objects stored under `refs/bugs/*`,
separate from file history, and are shared between clones with
`git bug pull` / `git bug push` against `origin`. The tracker is shared across
clones (this machine, Tangled); `.scratch/` is no longer the issue store.

## Conventions

- One bug per ticket. Reference bugs by the 8-char shortId (`git bug bug show <id>`
  prints it); git-bug has no issue numbers.
- A feature spec is its own bug labelled `spec`; implementation tickets carry a
  triage label (see `triage-labels.md`) and a `Spec: <shortId>` reference in their
  body. The spec bug's comments hold the ticket index.
- Done work is the built-in closed status (`git bug bug status close <id>`), not a
  label. `wontfix` is the exception: label it `wontfix` and close it.
- Conversation history is comments on the bug, oldest first.

## Command notes for this build (old-style CLI)

Verbs come **before** the bug id. Three quirks to respect:

- `git bug bug new -t` does not persist the title — always create, then set it
  with `title edit` (`new -m` additionally refuses to run without a title).
- Every `-F` file body (on `new` and on `comment edit`) silently strips every
  line starting with `#` as if it were a commit-message comment — markdown
  headings are lost.
- `-m "<text>"` stores text verbatim, headings included. For bodies containing
  headings: create the bug, then replace its first comment with
  `git bug bug comment edit <comment-id> -m "$(cat body.md)" --non-interactive`
  (the comment id comes from `git bug bug comment <bug-id>`).

```
git bug bug new -F body.md --non-interactive          # create; note the shortId it prints
git bug bug title edit <id> -t "NN: Title" --non-interactive
git bug bug label new <id> "ready-for-agent"          # add one or more labels
git bug bug label rm <id> "needs-triage"              # remove one
git bug bug comment new <id> -m "..." --non-interactive
git bug bug comment edit <comment-id> -m "..." --non-interactive
git bug bug status close <id>                         # or: status open <id>
git bug bug show <id>                                 # full details
git bug bug                                           # list open bugs (excerpt column; use title/show for names)
```

The plain `git bug bug` list shows a message excerpt, not the title — run
`git bug bug title <id>` when you need the name.

## When a skill says "publish to the issue tracker"

Create a bug with the body file, set its title, apply the right triage label, and
report the shortId back in the reply.

## When a skill says "fetch the relevant ticket"

`git bug bug show <id>` — the user will normally pass the shortId directly.

## Sync hygiene

- Run `git bug pull` before starting work or claiming a ticket (same discipline as
  `git fetch` before claiming a D-number).
- Run `git bug push` after creating or updating tickets. Ordinary `git push` never
  carries `refs/bugs/*`; syncing the tracker is always an explicit step.

## Migration history

2026-11-14 (D072): the live `localisation` effort migrated from
`.scratch/localisation/` — spec `11052b1` plus tickets `b67691f` (01),
`390fbde` (02), `d32e7c5` (03), `394f2c8` (04), `a0d6639` (05), `cbfde49` (06),
`4e037a5` (07); original numbers are kept in titles and resolved in the comments.
The old directory is kept locally as `.scratch/localisation-migrated/`
(gitignored backup, not the live effort). The released `grow-page-correctness`
batch stays in `.scratch/` as history, and `.scratch/` remains gitignored.

## Wayfinding operations

Used by `/wayfinder`. Unchanged by D072: wayfinder keeps its local-map model in
gitignored `.scratch/<effort>/` working files — maps are not tracker issues and
do not sync. The **map** is a file with one **child** file per ticket.

- **Map**: `.scratch/<effort>/map.md` (the Notes / Decisions-so-far / Fog body).
- **Child ticket**: `.scratch/<effort>/issues/NN-<slug>.md`, numbered from `01`, with the question in the body. A `Type:` line records the ticket type (`research`/`prototype`/`grilling`/`task`); a `Status:` line records `claimed`/`resolved`.
- **Blocking**: a `Blocked by: NN, NN` line near the top. A ticket is unblocked when every file it lists is `resolved`.
- **Frontier**: scan `.scratch/<effort>/issues/` for files that are open, unblocked, and unclaimed; first by number wins.
- **Claim**: set `Status: claimed` and save before any work.
- **Resolve**: append the answer under an `## Answer` heading, set `Status: resolved`, then append a context pointer (gist + link) to the map's Decisions-so-far in `map.md`.
