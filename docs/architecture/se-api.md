# The Swiss Equestrian API contract

`https://my.swiss-equestrian.ch/api/veranstaltersoftware/v2` (test system: `my-stage.…`).
The one frozen contract of EquiEvents: it makes the same requests as Rosson 3.3.15, the
show-office software in use today, so Swiss Equestrian sees nothing new.

Evidence: Rosson's code (`vendor/rosson-3.3.15/src/renderer/app/services/upload.service.js`,
`data/xhr.factory.js`, `data/resources/*`) and every request Rosson makes for the corpus shows,
captured offline (`golden/corpus/<show>/api/`, `golden/src/api-fixtures.mjs`). Code:
`crates/se-api`. Real calls only from the start of the 2027 season (pilot).

## Keys

- `access_key`: the event key Swiss Equestrian gives the organiser. Unlocks the event's non-public data
  (addresses etc.). Only available from the pilot (stage 5); never in fixtures or the repo.
- `rosson_key`: date-based (`ddmmyyyy` → Base64 → ROT13 → 10 characters), the local Swiss date
  of the request. Part of the contract, not a secret. Checked against Rosson's own function
  (`corpus/rosson-key.json`).
- Uploads also send `rosson_version`; `Xhr.get` adds `app_version`: the latest Rosson version
  (decision 3).

## Endpoints

| Endpoint | Method | Rosson uses it for | Payload |
|---|---|---|---|
| `getVeranstaltung` | GET, polled every 4 s while 202 | loading the event; `geaendert_nach` for updates | `access_key` |
| `getPersonen`, `getPferde` | GET, polled | the registry: every rider (with contacts) and horse, for a change of rider or horse and late entries | `access_key`; built (`crates/office/src/registry.rs`; the simulation answers with the library's riders and horses) |
| `getOffizielle` | GET, polled | settings, dashboard | `access_key` |
| `getStammdaten` | GET | master data | `access_key` |
| `lookupZuweisung` | GET | Swiss Equestrian id of an official's assignment | `access_key`, `person_id`, `funktion_id` |
| `putStartlisten` | POST | start lists of a published class | see below |
| `deleteStartlisten/<published id>` | DELETE | withdraw them | query `access_key`, `rosson_key` |
| `putResultate` | POST | results of a class (also finals) | see below |
| `putZwischenresultate` | POST | part results (driving, eventing) | `kopf`, `pruefung`, `zeilen` |
| `putVeranstaltung` | POST | officials, entry and waiting-list counts | see below |
| `putGeolocation` | POST | distances from towns to the show ground | `veranstaltungsortplz`, `orte` |
| `getBackups`, `getBackup/<id>`, `putBackup` | GET, POST | Rosson's backups on the server | dropped (decision 4) |
| `updateProgramme/<id>`, `updateWertungen/<id>` | PATCH | master-data editing, developer mode | not built: tests and scoring tables are changed in a show only, never sent back (P. Burkhard, 2026-11-14) |

### What an upload sends (corpus fixtures)

**`putResultate`**, one class:

- `pruefungen[0]`: `ausschreibung_pruefung_id`, `nummer`, `bezeichnung`, `datum` (ISO, UTC
  midnight), `disziplin_id`, `kategorie_id`, `wertung_id`, `anzahl_gemeldete_reiter`,
  `anzahl_gestartete_reiter`, `anzahl_klassierte_reiter`, `anzahl_vollendete_reiter`,
  `offizielle` (`id`, `person_id`, `funktion_id`, `buchstabe`), `weitere_felder`.
- `resultate[]`, one per rider (and per team in team classes): `pruefungs_nummer`, `rang`
  (99 when out), `klassiert`, `aufgabegrund_id`, `reiter_id`, `reiter_name` ("Last First"),
  `reiter_ort`, `pferd_id`, `pferd_name`, `stufe`, `pferde` (the entry's horses as Rosson builds them from Swiss Equestrian's horse records when
  it loads the event: signalement, owner, co-owner, breeder, sire, dam, dam's sire, pony data,
  `einsaetze`),
  team fields; jumping: `punkte_<round>` / `zeit_<round>` as entered (text); dressage:
  `pro_richter` (`person_id`, `buchstabe`, `punkte`, `rang`, `prozent`), `punkte_total`,
  `punkte_prozent`, and in classes with chosen programmes `kategorie_id` / `kategorie_code`.
- `veranstaltungsbeteiligungen`: the event's officials, as Rosson stores them (including its own
  fields `readonly`, `updated_at`); `delete_veranstaltungsbeteiligungen`: Swiss Equestrian ids of removed ones.

**`putStartlisten`**: per section of the published class a printable list: `position`,
`veranstaltung_id`, `rosson_startliste_id`, `nummer`, `name`, `datum` ("25.09.2026"),
`uhrzeit` ("08h00"), `logo`, `hat_startnummern`, `wertung_id`, `wertung_text` (localised, e.g.
"A au chrono"), `programm_name`, `anzahl_starts`, `anzahl_hc`, `anzahl_ns`, `weitere_felder`
(sponsors etc.; for dressage also the judges), `hat_startzeiten`, `kategorie_text`, and `zeilen`:
starters (`reihenfolge`, `start_position`, `kein_start`, `hors_concours`, `startnummer`,
`startzeit`, rider with club and town, `pferde` with pony size, signalement, owner, breeder,
`weiteres`) and text lines (breaks).

**`putVeranstaltung`**: per published class `anzahl_nennungen`, `anzahl_warteliste`,
`kosten_warteliste`, `anzahl_veranstalter_plaetze`; per final class its officials; the event's
officials.

### The sequence of calls

After `putResultate` succeeds, Rosson sends `putVeranstaltung`, then one `lookupZuweisung` **per
official of the event** (91 requests for 2026.8352, after every class). After `putStartlisten`:
`putVeranstaltung`. EquiEvents looks up only assignments without Swiss Equestrian id (decision 5).

## How EquiEvents builds the uploads

`crates/se-api` builds each request from the show's state; `tests/results_contract.rs` compares
it, field by field, with Rosson's request for every class of the corpus (81 classes, 2,014 result
lines: identical).

- **The ranking** comes from `show::ranking` (decided rules, ranks set by hand, placed riders);
  it differs from Rosson's only in classes with a decided rule (the test lists them).
- **One form per field** where Rosson sends several, depending on how a value was entered (E34,
  decided 2026-11-13): as Rosson's timekeeper imports write it, entered values as text, empty
  fields left out, reasons as numbers, `aufgabegrund_id: 0` when the rider finished (jumping;
  dressage: no field).
- **Rosson's slips that reach the server are kept**: `kategorie_id` of riders in classes with
  chosen programmes is the whole category record; no per-judge rank with a single rider (E33).
- **Except wrong numbers**: the percentage per judge is exact; Rosson's floating point could
  round a value just below a half down (E35, decided 2026-11-13).
- Judges inside a rider's `pro_richter` are in score-sheet order, E-H-C-M-B (Rosson: the order
  their marks were first typed in).

- **Start lists** come from `show::startlist` (the class preparation: blocks, start times,
  breaks, team and level headings, the inserted line); words Rosson writes in its interface
  language ("Stufe", "Richter", "Pause", the programme title) are written in the office's
  language. Rosson's scrambled order in some team classes is not copied (E36).
- **Lookups** (`lookupZuweisung`) once per assignment (E38).

Status, each checked against every corpus request (`tests/*_contract.rs`):

| Request | Corpus | Result |
|---|---|---|
| `putResultate` | 81 classes, 2,014 lines | identical; ranks differ only by decided rules |
| `putStartlisten` | 78 published classes, 82 lists, 2,281 lines | identical; 3 team classes in our (correct) order |
| `putVeranstaltung` | 6 events, and after each of 159 uploads | identical |
| `deleteStartlisten` | 6 | identical |
| sequence of an upload | 159 uploads | Rosson's requests in Rosson's order; lookups only for new assignments |
| `getOffizielle`, `getStammdaten`, `getVeranstaltung` | 6 events | identical requests; the same show loaded from the same responses |
| update (`getVeranstaltung` with `geaendert_nach`) | 6 events, 99 changes | identical requests; the same show, except decided differences (E39, E40) |

## Loading an event

On a first load Rosson asks for the national list of officials (`getOffizielle`) and the
master data (`getStammdaten`), then the event (`getVeranstaltung`, polled while the server
answers 202). The query is `access_key` and `app_version`; an update adds `geaendert_nach`
(the `letzte_aenderung` of the last delivery). The response formats are not documented; they
are read from Rosson's code (`VeranstaltungService.get`, `dashboard.controller.js`):

- `getStammdaten`: `data.stammdaten` with `aufgabegruende`, `disziplinen`,
  `pruefungsfunktionen`, `pruefungskategorien`, `programme`, `veranstalter`,
  `veranstaltungsfunktionen`, `wertungen`, `etiketten_vorlagen`; stored as delivered.
- `getOffizielle`: `data.offizielle`, the national list (name, town, canton, contact, roles:
  each a title in German, French and Italian, `rollen[].rolle_bezeichnung_de|fr|it`; the same
  titles as SE's public search of officials, kept in `se_api::titles` with SE's order).
- `getVeranstaltung`: `data.veranstaltung` (event fields, `ausschreibung_pruefungen`,
  `nennungen`, `nennungen_fa`, `equipen`, `beteiligungen`), `data.personen` (riders and
  officials, with contact data), `data.pferde`, and `letzte_aenderung`.

What Rosson builds while loading, and EquiEvents builds the same way (`crates/se-api/src/load.rs`),
because some of it goes back with the uploads:

- entries from both lists; withdrawn, refused and wrong entries (`abgemeldet`,
  `zurueckgewiesen`, `falschnennung`) do not start; the rider's name, town and club, and the
  horses (`pferdeAnreichern`: public data, driving phases; a guest horse's name puts the guest
  horse in every place) are written on the entry;
- the event's officials (`beteiligungen`) become assignments if the person was delivered:
  Rosson's id form `<event>_<millis>_<random>` unless Swiss Equestrian returns one
  (`rosson_id`); `readonly` if not on the national list; days of work by `parseInt`, 0 = none;
- Rosson's placeholder horse 5 ("PSEUDOPFERD") is always known;
- master data is stamped with the time it arrived (`updated_at`): the category record goes back
  whole with dressage results.

Non-public data (addresses, phones, email, birth dates; entry remarks and accompanying persons)
goes into its own operation, so it can be kept off devices and out of exports that do not need
it.

Checked: past events cannot be fetched, so the responses are rebuilt from the corpus (what
Rosson stored, minus what it adds itself) and given to Rosson and to EquiEvents
(`golden/src/load-fixtures.mjs`, `tests/load_contract.rs`): 6 events, 2,183 entries, 1,090
people, 1,302 horses, 46 officials, the same show. Requests identical.

One Rosson inconsistency found on the way: changing an entry's horse at the show updates the
horses written on the entry but not `pferd_id` (E41). EquiEvents keeps one current horse.

### Updates

The dashboard's update button asks for the master data and the officials again, then for the
event with `geaendert_nach` (Rosson: `checkForUpdates`). What Rosson does with the answer, and
EquiEvents too (`read_update`):

- people and horses delivered replace the records; the event's officials are created or
  updated (an assignment keeps its id, found by Swiss Equestrian's id);
- an entry the show has is taken only if its `geaendert_am` is later than the last delivered
  one; its rider and horses are then written on it again from the current records, **on that
  entry only** (so copies on other entries of the same rider or horse stay as they were, and
  are sent like that);
- new entries and teams are added; nothing is removed.

Where EquiEvents does better (local behaviour): changes are merged field by field with the
office's (E39), published classes are replaced whole and new ones appear (E40).

Checked the same way as loading: each corpus event gets an update with one of each kind of
change (withdrawn, another horse, another rider, a guest horse, winning points, fee, accompanying
person, hors concours and level, confirmed again, a changed person and horse record, new entry,
new class, changed class, officials changed and new, teams renamed, new, changed), given to
Rosson after its load and to EquiEvents after its own (`update.json`): the same show, except
the decided differences, which the test checks one by one.

## Sending

`crates/se-api/src/http.rs` sends a request as Rosson's AngularJS layer does: query sorted by
name, encoded like `$httpParamSerializer` (space as `+`; `:` `@` `$` `,` `;` kept),
`Accept: application/json, text/plain, */*`, and `Content-Type: application/json;charset=utf-8`
with a body (checked against Rosson's: `golden/corpus/http-get.json`, `http-send.json`). JSON
object keys may come in another order than Rosson's; JSON gives them none.

Around the requests (E42): polled endpoints (`getVeranstaltung`, `getOffizielle`,
`getPersonen`, `getPferde`) are asked again every 4 s while the answer is 202, for at most 10
minutes; reads without answer (or a 502/503/504) are tried 3 more times, writes are not;
errors carry Swiss Equestrian's `message`. No I/O in the core: the app sends (desktop: an HTTP
client; browser: `fetch`).

`crates/se-api/src/actions.rs` runs the office's actions in Rosson's order (load or update;
results; start lists; withdrawing them; the event) and turns the answers into operations: the
upload itself (`Uploaded`: when, by whom), the ids of officials (`AssignmentLookedUp`), the
event delivered. A failed main request changes nothing and says why; a failed follow-up is
reported, the others are still sent. Tested against a fake Swiss Equestrian answering from the
corpus (`tests/actions.rs`).

### From equievents.ch only

Only the equievents.ch server talks to Swiss Equestrian (decided 2026-11-13,
[data-and-sync.md](data-and-sync.md)); the apps never do (the user, 2026-11-21). Built
2026-11-21 (`crates/server/src/se.rs`, [server.md](server.md)):

- **Downloads** (the user, 2026-11-21: real events may be downloaded now): the key checked
  before anyone owns a show, the event loaded from its key alone, its updates (Rosson's « mise
  à jour »: `getVeranstaltung` with `geaendert_nach`), the riders and horses (`getPersonen`,
  `getPferde`, with the key of one of the account's shows, kept per account), Swiss
  Equestrian's demo events (for the apps' training shows). Only GETs of those five endpoints
  with one access key go out (`se_api::http::download_key`, `se::ENDPOINTS`), to `SE_SERVER`
  (production by default, or stage).
- **Nothing is sent to Swiss Equestrian** (the user, 2026-11-21: "equievents.ch must NEVER
  send anything to SE"): the server has no code that uploads. A show of equievents.ch « sends »
  from there to the server's simulation (the app pushes its changes first and names its latest
  operation; the server builds Rosson's requests from what the show is there, the simulation
  answers, and the answer goes into the show's log). A show only on a computer, a rehearsal or
  a training show sends to the simulation on that computer. Sending for real is for a later
  version, when the user decides.
- **Swiss Equestrian's demo events** (Rosson's « Manifestation test », the keys
  `demoveranstaltung` and `demoveranstaltung_fa`: driving), for the app's settings › Exercice:
  the server downloads `getStammdaten`, `getOffizielle`, `getVeranstaltung` and hands them to
  the app, which makes a training show of them (never on the server); the answers are kept in
  `<library>/swiss-equestrian/` to look at (the national list of officials: on the computer
  only, never in the repository). First download, 2026-11-15: « Demo Veranstaltung Springen &
  Dressur » (2024.0001: 11 classes, jumping, dressage, a combined class in dressage and
  jumping; 362 entries) and « Demo FAHRTURNIER » (2024.0000, 112 entries), in about 3 s each.
- **The simulation** (`se_api::simulation`, on the server for its shows, in the office for the
  others) is where « Envoyer » goes; it answers as Swiss Equestrian would, as far as the contract tells: it refuses a request
  without access key, or a write (upload, withdrawal) without today's `rosson_key`; answers
  `{}` to uploads and an id to lookups; does not simulate loading. It keeps the last 200
  requests per show, the access key masked, shown in the app ("Reçu par la simulation"): what
  would be sent, field by field.
- **The access key** of an event is not a secret (P. Burkhard, 2026-11-13), but it is kept in
  one place: on equievents.ch for its shows (the key entered when the show was claimed, or a
  newer one Swiss Equestrian accepted); on a computer, the system's keychain in the desktop app (macOS Keychain, Windows Credential
  Manager; service `ch.equievents.office`, entry `access_key:<event id>`); in development and
  tests a file only its owner can read (`<library>/secrets.json`). Not in the show's log, so
  not in an `.equievents` export; in an export in Rosson's format, as in Rosson's own (Rosson
  needs it), and read from such a file when it is imported. Only its last three characters are
  shown.
- **What was sent, and since.** Each upload is an operation (`Uploaded`). The show notes when a
  class's results, and a published class's start lists, last changed; sent before that, the app
  shows them as changed since and offers to send again (the class's header, the Swiss
  Equestrian screen).
- Requests are sent with the show released: a slow answer does not hold the office.

Next: sending for real, in a later version (the user decides when; the pilot was planned for
April 2027).

## What the data model needs for this

The model (`crates/show`) holds what scoring needs. The uploads also send, and so the model must
carry, from what Swiss Equestrian delivers:

- ids where we hold codes: discipline (`disziplin_id`), category (`kategorie_id`), officials'
  role (`funktion_id` / `rolle_id`);
- class texts: `weitere_felder`, time (`uhrzeit`), section names, scoring-table text in the
  event's language;
- entries: `stufe`, the horses and the rider as written on the entry at loading (`pferde`,
  name, town, club);
- officials: Swiss Equestrian assignment id (`equis_id`), responsible flag, days.

Approach: keep what Swiss Equestrian delivers verbatim per record (entry, horse, person, class, official)
next to the typed model, and send it back unchanged where Rosson does; typed fields are what
the app works with.

## Decisions (P. Burkhard, 2026-11-13)

1. **Payloads exactly as Rosson's**, field for field: no change to the API is possible at this
   point. EquiEvents sends everything Rosson sends, including fields only Rosson uses.
2. **Interaction exactly as Rosson's**, including Rosson's internal fields on officials
   (`readonly`, `updated_at`, local `id`): nobody knows which the server reads.
3. **`rosson_version`: the latest Rosson version**, found as Rosson's start-up update check finds
   it: electron-updater reads `https://production-rosson.s3.amazonaws.com/latest.yml`
   (`latest-mac.yml` on macOS; configured in `Rosson.app/Contents/Resources/app-update.yml`).
   Never backwards; offline: the last seen; never read: 3.3.15 (`crates/se-api/src/version.rs`).
4. **Rosson's server backups are dropped** (`putBackup`, `getBackups`, `getBackup`): the master
   copy of every show is on equievents.ch.
5. **`lookupZuweisung` only for new assignments** (those without Swiss Equestrian id), not for every official
   after every class.

Everything else in this document is the contract EquiEvents follows.
