# Rulebook

How EquiEvents ranks every class, in words. **Generated from the code** (`crates/scoring/src/rulebook.rs`): the rules as written at the top of each module of the engine, and every show-jumping scoring table as the engine reads it. Do not edit by hand: `EQUIEVENTS_UPDATE_RULEBOOK=1 cargo test -p scoring --test rulebook` writes it anew, and the test fails whenever the code says something else than this text.

Decisions where EquiEvents differs from Rosson or settles a point the regulations leave open are numbered S1, S2, … and explained in `docs/deviations.md`. Each rider's rank also comes with why (`scoring::jumping::Why`, `scoring::ranking::Reason`): the values the ranking compared, checked against the ranking itself in every test.

## Ranks, ties and placed riders (all disciplines)

Ranks, ties and placed riders, shared by all disciplines.

- Entries are ranked in order; the first gets rank 1.
- An entry tied with the one before it gets the same rank, and the next rank skips:
  1, 2, 2, 4 ("competition ranking").
- Eliminated or retired entries are *out*: listed after everyone who is ranked, all equal.
  Rosson stored them as rank 99; that number only survives at the API boundary.
- Tied entries are listed alphabetically by horse name (docs/deviations.md S4).
- "Placed" (klassiert) entries are those ranked within the placed count: by default 30 % of
  the results, rounded up, unless the organiser sets the number (S10).
- The show office can set a rank by hand (`with_manual_ranks`, S18); the others' ranks skip
  the numbers given by hand.

## Numbers and rounding

Numbers.

All marks, faults, times and percentages are exact decimals, never floating point: 0.1 + 0.2
is exactly 0.3, and a value is only rounded where a rule says so, with `round`.

Rosson used JavaScript floating point, which can differ from exact arithmetic in the last
digit (for example `(1.005).toFixed(2)` gives "1.00"). docs/deviations.md E5.

## Show jumping

Show jumping: entered faults and times → ranking, according to the class's scoring table.

#### The rule

1. **Deciding block.** Each rider is ranked in the first block of the scoring table
   (`table`) for which all required fields are entered. Faults and times of that block are
   added up.
2. **Order of blocks.** Everyone ranked in a more decisive block is ahead of everyone in a
   less decisive one — e.g. riders who rode both rounds of a two-round class ahead of riders
   who rode round 1 only.
3. **Within a block.** Fewer faults first (more points first in points-based blocks, S1),
   then the faster time. A missing or zero time counts as slowest.
4. **Ties.** Same block, same fault (or point) total and, if the block is timed, the same
   time total: same rank. 4+0 and 0+4 faults with the same time are a tie (S11). Tied riders
   are listed by horse name (S4).
5. **Jump-offs** (S13). Riders who rode the rounds before the jump-off are grouped by their
   score there (faults, or points; in a table-C round, the time) — time does not count for
   this. A group of two or more riders **equal first** is entitled to a jump-off; in
   championships with medal jump-offs (option `Options::medal_jump_offs`) so is every such
   group that includes a medal place (S14). Within such a group: the jump-off decides — faults,
   then a further jump-off if there is one, then the time. A rider of the group withdrawn,
   retired or eliminated in a jump-off ranks **equal last of that jump-off** — keeping the
   jump-offs completed before it (Springreglement 2.3.8; S8, S9, S29) — ahead of the next
   group. If every rider of a jump-off is out of it, nobody wins it: they are all equal last
   of it, e.g. five riders equal 5th (2.3.9; S30). A rider of the group with no result in it
   and not entered as out (not ridden yet, or cancelled) comes after those who rode it and
   ahead of those out of it; a jump-off nobody rode: all equal first of it. Other groups are
   ranked by time (rule 3).
6. **Winning rounds** (Siegerrunde, S17) work like a jump-off, except who is entitled: the
   placed riders of the rounds before it (30 %, or the number the organiser set). Whether
   points are carried into the winning round is part of the scoring table (table 29 adds the
   round-1 faults, tables 30 and 31 start from zero). Withdrawn, retired or eliminated in the
   winning round: equal last of the winning round.
7. **Second rounds** (S16) — round 2 of a two-round class, phase 2 of a normal two-phase
   class (S2): eliminated or retired there ranks **equal last of round 2**, ahead of the riders
   who only rode round 1. Withdrawn before round 2: ranked by round 1, like a rider who did not
   qualify (unlike a jump-off). In a normal two-phase class only riders clear in phase 1 can
   be in phase 2, and phase 2 follows without a break: withdrawing is not possible there, a
   rider who leaves the arena has retired — "withdrawn" entered for phase 2 counts as retired.
8. **Special two-phase classes** (S2), everyone rides both phases: eliminated, retired or
   withdrawn in either phase is out of the class.
9. Out otherwise — without any completed round, or after one but not in a jump-off or a
   second round — is **out**: equal last of the whole class (S31; Rosson ranked the second by
   its round).
10. Out riders are listed after all ranked riders: by reason code, then by horse name (S4).
11. **Missing time** in a points-based part (Zeitspringen, Punktespringen…): ranked last of
    those with the same points (Springreglement 4.3.3, 4.4.4; S32).
12. **Times** count to the hundredth: finer digits are cut, not rounded (7.9; S33).
13. **Placed:** in a class with a reduced second round, only riders who completed both rounds
    (3.4.2.1; S34); if nobody rode round 2 (cancelled), round 1 places.
14. **Knock-out** (table 55): duels; each round's field holds the duel's result (Wertung C:
    the time with 3 s per fault). The rider who goes furthest ranks first; in the furthest
    round the faster result wins; the losers of the same round are equal, eliminated or not —
    1st, 2nd, two 3rd, four 5th: eight placed (4.5; S35).

Source in Rosson: `RanglisteService.updateRanglisteSpringen`, `ResultatService.sortierenSpringen`.
Golden tests: every jumping class in `golden/corpus/`. Teams: `team`.

#### Why a rider stands where they do

`rank_explained` gives each rider a `Why`: the values the ranking compared, in the order
it compares them, each the rider's own ("round 1: 0 faults; jump-off: 4 faults, 33.00 s").
Whenever a class is ranked in a debug build (every test, the golden corpus included), the
order and the ties these values give are checked against the ranking itself: an explanation
that does not match the ranking fails the tests.

Not covered yet: ideal-time and maximum-time classes, manual ranks.

## Show jumping: teams

Team classes in show jumping.

#### The rule (P. Burkhard, 2026-11-13; docs/deviations.md S7)

A team's result is built **round by round** from its riders' results, then the team is
ranked like a single rider with the class's scoring table (`super::rank`). So the table
decides what counts — faults of both rounds with the time of both rounds, or with the time
of round 2 only — and a jump-off counts if there is one.

1. **Which results count, per round** (round 1, round 2, each jump-off):
   - team of 4 (Nations Cup formula): the team's worst result of the round is not counted —
     the most faults, then the slowest time; a rider without a result in that round
     (eliminated, retired, not started) is the worst;
   - in a jump-off usually one rider per team starts and that result counts; if all 4 start,
     the worst is dropped as well;
   - teams of 3: nothing is dropped; teams of 2: both results are always added;
   - **every result counts** when the class says so (`count_all`, a class option: the
     Ausschreibung defines how results count, Springreglement 4.6; S36): nothing is dropped,
     whatever the team's size.
2. **The team's values** are the sums, field by field, of the counted results.
3. **Team out:** a team of 4 with 2 riders out (eliminated, retired or withdrawn), a team of 3
   or 2 with 1 rider out — when every result counts, any team with 1 rider out — or a team
   without any result (P. Burkhard, 2026-11-13, 2026-11-17). Rosson kept
   ranking such teams with the results of the remaining riders.
4. **Riders** get their team's rank and placement (as in Rosson's `updateEquipenResultate`).

Rosson differs in two places (S7): it never drops a result in a jump-off, and in tables where
the time of both rounds counts it drops the rider worst over both rounds together instead of
per round.

## Dressage

Dressage: marks per judge → total per judge → percentage → ranking.

#### The rule

For each judge:
1. every movement's mark (including collective marks) times its coefficient, added up;
2. minus percentage penalties: that percentage of the points, rounded to 0.1;
3. minus point penalties (e.g. 2 points for the first error of course);
4. rounded to 2 decimals: the judge's total.

The entry's percentage is the judges' totals added up, divided by the maximum (the
programme's maximum points times the number of judges of the class), times 100, rounded to
2 decimals. A judge who has not marked the entry yet counts as 0.

**Tag der Jugend finals** (regional and Swiss, a class option): no ex aequo in ranks 1–3. Tied
entries there are separated by the sum of judges C and H's marks for the programme's given
movements, in their order (Führzügelklasse 2, 3; Einfacher Reitwettbewerb 6, 3;
Einsteigerdressur 15, 17; Dressuraufgabe 21, 24), then by drawing lots: the office sets those
ranks by hand (RL Tag der Jugend 10.3; S38).

Ranking: on the points, as the regulation ranks (RL Prüfungen 5; S3, decided 2026-11-17):
compared as the percentage to 4 decimals — within one programme the same order as the points,
and comparable when riders ride different programmes. The percentage is shown to 2 decimals;
two entries can show the same and rank apart. Equal entries rank equal and are listed
alphabetically by horse name (S4). Eliminated or retired entries are out.

Other programmes and ways of marking, as Rosson computes them (no corpus show has them: from
its code, `calcDressurResultat`, `getMaxPunkteDressur`):
- **Freestyle (Kür)**: technical (A) and artistic (B) part marks; a judge's total is half
  the points (after penalties); the part percentages are each part's points over its
  maximum (10 × coefficients) times the judges.
- **Children**: one sheet for the entry (judge C marks part A, the two E judges part B,
  together one result); the percentage is its total over the programme's maximum (not
  times the judges); parts A and B each over their maximum.
- **Totals entered per judge** (a class option): the judge's total as entered, less the
  ticked penalties; no part marks.

Source in Rosson: `PruefungsdurchfuehrungService.calcDressurResultat` and
`RanglisteService.updateRanglisteDressur`. Golden tests: every dressage class in
`golden/corpus/`. Not covered here: team dressage and driving dressage (their own rules).

## Driving

Driving (Fahren, attelage): the phases of a driving class, as Rosson scores them
(`pruefungsdurchfuehrung.service.js`; `docs/rosson/driving.md`), with Rosson's bugs left out
(`docs/deviations.md` S25).

- Dressage: each judge's total `160 − (points − penalties) × 160 / (movements × 10)` — the
  programme errors taken off the marks before the conversion to 160 (Fahrreglement 13.10 §3;
  S43; Rosson added them after it); the class's the judges' mean; lower is better.
- Marathon: phases A and B against their allowed and minimum times, B's obstacles' time × the
  factor and their penalties; lower is better.
- Cones: per round, time faults (A: half a point per second started over the allowed time, or
  half the overtime to the hundredth "internationally"; B: the time itself; JC: half a point
  per second started outside ±5 s around the allowed time) plus obstacle and other faults;
  lower is better, then (JC) the gap to the allowed time, else the time.
- Derby: the driven time and penalty seconds, each × the time factor; lower is better.
- Open: by time, points, penalty points, or points then time.
- Multitest: each judge's marks × coefficients less penalties A, B, C; the judges' mean;
  higher is better.

## Eventing

Eventing (Concours Complet): the cross-country phase and the overall ranking of a class made
of phases (dressage, show jumping, cross-country), as Rosson ranks them
(`pruefungsdurchfuehrung.service.js` `calcCrossResultat` 536–557, `rangliste.service.js`
`updateRanglisteCross` 709–727, `prepareGesamtResultate` and `updateRanglisteGesamt` 751–958).

Cross: the obstacle penalties as entered, plus, when the class counts them, 0.4 per second
started over the ideal time (none for being early). Ranked on those points, then the gap to
the ideal time, then the time, lowest first.

Overall: each phase gives a value (dressage 100 − %, jumping its penalties, cross its points;
or each phase's rank, "rank points"), added up; lowest first. A rider out in any phase is out
of the overall ranking; a phase not ridden adds nothing while it runs, and is out once it is
closed (eventing, E10).

Equal totals, eventing (Eventing-Reglement 5.9; decided 2026-11-17):
- CCN (dressage, jumping, cross): the cross result (obstacles, time and other penalties), then
  the time closer to the cross's ideal time, then dressage, then the faster jumping time;
  still equal, the rank is shared.
- Combined dressage + jumping: ranked by rank points, always; then the better dressage result.
- Combined jumping + cross: penalties; then the better cross result.
- Combined dressage + cross: penalties; then the better cross result, then dressage.

Equal totals, driving (Fahrreglement 13.10 §7; decided 2026-11-17):
- Full event (Vollprüfung: dressage, marathon, cones): the marathon, then dressage. Eliminated
  or not finished in a test: not out of the event — that test counts the highest penalties
  there + 25 %, and the driver ranks after everyone who finished every test (§7.1.2).
- Derby events (dressage + derby, with cones too): the derby, then dressage (§7.2).
- Any other (short events): dressage, then the driven dressage movements (without the
  collective marks), the most first; still equal, shared (§7.3).

Other classes made of phases: the phases in their order decide (lower value, then smaller gap
to the ideal time).

## Show jumping: every scoring table

A table is a list of parts (blocks), the most decisive first. A rider is ranked in the first part for which all its results are entered; everyone ranked in a part is ahead of everyone ranked in a part below it. In each part the values listed are added up and compared in the order given. Times count to the hundredth, finer digits cut (S33); a time that is missing or zero counts as the slowest. Riders equal in everything compared have the same rank and are listed by horse name (S4). Out without a jump-off or a second round to be equal last of: out, equal last of the class (S31).

### 1 · 2PHAC — 2 Phasen: 1. Wertung A mit Zeitmessung, 2. Wertung C

| Part | Results needed | Compared, in order |
|---|---|---|
| phase 2 | phase 2 time | faster time (phase 2 time) |
| phase 1 | phase 1 time, phase 1 faults | fewer faults (phase 1 faults); then faster time (phase 1 time) |

- **Two phases (normal):** only riders clear in phase 1 go on to phase 2, without a break. Eliminated or retired in phase 2: equal last of phase 2, ahead of the riders ranked by phase 1 — with faults in phase 1, out (S2, S16). Phase 2 follows without a break, so withdrawing is not possible: entered as withdrawn, it counts as retired.

### 2 · 2PHAOAZ — 2 Phasen: 1. Wertung A ohne Zeitmessung, 2. A mit Zeitmessung

| Part | Results needed | Compared, in order |
|---|---|---|
| phase 2 | phase 2 faults, phase 2 time | fewer faults (phase 2 faults); then faster time (phase 2 time) |
| phase 1 | phase 1 faults | fewer faults (phase 1 faults) |

- **Two phases (normal):** only riders clear in phase 1 go on to phase 2, without a break. Eliminated or retired in phase 2: equal last of phase 2, ahead of the riders ranked by phase 1 — with faults in phase 1, out (S2, S16). Phase 2 follows without a break, so withdrawing is not possible: entered as withdrawn, it counts as retired.

### 3 · 2PHAOC — 2 Phasen: 1. Wertung A ohne Zeitmessung, 2. Wertung C

| Part | Results needed | Compared, in order |
|---|---|---|
| phase 2 | phase 2 time | faster time (phase 2 time) |
| phase 1 | phase 1 faults | fewer faults (phase 1 faults) |

- **Two phases (normal):** only riders clear in phase 1 go on to phase 2, without a break. Eliminated or retired in phase 2: equal last of phase 2, ahead of the riders ranked by phase 1 — with faults in phase 1, out (S2, S16). Phase 2 follows without a break, so withdrawing is not possible: entered as withdrawn, it counts as retired.

### 4 · 2PHAZ — 2 Phasen Wertung A mit Zeitmessung

| Part | Results needed | Compared, in order |
|---|---|---|
| phase 2 | phase 2 faults, phase 2 time | fewer faults (phase 2 faults); then faster time (phase 2 time) |
| phase 1 | phase 1 faults, phase 1 time | fewer faults (phase 1 faults); then faster time (phase 1 time) |

- **Two phases (normal):** only riders clear in phase 1 go on to phase 2, without a break. Eliminated or retired in phase 2: equal last of phase 2, ahead of the riders ranked by phase 1 — with faults in phase 1, out (S2, S16). Phase 2 follows without a break, so withdrawing is not possible: entered as withdrawn, it counts as retired.

### 5 · 2PHAZPAZP — 2 Phasen Spez.: 1. Wertung A mit Zm + Punkte, 2. alle A m Zm + Punkte, Zeit 2. Phase

| Part | Results needed | Compared, in order |
|---|---|---|
| both phases | phase 1 faults, phase 2 faults, phase 2 time | fewer faults (phase 1 faults + phase 2 faults); then faster time (phase 2 time) |

- **Two phases (special):** everyone rides both phases. Eliminated, retired or withdrawn in either phase: out of the class (S2).

### 6 · 2PHCAZP — 2 Phasen Spez.: 1. Wertung C, 2. alle A mit Zeitmessung + Punkte, Zeit 1. Phase

| Part | Results needed | Compared, in order |
|---|---|---|
| phase 1 | phase 1 time | fewer faults (phase 2 faults); then faster time (phase 1 time) |

- **Two phases (special):** everyone rides both phases. Eliminated, retired or withdrawn in either phase: out of the class (S2).

### 7 · AOZM — Wertung A ohne Zeitmessung

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 faults | fewer faults (round 1 faults) |


### 12 · AZ2RUZ12 — 2 Umgänge (2.red.) A mit Zeitmessung (Zeit zählt aus beiden Umg.)

| Part | Results needed | Compared, in order |
|---|---|---|
| round 2 | round 1 time, round 2 time, round 1 faults, round 2 faults | fewer faults (round 1 faults + round 2 faults); then faster time (round 1 time + round 2 time) |
| round 1 | round 1 faults, round 1 time | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Round 2:** eliminated or retired in round 2: equal last of round 2, ahead of the riders ranked by round 1. Withdrawn before round 2: ranked by round 1 (S16).
- **Placed:** only riders who completed both rounds (Springreglement 3.4.2.1; S34); if nobody rode round 2 (cancelled), round 1 places.

### 14 · AZ — Wertung A mit Zeitmessung

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 time, round 1 faults | fewer faults (round 1 faults); then faster time (round 1 time) |


### 17 · AZ1AZ — Wertung A mit Zeitmessung und 1 Stechen mit Zeitmessung (A)

| Part | Results needed | Compared, in order |
|---|---|---|
| jump-off | jump-off 1 time, jump-off 1 faults | fewer faults (jump-off 1 faults); then faster time (jump-off 1 time) |
| round 1 | round 1 time, round 1 faults | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Jump-off:** ridden by the riders equal first after round 1 on faults — time does not count for this (S13); with the championship option, also every group equal on score that includes a medal place (S14). Within such a group the jump-off decides. Withdrawn, retired or eliminated in a jump-off: equal last of that jump-off, keeping those completed before it, ahead of the next group (S8, S9, S29); everyone out of it: nobody wins, all equal last of it (S30). No result and not out: after those who rode it; a jump-off nobody rode: all equal first. Everyone else is ranked by round 1.

### 18 · AZ1C — Wertung A mit Zeitmessung und 1 Stechen nach Wertung C

| Part | Results needed | Compared, in order |
|---|---|---|
| jump-off | jump-off 1 time | faster time (jump-off 1 time) |
| round 1 | round 1 faults, round 1 time | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Jump-off:** ridden by the riders equal first after round 1 on faults — time does not count for this (S13); with the championship option, also every group equal on score that includes a medal place (S14). Within such a group the jump-off decides. Withdrawn, retired or eliminated in a jump-off: equal last of that jump-off, keeping those completed before it, ahead of the next group (S8, S9, S29); everyone out of it: nobody wins, all equal last of it (S30). No result and not out: after those who rode it; a jump-off nobody rode: all equal first. Everyone else is ranked by round 1.

### 19 · AZ2A — Wertung A mit Zeitmessung und 2 Stechen ohne ZM

| Part | Results needed | Compared, in order |
|---|---|---|
| jump-off 2 | jump-off 2 faults | fewer faults (jump-off 2 faults) |
| jump-off 1 | jump-off 1 faults | fewer faults (jump-off 1 faults) |
| round 1 | round 1 time, round 1 faults | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Jump-offs:** ridden by the riders equal first after round 1 on faults — time does not count for this (S13); with the championship option, also every group equal on score that includes a medal place (S14). Within such a group jump-off 1 decides, then the next jump-off among those still equal, and so on (no time is taken). Withdrawn, retired or eliminated in a jump-off: equal last of that jump-off, keeping those completed before it, ahead of the next group (S8, S9, S29); everyone out of it: nobody wins, all equal last of it (S30). No result and not out: after those who rode it; a jump-off nobody rode: all equal first. Everyone else is ranked by round 1.

### 20 · AZ2AZ — Wertung A mit Zeitmessung und 2 Stechen mit Zeitmessung (A)

| Part | Results needed | Compared, in order |
|---|---|---|
| jump-off 2 | jump-off 2 faults, jump-off 2 time | fewer faults (jump-off 2 faults); then faster time (jump-off 2 time) |
| jump-off 1 | jump-off 1 time, jump-off 1 faults | fewer faults (jump-off 1 faults); then faster time (jump-off 1 time) |
| round 1 | round 1 time, round 1 faults | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Jump-offs:** ridden by the riders equal first after round 1 on faults — time does not count for this (S13); with the championship option, also every group equal on score that includes a medal place (S14). Within such a group jump-off 1 decides, then the next jump-off among those still equal, and so on; the time of the last jump-off they both completed comes last. Withdrawn, retired or eliminated in a jump-off: equal last of that jump-off, keeping those completed before it, ahead of the next group (S8, S9, S29); everyone out of it: nobody wins, all equal last of it (S30). No result and not out: after those who rode it; a jump-off nobody rode: all equal first. Everyone else is ranked by round 1.

### 21 · AZ2RUZ2 — 2 Umgänge (2.red.) A mit Zeitmessung (Zeit zählt aus dem 2. Umgang)

| Part | Results needed | Compared, in order |
|---|---|---|
| round 2 | round 1 faults, round 2 faults, round 2 time | fewer faults (round 1 faults + round 2 faults); then faster time (round 2 time) |
| round 1 | round 1 faults, round 1 time | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Round 2:** eliminated or retired in round 2: equal last of round 2, ahead of the riders ranked by round 1. Withdrawn before round 2: ranked by round 1 (S16).
- **Placed:** only riders who completed both rounds (Springreglement 3.4.2.1; S34); if nobody rode round 2 (cancelled), round 1 places.

### 23 · AZ2RU1AZZ2 — 2 Umgänge (2.red.) A mit Zeitmessung + 1 Stechen A mit Zeitmessung (Zeit zählt aus dem 2. Umgang)

| Part | Results needed | Compared, in order |
|---|---|---|
| jump-off | jump-off 1 time, jump-off 1 faults | fewer faults (jump-off 1 faults); then faster time (jump-off 1 time) |
| round 2 | round 1 faults, round 2 faults, round 2 time | fewer faults (round 1 faults + round 2 faults); then faster time (round 2 time) |
| round 1 | round 1 faults, round 1 time | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Jump-off:** ridden by the riders equal first after round 2 on faults — time does not count for this (S13); with the championship option, also every group equal on score that includes a medal place (S14). Within such a group the jump-off decides. Withdrawn, retired or eliminated in a jump-off: equal last of that jump-off, keeping those completed before it, ahead of the next group (S8, S9, S29); everyone out of it: nobody wins, all equal last of it (S30). No result and not out: after those who rode it; a jump-off nobody rode: all equal first. Everyone else is ranked by round 2.
- **Round 2:** eliminated or retired in round 2: equal last of round 2, ahead of the riders ranked by round 1. Withdrawn before round 2: ranked by round 1 (S16).
- **Placed:** only riders who completed both rounds (Springreglement 3.4.2.1; S34); if nobody rode round 2 (cancelled), round 1 places.

### 24 · AZ2RUSAZ2 — 2 Umgänge (2.red.) A mit Zeitmessung + 1 Siegerrunde A mit Zeitmessung (Zeit zählt aus dem 2. Umgang)

**Defined by EquiEvents** (S37): the published definition contradicts the Springreglement (3.1.4); the regulation is followed.

| Part | Results needed | Compared, in order |
|---|---|---|
| winning round | winning round faults, winning round time | fewer faults (round 1 faults + round 2 faults + winning round faults); then faster time (winning round time) |
| round 2 | round 1 faults, round 2 faults, round 2 time | fewer faults (round 1 faults + round 2 faults); then faster time (round 2 time) |
| round 1 | round 1 faults, round 1 time | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Winning round:** ridden by the placed riders after round 2 (30 % of the entries, or the number the organiser set; S17). What it adds up is the table's own: the winning round part above. Withdrawn, retired or eliminated in it: equal last of the winning round. The others are ranked by round 2.
- **Round 2:** eliminated or retired in round 2: equal last of round 2, ahead of the riders ranked by round 1. Withdrawn before round 2: ranked by round 1 (S16).
- **Placed:** only riders who completed both rounds (Springreglement 3.4.2.1; S34); if nobody rode round 2 (cancelled), round 1 places.

### 25 · AZ2S1.A2.C — Wertung A mit Zeitmessung und 2 Stechen, 1. (A), 2. (C)

| Part | Results needed | Compared, in order |
|---|---|---|
| jump-off 2 | jump-off 2 time | faster time (jump-off 2 time) |
| jump-off 1 | jump-off 1 faults | fewer faults (jump-off 1 faults) |
| round 1 | round 1 time, round 1 faults | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Jump-offs:** ridden by the riders equal first after round 1 on faults — time does not count for this (S13); with the championship option, also every group equal on score that includes a medal place (S14). Within such a group jump-off 1 decides, then the next jump-off among those still equal, and so on; the time of the last jump-off they both completed comes last. Withdrawn, retired or eliminated in a jump-off: equal last of that jump-off, keeping those completed before it, ahead of the next group (S8, S9, S29); everyone out of it: nobody wins, all equal last of it (S30). No result and not out: after those who rode it; a jump-off nobody rode: all equal first. Everyone else is ranked by round 1.

### 26 · AZ2UZ12 — 2 Umgängen A mit Zeitmessung

| Part | Results needed | Compared, in order |
|---|---|---|
| round 2 | round 1 faults, round 2 faults, round 2 time | fewer faults (round 1 faults + round 2 faults); then faster time (round 1 time + round 2 time) |
| round 1 | round 1 faults, round 1 time | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Round 2:** eliminated or retired in round 2: equal last of round 2, ahead of the riders ranked by round 1. Withdrawn before round 2: ranked by round 1 (S16).

### 27 · AZ2RU1AZZ12 — 2 Umgänge (2.red.) A mit Zeitmessung + 1 Stechen A mit Zeitmessung (Zeit zählt aus beiden Umg.)

| Part | Results needed | Compared, in order |
|---|---|---|
| jump-off | jump-off 1 time, jump-off 1 faults | fewer faults (jump-off 1 faults); then faster time (jump-off 1 time) |
| round 2 | round 1 faults, round 2 faults, round 2 time | fewer faults (round 1 faults + round 2 faults); then faster time (round 1 time + round 2 time) |
| round 1 | round 1 faults, round 1 time | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Jump-off:** ridden by the riders equal first after round 2 on faults — time does not count for this (S13); with the championship option, also every group equal on score that includes a medal place (S14). Within such a group the jump-off decides. Withdrawn, retired or eliminated in a jump-off: equal last of that jump-off, keeping those completed before it, ahead of the next group (S8, S9, S29); everyone out of it: nobody wins, all equal last of it (S30). No result and not out: after those who rode it; a jump-off nobody rode: all equal first. Everyone else is ranked by round 2.
- **Round 2:** eliminated or retired in round 2: equal last of round 2, ahead of the riders ranked by round 1. Withdrawn before round 2: ranked by round 1 (S16).
- **Placed:** only riders who completed both rounds (Springreglement 3.4.2.1; S34); if nobody rode round 2 (cancelled), round 1 places.

### 28 · AZ2UC — 2 Umgängen: 1. A mit Zeitmessung, 2. Wertung C

| Part | Results needed | Compared, in order |
|---|---|---|
| rounds 1 and 2 | round 1 faults, round 1 time, round 2 time | fewer faults (round 1 faults); then faster time (round 1 time + round 2 time) |


### 29 · AZAZSA — Wertung A mit Zeitmessung und Siegerrunde (A Zm) (m. Punkten aus N.P.)

| Part | Results needed | Compared, in order |
|---|---|---|
| winning round | winning round faults, winning round time | fewer faults (winning round faults + round 1 faults); then faster time (winning round time) |
| round 1 | round 1 time, round 1 faults | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Winning round:** ridden by the placed riders after round 1 (30 % of the entries, or the number the organiser set; S17). What it adds up is the table's own: the winning round part above. Withdrawn, retired or eliminated in it: equal last of the winning round. The others are ranked by round 1.

### 30 · AZAZSA0 — Wertung A mit Zeitmessung und Siegerrunde (A) (Start mit 0 Punkten)

| Part | Results needed | Compared, in order |
|---|---|---|
| winning round | winning round faults, winning round time | fewer faults (winning round faults); then faster time (winning round time) |
| round 1 | round 1 time, round 1 faults | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Winning round:** ridden by the placed riders after round 1 (30 % of the entries, or the number the organiser set; S17). What it adds up is the table's own: the winning round part above. Withdrawn, retired or eliminated in it: equal last of the winning round. The others are ranked by round 1.

### 31 · AZAZSC0 — Wertung A mit Zeitmessung und Siegerrunde (C) (Start bei 0)

| Part | Results needed | Compared, in order |
|---|---|---|
| winning round | winning round time | faster time (winning round time) |
| round 1 | round 1 time, round 1 faults | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Winning round:** ridden by the placed riders after round 1 (30 % of the entries, or the number the organiser set; S17). What it adds up is the table's own: the winning round part above. Withdrawn, retired or eliminated in it: equal last of the winning round. The others are ranked by round 1.

### 35 · AZI — Wertung A mit Zeitmessung Idealzeit

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 faults, round 1 time | fewer faults (round 1 faults); then closer to the class's ideal time, over or under the same (round 1 time) (S20) |


### 39 · BARR — Barrierenspringen

| Part | Results needed | Compared, in order |
|---|---|---|
| jump-off 4 | jump-off 4 faults | fewer faults (jump-off 4 faults) |
| jump-off 3 | jump-off 3 faults | fewer faults (jump-off 3 faults) |
| jump-off 2 | jump-off 2 faults | fewer faults (jump-off 2 faults) |
| jump-off 1 | jump-off 1 faults | fewer faults (jump-off 1 faults) |
| round 1 | round 1 faults | fewer faults (round 1 faults) |

- **Jump-offs:** ridden by the riders equal first after round 1 on faults — time does not count for this (S13); with the championship option, also every group equal on score that includes a medal place (S14). Within such a group jump-off 1 decides, then the next jump-off among those still equal, and so on (no time is taken). Withdrawn, retired or eliminated in a jump-off: equal last of that jump-off, keeping those completed before it, ahead of the next group (S8, S9, S29); everyone out of it: nobody wins, all equal last of it (S30). No result and not out: after those who rode it; a jump-off nobody rode: all equal first. Everyone else is ranked by round 1.

### 40 · C — Wertung C

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 time | faster time (round 1 time) |


### 41 · C1AZ — Wertung C und 1 Stechen mit Zeitmessung (A)

**Defined by EquiEvents** (S28): the published definition contradicts the Springreglement (2.3.2); the regulation is followed.

| Part | Results needed | Compared, in order |
|---|---|---|
| jump-off | jump-off 1 time, jump-off 1 faults | fewer faults (jump-off 1 faults); then faster time (jump-off 1 time) |
| round 1 | round 1 time | faster time (round 1 time) |

- **Jump-off:** ridden by the riders equal first after round 1 on time (table C: the time is the score) — time does not count for this (S13); with the championship option, also every group equal on score that includes a medal place (S14). Within such a group the jump-off decides. Withdrawn, retired or eliminated in a jump-off: equal last of that jump-off, keeping those completed before it, ahead of the next group (S8, S9, S29); everyone out of it: nobody wins, all equal last of it (S30). No result and not out: after those who rode it; a jump-off nobody rode: all equal first. Everyone else is ranked by round 1.

### 42 · DERBY — Derby

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 time, round 1 faults | fewer faults (round 1 faults); then faster time (round 1 time) |


### 44 · JAGD — Jagdspringen

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 time | faster time (round 1 time) |


### 55 · KNOCK — Knock-out Springen

**Knock-out** (S35): duels under Wertung C, round 1, round 2, round 3, round 4, round 5 in riding order; each round's field (`strafpunkte_initial_1`, then `strafpunkte_stechen_1` …) holds the duel's result, the time with 3 s per fault. The rider who reached the furthest round ranks first; in the furthest round the faster result wins; the losers of the same round are equal, eliminated or not (1st, 2nd, two 3rd, four 5th). Eight are placed (Springreglement 4.5).

### 58 · PROG — Progressives Punktespringen

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 points | more points (round 1 points); then faster time (round 1 time) |

- **Points:** the most points rank first (S1); a missing time is the slowest of equal points (S32).

### 59 · PROG2SAC — Progressives Punktespringen und 2 Stechen, 1. (A), 2. (C)

| Part | Results needed | Compared, in order |
|---|---|---|
| jump-off 2 | jump-off 2 time | faster time (jump-off 2 time) |
| jump-off 1 | jump-off 1 time, jump-off 1 faults | fewer faults (jump-off 1 faults); then faster time (jump-off 1 time) |
| round 1 | round 1 points | more points (round 1 points); then faster time (round 1 time) |

- **Jump-offs:** ridden by the riders equal first after round 1 on points — time does not count for this (S13); with the championship option, also every group equal on score that includes a medal place (S14). Within such a group jump-off 1 decides, then the next jump-off among those still equal, and so on; the time of the last jump-off they both completed comes last. Withdrawn, retired or eliminated in a jump-off: equal last of that jump-off, keeping those completed before it, ahead of the next group (S8, S9, S29); everyone out of it: nobody wins, all equal last of it (S30). No result and not out: after those who rode it; a jump-off nobody rode: all equal first. Everyone else is ranked by round 1.
- **Points:** the most points rank first (S1); a missing time is the slowest of equal points (S32).

### 60 · PROGJO — Progressives Punktespringen mit Joker

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 points | more points (round 1 points); then faster time (round 1 time) |

- **Points:** the most points rank first (S1); a missing time is the slowest of equal points (S32).

### 61 · PUIS — Puissance

**Defined by EquiEvents** (S27): the published definition contradicts the Springreglement (4.1.2); the regulation is followed.

| Part | Results needed | Compared, in order |
|---|---|---|
| jump-off 4 | jump-off 4 faults | fewer faults (jump-off 4 faults) |
| jump-off 3 | jump-off 3 faults | fewer faults (jump-off 3 faults) |
| jump-off 2 | jump-off 2 faults | fewer faults (jump-off 2 faults) |
| jump-off 1 | jump-off 1 faults | fewer faults (jump-off 1 faults) |
| round 1 | round 1 faults, round 1 time | fewer faults (round 1 faults); then faster time (round 1 time) |

- **Jump-offs:** ridden by the riders equal first after round 1 on faults — time does not count for this (S13); with the championship option, also every group equal on score that includes a medal place (S14). Within such a group jump-off 1 decides, then the next jump-off among those still equal, and so on (no time is taken). Withdrawn, retired or eliminated in a jump-off: equal last of that jump-off, keeping those completed before it, ahead of the next group (S8, S9, S29); everyone out of it: nobody wins, all equal last of it (S30). No result and not out: after those who rode it; a jump-off nobody rode: all equal first. Everyone else is ranked by round 1.

### 62 · PUNKT — Punktespringen

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 points | more points (round 1 points); then faster time (round 1 time) |

- **Points:** the most points rank first (S1); a missing time is the slowest of equal points (S32).

### 63 · STIL — Stilprüfung

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 points | more points (round 1 points); then faster time (round 1 time) |

- **Points:** the most points rank first (S1); a missing time is the slowest of equal points (S32).

### 64 · ZEIT — Zeitspringen

**Defined by EquiEvents** (S26): the published definition contradicts the Springreglement (4.3.4); the regulation is followed.

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 points | more points (round 1 points); then faster time (round 1 time) |

- **Points:** the most points rank first (S1); a missing time is the slowest of equal points (S32).

### 66 · Gymk — Gymkhana

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 points | more points (round 1 points); then faster time (round 1 time) |

- **Points:** the most points rank first (S1); a missing time is the slowest of equal points (S32).

### 101 · Rücke — Rückeprüfung

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 points | more points (round 1 points); then faster time (round 1 time) |

- **Points:** the most points rank first (S1); a missing time is the slowest of equal points (S32).

### 102 · STILOZM — Stilprüfung ohne Zeitmessung

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 points | more points (round 1 points) |

- **Points:** the most points rank first (S1); a missing time is the slowest of equal points (S32).

### 103 · STILMIDEAL — Stilprüfung mit Idealzeit

| Part | Results needed | Compared, in order |
|---|---|---|
| round 1 | round 1 points | more points (round 1 points); then closer to the class's ideal time, over or under the same (round 1 time) (S20) |

- **Points:** the most points rank first (S1); a missing time is the slowest of equal points (S32).
