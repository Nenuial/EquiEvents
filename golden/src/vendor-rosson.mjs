#!/usr/bin/env node
// Extract Rosson's app.asar into vendor/rosson-<version>/ (git-ignored) for the oracle and
// as porting reference.
//
//   node src/vendor-rosson.mjs [path/to/Rosson.app]
//
// asar layout: uint32 pickle size (4) | uint32 header size | uint32 payload size |
// uint32 JSON length | JSON header | file contents. File offsets in the header are
// relative to the end of the header (8 + header size).

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const app = process.argv[2] || join(root, 'Rosson.app');
const asar = readFileSync(join(app, 'Contents/Resources/app.asar'));

const headerSize = asar.readUInt32LE(4);
const jsonLength = asar.readUInt32LE(12);
const header = JSON.parse(asar.subarray(16, 16 + jsonLength).toString('utf8'));
const base = 8 + headerSize;

const pkg = JSON.parse(fileAt(['package.json']).toString('utf8'));
const target = join(root, 'vendor', `rosson-${pkg.version}`);

// Rosson ships its own agent instructions (CLAUDE.md, .claude/). Inside this workspace they
// would be picked up as instructions for *our* project, so they are renamed / skipped.
const renamed = { 'CLAUDE.md': 'ROSSON-CLAUDE.md' };
const skipped = new Set(['.claude']);

let count = 0;
(function walk(node, parts) {
  for (const [rawName, entry] of Object.entries(node.files || {})) {
    if (rawName.includes('/') || rawName === '..') throw new Error(`unsafe path in asar: ${rawName}`);
    if (parts.length === 0 && skipped.has(rawName)) continue;
    const name = (parts.length === 0 && renamed[rawName]) || rawName;
    const next = [...parts, name];
    if (entry.files) walk(entry, next);
    else if (!entry.unpacked && !entry.link) {
      const out = join(target, ...next);
      mkdirSync(dirname(out), { recursive: true });
      writeFileSync(out, asar.subarray(base + Number(entry.offset), base + Number(entry.offset) + entry.size));
      count++;
    }
  }
})(header, []);

console.log(`Extracted ${count} files of Rosson ${pkg.version} to ${target}`);

function fileAt(path) {
  let node = header;
  for (const p of path) node = node.files[p];
  return asar.subarray(base + Number(node.offset), base + Number(node.offset) + node.size);
}
