# Regulations check (2026-11-17)

The engine, as `docs/rulebook.md` describes it, read against Swiss Equestrian's regulations:
Springreglement 2027, Dressurreglement 2027, Eventing-Reglement 2027 and Fahrreglement (texts
from info.swiss-equestrian.ch, document library 595, 599, 594, 452). Each finding names the
article; every wording it rests on was checked in the regulation's text. The Generelles
Reglement (GR) and the FEI rules were not part of this check.

Verdicts: **differs** (the engine does something else than the regulation says) · **missing**
(the regulation has a rule the engine does not apply) · **unclear** (the regulation is
ambiguous, or relies on the Ausschreibung) · **note** (matches, but worth knowing). Each needs a
decision before it is fixed; decisions go to `docs/deviations.md` as S entries.

## Show jumping (Springreglement 2027)

Decided by P. Burkhard, 2026-11-17 (last column).

| # | Finding | Article | Verdict | Decision |
|---|---|---|---|---|
| J1 | Several jump-offs (tables 19, 20, 25, 39, 59, 61): a rider out in a later jump-off (e.g. declines jump-off 3 after two clears) loses every jump-off result and drops to equal last of jump-off 1. The regulation: equal last of *that* jump-off; riders still in after the last jump-off share first place. Cause: a score card has one out reason for all its rounds. | RL Prüfungen 2.3.7, 2.3.8 | differs | Fixed: equal last of that jump-off, the earlier ones kept (S29). |
| J2 | Zeitspringen (64): ranked on time only (Swiss Equestrian's table has no points). The regulation ranks on Gutpunkte first, then time. | 4.3.4 | differs | Fixed: good points, then time (S26). |
| J3 | Puissance (61): its normal round is ranked on faults only (the published table). The regulation judges it under Wertung A *with* time. | 4.1.2, 3.1.3 | differs | Fixed: the normal round is timed (S27). |
| J4 | Table 24's winning round compares the faults of rounds 1 and 2 and the winning round's time, never its own faults (the published table; E20). The regulation: a winning round is a jump-off, under the same rules as the round. | 3.1.4.1, 2.3.2 | differs | Fixed: rounds 1 + 2 + winning round faults, then its time (S37). |
| J5 | Table 41 (C1AZ, "Stechen mit Zeitmessung (A)"): jump-off riders equal on jump-off faults are separated by their **round-1** time — the published table's time field is `zeit_initial_1`. | 2.3.2 | differs | Fixed: the jump-off's own time (S28). |
| J6 | Two rounds, reduced second round (12, 21, 23, 24, 27): only riders who completed both rounds are classified (placed). The engine can place riders who rode round 1 only, and riders equal last of round 2 (S16). | 3.4.2.1 | differs | Fixed: only riders of both rounds placed; round 2 not ridden, round 1 places (S34). |
| J7 | Out riders with a result (rule 9, as Rosson did, no S entry): in tables with a jump-off, a rider eliminated outside the jump-off is ranked among the finishers; without a jump-off, after them. The regulation ranks out riders only in jump-offs. | 7.11, 2.3.8 (GR not checked) | differs / unclear | Changed: no result in round 1, out (equal last of the class); a result, then out, equal last of the jump-off (S31). |
| J8 | Everyone entitled to the jump-off out (withdrawn, retired, eliminated): the engine ties them all first. The regulation: a rider who gives up or declines a jump-off can never be the winner. | 2.3.9 | missing | Fixed: all out of it, all equal last of it; nobody rode it, all equal first (S30). |
| J9 | Knock-out (55): head-to-head duels under Wertung C, with fixed placings (1, 1, 2 and 4 riders). The engine reads it like a Puissance (S12). | 4.5 | missing | Fixed: duels, the furthest round first, eight placed (S35). |
| J10 | Time faults (1 per started 4 s; per started second in jump-offs and winning rounds) and Wertung C's conversion of faults into seconds are not computed: what the timekeeper or the office enters is taken. | 8.13.1 | missing | Kept: what the timekeeper enters. Table C rounds have a time field only, so the penalty seconds are inside the time. |
| J11 | Who rides a winning round: the regulation says *at most* 30 % of the starters; the engine rounds up, includes everyone tied at the cut, and counts entries that did not start. | 3.1.4.1 | unclear | Kept: rounding up (S17). |
| J12 | Tables 30 and 31 start the winning round from zero (S17); the regulation carries the round's faults unless the Ausschreibung says otherwise. | 3.1.4.1, 3.1.4.2 | note | Kept: special tables (S17). |
| J13 | Teams: the Ausschreibung defines how results count; the engine always drops the worst of four (S7) — no way to add all four. | 4.6 | missing | Option added: every result counts (S36). |
| J14 | Table 6 (2PHCAZP): the published table judges phase 1 under C; the regulation judges both phases under A. | 3.3.2.2 | note | Kept: a special table, from a former rulebook. |
| J15 | Ideal time: going over or under is listed among the faults, without a rate; the engine counts no penalty (S20). | 8.1.4, 3.6.2 | unclear | Kept: no penalty over or under (S20). |
| J16 | Points and time classes: a rider who does not finish is last of those with equal points; the engine needs a time of 0 entered for that. | 4.3.3, 4.4.4 | note | Fixed: the time is not needed, missing is the slowest (S32). |
| J17 | Times to the hundredth at most; the engine compares every decimal entered (a timing file with thousandths would break ties). | 7.9 | note | Fixed: cut at the hundredth (S33). |
| J18 | Ponies: from 10 entries per category, a separate ranking. | RL Pony 8 | missing | Not needed: the office divides by pony size before the class (S19). |

## Dressage (Dressurreglement 2027)

Decided by P. Burkhard, 2026-11-17 (last column).

| # | Finding | Article | Verdict | Decision |
|---|---|---|---|---|
| D1 | More errors of course than the programme allows: excluded from the classification. The engine takes off the programme's points, or ignores a ticked "Ausschluss" with no points; the rider stays ranked. | DR 8.3 (4) | differs | Fixed: the exclusion box switches the rider to eliminated; penalties one set for every judge (S40). |
| D2 | Young-horse (JP) programmes are judged in tenths, on a scale of 1 to 10, jointly by the jury; the engine allows halves only, 0 included, and counts a missing judge's sheet as 0. | DR 8.1, 8.2 (2); RL Prüfungen 9.1 | differs / missing | Fixed: JP programmes in tenths, one sheet for the jury (S39). |
| D3 | More than 35 entries: two series with two juries (up to 40 with the TD's agreement). The engine warns only above 90 (S19). | DR 5.2 (1) | differs | Fixed: told above 35, has to be divided above 40, hors concours not counted; the office divides (S19). |
| D4 | Tag der Jugend finals, ranks 1–3: no ex aequo — the sum of judges C and H on given movements, in a given order, then a draw. | RL Tag der Jugend 10.3 | missing | Fixed: a class option, ties in ranks 1–3 by C + H, then lots (S38). Movement 24 of the Dressuraufgabe TdJ: P. Burkhard checks. |
| D5 | International programmes are judged under the FEI rules (tie-breaks, penalties may differ). | RL Prüfungen 8 | unclear | Kept: judged as national programmes; the FEI rules are not followed. |
| D6 | Ranking: the regulation ranks on points; the engine on the percentage at 2 decimals (S3). The same order within one programme, except two point totals that round to the same percentage. | RL Prüfungen 5 | note | Fixed: ranked on the points (S3); no corpus result changes. |
| D7 | 30 % of the starters are placed; rounding up is Rosson's (S10). | DR 4.3 (1) | note | Kept: rounded up, 30 % of 65 is 20 (S10). |
| D8 | Kür ties by the artistic part (S22): not in the Dressurreglement (Merkblatt Kür); its wording speaks of the winner, not every rank. | — | unclear | Kept: every rank, not only the winner (S22). |

## Eventing (Eventing-Reglement 2027)

Decided by P. Burkhard, 2026-11-17 (last column).

| # | Finding | Article | Verdict | Decision |
|---|---|---|---|---|
| E1 | Overall ties: the regulation decides by the cross-country result, then the time closest to the cross's ideal time, then dressage, then the fastest jumping time. The engine compares the phases in their order — dressage first by default. | 5.9.2 | differs | Fixed: cross, ideal time, dressage, jumping time (S41). |
| E2 | The fastest jumping time (last tie-break) never reaches the overall ranking. | 5.9.2 | differs | Fixed: the jumping time is the last tie-break (S41). |
| E3 | Combined classes: dressage + jumping are ranked by rank points; dressage + cross ties go to the cross first. The engine ranks by penalties and dressage first by default. | 5.9.1 | differs (by default) | Fixed, always: D + J rank points then dressage; J + C, D + C penalties then the cross (then dressage) (S41). |
| E4 | Cross time penalties, 0.4 per second started over the time: off unless the class option is set. | 8.3.5 | differs (by default) | Kept: a class option, off by default, as in Rosson — timekeepers include the time penalties (S41). |
| E5 | Cross maximum time (twice the ideal time): exceeding it excludes; not checked. | 8.3.5 | missing | Fixed: over twice the ideal time, eliminated, when the ideal time is set (S41). |
| E6 | Jumping phase: 0.4 per second started over the time; off by default. | 8.2.2 | differs (by default) | Kept: as E4 (S41). |
| E7 | Cross obstacle penalties (refusals 20 / 40, the third excludes, three incidents exclude, a fall excludes, frangible devices 11, dangerous riding 25) are entered as one number; nothing derived or checked. | 8.3.6, 8.3.7 | missing | Kept: from the timekeeper and the jury (S41). |
| E8 | Time a rider was held on the course is credited; no field for it. | 8.3.8 | missing | Kept: the timekeeper's (S41). |
| E9 | 25-point sanctions (yellow card, mistreatment, dangerous riding) can fall in any phase; only the cross can carry them. | 2.2, 8.4, 8.5 | missing | Fixed: the jury's penalties in dressage and jumping, set on the overall ranking, sent in the phase's number (S41). |
| E10 | A phase not ridden and not marked out adds nothing: right during the show, wrong in a final ranking. | 1.3 | note | Fixed: a closed phase without a result is out (S41). |
| E11 | Dressage penalties: the regulation's example averages each judge's percentage; the engine divides the judges' total — the last digit can differ. | 8.1.2 | note | Fixed: the average of each judge's percentage (S41). |
| E12 | Cross style classes (style mark, deductions, ties by the ideal time) and indoor cross (time penalties per the jumping rules) have no phase of their own. | special classes | missing / unclear | Left out for now. |

## Driving (Fahrreglement)

Decided by P. Burkhard, 2026-11-17 (last column).

| # | Finding | Article | Verdict | Decision |
|---|---|---|---|---|
| F1 | Overall ties: in a Vollprüfung the marathon (B) decides; in RVP and DVP the derby (D). The engine compares the phases in their order, dressage first — in an RVP (dressage + derby) that ranks tied drivers exactly the wrong way round. | 13.10 §7.1.1, §7.2 | differs | Fixed: full event the marathon, then dressage; derby events the derby, then dressage (S42). |
| F2 | Vollprüfung: eliminated or not finishing one test is not out of the event — that test counts the highest penalties plus 25 %, and the driver ranks below everyone who finished all tests. The engine puts them out of the overall ranking. | 13.10 §7.1.2 | differs | Fixed: the highest penalties + 25 %, ranked after those who finished every test (S42). |
| F3 | Other classes: ties by the better result of the dressage movements driven. | 13.10 §7.3 | missing | Fixed: dressage, then the driven movements (S42). |
| F4 | Cones in a Vollprüfung: Wertung A, and the time faults computed to the hundredth ("international"), are mandatory; the engine defaults to 0.5 per started second and allows Wertung B. | 8.1.1, 8.4.4 | differs (by default) | Fixed: a full event's cones Wertung A, the overtime to the hundredth (S42). |
| F5 | Dressage programmes over 160 points: converted with a factor (0.8 … 0.55 for 300 points; the engine uses 160 / maximum, 0.533 for 300), penalties taken off before the conversion, the mean rounded before. | 13.10 §3 | differs / unclear | Programme errors off before the conversion; the factor kept as Rosson's, 160 / maximum — no test has 200 or 300 points (S43). |
| F6 | Marathon transfer phase: its time allowed and maximum time are not checked. | 7.3.2, 7.9 | missing | Kept: the jury's (S42). |
| F7 | Derby: over the maximum time (twice the time allowed) is elimination; not checked. | 9.1, 9.2 | missing | Kept: the jury's decision (S42). |
| F8 | Combinations with two dressage tests or two cones: one phase of each kind only. | 1.5 (2) | missing | **Not done** (left aside, P. Burkhard 2026-11-18): see below. |
| F9 | Presentation judged standing (level L): no phase of its own. | 13.10 §2 | missing | Fixed: a presentation phase, 10 − the judges' mean (S44). |
| F10 | Marathon time penalties: 0.25 per second — the engine counts per started second. | 7.9 | unclear | Kept: per second started (P. Burkhard); the rule's wording explained. |
| F11 | Cones Wertung B: faults are seconds; the engine adds what is entered. | 8.4.7 | note | Fixed: in Wertung B the faults are labelled seconds; no overtime penalty, over the maximum time eliminated (S42). |
| F12 | Neutralisation in whole minutes; not checked. | 7.6 (2) | note | Kept: seconds, as the office enters them. |

## Not done yet

Left aside or waiting, from the decisions above (2026-11-17, 2026-11-18):

- **J18 · Ponies:** none needed — the office divides by pony size before the class.
- **D4 · Tag der Jugend, Dressuraufgabe:** the regulation's tie movement 24 is not in the
  programme Swiss Equestrian delivers (23 movements): P. Burkhard checks.
- **E12 · Cross style classes and indoor cross:** left out for now.
- **F5 · Driving dressage factor:** Rosson's 160 / maximum, kept; the rules' 0.55 for 300 points
  and the factor for the tests in use (150, 210 to 260 points) to be asked of Swiss Equestrian.
  The FEI Juniors test is published with 370 points for 22 marks: driving counts each mark once
  (220), as Rosson.
- **F8 · Two dressage tests or two cones in one driving class:** one phase of each kind only.
  Needs: a second phase's id, which one counts in the ties, how Swiss Equestrian lists and takes
  two phases of one kind (not seen yet), and the Rosson file (one phase per kind).
- **Presentation (S44):** whether Swiss Equestrian takes a `praesentation` phase, at the pilot.
