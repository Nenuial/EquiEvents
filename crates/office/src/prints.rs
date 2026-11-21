//! What the show office prints, as data for the templates of [`typeset`]: start lists, rankings,
//! dressage judges' sheets, rankings over several classes, the participants, the timetable.
//!
//! Everything a print says is decided here, in the office's language (French, German or Italian:
//! the Italian of each French text, `crate::italian`); the templates only lay it out. The content follows Rosson's prints (docs: `vendor/rosson-3.3.15`,
//! `services/print/*`); the layout is new.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde_json::{json, Value};
use show::model::*;
use show::State;
use typeset::Template;

use crate::views::{self, StartLine};

/// The office's language for prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Fr,
    De,
    It,
}

impl Language {
    pub fn of(code: &str) -> Language {
        match code {
            "de" => Language::De,
            "it" => Language::It,
            _ => Language::Fr,
        }
    }

    fn code(self) -> &'static str {
        match self {
            Language::Fr => "fr",
            Language::De => "de",
            Language::It => "it",
        }
    }

    /// Swiss Equestrian's own words, which come in three languages (Italian when it has one).
    pub(crate) fn pick_se<'a>(self, fr: &'a str, de: &'a str, it: &'a str) -> &'a str {
        match self {
            Language::It if !it.trim().is_empty() => it,
            Language::It => fr,
            _ => self.pick(fr, de),
        }
    }

    pub(crate) fn pick<'a>(self, fr: &'a str, de: &'a str) -> &'a str {
        match self {
            Language::Fr => fr,
            Language::De => de,
            Language::It => crate::italian::tr(fr, de),
        }
    }
}

/// A print, ready for [`typeset::render`].
pub struct Print {
    pub template: Template,
    pub data: Value,
    /// A file name for it, without extension.
    pub name: String,
}

/// The words of every print.
fn words(lang: Language) -> Value {
    let pairs: &[(&str, &str, &str)] = &[
        ("printed", "Imprimé le", "Gedruckt am"),
        ("page", "Page", "Seite"),
        ("time", "Heure", "Zeit"),
        ("number", "N°", "Nr."),
        ("rider", "Cavalier", "Reiter"),
        ("horse", "Cheval", "Pferd"),
        ("carried", "Qualif.", "Qualif."),
        ("rank", "Rang", "Rang"),
        ("class", "Épreuve", "Prüfung"),
        ("judge", "Juge", "Richter"),
        ("movement", "Mouvement", "Lektion"),
        ("mark", "Note", "Note"),
        ("remarks", "Remarques", "Bemerkungen"),
        ("penalties", "Pénalités", "Strafpunkte"),
        ("total", "Total", "Total"),
        ("signature", "Signature", "Unterschrift"),
        ("tie_break", "Départage", "Entscheid"),
        ("participants", "Participants", "Teilnehmer"),
        ("classes", "Épreuves", "Prüfungen"),
        ("schedule", "Horaire", "Zeitplan"),
        ("starters", "Partants", "Starter"),
        ("judges", "Juges", "Richter"),
        ("date", "Date", "Datum"),
        ("fee", "Finance d’inscription", "Nenngeld"),
        ("licence", "Licence", "Lizenz"),
        ("address", "Adresse", "Adresse"),
        ("phone", "Téléphone", "Telefon"),
        ("passport", "N° de passeport", "Pass-Nr."),
        ("signalement", "Signalement", "Signalement"),
        ("pedigree", "Origines", "Abstammung"),
        ("level", "Niveau", "Stufe"),
        ("seconds", "Sec.", "Sek."),
        ("points", "Points", "Punkte"),
        ("round_1", "1re manche", "1. Umgang"),
        ("round_2", "2e manche", "2. Umgang"),
        ("jump_off", "Barrage", "Stechen"),
        ("officials_plan", "Plan des officiels", "Einsatzplan der Offiziellen"),
        ("official", "Officiel", "Offizielle:r"),
        ("event_roles", "Au concours", "Am Turnier"),
    ];
    Value::Object(pairs.iter().map(|(k, fr, de)| (k.to_string(), json!(lang.pick(fr, de)))).collect())
}

const WEEKDAYS: [(&str, &str, &str); 7] = [
    ("dimanche", "Sonntag", "domenica"),
    ("lundi", "Montag", "lunedì"),
    ("mardi", "Dienstag", "martedì"),
    ("mercredi", "Mittwoch", "mercoledì"),
    ("jeudi", "Donnerstag", "giovedì"),
    ("vendredi", "Freitag", "venerdì"),
    ("samedi", "Samstag", "sabato"),
];
const MONTHS: [(&str, &str, &str); 12] = [
    ("janvier", "Januar", "gennaio"),
    ("février", "Februar", "febbraio"),
    ("mars", "März", "marzo"),
    ("avril", "April", "aprile"),
    ("mai", "Mai", "maggio"),
    ("juin", "Juni", "giugno"),
    ("juillet", "Juli", "luglio"),
    ("août", "August", "agosto"),
    ("septembre", "September", "settembre"),
    ("octobre", "Oktober", "ottobre"),
    ("novembre", "November", "novembre"),
    ("décembre", "Dezember", "dicembre"),
];

/// `2026-09-25` → "vendredi 25 septembre" / "Freitag, 25. September" / "venerdì 25 settembre"
/// (with the year if asked).
pub fn long_date(iso: &str, lang: Language, year: bool) -> String {
    let mut parts = iso.get(..10).unwrap_or(iso).split('-').map(|p| p.parse::<i32>().ok());
    let (Some(Some(y)), Some(Some(m)), Some(Some(d))) = (parts.next(), parts.next(), parts.next()) else { return iso.to_string() };
    if !(1..=12).contains(&m) {
        return iso.to_string();
    }
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y };
    let weekday = ((yy + yy / 4 - yy / 100 + yy / 400 + t[(m - 1) as usize] + d) % 7) as usize;
    let (wd, month) = (WEEKDAYS[weekday], MONTHS[(m - 1) as usize]);
    let year = if year { format!(" {y}") } else { String::new() };
    match lang {
        Language::Fr => format!("{} {d} {}{year}", wd.0, month.0),
        Language::De => format!("{}, {d}. {}{year}", wd.1, month.1),
        Language::It => format!("{} {d} {}{year}", wd.2, month.2),
    }
}

/// What every print starts with: its title, language, when, the event.
fn base(state: &State, lang: Language, title: &str, printed: &str) -> Value {
    let event = state.event.as_ref();
    let dates = event.map(|e| if e.from == e.to { long_date(&e.from, lang, true) } else { format!("{} – {}", long_date(&e.from, lang, false), long_date(&e.to, lang, true)) }).unwrap_or_default();
    json!({
        "title": title,
        "language": lang.code(),
        "printed": printed,
        "labels": words(lang),
        "event": {
            // The first title line; the others repeat the place and dates, or name the organiser.
            "title": event.and_then(|e| e.title.first().cloned()).unwrap_or_default(),
            "place": event.map(|e| e.place.clone()).unwrap_or_default(),
            "dates": dates,
            "number": event.map(|e| e.number.clone()).unwrap_or_default(),
        },
    })
}

fn class<'a>(state: &'a State, id: &ClassId) -> Result<&'a Class, String> {
    state.classes.get(id).ok_or_else(|| format!("unknown class {id}"))
}

/// A role's name in the language.
fn role_name(state: &State, code: &str, lang: Language) -> String {
    state.class_roles().into_iter().find(|r| r.code == code).map(|r| lang.pick_se(&r.name_fr, &r.name_de, &r.name_it).to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| code.to_string())
}

fn person_name(state: &State, person: &PersonId) -> String {
    state.persons.get(person).map(|p| format!("{} {}", p.first_name, p.last_name)).or_else(|| state.officials.get(person).map(|o| format!("{} {}", o.first_name, o.last_name))).unwrap_or_else(|| format!("#{person}"))
}

/// A class's officials, by role: (role, names); dressage judges with their position.
/// A class's officials, by role; the judges at a position (dressage, driving: E H C M B…) last, a
/// line of their own. `positioned_only`: only them (every list but the judges' names them).
fn officials(state: &State, class: &ClassId, lang: Language, positioned_only: bool) -> Vec<(String, String)> {
    let mut by_role: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut judges: Vec<(String, String)> = Vec::new();
    for a in state.assignments.values().filter(|a| a.class.as_ref() == Some(class)) {
        let name = person_name(state, &a.person);
        match a.letter.as_deref().filter(|l| !l.trim().is_empty()) {
            Some(letter) => judges.push((letter.to_string(), name)),
            None if !positioned_only => by_role.entry(role_name(state, &a.role, lang)).or_default().push(name),
            None => {}
        }
    }
    judges.sort_by_key(|(letter, _)| "EHCMB".find(letter.as_str()).unwrap_or(9));
    let mut out: Vec<(String, String)> = by_role.into_iter().map(|(role, names)| (role, names.join(", "))).collect();
    if !judges.is_empty() {
        out.push((lang.pick("Juges", "Richter").to_string(), judges.iter().map(|(l, n)| format!("{l} {n}")).collect::<Vec<_>>().join(", ")));
    }
    out
}

/// A class's head: number, name, and what it is.
fn class_head(state: &State, c: &Class, lang: Language) -> Value {
    let published = state.published_classes.get(&c.published);
    let mut info: Vec<(String, String)> = vec![(lang.pick("Date", "Datum").into(), long_date(&c.date, lang, false))];
    if !c.time_text.is_empty() {
        info.push((lang.pick("Heure", "Zeit").into(), c.time_text.clone()));
    }
    if let Some(p) = published {
        info.push((lang.pick("Catégorie", "Kategorie").into(), format!("{} {}", p.discipline, p.category_text)));
    }
    if let Some(scoring) = c.scoring_text.clone().or_else(|| published.and_then(|p| p.scoring_text.clone())).filter(|s| !s.is_empty()) {
        info.push((lang.pick("Barème", "Wertung").into(), scoring));
    }
    // The ideal-time table: the ideal time (S20).
    if let Some(ideal) = c.options.ideal_time {
        info.push((lang.pick("Temps idéal", "Idealzeit").into(), crate::prepare::time_text(ideal)));
    }
    if let Some(p) = c.programme.and_then(|p| state.programmes.get(&p)) {
        info.push((lang.pick("Programme", "Programm").into(), format!("{} {}", p.name, show::master::programme_title(p, lang.code()))));
    }
    // The class's own lines (sponsors, prizes): after the rest; the judges' list sets them apart.
    let details = info.len();
    if let Some(extra) = &c.extra_fields {
        info.extend(extra.iter().map(|f| (f.key.clone(), f.value.clone())));
    }
    // A phase (eventing, driving): which, before the class's name.
    let name = match c.phase.as_ref() {
        Some(p) => format!("{} – {}", phase_name(p.kind, lang), crate::views::class_title(state, c)),
        None => crate::views::class_title(state, c),
    };
    let pairs: Vec<Value> = info.into_iter().map(|(k, v)| json!([k, v])).collect();
    json!({
        "number": c.number,
        "name": name,
        "details": pairs[..details],
        "extra": pairs[details..],
        "info": pairs,
        // On every list only the judges at a position (dressage, driving); on the judges' all of them.
        "officials": officials(state, &c.id, lang, true).into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
        "all_officials": officials(state, &c.id, lang, false).into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
        // A data URL (Rosson's), laid out by `typeset` as an image.
        "logo": c.logo.as_ref().and_then(|l| l.as_str()).filter(|l| l.starts_with("data:image")),
    })
}

/// The horse's second line: owner(s), signalement, pony size; the pedigree if asked, on a line of
/// its own: sire, dam, dam's sire, and the breeder when Swiss Equestrian lists one.
fn horse_details(entry: &Entry, horse: Option<&Horse>, pedigree: bool, lang: Language) -> String {
    let record = entry.horses_on_entry.first();
    let field = |k: &str| record.and_then(|r| r[k].as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let mut parts: Vec<String> = Vec::new();
    let owner = field("pferd_besitzer").or_else(|| horse.map(|h| h.owner.clone()).filter(|o| !o.is_empty()));
    if let Some(owner) = owner {
        parts.push(match field("pferd_mitbesitzer") {
            Some(co) => format!("{owner} / {co}"),
            None => owner,
        });
    }
    if let Some(s) = field("pferd_signalement").or_else(|| horse.map(|h| h.signalement.clone()).filter(|s| !s.is_empty())) {
        parts.push(s);
    }
    if let Some(pony) = field("pony_groessenkategorie") {
        parts.push(format!("Pony {pony}"));
    }
    let details = parts.join(" · ");
    if !pedigree {
        return details;
    }
    let (sire, dam, dam_sire) = (field("vater_name"), field("mutter_name"), field("vater_der_mutter_name"));
    let breeder = field("pferd_zuechter").or_else(|| horse.map(|h| h.breeder.clone()).filter(|b| !b.is_empty()));
    let mut line: Vec<String> = Vec::new();
    if sire.is_some() || dam.is_some() || dam_sire.is_some() {
        let name = |n: Option<String>| n.unwrap_or_else(|| "–".into());
        line.push(format!("{} {}", lang.pick("Père", "Vater"), name(sire)));
        line.push(format!("{} {}", lang.pick("Mère", "Mutter"), name(dam)));
        line.push(format!("{} {}", lang.pick("Père de la mère", "Muttervater"), name(dam_sire)));
    }
    if let Some(breeder) = breeder {
        line.push(format!("{} {breeder}", lang.pick("Éleveur", "Züchter")));
    }
    [details, line.join(" · ")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n")
}

fn rider_details(_state: &State, _entry: &Entry, town: &str, club: &str) -> String {
    [town, club].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
}

/// A rider's name, with " (J)" for juniors and " (Y)" for young riders, as Rosson prints them
/// (Swiss Equestrian's age category `nachwuchsreiter`: E, J, Y, S).
fn rider_name(state: &State, entry: &Entry, name: &str) -> String {
    match state.persons.get(&entry.rider).map(|p| p.youth.trim()) {
        Some(category @ ("J" | "Y")) => format!("{name} ({category})"),
        _ => name.to_string(),
    }
}

/// Start lists of classes, one after the other.
/// Which start list (Rosson's: the start list, with pedigree; "lite"; for the judges).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartListKind {
    /// Rider and horse on two lines each, with their details (pedigree if asked).
    Full { pedigree: bool },
    /// One short line a starter: number, horse, signalement, winning points, rider, owner.
    Lite,
    /// For the judges: a column for each starter's result, and the counts and the judge's visa to
    /// fill in.
    Judges,
}

pub fn start_lists(state: &State, classes: &[ClassId], lang: Language, pedigree: bool, printed: &str) -> Result<Print, String> {
    start_lists_as(state, classes, lang, StartListKind::Full { pedigree }, printed)
}

pub fn start_lists_as(state: &State, classes: &[ClassId], lang: Language, kind: StartListKind, printed: &str) -> Result<Print, String> {
    let pedigree = kind == StartListKind::Full { pedigree: true };
    let mut out = Vec::new();
    for id in classes {
        let c = class(state, id)?;
        // As published: by name until start numbers are given.
        let list = views::published_start_list(state, id)?;
        let programmes = state.class_discipline(c) == "CD" && c.programme.is_none();
        // Driving (Rosson's driving start lists): the phase's horses, the F licences and turnout,
        // the carriage width on the jury's list; the marathon's phase B start.
        let driving = c.phase.as_ref().map(|p| p.kind).filter(|_| state.driving(c));
        let settings = c.options.driving.clone().unwrap_or_default();
        let b_after = (driving == Some(PhaseKind::Marathon)).then(|| {
            let limit = settings.marathon_limits().0.unwrap_or_default();
            let transfer = settings.transfer.unwrap_or_default() * Decimal::from(60);
            // Rounded up to the minute (Rosson's `auf_60_runden`).
            ((limit + transfer) / Decimal::from(60)).ceil().to_string().parse::<u32>().unwrap_or(0)
        });
        let lines: Vec<Value> = list
            .lines
            .iter()
            .map(|line| match line {
                StartLine::Starter { entry, position, start_number, time, rider, town, club, horse, not_starting, hors_concours, carried, carried_out, .. } => {
                    let e = &state.entries[&EntryId(*entry)];
                    let mut horse_sub = horse_details(e, e.horse.as_ref().and_then(|h| state.horses.get(h)), pedigree, lang);
                    if programmes {
                        if let Some(p) = e.programme.and_then(|p| state.programmes.get(&p)) {
                            horse_sub = [p.name.clone(), horse_sub].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
                        }
                    }
                    let mark = if *not_starting { lang.pick("NP", "NS") } else if *hors_concours { "hc" } else if e.rider_changed { "RW" } else { "" };
                    // For the short list: the parts on their own.
                    let record = e.horses_on_entry.first();
                    let field = |k: &str| record.and_then(|r| r[k].as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
                    let h = e.horse.as_ref().and_then(|h| state.horses.get(h));
                    let owner = field("pferd_besitzer").or_else(|| h.map(|h| h.owner.clone())).unwrap_or_default();
                    let owner = owner.split(',').next().unwrap_or("").trim().to_string();
                    let mut line = json!({
                        "kind": "starter", "position": position, "time": time, "number": start_number,
                        "rider": rider_name(state, e, rider), "rider_sub": rider_details(state, e, town, club),
                        "horse": horse, "horse_sub": horse_sub, "mark": mark, "not_starting": not_starting,
                        "carried": if *carried_out { Some("–".to_string()) } else { carried.map(|c| c.to_string()) },
                        "town": town, "owner": owner,
                        "signalement": field("pferd_signalement").or_else(|| h.map(|h| h.signalement.clone())).unwrap_or_default(),
                        "points": e.winning_points.pair.map(|p| p.normalize().to_string()).unwrap_or_default(),
                    });
                    if let Some(kind) = driving {
                        let horses = show::model::horses_in_phase(e, kind);
                        let text = |h: &Value, k: &str| h[k].as_str().map(str::trim).filter(|s| !s.is_empty()).unwrap_or("–").to_string();
                        // The name as shown (a guest horse's in capitals), else a dash.
                        let shown = |h: &Value| match show::model::horse_on_entry_name(h).trim() {
                            "" => "–".to_string(),
                            name => name.to_string(),
                        };
                        line["horse"] = json!(horses.iter().map(|h| shown(h)).collect::<Vec<_>>().join(" · "));
                        line["horse_sub"] = json!(horses
                            .iter()
                            .map(|h| {
                                let mut s = format!("{} ({})", shown(h), text(h, "pferd_signalement"));
                                if pedigree {
                                    s += &format!(" {} {} × {} · {} {}", lang.pick("par", "v."), text(h, "vater_name"), text(h, "vater_der_mutter_name"), lang.pick("éleveur", "Züchter"), text(h, "pferd_zuechter"));
                                }
                                s
                            })
                            .collect::<Vec<_>>()
                            .join("\n"));
                        let licences: Vec<String> = state.persons.get(&e.rider).map(|p| p.licences.split(',').map(str::trim).filter(|l| l.starts_with('F')).map(str::to_string).collect()).unwrap_or_default();
                        let turnout = e.record["fahren_gespann"].as_str().map(str::to_string).or_else(|| e.record["fahren_gespann"].as_u64().map(|g| g.to_string())).filter(|g| !g.is_empty());
                        line["licence"] = json!([Some(licences.join(",")).filter(|l| !l.is_empty()), turnout.map(|g| format!("{g}-{}", lang.pick("Attel", "Sp")))].into_iter().flatten().collect::<Vec<_>>().join(" / "));
                        line["width"] = json!(e.record["wagenbreite"].as_str().map(str::to_string).or_else(|| e.record["wagenbreite"].as_u64().map(|w| w.to_string())).unwrap_or_default());
                        if let (Some(after), Some(t)) = (b_after, time.as_deref().and_then(|t| t.split_once(':')).and_then(|(h, m)| Some(h.trim().parse::<u32>().ok()? * 60 + m.trim().parse::<u32>().ok()?))) {
                            let b = t + after;
                            line["time_b"] = json!(format!("{:02}:{:02} (B)", b / 60 % 24, b % 60));
                        }
                    }
                    line
                }
                StartLine::Team { name, time, .. } => json!({ "kind": "heading", "text": match time { Some(t) => format!("{name} · {t}"), None => name.clone() } }),
                StartLine::Text { level: Some(level), .. } => json!({ "kind": "heading", "text": format!("{} {level}", lang.pick("Niveau", "Stufe")) }),
                StartLine::Text { pause: Some((from, to)), .. } => json!({ "kind": "note", "text": format!("{} {from} – {to}", lang.pick("Pause", "Pause")) }),
                StartLine::Text { text, .. } => json!({ "kind": "note", "text": text }),
            })
            .collect();
        let mut head = class_head(state, c, lang);
        let starters = list.lines.iter().filter(|l| matches!(l, StartLine::Starter { not_starting: false, .. })).count();
        head["info"].as_array_mut().unwrap().push(json!([lang.pick("Partants", "Starter"), starters.to_string()]));
        // The judges' list counts them apart (entered, HC, not starting).
        if kind != StartListKind::Judges {
            head["details"].as_array_mut().unwrap().push(json!([lang.pick("Partants", "Starter"), starters.to_string()]));
        }
        // The judges' list: entered, hors concours, not starting (as Rosson's, beside the class's own lines).
        let entered = list.lines.iter().filter(|l| matches!(l, StartLine::Starter { .. })).count();
        let hc = list.lines.iter().filter(|l| matches!(l, StartLine::Starter { hors_concours: true, .. })).count();
        let absent = list.lines.iter().filter(|l| matches!(l, StartLine::Starter { not_starting: true, .. })).count();
        head["counts"] = json!([[lang.pick("Inscrits", "Gemeldet"), entered], ["HC", hc], [lang.pick("Non-partants", "Nichtstarter"), absent]]);
        head["columns"] = json!({ "time": list.start_times, "number": list.start_numbers, "carried": list.qualifier.is_some(), "licence": driving.is_some(), "width": driving.is_some() && kind == StartListKind::Judges });
        head["lines"] = Value::Array(lines);
        if let Some(q) = &list.qualifier {
            head["note"] = json!(format!("{} {q}", lang.pick("Qualif. : pénalités de l’épreuve", "Qualif.: Strafpunkte der Prüfung")));
        }
        out.push(head);
    }
    let what = match kind {
        StartListKind::Judges => lang.pick("Liste de départ des juges", "Startliste Richter"),
        StartListKind::Lite => lang.pick("Liste de départ courte", "Startliste kurz"),
        StartListKind::Full { .. } => lang.pick("Liste de départ", "Startliste"),
    };
    let title = names(state, classes, what);
    let mut data = base(state, lang, &title, printed);
    data["classes"] = Value::Array(out);
    data["list_title"] = json!(what);
    data["variant"] = json!(match kind {
        StartListKind::Full { .. } => "full",
        StartListKind::Lite => "lite",
        StartListKind::Judges => "judges",
    });
    // What the judge fills in by hand, down the right of the class's head, in Rosson's order and
    // (in German) words: Swiss Equestrian's office looks for them there.
    data["judges_fields"] = json!({
        "from": lang.pick("De", "Von"),
        "to": lang.pick("à", "bis"),
        "counts": [
            lang.pick("Nombre de partants (sans HC)", "Anz. Gestartete (ohne HC)"),
            lang.pick("Nombre de partants HC", "Anz. Gestartete HC"),
            lang.pick("Nombre ayant terminé", "Anz. beendet"),
            lang.pick("Nombre de classés", "Anz. klassiert"),
            lang.pick("Visa du juge", "Visum Richter"),
        ],
    });
    data["labels"]["result"] = json!(lang.pick("Résultat", "Resultat"));
    data["labels"]["signalement"] = json!(lang.pick("Signalement", "Signalement"));
    data["labels"]["points"] = json!(lang.pick("SoP", "GWP"));
    data["labels"]["owner"] = json!(lang.pick("Propriétaire", "Besitzer"));
    data["labels"]["licence"] = json!(lang.pick("Lic.", "Liz."));
    data["labels"]["width"] = json!(lang.pick("Largeur", "Wagenbreite"));
    Ok(Print { template: Template::StartList, data, name: title })
}

/// "Liste de départ 12" / "… 12, 13".
fn names(state: &State, classes: &[ClassId], what: &str) -> String {
    let numbers: Vec<String> = classes.iter().filter_map(|c| state.classes.get(c)).map(|c| c.number.clone()).collect();
    format!("{what} {}", numbers.join(", "))
}

fn decimal(v: Option<Decimal>) -> String {
    v.map(|d| d.normalize().to_string()).unwrap_or_default()
}

fn time(v: Option<Decimal>) -> String {
    v.map(|d| format!("{:.2}", d)).unwrap_or_default()
}

/// A reason for being out, in the language.
fn reason(state: &State, code: Option<u32>, lang: Language) -> String {
    reason_text(&views::reason_name(state, code).unwrap_or_default(), lang)
}

/// A reason as Swiss Equestrian names it (`ausgeschieden`…), in the language.
fn reason_text(name: &str, lang: Language) -> String {
    let known: &[(&str, &str, &str)] = &[("ausgeschieden", "éliminé", "ausgeschieden"), ("aufgegeben", "abandon", "aufgegeben"), ("verzichtet", "retiré", "verzichtet"), ("andere", "autre", "andere")];
    known.iter().find(|(k, _, _)| k.eq_ignore_ascii_case(name)).map(|(_, fr, de)| lang.pick(fr, de).to_string()).unwrap_or_else(|| name.to_string())
}

/// A field's column label: "Pén." / "Temps", with the round when there are several.
fn field_label(field: &str, several: bool, two_phase: bool, lang: Language) -> String {
    let what = if field.starts_with("zeit") { lang.pick("Temps", "Zeit") } else { lang.pick("Pén.", "Strafp.") };
    if !several {
        return what.to_string();
    }
    let n = field.chars().last().unwrap_or('1');
    let round = if field.contains("stechen") {
        format!("{}{n}", lang.pick("B", "St"))
    } else if two_phase {
        format!("Ph{n}")
    } else {
        format!("{}{n}", lang.pick("P", "U"))
    };
    format!("{what} {round}")
}

/// Rankings of classes.
pub fn rankings(state: &State, classes: &[ClassId], lang: Language, printed: &str) -> Result<Print, String> {
    rankings_with(state, classes, lang, false, printed)
}

/// Rankings of classes. Dressage: each judge's total and percentage; `by_judge`, with that
/// judge's rank too, "66.45 % (3)" (Rosson's "Avec détail par juge" / "Mit Richterdetails").
pub fn rankings_with(state: &State, classes: &[ClassId], lang: Language, by_judge: bool, printed: &str) -> Result<Print, String> {
    let mut out = Vec::new();
    for id in classes {
        let c = class(state, id)?;
        // Eventing: the overall ranking, a column per phase (its result → what counts), the total.
        if !c.phases.is_empty() {
            out.push(overall_head(state, c, lang)?);
            continue;
        }
        let ranking = views::ranking(state, id)?;
        // Driving's phases: their own columns (Rosson's driving result lists).
        if let Some(kind) = c.phase.as_ref().map(|p| p.kind).filter(|_| state.driving(c)) {
            out.push(driving_ranking(state, c, kind, &ranking, lang)?);
            continue;
        }
        let dressage = state.class_discipline(c) == "CD";
        let fields = crate::results::fields(state, c);
        let two_phase = crate::results::two_phase(state, c);
        let rounds = fields.iter().map(|f| f.trim_start_matches("strafpunkte_").trim_start_matches("zeit_")).collect::<std::collections::BTreeSet<_>>().len();
        let judges: Vec<String> = {
            let mut j: Vec<String> = ranking.lines.iter().flat_map(|l| l.judges.keys().cloned()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
            let order = if c.options.team_3_2 { "HCMEB" } else { "EHCMB" };
            j.sort_by_key(|l| order.find(l.as_str()).unwrap_or(9));
            j
        };
        // Freestyle: parts A and B after the judges; Children: the parts only (one sheet).
        let parts = dressage && ranking.lines.iter().any(|l| l.part_a.is_some() || l.part_b.is_some());
        let children = dressage && c.programme.and_then(|p| state.programmes.get(&p)).and_then(|p| show::master::programme(p).ok()).is_some_and(|p| p.kind == scoring::dressage::ProgrammeKind::Children);
        let judges: Vec<String> = if children { Vec::new() } else { judges };
        let columns: Vec<String> = if dressage {
            let part_columns = if parts { vec!["A %".to_string(), "B %".to_string()] } else { Vec::new() };
            judges.iter().cloned().chain(part_columns).chain(std::iter::once("%".to_string())).collect()
        } else {
            fields.iter().map(|f| field_label(f, rounds > 1, two_phase, lang)).collect()
        };
        // A judge's rank: by their percentage, the highest first, equal percentages the same rank,
        // among those ranked (teams among teams; a team's riders among its riders).
        let teams = ranking.lines.iter().any(|l| l.team);
        let ranked = |l: &views::RankingLine| l.out.is_none() && (l.rank.is_some() || (teams && !l.team));
        let judge_rank = |l: &views::RankingLine, judge: &str, p: Decimal| 1 + ranking.lines.iter().filter(|o| o.team == l.team && ranked(o)).filter_map(|o| o.judge_percentages.get(judge)).filter(|v| **v > p).count();
        let judge_cell = |l: &views::RankingLine, judge: &str| -> String {
            let points = decimal(l.judges.get(judge).copied());
            match l.judge_percentages.get(judge) {
                Some(p) if by_judge && ranked(l) => format!("{points}\n{p:.2} % ({})", judge_rank(l, judge, *p)),
                Some(p) => format!("{points}\n{p:.2} %"),
                None => points,
            }
        };
        let lines: Vec<Value> = ranking
            .lines
            .iter()
            .map(|l| {
                let entry = l.id.parse::<u64>().ok().and_then(|id| state.entries.get(&EntryId(id)));
                let card = entry.and_then(|e| state.results.get(&(id.clone(), e.id.clone())));
                let out = entry.and_then(|e| state.out_reason(id, &e.id));
                let cells: Vec<String> = if dressage {
                    let mut cells: Vec<String> = judges.iter().map(|j| judge_cell(l, j)).collect();
                    if parts {
                        let part = |p: Option<rust_decimal::Decimal>| if out.is_some() { String::new() } else { p.map(|p| format!("{p:.2}")).unwrap_or_default() };
                        cells.push(part(l.part_a));
                        cells.push(part(l.part_b));
                    }
                    cells.push(match (l.percentage, out) {
                        (_, Some(code)) => reason(state, Some(code), lang),
                        (Some(p), None) => format!("{:.2}", p),
                        (None, None) => String::new(),
                    });
                    cells
                } else if let Some(code) = out {
                    let mut cells = vec![String::new(); fields.len().max(1)];
                    cells[0] = reason(state, Some(code), lang);
                    cells
                } else if l.team {
                    vec![String::new(); fields.len().saturating_sub(2)].into_iter().chain([decimal(l.faults), time(l.time)]).collect()
                } else {
                    fields.iter().map(|f| card.and_then(|c| c.values.get(f).copied()).map(|v| if f.starts_with("zeit") { time(Some(v)) } else { decimal(Some(v)) }).unwrap_or_default()).collect()
                };
                let (rider_sub, horse_sub) = match entry {
                    Some(e) => {
                        let (_, town, club) = rider_parts(state, e);
                        (rider_details(state, e, &town, &club), horse_details(e, e.horse.as_ref().and_then(|h| state.horses.get(h)), false, lang))
                    }
                    None => (String::new(), String::new()),
                };
                let rider = entry.map_or_else(|| l.rider.clone(), |e| rider_name(state, e, &l.rider));
                json!({ "rank": l.rank, "placed": l.placed, "team": l.team, "rider": rider, "rider_sub": rider_sub, "horse": l.horse, "horse_sub": horse_sub, "cells": cells })
            })
            .collect();
        let mut head = class_head(state, c, lang);
        let counted = state.results_counted(id);
        head["columns"] = json!(columns);
        head["lines"] = Value::Array(lines);
        head["summary"] = json!(format!("{counted} {} · {} {}", lang.pick("partants", "Starter"), ranking.placed_count, lang.pick("classés", "klassiert")));
        out.push(head);
    }
    let title = names(state, classes, if by_judge { lang.pick("Classement par juge", "Rangliste mit Richterdetails") } else { lang.pick("Classement", "Rangliste") });
    let mut data = base(state, lang, &title, printed);
    data["classes"] = Value::Array(out);
    Ok(Print { template: Template::Ranking, data, name: title })
}

/// A driving phase's result list (Rosson's `fahren-rangliste.print.service.js`): dressage each
/// judge's total with its rank and the judges' mean; marathon phases A and B, each obstacle and
/// the total; cones each round's points and time and the total; derby the time, the penalty
/// seconds and the total; open phases what they count; multitest each judge's total and the
/// mean. A rider out: the reason.
fn driving_ranking(state: &State, c: &Class, kind: PhaseKind, ranking: &views::RankingView, lang: Language) -> Result<Value, String> {
    let settings = c.options.driving.clone().unwrap_or_default();
    let two = |d: Option<Decimal>| d.map(|d| format!("{d:.2}")).unwrap_or_default();
    let card_of = |l: &views::RankingLine| l.id.parse::<u64>().ok().and_then(|e| state.results.get(&(c.id.clone(), EntryId(e))));
    let entry_of = |l: &views::RankingLine| l.id.parse::<u64>().ok().and_then(|e| state.entries.get(&EntryId(e)));
    let total = lang.pick("Total", "Total").to_string();
    // Dressage: each judge's rank (the lowest total first).
    let judges: Vec<String> = {
        // As riding's: E H C M B (driving's judges sit at C, B, E).
        let order = "EHCMB";
        let mut j: Vec<String> = ranking.lines.iter().flat_map(|l| l.judges.keys().cloned()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        j.sort_by_key(|l| order.find(l.as_str()).unwrap_or(9));
        j
    };
    let judge_rank = |judge: &str, value: Decimal| 1 + ranking.lines.iter().filter(|o| o.rank.is_some()).filter_map(|o| o.judges.get(judge)).filter(|v| **v < value).count();
    // Multitest: each judge's total, from its sheets.
    let multitest = (kind == PhaseKind::Multitest).then(|| views::multitest(state, &c.id)).transpose()?;
    let rounds = settings.rounds.unwrap_or(1) as usize;
    let obstacles = settings.obstacles.unwrap_or(5) as usize;
    let columns: Vec<String> = match kind {
        PhaseKind::Dressage => judges.iter().cloned().chain([total.clone()]).collect(),
        PhaseKind::Marathon => ["A".to_string(), "B".to_string()].into_iter().chain((1..=obstacles).map(|i| format!("{} {i}", lang.pick("Obst.", "Hind.")))).chain([total.clone()]).collect(),
        PhaseKind::Cones => (1..=rounds).flat_map(|r| [format!("{} {r}", lang.pick("Pén.", "Strafp.")), format!("{} {r}", lang.pick("Temps", "Zeit"))]).chain([lang.pick("Temps", "Zeit").to_string(), total.clone()]).collect(),
        PhaseKind::Derby => vec![lang.pick("Temps", "Zeit").into(), lang.pick("Fautes", "Abwürfe").into(), lang.pick("Autres", "Übrige").into(), total.clone()],
        PhaseKind::Open => vec![lang.pick("Secondes", "Sekunden").into(), lang.pick("Points", "Punkte").into(), total.clone()],
        PhaseKind::Presentation => (1..=3).map(|i| format!("{} {i}", lang.pick("Note", "Note"))).chain([total.clone()]).collect(),
        PhaseKind::Multitest => multitest.as_ref().map(|m| m.judges.iter().map(|j| j.letter.clone()).collect::<Vec<_>>()).unwrap_or_default().into_iter().chain([total.clone()]).collect(),
        _ => vec![total.clone()],
    };
    let lines: Vec<Value> = ranking
        .lines
        .iter()
        .map(|l| {
            let entry = entry_of(l);
            let card = card_of(l);
            let out = entry.and_then(|e| state.out_reason(&c.id, &e.id));
            let values = card.map(|c| c.values.clone()).unwrap_or_default();
            let v = |k: &str| values.get(k).copied();
            let mut cells: Vec<String> = match kind {
                PhaseKind::Dressage => judges.iter().map(|j| l.judges.get(j).map(|t| format!("{t:.2} ({})", judge_rank(j, *t))).unwrap_or_default()).collect(),
                PhaseKind::Marathon => match state.marathon_breakdown(c, &values) {
                    Some((score, each)) => [two(Some(score.phase_a)), two(Some(score.phase_b))].into_iter().chain(each.iter().map(|p| two(Some(*p)))).collect(),
                    None => vec![String::new(); 2 + obstacles],
                },
                PhaseKind::Cones => {
                    let driven = entry.and_then(|e| state.cones_rounds(c, e, &values).ok()).unwrap_or_default();
                    let mut cells: Vec<String> = (0..rounds).flat_map(|r| match driven.get(r).copied().flatten() {
                        Some(x) => [two(Some(x.points)), two(Some(x.time))],
                        None => [String::new(), String::new()],
                    }).collect();
                    cells.push(two(l.time));
                    cells
                }
                PhaseKind::Derby => vec![two(v("zeit")), decimal(v("abwuerfe")), decimal(v("uebrige"))],
                PhaseKind::Open => vec![two(v("sekunden")), decimal(v("punkte"))],
                PhaseKind::Presentation => ["note_1", "note_2", "note_3"].iter().map(|k| decimal(v(k))).collect(),
                PhaseKind::Multitest => {
                    let m = multitest.as_ref();
                    let line = m.and_then(|m| m.lines.iter().find(|x| x.entry.to_string() == l.id));
                    m.map(|m| m.judges.iter().map(|j| two(line.and_then(|x| x.totals.get(&j.assignment).copied()))).collect()).unwrap_or_default()
                }
                _ => Vec::new(),
            };
            cells.push(match out {
                Some(code) => reason(state, Some(code), lang),
                None => two(l.faults),
            });
            let (rider_sub, horse_sub) = match entry {
                Some(e) => {
                    let (_, town, club) = rider_parts(state, e);
                    let horses = show::model::horses_in_phase(e, kind);
                    let names: Vec<String> = horses.iter().filter(|h| h["pferdename"].is_string()).map(|h| show::model::horse_on_entry_name(h)).collect();
                    (rider_details(state, e, &town, &club), if names.len() > 1 { names.join(" · ") } else { horse_details(e, e.horse.as_ref().and_then(|h| state.horses.get(h)), false, lang) })
                }
                None => (String::new(), String::new()),
            };
            let rider = entry.map_or_else(|| l.rider.clone(), |e| rider_name(state, e, &l.rider));
            json!({ "rank": l.rank, "placed": l.placed, "team": l.team, "rider": rider, "rider_sub": rider_sub, "horse": l.horse, "horse_sub": horse_sub, "cells": cells })
        })
        .collect();
    let mut head = class_head(state, c, lang);
    let counted = state.results_counted(&c.id);
    head["columns"] = json!(columns);
    head["lines"] = Value::Array(lines);
    head["summary"] = json!(format!("{counted} {} · {} {}", lang.pick("partants", "Starter"), ranking.placed_count, lang.pick("classés", "klassiert")));
    Ok(head)
}

/// A phase's name, as Swiss Equestrian's driving rules (B: épreuve de terrain, Geländeprüfung;
/// the open phase: a special class, « épreuve spéciale », Spezialprüfung).
pub fn phase_name(kind: PhaseKind, lang: Language) -> &'static str {
    match kind {
        PhaseKind::Dressage => lang.pick("Dressage", "Dressur"),
        PhaseKind::Jumping => lang.pick("Saut", "Springen"),
        PhaseKind::Cross => lang.pick("Cross", "Gelände"),
        PhaseKind::Marathon => lang.pick("Terrain", "Gelände"),
        PhaseKind::Cones => lang.pick("Maniabilité", "Kegelfahren"),
        PhaseKind::Derby => lang.pick("Derby", "Derby"),
        PhaseKind::Open => lang.pick("Épreuve spéciale", "Spezialprüfung"),
        PhaseKind::Multitest => lang.pick("Multi-test", "Multitest"),
        PhaseKind::Presentation => lang.pick("Présentation", "Präsentation"),
    }
}

/// Eventing: a class made of phases, its overall ranking for the ranking print.
fn overall_head(state: &State, c: &Class, lang: Language) -> Result<Value, String> {
    let view = views::overall(state, &c.id)?;
    let phase_name = |k: PhaseKind| phase_name(k, lang);
    let mut columns: Vec<String> = view.phases.iter().map(|p| phase_name(p.kind).to_string()).collect();
    columns.push(lang.pick("Total", "Total").to_string());
    // Out: why (« éliminé », « abandon »…), else both words.
    let out_text = |why: &Option<String>| why.as_deref().map(|w| reason_text(w, lang)).unwrap_or_else(|| lang.pick("éliminé / abandon", "ausgeschieden / aufgegeben").to_string());
    let lines: Vec<Value> = view
        .lines
        .iter()
        .map(|l| {
            let mut cells: Vec<String> = l
                .cells
                .iter()
                .map(|cell| match (cell.out, &cell.value) {
                    (true, _) => out_text(&cell.reason),
                    (false, Some(v)) if !cell.text.is_empty() => format!("{} → {}", cell.text, v.normalize()),
                    (false, Some(v)) => v.normalize().to_string(),
                    (false, None) => String::new(),
                })
                .collect();
            cells.push(if l.out { out_text(&l.reason) } else { l.total.map(|t| format!("{t:.2}")).unwrap_or_default() });
            let rider = match l.start_number {
                Some(n) => format!("{n} · {}", l.rider),
                None => l.rider.clone(),
            };
            json!({ "rank": l.rank, "placed": l.placed, "team": false, "rider": rider, "rider_sub": l.club, "horse": l.horse, "horse_sub": "", "cells": cells })
        })
        .collect();
    let mut head = class_head(state, c, lang);
    head["columns"] = json!(columns);
    head["lines"] = Value::Array(lines);
    head["summary"] = json!(format!("{} {} · {} {}", view.lines.len(), lang.pick("partants", "Starter"), view.placed_count, lang.pick("classés", "klassiert")));
    Ok(head)
}

fn rider_parts(state: &State, e: &Entry) -> (String, String, String) {
    match (&e.rider_on_entry, state.persons.get(&e.rider)) {
        (Some(r), _) => (format!("{} {}", r.first_name, r.last_name), r.town.clone(), r.club.clone()),
        (None, Some(p)) => (format!("{} {}", p.first_name, p.last_name), p.town.clone(), p.club.clone()),
        _ => (format!("#{}", e.rider), String::new(), String::new()),
    }
}

/// Dressage judges' sheets of a class, as Swiss Equestrian's official form (Rosson's print of
/// it): judge by judge (H, C, M, E, B), a sheet per rider who starts, in start order — or one
/// judge's. `memo`: each judge's set starts with a sheet without rider, horse or number,
/// marked "Juge" across, for the judge's notes.
pub fn judge_sheets(state: &State, id: &ClassId, judge: Option<&AssignmentId>, memo: bool, lang: Language, printed: &str) -> Result<Print, String> {
    let c = class(state, id)?;
    let judges: Vec<&Assignment> = state.dressage_judges(id).into_iter().filter(|a| judge.is_none_or(|j| &a.id == j)).collect();
    if judges.is_empty() {
        return Err(lang.pick("L’épreuve n’a pas de juge de dressage : attribuez-les dans Officiels.", "Die Prüfung hat keine Dressurrichter: unter Offizielle zuteilen.").into());
    }
    let list = views::start_list(state, id)?;
    let starters: Vec<(u64, Option<u32>)> = list
        .lines
        .iter()
        .filter_map(|l| match l {
            StartLine::Starter { entry, start_number, not_starting: false, .. } => Some((*entry, *start_number)),
            _ => None,
        })
        .collect();
    // The tests the sheets use (a class's own, or each rider's in an "à choix" class).
    let mut tests = serde_json::Map::new();
    let mut test_of = |p: Option<u32>| -> Option<String> {
        let p = state.programmes.get(&p?)?;
        let key = p.id.to_string();
        if !tests.contains_key(&key) {
            tests.insert(key.clone(), judge_test(p, lang));
        }
        Some(key)
    };
    let date = c.date.split('-').rev().collect::<Vec<_>>().join(".");
    let place = state.event.as_ref().map(|e| e.place.clone()).unwrap_or_default();
    let mut sheets = Vec::new();
    for a in &judges {
        let town = state.persons.get(&a.person).map(|p| p.town.clone()).or_else(|| state.officials.get(&a.person).map(|o| o.town.clone())).unwrap_or_default();
        let judge_line = format!("{}{} ({})", person_name(state, &a.person), if town.is_empty() { String::new() } else { format!(", {town}") }, a.letter.clone().unwrap_or_default());
        let sheet = |test: &str, number: String, rider: String, horse: String, memo: bool| {
            json!({ "test": test, "memo": memo, "number": number, "rider": rider, "horse": horse, "class": c.number, "judge": judge_line, "date_place": format!("{date} {place}").trim() })
        };
        let mut own = Vec::new();
        // Team dressage: a sheet per team; 3/2, H, C and M the team's first, second, third rider.
        let teams = if state.published_classes.get(&c.published).is_some_and(|p| p.discipline == "CD" && p.team_size.is_some_and(|n| n > 0)) { state.class_teams(id) } else { Vec::new() };
        // 3/2 (OKV): the side judges mark the team on the OKV team test, the others the riders
        // on the test itself.
        let team_test = if c.options.team_3_2 {
            let (riders, side) = okv_team_tests(state, c.programme);
            if matches!(a.letter.as_deref(), Some("E" | "B")) { side } else { riders }
        } else {
            c.programme
        };
        for (team, members) in &teams {
            if members.iter().all(|e| e.status != show::model::EntryStatus::Entered) {
                continue;
            }
            let Some(test) = test_of(team_test) else { continue };
            let t = state.teams.get(team);
            let number = t.and_then(|t| t.start_number).map(|n| n.to_string()).unwrap_or_default();
            let place = ["H", "C", "M"].iter().position(|l| a.letter.as_deref() == Some(*l)).filter(|_| c.options.team_3_2);
            match place.and_then(|i| members.get(i)) {
                Some(e) => {
                    let (rider, _, _) = rider_parts(state, e);
                    let horse = views_horse(state, e);
                    own.push(sheet(&test, number, format!("{rider} ({}) · {}", e.rider.0, t.map(|t| t.name.clone()).unwrap_or_default()), horse, false));
                }
                None => own.push(sheet(&test, number, format!("{}{}", lang.pick("Équipe : ", "Mannschaft: "), t.map(|t| t.name.clone()).unwrap_or_default()), String::new(), false)),
            }
        }
        for (entry, number) in starters.iter().filter(|_| teams.is_empty()) {
            let e = &state.entries[&EntryId(*entry)];
            let Some(test) = test_of(e.programme.or(c.programme)) else { continue };
            let (rider, _, _) = rider_parts(state, e);
            let horse = views_horse(state, e);
            let horse = match e.horse.as_ref().filter(|h| h.0 != 5) {
                Some(h) => format!("{horse} ({})", h.0),
                None => horse,
            };
            own.push(sheet(&test, number.map(|n| n.to_string()).unwrap_or_default(), format!("{rider} ({})", e.rider.0), horse, false));
        }
        if memo {
            if let Some(test) = test_of(if teams.is_empty() { c.programme } else { team_test }).or_else(|| own.first().map(|s| s["test"].as_str().unwrap_or_default().to_string())) {
                own.insert(0, sheet(&test, String::new(), String::new(), String::new(), true));
            }
        }
        sheets.extend(own);
    }
    if sheets.is_empty() {
        return Err(lang.pick("Aucune reprise pour les cavaliers de l’épreuve.", "Kein Programm für die Reiter der Prüfung.").into());
    }
    let title = format!("{} {}", lang.pick("Feuilles de juge", "Richterblätter"), c.number);
    let mut data = base(state, lang, &title, printed);
    // Driving: no correction or coefficient; the carriage number and the driver (Rosson's).
    let driving = state.driving(c);
    data["driving"] = json!(driving);
    data["sheets"] = Value::Array(sheets);
    data["tests"] = Value::Object(tests);
    data["year"] = json!(c.date.get(..4).unwrap_or_default());
    // Rosson's words for the form.
    for (k, fr, de) in [
        ("start_number", "N° de départ", "Start-Nr."),
        ("rider", "Cavalier/ère", "Reiter/in"),
        ("horse", "Cheval", "Pferd"),
        ("class_number", "Épreuve n°", "Prüfungs-Nr."),
        ("judge", "Juge (position)", "Richter/in (Position)"),
        ("date_place", "Date/Lieu", "Datum/Ort"),
        ("number", "N°", "Nr."),
        ("criteria", "Critères de jugement", "Bewertungskriterien"),
        ("mark", "Note", "Note"),
        ("correction", "Corr.", "Korr."),
        ("coefficient", "Coeff.", "Koeff."),
        ("remarks", "Remarques", "Bemerkungen"),
        ("movements", "Figures", "Aufgaben"),
        ("ideas", "Idées directrices", "Leitgedanken"),
        ("subtotal", "Total intermédiaire", "Zwischentotal"),
        ("penalties", "Points de pénalité", "Strafpunkte"),
        ("result", "Résultat", "Resultat"),
        ("total_points", "Total des points", "Total Punkte"),
        ("percent", "Résultat en pourcentage", "Resultat in Prozent"),
        ("max_points", "Points max.", "Max. Punkte"),
        ("total_a", "Total note A / 2", "Total Note A / 2"),
        ("total_b", "Total note B / 2", "Total Note B / 2"),
        ("total_a_children", "Total note A", "Total Note A"),
        ("total_b_children", "Total note B", "Total Note B"),
        ("signature", "Signature du juge / de la juge :", "Unterschrift des Richters/der Richterin:"),
        ("carried_from", "Report de la page", "Übertrag von Seite"),
        ("carried_to", "Report à la page", "Übertrag auf Seite"),
        ("memo", "JUGE", "RICHTER"),
    ] {
        data["labels"][k] = json!(lang.pick(fr, de));
    }
    if driving {
        data["labels"]["start_number"] = json!(lang.pick("N° de voiture", "Wagen-Nr."));
        data["labels"]["rider"] = json!(lang.pick("Meneur/euse", "Fahrer/in"));
    }
    Ok(Print { template: Template::JudgeSheets, data, name: title })
}

/// OKV team dressage: the test the riders' judges (H, C, M) use and the side judges' (E, B)
/// "OKV Mannschaftsdressur …" version of it, with the same movements and other directive ideas
/// (OKV Veranstalterdossier Dressur: in Rosson the office changed the class's test to print
/// each). Either can be the class's test; without the other, the class's test for both.
fn okv_team_tests(state: &State, programme: Option<u32>) -> (Option<u32>, Option<u32>) {
    const PREFIX: &str = "okvmannschaftsdressur";
    let Some(p) = programme.and_then(|p| state.programmes.get(&p)) else { return (programme, programme) };
    let squash = |s: &str| s.split_whitespace().collect::<String>().to_lowercase();
    let name = squash(&p.name);
    let base = name.strip_prefix(PREFIX).unwrap_or(&name).to_string();
    let find = |wanted: &str| state.programmes.values().find(|q| squash(&q.name) == wanted).map(|q| q.id);
    (find(&base).or(programme), find(&format!("{PREFIX}{base}")).or(programme))
}

/// A dressage test as the judges' sheet shows it, in the language (German when a text is
/// missing): its title, arena, time, bridle and edition, and its tables — movements with their
/// letters, exercises and directive ideas (`lektionen`), criteria (`standard`), penalty points
/// (`strafpunkte`) — with the texts before and after them; kür and children's results.
fn judge_test(p: &ProgrammeDef, lang: Language) -> Value {
    let def: Value = serde_json::from_str(&p.definition).unwrap_or(Value::Null);
    let suffix = lang.pick("fr", "de");
    let tr = |v: &Value, key: &str| -> String {
        v[format!("{key}_{suffix}")].as_str().filter(|t| !t.trim().is_empty()).or_else(|| v[format!("{key}_de")].as_str()).unwrap_or("").trim().to_string()
    };
    // As Rosson writes a value: nothing for 0, empty or missing.
    let plain = |v: &Value| match v {
        Value::Number(n) if n.as_f64() != Some(0.0) => n.to_string(),
        Value::String(t) => t.trim().to_string(),
        _ => String::new(),
    };
    let children = def["children"] == true;
    let tables: Vec<Value> = def["tabellen"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|t| {
            let layout = t["layout"].as_str().unwrap_or("standard");
            let fields = t["felder"].as_array().map(Vec::as_slice).unwrap_or_default();
            let rows: Vec<Value> = fields
                .iter()
                .map(|f| match layout {
                    "lektionen" | "lektionen_ohne_noten" => json!({
                        "number": plain(&f["nummer"]),
                        "items": f["thema"].as_array().map(Vec::as_slice).unwrap_or_default().iter().map(|x| json!({ "position": tr(x, "position"), "text": tr(x, "thema"), "ideas": tr(x, "leitgedanken") })).collect::<Vec<_>>(),
                        "mark": plain(&f["note"]), "correction": plain(&f["korrektur"]), "coefficient": plain(&f["koeffizient"]), "remarks": tr(f, "bemerkungen"),
                    }),
                    "strafpunkte" => json!({ "text": tr(f, "beschreibung"), "penalty": tr(f, "strafe"), "received": plain(&f["erhaltene_strafpunkte"]), "remarks": tr(f, "bemerkungen") }),
                    _ => json!({
                        "number": plain(&f["nummer"]), "text": tr(&f["thema"], "thema"),
                        "mark": plain(&f["note"]), "correction": plain(&f["korrektur"]),
                        "coefficient": if children { String::new() } else { plain(&f["koeffizient"]) }, "remarks": tr(f, "bemerkungen"),
                    }),
                })
                .collect();
            json!({ "layout": layout, "title": tr(t, "titel"), "before": tr(t, "text_davor"), "after": tr(t, "text_danach"), "rows": rows })
        })
        .collect();
    let subtitle = [tr(&def, "platzart"), tr(&def, "zeit"), tr(&def, "zaeumung")].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" - ");
    // Swiss Equestrian's French title of the OKV team test is the riders' test's: the German
    // one, so that the side judges' sheet says what it is.
    let mut title = tr(&def, "titel");
    if p.name.to_lowercase().starts_with("okv") && !title.to_lowercase().contains("okv") {
        title = def["titel_de"].as_str().unwrap_or(&p.name).trim().to_string();
    }
    json!({
        "title": title,
        "subtitle": subtitle,
        "version": tr(&def, "version"),
        "fei": def["fei"] == true,
        "kuer": def["kuer"] == true,
        "children": children,
        "tables": tables,
        "result_after": tr(&def["texte"]["resultat"], "text_danach"),
        "max_points": p.max_points.normalize().to_string(),
    })
}

fn views_horse(state: &State, e: &Entry) -> String {
    e.guest_horse_shown().or_else(|| e.first_horse_on_entry()).or_else(|| e.horse.as_ref().and_then(|h| state.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default()
}

/// A ranking over several classes.
pub fn combined(state: &State, id: &str, lang: Language, printed: &str) -> Result<Print, String> {
    let view = views::combined(state, id)?;
    let r = &view.ranking;
    let penalties = r.method == "penalties";
    let percentages = r.method == "percentages";
    // The class that breaks ties (ranks; penalties by its penalties or result), and its place.
    let tie = r.classes.iter().find(|c| Some(&c.id) == r.tie_break.as_ref()).map(|c| c.number.clone()).unwrap_or_default();
    let tie_at = r.classes.iter().position(|c| Some(&c.id) == r.tie_break.as_ref());
    let rule = match (r.method, r.tie) {
        ("penalties", "faults") => format!("{} {tie} {}", lang.pick("Pénalités additionnées ; égalité : les pénalités de l’épreuve", "Strafpunkte addiert; Gleichstand: die Strafpunkte der Prüfung"), lang.pick("départagent.", "entscheiden.")),
        ("penalties", "result") => format!("{} {tie} {}", lang.pick("Pénalités additionnées ; égalité : le résultat de l’épreuve", "Strafpunkte addiert; Gleichstand: das Resultat der Prüfung"), lang.pick("départage.", "entscheidet.")),
        ("penalties", _) => lang.pick("Pénalités additionnées ; égalité : ex aequo.", "Strafpunkte addiert; Gleichstand: ex aequo.").to_string(),
        ("percentages", _) => format!("{} {tie} {}", lang.pick("Moyenne des pourcentages ; égalité : le pourcentage de l’épreuve", "Durchschnitt der Prozente; Gleichstand: der Prozentsatz der Prüfung"), lang.pick("départage.", "entscheidet.")),
        _ => format!("{} {tie} {}", lang.pick("Rangs additionnés ; égalité : le rang dans l’épreuve", "Ränge addiert; Gleichstand: der Rang in der Prüfung"), lang.pick("départage.", "entscheidet.")),
    };
    let percent = |p: Option<Decimal>| p.map(|p| format!("{:.2} %", p.round_dp(2))).unwrap_or_default();
    let lines: Vec<Value> = view
        .lines
        .iter()
        .map(|l| {
            let cells: Vec<Value> = l
                .cells
                .iter()
                .map(|c| match c.rank {
                    _ if c.missing => json!(["–", ""]),
                    // Out: why (« éliminé », « abandon »…).
                    None => json!([c.out.as_deref().map(|w| reason_text(w, lang)).unwrap_or_else(|| lang.pick("élim.", "ausg.").to_string()), ""]),
                    Some(rank) if percentages => json!([percent(c.percentage), format!("{rank}.")]),
                    Some(_) if penalties => json!([decimal(c.faults), time(c.time)]),
                    Some(rank) => json!([format!("{rank}."), if c.percentage.is_some() { percent(c.percentage) } else { decimal(c.faults) }]),
                })
                .collect();
            json!({
                "rank": l.rank, "placed": l.placed, "rider": l.rider, "rider_sub": l.club, "horse": l.horse, "cells": cells,
                "total": if percentages { percent(l.total) } else { decimal(l.total) },
                "tie_break": match (penalties, r.tie) {
                    (true, "none") => String::new(),
                    // The class's penalties (and time, by its result).
                    (true, by) => tie_at.and_then(|i| l.cells.get(i)).filter(|c| c.rank.is_some()).map(|c| if by == "result" { format!("{} / {}", decimal(c.faults), time(c.time)) } else { decimal(c.faults) }).unwrap_or_default(),
                    _ => l.tie_break.map(|t| format!("{}.", t.normalize())).unwrap_or_default(),
                },
            })
        })
        .collect();
    let mut data = base(state, lang, &r.name, printed);
    data["ranking"] = json!({ "name": r.name, "rule": rule, "classes": r.classes.iter().map(|c| c.number.clone()).collect::<Vec<_>>(), "lines": lines });
    Ok(Print { template: Template::Combined, data, name: r.name.clone() })
}

/// Everyone entered, rider by rider, with their horses and classes.
pub fn participants(state: &State, lang: Language, printed: &str) -> Print {
    let rows = views::participants(state);
    let mut riders: Vec<Value> = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        let rider = rows[i].rider_id;
        let first = &rows[i];
        let mut horses: BTreeMap<String, Vec<String>> = BTreeMap::new();
        while i < rows.len() && rows[i].rider_id == rider {
            let r = &rows[i];
            let classes = if r.starts.is_empty() {
                vec![r.class_number.clone()]
            } else {
                r.starts.iter().map(|s| match s.start_number { Some(n) => format!("{} (#{n})", s.number), None => s.number.clone() }).collect()
            };
            let mut text: Vec<String> = classes;
            if r.not_starting {
                text = text.into_iter().map(|c| format!("{c} NS")).collect();
            }
            horses.entry(r.horse.clone()).or_default().extend(text);
            i += 1;
        }
        let rider_sub = [first.town.clone(), first.club.clone()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        riders.push(json!({
            "rider": first.rider,
            "rider_sub": rider_sub,
            "horses": horses.into_iter().map(|(h, c)| json!({ "horse": h, "classes": c.join(" · ") })).collect::<Vec<_>>(),
        }));
    }
    let title = lang.pick("Participants", "Teilnehmer").to_string();
    let mut data = base(state, lang, &title, printed);
    data["riders"] = Value::Array(riders);
    Print { template: Template::Participants, data, name: title }
}

/// The entries in detail (Rosson's "Detail Nennungen"), for the show or one published class:
/// rider by rider (by name), with their phones; each horse's entries with the class, start
/// number, fees, what was paid (`underpaid` when less than the first fee), the rider's remark
/// and the office's note. Office use: it has the riders' phones.
pub fn entries_detail(state: &State, published: Option<&PublishedClassId>, lang: Language, printed: &str) -> Print {
    let money = |d: Option<Decimal>| d.map(|d| format!("CHF {}", d.round_dp(2).normalize())).unwrap_or_default();
    // The class and start number of an entry: its section if it is on a start list.
    let start_of = |e: &EntryId| state.starts.iter().find_map(|(c, s)| s.iter().find(|s| &s.entry == e).map(|s| (c.clone(), s.start_number)));
    let mut entries: Vec<&Entry> = state.entries.values().filter(|e| published.is_none_or(|p| &e.published == p) && !e.waiting_list).collect();
    let name = |e: &Entry| state.persons.get(&e.rider).map(|p| (p.last_name.clone(), p.first_name.clone())).unwrap_or_default();
    entries.sort_by(|a, b| scoring::ranking::compare_names(&format!("{} {}", name(a).0, name(a).1), &format!("{} {}", name(b).0, name(b).1)).then(a.rider.cmp(&b.rider)));
    let mut riders: Vec<Value> = Vec::new();
    let mut i = 0;
    while i < entries.len() {
        let rider = entries[i].rider.clone();
        let mut horses: Vec<(String, Vec<Value>)> = Vec::new();
        while i < entries.len() && entries[i].rider == rider {
            let e = entries[i];
            let horse = e.guest_horse_shown().or_else(|| e.horse.as_ref().and_then(|h| state.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default();
            let p = state.published_classes.get(&e.published);
            let fee = p.and_then(|p| p.fee);
            let fee2 = p.and_then(|p| crate::results::decimal_of(&p.record["nenngeld2"]));
            let (class, number) = start_of(&e.id).map(|(c, n)| (state.classes.get(&c).map_or_else(|| c.0.clone(), |c| c.number.clone()), n)).unwrap_or_else(|| (p.map(|p| p.number.clone()).unwrap_or_default(), None));
            let not_starting = if e.status == EntryStatus::NotStarting { " NS" } else { "" };
            let line = json!({
                "class": format!("{}{not_starting}", class),
                "number": number.map(|n| n.to_string()).unwrap_or_else(|| "–".into()),
                "fee": money(fee),
                "fee2": money(fee2),
                "paid": money(e.fee_paid),
                "underpaid": matches!((e.fee_paid, fee), (Some(paid), Some(fee)) if paid < fee),
                "remarks": state.entry_notes.get(&e.id).map(|n| crate::views::said(&n.remarks)).unwrap_or_default(),
                "comment": e.comment,
            });
            match horses.iter_mut().find(|(h, _)| *h == horse) {
                Some((_, lines)) => lines.push(line),
                None => horses.push((horse, vec![line])),
            }
            i += 1;
        }
        let person = state.persons.get(&rider);
        let contact = state.contacts.get(&rider);
        let phones = contact.map(|c| [c.phone_private.as_str(), c.phone_mobile.as_str()].into_iter().filter(|p| !p.trim().is_empty()).collect::<Vec<_>>().join(" · ")).unwrap_or_default();
        riders.push(json!({
            "rider": person.map(|p| format!("{} {}", p.last_name, p.first_name)).unwrap_or_default(),
            "town": person.map(|p| p.town.clone()).unwrap_or_default(),
            "phones": phones,
            "horses": horses.into_iter().map(|(h, e)| json!({ "horse": h, "entries": e })).collect::<Vec<_>>(),
        }));
    }
    let what = lang.pick("Inscriptions en détail", "Nennungen im Detail");
    let title = match published.and_then(|p| state.published_classes.get(p)) {
        Some(p) => format!("{what} · {} {}", lang.pick("épreuve", "Prüfung"), p.number),
        None => what.to_string(),
    };
    let mut data = base(state, lang, &title, printed);
    data["riders"] = Value::Array(riders);
    data["labels"]["fee"] = json!(lang.pick("Finance 1", "Nenngeld 1"));
    data["labels"]["fee2"] = json!(lang.pick("Finance 2", "Nenngeld 2"));
    data["labels"]["paid"] = json!(lang.pick("Payé", "Bezahlt"));
    Print { template: Template::Entries, data, name: title }
}

/// The timetable: day by day, the classes in their order.
pub fn schedule(state: &State, lang: Language, printed: &str) -> Print {
    let show = views::show("", state);
    let days: Vec<Value> = show
        .days
        .iter()
        .map(|d| {
            json!({
                "date": long_date(&d.date, lang, true),
                "classes": d.classes.iter().map(|c| {
                    let id = ClassId(c.id.clone());
                    let judges = officials(state, &id, lang, false).into_iter().map(|(role, names)| format!("{role}: {names}")).collect::<Vec<_>>().join(" · ");
                    json!({ "time": c.time, "number": c.number, "name": c.title, "sub": format!("{} {} · {}", c.discipline, c.category, c.scoring), "starters": c.starters - c.not_starting, "judges": judges })
                }).collect::<Vec<_>>(),
            })
        })
        .collect();
    let title = lang.pick("Horaire", "Zeitplan").to_string();
    let mut data = base(state, lang, &title, printed);
    data["days"] = Value::Array(days);
    Print { template: Template::Schedule, data, name: title }
}

/// A sheet of sticky labels: the two Zweckform sheets Rosson knows (3475: 3 × 8 of 70 × 36 mm;
/// 3484: 2 × 8 of 105 × 37 mm), or one the office describes (kept with the library's settings):
/// how many across and down, each label's size, the page's top and left margins, the gaps
/// between labels, the space inside, the text's size and alignment.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sheet {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub cols: u32,
    pub rows: u32,
    pub width_mm: f64,
    pub height_mm: f64,
    pub top_mm: f64,
    #[serde(default)]
    pub left_mm: f64,
    #[serde(default)]
    pub gap_x_mm: f64,
    #[serde(default)]
    pub gap_y_mm: f64,
    pub padding_mm: f64,
    pub font_pt: f64,
    /// `left`, `center` or `right`.
    #[serde(default = "left")]
    pub align: String,
}

fn left() -> String {
    "left".into()
}

impl Sheet {
    /// The sheets every office has.
    pub fn builtin() -> Vec<Sheet> {
        let zweckform = |id: &str, cols, width_mm, height_mm, top_mm, font_pt| Sheet {
            id: id.into(),
            name: format!("Zweckform {id} · {width_mm}×{height_mm}"),
            cols,
            rows: 8,
            width_mm,
            height_mm,
            top_mm,
            left_mm: 0.0,
            gap_x_mm: 0.0,
            gap_y_mm: 0.0,
            padding_mm: 5.0,
            font_pt,
            align: left(),
        };
        vec![zweckform("3475", 3, 70.0, 36.0, 4.5, 9.0), zweckform("3484", 2, 105.0, 37.0, 0.5, 9.5)]
    }

    /// A sheet every office has, by its id (3475 when unknown).
    pub fn of(id: &str) -> Sheet {
        let all = Sheet::builtin();
        all.iter().find(|s| s.id == id).unwrap_or(&all[0]).clone()
    }

    /// The labels fit on an A4 page, and can be read.
    pub fn check(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("a sheet needs a name".into());
        }
        if !(1..=20).contains(&self.cols) || !(1..=30).contains(&self.rows) {
            return Err("1 to 20 labels across, 1 to 30 down".into());
        }
        let sizes = [self.width_mm, self.height_mm, self.top_mm, self.left_mm, self.gap_x_mm, self.gap_y_mm, self.padding_mm];
        if sizes.iter().any(|v| !v.is_finite() || *v < 0.0) || self.width_mm < 10.0 || self.height_mm < 5.0 {
            return Err("sizes in millimetres, labels at least 10 × 5 mm".into());
        }
        let across = self.left_mm + self.cols as f64 * self.width_mm + (self.cols - 1) as f64 * self.gap_x_mm;
        let down = self.top_mm + self.rows as f64 * self.height_mm + (self.rows - 1) as f64 * self.gap_y_mm;
        if across > 210.5 || down > 297.5 {
            return Err(format!("the labels take {across:.0} × {down:.0} mm: more than an A4 page (210 × 297)"));
        }
        if !(5.0..=30.0).contains(&self.font_pt) {
            return Err("text of 5 to 30 points".into());
        }
        if !matches!(self.align.as_str(), "left" | "center" | "right") {
            return Err("aligned left, center or right".into());
        }
        Ok(())
    }

    fn layout(&self) -> Value {
        json!({
            "cols": self.cols, "rows": self.rows, "width_mm": self.width_mm, "height_mm": self.height_mm,
            "top_mm": self.top_mm, "left_mm": self.left_mm, "gap_x_mm": self.gap_x_mm, "gap_y_mm": self.gap_y_mm,
            "padding_mm": self.padding_mm, "font_pt": self.font_pt, "align": self.align,
        })
    }
}

/// What labels say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Labels {
    /// Every rider's address (private data), by name.
    Addresses,
    /// For the dressage judges' paper sheets: one per judge and rider who starts, judge by judge.
    Judges(ClassId),
    /// For the prizes: one per rank with a prize.
    Prizes(ClassId),
}

fn line(text: impl Into<String>, bold: bool) -> Value {
    json!({ "text": text.into(), "bold": bold })
}

/// Labels on a sheet, the first `skip` places left empty.
pub fn labels(state: &State, what: &Labels, sheet: Sheet, skip: usize, lang: Language, printed: &str) -> Result<Print, String> {
    let mut out: Vec<Value> = Vec::new();
    let title = match what {
        Labels::Addresses => {
            let riders: std::collections::BTreeSet<&PersonId> = state.entries.values().map(|e| &e.rider).collect();
            let mut people: Vec<&Person> = riders.into_iter().filter_map(|r| state.persons.get(r)).collect();
            people.sort_by(|a, b| (&a.last_name, &a.first_name).cmp(&(&b.last_name, &b.first_name)));
            for p in people {
                let Some(c) = state.contacts.get(&p.id) else { continue };
                let mut lines = vec![];
                if !c.salutation.is_empty() {
                    lines.push(line(c.salutation.clone(), false));
                }
                lines.push(line(format!("{} {}", p.first_name, p.last_name), true));
                if !c.street.is_empty() {
                    lines.push(line(c.street.clone(), false));
                }
                lines.push(line(format!("{} {}", c.postcode, p.town).trim().to_string(), false));
                if !c.country.is_empty() && c.country != "CH" {
                    lines.push(line(c.country.clone(), false));
                }
                out.push(json!({ "lines": lines }));
            }
            if out.is_empty() {
                return Err(lang.pick("Pas d’adresses : le concours n’a pas les coordonnées des cavaliers.", "Keine Adressen: das Turnier hat die Kontaktdaten der Reiter nicht.").into());
            }
            lang.pick("Étiquettes d’adresse", "Adressetiketten").to_string()
        }
        Labels::Judges(id) => {
            let c = class(state, id)?;
            let judges = state.dressage_judges(id);
            if judges.is_empty() {
                return Err(lang.pick("L’épreuve n’a pas de juge de dressage : attribuez-les dans Officiels.", "Die Prüfung hat keine Dressurrichter: unter Offizielle zuteilen.").into());
            }
            let list = views::start_list(state, id)?;
            for a in judges {
                for l in &list.lines {
                    let StartLine::Starter { entry, start_number, rider, horse, not_starting: false, .. } = l else { continue };
                    let e = &state.entries[&EntryId(*entry)];
                    // The judges need the markings, not the owner.
                    let signalement = e.horses_on_entry.first().and_then(|h| h["pferd_signalement"].as_str()).map(str::to_string).or_else(|| e.horse.as_ref().and_then(|h| state.horses.get(h)).map(|h| h.signalement.clone())).unwrap_or_default();
                    out.push(json!({ "lines": [
                        line(format!("{} {} · {}", lang.pick("N°", "Nr."), start_number.map(|n| n.to_string()).unwrap_or_else(|| "–".into()), long_date(&c.date, lang, false)), true),
                        line(format!("{} {} · {}", lang.pick("Épreuve", "Prüfung"), c.number, crate::views::class_title(state, c)), false),
                        line(rider_name(state, e, rider), true),
                        line(horse.clone(), false),
                        json!({ "text": signalement, "size": 7, "muted": true }),
                        line(format!("{} {} {}", lang.pick("Juge", "Richter"), a.letter.clone().unwrap_or_default(), person_name(state, &a.person)), false),
                    ] }));
                }
            }
            format!("{} {}", lang.pick("Étiquettes des juges", "Richteretiketten"), c.number)
        }
        Labels::Prizes(id) => {
            let c = class(state, id)?;
            let prizes = state.prizes.get(&c.published).ok_or_else(|| lang.pick("L’épreuve n’a pas de prix (Prix… au classement).", "Die Prüfung hat keine Preise (Preise… bei der Rangliste).").to_string())?;
            let event = state.event.as_ref().and_then(|e| e.title.first().cloned()).unwrap_or_default();
            let ranks = prizes.amounts.len().max(prizes.natural.len());
            for i in 0..ranks {
                let amount = prizes.amounts.get(i).filter(|a| !a.is_zero()).map(|a| format!("CHF {}.–", a.round_dp(2).normalize()));
                let natural = prizes.natural.get(i).filter(|n| !n.is_empty()).cloned();
                let mut lines = vec![json!({ "text": event, "size": 7, "muted": true })];
                // Rosson's first line: the sponsor.
                if !c.sponsor.trim().is_empty() {
                    lines.push(json!({ "text": c.sponsor.trim(), "bold": true, "size": 10 }));
                }
                lines.push(line(format!("{} {} · {}", lang.pick("Épreuve", "Prüfung"), c.number, crate::views::class_title(state, c)), false));
                lines.push(json!({ "text": format!("{}{}", i + 1, lang.pick(if i == 0 { "er rang" } else { "e rang" }, ". Rang")), "bold": true, "size": 12 }));
                lines.extend(amount.map(|a| line(a, true)));
                lines.extend(natural.map(|n| line(n, false)));
                out.push(json!({ "lines": lines }));
            }
            format!("{} {}", lang.pick("Étiquettes des prix", "Preisetiketten"), c.number)
        }
    };
    let mut data = base(state, lang, &title, printed);
    data["sheet"] = sheet.layout();
    data["skip"] = json!(skip.min(23));
    data["labels"] = Value::Array(out);
    Ok(Print { template: Template::Labels, data, name: title })
}

/// Letters to the riders (one per rider with entries, by name): the address for a window
/// envelope (`right`, or left), the office's `text`, the rider's entries with their fees.
pub fn letters(state: &State, text: &str, right: bool, place_date: &str, lang: Language, printed: &str) -> Print {
    let mut by_rider: BTreeMap<PersonId, Vec<&Entry>> = BTreeMap::new();
    for e in state.entries.values().filter(|e| e.status == EntryStatus::Entered && e.se_status != "abgemeldet" && !e.waiting_list) {
        by_rider.entry(e.rider.clone()).or_default().push(e);
    }
    let mut riders: Vec<(&Person, Vec<&Entry>)> = by_rider.into_iter().filter_map(|(r, e)| state.persons.get(&r).map(|p| (p, e))).collect();
    riders.sort_by(|(a, _), (b, _)| (&a.last_name, &a.first_name).cmp(&(&b.last_name, &b.first_name)));
    let letters: Vec<Value> = riders
        .into_iter()
        .map(|(p, entries)| {
            let c = state.contacts.get(&p.id).cloned().unwrap_or_default();
            let address: Vec<String> = [c.salutation.clone(), format!("{} {}", p.first_name, p.last_name), c.street.clone(), format!("{} {}", c.postcode, p.town).trim().to_string(), if c.country == "CH" { String::new() } else { c.country.clone() }]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect();
            let mut total = Decimal::ZERO;
            let mut rows: Vec<(String, Value)> = entries
                .iter()
                .map(|e| {
                    let published = state.published_classes.get(&e.published);
                    let fee = published.and_then(|p| p.fee).unwrap_or_default();
                    total += fee;
                    // Where it starts: the first section with this entry.
                    let start = state.starts.iter().find_map(|(c, s)| s.iter().find(|s| s.entry == e.id).map(|s| (c, s)));
                    let class = start.and_then(|(c, _)| state.classes.get(c));
                    let date = published.map(|p| p.date.clone()).unwrap_or_default();
                    (
                        format!("{date} {:0>6}", published.map(|p| p.number.clone()).unwrap_or_default()),
                        json!({
                            "date": long_date(&date, lang, false),
                            "time": class.map(|c| c.time_text.clone()).filter(|t| !t.is_empty()),
                            "class": class.map(|c| c.number.clone()).or_else(|| published.map(|p| p.number.clone())).unwrap_or_default(),
                            "category": published.map(|p| p.category_text.clone()).unwrap_or_default(),
                            "number": start.and_then(|(_, s)| s.start_number),
                            "horse": views_horse(state, e),
                            "fee": format!("{:.2}", fee),
                        }),
                    )
                })
                .collect();
            rows.sort_by(|a, b| a.0.cmp(&b.0));
            json!({ "address": address, "entries": rows.into_iter().map(|(_, r)| r).collect::<Vec<_>>(), "total": format!("{:.2}", total) })
        })
        .collect();
    let title = lang.pick("Lettres aux cavaliers", "Briefe an die Reiter").to_string();
    let mut data = base(state, lang, &title, printed);
    data["letters"] = Value::Array(letters);
    data["text"] = json!(text);
    data["window"] = json!(if right { "right" } else { "left" });
    data["place_date"] = json!(place_date);
    Print { template: Template::Letters, data, name: title }
}

/// A starter: the entry, its start number and time.
type Starter = (EntryId, Option<u32>, Option<String>);

/// Who starts in a class, in start order (not those who do not start).
fn starting(state: &State, id: &ClassId) -> Result<Vec<Starter>, String> {
    Ok(views::start_list(state, id)?
        .lines
        .into_iter()
        .filter_map(|l| match l {
            StartLine::Starter { entry, start_number, time, not_starting: false, .. } => Some((EntryId(entry), start_number, time)),
            _ => None,
        })
        .collect())
}

/// The stages a jumping class can have, in the order they are ridden (the scoring table's
/// field suffixes).
const STAGES: [&str; 6] = ["initial_1", "initial_2", "stechen_1", "stechen_2", "stechen_3", "stechen_4"];

/// One round (or phase, jump-off, winning round) of a class, as a card has it.
#[derive(Debug, Clone, PartialEq)]
struct Stage {
    /// Index in `STAGES`.
    at: usize,
    /// Penalty points written (table A), or points (style, Punktespringen: `points`).
    faults: bool,
    points: bool,
    /// A style class: the style judges' note, less the penalties.
    style: bool,
    /// The ideal-time table: no time penalties (ranked on the gap to the ideal time).
    ideal: bool,
    /// Timed.
    timed: bool,
    /// Earlier stages whose penalties (or time) this one's total adds.
    carry_faults: Vec<usize>,
    carry_time: Vec<usize>,
}

/// The rounds of a class from its scoring table: which there are, whether each counts penalties,
/// points or time (table C: time only, knock-downs in seconds), and what each carries over from
/// the earlier ones — the penalties of round 1 in round 2 and in the winning round, of phase 1
/// in a special two-phase class's phase 2, sometimes round 1's time too. Read from the table's
/// blocks: the block ranking on a stage adds the earlier stages' fields it sums.
fn stages(state: &State, c: &Class) -> (Vec<Stage>, bool, bool) {
    use scoring::jumping::table::{Phases, ScoringTable};
    let table = c.scoring_table.and_then(|t| state.scoring_tables.get(&t)).and_then(|d| ScoringTable::from_definition(d.id, &d.code, &d.name, &d.definition).ok());
    let Some(table) = table else { return (Vec::new(), false, false) };
    let two_phase = table.phases != Phases::None;
    let mut fields: Vec<String> = crate::results::fields(state, c);
    for b in &table.blocks {
        fields.extend(b.required.iter().chain(&b.time).chain(&b.faults).cloned());
    }
    let has = |kind: &str, at: usize| fields.iter().any(|f| *f == format!("{kind}_{}", STAGES[at]));
    let latest = |b: &scoring::jumping::table::Block| (0..STAGES.len()).rev().find(|&i| b.required.iter().any(|f| f.ends_with(STAGES[i])));
    let mut out = Vec::new();
    for (at, stage) in STAGES.iter().enumerate() {
        // Two-phase classes: phase 2 is in the second round's fields (timekeeping may write it
        // in the jump-off columns; there is no jump-off).
        if two_phase && at >= 2 {
            break;
        }
        let (faults, timed) = (has("strafpunkte", at), has("zeit", at));
        if !faults && !timed {
            continue;
        }
        let own_faults = format!("strafpunkte_{stage}");
        let own_time = format!("zeit_{stage}");
        let points = table.blocks.iter().any(|b| b.points && b.faults.contains(&own_faults));
        let style = points && table.code.starts_with("STIL");
        let ideal = table.blocks.iter().any(|b| b.ideal_time && b.required.iter().any(|f| f.ends_with(stage)));
        let (mut carry_faults, mut carry_time) = (Vec::new(), Vec::new());
        for b in table.blocks.iter().filter(|b| latest(b) == Some(at)) {
            for (e, earlier) in STAGES.iter().enumerate().take(at) {
                if faults && b.faults.iter().any(|f| f.ends_with(earlier)) && !carry_faults.contains(&e) {
                    carry_faults.push(e);
                }
                // Time carried only when added to this stage's own (not a tie on round 1's time).
                if timed && b.time.contains(&own_time) && b.time.iter().any(|f| f.ends_with(earlier)) && !carry_time.contains(&e) {
                    carry_time.push(e);
                }
            }
        }
        out.push(Stage { at, faults, points, style, ideal, timed, carry_faults, carry_time });
    }
    (out, two_phase, table.winning_round)
}

/// The rounds a card has: a row of obstacle boxes each (14 in a round or phase 1, 7 in a phase
/// 2 or a jump-off), below it what the jury adds up — penalties: (carried over +) obstacles +
/// time penalties; time: (carried over +) time + correction; table C: time + penalty seconds +
/// correction — and the totals (penalties or points, time) side by side. A class without a
/// table: a round and a jump-off. Team cards: always two rounds and a jump-off.
fn rounds(state: &State, c: &Class, lang: Language, team: bool) -> Vec<Value> {
    let (mut list, two_phase, winning_round) = stages(state, c);
    let plain = |at: usize| Stage { at, faults: true, points: false, style: false, ideal: false, timed: true, carry_faults: Vec::new(), carry_time: Vec::new() };
    if list.is_empty() {
        list = vec![plain(0), plain(2)];
    }
    if team {
        if !list.iter().any(|s| s.at == 1) {
            let first = list.iter().find(|s| s.at == 0).cloned().unwrap_or_else(|| plain(0));
            list.insert(1, Stage { at: 1, carry_faults: Vec::new(), carry_time: Vec::new(), ..first });
        }
        if !two_phase && !list.iter().any(|s| s.at >= 2) {
            list.push(plain(2));
        }
    }
    let rounds = list.iter().any(|s| s.at == 1);
    let jump_offs = list.iter().filter(|s| s.at >= 2).count();
    let name = |at: usize| -> (&str, &str) {
        match at {
            0 if two_phase => (lang.pick("Phase 1", "Phase 1"), lang.pick("phase 1", "Phase 1")),
            1 if two_phase => (lang.pick("Phase 2", "Phase 2"), lang.pick("phase 2", "Phase 2")),
            0 if rounds => (lang.pick("1re manche", "1. Umgang"), lang.pick("1re m.", "1. Umg.")),
            0 => (lang.pick("Parcours", "Umgang"), lang.pick("parcours", "Umgang")),
            1 => (lang.pick("2e manche", "2. Umgang"), lang.pick("2e m.", "2. Umg.")),
            2 if winning_round => (lang.pick("Round des gagnants", "Siegerrunde"), lang.pick("round", "Siegerr.")),
            _ if jump_offs == 1 => (lang.pick("Barrage", "Stechen"), lang.pick("barrage", "Stechen")),
            2 => (lang.pick("1er barrage", "1. Stechen"), lang.pick("1er barr.", "1. St.")),
            3 => (lang.pick("2e barrage", "2. Stechen"), lang.pick("2e barr.", "2. St.")),
            4 => (lang.pick("3e barrage", "3. Stechen"), lang.pick("3e barr.", "3. St.")),
            _ => (lang.pick("4e barrage", "4. Stechen"), lang.pick("4e barr.", "4. St.")),
        }
    };
    let carried = |what: &str, from: &[usize]| {
        let from = if from == [0, 1] && !two_phase { lang.pick("1re + 2e m.", "1. + 2. Umg.").to_string() } else { from.iter().map(|&e| name(e).1).collect::<Vec<_>>().join(" + ") };
        format!("{what} {from}")
    };
    list.iter()
        .map(|s| {
            let short = s.at >= 2 || (two_phase && s.at == 1);
            // Each sum: what the jury adds up, and its total (slot 0: penalties or points, slot 1:
            // time — side by side in the same place on every card, to rank them).
            let mut groups = Vec::new();
            // `minus`: the parts are taken away from the first (points, the style note).
            let group = |boxes: Vec<String>, minus: bool, total: &str, team: &str, slot: u8| json!({ "boxes": boxes, "minus": minus, "total": total, "team": team, "slot": slot });
            let time_penalties = lang.pick("Pén. temps", "Zeitfehler").to_string();
            if s.style {
                // The style judges' note (their sheets), less the round's penalties.
                let boxes = vec![lang.pick("Note de style", "Stilnote").to_string(), lang.pick("Pén. obstacles", "Hindernisfehler").to_string(), time_penalties];
                groups.push(group(boxes, true, lang.pick("Total des points", "Total Punkte"), lang.pick("Points équipe", "Punkte Equipe"), 0));
            } else if s.points {
                // Points, less the time penalties.
                let boxes = vec![lang.pick("Points", "Punkte").to_string(), time_penalties];
                groups.push(group(boxes, true, lang.pick("Total des points", "Total Punkte"), lang.pick("Points équipe", "Punkte Equipe"), 0));
            } else if s.faults {
                let mut boxes: Vec<String> = Vec::new();
                if !s.carry_faults.is_empty() {
                    boxes.push(carried(lang.pick("Pén.", "Fehler"), &s.carry_faults));
                }
                boxes.push(lang.pick("Pén. obstacles", "Hindernisfehler").into());
                // The ideal-time table has no time penalties.
                if !s.ideal {
                    boxes.push(time_penalties);
                }
                groups.push(group(boxes, false, lang.pick("Total pén.", "Total Fehler"), lang.pick("Pén. équipe", "Fehler Equipe"), 0));
            }
            if s.timed {
                let mut boxes: Vec<String> = Vec::new();
                if !s.carry_time.is_empty() {
                    boxes.push(carried(lang.pick("Temps", "Zeit"), &s.carry_time));
                }
                boxes.push(lang.pick("Temps", "Zeit").into());
                if !s.faults {
                    boxes.push(lang.pick("Pén. en secondes", "Strafsekunden").into());
                }
                boxes.push(lang.pick("Correction de temps", "Zeitkorrektur").into());
                groups.push(group(boxes, false, lang.pick("Temps total", "Total Zeit"), lang.pick("Temps équipe", "Zeit Equipe"), 1));
            }
            json!({ "label": name(s.at).0, "obstacles": if short { 7 } else { 14 }, "jump_off": s.at >= 2, "groups": groups })
        })
        .collect()
}

/// What a start card or a team card says on the class.
fn card_class(state: &State, c: &Class, lang: Language) -> Value {
    let published = state.published_classes.get(&c.published);
    json!({
        "number": c.number,
        "name": crate::views::class_title(state, c),
        "category": published.map(|p| p.category_text.clone()).unwrap_or_default(),
        "scoring": c.scoring_text.clone().or_else(|| published.and_then(|p| p.scoring_text.clone())).unwrap_or_default(),
        "date": long_date(&c.date, lang, false),
        "time": c.time_text,
    })
}

fn card_words(data: &mut Value, lang: Language) {
    for (k, fr, de) in [
        ("remarks", "Remarques", "Bemerkungen"),
        ("rank", "Rang", "Rang"),
        ("born", "Date de naissance", "Geburtsdatum"),
        ("owner", "Propriétaire", "Besitzer"),
        ("winning_points", "Somme de points", "Gewinnpunkte"),
        ("points_short", "SoP", "GWP"),
        ("horse_short", "cheval", "Pferd"),
        ("rider_short", "cav.", "Reiter"),
        ("pair_short", "couple", "Paar"),
        ("club", "Société", "Verein"),
        ("start_time", "Heure", "Zeit"),
        ("dropped", "Biffé", "Streichr."),
        ("team_total", "Total de l’équipe", "Total Equipe"),
    ] {
        data["labels"][k] = json!(lang.pick(fr, de));
    }
}

/// Start cards: one per starter, two to an A4 page (A5 each), for the jury to write the result:
/// the class's own rounds (from its scoring table), with obstacle boxes, penalties, time, time
/// penalties and total; the rank. The rider's birth date when known (age categories).
pub fn start_cards(state: &State, id: &ClassId, lang: Language, printed: &str) -> Result<Print, String> {
    start_cards_with(state, id, &CardOptions::default(), lang, printed)
}

/// Which start cards and how (the office's choice when printing them).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardOptions {
    /// One starter's card only (a late entry, a card lost); else the whole class's.
    pub entry: Option<EntryId>,
    /// Team cards: one team's only.
    pub team: Option<TeamId>,
    /// A card an A5 page; else two to an A4 page, to cut in two.
    pub a5: bool,
}

pub fn start_cards_with(state: &State, id: &ClassId, options: &CardOptions, lang: Language, printed: &str) -> Result<Print, String> {
    let c = class(state, id)?;
    let points = |d: Option<Decimal>| d.map(|d| d.normalize().to_string()).unwrap_or_else(|| "0".into());
    let mut starters = starting(state, id)?;
    if let Some(entry) = &options.entry {
        starters.retain(|(e, _, _)| e == entry);
        if starters.is_empty() {
            return Err(format!("entry {} does not start in class {}", entry, c.number));
        }
    }
    let cards: Vec<Value> = starters
        .into_iter()
        .map(|(entry, number, time)| {
            let e = &state.entries[&entry];
            let (rider, town, club) = rider_parts(state, e);
            let contact = state.contacts.get(&e.rider).cloned().unwrap_or_default();
            let horse = e.horses_on_entry.first();
            let field = |k: &str| horse.and_then(|h| h[k].as_str()).map(str::trim).unwrap_or("").to_string();
            let pedigree = [field("vater_name"), field("vater_der_mutter_name")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" × ");
            let address = [contact.street.clone(), format!("{} {town}", contact.postcode).trim().to_string()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
            let phone = [contact.phone_mobile.clone(), contact.phone_private.clone()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
            let owner = [field("pferd_besitzer"), field("pferd_mitbesitzer")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" / ");
            // Swiss style: 03.05.2010.
            let born = contact.birth_date.as_deref().and_then(|d| Some(format!("{}.{}.{}", d.get(8..10)?, d.get(5..7)?, d.get(..4)?))).unwrap_or_default();
            json!({
                "number": number,
                "time": time,
                "rider": rider_name(state, e, &rider),
                "born": born,
                "licence": e.rider.0.to_string(),
                "club": club,
                "address": if address.is_empty() { town } else { address },
                "phone": phone,
                "horse": views_horse(state, e),
                "passport": e.horse.as_ref().filter(|h| h.0 != 5).map(|h| h.0.to_string()).unwrap_or_default(),
                "signalement": field("pferd_signalement"),
                "pedigree": pedigree,
                "owner": owner,
                "level": e.stage.map(|s| s.to_string()).unwrap_or_default(),
                "hc": e.hors_concours,
                "gwp": [points(e.winning_points.rider), points(e.winning_points.horse), points(e.winning_points.pair)],
            })
        })
        .collect();
    let title = match (&options.entry, cards.first()) {
        (Some(_), Some(card)) => format!("{} {} – {}", lang.pick("Carte de départ", "Startkarte"), c.number, card["rider"].as_str().unwrap_or("")),
        _ => format!("{} {}", lang.pick("Cartes de départ", "Startkarten"), c.number),
    };
    let mut data = base(state, lang, &title, printed);
    data["cards"] = Value::Array(cards);
    data["paper"] = json!(if options.a5 { "a5" } else { "a4" });
    data["class"] = card_class(state, c, lang);
    data["rounds"] = Value::Array(rounds(state, c, lang, false));
    card_words(&mut data, lang);
    Ok(Print { template: Template::StartCards, data, name: title })
}

/// Team start cards: one page per team of a team class, in the rotation (see `teamcards.typ`).
pub fn team_cards(state: &State, id: &ClassId, lang: Language, printed: &str) -> Result<Print, String> {
    team_cards_with(state, id, &CardOptions::default(), lang, printed)
}

/// Team start cards, as the office chooses: every team's or one's; an A4 page each, or an A5
/// page (the same card, smaller).
pub fn team_cards_with(state: &State, id: &ClassId, options: &CardOptions, lang: Language, printed: &str) -> Result<Print, String> {
    let c = class(state, id)?;
    if c.teams.is_empty() {
        return Err(lang.pick("Ce n’est pas une épreuve par équipes.", "Das ist keine Mannschaftsprüfung.").into());
    }
    if let Some(team) = options.team.as_ref().filter(|t| !c.teams.contains(t)) {
        return Err(format!("team {} is not in class {}", team, c.number));
    }
    let mut starts = state.starts.get(id).cloned().unwrap_or_default();
    starts.sort_by_key(|s| s.order);
    let points = |d: Option<Decimal>| d.map(|d| d.normalize().to_string()).unwrap_or_else(|| "0".into());
    let teams: Vec<Value> = c
        .teams
        .iter()
        .enumerate()
        .filter(|(_, t)| options.team.as_ref().is_none_or(|one| one == *t))
        .filter_map(|(i, t)| {
            let team = state.teams.get(t)?;
            let riders: Vec<Value> = starts
                .iter()
                .filter_map(|s| state.entries.get(&s.entry).filter(|e| e.team.as_ref() == Some(t)).map(|e| (s, e)))
                .map(|(s, e)| {
                    let (rider, town, _) = rider_parts(state, e);
                    let field = |k: &str| e.horses_on_entry.first().and_then(|h| h[k].as_str()).unwrap_or("").to_string();
                    let short = rider.rsplit(' ').next().unwrap_or(&rider).to_string();
                    json!({
                        "number": s.start_number.map(|n| n.to_string()).unwrap_or_default(),
                        "short": short,
                        "horse": views_horse(state, e),
                        "signalement": field("pferd_signalement"),
                        "passport": e.horse.as_ref().filter(|h| h.0 != 5).map(|h| h.0.to_string()).unwrap_or_default(),
                        "rider": format!("{}, {town}", rider_name(state, e, &rider)),
                        "licence": e.rider.0.to_string(),
                        "level": e.stage.map(|s| s.to_string()).unwrap_or_default(),
                        "gwp_horse": points(e.winning_points.horse),
                        "gwp_rider": points(e.winning_points.rider),
                        "gwp_pair": points(e.winning_points.pair),
                        "not_starting": e.status == EntryStatus::NotStarting,
                    })
                })
                .collect();
            Some(json!({ "order": i + 1, "name": team.name, "riders": riders }))
        })
        .collect();
    let title = match (&options.team, teams.first()) {
        (Some(_), Some(team)) => format!("{} {} – {}", lang.pick("Carte de départ de l’équipe", "Equipen-Startkarte"), c.number, team["name"].as_str().unwrap_or("")),
        _ => format!("{} {}", lang.pick("Cartes de départ des équipes", "Equipen-Startkarten"), c.number),
    };
    let mut data = base(state, lang, &title, printed);
    data["teams"] = Value::Array(teams);
    data["paper"] = json!(if options.a5 { "a5" } else { "a4" });
    data["class_number"] = json!(c.number);
    data["class_name"] = json!(crate::views::class_title(state, c));
    data["class"] = card_class(state, c, lang);
    // Teams' rounds: as the scoring table has them, and always two rounds and a jump-off (team
    // classes usually go over two rounds, and teams tied for first jump off, whatever the table).
    let team_rounds = rounds(state, c, lang, true);
    data["rounds"] = Value::Array(team_rounds);
    // Teams of four: the worst result of a round is dropped (S7).
    data["dropped"] = json!(state.published_classes.get(&c.published).and_then(|p| p.team_size) == Some(4));
    card_words(&mut data, lang);
    for (k, fr, de) in [
        ("team_cards", "Cartes de départ des équipes", "Equipen-Startkarten"),
        ("team", "Équipe", "Equipe"),
        ("horse_short", "cheval", "Pferd"),
        ("rider_short", "cav.", "Reiter"),
        ("pair_short", "couple", "Paar"),
        ("signalement", "Signalement", "Signalement"),
        ("level", "Niv.", "Stufe"),
    ] {
        data["labels"][k] = json!(lang.pick(fr, de));
    }
    Ok(Print { template: Template::TeamCards, data, name: title })
}

/// Which judges' sheet a style class uses (from its category): licence classes ("B100 … avec
/// notes de style"), two judges; the initiation classes E40–E70 (Swiss Equestrian's "Einsteiger-
/// prüfung Springen" sheet) and the accompanied cavaletti class ECav ("Führzügel-Cavaletti-
/// Prüfung"), one judge, half marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleSheet {
    Licence,
    Initiation,
    Cavaletti,
}

impl StyleSheet {
    pub fn of(category: &str) -> StyleSheet {
        let words: Vec<String> = category.split(|c: char| !c.is_alphanumeric()).map(|w| w.to_lowercase()).collect();
        if words.iter().any(|w| w.starts_with("ecav")) {
            StyleSheet::Cavaletti
        } else if words.iter().any(|w| w.len() >= 3 && w.len() <= 4 && w.starts_with('e') && w[1..].chars().all(|c| c.is_ascii_digit())) {
            StyleSheet::Initiation
        } else {
            StyleSheet::Licence
        }
    }
}

/// The criteria of a style sheet, with their guiding ideas: Gestion Concours' sheet for licence
/// classes (the office's of 2019, five criteria), Swiss Equestrian's own for E and ECav (in
/// French and German as Swiss Equestrian words them).
fn style_criteria(kind: StyleSheet, lang: Language) -> Vec<(&'static str, Vec<&'static str>)> {
    if lang == Language::It {
        // The French sheet, in Italian.
        return style_criteria(kind, Language::Fr).into_iter().map(|(c, ideas)| (crate::italian::tr(c, ""), ideas.into_iter().map(|i| crate::italian::tr(i, "")).collect())).collect();
    }
    let fr = lang == Language::Fr;
    match (kind, fr) {
        (StyleSheet::Licence, true) => vec![
            ("Tenue, présentation", vec!["Habillement", "Harnachement", "Toilettage", "Entrée en piste correcte, cheval à la rêne"]),
            ("Conduite", vec!["Choix de la ligne"]),
            ("Correction dans l'emploi des aides", vec!["Choix et régularité du rythme dans les lignes, les conversions et l'abord de l'obstacle", "Transitions", "Efficacité"]),
            ("Style", vec!["Position du buste, de l'assiette et de la main", "Position de la jambe", "Regard du cavalier"]),
            ("Impression générale", vec!["Souplesse et liant dans le mouvement du saut", "Harmonie et légèreté"]),
        ],
        (StyleSheet::Licence, false) => vec![
            ("Tenue, Vorführung", vec!["Bekleidung", "Zäumung und Sattlung", "Pflegezustand des Pferdes", "Korrektes Einreiten, Pferd am Zügel"]),
            ("Führung", vec!["Linienwahl"]),
            ("Korrekte Hilfengebung", vec!["Regelmässigkeit der gewählten Kadenz auf geraden Linien, in Wendungen und im Anreiten der Hindernisse", "Übergänge", "Einwirkung"]),
            ("Stil", vec!["Haltung des Oberkörpers, des Sitzes und der Hände", "Schenkellage", "Blick des Reiters"]),
            ("Allgemeiner Eindruck", vec!["Losgelassenheit und lockeres Eingehen in die Bewegung beim Springen", "Harmonie und Leichtigkeit"]),
        ],
        (StyleSheet::Initiation, true) => vec![
            ("Tenue, présentation", vec!["Habillement", "Harnachement", "Toilettage", "Entrée en piste correcte et salut"]),
            ("Conduite", vec!["Choix de la ligne", "Conversions : fluidité, position correcte, galop sur le bon pied"]),
            ("Correction dans l'emploi des aides", vec!["Choix et régularité du rythme dans les lignes, les conversions et l'abord de l'obstacle", "Transitions", "Coordination sensible des aides"]),
            ("Assiette, action des aides, style", vec!["Décontraction et souplesse dans le mouvement du saut", "Assiette équilibrée, position du buste", "Jambes tranquilles à la sangle, longueur appropriée des étriers", "Tenue correcte des rênes, mains tranquilles", "Regard du cavalier / de la cavalière"]),
            ("Impression générale", vec!["Présentation harmonieuse", "Comportement approprié avec le cheval (horsemanship)"]),
        ],
        (StyleSheet::Initiation, false) => vec![
            ("Tenue / Vorstellung", vec!["Bekleidung", "Zäumung und Sattlung", "Pflegezustand des Pferdes/Ponys", "Korrektes Einreiten und Grüssen"]),
            ("Führung", vec!["Linienwahl", "Wendungen: fliessend, korrekte Stellung, richtiger Galopp"]),
            ("Hilfengebung", vec!["Regelmässigkeit und Angemessenheit der gewählten Kadenz auf geraden Linien, in Wendungen und im Anreiten der Hindernisse", "Übergänge", "Gefühlvolles Zusammenwirken der Hilfen"]),
            ("Sitz / Einwirkung / Stil", vec!["Losgelassenheit und lockeres Eingehen in die Bewegung beim Springen", "Ausbalancierter Sitz, Haltung des Oberkörpers", "Sichere Schenkellage am Gurt, passende Bügellänge", "Ruhige und korrekte Zügelhaltung", "Blick der Reiterin/des Reiters"]),
            ("Allgemeiner Eindruck", vec!["Harmonische Vorstellung", "Angemessener Umgang mit dem Pferd (Horsemanship)"]),
        ],
        (StyleSheet::Cavaletti, true) => vec![
            ("Tenue, présentation", vec!["Habillement de l'enfant et de l'accompagnateur/trice", "Harnachement", "Toilettage", "Entrée en piste correcte et salut"]),
            ("Cavalier / cavalière", vec!["Décontraction et souplesse", "Assiette équilibrée, position du buste (assiette légère sur les perches / cavaletti)", "Jambes à la sangle, longueur appropriée des étriers", "Tenue correcte des rênes, mains tranquilles", "Regard du cavalier / de la cavalière", "Action du cavalier / de la cavalière"]),
            ("Exécution", vec!["Comportement de l'accompagnateur/trice avec le poney/cheval et l'enfant (seulement sécurité de la paire ; la corde ne devrait idéalement pas être tendue pendant tout le parcours)", "Choix de la ligne, conversions", "Choix et régularité du rythme dans les lignes droites, les conversions et à l'abord des cavaletti/perches", "Choix adapté de l'allure"]),
            ("Impression générale", vec!["Présentation harmonieuse", "Comportement approprié avec le poney/cheval (horsemanship)"]),
        ],
        (StyleSheet::Cavaletti, false) => vec![
            ("Tenue / Vorstellung", vec!["Bekleidung Kind und Führperson", "Zäumung und Sattlung", "Pflegezustand Pferd/Pony", "Korrektes Einreiten und Grüssen"]),
            ("Reiter:in", vec!["Losgelassenheit und lockeres Eingehen in die Bewegung", "Ausbalancierter Sitz, Haltung des Oberkörpers (Entlastungssitz bei Stangen/Cavaletti)", "Schenkellage am Gurt, passende Bügellänge", "Ruhige und korrekte Zügelhaltung", "Blick der Reiterin/des Reiters", "Einwirkung Reiter:in"]),
            ("Ausführung", vec!["Umgang der Führperson mit dem Pony/Pferd und dem Kind (nur Sicherung des Paares / Führzügel sollte idealerweise die ganze Zeit durchhängen)", "Linienwahl/Wendungen", "Regelmässigkeit der gewählten Kadenz auf geraden Linien, in Wendungen und im Anreiten der Cavaletti/Stangen", "Passende Wahl der Gangarten"]),
            ("Allgemeiner Eindruck", vec!["Harmonische Vorstellung", "Angemessener Umgang mit dem Pferd (Horsemanship)"]),
        ],
    }
}

/// Style classes: the style judges' sheets (A5, a page per rider and judge, in start order;
/// licence classes judge 1's set, then judge 2's), on Gestion Concours' layout: the class and
/// start number, who and what is judged, the scale of marks, the criteria with their guiding
/// ideas and room for the mark and remarks, the total, the final result (the judges' points
/// less the faults), time and rank. The judges are the class's style judges (EquiEvents' own
/// role; Rosson took the jumping judges, "R").
pub fn style_sheets(state: &State, id: &ClassId, lang: Language, printed: &str) -> Result<Print, String> {
    let c = class(state, id)?;
    let published = state.published_classes.get(&c.published);
    let category = published.map(|p| p.category_text.clone()).unwrap_or_default();
    let kind = StyleSheet::of(&category);
    let scoring = c.scoring_text.clone().or_else(|| published.and_then(|p| p.scoring_text.clone())).unwrap_or_default();
    let untimed = c.scoring_table.and_then(|t| state.scoring_tables.get(&t)).is_some_and(|t| t.code == "STILOZM") || scoring.to_lowercase().contains("sans chrono") || scoring.to_lowercase().contains("ohne zeit");
    let mut assigned: Vec<&Assignment> = state.assignments.values().filter(|a| a.class.as_ref() == Some(id) && a.role == show::model::STYLE_JUDGE).collect();
    assigned.sort_by_key(|a| a.id.0.parse::<u64>().unwrap_or(u64::MAX));
    let slots = if kind == StyleSheet::Licence { 2 } else { 1 };
    let place = state.event.as_ref().map(|e| e.place.clone()).unwrap_or_default();
    let starters = starting(state, id)?;
    let mut pages = Vec::new();
    for slot in 0..slots {
        let judge = assigned.get(slot).map(|a| person_name(state, &a.person)).unwrap_or_default();
        for (entry, number, _) in &starters {
            let e = &state.entries[entry];
            let (rider, _, _) = rider_parts(state, e);
            pages.push(json!({
                "judge_label": format!("{} {}", lang.pick("Juge", "Richter"), slot + 1),
                "judge": judge,
                "rider": rider,
                "rider_id": e.rider.0.to_string(),
                "horse": views_horse(state, e),
                "horse_id": e.horse.as_ref().filter(|h| h.0 != 5).map(|h| h.0.to_string()).unwrap_or_default(),
                "number": number.map(|n| n.to_string()).unwrap_or_default(),
                "flag": if e.hors_concours { lang.pick("Hors concours", "Ausser Konkurrenz") } else { "" },
            }));
        }
    }
    let heading = match kind {
        StyleSheet::Licence => {
            let base = format!("{} {category}", lang.pick("Épreuve", "Stilprüfung")).trim().to_string();
            let more = if untimed { lang.pick(" avec notes de style, sans chrono", ", ohne Zeitmessung") } else { lang.pick(" avec notes de style", "") };
            format!("{base}{more}")
        }
        StyleSheet::Initiation => format!("{} · {category}", lang.pick("Épreuve d’initiation saut", "Einsteigerprüfung Springen")),
        StyleSheet::Cavaletti => format!("{} · {category}", lang.pick("Épreuve accompagnée sur cavalettis", "Führzügel-Cavaletti-Prüfung")),
    };
    let (licence, fr) = (kind == StyleSheet::Licence, lang != Language::De);
    // The scale as each sheet words it (Swiss Equestrian's E sheets: "mal", in French; Italian
    // from the French).
    let scale: Vec<&str> = match (fr, licence) {
        (true, true) => vec!["Excellent", "Très bien", "Bien", "Assez bien", "Satisfaisant", "Suffisant", "Insuffisant", "Assez mauvais", "Mauvais", "Très mauvais", "Non exécuté"],
        (true, false) => vec!["Excellent", "Très bien", "Bien", "Assez bien", "Satisfaisant", "Suffisant", "Insuffisant", "Assez mal", "Mal", "Très mal", "Non exécuté"],
        (false, _) => vec!["ausgezeichnet", "sehr gut", "gut", "ziemlich gut", "befriedigend", "genügend", "mangelhaft", "ziemlich schlecht", "schlecht", "sehr schlecht", "nicht ausgeführt"],
    };
    let scale: Vec<&str> = if lang == Language::It { scale.into_iter().map(|w| crate::italian::tr(w, "")).collect() } else { scale };
    let title = format!("{} {}", lang.pick("Feuilles de juge de style", "Stilrichterblätter"), c.number);
    let mut data = base(state, lang, &title, printed);
    data["kind"] = json!(match kind {
        StyleSheet::Licence => "licence",
        StyleSheet::Initiation => "initiation",
        StyleSheet::Cavaletti => "cavaletti",
    });
    data["heading"] = json!(heading);
    data["class_number"] = json!(c.number);
    data["place"] = json!(place);
    data["date"] = json!(long_date(&c.date, lang, true));
    data["logo"] = c.logo.clone().unwrap_or(Value::Null);
    data["pages"] = Value::Array(pages);
    data["scale"] = json!(scale.iter().enumerate().map(|(i, s)| json!([(10 - i).to_string(), s])).collect::<Vec<_>>());
    data["criteria"] = json!(style_criteria(kind, lang).into_iter().map(|(name, ideas)| json!({ "name": name, "ideas": ideas })).collect::<Vec<_>>());
    for (k, f, d) in [
        ("sheet", "Feuille de juge", "Richterblatt"),
        ("place", "Concours à", "Turnier"),
        ("date", "Date", "Datum"),
        ("rider", if licence { "Cavalier(ière)" } else { "Participant(e)" }, if licence { "Reiter/in" } else { "Teilnehmer:in" }),
        ("horse", if licence { "Cheval" } else { "Cheval / poney" }, if licence { "Pferd" } else { "Pferd / Pony" }),
        ("judge", "Juge", "Richter:in"),
        ("class", "Épreuve", "Prüfung"),
        ("start", "Départ", "Start-Nr."),
        ("scale", "Barème des notes", "Notenskala"),
        ("halves", "(demi-notes possibles)", "(halbe Noten möglich)"),
        ("criterion", "Critère", "Kriterien"),
        ("ideas", "Idée directrice", "Leitgedanken"),
        ("mark", "Note", "Noten"),
        ("remarks", "Observations", "Bemerkungen"),
        ("total", "Total", "Total"),
        ("final", "Résultat final", "Schlussresultat"),
        ("judge_1", "Juge 1", "Richter 1"),
        ("judge_2", "Juge 2", "Richter 2"),
        ("judge_points", "Points juge", "Punkte Richter:in"),
        ("faults", "Fautes", "abzgl. Fehler"),
        ("points", "points", "Punkte"),
        ("total_points", "Total", "Total Punkte"),
        ("time", "Chrono", "Zeit"),
        ("rank", "Rang", "Rang"),
    ] {
        data["labels"][k] = json!(lang.pick(f, d));
    }
    Ok(Print { template: Template::StyleSheets, data, name: title })
}

/// The officials' plan: each official, their roles at the event, and day by day their classes.
pub fn officials_plan(state: &State, lang: Language, printed: &str) -> Print {
    let event = views::event_officials(state, lang.code());
    let mut people: BTreeMap<PersonId, (String, String)> = BTreeMap::new();
    for a in state.assignments.values() {
        people.entry(a.person.clone()).or_insert_with(|| {
            state.persons.get(&a.person).map(|p| (format!("{} {}", p.first_name, p.last_name), p.town.clone())).or_else(|| state.officials.get(&a.person).map(|o| (format!("{} {}", o.first_name, o.last_name), o.town.clone()))).unwrap_or_default()
        });
    }
    let mut officials: Vec<(String, Value)> = people
        .into_iter()
        .map(|(person, (name, town))| {
            let roles: Vec<String> = event.rows.iter().filter(|r| r.person == person.0).map(|r| match r.days {
                Some(d) => format!("{} ({d} {})", r.role_name, lang.pick("j.", "T.")),
                None => r.role_name.clone(),
            }).collect();
            let mut days: BTreeMap<String, Vec<(Option<u32>, String)>> = BTreeMap::new();
            for a in state.assignments.values().filter(|a| a.person == person) {
                let Some(c) = a.class.as_ref().and_then(|c| state.classes.get(c)) else { continue };
                let role = match &a.letter {
                    Some(l) if a.role.starts_with("DRI") => l.clone(),
                    // The style judge, as in the officials table.
                    _ if a.role == show::model::STYLE_JUDGE => "S".into(),
                    _ => a.role.clone(),
                };
                days.entry(c.date.clone()).or_default().push((c.position, format!("{} ({role})", c.number)));
            }
            let days: Vec<Value> = days
                .into_iter()
                .map(|(date, mut classes)| {
                    classes.sort();
                    json!({ "date": long_date(&date, lang, false), "classes": classes.into_iter().map(|(_, c)| c).collect::<Vec<_>>().join(" · ") })
                })
                .collect();
            (name.to_lowercase(), json!({ "name": name, "town": town, "event_roles": roles.join(", "), "days": days }))
        })
        .collect();
    officials.sort_by(|a, b| a.0.cmp(&b.0));
    let title = lang.pick("Plan des officiels", "Einsatzplan").to_string();
    let mut data = base(state, lang, &title, printed);
    data["officials"] = Value::Array(officials.into_iter().map(|(_, o)| o).collect());
    Print { template: Template::Officials, data, name: title }
}

/// Driving's control sheets (Rosson's `kontrollblatt.print.service.js`), a page per starter in
/// start order: cones — the gates of each round to tick and the round's sums (driven time,
/// obstacle faults, allowed time already filled in, other faults, time over × 0.5); derby — the
/// round and the obstacle sheet; marathon — an obstacle sheet per obstacle and starter, the
/// obstacle's number filled in (Rosson's had it blank: the office copied it for each obstacle).
/// The penalty lists are the driving rules' (Swiss Equestrian has them in French and German:
/// German in Italian prints, as its other texts without Italian).
pub fn control_sheets(state: &State, id: &ClassId, lang: Language, printed: &str) -> Result<Print, String> {
    let c = class(state, id)?;
    let kind = c.phase.as_ref().map(|p| p.kind).filter(|_| state.driving(c));
    let kind = match kind {
        Some(k @ (PhaseKind::Cones | PhaseKind::Derby | PhaseKind::Marathon)) => k,
        _ => return Err(lang.pick("Les feuilles de contrôle sont pour la maniabilité, le derby et le terrain.", "Kontrollblätter gibt es für Kegelfahren, Derby und Gelände.").into()),
    };
    let parent = c.phase.as_ref().and_then(|p| state.classes.get(&p.of)).unwrap_or(c);
    let published = state.published_classes.get(&c.published);
    let settings = c.options.driving.clone().unwrap_or_default();
    let list = views::start_list(state, id)?;
    // The rules' texts: German where Swiss Equestrian has no Italian.
    let rules = |fr: &str, de: &str| -> String { if lang == Language::It { de.to_string() } else { lang.pick(fr, de).to_string() } };
    let mut sheets = Vec::new();
    let obstacles: Vec<Option<u32>> = if kind == PhaseKind::Marathon { (1..=settings.obstacles.unwrap_or(5)).map(Some).collect() } else { vec![None] };
    for obstacle in obstacles {
        for line in &list.lines {
            let StartLine::Starter { entry, start_number, not_starting: false, .. } = line else { continue };
            let e = &state.entries[&EntryId(*entry)];
            let (rider, _, _) = rider_parts(state, e);
            let licences: Vec<String> = state.persons.get(&e.rider).map(|p| p.licences.split(',').map(str::trim).filter(|l| l.starts_with('F')).map(str::to_string).collect()).unwrap_or_default();
            let turnout = e.record["fahren_gespann"].as_str().map(str::to_string).or_else(|| e.record["fahren_gespann"].as_u64().map(|g| g.to_string())).unwrap_or_default();
            let width = e.record["wagenbreite"].as_str().map(str::to_string).or_else(|| e.record["wagenbreite"].as_u64().map(|w| w.to_string())).unwrap_or_default();
            // Cones: each round's allowed time for the driver's licence, with the turnout's bonus.
            let licence = state.driving_licence(c, e);
            let bonus = settings.bonus.get(&turnout).copied().unwrap_or_default();
            let rounds: Vec<Value> = match kind {
                PhaseKind::Cones => (1..=settings.rounds.unwrap_or(1))
                    .map(|r| {
                        let allowed = licence.as_ref().and_then(|l| settings.allowed.get(&format!("{r}:{l}"))).map(|a| (*a + bonus).normalize().to_string()).unwrap_or_default();
                        json!({ "label": if r == 1 { lang.pick("1re manche", "1. Umgang") } else { lang.pick("2e manche", "2. Umgang") }, "allowed": allowed })
                    })
                    .collect(),
                PhaseKind::Derby => vec![json!({ "label": "", "allowed": "" })],
                _ => Vec::new(),
            };
            sheets.push(json!({
                "name": rider,
                "carriage": format!("{} / {}", start_number.map(|n| n.to_string()).unwrap_or_default(), licences.join(",")),
                "class": format!("{}: {} / {} {turnout}", parent.number, published.map(|p| p.category_text.clone()).unwrap_or_default(), lang.pick("Attelage", "Gespann")),
                "width": width,
                "rounds": rounds,
                "obstacle": obstacle,
            }));
        }
    }
    if sheets.is_empty() {
        return Err(lang.pick("Personne au départ.", "Niemand am Start.").into());
    }
    let elimination = lang.pick("Élimination", "Elimination").to_string();
    let catalogue: Vec<Value> = match kind {
        PhaseKind::Marathon => MARATHON_PENALTIES.iter().map(|(fr, de, p)| json!({ "text": rules(fr, de), "penalty": if p.is_empty() { elimination.clone() } else { p.to_string() } })).collect(),
        PhaseKind::Derby => DERBY_PENALTIES
            .iter()
            .map(|(fr, de, p)| json!({ "text": rules(fr, de), "penalty": match *p { "" => elimination.clone(), "dq" => lang.pick("Disqualification du cheval, élimination du meneur", "Disqualifikation Pferd, Elimination Fahrer").to_string(), p => p.to_string() } }))
            .collect(),
        _ => Vec::new(),
    };
    let title = format!("{} {}", lang.pick("Feuilles de contrôle", "Kontrollblätter"), c.number);
    let mut data = base(state, lang, &title, printed);
    data["kind"] = json!(match kind {
        PhaseKind::Cones => "cones",
        PhaseKind::Derby => "derby",
        _ => "marathon",
    });
    data["sheets"] = Value::Array(sheets);
    data["catalogue"] = Value::Array(catalogue);
    data["info"] = json!(match kind {
        PhaseKind::Derby => rules("Dans l’épreuve partielle D, les pénalités suivantes (en secondes) sont attribuées :", "In Teilprüfung D (Geländehindernis) werden den Konkurrenten folgende Strafsekunden vergeben:"),
        _ => rules("Dans l’épreuve partielle B, les points de pénalité suivants sont attribués :", "In Teilprüfung B (Geländehindernis) werden den Konkurrenten folgende Strafpunkte vergeben:"),
    });
    data["timing"] = json!(rules(
        "Chronométrage aux obstacles : le juge d’obstacle mesure le temps en 1/100 de seconde, à partir du moment où la première partie de l’attelage pénètre dans la zone de pénalité jusqu’au moment où la première partie la quitte.",
        "Zeitmessung an den Hindernissen: Der Hindernisrichter nimmt die Zeit in 1/100 Sekunden. Die Zeit ist von dem Moment an zu nehmen, in dem der erste Teil des Gespannes in die Strafzone einfährt, bis zu dem Moment, in dem der erste Teil des Gespannes sie wieder verlässt.",
    ));
    for (k, fr, de) in [
        ("discipline", "Discipline attelage", "Disziplin Fahren"),
        ("title_cones", "Maniabilité – feuille de contrôle", "Kegelfahren – Kontrollblatt"),
        ("title_derby", "Derby de terrain – feuille de contrôle", "Geländederby – Kontrollblatt"),
        ("obstacle_marathon", "Terrain – feuille de contrôle de l’obstacle", "Gelände – Hindernis-Kontrollblatt"),
        ("obstacle_derby", "Derby de terrain – feuille de contrôle de l’obstacle", "Geländederby – Hindernis-Kontrollblatt"),
        ("name", "Concurrent·e", "Konkurrent/in"),
        ("carriage", "N° de l’attelage", "Gespann-Nr."),
        ("class", "Épreuve", "Prüfung"),
        ("width", "Largeur de la voiture", "Wagenbreite"),
        ("allowance", "Supplément", "Zuschlag"),
        ("gate", "Écartement des cônes", "Spurbreite"),
        ("signature", "Signature du juge", "Unterschrift Richter/in"),
        ("driven", "Temps effectif", "Gefahrene Zeit"),
        ("obstacle_faults", "Fautes d’obstacles", "Hindernisfehler"),
        ("allowed", "Temps accordé", "Erlaubte Zeit"),
        ("other", "Autres pénalités", "Andere Fehler"),
        ("over", "Dépassement du temps", "Zeitüberschreitung"),
        ("time_penalty", "× 0,5 = pénalités de temps", "× 0.5 = Zeitstrafe"),
        ("seconds", "s", "Sek."),
        ("penalties", "pénalités", "Strafpunkte"),
        ("penalty_seconds", "Secondes de pénalité", "Strafsekunden"),
        ("total", "Total", "Total"),
        ("total_rounds", "Total des manches", "Total der Umgänge"),
        ("description", "Description", "Beschreibung"),
        ("notes", "Notes", "Notizen"),
        ("total_points", "Total points de pénalité", "Total Strafpunkte"),
        ("total_seconds", "Total secondes de pénalité", "Total Strafsekunden"),
        ("clock_1", "1re montre", "1. Uhr"),
        ("clock_2", "2e montre", "2. Uhr"),
        ("course_time", "Temps du parcours (1/100 s)", "Gefahrene Zeit (1/100 s)"),
        ("neutralised", "Neutralisation sans faute du meneur", "Neutralisation ohne Verschulden des Fahrers"),
        ("rounded", "Temps arrondi à la seconde suivante", "Zeit auf die ganze Sekunde"),
        ("total_time", "Total temps", "Total Zeit"),
        ("gates_order", "Ordre de passage des portes", "Durchfahrtsreihenfolge der Tore"),
        ("obstacle", "Obstacle n°", "Hindernis-Nr."),
    ] {
        data["labels"][k] = json!(lang.pick(fr, de));
    }
    Ok(Print { template: Template::ControlSheets, data, name: title })
}

/// The marathon's penalties at an obstacle (Rosson's list, from the driving rules): French,
/// German, penalty points (empty: elimination).
const MARATHON_PENALTIES: [(&str, &str, &str); 20] = [
    ("Chute d’un élément tombant", "Fall eines abwerfbaren Elementes", "2"),
    ("La meneuse ou le meneur ou la ou le groom empêchent un élément renversable de tomber en le touchant (partie du corps)", "Verhindern durch Fahrer:in oder Beifahrer:in durch Berührung (Körperteil), dass ein abwerfbares Element herunterfällt", "10"),
    ("Fouet manquant sur la voiture dans chaque phase (y compris l’entrée et la sortie d’un obstacle sans fouet dans la main)", "Fehlende Peitsche auf dem Wagen in jeder Phase (inkl. Einfahren oder Verlassen eines Hindernisses ohne Peitsche in der Hand)", "5"),
    ("Déposer intentionnellement le fouet dans l’obstacle", "Absichtliches Ablegen der Peitsche im Hindernis", "5"),
    ("Traits, chaînettes ou porte-limonières manquants ou déchirés à la sortie et pas corrigé dans les 30 mètres après la sortie d’obstacle", "Fehlende oder zerrissene Zugstrangen, Aufhalter oder Landenträger, wenn nicht korrigiert innerhalb von 30 Metern nach dem Ausfahren aus einem Hindernis", "10"),
    ("Assistance d’une aide (fouet, guides) (l’aide a toutefois le droit de tenir les guides lorsque l’attelage est à l’arrêt)", "Mithilfe der Beifahrerin oder des Beifahrers (Peitsche, Leine) (Beifahrer:innen dürfen die Leinen halten, wenn sich das Gespann im Zwangshalt befindet)", "20"),
    ("Le fait de ne pas passer les fanions rouges et blancs à l’entrée d’un obstacle", "Nichtdurchfahren der roten und weissen Flaggen bei der Einfahrt eines Hindernisses", ""),
    ("Le fait de ne pas passer les fanions rouges et blancs à la sortie d’un obstacle", "Nichtdurchfahren der roten und weissen Flaggen bei der Ausfahrt eines Hindernisses", ""),
    ("Passer une porte dans le mauvais sens ou dans le mauvais ordre (y compris l’omission d’une porte) sans corriger l’erreur", "Durchfahren der Tore in der falschen Reihenfolge oder Richtung (inkl. Auslassen eines Tores) ohne Korrektur", ""),
    ("Passer une porte dans le mauvais sens ou dans le mauvais ordre et corriger l’erreur avant de quitter l’obstacle", "Durchfahren der Tore in der falschen Reihenfolge oder Richtung mit Korrektur vor Verlassen des Hindernisses", "20"),
    ("Passer les fanions rouges et blancs à la sortie avant que toutes les portes d’un obstacle soient franchies correctement", "Durchfahren der roten und weissen Flaggen bei der Ausfahrt, bevor alle Tore eines Hindernisses korrekt gefahren sind", ""),
    ("Mise des pieds à terre (les deux pieds) d’un ou des deux aides dans l’obstacle, par événement", "Absteigen (beide Füsse am Boden) eines oder beider Beifahrer:innen im Hindernis, pro Vorkommnis", "5"),
    ("Mise des pieds à terre (les deux pieds) de la meneuse ou du meneur dans l’obstacle", "Absteigen der Fahrerin oder des Fahrers im Hindernis (beide Füsse am Boden)", "20"),
    ("Renversement du véhicule dans une phase ou un obstacle", "Kippen des Wagens in einer Phase oder in einem Hindernis", ""),
    ("Dételer et mener un cheval séparément au travers de toute partie de l’obstacle", "Ausspannen und durch ein Hindernis Führen eines Pferdes", ""),
    ("Dépassement du temps limite de cinq minutes par obstacle", "Überschreiten der Höchstzeit von fünf Min. im Hindernis", ""),
    ("Aide de complaisance", "Fremde Hilfe", ""),
    ("Le fait de ne pas s’arrêter immédiatement et de faire les corrections nécessaires lorsqu’un cheval passe par-dessus un trait", "Nicht sofort anhalten und korrigieren, wenn ein Pferd über die Stange tritt", "20"),
    ("Le fait de ne pas s’arrêter immédiatement lorsqu’un cheval passe par-dessus le timon, le maître palonnier, la limonière et/ou les traits d’un cheval de volée, etc.", "Nicht sofort anhalten und korrigieren, wenn ein Pferd über die Lande, Deichsel, Vorwaage, Strange eines Vorpferdes etc. tritt", ""),
    ("Une aide grimpe par-dessus les chevaux et/ou le timon", "Beifahrer:in klettert über Pferde und/oder Deichsel", "20"),
];

/// The derby's penalties at an obstacle (Rosson's list, from the driving rules): French,
/// German, penalty seconds (empty: elimination; "dq": the horse disqualified, the driver
/// eliminated).
const DERBY_PENALTIES: [(&str, &str, &str); 17] = [
    ("Chute d’une balle ou d’un élément tombant", "Abwerfen eines Balles oder eines fallenden Elementes", "10"),
    ("La meneuse ou le meneur ou un·e aide empêche un élément renversable de tomber en le touchant (partie du corps)", "Verhindern durch Fahrer:in oder Beifahrer:in, dass ein abwerfbares Element herunterfällt (durch Berührung mit einem Körperteil)", "10"),
    ("Renverser n’importe quel élément d’un obstacle pas encore franchi selon l’épreuve partielle C et que cet obstacle doive être reconstruit (le chronomètre est arrêté)", "Umwerfen eines beliebigen Teils eines noch nicht passierten Hindernisses gemäss Teilprüfung C, sodass dieses wieder aufgebaut werden muss (Zeitmessung wird angehalten)", "10 + 10"),
    ("Renverser ou déplacer un obstacle ou une partie d’un obstacle multiple selon l’épreuve partielle C, lors d’un passage incorrect et que cet obstacle doive être reconstruit (le chronomètre est arrêté)", "Umwerfen oder Verschieben eines Hindernisses oder eines Teils eines Mehrfachhindernisses gemäss Teilprüfung C bei nicht korrekter Durchfahrt, sodass dieses Hindernis wieder aufgebaut werden muss (Zeitmessung wird angehalten)", "10 + 10"),
    ("Erreur corrigée dans un obstacle selon l’épreuve partielle B", "Korrigiertes Verfahren in einem Hindernis gemäss Teilprüfung B", "40"),
    ("Ne pas suivre l’ordre de franchissement des obstacles, erreur non corrigée dans un obstacle selon l’épreuve partielle B", "Falsche Reihenfolge der Hindernisse oder nicht korrigiertes Verfahren in einem Hindernis gemäss Teilprüfung B", ""),
    ("Repasser un obstacle selon l’épreuve partielle C", "Erneutes Durchfahren eines Hindernisses gemäss Teilprüfung C", ""),
    ("Ne pas franchir un passage obligatoire", "Nichtpassieren eines Kontrolltores", ""),
    ("1re mise pied à terre d’une aide ou de la meneuse ou du meneur (le chronomètre n’est pas arrêté)", "1. Absteigen von Beifahrer:in oder Fahrer:in (Zeitmessung wird nicht angehalten)", "10"),
    ("2e mise pied à terre d’une aide ou de la meneuse ou du meneur (le chronomètre n’est pas arrêté)", "2. Absteigen von Beifahrer:in oder Fahrer:in (Zeitmessung wird nicht angehalten)", "20"),
    ("3e mise pied à terre d’une aide ou de la meneuse ou du meneur", "3. Absteigen von Beifahrer:in oder Fahrer:in", ""),
    ("Renversement du véhicule", "Umkippen des Wagens", ""),
    ("Si la ou le concurrent·e dépose ou perd le fouet durant l’épreuve, pour chaque événement", "Ablegen oder Verlieren der Peitsche während der Prüfung, pro Ereignis", "5"),
    ("Chute d’un cheval (épaule et hanche touchant le sol ou l’obstacle)", "Sturz eines Pferdes (Schulter und Hüfte auf Boden oder Hindernis)", "dq"),
    ("Utilisation du frein et/ou des guides et/ou du fouet par une aide", "Bedienen von Bremse und/oder von Leinen und/oder Peitsche durch den Beifahrer oder die Beifahrerin", ""),
    ("Aide de complaisance", "Fremde Hilfe durch Dritte", ""),
    ("Dételer et mener un cheval séparément au travers de tout ou partie de l’obstacle", "Ausspannen und Führen eines Pferdes durch ein ganzes oder ein Teil eines Hindernisses", ""),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_in_both_languages() {
        assert_eq!(long_date("2026-09-25", Language::Fr, false), "vendredi 25 septembre");
        assert_eq!(long_date("2026-09-27", Language::De, true), "Sonntag, 27. September 2026");
        assert_eq!(long_date("2024-02-29", Language::Fr, true), "jeudi 29 février 2024");
    }
}
