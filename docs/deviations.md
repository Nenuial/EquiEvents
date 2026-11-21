# Deviations ledger

Every place where the new app will behave differently from Rosson, and every Rosson behaviour we
deliberately do not copy, gets an entry here. Rosson's output is evidence, not truth.

- **Scoring and ranking (S)** entries change official results. Each needs a decision against the
  Swiss Equestrian regulations, a reference to the article, and a named person who signed it off. Until then
  the status is *open*, and the new engine has no golden test for that case.
- **Engineering and data (E)** entries are local sloppiness that we fix by design. API exchanges
  with my.swiss-equestrian.ch are the only frozen contract; everything else may change.

Evidence comes from the golden corpus (`golden/corpus/`, see `SUMMARY.md` there) and the Rosson
3.3.15 source (`vendor/rosson-3.3.15/`, created by `golden/src/vendor-rosson.mjs`).

Status values: *open* · *decided: keep Rosson* · *decided: change* · *done*.

## S — Scoring and ranking

### S1 · Points-based classes rank highest score first — *decided: keep Rosson 3.3.15*

- Rule (P. Burkhard, 2026-11-13): classes scored on points — style, progressive (PROG),
  Punktespringen and the like — rank the highest score first. In the definitions these are the
  blocks marked `prog: true` (tables 58, 59, 60, 62, 63; 102 and 103 to be checked).
- The stored rankings of 2026.8352 Prüfung 6 (Punktespringen, Rosson 3.3.14) and 2025.8161
  Prüfung 7 (Stilprüfung, 3.3.8) were computed lowest first and contradict the rule. Their golden
  expectation is the rule (= what Rosson 3.3.15 recomputes), not the stored ranks.

### S2 · Elimination and retirement in two-phase classes — *decided: change*

- Rule (P. Burkhard, 2026-11-13):
  - Normal two-phase tables (1–4, `2PHAC`, `2PHAOAZ`, `2PHAOC`, `2PHAZ`): eliminated or retired
    in phase 2 rank **equal last of phase 2**.
  - Special two-phase tables (5 `2PHAZPAZP`, 6 `2PHCAZP`: everybody rides both phases, time from
    one phase only): eliminated or retired in **either** phase means out of the whole class;
    all of them rank equal last.
- Rosson instead keeps a retired rider's partial result (`aufgabeMitResultat`) and ranks it
  inside the completed block (seen at the shows: 2025.8161 P5, P6; 2026.8352 P5).

### S3 · Dressage ties — *decided: rank on the points* (P. Burkhard, 2026-11-13, changed 2026-11-17)

- Rule: the regulation ranks on points (RL Prüfungen 5). The engine compares the percentage to 4
  decimals: within one programme the same order as the points, and comparable when riders ride
  different programmes. The percentage is shown and sent to 2 decimals, as before.
- Before (Rosson): the percentage at 2 decimals; two different point totals showing the same
  percentage ranked equal. That needs a class maximum over 5,000 points with half marks; the
  largest programme has 560 points (FEI Children Preliminary B), × 5 judges 2,800: it never
  happens, and no corpus result changes.

### S4 · Order of tied entries — *decided: change*

- Rule (P. Burkhard, 2026-11-13): tied entries are listed alphabetically by horse name.
  Rosson's order is arbitrary (its sort comparators never return 0).
- Applied to jumping ties as well until told otherwise.
- Riders who are out are listed by reason code (as Rosson did), then by horse name.

### S5 · Scoring tables without a usable definition — *decided* (P. Burkhard, 2026-11-14)

| Table | Code | Name | Problem | Scored |
|---|---|---|---|---|
| 30 | `AZAZSA0` | Wertung A mit Zeitmessung und Siegerrunde (A) (Start mit 0 Punkten) | Invalid JSON (trailing comma) | As published, the comma ignored |
| 66 | `Gymk` | Gymkhana | Empty definition | Points, then time |
| 101 | `Rücke` | Rückeprüfung | Empty definition | Points, then time |
| 103 | `STILMIDEAL` | Stilprüfung mit Idealzeit | Empty definition | Points, then the gap to the ideal time |

- 30: about 30 % go on to a winning round, and they start it from zero: the winning round alone
  ranks them, their first-round result is not carried. This is what the published definition
  says once its comma before a closing bracket is ignored (`parse_definition`).
- 103: a style class with an ideal time. The most points first; between equal points, the rider
  closest to the ideal time, over or under the same; equal points and an equal gap rank equal.
  The office enters the ideal time in the class details, as for table 35 (S20).
- 66: Gymkhana rules of the ZKV (Reglement ZKV-Gymkhana, valid from 2024-12-01) and of the Swiss
  Freiberger federation (Gymkhanareglement FM/HF 2027), scoring B: ten points per obstacle
  mastered, none for one given up; the most points first, then the faster time. The ZKV's
  scoring A (time plus 30 s per obstacle given up, no points) has no table of its own: the office
  enters the time with the 30 s added and 0 points for everyone.
- 101: Reglement Rückeprüfungen FM/HF (valid from 2026-08-25): five points per cone passed, less
  0.1 per cm of deviation, less penalties; "Die Rangierung erfolgt nach den erzielten
  Gutpunkten. Bei Punktegleichheit entscheidet die bessere, effektiv benötigte Zeit." The judge
  adds up the points; the office enters the total and the time.
- The three empty tables get their definition from `definition_text` (`crates/scoring/src/jumping/table.rs`)
  as long as Swiss Equestrian publishes them empty; a published definition replaces ours.

### S6 · Stored time/faults that do not follow from the entered values — *decided: not pursued* (P. Burkhard, 2026-11-14)

- Evidence: 2026.8352 Prüfung 7 (table 27, team class), 10 riders. Example: the winner is stored
  with `zeit: 1` while the jump-off fields are empty. Ranks are unaffected; these look like
  leftovers from earlier edits that Rosson never cleared (see E3).
- To check: whether these values went into the upload (`putResultate`) and appear online.

### S7 · Team classes — *decided: change*

- Rules (P. Burkhard, 2026-11-13):
  - Teams of 4 (Nations Cup formula): in each round the team's worst result is not counted. In a
    jump-off usually one rider per team starts; if all 4 start, the worst is dropped as well.
  - Teams of 3: no result is dropped. Teams of 2: the two results are always added.
  - Unless the class counts every result (option, S36): nothing is dropped.
  - What is added depends on the scoring table: faults of both rounds with the time of both
    rounds, or with the time of round 2 only. A jump-off always counts if there is one.
  - A team is **out** with 2 riders out of 4, or 1 out of 3 or 2 — eliminated, retired (and, in
    the engine, withdrawn). Rosson kept ranking such teams on the remaining results: 2026.8352
    P7, team 14061 (all three riders eliminated) was ranked 6th; every team below it one too low.
  - **Team-only classes** (not in the rulebook, but dozens a year, e.g. relay formats): only the
    team gets a result. Rosson users entered it on one rider ("the trick"); the new app has a
    class option and the result is entered on the team. Whether the API accepts a class with
    team rows only is to be checked on the Swiss Equestrian side.
- Rosson stored team rows with empty totals and computed team ranks as a side effect of
  displaying the ranking (E4).

### S8 · Withdrawing from a jump-off — *decided: change*

- Rule (P. Burkhard, 2026-11-13): a rider who qualified for the jump-off and withdraws ranks
  **equal last of the jump-off**, together with riders eliminated or retired in the jump-off.
- Rosson listed such a rider *first* (rule meant for the provisional list while the jump-off
  is running). A provisional list is a display concern and will be handled there.

### S9 · Eliminated or retired in a jump-off — *decided: change*

- Rule (P. Burkhard, 2026-11-13): equal last of the jump-off, together with withdrawn riders
  (S8). Rosson ranked them by their round-1 result. For second rounds see S16; several
  jump-offs, a jump-off everyone left: S29, S30.

### S10 · How many riders are placed — *decided: keep Rosson*

- Rule (P. Burkhard, 2026-11-13): ⌈30 % × number of results⌉ unless the organiser sets the
  number, in all disciplines. Results count eliminated and retired riders, not hors concours.

### S11 · What counts as a tie in jumping — *decided: change*

- Rule (P. Burkhard, 2026-11-13): equal when the deciding block, its fault (or point) total and
  its time total are equal. 4+0 and 0+4 faults with the same time are a tie.
- Rosson compared the entered fields one by one. Real case: 2025.8158 Prüfung 1, two riders
  with 4 faults and 36.49 s ranked 14 and 15 at the show; correct is 14 and 14.

### S12 · Copied codes inside two definitions — *decided: not pursued* (P. Burkhard, 2026-11-14)

- Table 44 (Jagdspringen, record code `JAGD`) has `"code": "DERBY"` inside its definition, table
  55 (Knock-out, `KNOCK`) has `"code": "PUIS"`. Rosson looks tables up by the record code, so
  this is cosmetic there; the new engine uses the record code too. The knock-out is ranked by
  its own rule (S35).

### S13 · Who jumps off — *decided*

- Rule (P. Burkhard, 2026-11-13): riders **equal first on points only** (faults, or points in
  points-based classes; time does not count) are entitled to the jump-off. Points classes can
  have jump-offs too (rare). A rider alone in first place wins without a jump-off.
- Rosson had no notion of who is entitled; it ranked by blocks, so a jump-off result entered for
  a rider outside the first group put that rider ahead of the winner.

### S14 · Medal jump-offs in championships — *decided, new feature*

- Rule (P. Burkhard, 2026-11-13): in championships, ties for medals are jumped off: gold and
  silver among the riders equal first (on 0, or 4, or 8...), and a separate jump-off for bronze
  among all riders equal on the next score (with time faults this can be 5 or 6). Rosson did not
  support this; users added 100 points to the bronze jump-off, or forced the ranks by hand.
- In the engine: class option "medal jump-offs"; every tied group that includes a medal place is
  entitled to its own jump-off, and riders are ranked within their group by it. Data entered with
  the "+100" trick still ranks correctly, since each group is ranked on its own.

### S15 · Ghost teams — *found in the data*

- 2025.8161 Prüfung 8 (team class): team 13103, whose three riders had all been withdrawn
  ("abgemeldet"), was still ranked 19th — with the results of team 13105, under which the team
  had apparently been re-registered. Every team from 21st down got a rank one too low in the
  published result. A team without a starting rider has no result in the engine.

### S16 · Eliminated, retired or withdrawn in round 2 — *decided: change*

- Rule (P. Burkhard, 2026-11-13): in round 2 of a two-round class, eliminated or retired ranks
  **equal last of round 2**, ahead of the riders who only rode round 1. **Withdrawn** before
  round 2 is ranked by round 1 — unlike a jump-off. Phase 2 of a normal two-phase class
  follows phase 1 without a break: withdrawing there is not possible, leaving the arena is
  retiring, so "withdrawn" entered for phase 2 counts as retired. Withdrawn team riders count
  towards a team being out (S7). Riders tied on the last placed rank are all entitled to a
  winning round (S17).
- Rosson ranked all of them by their round-1 result. Five corpus classes change: 2025.8118 P7,
  2025.8161 P9b and P9c, 2026.8352 P8 and P11.

### S17 · Winning rounds (Siegerrunde) — *decided*

- Rule (P. Burkhard, 2026-11-13): a winning round is essentially a jump-off, but the riders
  entitled to it are the **placed riders** (30 %) of the rounds before. Riders carry their
  points into it as the scoring table says (table 29 adds the round-1 faults automatically;
  tables 30 and 31 start from zero). Withdrawn, retired or eliminated in the winning round:
  equal last of it.
- Rosson pre-selected only the *clear* riders for the winning-round start list.
- Table 24's definition counts the carried faults of rounds 1 and 2 in the winning round but not
  the winning round's own faults (E20); EquiEvents counts them (S37).

### S18 · Ranks set by hand — *decided, implemented*

- Rule (P. Burkhard, 2026-11-13, keeping Rosson's option): the office can set a rider's or a
  team's rank by hand, with a reason, and can take the timekeeper's ranks over when importing
  a results file ("Rang übernehmen"). The rider gets that rank; everyone else keeps their order
  and their ties, and their ranks skip the numbers set by hand (a rank set for two riders takes
  two places, as any tie). Riders who are out stay out unless given a rank.
- In the engine: `scoring::ranking::with_manual_ranks`, one rule for every discipline (Rosson
  had three variants), cases in `crates/scoring/tests/cases/manual-ranks.md`. In the show: the
  operation `RankSet` (protected like results: R3, R4), and `State::ranking`, the one way from
  results to a class's ranking. A rank taken over from a timekeeper's file is replaced by the
  next file; one set by the office stays until the office removes it.
- Checked against Rosson 3.3.15 on the 53 individual jumping classes of the corpus, with the
  last-ranked rider sent as 1st by the timekeeper (`crates/timing/tests/rank_adoption.rs`):
  same ranking in all, except E32.

### S19 · Large classes: divided before they run, no split ranking — *decided: change*

- Rosson 3.3.15 splits the *ranking* of a jumping class with more than 69 entries, after it was
  run: every second rider by rank goes to a second class `_2` with their result, officials
  copied (`pruefungsdurchfuehrung.service.js` `splitPruefung`, `mergePruefungen`).
- Rule (P. Burkhard, 2026-11-13): this no longer happens. A class with more than 90 entries is
  divided into sections before it is run.
- Team classes count teams, not riders: more than 90 teams (never seen in practice; the
  largest, 2026.8352 Prüfung 7, has 132 riders in 44 teams).
- In the app: no ranking split. A class with more than 90 entries on its start list (teams in
  team classes) is flagged (on the classes and above its start list), with *Diviser en N
  parties* proposed (N so that no section has more than 90); a team class is divided by whole
  teams (`office::prepare::split`, `office::views::MAX_ENTRIES`). Not a block: results can
  still be entered, the warning stays.
- Dressage (P. Burkhard, 2026-11-17; Dressurreglement 5.2 (1)): more than 35 starters,
  hors concours not counted (as the regulation says), is told « two series with two juries, the technical delegate can
  allow up to 40 »; more than 40, « has to be divided ». Dividing stays the office's decision;
  *Diviser…* proposes series of at most 35.

### S20 · Ideal-time table — *decided*

- Table 35 (AZI, "Barème A au chronomètre temps idéal"): Rosson ranks by penalties, then the
  gap to the class's ideal time (`idealzeit`, "mm:ss"); a time of twice the ideal time or more
  is made "retired" automatically (`rangliste.service.js` 276–286).
- Rule (P. Burkhard, 2026-11-14): the office enters the ideal time; riders rank on penalties,
  least first; between equal penalties the one closest to the ideal time is better, over or
  under the same; equal gaps tie. The table has no time penalties. The maximum time is an
  elimination the timekeeper decides, never the office: no automatic retirement.
- In the app: *Détails…* of a class on that table asks for the ideal time (75, 74.5 or 1:15);
  until it is set the class is not ranked ("temps idéal à saisir"); start lists and rankings
  print it. Read from and written to Rosson's `idealzeit` (`scoring::jumping`, case file
  `ideal-time.md`; `office::prepare::set_details`).
- Not this table: Rosson's `idealzeit_berechnen` and `max_zeit_berechnen` (the gap, or 0.4
  penalties per second over the maximum time, counted as penalties) belong to combined
  classes' jumping and cross-country phases (eventing): kept as class options and to be built
  with combined classes.

### S21 · Style judges: a role of their own — *decided: change*

- Rosson prints the style sheets for the class's officials with Swiss Equestrian's role "R"
  (`Richter`, the jumping judge), and sends them to Swiss Equestrian as such.
- Rule (P. Burkhard, 2026-11-14): style judges are not jumping judges. They get a role of their
  own, which is never sent to Swiss Equestrian.
- In the app: the role *juge de style · Stilrichter* (`show::model::STYLE_JUDGE`, code
  `eq-style`; EquiEvents' own roles start with `eq-`), key **S** in the officials table (not in
  dressage classes). The style sheets are for the class's style judges; start lists and
  rankings name them. Never in `putVeranstaltung`, `putResultate` or `lookupZuweisung`
  (`Assignment::for_swiss_equestrian`), and not in the Rosson-format export (Rosson would send
  them; the `.equievents` export keeps them).

### S22 · Freestyle, Children and totals per judge — *from Rosson's code*

- Freestyle (Kür, programme `kuer`): a movement's part is its table's German title, "(A)" or
  "(B)"; a judge's total is half the points, unless the judge gave the total
  (`pruefungsdurchfuehrung.service.js` 125–162, 380–404). Each judge's parts are halved and
  written to a tenth, half up, before anything is added: the class's part A is the judges'
  written parts over the part's maximum × judges / 2. Sent: `punkte_prozent_a` / `_b`, and
  per judge `prozent_a` / `_b` (`upload.service.js` 238–248, 432–437).
- Children (programme `children`): one sheet for the judges ("die drei Richter zusammen ergeben
  ein Resultat"): the office enters the first judge's; the percentage is over the programme's
  maximum, not × judges. A movement's part is its table's `teilnote` ("A", "B"). Sent as two
  `pro_richter` rows, `C (A)` and `E (B)`, with the parts' points and percentages
  (`upload.service.js` 225–242), no `punkte_prozent_a` / `_b`.
- Ours where Rosson's differs: `punkte_total` of a Children result is its one total (Rosson
  copies the sheet to every judge and adds the copies). A judge who gave a total has no parts:
  none sent (Rosson sends "0.00").
- Totals per judge (Rosson's `manuelle_eingabe`): *Détails…* of a dressage class, before any
  sheet. The judge's total is entered, up to the programme's maximum; marks are refused; the
  penalties still count (`office::prepare::edit_sheet`).
- Freestyle ties — *decided: change* (P. Burkhard, 2026-11-14: "follow the SE rule, it's
  correct"). Swiss Equestrian's Kür tests say "Im Falle von Punktgleichheit im Endresultat
  gewinnt der Teilnehmer mit dem höheren Resultat für die künstlerische Gestaltung" (GAK, LK,
  MK, INTER I K, GEORGK, GPK, `texte.resultat`; the para tests PD IV K and PD V K have no text);
  Rosson ignores it (`rangliste.service.js` 187–235: ties broken only for OKV team dressage, by
  judge E). Ours: equal percentages in a freestyle are ranked by the artistic part (B), every
  freestyle, every rank; equal again, S3/S4. A judge who gave a total has no parts: no
  tie-break (`scoring::dressage::Score::tie_break`).

### S23 · Team dressage — *from Rosson's code and the OKV's rules* (decided, P. Burkhard, 2026-11-14)

Team dressage is the OKV's (Ostschweizerischer Kavallerie- und Reitverband; Swiss Equestrian's
dressage rules do not have it): Reglement Mannschaftsdressurprüfungen 2026 — teams of three
riding one test together (GA 02/60 in qualifiers, GA 04/60 in the final), five judges, H, C, M
one rider each for 50 %, the side judges E and B the team for 50 %. We follow the OKV's rules.

- One sheet per team and judge, never per rider (`resultat-form.controller.js` 60–84). The
  team's percentage (`pruefungsdurchfuehrung.service.js` 279–450):
  - as an entry's, by default;
  - five judges 3/2 (Rosson's `mannschaft`, its checkbox "Mannschaftsdressur (5 Richter
    Wertung 3/2)"): each judge's percentage over 10 × the movements' coefficients (freestyle:
    the programme's maximum), Σ % (H, C, M) / 6 + Σ % (E, B) / 4, the divisors fixed;
  - OKV (the test's `okv_mannschaftsdressur`): the mean of H, C, M's and the mean of E, B's,
    averaged (the same as 3/2 with every sheet).
  - Ties (3/2 and OKV): "die Equipe mit der höheren Note der beiden Seitenrichter" — E and B
    together. Rosson: judge E alone, and nothing in 3/2.
- Only teams are ranked; a team with a rider who does not start or rides hors concours is not
  (`rangliste.service.js` 77–100). Placed: 30 % of the teams. The riders are listed under their
  team without a rank; 3/2, each with their judge's total.
- 3/2: H, C and M judge the team's first, second and third rider; the judges are listed
  H C M E B. Rosson takes "first, second, third" from three lists (the live order for the
  sheets, a copy kept with the result for the ranking and upload): ours is one, the team's order
  in the start list.
- Sent (`upload.service.js` 214–293, 402–404): the team's row with `pro_richter` (3/2: E and B
  only), `punkte_total`, `punkte_prozent`; each rider's row without rank or points, 3/2 with
  their judge's `pro_richter`.
- Judges' sheets printed per team: 3/2, H, C and M with their rider and horse and the team's
  number; otherwise "Équipe : name" (Rosson prints a sheet per rider for a plain team class,
  which does not match its entry by team: ours matches).
- 3/2: the side judges' sheets on the OKV team test ("OKV Mannschaftsdressur GA 02/60": the same
  movements, the team's directive ideas), H, C and M's on the test itself, whichever the class
  has (in Rosson the office changed the class's test between the two prints, as the OKV's
  Veranstalterdossier explains). Its French title at Swiss Equestrian is the riders' test's: the
  German one is printed.
- A team out (retired, eliminated) entered on its row in the entry screen, as a rider's.
- Not built, by decision: a team's own test (Rosson's `programm_id` on a team): every team
  rides the class's test (OKV).

### S24 · Eventing (Concours Complet) — *from Rosson's code*

Rosson's eventing as read from its code: `docs/rosson/eventing.md`. Ours:
- A CC class set up becomes a class made of phases (its overall ranking, no starters of its own)
  and a class per phase, `<class>_cd`, `_cs`, `_cross`, as Swiss Equestrian lists them
  (`kombipruefung_disziplinen`), in Rosson's order (dressage, cross, jumping), every entry in
  each in the same drawn order. A phase is entered, printed and sent like a class of its kind
  (cross: penalties and a time "5:12" against the ideal time). A rider has one start number in
  every phase.
- Cross: 0.4 per second started over the ideal time when the phase counts time (Rosson's
  `idealzeit_berechnen`), nothing for being early; ranked by points, then the gap to the ideal
  time, then the time. Jumping phase: 0.4 per second started over its maximum time when it counts
  it (`max_zeit_berechnen`), added to the first round. A jumping phase on the ideal-time table
  (35) is scored as that table in show jumping (S20): penalties, then the gap to the ideal time,
  no time penalties; in the overall ranking that gap breaks a tie in the phase, as the cross's
  does. Rosson added 1 per second off the ideal time, early or late: not in the eventing rules
  (EVR 2027, 8.2: "Wertung A mit Zeitmessung", 0.4 per second started over the allowed time), a
  rule of the old jumping regulations, so not copied (P. Burkhard, 2026-11-14).
- Overall: dressage 100 − %, jumping and cross their penalties (or each phase's rank); lowest
  total first; ties by the phases in their order (drag and drop), the lower value then the smaller
  gap; out of a phase, out of the class; a phase not ridden adds nothing.
- Sent: a start list per phase ("12 Dressur"); the overall as `putResultate` with Rosson's CC
  fields (`punkte_total`, `punkte_dressur`, `strafpunkte_springen_hindernis`,
  `strafpunkte_cross_hindernis`); a phase as `putZwischenresultate`. Ours where Rosson's differs:
  each phase's penalties are sent also when the overall counts ranks (Rosson sends zeros).
- The Rosson file: phases in `zwischenpruefungen`, their starters and results naming the class
  and the phase, a dressage phase's judges on the class (as Rosson keeps them).
- Not divided into series (Rosson allows it; to come if needed).

### S25 · Driving (Fahren) — *from Rosson's code*

Rosson's driving as read from its code, with its bugs: `docs/rosson/driving.md`. Ours:
- A CA class set up becomes its phases (Swiss Equestrian's `fahren_zwischenpruefungen`), as
  eventing (S24), with Rosson's defaults. Judges at C R S P V.
- Dressage: each judge's total `160 − marks × 160 / (movements × 10) + penalty points`
  (coefficients 1), the judges' mean, lowest first. Marathon: start and finish typed as times of
  day; phases A and B against their minimum and allowed times, `factor` per second started (0.25),
  phase B's obstacles' time × factor and their penalties. Cones: the driver's licence (the
  class's first they hold; FB also as BRF) gives the allowed time per round, plus the turnout's
  bonus; scorings A (half a point per second started over; "international": half the overtime to
  the hundredth), B (the time), youth cup (±5 s window, the gap breaks ties). Derby: time and
  penalty seconds × the factor. Open: by time, points, penalty points, or points then time.
  Multitest: each judge's marks × Rosson's coefficients (B1, B4) less penalties A, B, C; the
  judges' mean, highest first.
- Overall: driving dressage counts its total; an open phase or a multitest makes the class rank
  by ranks (Rosson's `rangpunkte`; with a multitest Rosson tried `punkte`, half-built).
- Sent as Rosson: per phase `putZwischenresultate`; overall `strafpunkte_<phase>` or
  `rang_<phase>`, the total, `aufgabegrund_<phase>_id`; only horses starting in a phase.
- Rosson's bugs not copied: marathon times over an hour (hours × 360); a missing licence
  silently giving no time faults (ours: the phase waits for its allowed time); string
  concatenation of times; the overall's columns paired by index; multitest's unused "total A".
- Marathon: over a phase's time limit (A: the allowed time + 20 %, B: twice; or as set) or
  five minutes in an obstacle, eliminated, as the rules say ("Le dépassement du temps limite
  dans une phase entraîne l'élimination"; "Le temps limite pour chaque obstacle est de cinq
  minutes"). Rosson computed the limits (`max_zeit`) but did not apply them (P. Burkhard,
  2026-11-14).
- Cones: a round driven over its time limit eliminates, as in Rosson (reason 27) and the rules
  ("le temps limite est le double du temps accordé … Dépasser le temps limite entraîne
  l'élimination"): the limit per round and licence, twice the allowed time unless set; the
  time driven, without the turnout's bonus, as Rosson. Computed, so it follows a changed limit;
  the office sees "Éliminé : temps limite dépassé". (The maximum time as the timekeeper's
  elimination is jumping's, S20; P. Burkhard, 2026-11-14.)
- Checked against two real shows (golden corpus 2025.8126, Rosson 3.3.4, and 2026.8299, Rosson
  3.3.14, Tavannes: four classes of dressage and cones A each, 83 starters): every phase and
  overall rank, placing, each judge's dressage total, the cones points and times and the overall
  totals as Rosson stored them (249 of 249, and the overall as printed at the show). Two placing
  flags corrected in the golden test: Rosson ranks before it recounts the placed riders when a
  result is saved, so its flags use the count before the last result (bug 18 in
  `docs/rosson/driving.md`); ours follow its own rule, rank ≤ the placed count.
  Marathon, derby, open phases and multitest have no real show yet.
- Prints of driving (2026-11-14): start lists with each phase's horses, the F licences and
  turnout, the carriage width on the jury's list, the marathon's phase B start; the day's start
  lists in one document; the judges' sheet without correction or coefficient; control sheets —
  cones (each round's gates, the driver's allowed time filled in), derby (the round and the
  obstacle sheet), marathon (an obstacle sheet per obstacle and starter, the obstacle's number
  filled in; Rosson's had it blank). The penalty lists are the driving rules' as Rosson prints
  them; Swiss Equestrian has no Italian edition: German in Italian prints. Better French than
  Rosson's for the widths: « Largeur de la voiture », « Supplément », « Écartement des cônes »
  (Rosson: « Écartement » for the allowance, « Largeur » for the gates).
- Result lists per phase kind (Rosson's `fahren-rangliste`): dressage each judge's total with
  its rank and the judges' mean on 160 (before: the total column was empty, the list expected a
  percentage); marathon phases A and B, each obstacle, the total; cones each round's points and
  time, the time, the total; derby time, knock-downs, others, total; open seconds, points,
  total; multitest each judge's total and the mean.
- Teams in cones: the OKV Fahrcup's "Equipe Kegelfahren", on its rules (Reglement Fahrcup
  2026, 4.2–4.3; P. Burkhard, 2026-11-14: "follow the OKV rules"): every round of the team's
  drivers, the worst struck (most points, then the slower), the others' points added; a round
  a driver does not complete (over its time limit, or not driven by a driver out) credited
  125 % of the worst first round driven in the class; equal points for the win shared until the
  jump-off (ranks set by hand); further down the better struck round, then the time of the
  counted rounds; rank points 12, 10, 9 … 2, then 1. The class's ranking is its team cones'.
  Sent as Rosson: a row per team, then its drivers with their points without the struck round.
  Rosson's deviations from the rules not copied: all rounds' times (the rules: the counted
  ones), ties never seen, a team out with one driver out, the struck round written as a result.
  Not written yet: Rosson's own team records in its format (Rosson rebuilds them when a result
  is saved).
- Ranks set by hand (Rosson's `customRang`): typed on the ranking (a rider, or a team), after a
  jump-off or a jury decision; empty gives the computed rank back.
- Multitest entered as Rosson's form (its programmes B1 and B4, which the rulebook does not
  describe: its "Prüfung F: Multitest" is the licence exam): per judge the items 0 to 10 in
  tenths, penalties A (dressage) and C (ground test) whole, B (per obstacle) in halves; each
  judge's total once the sheet has every item; the judges' mean, the most first.
- Horses × phases (`einsaetze`): edited in an entry's tools (a horse × phase grid); start lists
  and phase results sent with only the phase's horses, as Rosson (`getPferdeMitEinsatz`); a
  driving start list's line says the F licences and the turnout ("FB / 2 Sp"); the phases'
  lists named as Rosson's (Gelände, Kegelfahren, Derby, Multitest; the open phase as the
  rulebook: Spezialprüfung).

### S26 · Zeitspringen: good points, then time — *decided: follow the Springreglement* (P. Burkhard, 2026-11-17)

- Table 64 (ZEIT) as published ranks on time only. The Springreglement (4.3.4) ranks on good
  points, the most first, then on time. EquiEvents defines the table itself
  (`scoring::jumping::table::definition_text`, `regulation_override`): `strafpunkte_initial_1`
  holds the good points, then `zeit_initial_1`. A rider without a time is last of equal points
  (S32). Case: `special-classes.md`.

### S27 · Puissance: the normal round is timed — *decided: follow the Springreglement* (P. Burkhard, 2026-11-17)

- Table 61 (PUIS) as published ranks the normal round on faults only. The Springreglement
  (4.1.2) judges it under Wertung A with time: EquiEvents adds `zeit_initial_1` to it. The
  jump-offs (up to four) stay without time; riders still clear after the last one share first
  place (S29).

### S28 · Wertung C with a jump-off under A: the jump-off's own time — *decided: follow the Springreglement* (P. Burkhard, 2026-11-17)

- Table 41 (C1AZ) as published separates jump-off riders equal on faults by their round-1 time.
  A jump-off is judged as the Springreglement says for it (2.3.2): by its own time,
  `zeit_stechen_1`.

### S29 · Several jump-offs: out of a later one keeps the earlier ones — *decided: follow the Springreglement* (P. Burkhard, 2026-11-17)

- Rule: a rider withdrawn, retired or eliminated in jump-off *n* ranks equal last of jump-off
  *n*, ahead of the riders who stopped at an earlier jump-off, and keeps the jump-offs completed
  before (RL Prüfungen 2.3.7, 2.3.8). Riders still equal after the last jump-off share first
  place.
- Before: a score card has one out reason for all its rounds, so a rider out of jump-off 3 lost
  every jump-off result and fell to equal last of jump-off 1. Now the jump-off the rider is out
  of is the first one without a result.

### S30 · A jump-off everyone left, a jump-off nobody rode — *decided* (P. Burkhard, 2026-11-17)

- Every rider of a jump-off out of it (withdrawn, retired, eliminated): nobody wins it — a rider
  who gives it up cannot be the winner (RL Prüfungen 2.3.9). They are all equal last of it: five
  riders, all 5th.
- Nobody of the jump-off has a result and nobody is out (it was cancelled): all equal first.
- Some rode it: those out of it are equal last of it, after those who rode it. A rider with no
  result there and not entered as out (not ridden yet) comes between them — the provisional
  list while the jump-off runs.

### S31 · Out riders: equal last of the class, or of their jump-off — *decided: change* (P. Burkhard, 2026-11-17)

- Eliminated or retired without a result in round 1: **out**, equal last of the whole class.
- With a result in round 1 and out after it: the rider was in the jump-off — **equal last of the
  jump-off** (S8, S9). That includes a group that rode a jump-off it was not entitled to by the
  class's options (a championship's medal jump-off in a class from Rosson, which never has the
  option: 2026.8352 P8). A rider with a round-1 result out but not in a jump-off: out. Second
  rounds: S16.
- Rosson ranked out riders with a result by that result, after the finishers. No corpus result
  changes.

### S32 · Points and time: a missing time — *decided: follow the Springreglement* (P. Burkhard, 2026-11-17)

- In a points-based part (Zeitspringen, Punktespringen, …) a rider who does not finish is last of
  those with the same points (Springreglement 4.3.3, 4.4.4). The part no longer needs its time to
  be entered (before: a time of 0 had to be typed); a missing time is the slowest.

### S33 · Times to the hundredth — *decided* (P. Burkhard, 2026-11-17)

- Times count to the hundredth (Springreglement 7.9): finer digits from a timing file are cut,
  not rounded (61.239 → 61.23), before anything is compared or added.

### S34 · Reduced second round: only riders of both rounds are placed — *decided: follow the Springreglement* (P. Burkhard, 2026-11-17)

- Tables with a reduced second round (`2RU`: 12, 21, 23, 24, 27): only riders who completed both
  rounds are classified (Springreglement 3.4.2.1). Riders who rode round 1 only, and riders equal
  last of round 2 (S16), keep their rank but are never placed, even within the placed count
  (`Ranked::placeable`, `show::ranking::Line::placeable`).
- If nobody rode round 2 (cancelled), round 1 places as usual, like a cancelled jump-off (S30):
  2025.8161 P10, where Rosson placed four on round 1, unchanged.

### S35 · Knock-out — *decided: follow the Springreglement* (P. Burkhard, 2026-11-17)

- Table 55 (KNOCK) as published reads like a Puissance (S12). The Springreglement (4.5): duels,
  under Wertung C. Each round's field (`strafpunkte_initial_1` for round 1,
  `strafpunkte_stechen_1` for round 2, …) holds the duel's result: the time with the penalty
  seconds, as the timekeeper gives it. The rider who reached the furthest round ranks first; in
  the furthest round the faster result wins; the losers of the same round are equal, eliminated
  or not. Placed: eight — 1st, 2nd, two 3rd, four 5th (4.5 (6)) — unless the organiser sets
  another number. A tied duel is ridden again (4.5 (4)): the result entered is the repeat's.

### S36 · Teams: every result counts — *decided, new option* (P. Burkhard, 2026-11-17)

- The Ausschreibung defines how a team's results count (Springreglement 4.6). By default the
  Nations Cup formula (S7): a team of four drops its worst result per round. A class option
  (*Détails…* of a team jumping class: « Équipes : tous les résultats comptent ») counts every
  rider's result instead; then one rider out puts the team out.

### S37 · Table 24: the winning round's own faults count — *decided: follow the Springreglement* (P. Burkhard, 2026-11-17)

- Table 24 (AZ2RUSAZ2) as published adds the faults of rounds 1 and 2 in the winning round but
  not the winning round's own (E20), and Rosson ranks it so: it adds the faults each part of the
  definition lists (`rangliste.service.js` 290–293). The Springreglement (3.1.4): the winning
  round is a jump-off, the normal round's faults carried. EquiEvents defines the table itself:
  round 1 + round 2 + winning round faults, then the winning round's time — as table 29 does
  for one round. The winning round needs its faults entered (0 when clear), like table 29. No
  corpus class uses table 24.

### S38 · Tag der Jugend finals: no ex aequo in ranks 1–3 — *decided, new option* (P. Burkhard, 2026-11-17)

- Regional and Swiss finals of the Tag der Jugend (RL Tag der Jugend 10.3): entries tied in
  ranks 1–3 are separated by the sum of judges C and H's marks for given movements, in order,
  then by drawing lots. Führzügelklasse 2, 3; Einfacher Reitwettbewerb 6, 3; Einsteigerdressur
  15, 17; Dressuraufgabe 21, 24 (`show::master::youth_final_movements`).
- A class option (*Détails…* of a class with one of these programmes: « Finale Journée de la
  jeunesse »). Entries still tied are marked « tirage au sort » on the ranking: the office draws
  lots and sets their ranks by hand (S18). Ties further down stay.
- The Dressuraufgabe TdJ that Swiss Equestrian delivers (2018 edition) has 23 movements: the
  regulation's movement 24 separates nobody, lots come after movement 21. *To check with Swiss
  Equestrian.*

### S39 · Young horses' programmes: tenths, one sheet for the jury — *decided* (P. Burkhard, 2026-11-17)

- Every programme is marked in halves; the JP programmes (young horses) in tenths
  (Dressurreglement 8.2 (2)). Rosson takes any mark from 0 to 10 in hundredths; the office's
  sheet entry now takes tenths in JP programmes (`show::master::in_tenths`), halves elsewhere.
  The tablet import takes any mark from 0 to 10, as Rosson.
- JP programmes are judged jointly (8.1): the jury's one sheet, entered as the first judge's,
  counts for every judge of the class (`show::master::judged_jointly`), as Rosson does for
  Children. The percentage is the jury's; Swiss Equestrian gets the same row for each judge.
  Before (and in Rosson): a judge without a sheet counted 0.

### S40 · Penalties: one set for the rider; the exclusion box eliminates — *decided: change* (P. Burkhard, 2026-11-17)

- Penalties (errors of course…) are ticked once for the rider: the same on every judge's sheet
  (`set_penalties`). Rosson kept a set per judge, which could differ. The tablet import makes
  one set from the judges' rows: each penalty as often as the judge who ticked it most.
- The exclusion box (the programme's penalty without a value, "Ausschluss": more errors than the
  programme allows) switches the rider to eliminated, and back when unticked; ticked on a
  tablet, the rider is imported eliminated (Dressurreglement 8.3 (4)). Rosson ignored it: no
  value, the rider stayed ranked.
- Penalties ticked before a judge's marks are kept on that judge's sheet but do not count as the
  judge having marked.
- Shows from Rosson keep their sheets as they were (their results were published): in the corpus
  one rider (2026.8298 P2) has a penalty from one judge only; the office's next tick makes it
  one set.

### S41 · Eventing: the overall ranking by the Eventing-Reglement — *decided: change* (P. Burkhard, 2026-11-17)

- CCN (dressage, jumping, cross; 5.9.2): penalties added; equal, the cross result (obstacles,
  time and other penalties), then the time closer to the cross's ideal time, then dressage,
  then the faster jumping time; still equal, the rank is shared. Before: the phases in their
  order, dressage first by default; the jumping time never compared.
- Combined classes (5.9.1), always, whatever the class option: dressage + jumping by rank
  points, equal → the better dressage; jumping + cross by penalties, equal → the better cross;
  dressage + cross by penalties, equal → the cross, then dressage. The rank-points option is not
  offered for eventing (driving keeps it).
- Dressage penalties: 100 − the average of each judge's percentage, each to 2 decimals (8.1.2's
  example); before, the judges' points together over the maximum — the last digit could differ.
  The dressage phase's own percentage is the same average.
- The cross: over the maximum time, twice the ideal time, eliminated (8.3.5), when the ideal time
  is set. Time penalties over the ideal time (8.3.5) and over the jumping's maximum time (8.2.2)
  stay class options, off by default, as in Rosson (`idealzeit_berechnen`,
  `max_zeit_berechnen`): timekeepers usually include them.
- A phase closed (final) without a result for a rider: out of the overall ranking; while the
  phase runs, it adds nothing (the provisional ranking).
- The jury's penalties (25 for a yellow card, dangerous riding, mistreatment; 2.2, 8.4, 8.5)
  in dressage and jumping: set by the office on the overall ranking (a rider's dressage or
  jumping cell), added to that phase's penalties and sent in its number to Swiss Equestrian
  (`punkte_dressur`, `strafpunkte_springen_hindernis`); the cross carries them in its own points
  (P. Burkhard, 2026-11-17). Rosson has no such field: not in its export. In a combined
  dressage + jumping class (rank points) they count in the dressage tie-break only.
- Left to the timekeeper and the jury: cross obstacle penalties and eliminations (8.3.6), time
  held on the course (8.3.8). Not covered: cross style classes and indoor cross (special
  classes, for later).
- No eventing class in the corpus (its classes of phases are driving): the rules are checked by
  `scoring::eventing` tests and `crates/office/tests/eventing.rs`.

### S42 · Driving: the overall ranking by the Fahrreglement — *decided: change* (P. Burkhard, 2026-11-17)

- Full event (Vollprüfung: dressage, marathon, cones; 13.10 §7.1): penalties added; equal, the
  marathon, then dressage. Eliminated or not finished in a test is not out of the event: that
  test counts the highest penalties there + 25 %, and the driver ranks after everyone who
  finished every test (§7.1.2). Before: out of the overall ranking.
- Derby events (dressage + derby, RVP; with cones, DVP; §7.2): equal, the derby, then dressage.
- Short events (any other combination; §7.3): equal, dressage, then the driven dressage movements
  (the judges' mean of the marks without the collective marks, "Gesamteindruck"), the most
  first; still equal, shared. Before: the phases in their order. Corpus: 2025.8126 P2 and P4
  (dressage + cones), two drivers equal in dressage and cones whom Rosson ranked equal.
- Full and derby events rank by penalties, by the rules; the rank-points option stays for short
  events.
- A full event's cones: Wertung A, the overtime to the hundredth × 0.5, always (8.1.1, 8.4.4
  (4)); the class's own setting does not apply there (*Détails…* shows it locked).
- Cones in Wertung B (P. Burkhard, 2026-11-18): the faults are seconds, added to the time
  (8.4.6, 8.4.7: 5 s a ball where Wertung A counts 3 points); the fields say « (s) » and the
  entry recalls it. No penalty for overtime in Wertung B; over the maximum time, eliminated.
  Before (and in Rosson): the same fields, no unit.
- Left to the jury: the marathon's transfer and the derby's maximum time (7.3.2, 7.9, 9.1, 9.2).

### S43 · Driving dressage: programme errors off before the conversion — *decided: change* (P. Burkhard, 2026-11-18)

- The Fahrreglement takes programme errors and incidents off the marks on every judge's sheet,
  then converts tests of more than 160 points to the 160 basis (13.10 §3). A judge's penalties:
  160 − (points − penalties) × 160 / maximum. Rosson added the penalties after the conversion
  (`160 − points × 160 / max + penalties`): in a 200-point test 5 penalty points counted 5, now
  4. On 160-point tests nothing changes; no corpus result changes.
- The factor stays Rosson's, 160 / maximum (P. Burkhard, 2026-11-18). The rules give 0.8 for
  200 points (the same) and 0.55 for 300, nothing in between; no test in Swiss Equestrian's list
  has 200 or 300 points (150, 160, 210, 240, 250, 260; the FEI Juniors test is published with
  370 for 22 marks, counted as 220). The rounding stays Rosson's too: each judge, then the mean
  (the rules: the mean of the points).

### S44 · Driving: the presentation as a phase of its own — *decided, new* (P. Burkhard, 2026-11-18)

- The presentation (Prüfung A I) judged standing, as at level L (Fahrreglement 6.2.1): a phase
  the office adds to a driving class made of phases (« Ajouter la présentation » on the overall
  ranking), first, its riders in the first phase's order. Each judge's overall mark, 0 to 10 in
  halves (up to three judges); the penalties: 10 − their mean, to the hundredth (13.10 §2);
  added to the overall total. Swiss Equestrian's list of phases names it `praesentation`
  (assumed: no corpus class has it).
- Rosson has no presentation: its file leaves the phase and its values out.
- Sent to Swiss Equestrian as a phase like the others (`praesentation`): *whether Swiss
  Equestrian takes it is to be seen at the pilot (April 2027).*

## E — Engineering and data (fixed by design)

| # | Rosson | New app |
|---|---|---|
| E1 | Ids of mixed types: `pruefung_id: "108196"` (string) next to `ausschreibung_pruefung_id: 108196` (number). | Typed ids, one type per entity. |
| E2 | The full entry (with address, phone, birth date) is copied into results, teams, Prüfungen and the log; copies go stale. | Normalised model, references only. |
| E3 | Computed values are stored next to the inputs and not cleared when inputs change (S6). | Inputs are the source of truth; derived values are recomputed deterministically and carry their provenance. |
| E4 | Rankings and team results are written as a side effect of *viewing* a ranking. | Pure scoring functions; recomputation is explicit. |
| E5 | Scores as floats and strings (`toFixed`, `"64.76"`). | Exact decimals; rounding rules written down per table. |
| E6 | A database write that fails inside a `.then` never settles: endless spinner, the failure is invisible. | Every write reports success or a visible error. |
| E7 | Every query is a full-table scan in a read-write transaction, filtered row by row. | Indexed SQLite queries. |
| E8 | The build ships the print web-worker scripts and a debug helper inside the main bundle (gulp ignores exclusion globs listed before the includes). The worker's `onmessage` runs in the main window and throws on every window message. | Build output is checked; workers are separate entry points. |
| E9 | `StorageFactory` exports a `find` function that does not exist; it works only because Chromium has `window.find()`. | Lint/type checks catch undefined names. |
| E10 | Misleading names, e.g. a variable `accessKey` that holds the event number. | Names say what they hold. |
| E11 | `nodeIntegration` on, `contextIsolation` off, Chromium 100; admin password hard-coded in the client; a pseudo horse (id 5) injected into horse data. The date-based `rosson_key` is kept: it is the API contract. | Tauri with a minimal command surface; admin rights checked server-side; no fake records. |
| E12 | Birth dates stored on every entry copy although they are only printed. | Data minimisation: store only what a feature needs. |
| E13 | Exports are URL-encoded JSON behind a version line, without a schema; the format drifted across 3.3.x versions (e.g. `stufe` copied onto results since some 3.3.x). | One show per file, versioned, checked on reading (`show::export`); importer for Rosson exports from 3.3.0 on (`rosson-import`). |
| E14 | Duplicate result rows can occur (`doppelteLoeschen` exists to remove them). | Uniqueness enforced: one result per starter per round. |
| E15 | Two database calls before initialisation open two connections. | Single storage layer with one connection pool. |
| E16 | Copy-paste bug in the ideal-time tie check (`resultat.idealzeit && resultat.idealzeit`). | Covered by tests. |
| E17 | Dressage judges are matched on the exact role code `DRI`; Tag der Jugend judges (`DRITdj`) are not counted. | Judges of a class are an explicit list. |
| E18 | Withdrawing a whole team leaves its result row behind, still ranked (S15). | Results belong to entries; withdrawing an entry withdraws its results. |
| E19 | Master data: table 25's second jump-off needs `zeit_stechen_2`, which is not among its visible fields, so it cannot be entered. | Report to Swiss Equestrian master data; definitions are validated on import. |
| E20 | Master data: table 24's winning round adds the faults of rounds 1 and 2 but not its own faults (`strafpunkte_stechen_1` is neither required nor counted). | Defined by EquiEvents, its faults counted (S37); report to Swiss Equestrian master data. |
| E21 | Timekeeping import reads text files as raw bytes without decoding Windows-1252: a guest horse named e.g. "Étoile d’or" (curly apostrophe) can never be matched. | Files are decoded (UTF-8 or Windows-1252). |
| E22 | Timekeeping import matches a row by rider and horse only and takes the first entry: when the same pair starts twice in a class (relay teams), results land on the wrong entry — in 2025.8161 P8 on a withdrawn entry. | The start number decides; still ambiguous → reported. |
| E23 | Timekeeping import silently skips rows with an unknown code in Aufgabe, imports results for entries that do not start, and writes `hc` / `Nichtstarter` from the file onto the entry. | Unknown codes and not-starting entries are reported; entry changes are the office's decision. |
| E24 | Two-phase classes are recognised by the word "Phase" in the table's German name; the Excel import checks the faults column to decide where the phase-2 time comes from. | Recognised by table code; each field from its own column. |
| E25 | The `ist_phase` flag only exists in the 2026 master data, so it cannot be relied on for older shows. | Two-phase tables recognised by their codes. |
| E26 | Dressage tablet import ignores the `judge` column and cuts the rows into blocks of as many rows as the class has judges: one missing or extra row shifts every following rider's marks to the wrong judge or rider. | Each row goes to the judge at its position (`timing::tablet`; test `a_missing_row_does_not_shift_the_following_marks`). |
| E27 | Tablet import carries the last status it saw over to the following riders: a rider without status after an eliminated one is imported as eliminated. A rider with one empty judge row is skipped entirely. | Status per rider from its own rows; marks without status are reported; empty rows leave that judge's sheet as it was. |
| E28 | Tablet import of text files splits on CR *and* LF, then drops every second line: only Windows line ends (CRLF) work; with LF half the riders are lost. | Any line ends (shared reader with the timekeeping files). |
| E29 | Tablet import takes marks by cell position instead of the movement number in the column name (works only while programmes number 1, 2, 3, …), ticks every penalty *with the same value* as the one given, and reads penalty values as whole numbers. In 2026.8298 P9 ("TdJ Einste": three errors of 2 points) one error becomes three: 6 points off instead of 2. | Marks by movement number; each penalty by its own column, checked against its value; anything that does not fit is reported. |
| E30 | Tablet import stores a judge total computed by a second formula: for freestyle `(points − points × % / 100) / 2`, unrounded and without point penalties, where the results screen computes `points − round(points × % / 100, 0.1) − penalty points`. A freestyle imported from tablets is ranked on halved totals until every rider is saved again on the results screen. | Totals are never stored; one engine computes them (E3). |
| E31 | Tablet export: the penalty columns come from whatever programme the results screen opened last, not from the class's; in classes where every rider chooses the programme, penalty cells beyond a rider's programme are not marked `-` (a shadowed loop variable); in judges'-totals classes every row is one cell longer than the header; Children classes overwrite the first judge's position in memory with the joined positions ("E / C"). | Columns from the class's (or rider's) programme; no shared mutable state. |
| E32 | A rank set by hand for a rider who is out is not skipped by the others: Rosson collects the numbers set by hand only among ranked riders, so the class gets two winners (2025.8161 P5, P6 and 2026.8352 P5 when a retired rider is sent as 1st). | One rule for all riders (S18). |
| E33 | Results upload, dressage: each judge's rank (`pro_richter.rang`) is set inside a sort comparator; with a single rider the sort never runs and the rank is not sent (2026.8298 P10). | **Kept in the upload** (the API is frozen, decision 2026-11-13); our own rankings per judge are complete. |
| E34 | Results upload: the same field goes out as text or as a number, and "nothing" as `0`, `""` or no field, depending on how a value was entered (162 values in the corpus). | One form, as Rosson's timekeeper imports write it (most of its uploads): entered values as text, empty fields left out, reasons as numbers, `0` when the rider finished (jumping; dressage: no field). Judges in score-sheet order (Rosson: the order marks were typed in, 44 of 245 dressage riders). *Decided 2026-11-13 (P. Burkhard).* |
| E35 | Results upload, dressage: the percentage per judge is computed in floating point, so a value exactly between two hundredths can round down (60.625 sent as "60.62"; 2026.8298 P9, two riders). | Exact (60.63), rounded half up like every result (decided 2026-11-13). |
| E36 | Start list upload, team classes: when riders of a team that is no longer in the class are still starters (withdrawn teams, the ghost team of S15), Rosson's sort becomes inconsistent and scrambles the published start list: riders listed under another team (2025.8158 P9), a team heading lost (2025.8161 P8), the whole list in near rotation order (2026.8352 P7). | Team by team in the class's team order; riders of teams no longer in the class at the end. |
| E37 | Start list: the line "after the n-th starter" (arena maintenance, a section title) is put after the n-th *line*, block headings included, so with blocks it lands too early; riders without a level get a heading "Stufe null". | After the n-th starter; no heading without a level (`show::startlist`). Not used together in the corpus: no difference there. |
| E38 | After every results upload Rosson asks Swiss Equestrian for the id of every official of the event (91 requests for 2026.8352, after each of 19 classes), also for those it asked before without getting an id. | Each assignment is looked up once, and again only when it changes (decision 2026-11-13). |
| E39 | Updates from Swiss Equestrian (`checkForUpdates`): when an entry changed there, Rosson overwrites its rider, horses, status, team, fee and winning points with Swiss Equestrian's values, so an unrelated change (a fee paid) undoes a change of horse made at the show. It ignores changes of hors concours, level and chosen programme. An entry confirmed again after a withdrawal stays "does not start" for good. | Merged field by field against the last delivery (`show::delivered`): what Swiss Equestrian changed is taken, the office's other changes stay; if both changed the same thing, Swiss Equestrian's later change wins and the office is told. Without changes by the office the result is Rosson's apart from those three (`crates/se-api/tests/load_contract.rs`). Local behaviour, by the "do it right" rule; *to confirm (P. Burkhard)*. |
| E40 | Updates: of a published class Rosson takes only the category, date, team size and driving licences (a new scoring text or number of levels is ignored). A class Swiss Equestrian adds after the first load is stored without its event and is never shown (all 6 corpus events: `golden/corpus/<show>/api/update.json`, `classes_shown`). | A published class is Swiss Equestrian's: an update replaces it whole, and new classes appear. |
| E41 | A change of horse at the show updates the horses written on the entry (`pferde`) but not `pferd_id`: the entry names two horses (8 entries in the corpus, e.g. 2026.8352 entry 3113901). The timekeeping files use the first. | One current horse per entry. |
| E42 | Talking to Swiss Equestrian: a request answered 202 is asked again every 4 s until the app is closed; nothing is tried again after a network error; a failed results upload shows only the HTTP status text, never Swiss Equestrian's message; a failed `putVeranstaltung` after an upload is swallowed; the first failed official lookup stops the others. | Same requests, same order, same URL encoding and headers (checked against Rosson's, `golden/corpus/http-*.json`). Polling stops after 10 minutes with a clear message; reads are tried again 3 times; writes are not retried automatically; errors carry Swiss Equestrian's message; a failed follow-up is reported and the others still sent; every accepted upload is recorded in the show's log, with who and when (`se_api::http`, `se_api::actions`). |
| E43 | Prizes: the default minimum prize is (entry fee − 10) × 2 (`preisgeld.controller.js`): Swiss Equestrian's tax per entry is long no longer 10 CHF; the scale is computed in floating point. | (Entry fee − 18) × 2, the tax being 18 CHF (P. Burkhard, 2026-11-13; one constant, `office::results::ENTRY_TAX`); in decimals. The scale and its minimum in show jumping only (P. Burkhard, 2026-11-19): in the other disciplines Rosson offers them too, but they are a jumping rule; there the prizes are typed rank by rank. |
| E44 | A guest horse's name is shown as typed, by the office or by the rider entering on Swiss Equestrian, usually not in capitals as every registry horse's. | In capitals wherever it is shown: the app's lists, start lists, prints, exports for people, the timekeeping and dressage tablet files (read back whatever the case), the history (P. Burkhard, 2026-11-21; `show::model::guest_horse_name`). The name entered stays as it was in the show and goes back so to Rosson and Swiss Equestrian; a guest horse typed in two cases counts as one horse in a ranking over several classes. |
