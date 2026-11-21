# The EquiEvents app

One UI (React, TypeScript, Tailwind) for the desktop app (Tauri 2: macOS, Windows) and, later,
the browser. Everything it shows and does goes through the office core (`crates/office`):
one command, `call(command, args)`, answered in JSON. The UI holds no show logic.

What it does so far:

- **Library**: import a Rosson export, as the event ended (all results) or as Swiss
  Equestrian delivered it (to rehearse a show); open, export (`.equievents`, with its history;
  or *Rosson*: in Rosson's format with the event's access key, a copy that also opens in
  Rosson 3.3.15), delete.
- **Classes**: day by day, with status and what was sent to Swiss Equestrian; a class's start
  list and ranking side by side. Published classes not set up yet: **Préparer** creates the
  class with its entries, the start order drawn; *Suite de…* sets it up as a follow-up of a
  class (the same sections, rider and horse at the same place, the others at the end).
- **Dividing a class** (*Diviser…*, U13): every rider with their horses, town, club, the
  entries' comments (often the coach), pony size and level; sections proposed by a rule
  (balanced, club, town, comment, level, ponies AB · C · D or ABC · D), changed rider by rider or
  for everyone a search finds; the result shown section by section, then applied as one change.
  A rider's horses stay together, except by pony size or level (sections ranked separately):
  each horse goes where it is ranked, and each can be moved on its own. *Parties…* divides
  again, or makes the class whole.
- **Preparing a class**: draw the order again (a rider's horses as far apart as possible: at
  the start, at even steps and at the end, U11), give start numbers, split into sections (10a,
  10b, …; team classes by whole teams); a class with more than 90 entries (team classes: 90 teams) says so and proposes
  *Diviser en N parties* (it is divided before it runs; no ranking is split afterwards,
  `docs/deviations.md` S19); rename it, set its time (click on them); *Détails…*: its number and day, how many
  are placed, the sponsor on its prize labels, extra lines for its lists, its logo. In the start list: drag a starter, or
  select it and `Alt+↑↓` or type its place; a rider with several horses is marked 1/2, 2/2 (red
  when they are close), and selecting one shows them all: swap them among their places, the
  numbers staying with the horses or, if asked, following the places (U12); `N` does not start, `H` hors concours; send a starter to another
  section; *Ordre…*: the order again by rider, horse or winning points, as another class,
  after another class's ranking (the worst first), or from a chosen starter; *Mise en page…*: a plain list, blocks (N per block, each under a title), or start
  times (the first, every N minutes, breaks; non-starters give their time up or not), and a
  line after the n-th starter (ground care). Entries Swiss Equestrian delivers after the class
  was set up are on no start list: listed above it (and +N on the class) until put at the end
  of a list, with the next number. A final class cannot be changed (reopen it first).
- **Participants**: rider by rider, searchable (`/`); does not start and hors concours right
  in the row; **late entries** (*+ Inscription tardive*: a class, a rider of the show or typed
  in with the licence number, a horse of the show, typed in with its passport number, or a
  guest horse; team classes: a team with a free place, or a new one; at the end of the class's
  start list, with the next number); `…` on an entry:
  another rider (RW on start lists), another class (its starts and results in the old one go),
  level, the office's note, the rider's address and phones (kept when Swiss Equestrian delivers
  again), taking back an office entry; click a horse to change it (a horse of the show, or a guest horse by name); a
  class's start list opens *beside* the list, changeable there too (U2, U3 in
  `docs/ux/brief.md`); `Esc` closes it.
- **Officials** (`⌘3`, U6): per show day, the officials as rows and the day's classes as
  columns; select a cell (click, arrows, Tab) and type the role: `P` president, `J`/`R` judge,
  `W` water-jump judge, `A` warm-up arena; in dressage `E H C M B` the judge's position, `T` Tag
  der Jugend; Delete takes it away; the same roles are buttons in the bar under the table.
  `Shift+←→` or Shift+click selects several classes of an official's row (*Toute la journée*:
  all of them): one key gives each its role. Officials are added from Swiss Equestrian's national list (results open under the field).
  Above the days, the officials of the event itself (technical delegate, course designer, vet…):
  role, days of work, responsible; Swiss Equestrian's own stay as they are.
- **Dressage entry** (class screen, mode *Saisie*, `E`; U4, U5): the riders on the left with a
  chip per judge (complete, partial); the selected rider's sheets side by side, a column per
  judge, movements with their texts and coefficients, penalties to tick, totals and percentages
  as they are typed (provisional until every sheet is complete); *N° …* (`⌘J`) jumps to a
  start number. `Enter` goes down the column,
  then to the next judge and the next rider; `Alt+↑↓` changes rider. A sheet entered for the
  wrong rider or judge: *Envoyer…* moves it (swaps it with one already there), *De côté* puts it
  aside, to be placed later for any judge. Fast entry (option): `6.` writes 6.5. Special
  classes: the office chooses the programme.
- **Letters to the riders** (classes screen): one per rider, the address for a window envelope
  (left or right), the office's text, their entries with fees and the total.
- **Excel files**: the participants with their contact details and every entry (classes
  screen), a class's start list and ranking (by them).
- **Fixed start numbers** (classes screen): one number per entry for the whole show, given out
  by name, horse, class and start order, or at random; turning them off clears them.
- **Prizes** (*Prix…* by a class's ranking): an amount and a prize in kind per rank, computed as
  Rosson does (from the first prize, rounded up to 5 CHF, 80 % for the next, never below the
  minimum: (fee − 18 CHF tax) × 2) and changed by hand; on the printed ranking.
- **Labels** on Zweckform sheets (3475: 70 × 36 mm; 3484: 105 × 37 mm), the places already
  used skipped: riders' addresses (classes screen), the dressage judges' labels (one per judge
  and rider, class header), prize labels (one per rank, by the ranking).
- **History** (`⌘5`): every change of the show, latest first, said in the office's language,
  with who, when and on which device; an import or a file on one line; searchable.
- **Final classes**: *Clôturer l'épreuve* in a class's header (nothing changes it then);
  *Rouvrir…* asks why.
- **Undo** (`⌘Z`, or the button in the note that confirms each change): the office's own
  changes, one by one. Undoing adds changes; the show's history keeps everything.
- **Show-jumping results** (class screen, mode *Saisie*, `E`): the starters in start order, a
  field per column of the class's scoring table (penalties, time; rounds, phases, jump-offs),
  the status (éliminé, abandon, retiré…), and the rank, penalties and time as they are typed.
  `Enter` goes down the column, `Tab` across; *N° …* (`⌘J`) jumps to a start number. The timekeeper's start list (Excel, Rosson's
  format) and results file (Excel or text) are buttons in the bar; what the file could not
  say is listed to check; *Reprendre ses rangs* takes the timekeeper's ranks over. Undo takes
  a whole file back.
- **Taking away**: *Effacer les résultats* (ranking) and *Supprimer les listes* (start list: the
  class is "not set up" again), each asked twice and undone with `⌘Z`.
- **Finals after a qualifier** (U9): in the start list, *Qualification* names the class the
  riders come from; the start list shows the penalties each brings (column *Qualif.*), and
  *Ordre inverse de la qualification* sets the start order: the qualifier's winner last.
- **Rankings over several classes** (U8, on the classes screen): penalties added (a tie: the
  time in the last class) or ranks added (a tie: the rank in a class picked for it); riders
  with the same horse in every class; eliminated or retired in one of them, at the end without
  a rank.
- **Swiss Equestrian** (`⌘4`): the server (the simulation until the pilot: nothing leaves the
  computer), the event's access key (kept in the system's keychain), and for every class its
  start lists and results: when they were sent, by whom, changed since; send, send again,
  withdraw start lists; send the event. The simulation shows what it received, request by
  request. A class with results has a send button in its header too.
- **Prints** (PDF, opened in the system's viewer; in French or German as the office): in a
  class's header the start list (also with pedigree), the ranking, the dressage judges' sheets
  (Swiss Equestrian's official form as Rosson prints it, all judges in one document, a sheet
  per rider and judge; optionally a memo sheet for each judge, marked across); on the
  classes screen the timetable and the participants; a ranking over several classes; start
  cards (A5, two to a page, for the jury: per round of the class's scoring table the obstacle
  boxes and, below, the sums — penalties, time, what the table carries over from an earlier
  round; in team classes a page per team with two rounds and a jump-off)
  and, in style classes, the style judges' sheets. Made by
  the core with Typst (`crates/typeset`), kept in `<library>/prints/<show>/`.
- **On a phone** (U10): every screen above works on a narrow screen. Tables become lists
  (participants, classes) or keep their first column in view (officials); what the keyboard
  does is also a button: the roles of the selected official's cell, `↑ ↓` on the selected
  starter, the judge tabs and `↑ ↓` riders in dressage entry (one judge's sheet at a time).
  On a touch screen the row buttons (NP, HC…) are always shown and larger.
- French and German; light and dark following the system; the accent colour (coral by default,
  or pétrole, bordeaux, laiton, cobalt) chosen in the settings.

Nothing without an equievents.ch account (the user, 2026-11-21): signed out, the app shows only
signing in and making an account. Swiss Equestrian only through equievents.ch: events loaded with
their key, their updates, the riders and horses (per account), the demo events; never sent to
(« Envoyer » goes to the simulation).

Not yet: the rest of Rosson (`docs/rosson-inventory.md`).

## Running it

```bash
cd app && npm install            # once (node_modules is excluded from Dropbox sync)

# A local equievents.ch (the app needs an account): its Swiss Equestrian a stand-in, answering
# PRACTICE-KEY with the practice event (createdb equievents_dev, once):
DATABASE_URL="host=localhost dbname=equievents_dev" cargo run -p server --example se_stand_in

# In a browser, for working on the UI (data from a library folder of your choice):
EQUIEVENTS_SERVER=http://localhost:3999 cargo run -p office --bin office-dev-server -- /tmp/equievents-library
npm run dev                      # http://localhost:5173
# (files picked or saved in the browser's dialogs pass through the dev server's /files)

# The desktop app, live-reloading:
npx tauri dev

# The desktop app as a bundle (target/debug/bundle/macos/EquiEvents.app):
npx tauri build --debug --bundles app
```

The desktop app keeps its shows in `~/Library/Application Support/ch.equievents.office/library`
(Windows: `%APPDATA%\ch.equievents.office\library`).

Versions: the Rust crates follow what Rust 1.84 can build (Tauri 2.11, dialog plugin 2.7); the
npm packages `@tauri-apps/*` must be on the same minor versions.

The mark (the E: a horseshoe on its side with a bar) is drawn in `src/ui/Logo.tsx` (in the app,
on the chosen accent), `public/favicon.svg` (the browser's tab) and `src-tauri/icons/mark.svg`
(the desktop app's icon). The icons of every size come from the last one; to make them again,
`npx tauri icon src-tauri/icons/mark.svg -o <a scratch folder>` and copy back the files that
`src-tauri/icons` has (not its android and ios folders).
