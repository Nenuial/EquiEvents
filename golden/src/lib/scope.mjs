// Cut one event out of a Rosson export, keeping only what the scoring and ranking code needs.
//
// Event-owned stores are filtered by `veranstaltung_id`. Reference stores (scoring tables,
// dressage tests, disciplines, ...) are public Swiss Equestrian master data and are kept as they are,
// except the national lists of officials and organisers, which are cut down to the
// entries this event actually references.

const EVENT_STORES = [
  'ausschreibung_pruefungen',
  'pruefungen',
  'zwischenpruefungen',
  'nennungen',
  'starter',
  'equipen',
  'resultate',
  'zwischenresultate',
  'finalpruefungen',
  'offizielle_zuweisungen',
];

const REFERENCE_STORES = [
  'disziplinen',
  'wertungen',
  'programme',
  'aufgabegruende',
  'pruefungsfunktionen',
  'veranstaltungsfunktionen',
];

// Deliberately dropped:
//   log                  copies of Prüfung records, not needed for scoring
//   benachrichtigungen   app notifications
//   pferde, alle_personen  national registers (Rosson never exports them anyway)
//   downloads, error_log, etiketten_vorlagen, geolocation  local UI state

const byId = rows => [...rows].sort((a, b) => String(a.id).localeCompare(String(b.id), 'en', { numeric: true }));

export function eventsIn(data) {
  return (data.veranstaltungen || []).map(v => ({ id: v.id, nr: v.nr }));
}

/** Facts used to pick the most complete snapshot of an event among several exports. */
export function snapshotStats(data, eventId) {
  const own = store => (data[store] || []).filter(r => r.veranstaltung_id === eventId);
  const resultate = own('resultate');
  const pruefungen = own('pruefungen');
  const touched = [...resultate, ...pruefungen, ...own('starter')].map(r => Number(r.updated_at) || 0);
  return {
    resultate: resultate.length,
    starter: own('starter').length,
    pruefungen: pruefungen.length,
    definitiv: pruefungen.filter(p => p.definitiv).length,
    lastChange: touched.length ? Math.max(...touched) : 0,
  };
}

export function scopeEvent(data, eventId) {
  const veranstaltung = (data.veranstaltungen || []).find(v => v.id === eventId);
  if (!veranstaltung) throw new Error(`event ${eventId} not in export`);

  const out = { veranstaltungen: [veranstaltung] };
  for (const store of EVENT_STORES) {
    out[store] = byId((data[store] || []).filter(r => r.veranstaltung_id === eventId));
  }
  for (const store of REFERENCE_STORES) {
    out[store] = byId(data[store] || []);
  }

  // Prüfungskategorien: only those used by this event's classes.
  const kategorien = new Set(out.ausschreibung_pruefungen.map(a => a.kategorie_id));
  out.pruefungskategorien = byId((data.pruefungskategorien || []).filter(k => kategorien.has(k.id)));

  // Officials: only people assigned to this event (assignments or event roles).
  const personen = new Set([
    ...out.offizielle_zuweisungen.map(z => z.person_id),
    ...(veranstaltung.beteiligungen || []).map(b => b.person_id),
  ]);
  out.offizielle = byId((data.offizielle || []).filter(o => personen.has(o.id)));

  // Organisers: only the one named on the event.
  out.veranstalter = byId((data.veranstalter || []).filter(v => v.name === veranstaltung.veranstalter));

  // Per-event UI settings Rosson keeps in localStorage (some affect prints, e.g. layout options).
  const local = (data.localstore || {})[eventId];
  out.localstore = local ? { [eventId]: local } : {};

  return out;
}
