# Step 2 — data model, storage and sync

Status: proposal, 2026-11-13; updated with Swiss Equestrian's answers the same day. Defaults are marked *(default)* where an answer from Swiss
Equestrian is still needed (questions at the end).

## Goals

- A show runs **without internet**: every device keeps working on its own data.
- **Several devices** at a show (show office, a second office laptop, a phone) see the same data
  through equievents.ch, and catch up when they reconnect.
- **Nothing is lost and everything is traceable**: who entered or changed which result, when, on
  which device. Official results need that for protests.
- **Same core in the browser and on the desktop** (Tauri on macOS and Windows).
- The Swiss Equestrian API exchanges stay exactly as they are; everything else is designed properly
  (docs/deviations.md E1–E19).

## Data model

Normalised, one fact in one place. Rosson copied the whole entry — with address and phone —
into every result, team and log record (E2); here results reference the entry.

| Entity | Key | Notes |
|---|---|---|
| Person | Swiss Equestrian person id | Public part (name, town, canton, nationality, club, licences, youth status) separate from the non-public part (address, email, phones), which only exists when the event key was used and is never synced to devices that do not need it. |
| Horse | Swiss Equestrian horse id | Name, signalement, owner, breeder, pedigree (all public). |
| Event | Swiss Equestrian event id | Number, title lines, place, dates, organiser. The access key is not a secret (P. Burkhard, 2026-11-13); it is kept in one place (the computer's keychain; later the server), not in the show's log, and goes into Rosson-format exports as in Rosson's. |
| Published class | Swiss Equestrian class id (Ausschreibung-Prüfung) | As published: discipline, category, scoring table or programme, team size, entry fee. Read-only, comes from the API. |
| Class | local id | What is actually run: a published class can be split into sections (10a, 10b, 10c) or merged. Date, time, options (medal jump-offs, placed count set by hand, judges). Status: planned → running → final. |
| Entry | Swiss Equestrian entry id | Rider, horse, published class, status (entered / withdrawn / not starting), hors concours, team, individual programme. |
| Team | Swiss Equestrian team id or local id | Name, class, riders (via their entries). |
| Start | local id | Entry in a class, start order, start number. |
| Round result | (class, entry, round) | What was entered: faults, time, points; or an elimination with its reason. Inputs only — ranks and totals are computed, never stored as truth (E3). |
| Judge sheet | (class, entry, judge) | Dressage marks and ticked penalties of one judge. |
| Official assignment | local id | Person, role, class or whole event, judge letter. |
| Master data | Swiss Equestrian ids | Scoring tables, dressage programmes, disciplines, categories, reasons. Versioned: a class keeps the version it was run with. |

Ids are typed and of one kind each (E1). Numbers are exact decimals (E5).

## Operation log

Every change is an **operation**, appended to a log and never edited:

```
op id (ULID) · device · author · timestamp (hybrid logical clock) · kind · payload
```

Examples of kinds: `EventImported`, `EntryWithdrawn`, `StartOrderSet`, `RoundResultEntered`,
`JudgeSheetEntered`, `ClassFinalised`, `ClassReopened`, `ResultsUploaded`.

- **The current state is computed from the log** into SQLite tables (a projection). The
  projection can always be rebuilt; the log is the truth.
- **Audit trail for free:** the history of a result is the list of its operations.
- **Sync is exchanging operations:** a device sends the operations the other side has not seen
  and receives the ones it is missing. Order is the hybrid logical clock, so all devices end up
  with the same state regardless of the order in which operations arrive.
- **Conflicts** are settled by explicit, deterministic rules — field-by-field edits, the later
  change wins *and* is listed for the office, late changes never alter a final class, clocks are
  checked: [reconciliation.md](reconciliation.md).
- **Finalised classes are frozen:** after `ClassFinalised`, results can only change after an
  explicit `ClassReopened` (with author and reason), which is what an official correction is.

## Who syncs with whom

Decided by P. Burkhard, 2026-11-13 (replacing the first proposal, where the show-office laptop
was a hub on the show's network and called Swiss Equestrian itself):

```
   show-office laptop ─┐                                   ┌─ Swiss Equestrian API
   second laptop ──────┼── internet ── www.equievents.ch ──┤   (my.swiss-equestrian.ch)
   phone (web app) ────┘               master of every show └─ daily backups (hosting)
```

- **equievents.ch holds the master** of every show: its full operation log.
- **Only the equievents.ch server talks to Swiss Equestrian**: loading an event, start lists,
  results, official lookups, with the same requests as Rosson (the date-based `rosson_key`).
  The event's access key is kept there (not a secret, but one place for it).
- **The apps talk only to equievents.ch.** A laptop **works offline** (start lists, results,
  prints): its changes stay in its show file and go up when the connection is back. Several
  computers at a show share data **only through equievents.ch** (no local-network hub): while
  the internet is down each keeps its own changes, and they meet again once online, by the
  reconciliation rules ([reconciliation.md](reconciliation.md)).
- **Sending to Swiss Equestrian** (*Envoyer*, built 2026-11-21, to the server's simulation: this
  first version never sends to Swiss Equestrian): the app first pushes its changes, then asks the
  server to send, naming the last of its operations the send must include; the server sends
  only once it has them, so it never sends results older than what the office sees. Swiss
  Equestrian's answer comes back as an operation (`Uploaded`): "sent" means accepted.
- **Backups**: the server is backed up daily. Nothing is backed up to Swiss Equestrian
  (Rosson's `putBackup` is not built). A show office can keep its own copy by exporting a show
  (below).

## Which shows a computer keeps (2026-11-15: archives built, the rest proposed)

Asked by the user: a computer should hold only some of the account's shows, others stay online
only (reached with an internet connection), and finished shows get archived.

Today: the library lists the shows on this computer; signed in, a section lists the account's
shows not on it (*Télécharger*); *Supprimer…* moves the local copy to the library's `deleted`
folder and leaves the show on equievents.ch as it is.

Proposed:

- **One list, every show of the account**, each marked *Sur cet ordinateur* (kept: works
  offline, syncs) or *En ligne* (on equievents.ch only). The library's subtitle says both
  (« 5 concours · 2 sur cet ordinateur »); signed out, only « sur cet ordinateur »; in the web
  app on equievents.ch, « sur votre compte ».
- **Garder sur cet ordinateur / Retirer de cet ordinateur**, like a file kept offline in
  Dropbox. Retirer waits until every change of this copy is on equievents.ch; without a
  connection it says so and keeps the copy (or, asked, puts it in `deleted` with its changes).
- **Opening an online-only show**: it needs internet. The app fetches the show's log into a
  temporary copy (the same core and screens: a 450-entry show is 0.7 MB) and sends each change
  at once. If the connection drops, it says so and offers to keep the show on this computer,
  which then works offline like any kept show. Closing it drops the temporary copy.
- **Archives** (decided by the user, 2026-11-15; built): archived is only how shows are listed.
  The show's responsable archives it, or takes it out, on equievents.ch (its ⋯ menu in the
  library); every member's library lists it under *Archives* (by year) after its next sync,
  offline too (the show file keeps the state and one's role). It works as before: it opens,
  changes, syncs and sends. A show only on a computer is archived there. The responsable's
  account may archive the shows it owns on its own, some weeks after their last day (1–104;
  off: by hand only); the server takes a show's last day from its operations (the event's,
  or a later day a class moved to) and checks whenever a library lists its shows. A show taken
  out of the archives by hand is not archived on its own again.

## Storage

- Server (equievents.ch): Postgres, every show's log (`crates/server`,
  [server.md](server.md)).
- Desktop: SQLite file per show, through the Rust core; it also knows which operations
  equievents.ch has and its link to the show there (the apps' sync: [server.md](server.md)).
- Browser: the same schema in SQLite compiled to WebAssembly, stored in the browser's private
  file system (OPFS); the Rust core runs compiled to WebAssembly in a worker.
- One schema, one set of migrations, both written once in the Rust core.

## Show exports

One show per file (`.equievents`, `crates/show/src/export.rs`): a zip with `manifest.json`
(format and version, the event, when and by which app, number of operations, whether private
data is included) and `operations.jsonl` (the log, one operation per line). The log is the
show, so an export carries its whole history; importing adds the operations a show file does
not have yet (by id), so the same file also brings a copy up to date. An export can leave out
the private data (contacts, entry notes) for handing a show to someone who does not need it.
Files from a newer app, incomplete files and files of another event are refused with a clear
message. There is no export of a whole database (Rosson's), and none is needed: the master copy
of every show is on equievents.ch.

A show can also be exported **in Rosson's format** (one event, as Rosson exports it): a show
office's local copy that opens in Rosson 3.3.15 as well, a fallback while EquiEvents is new; it
carries the show as it is, not its history. EquiEvents reads it back like any Rosson export.

Checked: every corpus show, imported from Rosson, exported, imported into a new show file and
exported again gives the same state and the same bytes (`rosson-import/tests/show_file.rs`).

## Taking over from Rosson

`rosson-import` reads both kinds of Rosson export (one event, or the whole database: pick the
event) from Rosson 3.3.0 on, and turns an event into operations in two ways:

- **as it ended** (`ops`): classes, start lists, officials, every result; to continue a show or
  keep its history. The golden corpus is the test: every show imports and scores to the same
  2,013 results.
- **as Swiss Equestrian delivered it** (`delivered`): the responses Rosson loaded the event from,
  rebuilt from what it stored, for `se_api::load`; to rehearse a show with a past event through
  the same code as a real one (past events cannot be fetched from Swiss Equestrian). Kept for
  the tests and the practice event; the app imports a Rosson export as it ended only (the user,
  2026-11-15: a real event is the real show, with all its data; a file with several events
  imports those ticked).

## Training shows (2026-11-15)

Made from the app's settings (*Exercice*), as Rosson's « Manifestation test »: the **practice
event** (« Concours d'exercice EquiEvents », three days from the day it is made: jumping,
dressage and driving classes and entries of corpus shows, every person, horse, owner, breeder,
club, team, organiser and place invented; `crates/office/training/practice.json`, made and
leak-checked by `examples/practice_event.rs`), and **Swiss Equestrian's demo events**,
downloaded ([se-api.md](se-api.md)). Both load through `se_api::load` like a real event. A
training show is known by its first operations' author (`training: …`); it is never synced: no
event key, `put_online` refuses it, and it sends only to the simulation (with a placeholder key)
whatever server is set, so the office can practise sending.

Every Rosson export from 2025 and 2026 in the organiser's archive (27 files, 29 events) imports
both ways (ignored tests that read the archive locally; it holds personal data and stays out of
the repository).

## Answers from Swiss Equestrian (2026-11-13)

- **Data entry:** mostly the show office. Show-jumping times come from the timekeepers through an
  Excel file (export and import in the results panel); the office can also enter or correct.
  Dressage: entered by the office; tablet entry by judges exists but is rarely used.
  → The Excel exchange with timekeepers is an external interface like the API: its format is kept.
- **Hub:** first a local hub in the show office, cloud when available; since 2026-11-13: no
  local hub, the apps work offline and sync only with equievents.ch (above).
- **Accounts:** personal accounts. Show offices register on the website and download the apps
  there; the app does nothing without an equievents.ch account (the user, 2026-11-21), and an
  event's show needs its key.
- **Live results:** not this app's job. Results are uploaded to Swiss Equestrian when a class is finished
  (as Rosson does) and published on info.swiss-equestrian.ch.
- **Cloud:** the web app is hosted at **www.equievents.ch**, which also keeps the **master copy**
  of every show (its full operation log) and alone talks to Swiss Equestrian. A laptop keeps a
  show running without internet and syncs with equievents.ch whenever it can. The master copy
  follows the same reconciliation rules as every other device.
