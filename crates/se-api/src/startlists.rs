//! `putStartlisten`: the start lists of a published class (all its sections), as Rosson uploads
//! them (`UploadService.uploadStartlisten`, `StartlisteService.getStartlistenDaten`).
//!
//! The lines come from [`show::startlist`]; this module writes them in Rosson's shape. A few words
//! appear in the office's language, as Rosson writes them in its interface language: "Stufe 2",
//! "Pause 10:00 - 10:30", "Richter C", "Programm: L12/60".

use serde_json::{json, Map, Value};
use show::model::*;
use show::startlist::Line;
use show::State;

use crate::{Keys, Method, Request};

/// The office's language, for the words Rosson writes into uploads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    De,
    Fr,
    It,
}

impl Language {
    /// `de`, `fr`, `it`.
    pub fn code(self) -> &'static str {
        match self {
            Language::De => "de",
            Language::Fr => "fr",
            Language::It => "it",
        }
    }
}

/// A word as Rosson 3.3.15 writes it (`src/renderer/app/i18n`). Rosson has no Italian for some
/// of them and falls back to German.
pub fn word(language: Language, key: &str) -> &'static str {
    match (key, language) {
        ("RICHTER", Language::De) => "Richter",
        ("RICHTER", Language::Fr) => "Juge",
        ("RICHTER", Language::It) => "Giudice",
        ("STUFE", Language::Fr) => "Degré",
        ("STUFE", _) => "Stufe",
        ("PROGRAMM", Language::Fr) => "Programme",
        ("PROGRAMM", _) => "Programm",
        ("PAUSE", _) => "Pause",
        ("GESPANN_KURZ", Language::Fr) => "Attel",
        ("GESPANN_KURZ", _) => "Sp",
        ("KEIN_PROGRAMM", Language::Fr) => "Pas de programme",
        ("KEIN_PROGRAMM", _) => "Kein Programm",
        // Eventing's phases (Rosson's DISZIPLIN_KEYS; Italian: ours).
        ("CD", Language::De) => "Dressur",
        ("CD", _) => "Dressage",
        ("CS", Language::De) => "Springen",
        ("CS", Language::Fr) => "Saut",
        ("CS", Language::It) => "Salto",
        ("CROSS", _) => "Cross",
        // Driving's phases (Rosson's DISZIPLINEN; the open phase as the rulebook names it).
        ("GE", Language::De) => "Gelände",
        ("GE", Language::Fr) => "Terrain",
        ("GE", Language::It) => "Terreno",
        ("HI", Language::De) => "Kegelfahren",
        ("HI", Language::Fr) => "Maniabilité",
        ("HI", Language::It) => "Coni",
        ("DE", _) => "Derby",
        ("OF", Language::De) => "Spezialprüfung",
        ("OF", Language::Fr) => "Épreuve spéciale",
        ("OF", Language::It) => "Prova speciale",
        ("MT", Language::Fr) => "Multi-test",
        ("MT", _) => "Multitest",
        _ => "",
    }
}

/// `YYYY-MM-DD` → `DD.MM.YYYY`.
fn swiss_date(date: &str) -> String {
    let parts: Vec<&str> = date.get(..10).unwrap_or(date).split('-').collect();
    match parts.as_slice() {
        [y, m, d] => format!("{d}.{m}.{y}"),
        _ => date.to_string(),
    }
}

/// The upload of a published class's start lists (all its sections, in their order).
pub fn put_startlisten(state: &State, published: &PublishedClassId, keys: &Keys, language: Language) -> Result<Request, String> {
    // Eventing: the phases' lists (the class made of them has no starters of its own).
    let mut classes: Vec<&Class> = state.classes.values().filter(|c| &c.published == published && c.phases.is_empty()).collect();
    classes.sort_by_key(|c| (c.position.unwrap_or(u32::MAX), c.number.clone()));
    let lists = classes.iter().map(|c| start_list(state, c, language)).collect::<Result<Vec<_>, _>>()?;
    let mut body = keys.envelope();
    body["ausschreibung_pruefung_id"] = json!(published.0);
    body["startlisten"] = Value::Array(lists);
    Ok(Request { method: Method::Post, path: "/putStartlisten".into(), query: Vec::new(), body: Some(body) })
}

/// One section's start list (`getStartlisteDaten`).
fn start_list(state: &State, class: &Class, language: Language) -> Result<Value, String> {
    let list = state.start_list(&class.id)?;
    let published = state.published_classes.get(&class.published).ok_or("no published class")?;
    let starts: Vec<&Start> = state.starts.get(&class.id).into_iter().flatten().filter(|s| state.entries.contains_key(&s.entry)).collect();
    let entries: Vec<&Entry> = starts.iter().map(|s| &state.entries[&s.entry]).collect();
    let dressage = state.class_discipline(class) == "CD";
    let individual = class.programme.and_then(|p| state.programmes.get(&p)).is_some_and(show::master::is_individual);

    // Extra lines, and for dressage the judges.
    let mut extra: Vec<Value> = class.extra_fields.iter().flatten().map(|f| json!({ "key": f.key, "value": f.value })).collect();
    if dressage {
        for judge in state.dressage_judges(&class.id) {
            let person = state.persons.get(&judge.person).map_or("-".to_string(), |p| format!("{} {}, {}", p.first_name, p.last_name, p.town));
            extra.push(json!({ "key": format!("{} {}", word(language, "RICHTER"), judge.letter.clone().unwrap_or_default()), "value": person }));
        }
    }

    let mut kopf = Map::new();
    kopf.insert("position".into(), json!(class.position));
    kopf.insert("veranstaltung_id".into(), json!(state.event.as_ref().map(|e| e.id.0)));
    kopf.insert("ausschreibung_pruefung_id".into(), json!(class.published.0));
    kopf.insert("rosson_startliste_id".into(), json!(class.id.0));
    // Eventing: "12 Dressur", "12 Cross", as Rosson names a phase's list.
    let number = match &class.phase {
        Some(p) => format!("{} {}", class.number, word(language, p.kind.discipline())),
        None => class.number.clone(),
    };
    kopf.insert("nummer".into(), json!(number));
    kopf.insert("name".into(), json!(class.name));
    kopf.insert("datum".into(), json!(swiss_date(&class.date)));
    let time = match &class.start_list.layout {
        StartListLayout::StartTimes { first, .. } => first.to_string(),
        _ => class.time_text.clone(),
    };
    kopf.insert("uhrzeit".into(), json!(time));
    if let Some(logo) = &class.logo {
        kopf.insert("logo".into(), logo.clone());
    }
    kopf.insert("hat_startnummern".into(), json!(list.start_numbers));
    if let Some(table) = class.scoring_table {
        kopf.insert("wertung_id".into(), json!(table));
    }
    if let Some(text) = &class.scoring_text {
        kopf.insert("wertung_text".into(), json!(text));
    }
    let programme_name = match class.programme {
        None => String::new(),
        Some(p) => state.programmes.get(&p).map_or(word(language, "KEIN_PROGRAMM").to_string(), |p| show::master::programme_title(p, language.code())),
    };
    kopf.insert("programm_name".into(), json!(programme_name));
    kopf.insert("anzahl_starts".into(), json!(entries.len()));
    kopf.insert("anzahl_hc".into(), json!(entries.iter().filter(|e| e.hors_concours).count()));
    kopf.insert("anzahl_ns".into(), json!(entries.iter().filter(|e| e.status == EntryStatus::NotStarting).count()));
    kopf.insert("weitere_felder".into(), Value::Array(extra));
    kopf.insert("hat_startzeiten".into(), json!(list.start_times));
    kopf.insert("kategorie_text".into(), json!(published.category_text));

    let blocks = matches!(class.start_list.layout, StartListLayout::Blocks { .. });
    let mut zeilen = Vec::new();
    for (i, line) in list.lines.iter().enumerate() {
        let reihenfolge = i + 1;
        let text = |t: String| json!({ "typ": "text", "reihenfolge": reihenfolge, "text": t });
        zeilen.push(match line {
            Line::Starter { entry, position, time } => starter_line(state, class, entry, reihenfolge, *position, *time, individual, language),
            Line::Team { team, time } => {
                let t = state.teams.get(team);
                let mut z = json!({ "typ": "text", "reihenfolge": reihenfolge, "text": t.map(|t| t.name.clone()).unwrap_or_default() });
                if let Some(n) = t.and_then(|t| t.start_number) {
                    z["startnummer"] = json!(n);
                }
                if !blocks {
                    z["startzeit"] = json!(time.map_or("--:--".to_string(), |t| t.to_string()));
                }
                z
            }
            Line::Block(t) | Line::Inserted(t) => text(t.clone()),
            Line::Level(level) => text(format!("{} {level}", word(language, "STUFE"))),
            Line::Break(from, to) => text(format!("{} {from} - {to}", word(language, "PAUSE"))),
        });
    }
    kopf.insert("zeilen".into(), Value::Array(zeilen));
    Ok(Value::Object(kopf))
}

#[allow(clippy::too_many_arguments)]
fn starter_line(state: &State, class: &Class, entry_id: &EntryId, reihenfolge: usize, position: u32, time: Option<TimeOfDay>, individual: bool, language: Language) -> Value {
    let entry = &state.entries[entry_id];
    let start = state.starts.get(&class.id).and_then(|s| s.iter().find(|s| &s.entry == entry_id));
    let person = state.persons.get(&entry.rider);
    let (first, last, town, club) = match (&entry.rider_on_entry, person) {
        (Some(r), _) => (r.first_name.clone(), r.last_name.clone(), r.town.clone(), r.club.clone()),
        (None, Some(p)) => (p.first_name.clone(), p.last_name.clone(), p.town.clone(), p.club.clone()),
        _ => Default::default(),
    };
    // Driving: only the horses that start in the phase (Rosson's `getPferdeMitEinsatz`).
    let phase = class.phase.as_ref().map(|p| p.kind).filter(|_| state.driving(class));
    let horses: Vec<Value> = entry
        .horses_on_entry
        .iter()
        .filter(|h| phase.is_none_or(|k| show::model::horses_in_phase(entry, k).contains(h)))
        .map(|h| {
            let owners: Vec<&str> = [&h["pferd_besitzer"], &h["pferd_mitbesitzer"]].into_iter().filter_map(|v| v.as_str()).filter(|s| !s.is_empty()).collect();
            let mut horse = Map::new();
            // Fields the delivered record does not have are left out, as Rosson does.
            for (to, from) in [("pferd_id", "pferd_id"), ("name", "pferdename"), ("pony_groessenkategorie", "pony_groessenkategorie"), ("signalement", "pferd_signalement")] {
                if let Some(v) = h.get(from) {
                    horse.insert(to.into(), v.clone());
                }
            }
            horse.insert("besitzer".into(), json!(owners.join(" / ")));
            if let Some(v) = h.get("pferd_zuechter") {
                horse.insert("zuechter".into(), v.clone());
            }
            Value::Object(horse)
        })
        .collect();

    // Further information: the level (team classes), the chosen programme.
    let mut more: Vec<String> = Vec::new();
    if !class.teams.is_empty() {
        if let Some(level) = entry.stage.filter(|l| *l != 0) {
            more.push(format!("{}: {level}", word(language, "STUFE")));
        }
    }
    // Driving: the driver's F licences and the turnout, "FB,FL / 2 Sp" (Rosson's `getWeiteres`).
    if phase.is_some() {
        let held = entry.record["lizenzen"].as_str().map(str::to_string).or_else(|| person.map(|p| p.licences.clone())).unwrap_or_default();
        let licences: Vec<&str> = held.split(',').map(str::trim).filter(|l| l.starts_with('F')).collect();
        let turnout = entry.record["fahren_gespann"].as_str().map(str::to_string).or_else(|| entry.record["fahren_gespann"].as_u64().map(|g| g.to_string())).filter(|g| !g.is_empty());
        let info: Vec<String> = [Some(licences.join(",")).filter(|l| !l.is_empty()), turnout.map(|g| format!("{g} {}", word(language, "GESPANN_KURZ")))].into_iter().flatten().collect();
        if !info.is_empty() {
            more.push(info.join(" / "));
        }
    }
    if individual {
        if let Some(p) = entry.programme.and_then(|p| state.programmes.get(&p)) {
            more.push(format!("{}: {}", word(language, "PROGRAMM"), p.name));
        }
    }

    let mut z = json!({
        "typ": "starter",
        "reihenfolge": reihenfolge,
        "start_position": position,
        "kein_start": entry.status == EntryStatus::NotStarting,
        "hors_concours": entry.hors_concours,
        "startzeit": time.map_or("--:--".to_string(), |t| t.to_string()),
        "reiter_id": entry.rider.0,
        "reiter_vorname": first,
        "reiter_nachname": last,
        "reiter_ort": town,
        "reiter_verein": club,
        "pferde": horses,
        "weiteres": more.join("\n"),
    });
    if let Some(n) = start.and_then(|s| s.start_number) {
        z["startnummer"] = json!(n);
    }
    z
}
