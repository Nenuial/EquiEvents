#!/usr/bin/env node
// Loading an event from the Swiss Equestrian API, Rosson's side, as fixtures for the Rust tests
// (crates/se-api, tests/load_contract.rs).
//
//   node src/load-fixtures.mjs [show ...]
//
// Past events cannot be fetched any more, so the responses are rebuilt from the corpus: what
// Rosson stored is what it received plus what it adds itself, so taking its own additions off
// gives back what Swiss Equestrian delivered (`delivered` below; Rosson's additions are in
// VeranstaltungService.get, nennungAnreichern, pferdeAnreichern, zuweisungErstellen and
// DatabaseFactory.put). What happened at the show is left out: entries made at the show,
// start numbers, "does not start" set by the office, teams' order. Except horses: entries come
// with the horse they were ridden with (see `asRidden`).
//
// Rosson 3.3.15 then loads the event into an empty database the way its dashboard does
// (getOffizielle and getStammdaten, then VeranstaltungService.get), answered by a stub, and what
// it stored is written next to the response. Per show, corpus/<show>/api/load.json:
//   requests   the requests Rosson made, in order (no request leaves the process)
//   response   the getVeranstaltung response
//   stored     what Rosson stored for the event: veranstaltungen, ausschreibung_pruefungen,
//              nennungen, equipen, offizielle_zuweisungen, pferde, personen
// getStammdaten and getOffizielle are answered with the snapshot's master data and officials
// (Rosson stores those as delivered), so they are not repeated here.
//
// Then an update, as the dashboard's update button fetches it (getVeranstaltung with
// `geaendert_nach`, VeranstaltungService.checkForUpdates): one of each kind of change Swiss
// Equestrian can send (`updateFor`). corpus/<show>/api/update.json: the requests, the response,
// the changes made, what Rosson stored afterwards that differs from load.json's (`stored`: new
// and changed records), and the published classes Rosson shows.

import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { bootRosson } from './oracle/rosson.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const corpusDir = join(here, '../corpus');
const vendorDir = join(here, '../../vendor/rosson-3.3.15');
const ACCESS_KEY = '<access_key>';
const KEY = '<rosson_key>';

// Fields as Swiss Equestrian delivers them (the others are Rosson's own).
const API = {
  veranstaltung: ['id', 'nr', 'disziplin_code', 'von', 'bis', 'erste_titelzeile', 'zweite_titelzeile', 'dritte_titelzeile',
    'veranstalter', 'ort', 'postleitzahl', 'nennschluss', 'beteiligungen'],
  ausschreibung_pruefung: ['id', 'datum', 'nummer', 'index', 'disziplin_id', 'kategorie_id', 'kategorie_text', 'wertung_id',
    'wertung_text', 'springen_anzahl_stufen', 'equipe_anzahl_nennungen', 'kombipruefung_disziplinen', 'fahren_zwischenpruefungen',
    'fahren_freier_text', 'fahren_gespanne', 'fahren_lizenzen', 'fahren_nur_pony', 'bemerkungen', 'nenngeld1', 'nenngeld2',
    'status', 'geaendert_am', 'reservierte_plaetze', 'anzahl_veranstalter_startplaetze'],
  nennung: ['id', 'ausschreibung_pruefung_id', 'nenner_id', 'reiter_id', 'pferd_id', 'pferd2_id', 'pferd3_id', 'pferd4_id',
    'pferd5_id', 'pferd6_id', 'gastpferd_name', 'gwp_reiter', 'gwp_pferd', 'gwp_paar', 'stufe', 'equipe_id', 'fahren_gespann',
    'wagenbreite', 'hors_concours', 'programm_an_besitzer', 'bemerkungen', 'begleit_person', 'nenngeld_bezahlt', 'status',
    'programm_id', 'programm_name', 'erstellt_am', 'geaendert_am'],
  equipe: ['id', 'name', 'veranstaltung_id', 'ausschreibung_pruefung_id'],
  person: ['id', 'anrede', 'vorname', 'nachname', 'strasse', 'postleitzahl', 'ort', 'email', 'telefon_privat', 'telefon_mobil',
    'sprache', 'land', 'land_de', 'land_fr', 'land_it', 'land_en', 'lizenzen', 'lizenzen_status', 'nachwuchsreiter',
    'vereinsname', 'geboren_am', 'geaendert_am'],
};
// The stores Rosson fills from getStammdaten (dashboard.controller.js), and getOffizielle's.
const MASTER_DATA = ['aufgabegruende', 'disziplinen', 'pruefungsfunktionen', 'pruefungskategorien', 'programme', 'veranstalter',
  'veranstaltungsfunktionen', 'wertungen'];
const STORED = ['veranstaltungen', 'ausschreibung_pruefungen', 'nennungen', 'equipen', 'offizielle_zuweisungen', 'pferde', 'personen'];

// A rider's person record as Rosson copies it onto entries (nennungAnreichern), and a horse's
// (pferdeAnreichern): delivered field <- field on the entry.
const RIDER = { vorname: 'reiter_vorname', nachname: 'reiter_nachname', strasse: 'reiter_strasse', postleitzahl: 'reiter_plz',
  ort: 'reiter_ort', land: 'reiter_land', telefon_privat: 'telefon_privat', telefon_mobil: 'telefon_mobil', email: 'email',
  geboren_am: 'geboren_am', nachwuchsreiter: 'nachwuchsreiter', lizenzen: 'lizenzen', vereinsname: 'vereinsname' };
const HORSE = { name: 'pferdename', signalement_label: 'pferd_signalement', besitzer: 'pferd_besitzer', mitbesitzer: 'pferd_mitbesitzer',
  zuechter: 'pferd_zuechter', hat_pony_startbewilligung: 'hat_pony_startbewilligung', pony_groessenkategorie: 'pony_groessenkategorie',
  vater_name: 'vater_name', mutter_name: 'mutter_name', vater_der_mutter_name: 'vater_der_mutter_name' };

const pick = (record, fields) => Object.fromEntries(fields.filter(f => f in record).map(f => [f, record[f]]));
const strip = (rows, fields) => rows.map(r => Object.fromEntries(Object.entries(r).filter(([k]) => !fields.includes(k))));

const HORSE_FIELDS = ['pferd_id', 'pferd2_id', 'pferd3_id', 'pferd4_id', 'pferd5_id', 'pferd6_id'];

/**
 * An entry as delivered, with the horses it was ridden with: Rosson's change of horse at the
 * show updates the horses written on the entry (`pferde`) but not `pferd_id`, and the horse
 * first entered is often not known any more.
 */
function asRidden(n) {
  const out = pick(n, API.nennung);
  (n.pferde || []).forEach((p, i) => { out[HORSE_FIELDS[i]] = p.gastpferd ? 5 : p.pferd_id; });
  if (n.pferde?.length) out.gastpferd_name = n.pferde[0].gastpferd ? n.pferde[0].pferdename : null;
  return out;
}

/** The getVeranstaltung response the stored event was loaded from. */
export function delivered(snapshot) {
  const stored = snapshot.veranstaltungen[0];
  // Entries made at the show have no Swiss Equestrian creation time.
  const nennungen = snapshot.nennungen.filter(n => n.erstellt_am);
  const latestFirst = [...nennungen].sort((a, b) => String(b.geaendert_am).localeCompare(String(a.geaendert_am)));

  // People: riders as copied onto their entries, officials as Rosson kept them on the event.
  const personen = new Map();
  for (const n of latestFirst) {
    if (personen.has(n.reiter_id)) continue;
    const person = { id: n.reiter_id };
    for (const [to, from] of Object.entries(RIDER)) if (from in n) person[to] = n[from];
    for (const l of ['de', 'fr', 'it', 'en']) if (n.reiter_laender) person[`land_${l}`] = n.reiter_laender[l];
    personen.set(n.reiter_id, person);
  }
  // (Rosson keeps officials from earlier loads; only those the event still lists are delivered.)
  const officials = new Set((stored.beteiligungen || []).map(b => Number(b.person_id)));
  for (const p of stored.offizielle_personen || []) {
    if (!personen.has(p.id) && officials.has(Number(p.id))) personen.set(p.id, pick(p, API.person));
  }

  // Horses: as copied onto entries (the guest horse is Rosson's own placeholder).
  const pferde = new Map();
  for (const n of latestFirst) {
    for (const p of n.pferde || []) {
      if (p.gastpferd || p.pferd_id === 5 || pferde.has(p.pferd_id)) continue;
      const pferd = { id: p.pferd_id };
      for (const [to, from] of Object.entries(HORSE)) if (from in p) pferd[to] = p[from];
      pferde.set(p.pferd_id, pferd);
    }
  }

  const byId = (a, b) => a.id - b.id;
  return {
    data: {
      veranstaltung: {
        ...pick(stored, API.veranstaltung),
        ausschreibung_pruefungen: snapshot.ausschreibung_pruefungen.map(a => pick(a, API.ausschreibung_pruefung)).sort(byId),
        nennungen: nennungen.map(asRidden).sort(byId),
        nennungen_fa: [],
        equipen: snapshot.equipen.map(e => pick(e, API.equipe)).sort(byId),
      },
      personen: [...personen.values()].sort(byId),
      pferde: [...pferde.values()].sort(byId),
    },
    letzte_aenderung: stored.letzte_aenderung,
  };
}

/** The master data and officials responses, from what Rosson stored. */
function masterData(snapshot) {
  const stammdaten = Object.fromEntries(MASTER_DATA.map(s => [s, strip(snapshot[s] || [], ['updated_at', 'searchterms'])]));
  return {
    getStammdaten: { data: { stammdaten: { ...stammdaten, etiketten_vorlagen: [] } } },
    getOffizielle: { data: { offizielle: strip(snapshot.offizielle || [], ['updated_at', 'searchterms']) } },
  };
}

/** Answer the page's requests from `responses` (by path) and record them. */
function answerRequests(w, responses) {
  const recorded = [];
  // The requests as sent (URL, headers), for crates/se-api's HTTP layer.
  recorded.raw = [];
  let pending = 0;
  class Responder {
    constructor() { this.headers = {}; this.readyState = 0; this.status = 0; this.responseText = ''; this.response = ''; this.statusText = ''; }
    open(method, url) { this.method = method; this.url = url; this.readyState = 1; }
    setRequestHeader(name, value) { this.headers[name] = value; }
    getAllResponseHeaders() { return 'content-type: application/json'; }
    getResponseHeader(name) { return name.toLowerCase() === 'content-type' ? 'application/json' : null; }
    abort() {}
    send(body) {
      const url = new URL(this.url);
      const path = url.pathname.replace(/^\/api\/veranstaltersoftware\/v2/, '');
      recorded.raw.push({ method: this.method, url: this.url.replace(/^https:\/\/[^/]+\/api\/veranstaltersoftware\/v2/, ''), headers: { ...this.headers } });
      recorded.push({
        method: this.method,
        path,
        query: Object.fromEntries([...url.searchParams].map(([k, v]) => [k, k === 'rosson_key' ? KEY : v])),
        body: body ? JSON.parse(body) : null,
      });
      const answer = responses[path.replace(/^\//, '')];
      pending++;
      w.setTimeout(() => {
        this.readyState = 4;
        this.status = answer ? 200 : 404;
        this.statusText = answer ? 'OK' : 'Not Found';
        this.responseText = this.response = JSON.stringify(answer || {});
        pending--;
        this.onload?.(); this.onreadystatechange?.(); this.onloadend?.();
      }, 0);
    }
  }
  w.XMLHttpRequest = Responder;
  recorded.settle = async () => {
    for (let i = 0; i < 200 && (pending > 0 || i < 5); i++) await new Promise(r => setTimeout(r, 5));
  };
  return recorded;
}

/** Fixed time and a seeded Math.random, so ids and stamps are the same on every run. */
function deterministic(w) {
  const RealDate = w.Date;
  const fixed = Date.UTC(2026, 10, 13, 12);
  w.Date = class extends RealDate {
    constructor(...args) { if (args.length) super(...args); else super(fixed); }
    static now() { return fixed; }
  };
  let seed = 42;
  w.Math.random = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
}

/**
 * An update Swiss Equestrian could send after the first delivery: one of each kind of change,
 * on entries picked in id order. Changed entries get a newer change time, except one (Rosson
 * ignores it). Returns the response and the list of changes.
 */
export function updateFor(response) {
  const r = JSON.parse(JSON.stringify(response));
  const v = r.data.veranstaltung;
  const later = '2026-11-13T10:00:00+01:00';
  const changes = [];
  const delivered = new Map();
  const deliver = (n, what, bump = true) => {
    if (bump) n.geaendert_am = later;
    delivered.set(n.id, n);
    changes.push({ entry: n.id, what });
  };
  const usedRiders = new Set();
  const plain = v.nennungen.filter(n => n.status === 'definitiv' && n.pferd_id && n.pferd_id !== 5 && !n.gastpferd_name && !n.equipe_id && !n.pferd2_id);
  const take = () => {
    const n = plain.find(n => !delivered.has(n.id) && !usedRiders.has(n.reiter_id));
    usedRiders.add(n.reiter_id);
    return n;
  };

  const withdrawn = take(); withdrawn.status = 'abgemeldet'; deliver(withdrawn, 'withdrawn (abgemeldet)');
  const horse = take(); horse.pferd_id = r.data.pferde.find(p => p.id !== horse.pferd_id).id; deliver(horse, 'another horse');
  const rider = take(); rider.reiter_id = plain.find(n => !usedRiders.has(n.reiter_id)).reiter_id; usedRiders.add(rider.reiter_id); deliver(rider, 'another rider');
  const guest = take(); guest.pferd_id = 5; guest.gastpferd_name = 'GASTPFERD TEST'; deliver(guest, 'a guest horse');
  const points = take(); points.gwp_reiter = (points.gwp_reiter || 0) + 10; points.gwp_paar = (points.gwp_paar || 0) + 10; deliver(points, 'winning points');
  const fee = take(); fee.nenngeld_bezahlt = (fee.nenngeld_bezahlt || 0) + 5; deliver(fee, 'entry fee paid');
  const companion = take(); companion.begleit_person = 'Begleitperson'; deliver(companion, 'accompanying person');
  const quiet = take(); quiet.status = 'abgemeldet'; deliver(quiet, 'withdrawn, without a newer change time', false);
  const ignored = take(); ignored.hors_concours = true; ignored.stufe = 2; deliver(ignored, 'hors concours and level');
  const back = v.nennungen.find(n => !delivered.has(n.id) && n.status === 'abgemeldet' && n.pferd_id && n.pferd_id !== 5 && !n.equipe_id && !n.gastpferd_name);
  back.status = 'definitiv'; deliver(back, 'confirmed again after a withdrawal');

  // A person's and a horse's record change; an entry of each changes too (Rosson rewrites the
  // copies on changed entries only).
  const moved = take(); const person = r.data.personen.find(p => p.id === moved.reiter_id); person.ort = 'Neuenstadt';
  deliver(moved, "rider's town changed in the person record");
  const sold = take(); const pferd = r.data.pferde.find(p => p.id === sold.pferd_id); pferd.besitzer = 'Neue Besitzerin, Bern';
  deliver(sold, "horse's owner changed in the horse record");

  // A new entry, a new class with an entry, a class changed.
  const first = v.ausschreibung_pruefungen[0];
  const template = plain[plain.length - 1];
  const maxEntry = Math.max(...v.nennungen.map(n => n.id));
  const newEntry = { ...template, id: maxEntry + 1, ausschreibung_pruefung_id: first.id, erstellt_am: later, geaendert_am: later };
  v.nennungen.push(newEntry); deliver(newEntry, 'new entry', false);
  const newClass = { ...first, id: Math.max(...v.ausschreibung_pruefungen.map(a => a.id)) + 1, nummer: '99', geaendert_am: later };
  v.ausschreibung_pruefungen.push(newClass); changes.push({ class: newClass.id, what: 'new class' });
  const inNewClass = { ...template, id: maxEntry + 2, ausschreibung_pruefung_id: newClass.id, erstellt_am: later, geaendert_am: later };
  v.nennungen.push(inNewClass); deliver(inNewClass, 'new entry in the new class', false);
  first.kategorie_text = `${first.kategorie_text} (neu)`;
  first.wertung_text = 'Neue Wertung';
  first.springen_anzahl_stufen = 3;
  changes.push({ class: first.id, what: 'category text, scoring text, levels' });

  // Officials: one changed, one new.
  v.beteiligungen[0].anzahl_einsatztage = '2.0';
  changes.push({ official: v.beteiligungen[0].id, what: 'days of work' });
  const role = v.beteiligungen[v.beteiligungen.length - 1].veranstaltungsfunktion_id;
  const official = { id: Math.max(...v.beteiligungen.map(b => b.id)) + 1, veranstaltungsfunktion_id: role, person_id: template.reiter_id, nummer: null, anzahl_einsatztage: '1.0', verantwortlicher: true };
  v.beteiligungen.push(official); changes.push({ official: official.id, what: 'new official' });

  // Teams: one renamed, one new, a rider moved to another team.
  if (v.equipen.length >= 2) {
    v.equipen[0].name = `${v.equipen[0].name} (neu)`; changes.push({ team: v.equipen[0].id, what: 'renamed' });
    const newTeam = { ...v.equipen[0], id: Math.max(...v.equipen.map(e => e.id)) + 1, name: 'Neue Equipe' };
    v.equipen.push(newTeam); changes.push({ team: newTeam.id, what: 'new team' });
    const member = v.nennungen.find(n => !delivered.has(n.id) && n.equipe_id === v.equipen[1].id && n.pferd_id && n.pferd_id !== 5);
    member.equipe_id = newTeam.id; deliver(member, 'moved to the new team');
  }

  // What the update delivers: the changed entries, everything else in full; the people and
  // horses of the changed entries and the officials.
  const entries = [...delivered.values()];
  const people = new Set([...entries.map(n => n.reiter_id), ...v.beteiligungen.map(b => b.person_id), person.id]);
  const horses = new Set([...entries.flatMap(n => HORSE_FIELDS.map(f => n[f])).filter(h => h && h !== 5), pferd.id]);
  return {
    response: {
      data: {
        veranstaltung: { ...pick(v, API.veranstaltung), ausschreibung_pruefungen: v.ausschreibung_pruefungen, nennungen: entries, nennungen_fa: [], equipen: v.equipen },
        personen: r.data.personen.filter(p => people.has(p.id)),
        pferde: r.data.pferde.filter(p => horses.has(p.id)),
      },
      letzte_aenderung: '2026-11-13 10:00:00',
    },
    changes,
  };
}

const write = (path, value) => writeFileSync(path, `${JSON.stringify(value, null, 1)}\n`);

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  const shows = process.argv.slice(2).length ? process.argv.slice(2) : readdirSync(corpusDir).filter(d => /^\d{4}\.\d+$/.test(d)).sort();
  for (const nr of shows) {
    const snapshot = JSON.parse(readFileSync(join(corpusDir, nr, 'snapshot.json'), 'utf8'));
    const response = delivered(snapshot);
    const rosson = bootRosson(vendorDir);
    deterministic(rosson.window);
    rosson.get('ApiService').rossonKey = () => KEY;
    const responses = { ...masterData(snapshot), getVeranstaltung: response };
    const requests = answerRequests(rosson.window, responses);

    // As the dashboard loads a new event (dashboard.controller.js, loadVeranstaltung).
    const dashboard = rosson.get('$controller')('DashboardController', { $stateParams: {} });
    await Promise.all([dashboard.getOffizielle(ACCESS_KEY), dashboard.getStammdaten(ACCESS_KEY)]);
    await rosson.get('VeranstaltungService').get(ACCESS_KEY);
    // Rosson does not wait for the entries and teams it stores; wait until they are all there.
    const db = rosson.get('DatabaseFactory');
    const expected = response.data.veranstaltung.nennungen.length;
    for (let i = 0; i < 400; i++) {
      const [n, e] = [(await db.getAll('nennungen')).length, (await db.getAll('equipen')).length];
      if (n >= expected && e >= response.data.veranstaltung.equipen.length) break;
      await new Promise(r => setTimeout(r, 10));
    }
    await requests.settle();

    const stored = {};
    for (const store of STORED) stored[store] = rosson.fromPage(await db.getAll(store)).sort((a, b) => String(a.id).localeCompare(String(b.id), 'en', { numeric: true }));
    const out = join(corpusDir, nr, 'api');
    mkdirSync(out, { recursive: true });
    write(join(out, 'load.json'), {
      action: 'loading the event: getOffizielle and getStammdaten, then getVeranstaltung (dashboard, VeranstaltungService.get)',
      requests: [...requests],
      response,
      stored,
    });

    // The update, as the dashboard's update button does it (dashboard.controller.js, checkForUpdates).
    const update = updateFor(response);
    responses.getVeranstaltung = update.response;
    requests.length = 0;
    const [event] = await db.getAll('veranstaltungen');
    await dashboard.checkForUpdates({ stopPropagation() {} }, event);
    for (let i = 0; i < 100; i++) {
      const all = await db.getAll('nennungen');
      if (update.response.data.veranstaltung.nennungen.every(n => all.some(s => s.id === n.id && s.geaendert_am === n.geaendert_am))) break;
      await new Promise(r => setTimeout(r, 10));
    }
    await requests.settle();
    // Only what the update changed (new or different records); the rest is as after loading.
    const updated = {};
    for (const store of STORED) {
      const before = new Map(stored[store].map(r => [r.id, JSON.stringify(r)]));
      updated[store] = rosson.fromPage(await db.getAll(store))
        .filter(r => before.get(r.id) !== JSON.stringify(r))
        .sort((a, b) => String(a.id).localeCompare(String(b.id), 'en', { numeric: true }));
    }
    const visible = (await rosson.get('AusschreibungPruefungService').loadAusschreibungPruefungen(event.id)).map(a => a.id).sort((a, b) => a - b);
    write(join(out, 'update.json'), {
      action: 'updating the event: getOffizielle and getStammdaten, then getVeranstaltung with geaendert_nach (dashboard, VeranstaltungService.checkForUpdates)',
      requests: [...requests],
      changes: update.changes,
      response: update.response,
      stored: updated,
      classes_shown: visible,
    });

    // The requests as sent, with the URL and headers (the first show only).
    if (nr === shows[0]) write(join(corpusDir, 'http-get.json'), requests.raw);
    rosson.close();
    console.log(`${nr}: ${expected} entries, ${response.data.personen.length} people, ${response.data.pferde.length} horses, ${stored.offizielle_zuweisungen.length} officials`);
  }
}
