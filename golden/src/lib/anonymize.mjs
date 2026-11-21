// Remove non-public personal data from an event snapshot before it goes into the corpus.
//
// Rule (from Swiss Equestrian): everything published on info.swiss-equestrian.ch is public.
// That covers rider names and hometowns, canton, nationality, clubs, horse names and
// signalement, owners, breeders, team names, custom print fields, officials' names,
// prize money and all results. Those are kept, so fixtures can be compared one-to-one
// with the published result lists. Horse pedigree is not personal data and is kept too.
//
// Removed, because it is not published (or not needed, so minimised):
//   - riders' street, email, phone numbers, birth date (Rosson only prints it; youth status
//     comes separately as `nachwuchsreiter`)
//   - officials' and event contacts' street, email, phone numbers (incl. the duty phone
//     stored as `beteiligungen[].nummer`)
//   - organisers' free-text remarks on entries, accompanying persons, salutations
//   - the event access key (it unlocks the non-public data on the Swiss Equestrian API)
// Dropped for size only: logos (base64 images) and search-term indexes.
//
// Rosson copies the full entry (nennung) into results, teams and Prüfungen, so the rules
// apply by field name to every nested object rather than per store.

const MIN_SECRET_LEN = 4;

export function makeAnonymizer() {
  /** Removed original values; none of them may appear anywhere in the output. */
  const secrets = new Map(); // value -> field it came from
  const isEmpty = v => v === null || v === undefined || v === '';
  const isClassDefinition = o => 'kategorie_id' in o && 'disziplin_id' in o; // public Ausschreibung text

  const remove = (field, replacement) => v => {
    if (isEmpty(v)) return v;
    if (typeof v === 'string' && v.trim().length >= MIN_SECRET_LEN) secrets.set(v.trim(), field);
    return typeof replacement === 'function' ? replacement(v) : replacement;
  };

  const rules = {
    // Contact data (riders, officials, event contacts)
    strasse: remove('strasse', 'Strasse 1'),
    reiter_strasse: remove('reiter_strasse', 'Strasse 1'),
    email: remove('email', 'entfernt@example.invalid'),
    telefon_privat: remove('telefon_privat', '+41 00 000 00 00'),
    telefon_mobil: remove('telefon_mobil', '+41 00 000 00 00'),
    telefon_arbeit: remove('telefon_arbeit', '+41 00 000 00 00'),
    geboren_am: remove('geboren_am', null),
    nummer: (v, o) => ('veranstaltungsfunktion_id' in o ? remove('beteiligung.nummer', '+41 00 000 00 00')(v) : v),
    anrede: remove('anrede', ''),
    begleit_person: remove('begleit_person', 'Begleitperson'),

    // Free text written by organisers about entries (the class description stays: it is public)
    bemerkungen: (v, o) => (isClassDefinition(o) ? v : remove('bemerkungen', t => `[${String(t).length} Zeichen entfernt]`)(v)),
    kommentar: remove('kommentar', t => `[${String(t).length} Zeichen entfernt]`),

    // Secrets
    access_key: remove('access_key', 'REDACTED'),

    // Size only
    logo: v => (isEmpty(v) ? v : null),
    searchterms: () => [],
  };

  function walk(node) {
    if (Array.isArray(node)) return node.map(walk);
    if (!node || typeof node !== 'object') return node;
    const out = {};
    for (const [key, value] of Object.entries(node)) {
      out[key] = rules[key] ? rules[key](value, node) : walk(value);
    }
    return out;
  }

  return { anonymize: walk, secrets };
}

// Public fields a removed value may legitimately reappear in: an organiser remark that is
// just a rider's or horse's name, the team or a club is public text, not a leak.
const PUBLIC_FIELDS = new Set([
  // people and horses as published in start and result lists
  'vorname', 'nachname', 'reiter_vorname', 'reiter_nachname', 'sortable_name', 'reiter_ort', 'ort',
  'pferdename', 'gastpferd_name', 'pferd_besitzer', 'pferd_mitbesitzer', 'pferd_zuechter',
  'vater_name', 'mutter_name', 'vater_der_mutter_name', 'pferd_signalement', 'vereinsname',
  // event, class and master-data texts
  'name', 'titel', 'kurztitel', 'veranstalter', 'erste_titelzeile', 'zweite_titelzeile', 'dritte_titelzeile',
  'kategorie_text', 'wertung_text', 'platzpflege_text', 'programm_json', 'bewertung_json',
  'bezeichnung_de', 'bezeichnung_fr', 'bezeichnung_it',
]);

/**
 * Check that no removed value survives in the output, as a case-insensitive substring of
 * any string, except inside the public fields above. Returns findings as {field, at, length}
 * without the values, so the report is safe to print. Must be empty before committing.
 */
export function leakCheck(output, secrets) {
  const needles = [...secrets].map(([value, field]) => ({ value: value.toLowerCase(), field }));
  const findings = new Map();
  (function visit(node, path, key) {
    if (typeof node === 'string') {
      if (PUBLIC_FIELDS.has(key)) return;
      const text = node.toLowerCase();
      for (const n of needles) {
        if (text.includes(n.value)) findings.set(`${n.field}@${path}`, { field: n.field, at: path, length: n.value.length });
      }
    } else if (Array.isArray(node)) node.forEach(x => visit(x, `${path}[]`, key));
    else if (node && typeof node === 'object') {
      for (const [k, x] of Object.entries(node)) visit(x, /^\d+$|_\d{6,}_/.test(k) ? `${path}.<id>` : `${path}.${k}`, k);
    }
  })(output, '', '');
  return [...findings.values()];
}
