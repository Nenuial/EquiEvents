# EquiEvents

A proof of concept of modern event-management software for Swiss equestrian shows: entries,
start lists, scoring and rankings, prints, and the exchange with Swiss Equestrian. It runs on the
show office's laptops, offline when the show ground has no connection, and keeps every show on
equievents.ch, shared by the office's computers.

**Status:** a proof of concept, not in use at shows yet.

## What it is

- **equievents.ch**, the master copy of every show: accounts (anyone makes their own), the
  desktop app's downloads and updates, the sync between an office's computers, nightly backups.
  It is the only part that talks to Swiss Equestrian, and only downloads: an event with its key
  (the first account to enter it is the show's responsable), its updates, the riders and horses
  (per account). It never sends anything to Swiss Equestrian; « Envoyer » goes to a simulation
  that shows what would be sent.
- **The desktop app** (macOS, Windows): a show from its classes and entries to its rankings and
  prints. It works offline and syncs with equievents.ch when it can; it needs an equievents.ch
  account.
- **Scoring you can read and check:** one module per discipline and scoring table, each rule
  traced to the regulations, tested on hand-written cases and on real shows.
- **The outside world, as it is:** Swiss Equestrian's organiser API
  (`my.swiss-equestrian.ch/api/veranstaltersoftware/v2`), the timekeepers' files and the
  dressage judges' tablets keep their formats. Exports of Rosson, the show-office software in
  use today, can be imported, so a show can be taken over as it is.

## Architecture

- A **Rust core** (data model, operation log, scoring and rankings, start lists, timing files,
  PDFs with Typst), shared by the server and the app.
- **Every change is an operation** in a show's log; the state is computed from it. Devices
  exchange operations through equievents.ch and reach the same state by explicit rules
  (`docs/architecture/data-and-sync.md`, `reconciliation.md`).
- **One interface** (React, TypeScript, Tailwind) in a Tauri 2 desktop shell; every screen also
  works at phone width.
- **The server**: Rust (axum) and Postgres, one Docker image behind a reverse proxy
  (`docs/architecture/server.md`).

## Layout

| Path | What |
|---|---|
| `crates/scoring/` | The scoring engine: pure rules, one module per discipline, each starting with its rule in plain language. |
| `crates/scoring/tests/cases/` | Hand-written test cases as Markdown tables, readable and extendable without Rust (see the README there). |
| `crates/show/` | A show's data model, the operation log every change goes through, its SQLite file, show exports (`.equievents`). |
| `crates/se-api/` | Swiss Equestrian's organiser API: keys, endpoints, requests, loading an event, the simulation (`docs/architecture/se-api.md`). Sans-IO: the server sends. |
| `crates/timing/` | Files exchanged with the timekeepers (show jumping) and the judges' tablets (dressage). |
| `crates/rosson-import/` | Reads Rosson's export files into a show: as the event ended, or as Swiss Equestrian delivered it (to rehearse with a past event). |
| `crates/office/` | The office application for every front end: the show library, imports and exports, what each screen shows; one entry point (`Office::call`). |
| `crates/typeset/` | The PDFs: Typst templates filled with each print's data, tested on what the pages say. |
| `crates/server/` | equievents.ch: shows and their sync, accounts, Swiss Equestrian (downloads only), the app's downloads and updates, backups (`Dockerfile`, `docker-compose.yml` at the root). |
| `app/` | The app: the interface and the Tauri 2 desktop shell (`app/src-tauri`); see its README. |
| `golden/` | Tests from real shows (kept off this repository: they hold personal data) and a reference oracle (see its README). |
| `scripts/` | Releases of the desktop app (`release.mjs`), the image's release files, the corpus guard. |
| `docs/` | Architecture, decisions, the regulations checked, the office's wishes (`docs/ux/brief.md`), behaviours that differ from today's practice and why (`docs/deviations.md`), going live (`docs/deployment.md`). |

## Tests

```bash
cargo test --workspace                                         # everything (much of it needs the local golden corpus)
cargo test -p scoring --test cases -- --nocapture              # the hand-written cases, one line each
cargo test -p rosson-import --test golden -- --nocapture       # the real shows, with a report per class
cargo test -p timing -- --nocapture                            # timekeeping and tablet files against reference files
cargo test -p se-api -- --nocapture                            # requests to, and loading from, Swiss Equestrian against reference captures
cargo test -p rosson-import --test show_file -- --nocapture    # show exports: out and back in, the same show
cargo test -p server                                           # the server's API, against a local Postgres (a throwaway database)
```

The golden corpus (real shows) stays on the developer's computer and is never in this
repository: CI runs what needs no corpus (`.github/workflows/ci.yml`). The corpus guard keeps it
out of every commit and push: `git config core.hooksPath .githooks`, once per clone
(`scripts/corpus-guard.mjs`).

## Running it

The app needs an equievents.ch account; locally, a local server (Postgres 18):

```bash
createdb equievents_dev
# A local equievents.ch whose Swiss Equestrian is a stand-in (PRACTICE-KEY loads the practice event):
DATABASE_URL="host=localhost dbname=equievents_dev" cargo run -p server --example se_stand_in
# The app in a browser, against it (app/README.md):
EQUIEVENTS_SERVER=http://localhost:3999 cargo run -p office --bin office-dev-server -- /tmp/equievents-library
cd app && npm install && npm run dev    # http://localhost:5173
```

A server without Swiss Equestrian (keys not checked): `SE_SERVER=none cargo run -p server --
web`, with `DATABASE_URL` set. Deployment, the app's releases and going live, in order:
`docs/architecture/server.md`, `docs/architecture/releases.md`, `docs/deployment.md`.

If the project lives in a synced folder (Dropbox), keep the folders of generated files out of
its sync: `xattr -w 'com.apple.fileprovider.ignore#P' 1 <folder>` for `target/`,
`app/node_modules`, `golden/node_modules` and `vendor/`.
