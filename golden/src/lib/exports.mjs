// Reading Rosson export files.
//
// Rosson writes two kinds of export, both as `Rosson: <version>\n` followed by
// encodeURIComponent(JSON) of its IndexedDB stores:
//   {nr}_export_{yyyymmdd}.txt    one event   ("Veranstaltung abschliessen" dialog)
//   rosson_export_{yyyymmdd}.txt  whole local database (Settings), can hold several events
// Collaboration files ({nr}_export_master.txt, ..._client.txt) are partial and ignored.

import { readFileSync, readdirSync } from 'node:fs';
import { basename, join } from 'node:path';

const PER_EVENT = /^(\d{4}\.\d+)_export_(\d{8})\.txt$/;
const FULL_DB = /^rosson_export_(\d{8})\.txt$/;
const SKIP_DIRS = new Set(['node_modules', '.git', '.next', '.quarto', '.Rproj.user']);

/** @returns {{kind: 'event', nr: string, date: string} | {kind: 'full', date: string} | null} */
export function classify(path) {
  const name = basename(path);
  let m = PER_EVENT.exec(name);
  if (m) return { kind: 'event', nr: m[1], date: m[2] };
  m = FULL_DB.exec(name);
  if (m) return { kind: 'full', date: m[1] };
  return null;
}

/** Recursively find export files below `root`. */
export function findExports(root) {
  const found = [];
  const walk = dir => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (!SKIP_DIRS.has(entry.name)) walk(join(dir, entry.name));
      } else if (entry.isFile()) {
        const path = join(dir, entry.name);
        const kind = classify(path);
        if (kind) found.push({ path, ...kind });
      }
    }
  };
  walk(root);
  return found.sort((a, b) => a.path.localeCompare(b.path));
}

/** Parse one export file. Throws with a clear message if the format is not what we expect. */
export function readExport(path) {
  const raw = readFileSync(path, 'utf8');
  const newline = raw.indexOf('\n');
  const header = raw.slice(0, newline).trim();
  const m = /^Rosson: (\d+\.\d+\.\d+)$/.exec(header);
  if (!m) throw new Error(`${path}: unexpected header ${JSON.stringify(header.slice(0, 40))}`);
  const data = JSON.parse(decodeURIComponent(raw.slice(newline + 1)));
  return { version: m[1], data };
}

/** Compare dotted version strings numerically. */
export function compareVersions(a, b) {
  const pa = a.split('.').map(Number);
  const pb = b.split('.').map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d) return d;
  }
  return 0;
}
