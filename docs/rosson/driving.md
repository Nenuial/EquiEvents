# Rosson 3.3.15: driving (Fahren, attelage, CA)

What Rosson does, read from its code (paths under `vendor/rosson-3.3.15/src/renderer/app/`;
PD = `pruefungsdurchfuehrung/pruefungsdurchfuehrung.service.js`, RL =
`pruefungsdurchfuehrung/rangliste/rangliste.service.js`, RS = `services/resultat.service.js`, ZP =
`services/zwischenpruefungen.service.js`), 2026-11-14. Driving classes are made of phases like
eventing (`docs/rosson/eventing.md`); here what is specific. Our decisions: `docs/deviations.md`.

## Recognition

- Class `disziplin_code 'CA'`; phases by code: dressage `CD`, marathon (`gelaende`) `GE`, cones
  (`hindernis`) `HI`, derby `DE`, open (`offene`) `OF`, multitest `MT` (`app.module.js:18–29`).
  Elimination reason id 27.
- Swiss Equestrian's class: `fahren_zwischenpruefungen` (phase names: dressur, gelaende,
  hindernis…), `fahren_gespanne` (e.g. ['2','4']), `fahren_lizenzen` (FB, FL, FM, FS),
  `fahren_nur_pony`.
- Parent: `fahren = true`, `customBuchstaben 'CRSPV'` (judge letters; or CBEHM),
  `wertung_nach` = `rangpunkte` with an open phase, `punkte` with multitest, else `strafpunkte`.
- Phase `sorter` and defaults (ZP:270–306): dressage 0; marathon 3 (`phase_b = {anzahl_hindernisse
  5, faktor 0.25}`); cones 4 (scoring A, `wertung_id` 1); derby 5 (`zeitfaktor 0.25`); open 6;
  multitest 7 (`multitest_typ 'B4'`).

## Entries

- `fahren_gespann` ('1', '2', '4'; printed "2-Sp"), `wagenbreite` (cm, text), `lizenzen`
  ('FB,FL': the F licences), the start number as carriage number, up to 6 horses (`pferd_id …
  pferd6_id`), each with `einsaetze` (the phases it starts in; default all). Guest horse
  `pferd_id 5`. UI: a horse × phase checkbox matrix (`components/fahren-pferde-verwalten`).

## Scoring per phase

- Dressage (PD:279–342): judges `DRI` of the parent, letters C R S P V; coefficients forced to 1;
  `maxPunkte = fields × 10`; per judge `total = round2(160 − punkte × 160 / maxPunkte +
  strafpunkte)` (penalty points only, from the ticked penalty tables); `gesamttotal =
  round2(mean of the judges)`. Lower is better. Out: rank 99.
- Marathon (PD:452–534): phases A and B: start and finish times, penalty points, neutralisation
  (s); phase B obstacles: time, points, neutralisation. Obstacles: `Σtime × faktor` (0.25) +
  `Σpoints`. Each phase: `t = finish − start − neutralisation` (B also minus obstacles'
  neutralisation); under `min_zeit`: `ceil(min − t) × faktor`; over `erlaubte_zeit`:
  `ceil(t − allowed) × faktor`. `max_zeit` is not scored. Settings: from distance and speed;
  A: min = allowed − 2 min, max = allowed × 1.2; B: min = allowed − 3 min, max = allowed × 2.
- Cones (PD:559–646, scorings `wertungen.service.js:33–68`): A (1 round, id 1), A2U (id 2), B (id
  5), B2U (id 6), JC2U (id 7, ±5 s). Per round: time, obstacle faults, other faults (points as
  entered). Allowed time per licence (`erlaubte_zeit_<LIZ>`; Einlaufprüfung: + bonus per
  turnout). Time faults: A `ceil(over) / 2` (international: `round2(over × 0.5)`); B: the driven
  time is added; JC: a ±5 s window around the allowed time, outside `ceil(distance) / 2`.
  Over `max_zeit`: eliminated (27). Total = Σ rounds; time total = Σ times.
- Derby: driven time, penalty seconds (knock-downs, others), each × `zeitfaktor`, round2, added.
- Open: seconds and/or points; scorings: time (asc), points (desc), penalty points (asc),
  points-then-time.
- Multitest (PD:1499–1554, programmes B1/B4): per judge marks 0–10 × coefficient − penalties A
  (dressage), B (per obstacle, 0.5 steps), C (ground test); total = round2(mean of judges),
  higher is better.

## Rankings

- Dressage ascending (no tie-break); cones: points, then (JC) deviation or time; marathon and
  derby: points; open per scoring; multitest descending. Teams in cones: the worst round of the
  members dropped; team order by points, then the dropped result; team points 12, 10, 9 … 2.
- Overall (shared with eventing): dressage as its 160-scale total (not 100 − %), deviations (JC)
  in the tie-break; open phases need `rangpunkte`.

## Prints

- Start lists (`fahren-startlisten.print.service.js`): order, HC/NS, carriage no., licence,
  driver/club, horses of the phase (signalement), optional pedigree and carriage width ("jury"
  list); marathon: start time A and B; cones/derby with start times; team cones with teams.
- Day start lists: the phases of one date together.
- Judge sheet: no Korr./Koeff. columns; carriage number, driver.
- Control sheets (`kontrollblatt.print.service.js`): cones (carriage width, allowance, track
  width, two rounds: time, faults, allowed time, other faults, time over × 0.5), marathon and derby
  obstacle sheets with fixed penalty catalogues (`i18n/de.js:766–802`).
- Rankings (`fahren-rangliste.print.service.js`): per kind (dressage per judge with judge rank;
  marathon A/B and each obstacle; cones per round; open; multitest per judge A+B+C; derby); overall:
  per phase penalties / rank, total.

## Upload

- Start lists per phase (`weiteres` "FB,FL / 2 Sp", horses by `einsaetze`). Phase results via
  `/putZwischenresultate` (dressage `{letter: total}`; cones per round). Overall via
  `/putResultate`: `strafpunkte_<discipline>` + `strafpunkte_total` (or `rang_<discipline>` +
  `rangpunkte_total`), `aufgabegrund_<discipline>_id`; no `pro_richter`.

## Rosson's bugs (not to copy)

1. Marathon: hours × 360 instead of 3600 (PD:464). 2. Settings loops write junk keys '0', '1'.
3. Cones times as strings (bonus and ±5 s concatenated). 4. No matching licence: no time faults,
no elimination, silently. 5. `total.defaultValue = 160` averaged into dressage totals. 6. Dressage
import uses percentages. 7. Coefficient counting differs between programmes. 8. Multitest
half-finished (`total_a` unused, wrong fields in layout and print, `punkte` not selectable,
overall upload sends zeros). 9. Overall layout always shows `strafpunkte`. 10. Overall columns
paired by index with the SE phase list: wrong after reordering. 11. Team cones: three bugs (dropped
round written to results, sort before filter, ties never detected). 12. Day start list ignores a
phase's own date. 13. Missing label DATUM/ORT; start list crashes without `uhrzeit`. 14. Single-phase
dressage upload: `strafpunkte_total` 0, `punkte_prozent` on the 160 scale. 15. Derby computed only
in the form. 16. Start-list settings leak between phases. 17. Codes OP/OF, HI inconsistent.
18. Saving a result ranks before recounting the placed riders (`pruefungsdurchfuehrung.controller.js:147–150`,
any class): the placing flags use the count before that result (seen twice in cones phases of 11:
3 placed instead of 4).
