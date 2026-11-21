// Let Rosson recompute an event from its own inputs, the way its UI does, and return the
// results it ends up storing.
//
// Steps (each mirrors Rosson code, file references are to vendor/rosson-3.3.15/src/renderer/app):
//   1. Load the snapshot into Rosson's database (like importing a backup).
//   2. Start the services the results screen starts
//      (pruefungsdurchfuehrung.controller.js → initServices).
//   3. For every Prüfung: select it (controller → changePruefung), re-save every individual
//      result through the UI's save path (PruefungsdurchfuehrungService.saveResultat, once per
//      judge for dressage; for team classes this also rebuilds the team row), then do what
//      the controller does after a save: RanglisteService.updateRangliste, then getRangliste
//      (which, for team classes, writes the team aggregates as a side effect).
//
// Before loading, every computed field is removed from the stored results, so Rosson has to
// derive them from what people entered (scores, times, faults, penalties). Otherwise a
// recompute that silently did nothing would look like a perfect match.

// Fields Rosson derives. For dressage `strafpunkte` is an input (penalties ticked per judge);
// for jumping it is the computed fault total, so it is only blanked when it is a number.
const COMPUTED = ['rang', 'klassiert', 'zeit', 'gesamttotal', 'total', 'max', 'herleitung', 'teilNote', 'duplikat'];
// Team rows (one per team, no starter) are entirely derived from the members' results.
const COMPUTED_TEAM = ['resultate', 'anzahlResultate', 'anzahlAufgaben'];

export const isTeamRow = r => !r.starter_id && (r.equipe_id != null || Array.isArray(r.resultate));

export function blankComputed(resultat) {
  const r = { ...resultat };
  for (const f of COMPUTED) delete r[f];
  if (isTeamRow(r)) for (const f of COMPUTED_TEAM) delete r[f];
  if (typeof r.strafpunkte !== 'object') delete r.strafpunkte;
  return r;
}

export async function recomputeEvent(snapshot, rosson, { blank = true } = {}) {
  const { get, fromPage } = rosson;
  const svc = name => get(name);
  const db = svc('DatabaseFactory');
  await loadEvent(snapshot, rosson, { blank });
  const PS = svc('PruefungenService');
  const PDS = svc('PruefungsdurchfuehrungService');
  const problems = [];
  return recomputeClasses(snapshot, rosson, PS, PDS, db, problems).then(() => ({ problems }))
    .then(async ({ problems }) => ({ resultate: fromPage(await db.getAll('resultate')), problems }));
}

/** Steps 1 and 2: load the snapshot into Rosson's database and start the results-screen services. */
export async function loadEvent(snapshot, rosson, { blank = false } = {}) {
  const { get, toPage, window: w } = rosson;
  const svc = name => get(name);
  const db = svc('DatabaseFactory');

  // 1. Load data (results stripped of computed fields).
  for (const [store, rows] of Object.entries(snapshot)) {
    if (!Array.isArray(rows) || !rows.length) continue;
    const data = store === 'resultate' && blank ? rows.map(blankComputed) : rows;
    await db.put(store, toPage(data));
  }
  const event = snapshot.veranstaltungen[0];
  w.localStorage.setItem('currentVeranstaltung', event.nr);
  for (const [key, value] of Object.entries(snapshot.localstore || {})) svc('StorageFactory').set(key, toPage(value));

  // 2. Start services like the results screen does (but await all of them).
  const VS = svc('VeranstaltungService');
  await VS.init();
  if (!VS.selected) throw new Error(`Rosson did not select event ${event.nr}`);
  svc('BloeckeService').setBloecke();
  await svc('ZwischenpruefungService').init();
  await svc('UploadService').init();
  await svc('ResultatService').init();
  await svc('ResultatService').loadAll();
  await svc('WertungsService').init();
  await svc('StarterService').init();
  await svc('EquipenService').init();
  await svc('NennungenService').init();
  await svc('PruefungenService').init();
  await svc('ProgrammeService').init();
  await svc('AusschreibungPruefungService').loadAusschreibungPruefungen(VS.selected.id);
  await svc('OffizielleService').init();
}

/** Step 3: per Prüfung, re-save every result and recompute the ranking. */
async function recomputeClasses(snapshot, rosson, PS, PDS, db, problems) {
  const svc = name => rosson.get(name);
  // 3. Per Prüfung.
  for (const p of snapshot.pruefungen) {
    const pruefung = PS.get(p.id);
    if (!pruefung) { problems.push({ pruefung: p.id, problem: 'Rosson cannot find this Prüfung' }); continue; }
    selectPruefung(rosson, pruefung);

    const results = (await db.getAll('resultate')).filter(r => String(r.pruefung_id) === String(p.id) && !isTeamRow(r));
    for (const resultat of results) {
      try {
        await resave(rosson, pruefung, resultat);
      } catch (e) {
        problems.push({ pruefung: p.id, resultat: resultat.id, problem: `save failed: ${e?.message || e}`, at: String(e?.stack || '').split('\n').slice(1, 4).map(l => l.trim()).join(' < ') });
      }
    }
    try {
      await svc('RanglisteService').updateRangliste(pruefung);
      await PDS.anzahlKlassierteBerechnen(pruefung);
      svc('RanglisteService').getRangliste(pruefung, undefined);
      await db.getAll('resultate'); // waits for the writes getRangliste started (same store)
    } catch (e) {
      problems.push({ pruefung: p.id, problem: `ranking failed: ${e?.message || e}`, at: String(e?.stack || '').split('\n').slice(1, 4).map(l => l.trim()).join(' < ') });
    }
  }

}

// pruefungsdurchfuehrung.controller.js → changePruefung({ pruefungID })
export function selectPruefung({ get }, pruefung) {
  const APS = get('AusschreibungPruefungService');
  const PS = get('PruefungenService');
  get('NennungenService').selected = undefined;
  get('ZwischenpruefungService').selected = undefined;
  get('EquipenService').selected = undefined;
  APS.selected = APS.get(pruefung.ausschreibung_pruefung_id);
  PS.setBlock(pruefung);
  PS.selected = pruefung;
  pruefung.equipen = get('EquipenService').getEquipenFuerPruefung(pruefung.id, true);
}

// The UI's save path for one result (resultat-form.controller.js → saveResultat).
async function resave({ get }, pruefung, resultat) {
  const PS = get('PruefungenService');
  const PDS = get('PruefungsdurchfuehrungService');

  if (resultat.typ !== 'dressur') {
    await PDS.saveResultat(resultat, false);
    return;
  }
  if (!resultat.details) return; // nothing entered

  // resultat-form.controller.js → initDressurProgramm: the Prüfung's programme, or the
  // rider's/team's own when the class has individual programmes.
  let programmID = pruefung.programm_id;
  if (PS.hatIndividuellesProgramm(pruefung)) {
    const starter = get('StarterService').get(resultat.starter_id);
    const nennung = starter && get('NennungenService').get(starter.nennung_id);
    programmID = nennung?.programm_id ?? resultat.equipe?.programm_id;
  }
  PDS.initDressurProgramm(programmID);

  const judges = PS.getDressurRichter(pruefung.id).map(j => j.id).filter(id => resultat.details[id]);
  if (PS.istChildren(pruefung)) judges.splice(1); // saveResultat loops over all judges itself
  for (const judge of judges) {
    PDS.selectedRichterID = judge;
    await PDS.saveResultat(resultat, false, judge);
  }
}
