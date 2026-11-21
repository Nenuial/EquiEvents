# Golden corpus and Rosson oracle

Real show data plus a way to ask Rosson (the show-office software in use today) what it computes
from it: the safety net for EquiEvents' scoring, and the evidence base for `docs/deviations.md`.

## What is here

**`corpus/` is not in the repository**, which is public: real shows, leak-checked but with riders'
and horses' names. It stays on this computer (ignored by git) and is rebuilt from the exports in
Dropbox with `npm run corpus` and the steps below. Without it, the tests that read it fail (the
golden tests, many of `crates/office`, `crates/timing`, `crates/se-api`, `crates/rosson-import`).

| Path | What |
|---|---|
| `corpus/<show>/snapshot.json` | One show as Rosson stored it, cut down to that show and stripped of non-public personal data. |
| `corpus/<show>/rankings.md` | The stored results as readable tables, one per Prüfung, to compare with published result lists. |
| `corpus/<show>/source.json` | Which export was used and why, the other candidates, and the leak-check result. |
| `corpus/<show>/oracle.json` | What Rosson 3.3.15 recomputes from the entered values, compared with what it stored. |
| `corpus/<show>/timing/` | Rosson's timekeeping files for every jumping class (start list as text and Excel, a results file as timekeepers send it, what Rosson reads from it) and its judges' tablet file for every dressage class (`<class>-tablet*`: export, a filled-in copy, what Rosson reads from it). Each file is imported into an empty class, so the fixture shows what Rosson read, not what was stored. (`src/timing-fixtures.mjs`) |
| `corpus/<show>/api/` | Every request Rosson makes to the Swiss Equestrian API for an action (uploading a class's results, the start lists, the event; withdrawing start lists), in order, captured with the network stubbed out (`src/api-fixtures.mjs`). `corpus/rosson-key.json`: Rosson's date-based key for a few dates. |
| `corpus/<show>/api/load.json` | Loading the event from Swiss Equestrian: the `getVeranstaltung` response rebuilt from the snapshot (past events cannot be fetched any more), the requests Rosson makes, and what it stores when it loads that response into an empty database (`src/load-fixtures.mjs`). |
| `corpus/SUMMARY.md` | Coverage per discipline and scoring table, and how much Rosson reproduces. |
| `src/` | The tools (Node, no build step). |

## Principles

- **Rosson is evidence, not truth.** A difference between the new engine and Rosson is resolved
  in `docs/deviations.md`, against the regulations, with a named person signing off.
- **Only public data goes into the corpus.** Everything published on info.swiss-equestrian.ch is
  public (riders, hometowns, clubs, horses, owners, breeders, results). Street addresses, emails,
  phone numbers, birth dates, organisers' remarks and the event access key are removed
  (`src/lib/anonymize.mjs`). A leak check verifies that no removed value appears anywhere in
  the output; the build fails otherwise.
- **Raw exports never enter the repository.** They stay in Dropbox (`Concours/`).
- **Nothing of them either** (the user, 2026-11-21: "avoid any data from the corpus ever
  reaching github"): `scripts/corpus-guard.mjs` reads the corpus and the exports on this
  computer and stops a commit or a push that holds any of their people, horses, contacts or
  keys, or a file that must never be committed (the hooks in `.githooks/`: `git config
  core.hooksPath .githooks`, once per clone). Tests take names from the show they load, never
  written out; a test's own data is invented.

## Running it

Requires Node 22+ and `Rosson.app` in the repository root.

```bash
cd golden
npm install
npm run vendor     # extract Rosson 3.3.15 from Rosson.app into ../vendor/ (git-ignored)
npm run corpus     # find exports in ~/Library/CloudStorage/Dropbox/Concours, build corpus/
npm run check      # let Rosson recompute every show; writes corpus/<show>/oracle.json
npm run summary    # write corpus/SUMMARY.md
npm run timing     # Rosson's timekeeping and tablet files, for crates/timing's contract tests
npm run api        # Rosson's requests to the Swiss Equestrian API, for crates/se-api's contract tests (no network)
npm run load       # Rosson loading each event from a rebuilt Swiss Equestrian response, for crates/se-api (no network)
```

`npm run corpus` picks, per show, the export with the latest change *inside the data* (not the
latest file name), because the last export of a show is sometimes missing. Shows from 2026 and
2025 are used; files older than Rosson 3.3.0 are skipped (format drift).

## How the oracle works

`src/oracle/rosson.mjs` loads Rosson's shipped bundle into jsdom with an in-memory IndexedDB and
stubs for Electron. `src/oracle/recompute.mjs` then replays what the results screen does: load the
show, start the same services, and for every Prüfung select it, re-save every result through the
UI's save path and recompute the ranking. All computed fields (ranks, totals, percentages,
times, faults, team rows) are removed first, so Rosson has to derive them from the entered values.

A difference in `oracle.json` means one of: a manual override at the show, a different Rosson
version at the show, inputs edited after ranking, or Rosson being inconsistent with itself.

## Results so far (2026-11-13)

6 shows, 2,038 results; Rosson 3.3.15 reproduces 1,886 (93 %). Dressage: 245/245. All open
differences are listed in `docs/deviations.md` (S1–S7).
