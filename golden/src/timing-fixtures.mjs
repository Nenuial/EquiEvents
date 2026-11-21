#!/usr/bin/env node
// Rosson's side of the timekeeping file contract, as fixtures for the Rust tests (crates/timing).
//
//   node src/timing-fixtures.mjs [show ...]
//
// For every show-jumping class of every corpus show, in corpus/<show>/timing/:
//   <class>.tsv / <class>.xlsx          the start list as Rosson 3.3.15 exports it for timekeepers
//   <class>-results.tsv / .xlsx         a results file as a timekeeper sends it back, built from
//                                       the results stored at the show
//   <class>-imported.json               what Rosson reads from each results file, per entry (the
//                                       class's results are removed before each import)
//   <class>-ranks.tsv                   the results file with the last-ranked rider sent as 1st
//   <class>-ranks-imported.json         Rosson's ranking after importing it without and with
//                                       "Rang übernehmen" (take the timekeeper's ranks over)
//
// The results files use the conventions of the timekeeping software as Rosson expects them:
// "OK" in the Aufgabe column for riders who finished, DISQUAL / RET / DNS / DSQ otherwise, and for
// two-phase classes phase 2 in the jump-off columns ("Strafpunkte 1 St", "Zeit 1 St").

import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { bootRosson } from './oracle/rosson.mjs';
import { loadEvent, selectPruefung } from './oracle/recompute.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const corpusDir = join(here, '../corpus');
const vendorDir = join(here, '../../vendor/rosson-3.3.15');
const xlsx = createRequire(join(vendorDir, 'package.json'))('xlsx');

const HEAD = ['Start-Nr', 'Name Pferd', 'Reiter', 'Pr Bezeichnung', 'PSerie', 'Rang', 'Lizenz Nr', 'Pass Nr Pferd', 'Signalement',
  'Strafpunkte NP', 'Zeit NP', 'Aufgabe', 'Strafpunkte NP 2 Umg', 'Zeit NP 2 Umg', 'Strafpunkte 1 St', 'Zeit 1 St', 'Strafpunkte 2 St',
  'Zeit 2 St', 'Strafpunkte 3 St', 'Zeit 3 St', 'Strafpunkte 4 St', 'Zeit 4 St', 'hc', 'Nichtstarter', 'Reihenfolge-Nr'];
const COLUMN = {
  strafpunkte_initial_1: 'Strafpunkte NP', zeit_initial_1: 'Zeit NP', strafpunkte_initial_2: 'Strafpunkte NP 2 Umg', zeit_initial_2: 'Zeit NP 2 Umg',
  strafpunkte_stechen_1: 'Strafpunkte 1 St', zeit_stechen_1: 'Zeit 1 St', strafpunkte_stechen_2: 'Strafpunkte 2 St', zeit_stechen_2: 'Zeit 2 St',
};
const REASON_TEXT = { 21: 'DISQUAL', 22: 'RET', 23: 'DNS' };
// Dressage reasons (24 ausgeschieden, 25 aufgegeben, 26 verzichtet) use the same texts (code − 3).
const id = v => (v === null || v === undefined ? '' : String(v));
// Windows-1252, as timekeeping software writes text files (Latin-1 plus €‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ).
const CP1252_EXTRA = '€\u0081‚ƒ„…†‡ˆ‰Š‹Œ\u008DŽ\u008F\u0090‘’“”•–—˜™š›œ\u009DžŸ';
const cp1252 = text => Buffer.from([...text].map(c => {
  const i = CP1252_EXTRA.indexOf(c);
  if (i >= 0) return 0x80 + i;
  const code = c.codePointAt(0);
  return code < 256 ? code : 0x3f; // '?'
}));

// Zip entries carry the time they were written: set to a fixed date (1980-01-01), so that
// regenerating the fixtures changes no file unless its content changes.
function fixedZipTimes(bytes) {
  const b = Buffer.from(bytes);
  let end = b.length - 22;
  while (end >= 0 && b.readUInt32LE(end) !== 0x06054b50) end--;
  if (end < 0) throw new Error('not a zip file');
  let at = b.readUInt32LE(end + 16);
  for (let n = b.readUInt16LE(end + 10); n > 0; n--) {
    if (b.readUInt32LE(at) !== 0x02014b50) throw new Error('bad zip central directory');
    const local = b.readUInt32LE(at + 42);
    b.writeUInt32LE(0x00210000, at + 12); // time 00:00:00, date 1980-01-01
    b.writeUInt32LE(0x00210000, local + 10);
    at += 46 + b.readUInt16LE(at + 28) + b.readUInt16LE(at + 30) + b.readUInt16LE(at + 32);
  }
  return b;
}
const writeXlsx = (path, bytes) => writeFileSync(path, fixedZipTimes(bytes));
const fixXlsx = path => writeXlsx(path, readFileSync(path));

// Remove a class's results before importing a file into it, so that what Rosson reports
// afterwards is what it read from the file, not what was stored at the show. (Rosson creates a
// result when it reads one for a starter without.)
async function clearResults(rosson, p) {
  const db = rosson.get('DatabaseFactory');
  for (const r of rosson.fromPage(await db.getAll('resultate')).filter(x => String(x.pruefung_id) === String(p.id))) await db.remove('resultate', r.id);
  await rosson.get('ResultatService').loadAll();
}

const shows = process.argv.slice(2).length ? process.argv.slice(2) : readdirSync(corpusDir).filter(d => /^\d{4}\.\d+$/.test(d)).sort();
for (const nr of shows) {
  const snapshot = JSON.parse(readFileSync(join(corpusDir, nr, 'snapshot.json'), 'utf8'));
  const out = join(corpusDir, nr, 'timing');
  mkdirSync(out, { recursive: true });
  const rosson = bootRosson(vendorDir);
  await loadEvent(snapshot, rosson);
  const PS = rosson.get('PruefungenService');
  const PDS = rosson.get('PruefungsdurchfuehrungService');
  const db = rosson.get('DatabaseFactory');

  const disziplin = Object.fromEntries(snapshot.disziplinen.map(d => [d.id, d.code]));
  const ap = Object.fromEntries(snapshot.ausschreibung_pruefungen.map(a => [a.id, a]));
  const wertung = Object.fromEntries(snapshot.wertungen.map(w => [w.id, w]));
  const nennung = Object.fromEntries(snapshot.nennungen.map(n => [n.id, n]));
  let classes = 0;

  for (const p of snapshot.pruefungen) {
    if (disziplin[ap[p.ausschreibung_pruefung_id]?.disziplin_id] === 'CD') {
      try {
        await dressageFixtures(rosson, snapshot, p, out);
      } catch (e) {
        // Rosson itself fails on this class: recorded, so the Rust test knows there is no fixture.
        writeFileSync(join(out, `${String(p.nummer).replace(/[^\w.-]/g, '_')}-tablet-rosson-failed.txt`), `${e?.stack || e}\n`);
        console.log(`  dressage ${p.nummer}: Rosson failed: ${String(e?.message || e).slice(0, 80)}`);
      }
      continue;
    }
    if (disziplin[ap[p.ausschreibung_pruefung_id]?.disziplin_id] !== 'CS') continue;
    const pruefung = PS.get(p.id);
    const name = String(p.nummer).replace(/[^\w.-]/g, '_');
    selectPruefung(rosson, pruefung); // as in the UI: export and import from the class's results screen

    // 1. Start list export, both formats.
    PDS.exportStarter(`tsv:${p.id}`, pruefung, {}, false);
    writeFileSync(join(out, `${name}.tsv`), rosson.files.get(`tsv:${p.id}`));
    PDS.exportStarter(join(out, `${name}.xlsx`), pruefung, {}, true);
    fixXlsx(join(out, `${name}.xlsx`));

    // 2. A results file as the timekeeper sends it.
    const twoPhase = (wertung[p.wertung_id]?.bezeichnung_de || '').includes('Phase');
    const starters = snapshot.starter.filter(s => String(s.pruefung_id) === String(p.id)).sort((a, b) => a.reihenfolge - b.reihenfolge);
    const rows = [HEAD];
    for (const s of starters) {
      const n = nennung[s.nennung_id];
      if (!n) continue;
      const r = snapshot.resultate.find(x => x.starter_id === s.id);
      const row = Object.fromEntries(HEAD.map(h => [h, '']));
      Object.assign(row, {
        'Start-Nr': id(n.startnummer), 'Name Pferd': id(n.pferde?.[0]?.pferdename), 'Reiter': `${n.reiter_vorname} ${n.reiter_nachname}`,
        'Pr Bezeichnung': id(p.nummer), 'Lizenz Nr': id(n.reiter_id), 'Pass Nr Pferd': id(n.pferde?.[0]?.pferd_id), 'Reihenfolge-Nr': id(s.reihenfolge),
        'hc': n.hors_concours ? 'hc' : '',
      });
      if (r?.details) {
        const code = Number(r.details.aufgabe) || 0;
        row.Aufgabe = code > 0 ? (REASON_TEXT[code] || 'DSQ') : 'OK';
        for (const [field, column] of Object.entries(COLUMN)) {
          const target = twoPhase && field.endsWith('initial_2') ? COLUMN[field.replace('initial_2', 'stechen_1')] : column;
          if (r.details[field] !== undefined && r.details[field] !== null && r.details[field] !== '') row[target] = id(r.details[field]);
        }
      }
      rows.push(HEAD.map(h => row[h]));
    }
    const tsv = cp1252(rows.map(r => r.join('\t')).join('\r\n'));
    writeFileSync(join(out, `${name}-results.tsv`), tsv);
    const typed = rows.map((r, i) => (i === 0 ? r : r.map(v => (v !== '' && /^-?\d+(\.\d+)?$/.test(v) ? Number(v) : v))));
    const book = xlsx.utils.book_new();
    xlsx.utils.book_append_sheet(book, xlsx.utils.aoa_to_sheet(typed), `Resultate ${name}`.slice(0, 31));
    const xlsxBytes = fixedZipTimes(xlsx.write(book, { type: 'buffer', bookType: 'xlsx' }));
    writeFileSync(join(out, `${name}-results.xlsx`), xlsxBytes);

    // 3. What Rosson reads from each.
    const imported = {};
    for (const [format, bytes] of [['tsv', tsv], ['xlsx', xlsxBytes]]) {
      await clearResults(rosson, p);
      const file = new rosson.window.File([new rosson.window.Uint8Array(bytes)], `results.${format}`);
      await PDS.importSpringen({ file: cb => cb(file) }, pruefung, {}, false);
      const after = rosson.fromPage(await db.getAll('resultate')).filter(x => String(x.pruefung_id) === String(p.id) && x.starter_id);
      const starterEntry = Object.fromEntries(snapshot.starter.map(s => [s.id, s.nennung_id]));
      imported[format] = Object.fromEntries(after.map(x => [String(starterEntry[x.starter_id]), {
        values: Object.fromEntries(Object.entries(x.details || {}).filter(([k, v]) => /^(zeit|strafpunkte)_/.test(k) && v !== undefined && v !== null && v !== '').map(([k, v]) => [k, String(v)])),
        out: Number(x.details?.aufgabe) || null,
      }]).sort(([a], [b]) => a.localeCompare(b)));
    }
    writeFileSync(join(out, `${name}-imported.json`), `${JSON.stringify({ class: p.nummer, table: p.wertung_id, imported }, null, 1)}\n`);

    // 4. The timekeeper's rank taken over (Rosson's option "Rang übernehmen"): the last-ranked
    // rider is sent as 1st; what Rosson ranks then.
    const ranked = starters.map(s => [s, snapshot.resultate.find(x => x.starter_id === s.id)]).filter(([, r]) => r && Number(r.rang) > 0 && Number(r.rang) < 99);
    if (!(p.equipen || []).length && ranked.length >= 3) {
      const [last] = ranked.sort((a, b) => Number(b[1].rang) - Number(a[1].rang))[0];
      const rankRows = rows.map((r, i) => (i > 0 && r[HEAD.indexOf('Reihenfolge-Nr')] === id(last.reihenfolge) && r[HEAD.indexOf('Lizenz Nr')] === id(nennung[last.nennung_id].reiter_id) ? r.map((v, c) => (c === HEAD.indexOf('Rang') ? '1' : v)) : r));
      const rankTsv = cp1252(rankRows.map(r => r.join('\t')).join('\r\n'));
      writeFileSync(join(out, `${name}-ranks.tsv`), rankTsv);
      const starterEntry = Object.fromEntries(snapshot.starter.map(s => [s.id, s.nennung_id]));
      const ranking = async adopt => {
        await clearResults(rosson, p);
        await PDS.importSpringen({ file: cb => cb(new rosson.window.File([new rosson.window.Uint8Array(rankTsv)], 'ranks.tsv')) }, pruefung, {}, adopt);
        selectPruefung(rosson, pruefung);
        await rosson.get('RanglisteService').updateRangliste(pruefung);
        const list = rosson.get('RanglisteService').getRangliste(pruefung, undefined);
        await db.getAll('resultate');
        return Object.fromEntries(list.filter(x => x.starter_id).map(x => [String(starterEntry[x.starter_id]), { rank: Number(x.customRang || x.rang) || null, manual: Number(x.customRang) || null }]).sort(([a], [b]) => a.localeCompare(b)));
      };
      const before = await ranking(false);
      const after = await ranking(true);
      writeFileSync(join(out, `${name}-ranks-imported.json`), `${JSON.stringify({ class: p.nummer, rank_1: String(last.nennung_id), before, after }, null, 1)}\n`);
    }
    classes++;
  }
  rosson.close();
  console.log(`${nr}: ${classes} jumping classes`);
}

// Dressage: the file for judges' tablets (Rosson: exportStarterDressur / importDressur).
//   <class>-tablet.xlsx            the file as Rosson exports it
//   <class>-tablet-results.xlsx    filled in, as a tablet app sends it back (marks from the show)
//   <class>-tablet-results.tsv     the same as text, CRLF line ends (Rosson needs them)
//   <class>-tablet-imported.json   what Rosson reads: per entry and judge, marks and ticked penalties
async function dressageFixtures(rosson, snapshot, p, out) {
  const PS = rosson.get('PruefungenService');
  const PDS = rosson.get('PruefungsdurchfuehrungService');
  const db = rosson.get('DatabaseFactory');
  if ((p.equipen || []).length || p.manuelle_eingabe) return;
  const pruefung = PS.get(p.id);
  const name = String(p.nummer).replace(/[^\w.-]/g, '_');
  selectPruefung(rosson, pruefung); // the export reads the selected class and its programme
  // As on the results screen: the class's programme, or (individual programmes) the programme of
  // the rider opened — Rosson's import reads the penalty columns from whatever programme is open.
  const firstStarter = snapshot.starter.filter(s => String(s.pruefung_id) === String(p.id)).sort((a, b) => a.reihenfolge - b.reihenfolge)[0];
  const firstEntry = firstStarter && snapshot.nennungen.find(n => n.id === firstStarter.nennung_id);
  PDS.initDressurProgramm(PS.hatIndividuellesProgramm(pruefung) ? firstEntry?.programm_id : pruefung.programm_id);
  const exportPath = join(out, `${name}-tablet.xlsx`);
  PDS.exportStarterDressur(exportPath, pruefung, {});
  fixXlsx(exportPath);

  // Fill it in from the marks stored at the show.
  const sheet = xlsx.readFile(exportPath);
  const grid = xlsx.utils.sheet_to_json(sheet.Sheets[sheet.SheetNames[0]], { header: 1, defval: '' });
  const head = grid[0];
  const col = h => head.indexOf(h);
  const judges = PS.getDressurRichter(pruefung.id);
  const starterOf = Object.fromEntries(snapshot.starter.filter(s => String(s.pruefung_id) === String(p.id)).map(s => [s.nennung_id, s.id]));
  const resultOf = Object.fromEntries(snapshot.resultate.filter(r => String(r.pruefung_id) === String(p.id)).map(r => [r.starter_id, r]));
  const nennungByRider = (rider, horse) => snapshot.nennungen.find(n => String(n.reiter_id) === String(rider) && String(n.pferde?.[0]?.pferd_id ?? '') === String(horse) && starterOf[n.id]);
  const filled = [head];
  for (const row of grid.slice(1)) {
    const r = [...row];
    const n = nennungByRider(row[col('rider_id')], row[col('horse_id')]);
    const res = n && resultOf[starterOf[n.id]];
    const judge = judges.find(j => j.buchstabe === row[col('judge')]);
    if (res?.details && judge) {
      const code = Number(res.details.aufgabe) || 0;
      r[col('status')] = code > 0 ? (REASON_TEXT[code - 3] || 'DSQ') : 'OK';
      const marks = res.details[judge.id] || {};
      for (const [movement, mark] of Object.entries(marks)) if (col(`V${movement}`) >= 0 && mark !== '' && mark !== null) r[col(`V${movement}`)] = String(mark);
      const tables = (res.strafpunkte || {})[judge.id] || {};
      for (const [t, list] of Object.entries(tables)) (list || []).forEach((pen, i) => {
        if (!pen?.checked) return;
        const c = col(`penalty_${i + 1}`) >= 0 ? col(`penalty_${i + 1}`) : col(`penalty_${Number(t) + 1}_${i + 1}`);
        if (c >= 0) r[c] = String(pen.strafpunkte ?? pen.strafprozente);
      });
    }
    filled.push(r);
  }
  const book = xlsx.utils.book_new();
  xlsx.utils.book_append_sheet(book, xlsx.utils.aoa_to_sheet(filled), 'Tablet');
  const xlsxBytes = fixedZipTimes(xlsx.write(book, { type: 'buffer', bookType: 'xlsx' }));
  writeFileSync(join(out, `${name}-tablet-results.xlsx`), xlsxBytes);
  const tsv = cp1252(filled.map(r => r.join('\t')).join('\r\n'));
  writeFileSync(join(out, `${name}-tablet-results.tsv`), tsv);

  const imported = {};
  const starterEntry = Object.fromEntries(snapshot.starter.map(s => [s.id, s.nennung_id]));
  for (const [format, bytes] of [['tsv', tsv], ['xlsx', xlsxBytes]]) {
    await clearResults(rosson, p);
    const file = new rosson.window.File([new rosson.window.Uint8Array(bytes)], `tablet.${format}`);
    await PDS.importDressur({ file: cb => cb(file) }, pruefung, {});
    const after = rosson.fromPage(await db.getAll('resultate')).filter(x => String(x.pruefung_id) === String(p.id) && x.starter_id);
    imported[format] = Object.fromEntries(after.map(x => [String(starterEntry[x.starter_id]), {
      out: Number(x.details?.aufgabe) || null,
      judges: Object.fromEntries(judges.map(j => [String(j.id), {
        marks: Object.fromEntries(Object.entries((x.details || {})[j.id] || {}).filter(([k, v]) => /^\d+$/.test(k) && v !== '' && v !== null).map(([k, v]) => [k, String(v)])),
        // Ticked penalties by position: "<table>_<field>" (from 1) → value.
        penalties: Object.fromEntries(Object.entries(((x.strafpunkte || {})[j.id]) || {}).flatMap(([t, list]) => (list || []).map((pen, i) => [`${Number(t) + 1}_${i + 1}`, pen]))
          .filter(([, pen]) => pen?.checked).map(([k, pen]) => [k, String(pen.strafpunkte ?? pen.strafprozente)])),
      }])),
    }]).sort(([a], [b]) => a.localeCompare(b)));
  }
  writeFileSync(join(out, `${name}-tablet-imported.json`), `${JSON.stringify({ class: p.nummer, programme: pruefung.programm_id, judges: judges.map(j => ({ id: String(j.id), letter: j.buchstabe })), imported }, null, 1)}\n`);
  console.log(`  dressage ${p.nummer}: ${grid.length - 1} tablet rows`);
}
