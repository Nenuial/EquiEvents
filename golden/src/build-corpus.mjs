#!/usr/bin/env node
// Build the golden corpus from real Rosson show exports.
//
//   node src/build-corpus.mjs [--source DIR] [--years 2026[,2025]] [--min-version 3.3.0]
//
// For every show found, all its exports (per-event and full-database) are compared and the
// most complete snapshot is used: the one with the latest change inside the data, not the
// latest file name, because the final export is sometimes missing. The snapshot is cut down
// to that event, stripped of non-public personal data (see lib/anonymize.mjs), leak-checked,
// and written to corpus/<nr>/.

import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { compareVersions, findExports, readExport } from './lib/exports.mjs';
import { eventsIn, scopeEvent, snapshotStats } from './lib/scope.mjs';
import { leakCheck, makeAnonymizer } from './lib/anonymize.mjs';
import { renderRankings } from './lib/rankings.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const goldenDir = join(here, '..');

const args = Object.fromEntries(
  process.argv.slice(2).reduce((acc, a, i, all) => (a.startsWith('--') ? [...acc, [a.slice(2), all[i + 1]]] : acc), []),
);
const source = args.source || join(process.env.HOME, 'Library/CloudStorage/Dropbox/Concours');
const years = (args.years || '2026').split(',');
const minVersion = args['min-version'] || '3.3.0';
const corpusDir = join(goldenDir, 'corpus');

const fileYear = f => (f.kind === 'event' ? f.nr.slice(0, 4) : f.date.slice(0, 4));
const files = findExports(source).filter(f => years.includes(fileYear(f)));
console.log(`Found ${files.length} export files for ${years.join(', ')} under ${source}`);

// Read every candidate once and record which events it holds.
const candidates = [];
for (const f of files) {
  const { version, data } = readExport(f.path);
  const rel = relative(source, f.path);
  if (compareVersions(version, minVersion) < 0) {
    console.log(`  skip ${rel} (Rosson ${version} < ${minVersion})`);
    continue;
  }
  const sha256 = createHash('sha256').update(readFileSync(f.path)).digest('hex');
  for (const ev of eventsIn(data)) {
    if (!years.includes(String(ev.nr).slice(0, 4))) continue;
    candidates.push({ ...f, rel, version, sha256, data, event: ev, stats: snapshotStats(data, ev.id) });
  }
}

const byEvent = new Map();
for (const c of candidates) {
  if (!byEvent.has(c.event.nr)) byEvent.set(c.event.nr, []);
  byEvent.get(c.event.nr).push(c);
}

const summary = [];
let leaks = 0;
for (const [nr, list] of [...byEvent].sort()) {
  list.sort((a, b) =>
    b.stats.lastChange - a.stats.lastChange || b.stats.resultate - a.stats.resultate || b.date.localeCompare(a.date));
  const chosen = list[0];
  const { anonymize, secrets } = makeAnonymizer();
  const snapshot = anonymize(scopeEvent(chosen.data, chosen.event.id));
  const findings = leakCheck(snapshot, secrets);

  const dir = join(corpusDir, nr);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'snapshot.json'), `${JSON.stringify(snapshot, null, 1)}\n`);
  writeFileSync(join(dir, 'rankings.md'), renderRankings(nr, snapshot));
  writeFileSync(join(dir, 'source.json'), `${JSON.stringify({
    event: nr,
    chosen: { file: chosen.rel, rosson: chosen.version, sha256: chosen.sha256, ...chosen.stats },
    reason: 'latest change inside the data, then most results, then latest file date',
    candidates: list.map(c => ({ file: c.rel, kind: c.kind, rosson: c.version, ...c.stats })),
    removedValues: secrets.size,
    leakCheck: findings,
  }, null, 2)}\n`);

  leaks += findings.length;
  summary.push({ nr, chosen, alternatives: list.length - 1, findings, removed: secrets.size });
}

console.log('\nEvent       Rosson  Prüf.  definitiv  Starter  Resultate  Last change       Chosen export');
for (const { nr, chosen, alternatives } of summary) {
  const s = chosen.stats;
  console.log(`${nr.padEnd(11)} ${chosen.version.padEnd(7)} ${String(s.pruefungen).padStart(5)}  ${String(s.definitiv).padStart(9)}  ${String(s.starter).padStart(7)}  ${String(s.resultate).padStart(9)}  ${new Date(s.lastChange).toISOString().slice(0, 16)}  ${chosen.rel.split('/').pop()} (+${alternatives} other)`);
}
for (const { nr, findings, removed } of summary) {
  console.log(`Leak check ${nr}: ${removed} non-public values removed, ${findings.length} found again in the output`);
  for (const f of findings.slice(0, 40)) console.log(`  ${f.field} (${f.length} chars) → ${f.at}`);
}
console.log(leaks ? `\n${leaks} leaks — fix before committing.` : '\nLeak check passed.');
process.exitCode = leaks ? 1 : 0;
