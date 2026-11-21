# Rosson 3.3.15: eventing (Concours Complet, CC)

What Rosson does, read from its code (paths under `vendor/rosson-3.3.15/src/renderer/app/`),
2026-11-14. The reference for EquiEvents' eventing; our decisions are in `docs/deviations.md`.

## Recognition and phases

- Class: `ausschreibung.disziplin_code === 'CC'` (`ausschreibung-pruefung.service.js:36`).
  Swiss Equestrian sends which phases: `kombipruefung_disziplinen`, e.g.
  `['dressur','springen','cross']` (:171–173). Allowed: dressur, paradressur, springen, cross.
- Phases are created by "Startlisten erstellen" (`startlisten-erstellen.controller.js:131–175`,
  `pruefungen.service.js:244–399`, `zwischenpruefungen.service.js:85–99, 252–306`): a copy of the
  parent with `parent_pruefung_id`, `id = <parent>_cd | _cs | _cross`, `code`,
  `nummer = <nr>_<code>`, `zwischenpruefung: true`, `sorter` (dressur 0, cross 1, springen 2).
  The parent gets `kombination = true`, `wertung_nach` (`strafpunkte` | `rangpunkte`).
- Cross is scored on the ideal-time table (AZI), jumping on AZ by default; a phase's date is the
  parent's unless changed.
- Phase fields: schedule (`datum`, `uhrzeit`, `zeit`, `type`, `springen_typ`, `programmDuration`,
  `pausen`, `platzpflege_*`, blocks), scoring (`manuelle_eingabe`, `programm_id`, `wertung_id`,
  `idealzeit` "mm:ss", `idealzeit_berechnen`, `max_zeit` "mm:ss:hh", `max_zeit_berechnen`,
  `anzahlKlassiert`, `customKlassierte`), status (`startliste_hochgeladen_am`,
  `rangliste_hochgeladen_am`, `position`).
- Phase order = `sorter`; it is the overall ranking's tie-break order. Reordered by drag and drop
  (`shared/zwischenpruefungen-sortieren/*`, "Wertung Teilprüfungen (Punktegleichheit)").

## Start lists

- One `starter` per rider per phase (`starter.service.js:84–104`). First phase sorted by start
  number or drawn, then copied to the others (`reihenfolgeSynchronisieren` 263–276); afterwards
  each phase has its own order ("Reihenfolge anpassen" syncs all; moving one starter asks; "by
  ranking" = reverse of the current overall ranking, riders without result last).
- Times per phase from its own settings. The start number is the entry's, shared by all phases.

## Entry

- Phase chosen in a dropdown ("Gesamt" + phases). Dressage: the normal form, judges of the parent.
  Jumping and cross: the jumping form; in practice `strafpunkte_initial_1` (penalties as one
  number) and `zeit_initial_1` (cross: `mm:ss`). No refusal count, no phases A/B in eventing.
- Result records in `resultate` with `zwischenpruefung_id`; phases matched by `nennung_id`.

## Scoring

- Dressage: `gesamttotal = Σ judges / max × 100` (2 decimals); overall value `100 − gesamt`.
- Jumping: penalties (+ ideal-time gap if that table and `idealzeit_berechnen`; time ≥ 2 × ideal
  = out); if `max_zeit_berechnen`: `+ ceil(t − max) × 0.4`.
- Cross (`calcCrossResultat`, `pruefungsdurchfuehrung.service.js:536–557`): integer seconds;
  `abweichung = |t − ideal|`; over the ideal time `ceil(abweichung) × 0.4` if
  `idealzeit_berechnen`; nothing for being early; points = obstacle penalties + time points.
  Ranked by points, then `abweichung`, then time, ascending (`rangliste.service.js:709–727`).
- Out (`details.aufgabe > 0`): rank 99, after the others. Ties share the rank. Placed:
  `ceil(0.3 × results)` unless set.

## Overall (`rangliste.service.js:729–958`)

- Per phase: penalties (or the phase rank, `rangpunkte`), deviations, out. Out in any phase: out
  overall. A phase not ridden adds nothing.
- Sorted by total ascending; ties: phases in `sorter` order, the lower value, then the smaller
  deviation; shared rank only if all equal.

## Prints

- Per phase: the usual start lists, rankings (cross cell "P: x.xx, T: Xmin Ys (±dev)"), judge
  sheets. Overall (`printGesamtRangliste` 555–655, `getLayoutCCGesamt` 1189–1233): rank, placed,
  rider (start number), horse, owner, club, sire / dam's sire, licence / passport / signalement,
  one result line per phase, penalties or rank points per phase, total (or the reason).

## Upload

- Start lists: one per phase (`rosson_startliste_id` = phase id, `nummer` "<nr> Dressur" …).
- Overall (`/putResultate`, CC branch 351–369): one row per rider: `punkte_total`,
  `punkte_dressur` (100 − %), `strafpunkte_springen_hindernis`, `strafpunkte_cross_hindernis`,
  rank 99 when out; no per-phase details.
- Phase results (`/putZwischenresultate` 474–582): `kopf{rosson_zwischenpruefung_id, position,
  code, name, datum, uhrzeit, wertung_id, wertung_text, programm_name}`, `pruefung{…}`,
  `zeilen[{position, rang, reiter_id, cache_reiter_name, cache_reiter_ort, pferde,
  resultat_details, resultat_layout}]`. A class of one phase uploads it as a normal result.

## Export

- Stores `zwischenpruefungen` (keyPath id), `resultate` (phase rows carry `zwischenpruefung_id`;
  overall rows `typ: 'gesamt'`), `starter` (per phase). `zwischenresultate` is legacy: empty.
