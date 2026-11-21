//! Makes the practice event (`crates/office/training/practice.json`): « Concours d'exercice »,
//! a show to learn and try EquiEvents on, never synced (decided by the user, 2026-11-15).
//!
//! Its classes and entries are those of real shows (the golden corpus: classes, categories,
//! tests, tables, entries, teams, levels), over three days, with every person, horse, owner,
//! breeder, team, organiser and place invented. It is kept as Swiss Equestrian's three answers
//! (`getStammdaten`, `getOffizielle`, `getVeranstaltung`), so it loads through the same code as
//! a real event; its dates follow the day it is made (`office::training`).
//!
//! `cargo run -p office --example practice_event` writes the file and checks that no name of the
//! real shows is in it; `tests/training.rs` checks the file is what this makes.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use serde_json::{json, Map, Value};

/// Which classes of which show, on which day (0, 1, 2) of the practice event.
const PICKED: [(&str, &[(&str, usize)]); 3] = [
    ("2026.8352", &[("1", 0), ("5", 0), ("6", 0), ("7", 1), ("8", 2), ("12", 2)]),
    ("2026.8298", &[("11", 0), ("1", 1), ("5", 1), ("7", 1), ("12", 1)]),
    ("2026.8299", &[("1", 2), ("2", 2), ("4", 2)]),
];
/// The practice event's days before they follow the day it is made (a Friday to a Sunday).
const DAYS: [&str; 3] = ["2026-01-02", "2026-01-03", "2026-01-04"];
const EVENT_ID: u64 = 1;

const FIRST: [&str; 48] = [
    "Aline", "Bastien", "Camille", "Damien", "Elodie", "Fabien", "Gaëlle", "Hugo", "Inès", "Jonas", "Katia", "Loïc", "Maëlle", "Nathan", "Océane", "Pascale", "Quentin", "Romane", "Simon", "Tania", "Ugo",
    "Valérie", "Yann", "Zoé", "Anna", "Beat", "Claudia", "Daniel", "Eva", "Florian", "Gabriela", "Hans", "Irene", "Jürg", "Karin", "Lukas", "Monika", "Nina", "Oliver", "Petra", "Reto", "Sandra", "Thomas",
    "Ursula", "Vreni", "Walter", "Chiara", "Matteo",
];
const LAST: [&str; 48] = [
    "Exemple", "Essai", "Modèle", "Pratique", "Démo", "Rêne", "Leçon", "Manège", "Carrière", "Paddock", "Chandelier", "Oxer", "Rivière", "Volte", "Galop", "Trot", "Licol", "Sellier",
    "Musterli", "Probelauf", "Übungsberg", "Beispiel", "Trainer", "Reitbahn", "Sattelkammer", "Stallgasse", "Hufeisen", "Weidezaun", "Koppel", "Halfter", "Steigbügel", "Zaumzeug", "Esempio", "Prova",
    "Maneggio", "Galoppo", "Briglia", "Staffa", "Sella", "Cavezza", "Ostacolo", "Percorso", "Pratico", "Lezione", "Campo", "Prato", "Fieno", "Avena",
];
const HORSE_FIRST: [&str; 40] = [
    "Alto", "Brio", "Cador", "Danseur", "Etoile", "Flocon", "Galant", "Hermine", "Indigo", "Jasmin", "Kalimba", "Lumière", "Mirage", "Nuage", "Opaline", "Pépite", "Quasar", "Rubis", "Saphir",
    "Tilleul", "Utopie", "Vanille", "Wapiti", "Xylo", "Yuzu", "Zénith", "Aubépine", "Bourrasque", "Cerise", "Dragée", "Embrun", "Fougère", "Grenadine", "Houblon", "Ivoire", "Jonquille", "Kiwi",
    "Lavande", "Mandarine", "Noisette",
];
const HORSE_LAST: [&str; 10] = [" de l'Essai", " du Pré-Vert", " des Exercices", " de la Démo", " du Manège", " CH", " II", " Z", " vom Übungshof", " della Prova"];
/// Clubs and the horses' parents, invented as well.
const CLUBS: [&str; 12] = [
    "Club équestre de l'Essai", "Société de cavalerie de la Démo", "Manège de l'Exemple", "Cercle hippique du Pré-Vert", "Reitverein Übungsberg", "Kavallerieverein Beispiel", "Reitclub Probelauf",
    "Società ippica La Prova", "Club hippique des Exercices", "Ecurie du Paddock", "Reitsportverein Muster", "Manège de la Leçon",
];
const PARENTS: [&str; 12] = ["Grand Essai", "Vieux Modèle", "Bel Exemple", "Noble Démo", "Haut Pratique", "Fier Galop", "Doux Trot", "Vif Paddock", "Brave Volte", "Sage Licol", "Alter Muster", "Vecchia Prova"];
const TEAMS: [&str; 12] = ["Les Essais", "Les Exemples", "Les Barres", "Les Oxers", "Les Voltes", "Les Paddocks", "Übungsteam", "Die Probe", "Gli Esempi", "Les Galops", "Les Licols", "Les Trotteurs"];
const TOWNS: [(&str, &str); 6] = [("1000", "Lausanne"), ("2000", "Neuchâtel"), ("1700", "Fribourg"), ("3000", "Bern"), ("1950", "Sion"), ("6900", "Lugano")];
const REMARKS: [&str; 6] = ["Box avec paddock svp", "Arrivée le matin même", "Cheval sensible au bruit", "Bitte Box neben Stallkollege", "Départ tôt si possible", "Partenza nel pomeriggio"];

fn snapshot(root: &Path, show: &str) -> Value {
    let text = std::fs::read_to_string(root.join(format!("golden/corpus/{show}/snapshot.json"))).unwrap_or_else(|e| panic!("{show}: {e}"));
    serde_json::from_str(&text).unwrap()
}

fn id(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

fn ascii(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'ï' | 'î' => 'i',
            'ô' | 'ö' => 'o',
            'ü' | 'û' | 'ù' => 'u',
            'ç' => 'c',
            c => c,
        })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect::<String>()
        .to_lowercase()
}

/// Invented names, the same for the same id every time.
struct Names {
    people: HashMap<u64, (u64, String, String)>,
    horses: HashMap<u64, (u64, String)>,
    /// Clubs and the horses' parents, by their real name.
    clubs: HashMap<String, String>,
    parents: HashMap<String, String>,
}

impl Names {
    fn person(&mut self, old: u64) -> (u64, String, String) {
        if let Some(p) = self.people.get(&old) {
            return p.clone();
        }
        // Every pair of a first and a last name once, in a fixed shuffled order (37 and 48 × 48
        // have no common factor).
        let n = self.people.len();
        let pairs = FIRST.len() * LAST.len();
        assert!(n < pairs, "more people than names");
        let k = n * 37 % pairs;
        let (first, last) = (FIRST[k / LAST.len()], LAST[k % LAST.len()]);
        let p = (900_001 + n as u64, first.to_string(), last.to_string());
        self.people.insert(old, p.clone());
        p
    }

    fn horse(&mut self, old: u64) -> (u64, String) {
        if let Some(h) = self.horses.get(&old) {
            return h.clone();
        }
        let n = self.horses.len();
        let round = n / (HORSE_FIRST.len() * HORSE_LAST.len());
        let name = format!("{}{}{}", HORSE_FIRST[n % HORSE_FIRST.len()], HORSE_LAST[(n / HORSE_FIRST.len()) % HORSE_LAST.len()], if round > 0 { format!(" {}", round + 1) } else { String::new() });
        let h = (800_001 + n as u64, name);
        self.horses.insert(old, h.clone());
        h
    }

    fn club(&mut self, real: &str) -> String {
        if real.trim().is_empty() {
            return String::new();
        }
        let n = self.clubs.len();
        self.clubs.entry(real.to_string()).or_insert_with(|| format!("{}{}", CLUBS[n % CLUBS.len()], if n >= CLUBS.len() { format!(" {}", n / CLUBS.len() + 1) } else { String::new() })).clone()
    }

    fn parent(&mut self, real: &str) -> String {
        if real.trim().is_empty() {
            return String::new();
        }
        let n = self.parents.len();
        self.parents.entry(real.to_string()).or_insert_with(|| format!("{} {}", PARENTS[n % PARENTS.len()], n / PARENTS.len() + 1)).clone()
    }

    /// Someone who is not a rider or an official here (an owner, a breeder).
    fn other(&mut self, text: &str) -> String {
        if text.trim().is_empty() {
            return String::new();
        }
        let key = 7_000_000_000 + text.bytes().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64)) % 1_000_000_000;
        let (_, f, l) = self.person(key);
        format!("{f} {l}")
    }
}

/// The three answers, from the picked classes, every name invented; and the original names (to
/// check none is left).
pub fn make(root: &Path) -> (Value, BTreeSet<String>) {
    let mut names = Names { people: HashMap::new(), horses: HashMap::new(), clubs: HashMap::new(), parents: HashMap::new() };
    let mut originals: BTreeSet<String> = BTreeSet::new();
    let mut remember = |s: &Value| {
        if let Some(s) = s.as_str().map(str::trim).filter(|s| s.chars().count() >= 4) {
            originals.insert(s.to_lowercase());
        }
    };

    let mut stammdaten: BTreeMap<String, BTreeMap<u64, Value>> = BTreeMap::new();
    let mut offizielle: BTreeMap<u64, Value> = BTreeMap::new();
    let mut classes: Vec<(usize, Value)> = Vec::new();
    let mut entries: Vec<Value> = Vec::new();
    let mut teams: Vec<Value> = Vec::new();
    let mut personen: BTreeMap<u64, Value> = BTreeMap::new();
    let mut pferde: BTreeMap<u64, Value> = BTreeMap::new();
    let mut beteiligungen: Vec<Value> = Vec::new();

    for (show, picked) in PICKED {
        let snap = snapshot(root, show);
        // Master data: Swiss Equestrian's, the latest of each record.
        let master = rosson_import::delivered::master_data(&snap);
        for (table, rows) in master["data"]["stammdaten"].as_object().unwrap() {
            for row in rows.as_array().into_iter().flatten() {
                if let Some(i) = id(&row["id"]) {
                    stammdaten.entry(table.clone()).or_default().insert(i, row.clone());
                }
            }
            stammdaten.entry(table.clone()).or_default();
        }
        let delivered = rosson_import::delivered::event(&snap);
        let event = &delivered["data"]["veranstaltung"];
        for k in ["erste_titelzeile", "zweite_titelzeile", "dritte_titelzeile", "ort", "veranstalter"] {
            remember(&event[k]);
        }
        for p in delivered["data"]["personen"].as_array().into_iter().flatten() {
            remember(&json!(format!("{} {}", p["vorname"].as_str().unwrap_or(""), p["nachname"].as_str().unwrap_or(""))));
            remember(&p["nachname"]);
        }
        for h in delivered["data"]["pferde"].as_array().into_iter().flatten() {
            for k in ["name", "besitzer", "mitbesitzer", "zuechter"] {
                remember(&h[k]);
            }
        }
        for o in rosson_import::delivered::officials(&snap)["data"]["offizielle"].as_array().into_iter().flatten() {
            remember(&json!(format!("{} {}", o["vorname"].as_str().unwrap_or(""), o["nachname"].as_str().unwrap_or(""))));
            remember(&o["nachname"]);
            let (new, first, last) = names.person(id(&o["id"]).unwrap());
            let (plz, town) = TOWNS[new as usize % TOWNS.len()];
            let mut o = o.clone();
            o["id"] = json!(new);
            o["vorname"] = json!(first);
            o["nachname"] = json!(last);
            o["strasse"] = json!(format!("Chemin de l'Exercice {}", new % 90 + 1));
            o["postleitzahl"] = json!(plz);
            o["ort"] = json!(town);
            o["email"] = json!(format!("{}.{}@example.com", ascii(&first), ascii(&last)));
            o["telefon_mobil"] = json!(format!("+41 79 000 {:02} {:02}", new % 100, new / 100 % 100));
            o["telefon_privat"] = json!("");
            o["telefon_arbeit"] = json!("");
            offizielle.insert(new, o);
        }
        for t in event["equipen"].as_array().into_iter().flatten() {
            remember(&t["name"]);
        }

        let by_number: HashMap<String, &Value> = event["ausschreibung_pruefungen"].as_array().unwrap().iter().map(|a| (a["nummer"].as_str().unwrap().to_string(), a)).collect();
        let picked_ids: BTreeSet<u64> = picked.iter().map(|(n, _)| id(&by_number[*n]["id"]).unwrap()).collect();
        for (number, day) in picked {
            classes.push((*day, by_number[*number].clone()));
        }
        for n in event["nennungen"].as_array().unwrap() {
            if picked_ids.contains(&id(&n["ausschreibung_pruefung_id"]).unwrap_or(0)) {
                entries.push(n.clone());
            }
        }
        for t in event["equipen"].as_array().into_iter().flatten() {
            if picked_ids.contains(&id(&t["ausschreibung_pruefung_id"]).unwrap_or(0)) {
                teams.push(t.clone());
            }
        }
        let riders: BTreeSet<u64> = entries.iter().flat_map(|n| [id(&n["reiter_id"]), id(&n["nenner_id"])]).flatten().collect();
        for p in delivered["data"]["personen"].as_array().into_iter().flatten() {
            let old = id(&p["id"]).unwrap();
            let official = event["beteiligungen"].as_array().into_iter().flatten().any(|b| id(&b["person_id"]) == Some(old));
            if !riders.contains(&old) && !official {
                continue;
            }
            let (new, first, last) = names.person(old);
            let (plz, town) = TOWNS[new as usize % TOWNS.len()];
            let mut p = p.clone();
            p["id"] = json!(new);
            p["vorname"] = json!(first);
            p["nachname"] = json!(last);
            p["strasse"] = json!(format!("Route de la Démo {}", new % 90 + 1));
            p["postleitzahl"] = json!(plz);
            p["ort"] = json!(town);
            p["email"] = json!(format!("{}.{}@example.com", ascii(&first), ascii(&last)));
            p["telefon_mobil"] = json!(format!("+41 79 000 {:02} {:02}", new % 100, new / 100 % 100));
            p["telefon_privat"] = json!("");
            if let Some(club) = p.get("vereinsname").and_then(Value::as_str).map(str::to_string) {
                p["vereinsname"] = json!(names.club(&club));
            }
            if p.get("geboren_am").is_some_and(|b| !b.is_null()) {
                let young = p["nachwuchsreiter"] == true;
                p["geboren_am"] = json!(format!("{}-0{}-1{}", if young { 2009 + new % 8 } else { 1970 + new % 30 }, new % 9 + 1, new % 9));
            }
            personen.insert(new, p);
        }
        let horses: BTreeSet<u64> = entries.iter().flat_map(|n| ["pferd_id", "pferd2_id", "pferd3_id", "pferd4_id", "pferd5_id", "pferd6_id"].map(|f| id(&n[f]))).flatten().collect();
        for h in delivered["data"]["pferde"].as_array().into_iter().flatten() {
            let old = id(&h["id"]).unwrap();
            if !horses.contains(&old) {
                continue;
            }
            let (new, name) = names.horse(old);
            let mut h = h.clone();
            h["id"] = json!(new);
            h["name"] = json!(name);
            for k in ["besitzer", "mitbesitzer", "zuechter"] {
                let invented = names.other(h[k].as_str().unwrap_or(""));
                if h.get(k).is_some() {
                    h[k] = json!(invented);
                }
            }
            for k in ["vater_name", "mutter_name", "vater_der_mutter_name"] {
                let invented = names.parent(h[k].as_str().unwrap_or(""));
                if h.get(k).is_some() {
                    h[k] = json!(invented);
                }
            }
            pferde.insert(new, h);
        }
        for b in event["beteiligungen"].as_array().into_iter().flatten() {
            let mut b = b.clone();
            b["person_id"] = json!(names.person(id(&b["person_id"]).unwrap()).0);
            beteiligungen.push(b);
        }
    }

    // The classes, numbered over the days; their ids, the entries' and the teams' made new.
    classes.sort_by_key(|(day, _)| *day);
    let mut class_ids: HashMap<u64, u64> = HashMap::new();
    let published: Vec<Value> = classes
        .iter()
        .enumerate()
        .map(|(i, (day, a))| {
            let new = 9_001 + i as u64;
            class_ids.insert(id(&a["id"]).unwrap(), new);
            let mut a = a.clone();
            a["id"] = json!(new);
            a["nummer"] = json!((i + 1).to_string());
            a["index"] = json!(i + 1);
            let time = a["datum"].as_str().and_then(|d| d.get(10..)).unwrap_or("").to_string();
            a["datum"] = json!(format!("{}{time}", DAYS[*day]));
            a["bemerkungen"] = json!("Épreuve d'exercice, selon règlement. Übungsprüfung, gemäss Reglement.");
            if a.get("fahren_freier_text").is_some() {
                a["fahren_freier_text"] = json!("");
            }
            a
        })
        .collect();
    let mut team_ids: HashMap<u64, u64> = HashMap::new();
    let teams: Vec<Value> = teams
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let new = 7_001 + i as u64;
            team_ids.insert(id(&t["id"]).unwrap(), new);
            let mut t = t.clone();
            t["id"] = json!(new);
            t["veranstaltung_id"] = json!(EVENT_ID);
            t["ausschreibung_pruefung_id"] = json!(class_ids[&id(&t["ausschreibung_pruefung_id"]).unwrap()]);
            t["name"] = json!(format!("{} {}", TEAMS[i % TEAMS.len()], i / TEAMS.len() + 1));
            t
        })
        .collect();
    let entries: Vec<Value> = entries
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut n = n.clone();
            n["id"] = json!(600_001 + i as u64);
            n["ausschreibung_pruefung_id"] = json!(class_ids[&id(&n["ausschreibung_pruefung_id"]).unwrap()]);
            for f in ["reiter_id", "nenner_id"] {
                if let Some(old) = id(&n[f]) {
                    n[f] = json!(names.person(old).0);
                }
            }
            for f in ["pferd_id", "pferd2_id", "pferd3_id", "pferd4_id", "pferd5_id", "pferd6_id"] {
                if let Some(old) = id(&n[f]).filter(|h| *h != se_api::load::PLACEHOLDER_HORSE) {
                    n[f] = json!(names.horse(old).0);
                }
            }
            if let Some(t) = id(&n["equipe_id"]) {
                n["equipe_id"] = json!(team_ids.get(&t).copied());
            }
            if n["gastpferd_name"].is_string() {
                n["gastpferd_name"] = json!(format!("Invité{}", HORSE_LAST[i % HORSE_LAST.len()]));
            }
            n["bemerkungen"] = json!(if i % 17 == 3 { REMARKS[i % REMARKS.len()] } else { "" });
            if n.get("begleit_person").is_some() {
                n["begleit_person"] = json!("");
            }
            n
        })
        .collect();

    // The organiser, invented; master data as Swiss Equestrian has it.
    stammdaten.insert("veranstalter".into(), BTreeMap::from([(1, json!({ "id": 1, "name": "Société hippique de l'Exercice", "regionalverband": "AVSH" }))]));
    let stammdaten: Map<String, Value> = stammdaten.into_iter().map(|(table, rows)| (table, Value::Array(rows.into_values().collect()))).collect();
    let mut stammdaten = stammdaten;
    stammdaten.entry("etiketten_vorlagen").or_insert(json!([]));

    let event = json!({
        "id": EVENT_ID,
        "nr": "EXERCICE",
        "disziplin_code": ["CS", "CD", "CA"],
        "von": DAYS[0],
        "bis": DAYS[2],
        "erste_titelzeile": "CONCOURS D'EXERCICE EQUIEVENTS",
        "zweite_titelzeile": "Pour apprendre et essayer · Zum Lernen und Ausprobieren · Per imparare e provare",
        "dritte_titelzeile": "Société hippique de l'Exercice",
        "veranstalter": "Société hippique de l'Exercice",
        "ort": "Les Prés-d'Exercice",
        "postleitzahl": "1000",
        "nennschluss": null,
        "beteiligungen": beteiligungen,
        "ausschreibung_pruefungen": published,
        "nennungen": entries,
        "nennungen_fa": [],
        "equipen": teams,
    });
    let practice = json!({
        "getStammdaten": { "data": { "stammdaten": stammdaten } },
        "getOffizielle": { "data": { "offizielle": offizielle.into_values().collect::<Vec<_>>() } },
        "getVeranstaltung": { "data": { "veranstaltung": event, "personen": personen.into_values().collect::<Vec<_>>(), "pferde": pferde.into_values().collect::<Vec<_>>() } },
    });
    (practice, originals)
}

/// The original names found in the practice event (none, or it is not written).
pub fn leaks(practice: &Value, originals: &BTreeSet<String>) -> Vec<String> {
    // Only what names people, horses and places: master data is Swiss Equestrian's own.
    let mut text = String::new();
    for part in [&practice["getOffizielle"], &practice["getVeranstaltung"]] {
        text.push_str(&part.to_string().to_lowercase());
    }
    originals
        .iter()
        .filter_map(|o| {
            let at = text.find(o.as_str())?;
            let mut from = at.saturating_sub(40);
            while !text.is_char_boundary(from) {
                from -= 1;
            }
            let mut to = (at + o.len() + 40).min(text.len());
            while !text.is_char_boundary(to) {
                to += 1;
            }
            Some(format!("{o}: …{}…", &text[from..to]))
        })
        .collect()
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (practice, originals) = make(&root);
    let found = leaks(&practice, &originals);
    if !found.is_empty() {
        eprintln!("{} names of the real shows are in the practice event: {found:?}", found.len());
        std::process::exit(1);
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("training/practice.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, serde_json::to_string(&practice).unwrap() + "\n").unwrap();
    let event = &practice["getVeranstaltung"]["data"];
    println!(
        "{}: {} classes, {} entries, {} riders and officials, {} horses; {} original names checked",
        path.display(),
        event["veranstaltung"]["ausschreibung_pruefungen"].as_array().unwrap().len(),
        event["veranstaltung"]["nennungen"].as_array().unwrap().len(),
        event["personen"].as_array().unwrap().len(),
        event["pferde"].as_array().unwrap().len(),
        originals.len()
    );
}
