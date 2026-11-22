# Reconciliation rules

How the show's devices agree when they changed the same things while apart. These rules are
deterministic: every device that has received the same operations computes the same state, the
same conflicts and the same ignored operations, whatever order the operations arrived in.
Status per rule: **implemented** (with tests in `crates/show/tests/`) or **planned**.

## Principles

1. **The log is the truth.** Operations are never edited or deleted. The state is computed by
   applying them in stamp order (hybrid logical clock: time, counter, device id).
2. **Nothing is lost silently.** When a rule decides between two changes, the losing change stays
   in the log and is listed as a conflict for the show office.
3. **Official results do not change behind anyone's back.** Once a class is final, only an
   explicit reopening (with author and reason) lets its results change again.

## Rules

### R1 · Changes are field by field — implemented

An edit names each field it changes, the value it saw before, and the new value
(`ResultEdited`: a rider's faults, time, points or reason for being out). Two devices editing
*different* fields of the same score card (timekeeper: time; office: faults) both take effect,
without conflict.

### R2 · Same field, concurrent edits: the later one wins, and it is a conflict — implemented

If the value a change saw before is not the value the field has when the change is applied,
someone else changed it in between. The change is applied (the later stamp wins — the same
decision on every device), and a **conflict** is recorded with the three values: the one the
author saw, the one that was overwritten, and the new one. The office reviews and, if needed,
corrects with a new edit.

Whole score cards (`ResultEntered`, e.g. an import from a timing file) replace the card; made at
the show, what the card was is checked as anything else (R12).

### R3 · Final classes ignore changes until reopened — implemented

A change to a class's results stamped after `ClassFinalised` and before a `ClassReopened` is not
applied; it is listed as ignored with the reason.

### R4 · Finalisation records what it was based on — implemented

`ClassFinalised` lists the operations on the class's results that the finalising device knew.
A change stamped *before* the finalisation but unknown to it — made on a device that was offline
and synced later — is **not applied** and is listed as a conflict ("late change for a final
class"). The result that was finalised (and uploaded) stays what it was; the office decides
whether to reopen the class and apply the late change.

### R5 · Clocks are checked when operations are received — implemented

The hub (since 2026-11-13: equievents.ch) refuses operations stamped more than 5 minutes in its future: a device with a wrong
clock would otherwise win every conflict. Devices take the hub's clock into account when they
sync (`Clock::observe`), so their stamps stay in order with the hub's.

### R6 · Receiving the same operation twice changes nothing — implemented

Operations have unique ids; a file stores each one once.

### R7 · Start lists are merged rider by rider — implemented

A change to a start list (`StartsSet`) carries the list its author saw (`seen`, filled in by
the office when the change is made, `State::fill_seen`). Applied, only what the author changed
counts, over what others changed meanwhile (`show::merge::starts`):

- **Order**: the author's moves are made again on the current order (riders that did not move
  are a longest common sequence of the two lists). What they put first goes first, what they put
  last goes last, the others after the rider before them that others did not move (else before
  the one after them that nobody moved). Riders others added stay where they are; riders others
  took out stay out. Team classes: each rider's place in the team is a field, as numbers are.
- **Numbers**: rider by rider; the author's where they changed it.
- **Intent**: a late entry takes the next number when the change is applied (`next_numbers`),
  after numbers another computer gave meanwhile; a numbering gives the next numbers to riders its
  author did not know (`numbered`).
- **Conflicts**: the same rider moved on both (`StartOrder`), the same rider numbered on both
  (`StartNumbers`, with the numbers replaced), two riders with one number after the merge
  (`DuplicateNumbers`). The later change wins.

Operations without `seen` (imports, operations made before 2026-11-16) replace the list.

### R11 · Classes are merged field by field — implemented

A change to a class (`ClassSet`) carries the class its author saw. Applied, only the fields they
changed count (`show::merge::class`): a name changed here and a time changed there both stay,
also when one of them moves the class in its day (every class of the day is changed then). Fields
of the options and of the start list setup are fields of their own; the teams' starting order
is merged as a sequence. The same field changed on both: the later wins, a `ClassEdited`
conflict with the value replaced. A change to a class taken away meanwhile (sections put back
together) is not applied (`ClassGone`).

**Undo** after a sync takes back only one's own change: the undo carries, as what it saw, the
state right after the change it undoes, so what others changed since stays.

### R12 · Anything changed apart is said — implemented

Decided by the user, 2026-11-22 ("the users need to be aware that what they did clashed, in all
situations, always"): two computers offline gave a rider two different horses; the later stayed
and no one was told. Every change made at the show now carries what its author saw of each thing
it sets (`Operation::saw`, `show::touched`): an entry's fields one by one (horse, rider, status,
hors concours, class, level, note, the horses' phases), an official's role, a rider's address, a
team, prizes, a published class's printed category and levels, a test or scoring table, a
ranking over several classes, a rank set by hand, the jury's penalties, a whole score card, a
judge's sheet mark by mark (its penalties, its total). Applied where one of them is no longer what
the author saw — changed on another device, unknown to the author:

- the change is applied all the same (the later wins, as R2) and is a **conflict** (`change`)
  with what it replaced, the change it went over, both authors and times; the other version can
  be put back. Both giving the same value replaced nothing: no conflict.
- what it changed is gone meanwhile (an entry taken back), or its class was finalised meanwhile
  (a change stamped after the finalisation, R3): it is not applied, and that is a conflict too
  (`change_gone`).

Different things are no clash: a horse here and a note there both stay, two marks of the same
sheet too. Classes, start lists and results' fields keep their own rules (R11, R7, R2); deliveries
and imports carry nothing (Swiss Equestrian's). An undo carries, as what it saw, the state right
after the change it undoes: undone after someone else changed the same thing, it is said.
Operations made before 2026-11-22 carry nothing and are not checked. Tests:
`crates/show/tests/clashes.rs`, `two_horses_for_one_rider` in `crates/office/tests/offline.rs`.

### R13 · What a change relies on, changed apart, is said — implemented

Decided by the user, 2026-11-22 ("we need to cover as many options as possible"). Two changes of
*different* things may not go together: a result entered for a rider withdrawn on another
computer, moved to another class there, or given another horse or rider; marks of a judge taken
off the class (or replaced by someone else, or moved to another place) there; results entered
under a scoring table or test changed there; and the other way round (a rider withdrawn, moved,
given another horse, taken back while results were entered for them; a judge taken off while they
had marks entered; a class's scoring changed, or the class taken away, while results were entered
in it). A change also carries what it relies on (`touched::needs`), as fingerprints. Applied where
one of them is no longer what its author saw, both changes are kept and a conflict (`basis`)
says what changed and what it is now; the office checks they go together. Not applied (the
thing is gone), it is `change_gone`. A class's name or time changed is no clash with its results;
a judge's role code neither. This is what R10 planned.

### R14 · Sent to Swiss Equestrian without what was made before — implemented

A send records the changes what it sent was made of (`Uploaded::based_on`: a class's results,
a published class's start lists, a ranking over several classes). A change made before the send
but unknown to it — made offline, synced after — is not at Swiss Equestrian: what was sent
counts as changed after the send (« to send again » on the sending page), and a conflict
(`late_upload`) says how many changes. Results finalised before sending were already safe (R4);
results or start lists sent while the class runs are now too. The event's own data (officials,
entry counts) is not followed.

### R8 · Swiss Equestrian's data and the office's changes — implemented in part

Data downloaded from Swiss Equestrian (entries, riders, horses) is the base; changes made at the show (rider
or horse change, withdrawal) are separate operations on top. Downloading again updates the base
and keeps the local changes; if Swiss Equestrian changed the same field, its value wins and the
office is shown both: a conflict (`delivered`, since 2026-11-22) with the office's value, which
can be put back. Still planned: a layer that keeps the office's value instead.

### R9 · Who may change what — planned

Every operation carries its author. Roles decide what is applied: a judge's device only its own
judge sheets, timekeeping only jumping score cards, the office everything. Operations by an
author without the right are ignored and listed, on every device alike.

### R10 · Results of riders who do not start — implemented by R13

A result entered for an entry withdrawn at the same time on another device is kept (a rider who
does not start is not ranked) and listed as a conflict (R13).

### Who is told, and old apps

Since 2026-11-22 equievents.ch takes changes only from apps that say what their authors saw
(`X-EquiEvents-Sync: 2`, `show::oplog::SYNC_VERSION`); an older app is told, in its account's
language, to update (426, `update`): clashes with its changes would go unsaid. Someone no longer
on an event at equievents.ch (released by its owner, or taken off it) while working offline is
told how many changes made on their computer reached no one (`stranded`), with the copy kept.

## What the office sees

Every device finds the same conflicts (they come from the log), whatever was changed (R12). Each conflict names the change
kept and the earlier one it went over (`over`: the last operation that changed the same thing),
with their authors (the account's name) and times, and what was replaced (`lost`). The app shows
those not yet looked at on this computer beside the sync state, and once as a note when a sync
brings new ones; the dialog lists them, each with « Remettre ma version » / « Remettre la
version de … » (the class's fields as they were; the earlier start list change made again, its
moves and numbers over what is there now; the result's value as it was), and « Compris ». What
was looked at is kept in the show file of this computer (`conflicts_seen`), not shared: each
person sees what concerns them. Tests: `crates/show/tests/merge.rs`, `crates/office/tests/
offline.rs`, and `two_laptops_offline_on_the_same_class` in `crates/server/tests/api.rs` (two
laptops, the server down while they work, then synced through it).
