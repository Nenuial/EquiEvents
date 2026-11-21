#!/usr/bin/env node
// Ask Rosson to recompute every corpus event and compare with what it stored at the show.
//
//   node src/check-corpus.mjs [event-nr ...]
//
// Writes corpus/<nr>/oracle.json. A difference means one of: a manual override at the show,
// a different Rosson version at the show, data edited after ranking, or Rosson being
// inconsistent with itself. Each needs a look before the result is used as a test case.

import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { bootRosson } from './oracle/rosson.mjs';
import { recomputeEvent } from './oracle/recompute.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const corpusDir = join(here, '../corpus');
const vendorDir = join(here, '../../vendor/rosson-3.3.15');

// Inputs, embedded copies and bookkeeping: not part of what Rosson computes.
// `stufe` is a copy of the entry's height level that newer 3.3.x versions sync onto each
// result when saving; 2025 data predates that.
const NOT_COMPUTED = new Set(['updated_at', 'nennung', 'equipe', 'details', 'strafpunkte_details', 'id', 'starter_id',
  'pruefung_id', 'veranstaltung_id', 'typ', 'zwischenpruefung_id', 'equipe_id', 'stufe']);

const stable = v => JSON.stringify(v, (k, x) => (x && typeof x === 'object' && !Array.isArray(x)
  ? Object.fromEntries(Object.entries(x).sort(([a], [b]) => a.localeCompare(b))) : x));

function compare(stored, recomputed, snapshot) {
  const byId = new Map(recomputed.map(r => [r.id, r]));
  const pruefungName = Object.fromEntries(snapshot.pruefungen.map(p => [String(p.id), `${p.nummer} ${p.name}`]));
  const perPruefung = {};
  for (const s of stored) {
    const key = String(s.pruefung_id);
    const entry = perPruefung[key] ||= { pruefung: pruefungName[key] || key, results: 0, identical: 0, fields: {} };
    entry.results++;
    const r = byId.get(s.id);
    if (!r) { (entry.fields['<missing after recompute>'] ||= { count: 0, examples: [] }).count++; continue; }
    let same = true;
    for (const field of new Set([...Object.keys(s), ...Object.keys(r)])) {
      if (NOT_COMPUTED.has(field)) continue;
      const a = s[field];
      const b = r[field];
      if (stable(a) === stable(b)) continue;
      same = false;
      const kind = a !== undefined && b !== undefined && String(a) === String(b) ? 'type only' : 'value';
      const f = entry.fields[`${field} (${kind})`] ||= { count: 0, examples: [] };
      f.count++;
      if (f.examples.length < 5) f.examples.push({ resultat: s.id, stored: a ?? null, recomputed: b ?? null });
    }
    if (same) entry.identical++;
  }
  return perPruefung;
}

const events = process.argv.slice(2).length ? process.argv.slice(2) : readdirSync(corpusDir).filter(d => /^\d{4}\.\d+$/.test(d)).sort();
for (const nr of events) {
  const snapshot = JSON.parse(readFileSync(join(corpusDir, nr, 'snapshot.json'), 'utf8'));
  const rossonLog = [];
  const rosson = bootRosson(vendorDir, { log: (level, ...args) => rossonLog.push(`${level}: ${args.map(a => (typeof a === 'string' ? a : JSON.stringify(a)?.slice(0, 300))).join(' ')}`.slice(0, 400)) });
  const t0 = Date.now();
  const { resultate, problems } = await recomputeEvent(snapshot, rosson);
  rosson.close();

  const perPruefung = compare(snapshot.resultate, resultate, snapshot);
  const total = Object.values(perPruefung).reduce((a, p) => ({ results: a.results + p.results, identical: a.identical + p.identical }), { results: 0, identical: 0 });
  writeFileSync(join(corpusDir, nr, 'oracle.json'), `${JSON.stringify({
    event: nr,
    rosson: rosson.version,
    note: 'stored = what Rosson saved at the show; recomputed = what Rosson computes now from the same inputs',
    ...total,
    problems,
    perPruefung,
    rossonLog: [...new Set(rossonLog)].slice(0, 200),
    // Rosson 3.3.15's computed values for every result: the second reference for the engine
    // tests, used where a stored show result is known to be wrong (docs/deviations.md).
    recomputed: Object.fromEntries(resultate
      .filter(r => snapshot.resultate.some(s => s.id === r.id))
      .map(r => [r.id, Object.fromEntries(['rang', 'klassiert', 'zeit', 'strafpunkte', 'gesamttotal', 'total', 'max']
        .filter(f => r[f] !== undefined && !(f === 'strafpunkte' && typeof r[f] === 'object')).map(f => [f, r[f]]))])),
  }, null, 1)}\n`);

  console.log(`\n${nr}: ${total.identical}/${total.results} results identical after recompute (${Date.now() - t0} ms)${problems.length ? `, ${problems.length} problems` : ''}`);
  for (const p of Object.values(perPruefung)) {
    const diffs = Object.entries(p.fields).map(([f, x]) => `${f}: ${x.count}`).join(', ');
    console.log(`  ${String(p.pruefung).slice(0, 60).padEnd(60)} ${String(p.identical).padStart(3)}/${String(p.results).padEnd(3)} ${diffs}`);
  }
  for (const pr of problems.slice(0, 10)) console.log(`  problem: ${JSON.stringify(pr).slice(0, 200)}`);
}
