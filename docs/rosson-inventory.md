# What Rosson does, and what EquiEvents still has to

The backlog for "build everything else from Rosson" (P. Burkhard, 2026-11-13): every feature
of Rosson 3.3.15 except loading an event from Swiss Equestrian. Made from Rosson's code
(`vendor/rosson-3.3.15/src/renderer/app/…`, main process `src/main/…`); file references are
from there. Each item is rebuilt the EquiEvents way (docs/ux/brief.md), not copied.

Size: S small, M medium, L large. **Done** items are listed in the app's README.

Rosson has none of: stable/box lists, invoicing or payment tracking (fees are only shown and
summed), creating persons or horses (only Swiss Equestrian's registry, or a guest horse), undo,
statistics on the dashboard, keyboard shortcuts beyond Enter-as-Tab.

## Participants (`teilnehmer/`)
- **Search** across licence, names, town, horse id and name (all words must match). Done.
- **A rider's address** (street, postcode, town) edited; written to all their entries, kept when
  the registry is downloaded again (`personenliste.service.js:126-170`). **Not built, on purpose**
  (P. Burkhard, 2026-11-14: the show office does not touch riders' and horses' data): *Adresse de
  …* in an entry's tools shows Swiss Equestrian's address and contacts, read only. (Built
  2026-11-13 and taken out; an address set then, in a show's log, still counts.)
- **A rider's starts**: entries not yet on a start list, by published class; starts by class
  with date, time, number/position, hc, NS; swap their two start positions in a class (`:186`). M
- **Change of rider** (`components/reiterwechsel/*`): a person from the registry (≥3
  characters), the entries to transfer (classes not final), all person fields copied, the entry
  flagged `reiterwechsel` (printed "RW"); guest horses only for riders with a guest licence
  (`/gast|RBE/`, `nennungen.service.js:595`). M — **done** (2026-11-14: riders of the show or
  of the registry, never typed in; the guest-licence rule; horses likewise, those of the registry
  only when their fee is paid for the show's year).
- **Move an entry to another published class** (`in-ausschreibung-verschieben/*:34-56`): its
  starts and results deleted, its dressage test cleared unless the target is à-la-carte; not
  for final or team classes. M — **done**.
- **Distances** from riders' towns (server geocoding, `putGeolocation`): upload, later. S
- **Participants as Excel**: licence, name, address, email, phones, salutation, language
  (`export.service.js:49`). S — **done** (2026-11-13)

## Class preparation (`pruefungsvorbereitung/`)
- **Fixed start numbers for the whole show**, and giving them out: sorted by name, horse,
  distance, GWP, start order (by class, then section) or random; ascending/descending; grouped
  by published class (`fixe-startnummern-vergeben/*`). Turning it off clears the numbers. S — **done** (2026-11-13; by winning points too; by
  distance waits for the server: Rosson asks Swiss Equestrian to geocode the riders' towns).
- **All entries as Excel** (~45 columns: times, positions per phase, pedigree, GWP, level,
  team, hc/NS, status, fee, remarks, comment; `export.service.js:192-395`). M — **done** (2026-11-13)
- **Published class**: category text, number of levels (Stufen; fewer clears levels above),
  remarks, fees 1/2, maximum entries, pony-only. S — **done** (2026-11-14, in the class's
  *Détails…*: what Swiss Equestrian published and the entries by status; the category as printed
  and the levels changed; `PublishedClassEdited`).
- **Sections** (`startliste-teilen/*`, Rosson: pick starters by hand from a table): **done**
  better (2026-11-13, U13): rules (balanced, club, town, comment, level, pony sizes), by hand
  or by search, a preview, divided again or made whole; a rider's horses in one section, except
  by pony size or level (ranked separately: each horse to its own).
- **Riders with several horses** (U11, U12): **done** (2026-11-13, better than Rosson, which
  put a rider's first horse early and all others at the end): horses at the start, at even
  steps and at the end (B1 A B2; C1 B1 A C2 A B2 C3; any number), in the draw and the sorted
  orders, level by level; marked n/k on the start list, red when closer than half the even
  spacing; a rider's horses swapped among their places, numbers following the places if asked.
- **Start list rules** (checked 2026-11-13): maximum starters **done** (*Détails…*: the
  entries made last wait on no start list and move up from there); groups by level, and in a
  pony class by pony size (A, B, C, D), **done**; a rider's horses spread out (U11, beyond
  Rosson); the dressage test's duration shown where the time between starts is set (offices set
  more than the test, e.g. 8 min for a 4'00 test). Earlier notes: (`services/nennungen.service.js:331-369`,
  `pruefungen.service.js:244-396`): 1–6 sections a–f; dressage by blocks (default 10) or by
  times (programme duration); "Nennungen nachrücken"; maximum starters (cut in order, kept as
  `max_startende`); jumping ground care ("Platzpflege") after N with a text; at most 2 entries
  per rider grouped; riders with 2 horses: one early, the other at the end, the lower level or
  pony size first; groups by level and pony size.
- **Start list of a follow-up class** (`folgepruefung-erstellen/*`): the sections of another
  class cloned, the order following it (rider and first horse matched; others at the end). M —
  **done** (2026-11-13: *Suite de…* on a class not set up yet).
- **Late entries / organiser's places** (`startplatz-vergeben/*`): a rider from the registry,
  a horse from the registry whose paid sport years include the show's year, or a guest horse
  (guest licence); in team classes join a team with a free place or make a new team; new entry
  `definitiv`, `ist_vergebener_startplatz`. M — **done** (2026-11-13; with the registry 2026-11-14:
  riders and horses of the show or of the registry, never typed in, the paid-year and
  guest-licence rules; team classes: 2026-11-13, a team with a free place or a new one at the end
  of the rotation).
- **Prize money** (`preisgeld/*`): up to 30 ranks; minimum (fee 1 − 10) × 2; computed from the
  first prize, each rounded up to 5 CHF, × 0.8 for the next, never below the minimum; a natural
  prize per rank. S — **done** (2026-11-13; in decimals; the scale in show jumping only, and never
  printed on the ranking, only on the prize labels, 2026-11-19).
  Prize labels: with the labels.
- **Delete a class's start lists locally** (classes, starts, results, officials, finals;
  numbers unless fixed). S — **done** (2026-11-13: *Supprimer les listes*, asked twice;
  refused while Swiss Equestrian has them, or another class uses it as qualifier or in a
  combined ranking).
- **Class details**: unique number, date, time, logo (jpg/png ≤ 400 px), sponsor text for
  labels, extra key/value lines printed on lists, counts. S — **done** (2026-11-13: *Détails…* on the class: number,
  day of the show, placed count, sponsor on the prize labels, extra lines, logo made a PNG of at
  most 400 px; the logo printed at the top of the class's lists).
- **Dressage**: blocks or times, block size, start time, duration, **breaks**, totals per
  judge, team dressage (5 judges, 3/2), custom judge letters, the test (filter by discipline). S
  — **done** 2026-11-14 (team dressage too, S23: a sheet per team and judge, 3/2 in *Détails…*,
  OKV from the test): the test's duration (its longest,
  rounded up) the start interval by default and one click away; the tests offered are the
  class's discipline's; judges' totals in the ranking. Custom letters are driving's (C R S P V)
  and team dressage's (H C M E B): with them.
- **Layouts by discipline, as Rosson** (2026-11-19): blocks in dressage only (its classes and
  the dressage phases); start times in dressage and the phases of eventing and driving
  (jumping, cross, marathon, cones, derby, open; Rosson offers jumping's only to a phase); a
  plain list everywhere, with « Entretien après le n° » (Rosson's « Entretien de la place
  après ») except with start times (their breaks are by the clock). A jumping class: the plain
  list and the ground care.
- **Jumping**: with or without start times, ground care, duration, scoring table and text,
  **ideal time** — **done** 2026-11-14 (S20: the ideal-time table, the office enters the ideal
  time in *Détails…*; the scoring table chosen in *Détails…* before results, 2026-11-14). The **maximum time** is the timekeeper's elimination, not the office's; Rosson's
  0.4 per second started over it is for combined classes' jumping and cross phases (eventing):
  see combined classes. S
- **Start times** (`pruefungen.service.js:799-835`): each start = previous + duration; a slot
  overlapping a break moves to its end; with "nachrücken" NS get `--:--` and no slot; blocks
  every N starters, each with an editable time label; ground care row once after N; a break row
  where a start equals a break's end. M — **done** (2026-11-13, the core had it; the app sets it
  up: *Mise en page…*; a dressage test's duration as the interval, 2026-11-14).
- **Start order**: by another class; rotated to start at a chosen entry; by a phase's ranking
  (worst first). Team start numbers: position + 2-digit team order (rider 1 of team 3 = 103). S —
  **done** (2026-11-13: *Ordre…*: by rider, horse or winning points, up or down; as another
  class; after another class's ranking, the worst first, without a result at the end; from a
  chosen starter. Names compared without accents. Not for team classes; distance not kept).
  Team start numbers **done** (2026-11-14: the rider's place in the team then the team's in two
  digits, 103; the teams numbered by their place; dressage team classes: the teams only).
- **Start list as Excel.** S — **done** (2026-11-13)
- **An entry**: level, carriage width (driving), companion, remarks and office comment, **start
  position by hand** (others shift; copy to all phases?; **done** 2026-11-13: *Place* on the
  selected starter), delete (office's places only), the box
  of entries on no start list (put them in a section; number = highest + 1) — **done**
  (2026-11-13: above the start list, and +N on the class; Rosson's rule: not those who do not
  start nor those turned down). S
- **Teams**: name, start position, on/off the start list as a whole, team dressage test. M —
  name, start position, on/off as a whole: **done** (2026-11-13, ✎ ↑ ↓ NP on the team's
  heading; the riders' rotation follows). Team dressage: **done** 2026-11-14 (S23); a team's
  own test (Rosson's `programm_id` on a team) is not built: the class's test.
- **Phases' order** (combined classes) by drag and drop. S
- **Ordering by drag and drop** everywhere Rosson typed order numbers (P. Burkhard, 2026-11-14):
  **done** for the timetable's classes, a start list's starters, the teams' order, a rider's
  horses (a grip per row: mouse, finger, keyboard); the phases' order with combined classes.

## Finals (`finalrangliste/`) — **done** 2026-11-14
- Rankings over several classes on Rosson's rules: ranks added (ties class by class) or, for
  dressage, the mean percentage; jumping and dressage mixed (ranks); missing a class last, out
  in one after the ranked; 30 % placed; numbered as Rosson (6.1). **Sent to Swiss Equestrian** as
  Rosson sends them (`putResultate`, one class numbered as the final; mixed disciplines as
  discipline 4, category 462, with the ranks per class). **Team finals** (2026-11-14): over team
  classes the teams are ranked, matched by name, ranks or penalties added; sent as a row per
  team then a row per rider. Better than Rosson: two teams of one name in a class are told
  apart ("… (2)"); Rosson merged them and left both unranked. No corpus show has two team
  classes: tested on one class's teams.

## Running a class (`pruefungsdurchfuehrung/`)
- Start-number quick jump (type a number, Enter). S — **done** (2026-11-13: *N° …* on the
  results screens, ⌘J; without start numbers, the place in the start order).
- ~~**Split a ranking into two classes** (jumping, > 69 entries, by rank, every second one to
  class `_2`)~~ — **not built**: no longer allowed; a class with more than 90 entries is divided
  into sections before it runs (docs/deviations.md S19; flagged with *Diviser en N parties*).
- Ranking as Excel; refresh; delete all results (not when final). S — **done** (Excel and
  refresh: always live; *Effacer les résultats* 2026-11-13: results, sheets, ranks by hand).

## Officials (`offizielle/`)
- Roles at the event level (not per class), days worked, "responsible" flag: **done**
  (2026-11-13). **Missing roles** (required vs given): Swiss Equestrian delivers no required
  roles (`erforderliche_rollen` empty in every corpus show): nothing to compare. One person to
  several classes at once; per person view; their plan (Einsatzplan) printed. S–M — **done**
  (2026-11-13: Shift+←→ or Shift+click along a row, *Toute la journée* in the roles at the cell, one
  change for all; per person: the rows of each day, and the plan printed).

## Overview (`uebersicht/`)
- All classes and phases: position, date, time, entries (total/NS/hc), sent when, final or
  open; **position editable** (orders the timetable and riders' letters). S
- Closing the event (upload of counts and fees): with uploads.

## Combined classes (Zwischenprüfungen) and driving — L overall — eventing **done** 2026-11-14 (S24); driving **done** 2026-11-14 (S25): scored, entered (multitest too), horses by phase, team cones (OKV Fahrcup), time limits, sent, kept in the Rosson file, printed (start lists, day lists, judges' sheets, control sheets, result lists)
- Phases per class (eventing: dressage, jumping, cross; driving: dressage, marathon, cones,
  derby, open, multitest), each with its own date, time, list, order and results.
- Overall ranking by penalties (dressage 100 − %, driving dressage as penalties; cross: points
  + the gap to the ideal time (`idealzeit_berechnen`), jumping and cross phases: 0.4 per second
  started over the maximum time (`max_zeit_berechnen`) — keep both working), by rank points,
  or points (multitest); ties by phases in
  order, then deviation; retired in any phase at the bottom (`rangliste.service.js:751-957`). M
- Marathon, cones (scorings A, B, JC; times by licence category), derby, open, multitest: rules
  in `pruefungsdurchfuehrung.service.js:452-646, 1499-1554, 2162-2177`. M–L each.
- Driving entries: carriage type and width, several horses with their phases, F licences. M

## Labels (`etiketten/`) — M — **done** (2026-11-13: the two Zweckform sheets, skip N; custom sheets 2026-11-14: described by the office — across, down, label size, margins, gaps, padding, text size and alignment — with a live A4 preview, kept with the library)
- Sheets Zweckform 3475 (70×36) and 3484 (105×37), or custom; skip N used labels.
- Riders' addresses; judge-sheet labels (one per judge and starter); prize labels per rank
  (sponsor, class, rank, CHF or natural prize).

## Working on two computers (`collaboration/`) — M
- Rosson: a master file of the whole show, a client enters results, sends one class back as a
  file. In EquiEvents: sync of the operation log (docs/architecture/data-and-sync.md).

## Backups and history
- Automatic backups (Rosson: on Swiss Equestrian's server, `putBackup`); history of changes by
  kind, searchable. EquiEvents keeps every change in the show's log: **history screen done**
  (2026-11-13). Backups: **not built** — no backups to Swiss Equestrian; the master of every
  show is on equievents.ch, whose server is backed up daily (P. Burkhard, 2026-11-13).

## Exports of a show
- **In Rosson's format** (`<nr>_export_<date>.txt`, one event): a show office's local copy
  that also opens in Rosson 3.3.15. **Done** (2026-11-13, *Rosson* in the library;
  `rosson_import::to_rosson`): Swiss Equestrian's records as the show kept them, with the show's
  classes, start lists and numbers, layouts, results, judges' sheets and ticked penalties,
  ranks set by hand, officials, teams, prizes, class details; contacts and remarks included.
  Read back into EquiEvents it is the same show (all corpus shows, and a show run in
  EquiEvents: `rosson-import/tests/to_rosson.rs`, `office/tests/rosson_export.rs`). With the
  event's access key, as Rosson's (not a secret; read back on import). Rankings over several
  classes (Rosson's Finalprüfungen) **read and written** 2026-11-14 (Rosson's records back as
  they were). Eventing and driving: the phases and their results, and the overall rows Rosson
  builds only when a result is saved (2026-11-14; as Rosson stored them at a real driving show,
  2025.8126). Not written: sheets put aside, the history (Rosson has no store for either). Not
  yet opened in Rosson 3.3.15 itself: to be tried by hand.
- In EquiEvents' format (`.equievents`): the show with its whole history. **Done.**

## Settings — **done** 2026-11-14, Italian too (the app, prints and exports in French, German or Italian; Swiss Equestrian's own Italian for roles and dressage tests, German where it has none; ⚙ in the library: the zoom, 50–150 %, also ⌘+ ⌘− ⌘0; developer mode: a show's scoring tables' definitions changed, checked as the app reads them, undone with ⌘Z, never sent back to Swiss Equestrian as Rosson's developer mode could (P. Burkhard, 2026-11-14); administrators of equievents.ch only, also acting as an account, the dressage tests' no longer (Rosson's early fix for wrong coefficients; the user, 2026-11-22); the error log, also in `<library>/errors.log`, keys masked)
- Zoom (0.5–1.5), language (de, fr, it), registry download (API: **done** 2026-11-14, from the
  pickers, through the show's connection; the simulation answers with the library's riders and
  horses), test events (API), database
  export/import/clear (EquiEvents: one show per file instead; an event's export in Rosson's
  format **done** 2026-11-13, see below), developer mode (the
  scoring-table editor, error log). Elsewhere: fixed numbers, table columns, letter text and
  address position, label templates.

## Prints (`services/print/*`, `print-selection/*`)
Logos in the header or the footer, or none. Done in EquiEvents (2026-11-13): start list (with
pedigree as an option), ranking, judges' sheets, ranking over several classes, participants,
timetable. To do:
- dressage judges' sheets (`richterblatt*.print.service.js`) — **done again** 2026-11-14 as
  Swiss Equestrian's official form, the way Rosson prints it: the federation's head and logo
  (FEI's for FEI tests), the test's title, arena, time, bridle, edition; start number, rider
  (licence), horse (passport), class, judge with town and position, date and place; the
  test's tables in its three layouts (movements: letters, exercises, directive ideas; criteria;
  penalty points) with mark, correction, coefficient, remarks and the shaded subtotals; texts
  before and after; kür and children's A/B marks; total, percent of the maximum, signature; a
  sheet's two pages linked by "Report à la page 2" / "Report de la page 1"; footer with the
  test and "© year Swiss Equestrian". Rosson's widths, rules and colours; Gilroy (its font,
  licensed) when installed, else Helvetica. **One document** for all judges (Rosson: a file per
  judge), and optionally a **memo sheet** first for each judge (no rider, horse or number,
  "JUGE"/"RICHTER" across). Not yet: driving's variant (no correction or coefficient). Team
  dressage's split of riders among judges: **done** 2026-11-14 (S23);
- dressage ranking "with / without each judge's details" (`ranglisten.print.service.js`,
  `getRanglisteProRichter`) — **done** 2026-11-19: each judge's column has their points and
  percentage; *Classement par juge* adds that judge's rank, "66.45 % (3)" (by the judge's
  percentage, equal percentages the same rank, among those ranked);
- start list "lite", jury, for judges (with a visa column), "Promotion" (pedigree), start order
  — **done** (2026-11-13: *Liste de départ ▾*: full, with pedigree, short, for the judges with a
  result column and the counts and visa to fill in; the start order is the full list). The
  judges' list (2026-11-16): what the judge fills in down the right of the class's head, in
  Rosson's order — from … to …, starters without HC, starters HC, finished, placed, the judge's
  visa — Swiss Equestrian's office looks for them there; German in Rosson's words;
- per starter: start cards (**done**, redesigned 2026-11-13/14: Rosson copied the old paper
  card; ours fill the A5 — a big number box (to write in when the list has none), rider and
  horse side by side (licence, club, address, phone; passport, signalement, pedigree, owner,
  winning points), then each round of the class's own scoring table (round or phase 1, round 2
  or phase 2, jump-offs, winning round): a numbered box per obstacle (14; 7 in a jump-off or
  phase 2), below them what the jury adds up — penalties: (carried over +) obstacles + time
  penalties; time: (carried over +) time + correction; table C: time + penalty seconds +
  correction — and at the right **the round's totals side by side** (penalties or points, time),
  in the same place on every card to rank them. What
  is carried over is read from the table's blocks: round 1's penalties in round 2 (and its time
  in the "temps total des 2 manches" tables), phase 1's in a special two-phase class's phase 2,
  the earlier rounds' in a winning round (not "départ au 0"). The boxes as tall as the card
  allows; remarks and rank (no visa). Teams': a page per team, its riders with level and
  winning points, a block per round — **always two rounds and a jump-off**, team classes often
  go over both — per rider the same obstacles and sums, the team's penalties and time under the
  riders' totals, in teams of four a box to mark the result dropped), control sheet (driving:
  with driving), style class sheets (**done**, redone 2026-11-14 on Gestion Concours'
  layout, better than Rosson's, in EquiEvents' colours: A5, a page per rider and judge; licence
  classes with two judges (the office's five criteria), and — missing in Rosson — Swiss
  Equestrian's sheets for the initiation classes E40–E70 ("Einsteigerprüfung Springen") and the
  accompanied cavaletti class ECav ("Führzügel-Cavaletti-Prüfung"), one judge, half marks, in
  French and German as Swiss Equestrian words them; judges: the class's style judges, a role of
  their own, never sent to Swiss Equestrian — Rosson took the jumping judges "R", deviation S21);
- riders' letters (Serienbrief: their entries with date, time, class, number, level, horse, fee
  paid, total; address left or right; free text), entries in detail (per show, person, class;
  **done** 2026-11-13 for the show and a class: riders with phones, each horse's entries with
  class, number, fees, paid in bold when less, remarks and the office's note);
- labels (see above); officials' plan; driving: day start lists and its own prints — **done** 2026-11-14 (S25).

## Most needed for a one-person office (the inventory's view)
1. Late entries, with guest horse, teams and the "no class" box.
2. Start times with breaks, blocks, nachrücken and ground care.
3. Change of rider; moving an entry to another class.
4. An entry's details: start position by hand, comment, level, delete.
5. Prize money and prize labels.
6. Labels on real sheets.
7. Fixed start numbers for the whole show.
8. The overview with the classes' order (timetable) and open/final status.
9. Riders' addresses and letters with their fees.
10. History of changes (and backups).
