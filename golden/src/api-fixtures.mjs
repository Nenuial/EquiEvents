#!/usr/bin/env node
// Rosson's side of the Swiss Equestrian API contract (my.swiss-equestrian.ch/api/veranstaltersoftware/v2), as
// fixtures for the Rust tests (crates/se-api).
//
//   node src/api-fixtures.mjs [show ...]
//
// Per corpus show, in corpus/<show>/api/, every request Rosson 3.3.15 makes for an action, in order:
//   results-<class>.json            uploading a class's results from the results screen
//   startlists-<published id>.json  uploading the start lists of a published class
//   event.json                      uploading the event (officials, entry counts)
//   delete-startlists.json          withdrawing a published class's start lists
// and rosson-key.json: Rosson's date-based key (`rosson_key`) for a few dates.
//
// No request leaves the process: XMLHttpRequest is replaced by a recorder that answers 200 {}.
// The key is replaced by "<rosson_key>" in the recorded requests; the event key (`access_key`) is
// already removed from the corpus.

import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { bootRosson } from './oracle/rosson.mjs';
import { loadEvent, selectPruefung } from './oracle/recompute.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const corpusDir = join(here, '../corpus');
const vendorDir = join(here, '../../vendor/rosson-3.3.15');
const KEY = '<rosson_key>';

/** Replace the page's XMLHttpRequest by a recorder. Returns the list requests are recorded in. */
function recordRequests(w) {
  const recorded = [];
  // The requests as sent (URL, headers), for crates/se-api's HTTP layer.
  recorded.raw = [];
  let pending = 0;
  class Recorder {
    constructor() { this.headers = {}; this.readyState = 0; this.status = 0; this.responseText = ''; this.response = ''; this.statusText = ''; }
    open(method, url) { this.method = method; this.url = url; this.readyState = 1; }
    setRequestHeader(name, value) { this.headers[name] = value; }
    getAllResponseHeaders() { return 'content-type: application/json'; }
    getResponseHeader(name) { return name.toLowerCase() === 'content-type' ? 'application/json' : null; }
    abort() {}
    send(body) {
      let parsed = body;
      try { parsed = body ? JSON.parse(body) : null; } catch { /* not JSON */ }
      const url = new URL(this.url);
      recorded.raw.push({ method: this.method, url: this.url.replace(/^https:\/\/[^/]+\/api\/veranstaltersoftware\/v2/, ''), headers: { ...this.headers } });
      recorded.push({
        method: this.method,
        path: url.pathname.replace(/^\/api\/veranstaltersoftware\/v2/, ''),
        query: Object.fromEntries([...url.searchParams].map(([k, v]) => [k, k === 'rosson_key' ? KEY : v])),
        body: parsed,
      });
      pending++;
      w.setTimeout(() => {
        this.readyState = 4; this.status = 200; this.statusText = 'OK'; this.responseText = '{}'; this.response = '{}';
        pending--;
        this.onload?.(); this.onreadystatechange?.(); this.onloadend?.();
      }, 0);
    }
  }
  w.XMLHttpRequest = Recorder;
  recorded.settle = async () => {
    for (let i = 0; i < 200 && (pending > 0 || i < 5); i++) await new Promise(r => setTimeout(r, 5));
  };
  return recorded;
}

/** Rosson's date-based key for some dates (the page's Date is set to each date in turn). */
function rossonKeys(rosson) {
  const w = rosson.window;
  const RealDate = w.Date;
  const api = rosson.get('ApiService');
  const keys = {};
  for (const day of ['2025-12-31', '2026-01-01', '2026-11-13', '2027-04-01', '2027-04-10']) {
    w.Date = class extends RealDate {
      constructor(...args) { if (args.length) super(...args); else super(`${day}T12:00:00`); }
    };
    keys[day] = api.rossonKey();
  }
  w.Date = RealDate;
  return keys;
}

const write = (path, value) => writeFileSync(path, `${JSON.stringify(value, null, 1)}\n`);

const shows = process.argv.slice(2).length ? process.argv.slice(2) : readdirSync(corpusDir).filter(d => /^\d{4}\.\d+$/.test(d)).sort();
for (const nr of shows) {
  const snapshot = JSON.parse(readFileSync(join(corpusDir, nr, 'snapshot.json'), 'utf8'));
  const out = join(corpusDir, nr, 'api');
  mkdirSync(out, { recursive: true });
  const rosson = bootRosson(vendorDir);
  if (nr === shows[0]) write(join(corpusDir, 'rosson-key.json'), rossonKeys(rosson));
  // Rosson stamps what it saves with the time (e.g. officials' updated_at, sent along): a fixed
  // time, so that the fixtures do not change from one run to the next.
  {
    const RealDate = rosson.window.Date;
    const fixed = Date.UTC(2026, 10, 13, 12);
    rosson.window.Date = class extends RealDate {
      constructor(...args) { if (args.length) super(...args); else super(fixed); }
      static now() { return fixed; }
    };
  }
  await loadEvent(snapshot, rosson); // as stored at the show
  rosson.get('ApiService').rossonKey = () => KEY;
  const requests = recordRequests(rosson.window);
  const PS = rosson.get('PruefungenService');
  const US = rosson.get('UploadService');
  const VS = rosson.get('VeranstaltungService');
  const APS = rosson.get('AusschreibungPruefungService');
  const record = async (file, action, run) => {
    requests.length = 0;
    try {
      await run();
      await requests.settle();
      write(join(out, file), { action, requests: [...requests] });
    } catch (e) {
      write(join(out, file), { action, rosson_failed: String(e?.stack || e).split('\n').slice(0, 4).join('\n') });
      console.log(`  ${file}: Rosson failed: ${String(e?.message || e).slice(0, 80)}`);
    }
  };

  // Results, class by class, as the results screen uploads them.
  let classes = 0;
  for (const p of snapshot.pruefungen) {
    if (!snapshot.resultate.some(r => String(r.pruefung_id) === String(p.id))) continue;
    const pruefung = PS.get(p.id);
    const name = String(p.nummer).replace(/[^\w.-]/g, '_');
    await record(`results-${name}.json`, `uploadResultate, class ${p.nummer}`, async () => {
      selectPruefung(rosson, pruefung);
      // Rosson's first ranking after loading can come back empty (its services finish loading
      // asynchronously); the results screen shows it again a moment later.
      let rangliste = [];
      for (let attempt = 0; attempt < 3 && !rangliste.length; attempt++) {
        await new Promise(r => setTimeout(r, 20));
        rangliste = rosson.get('RanglisteService').getRangliste(pruefung);
      }
      await US.uploadResultate(pruefung, rangliste);
    });
    classes++;
  }

  // Start lists, per published class.
  for (const ap of snapshot.ausschreibung_pruefungen.filter(a => snapshot.pruefungen.some(p => String(p.ausschreibung_pruefung_id) === String(a.id)))) {
    await record(`startlists-${ap.id}.json`, `uploadStartlisten, published class ${ap.id}`, async () => {
      await US.uploadStartlisten(VS.selected, APS.get(ap.id));
    });
  }

  await record('event.json', 'uploadVeranstaltung', async () => { await US.uploadVeranstaltung(VS.selected); });
  const firstPublished = snapshot.ausschreibung_pruefungen.find(a => snapshot.pruefungen.some(p => String(p.ausschreibung_pruefung_id) === String(a.id)));
  await record('delete-startlists.json', `deleteStartlisten, published class ${firstPublished.id}`, async () => { await US.deleteStartlisten(APS.get(firstPublished.id)); });

  // One request of each kind as sent, with the URL and headers (the first show only).
  if (nr === shows[0]) {
    const kinds = new Map();
    for (const r of requests.raw) kinds.set(`${r.method} ${r.url.split(/[/?]/)[1]}`, kinds.get(`${r.method} ${r.url.split(/[/?]/)[1]}`) || r);
    write(join(corpusDir, 'http-send.json'), [...kinds.values()]);
  }
  rosson.close();
  console.log(`${nr}: ${classes} classes`);
}
