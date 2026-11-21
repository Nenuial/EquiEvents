#!/usr/bin/env node
// Keeps the golden corpus out of the repository (the user, 2026-11-21: "avoid any data from the
// corpus ever reaching github"). The corpus (golden/corpus: real shows) stays on this computer;
// this script reads it here and looks for its people and horses in what git is about to keep:
//
//   node scripts/corpus-guard.mjs --staged   what is being committed (the pre-commit hook)
//   node scripts/corpus-guard.mjs --push     what is being pushed (the pre-push hook; refs on stdin)
//   node scripts/corpus-guard.mjs --tree     every file git tracks, as it is on disk
//
// Looked for: riders', officials', owners' and breeders' full names, horses' names (one word: 7
// letters or more; several: 5 letters or more in words of 3 or more), e-mail addresses, phone
// numbers, street addresses, events' access keys (sires and dams are not: public pedigree,
// often ordinary words); and
// files that must never be committed (golden/corpus, Rosson exports, show files). What is found
// is shown as file:line and the kind, never the value. Anything found stops the commit or push.
// The hooks: `git config core.hooksPath .githooks` (once per clone). On a computer without the
// corpus only the file names are checked (and it says so).

import { execFileSync } from 'node:child_process';
import { chmodSync, existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join } from 'node:path';

const root = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim();
const corpus = join(root, 'golden', 'corpus');
const mode = process.argv[2] ?? '--tree';
// Bumped when what is looked for changes (the cache is then made again).
const GUARD_VERSION = 2;
// `--show`: also print what was found (to review, on this computer only: never in a log kept).
const show = process.argv.includes('--show');
// What the corpus holds in place of removed values (golden/src/lib/anonymize.mjs): not data.
const placeholders = new Set(['strasse 1', 'entfernt example invalid', '41 00 000 00 00', '41000000000', 'begleitperson', 'redacted']);
// Not anyone's: Rosson's placeholder horse, Swiss Equestrian's demo keys and its own address
// (on the judges' sheets), words that are also some horse's name, the practice event's invented
// horses that happen to share a real one's name.
const public_ = new Set([
  'pseudopferd', 'demoveranstaltung', 'demoveranstaltung fa', 'postfach', 'papiermuhlestrasse 40', 'papiermuhlestrasse 40 h',
  'nessuno', 'level h', 'les pres', 'pepite ii', 'quasar ii',
]);

/** Lower case, no accents, words separated by one space. */
const norm = s =>
  String(s ?? '')
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();

// What must never be committed, by its path.
const forbidden = [/^golden\/corpus\//, /_export_.*\.txt$/i, /^rosson_export_/i, /\.equievents$/i, /(^|\/)secrets\.json$/, /(^|\/)\.env$/];

// The corpus's people and horses, by kind.
const needles = new Map(); // normalized text -> kind
const exact = new Map(); // raw (lower case) text -> kind: e-mails, keys
const add = (kind, text, { single = 7 } = {}) => {
  const n = norm(text);
  if (!n || placeholders.has(n) || public_.has(n)) return;
  const words = n.split(' ');
  if (words.length === 1 && (n.length < single || /^\d+$/.test(n))) return;
  // Several words: enough of a name (not "st line" in code).
  if (words.length > 1 && words.filter(w => w.length >= 3).join('').length < 5) return;
  if (!needles.has(n)) needles.set(n, kind);
};
const person = (kind, first, last) => {
  if (!norm(first) || !norm(last)) return;
  add(kind, `${first} ${last}`, { single: Infinity });
  add(kind, `${last} ${first}`, { single: Infinity });
};
const contact = (r, prefix = '') => {
  for (const k of ['email']) if (r?.[k] && String(r[k]).includes('@') && !placeholders.has(norm(r[k]))) exact.set(String(r[k]).toLowerCase().trim(), 'e-mail');
  for (const k of ['telefon_mobil', 'telefon_privat', 'telefon_arbeit']) {
    const digits = String(r?.[k] ?? '').replace(/\D/g, '');
    if (digits.length >= 9 && !placeholders.has(digits)) needles.set(digits, 'phone');
  }
  if (r?.[`${prefix}strasse`] && norm(r[`${prefix}strasse`]).length >= 8) add('street', r[`${prefix}strasse`], { single: 8 });
};
// "Name Firstname, Town" (owners, breeders): the name before the comma, two words or more.
const owner = (kind, text) => {
  const name = String(text ?? '').split(',')[0];
  if (norm(name).split(' ').length >= 2) add(kind, name, { single: Infinity });
};

// Every record of a show (a corpus snapshot, a Rosson export): its people, horses, contacts, key.
function walk(v) {
  if (Array.isArray(v)) return v.forEach(walk);
  if (!v || typeof v !== 'object') return;
  person('person', v.vorname, v.nachname);
  person('rider', v.reiter_vorname, v.reiter_nachname);
  contact(v);
  contact(v, 'reiter_');
  add('horse', v.pferdename);
  add('horse', v.gastpferd_name);
  add('person', v.begleit_person, { single: Infinity });
  for (const k of ['pferd_besitzer', 'pferd_mitbesitzer', 'besitzer', 'mitbesitzer']) owner('owner', v[k]);
  for (const k of ['pferd_zuechter', 'zuechter']) owner('breeder', v[k]);
  if (typeof v.access_key === 'string' && v.access_key.length >= 6 && !placeholders.has(norm(v.access_key)) && !public_.has(norm(v.access_key))) exact.set(v.access_key.toLowerCase(), 'access key');
  Object.values(v).forEach(walk);
}

// What the sources hold is kept between runs (they rarely change; reading them takes a while), in
// a file only this user can read, outside the repository.
const cacheFile = join(tmpdir(), `equievents-corpus-guard-${process.getuid?.() ?? 'user'}.json`);
const exportsDir = process.env.EQUIEVENTS_EXPORTS || join(homedir(), 'Dropbox', 'Concours');
const files = dir => {
  try {
    return readdirSync(dir, { withFileTypes: true }).flatMap(e => (e.isDirectory() ? files(join(dir, e.name)) : /_export_.*\.txt$/i.test(e.name) ? [join(dir, e.name)] : []));
  } catch {
    return [];
  }
};
const snapshots = existsSync(corpus) ? readdirSync(corpus).map(show => join(corpus, show, 'snapshot.json')).filter(f => existsSync(f)) : [];
const exportFiles = files(exportsDir);
const sources = [...snapshots, ...exportFiles].map(f => `${f}:${statSync(f).size}:${statSync(f).mtimeMs}`).join('\n') + `\n${GUARD_VERSION}`;
let shows = snapshots.length;
let exports = 0;
let cached = null;
try {
  cached = JSON.parse(readFileSync(cacheFile, 'utf8'));
} catch {
  cached = null;
}
if (cached?.sources === sources) {
  for (const [k, v] of cached.needles) needles.set(k, v);
  for (const [k, v] of cached.exact) exact.set(k, v);
  exports = cached.exports;
} else {
  for (const file of snapshots) walk(JSON.parse(readFileSync(file, 'utf8')));
  // The Rosson exports the corpus is made from (and the archive some tests read here), with all
  // their people: `EQUIEVENTS_EXPORTS`, else the Dropbox folder of shows.
  for (const file of exportFiles) {
    const text = readFileSync(file, 'utf8');
    const body = text.slice(text.indexOf('\n') + 1).trim();
    try {
      walk(JSON.parse(body.startsWith('{') ? body : decodeURIComponent(body)));
      exports++;
    } catch {
      // Not an export this script reads: skipped.
    }
  }
  if (shows || exports) {
    writeFileSync(cacheFile, JSON.stringify({ sources, exports, needles: [...needles], exact: [...exact] }), { mode: 0o600 });
    chmodSync(cacheFile, 0o600);
  }
}

const longest = Math.max(1, ...[...needles.keys()].map(n => n.split(' ').length));
const emails = /[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}/gi;

/** What a line holds of the corpus: the kinds found (with `--show`, what). */
function found(line) {
  const kinds = new Set();
  const words = norm(line).split(' ').filter(Boolean);
  for (let i = 0; i < words.length; i++) {
    for (let k = 1; k <= longest && i + k <= words.length; k++) {
      const text = words.slice(i, i + k).join(' ');
      const kind = needles.get(text);
      if (kind) kinds.add(show ? `${kind} «${text}»` : kind);
    }
  }
  const digits = line.replace(/[\s./()-]/g, '').match(/\d{9,}/g) ?? [];
  for (const d of digits) if (needles.get(d) === 'phone') kinds.add('phone');
  for (const m of line.match(emails) ?? []) if (exact.has(m.toLowerCase())) kinds.add(exact.get(m.toLowerCase()));
  const lower = line.toLowerCase();
  for (const [text, kind] of exact) if (kind === 'access key' && lower.includes(text)) kinds.add(kind);
  return [...kinds];
}

const hits = [];
const look = (path, lines) => {
  if (forbidden.some(f => f.test(path))) hits.push(`${path}: a file that must never be committed`);
  if (!shows && !exports) return;
  for (const [n, line] of lines) {
    const kinds = found(line);
    if (kinds.length) hits.push(`${path}:${n}: ${kinds.join(', ')}`);
  }
};

/** The added lines of a patch (`git diff -U0`), by file, with their line numbers. */
function patch(text) {
  let path = null;
  let at = 0;
  const files = new Map();
  for (const line of text.split('\n')) {
    if (line.startsWith('+++ ')) {
      path = line.slice(4).replace(/^b\//, '');
      if (path === '/dev/null') path = null;
      else files.set(path, []);
    } else if (line.startsWith('@@')) {
      at = Number(/\+(\d+)/.exec(line)?.[1] ?? 0);
    } else if (path && line.startsWith('+')) {
      files.get(path).push([at++, line.slice(1)]);
    } else if (line.startsWith('diff --git')) {
      // A new file with no added lines (empty, or binary) still has its name checked.
      const m = / b\/(.+)$/.exec(line);
      if (m && !files.has(m[1])) files.set(m[1], []);
      path = null;
    }
  }
  return files;
}

const git = (...args) => execFileSync('git', args, { encoding: 'utf8', maxBuffer: 1 << 30, cwd: root });

if (mode === '--tree') {
  for (const path of git('ls-files', '-z').split('\0').filter(Boolean)) {
    const file = join(root, path);
    if (!existsSync(file)) continue;
    const bytes = readFileSync(file);
    if (bytes.includes(0)) {
      look(path, []);
      continue;
    }
    look(path, bytes.toString('utf8').split('\n').map((l, i) => [i + 1, l]));
  }
} else if (mode === '--staged') {
  for (const [path, lines] of patch(git('diff', '--cached', '-U0', '--no-color', '--no-ext-diff'))) look(path, lines);
} else if (mode === '--push') {
  const input = readFileSync(0, 'utf8');
  for (const line of input.split('\n').filter(Boolean)) {
    const [, local] = line.split(' ');
    if (/^0+$/.test(local)) continue; // a branch deleted
    // Every commit not on the remote yet, each one's additions.
    for (const commit of git('rev-list', local, '--not', '--remotes').split('\n').filter(Boolean)) {
      for (const [path, lines] of patch(git('show', '--format=', '-U0', '--no-color', '--no-ext-diff', '--root', commit))) look(path, lines);
    }
  }
} else {
  console.error('corpus-guard: --staged, --push or --tree');
  process.exit(2);
}

if (!shows && !exports) console.error(`corpus-guard: no corpus at ${corpus} and no exports in ${exportsDir}: only file names checked`);
else console.error(`corpus-guard: ${shows} corpus shows and ${exports} Rosson exports, ${needles.size + exact.size} names and values looked for`);
if (hits.length) {
  console.error(`corpus-guard: data of the golden corpus found (${hits.length}):\n  ${hits.join('\n  ')}`);
  process.exit(1);
}
console.error('corpus-guard: nothing of the corpus found');
