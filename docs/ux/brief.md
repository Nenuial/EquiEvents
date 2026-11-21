# The office app: what it must do better

Requirements from P. Burkhard (2026-11-13), in his words where possible, and how the app
answers them. A living document: improvements will keep coming, mostly once there is a working
app to try. Each item has an id (U…) so code and discussions can point at it.

## Who uses it

- **98 % of shows: one person** does the whole show office. The app is built for one operator
  who knows it well: keyboard first, everything at hand, nothing in the way.
- 2 %: several people at once, usually on different classes; possibly two people entering
  dressage results for different riders of the same class. The operation log already merges
  that (docs/architecture/reconciliation.md).
- **Judges and timekeepers do not use the app.** Timekeepers exchange Excel files with the
  office; judges' tablet files go through the office too.

## Layout

- **U1. Modals, not panels; one thing per screen** (2026-11-13, revised 2026-11-14). Rosson
  opens details in a cramped strip on the right side, sometimes only after a click on a button.
  The first answer (panes side by side, details opening in place under a row) was not the right
  one: *"The UX/UI feels overloaded in many places. Buttons everywhere. They are small. […] Not
  everything has to be crammed into the same page. It has to feel logical. […] I prefer having
  modals than […] opening panels everywhere!"* So: a screen shows one thing, full width; its
  parts are **tabs** (a class: start list, entry, ranking, settings; the officials: the event,
  then each day); details, settings and every "do this" open in a **dialog** over the screen
  (closed with Esc), never in a panel sliding in from a side or opening under a row. A screen
  shows its main actions only, large enough for a finger; the others are in **one menu** (⋯,
  *Organiser*). A dialog has one button that does it (the others: cancel, or a destructive one
  on the left). Frequent actions keep a keyboard shortcut. The rules: docs/ux/design.md.

- **U14. Style, and a light and a dark theme** (2026-11-14). *"The UI is flat. It lacks style!
  The app looks like a Microsoft Access app from the 1990s!!! […] Also we should have a dark and
  light theme."* One design system (docs/ux/design.md): surfaces in layers with soft shadows, a
  navy frame, a coral accent kept for the one main action of a screen, states as coloured pills
  with icons, empty states that say what to do, Inter as the typeface. Light, dark, or the
  computer's choice, from the frame or the settings; remembered on the device.

- **U10. Quick things on the phone, on the fly** (2026-11-13). Every screen works on a phone:
  the show office walks around the show ground and must be able to look up a rider, mark a
  withdrawal, check a start list, fix an official… Keyboard-first stays for the office desk;
  on touch screens the same actions are buttons (no key to type), large enough to tap, and wide
  tables become lists or scroll sideways with their first column kept in view.

## Participants

- **U2. Work from the participants, without switching to class preparation.** Many tasks need
  a jump from Teilnehmer to Prüfungsvorbereitung today. From a rider's or an entry's row:
  move an entry to another class or section, does-not-start, change horse or rider, start
  number, hors concours…
- **U3. Peek at a class's start list from there**, and reorder it, without leaving the
  participants.

## Dressage results

- **U4. Marks entered in the wrong place are never lost.** Today, marks typed for the wrong
  rider must be erased and typed again. In EquiEvents a judge's sheet can be **sent** to another
  rider or another judge, two sheets can be **swapped**, and a sheet can be **cut** (« Couper »,
  scissors: it goes to the clipboard, which shows while it holds sheets) and pasted later, also
  for another judge. A sheet can be **deleted** (undo brings it back). Small icon buttons in each
  judge's column, each with a tooltip (2026-11-15).
- **U5. Fast entry (optional, for power users):** typing `6.` writes `6.5`.

## Officials

- **U6. Officials as a table**, like JuryApp's (`JuryApp/jury-web/client/src/components/JuryMatrix.jsx`):
  per show day, officials as rows, classes as columns (number, category, time, scoring), a
  role in each cell, typed with one key (P, J/R, A, …), moved through with arrows and Tab; a
  class can be duplicated for a section, moved, hidden. For every discipline and role Rosson
  knows. Rosson's officials screen is the worst of the app.
- **U7. Later: take the jury from JuryApp directly** (an interface between the two apps).
  JuryApp covers show jumping for now.

## Show jumping

- **U8. A ranking over several classes** (e.g. a championship over two or three classes).
  Rosson's final ranking only adds rank points (dressage: percentages). Two ways, chosen for
  each ranking (2026-11-13):
  - **Penalties added:** the penalties of all the classes added together; a tie is not broken
    (ex aequo), unless chosen (2026-11-15): by the penalties in a class picked for it (ties may
    remain), or its result there (penalties, then time). The time in the last class was offered
    for a day, then taken away.
  - **Ranks added:** the ranks in the classes added together; a tie is broken by the rank in
    one of the classes, picked explicitly.

  Settled (2026-11-13): penalties are obstacle and time penalties together (the timekeeper's
  file has them together); only for classes of one round; the same rider and horse in every
  class; eliminated or retired in one class: listed after the ranked, without a rank.
- **U9. Start lists with the penalties a rider brings from another class** (finals where riders
  carry the points of the qualifier). Rules (2026-11-13): the penalties are only shown on the
  start list; the timekeeper includes them in the final's results. What is carried: the
  qualifier's total penalties (usually a one-round table A class). The start order can be the
  qualifier's ranking reversed, even if some of its riders are not in the final. Who starts the
  final is the office's decision. Swiss Equestrian gets the final's own result.

- **U11. Riders with several horses, as far apart as possible** (2026-11-13). Riders with two
  horses (group B) start at the beginning and at the end, riders with one (A) in between:
  **B1 A B2**. With three horses (C): **C1 B1 A C2 A B2 C3**, A cut in two. In general a rider's
  horses sit at the start, at even steps, and at the end (2: start, end; 3: start, middle, end;
  4: thirds); where blocks meet, the riders with most horses are outermost; one-horse riders
  fill the stretches between, in proportion. Inside a block the riders are drawn, and each keeps
  the same place in each of their blocks, so every rider of a group gets the same gap. Sorting
  (name, horse, winning points) sorts each block. Rosson managed two horses only (first early,
  all others at the end).
- **U12. A rider's horses rearranged, numbers following if asked** (2026-11-13). Moving a
  starter keeps its start number (as Rosson: good). For one rider's horses: swap their start
  places among the places they hold; an option makes the numbers follow the places (horses 2
  and 64 swapped: the earlier start keeps the lower number), for any number of horses.
- **U13. Splitting a class into sections, cleanly** (2026-11-13). Today a nightmare. The office
  sees every rider with their horses (a rider's horses always stay in one section), where they
  live, their club, the entry's comment (the coach is often written there), pony size and level;
  tags them into sections (two, three or more), by hand or by a rule — by coach, club, town,
  pony size (AB · C · D, or ABC · D), level, or balanced sizes; sees a preview (who is in which
  section, how many) and changes it again before applying. Within each section, U11 applies.
  Correction (2026-11-13): sections by **pony size or level are ranked separately**, so there
  each horse goes to the section of its size or level, even when a rider's horses end up in
  two sections; by club, town, comment or balanced sizes a rider's horses stay together.

## Status (2026-11-15)

- U1: the show's frame (a sidebar on a computer; a header and a bottom bar on a phone), screens
  in tabs, dialogs for details and settings (an entry, a start list peeked from the participants,
  dividing, order, layout, prizes, officials, the access key…), main actions only, the rest in
  one menu (2026-11-14).
- U2: from the participants: does not start, hors concours, change of horse. Still to come:
  move to another class, change of rider, start number.
- U3: a class's start list peeked from the participants, in a dialog, reordered there (drag,
  `Alt+↑↓`).
- U6: the officials in tabs: the event's own (a list; one opens in a dialog: role, days,
  responsible), then each day as a table (officials as rows, classes as columns), roles typed
  with one key or picked in the roles a click opens at the cell, officials found in Swiss
  Equestrian's national list in a dialog; undo.
- U10: every screen so far works on a phone: header, classes, a class (start list, ranking,
  both one under the other), participants (a list; a peeked start list takes the screen),
  officials (roles at the cell), dressage entry (riders, then one rider's sheet, one judge at a
  time).
  New screens are built for phones too.
- U4: dressage entry: a judge's sheet sent to another rider or judge (swapped if one is there),
  put aside and placed later, also for another judge; nothing is typed twice; undo.
- U5: fast entry (option): `6.` writes 6.5 and moves on.
- U8: rankings over several classes, by penalties or by ranks, on the classes screen.
- U9: a final's qualifier, its penalties on the start list, its ranking reversed as start order.
- Show-jumping results: entered by hand or from the timekeeper's file, the timekeeper's start
  list out (Rosson's format).
- U11, U12: a rider's horses spread out in the draw and sorted orders; marked on the start
  list; swapped among their places, numbers following if asked.
- U13: *Diviser…* on the start list (a dialog, one colour per section): every rider with their horses, town, club, comments
  (office's note, rider's remark), pony size, level; sections (one to six) proposed by a rule
  (balanced, club, town, comment, level, ponies AB · C · D or ABC · D), changed rider by rider
  or by a search (all found into one section); the result section by section; applied as one
  change; *Parties…* changes it again (one: the class whole again). A rider's horses stay in
  one section, spread out there; by pony size or level each horse goes where it is ranked
  (each horse can also be moved on its own).
- U14: the design system (app/src/ui), light and dark themes (2026-11-14).
- U7: to come.

## Already built in, waiting for screens

- History of every change (who, when, on which device): the base for undo and for a change log.
- Works offline; uploads are recorded (who, when).
- A notice when a Swiss Equestrian update replaces something the office changed (E39).
- One-file exports of a show, with or without private data.
- Old Rosson events imported as they ended, or as delivered to rehearse a show.
