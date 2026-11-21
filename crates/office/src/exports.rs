//! Excel files of the show (Rosson's "Export"): the participants with their contact details,
//! every entry, a class's start list, a class's ranking. One sheet each, a header row, values as
//! numbers where they are numbers.

use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::{Format, Workbook};
use show::model::*;
use show::State;

use crate::prints::Language;
use crate::views::{self, StartLine};

/// A cell.
pub enum Cell {
    Text(String),
    Number(f64),
    Empty,
}

impl From<String> for Cell {
    fn from(s: String) -> Cell {
        if s.is_empty() {
            Cell::Empty
        } else {
            Cell::Text(s)
        }
    }
}

impl From<&str> for Cell {
    fn from(s: &str) -> Cell {
        Cell::from(s.to_string())
    }
}

fn num<T: Into<f64>>(n: Option<T>) -> Cell {
    n.map_or(Cell::Empty, |n| Cell::Number(n.into()))
}

fn dec(d: Option<rust_decimal::Decimal>) -> Cell {
    d.and_then(|d| d.to_f64()).map_or(Cell::Empty, Cell::Number)
}

/// Column headings in the office's language (French and German side by side, one for one).
fn both(lang: Language, fr: &[&'static str], de: &[&'static str]) -> Vec<&'static str> {
    fr.iter().zip(de).map(|(f, d)| lang.pick(f, d)).collect()
}


/// One sheet: a bold header row, then the rows; the header stays in view.
pub fn workbook(sheet: &str, headers: &[&str], rows: Vec<Vec<Cell>>) -> Result<Vec<u8>, String> {
    let err = |e: rust_xlsxwriter::XlsxError| e.to_string();
    let mut book = Workbook::new();
    let ws = book.add_worksheet();
    let name: String = sheet.chars().filter(|c| !"[]:*?/\\".contains(*c)).take(31).collect();
    ws.set_name(&name).map_err(err)?;
    let bold = Format::new().set_bold();
    for (c, h) in headers.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &bold).map_err(err)?;
    }
    for (r, row) in rows.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            let (r, c) = (r as u32 + 1, c as u16);
            match cell {
                Cell::Text(s) => {
                    ws.write_string(r, c, s).map_err(err)?;
                }
                Cell::Number(n) => {
                    ws.write_number(r, c, *n).map_err(err)?;
                }
                Cell::Empty => {}
            }
        }
    }
    ws.set_freeze_panes(1, 0).map_err(err)?;
    ws.autofit();
    book.save_to_buffer().map_err(err)
}

fn horse_name(state: &State, e: &Entry) -> String {
    e.guest_horse.clone().or_else(|| e.horses_on_entry.first().and_then(|h| h["pferdename"].as_str()).map(str::to_string)).or_else(|| e.horse.as_ref().and_then(|h| state.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default()
}

/// The riders with their contact details (private data), by name.
pub fn participants(state: &State, lang: Language) -> Result<Vec<u8>, String> {
    let headers: &[&str] = &both(lang, &["Licence", "Titre", "Prénom", "Nom", "Rue", "NPA", "Lieu", "Pays", "E-mail", "Téléphone", "Mobile", "Langue", "Société", "Licences"], &["Lizenz", "Anrede", "Vorname", "Name", "Strasse", "PLZ", "Ort", "Land", "E-Mail", "Telefon", "Mobile", "Sprache", "Verein", "Lizenzen"]);
    let riders: std::collections::BTreeSet<&PersonId> = state.entries.values().map(|e| &e.rider).collect();
    let mut people: Vec<&Person> = riders.into_iter().filter_map(|r| state.persons.get(r)).collect();
    people.sort_by(|a, b| (&a.last_name, &a.first_name).cmp(&(&b.last_name, &b.first_name)));
    let rows = people
        .into_iter()
        .map(|p| {
            let c = state.contacts.get(&p.id).cloned().unwrap_or_default();
            vec![
                Cell::Number(p.id.0 as f64),
                c.salutation.into(),
                p.first_name.as_str().into(),
                p.last_name.as_str().into(),
                c.street.into(),
                c.postcode.into(),
                p.town.as_str().into(),
                c.country.into(),
                c.email.into(),
                c.phone_private.into(),
                c.phone_mobile.into(),
                c.language.into(),
                p.club.as_str().into(),
                p.licences.as_str().into(),
            ]
        })
        .collect();
    workbook(lang.pick("Participants", "Teilnehmer"), headers, rows)
}

/// Every entry: class, rider, horse, what it is, where it starts.
pub fn entries(state: &State, lang: Language) -> Result<Vec<u8>, String> {
    let headers: &[&str] = &both(lang, &["Épreuve", "Date", "Licence", "Cavalier", "Lieu", "Société", "N° cheval", "Cheval", "Statut", "NP", "HC", "Niveau", "Équipe", "Départs (épreuve, n°, ordre)", "Finance payée", "Place org.", "Changement de cavalier", "Note du secrétariat"], &["Prüfung", "Datum", "Lizenz", "Reiter", "Ort", "Verein", "Pferde-Nr.", "Pferd", "Status", "NS", "HC", "Stufe", "Equipe", "Starts (Prüfung, Nr., Reihenfolge)", "Nenngeld bezahlt", "Org.-Platz", "Reiterwechsel", "Notiz des Sekretariats"]);
    let mut rows: Vec<(String, String, Vec<Cell>)> = Vec::new();
    for e in state.entries.values() {
        let p = state.published_classes.get(&e.published);
        let person = state.persons.get(&e.rider);
        let starts: Vec<String> = state
            .starts
            .iter()
            .filter_map(|(c, starts)| starts.iter().find(|s| s.entry == e.id).map(|s| (c, s)))
            .map(|(c, s)| format!("{} #{} ({})", state.classes.get(c).map_or_else(|| c.to_string(), |c| c.number.clone()), s.start_number.map(|n| n.to_string()).unwrap_or_else(|| "–".into()), s.order))
            .collect();
        let yes = |b: bool| Cell::from(if b { "x" } else { "" });
        let number = p.map(|p| p.number.clone()).unwrap_or_default();
        let name = person.map(|p| format!("{} {}", p.last_name, p.first_name)).unwrap_or_default();
        rows.push((
            format!("{number:0>6}"),
            name.to_lowercase(),
            vec![
                number.as_str().into(),
                p.map(|p| p.date.clone()).unwrap_or_default().into(),
                Cell::Number(e.rider.0 as f64),
                person.map(|p| format!("{} {}", p.first_name, p.last_name)).unwrap_or_default().into(),
                person.map(|p| p.town.clone()).unwrap_or_default().into(),
                person.map(|p| p.club.clone()).unwrap_or_default().into(),
                num(e.horse.as_ref().map(|h| h.0 as f64)),
                horse_name(state, e).into(),
                e.se_status.as_str().into(),
                yes(e.status == EntryStatus::NotStarting),
                yes(e.hors_concours),
                num(e.stage.map(|s| s as f64)),
                e.team.as_ref().and_then(|t| state.teams.get(t)).map(|t| t.name.clone()).unwrap_or_default().into(),
                starts.join(" · ").into(),
                dec(e.fee_paid),
                yes(e.organiser_place),
                yes(e.rider_changed),
                e.comment.as_str().into(),
            ],
        ));
    }
    rows.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    workbook(lang.pick("Inscriptions", "Nennungen"), headers, rows.into_iter().map(|(_, _, r)| r).collect())
}

/// A class's start list as published (as printed: by name before start numbers, no place).
pub fn start_list(state: &State, class: &ClassId, lang: Language) -> Result<Vec<u8>, String> {
    let list = views::published_start_list(state, class)?;
    let headers: &[&str] = &both(lang, &["Heure", "N°", "Cavalier", "Lieu", "Société", "Cheval", "NP", "HC"], &["Zeit", "Nr.", "Reiter", "Ort", "Verein", "Pferd", "NS", "HC"]);
    let rows = list
        .lines
        .iter()
        .filter_map(|l| match l {
            StartLine::Starter { time, start_number, rider, town, club, horse, not_starting, hors_concours, .. } => Some(vec![
                time.clone().unwrap_or_default().into(),
                num(start_number.map(|n| n as f64)),
                rider.as_str().into(),
                town.as_str().into(),
                club.as_str().into(),
                horse.as_str().into(),
                Cell::from(if *not_starting { "x" } else { "" }),
                Cell::from(if *hors_concours { "x" } else { "" }),
            ]),
            _ => None,
        })
        .collect();
    workbook(&format!("{} {}", lang.pick("Départ", "Start"), list.class.number), headers, rows)
}

/// A class's ranking as it stands.
pub fn ranking(state: &State, class: &ClassId, lang: Language) -> Result<Vec<u8>, String> {
    let r = views::ranking(state, class)?;
    let headers: &[&str] = &both(lang, &["Rang", "Classé", "Cavalier", "Société", "Cheval", "Pénalités", "Temps", "%"], &["Rang", "Klassiert", "Reiter", "Verein", "Pferd", "Strafpunkte", "Zeit", "%"]);
    let rows = r
        .lines
        .iter()
        .map(|l| vec![num(l.rank.map(|n| n as f64)), Cell::from(if l.placed { "x" } else { "" }), l.rider.as_str().into(), l.club.as_str().into(), l.horse.as_str().into(), dec(l.faults), dec(l.time), dec(l.percentage)])
        .collect();
    workbook(&format!("{} {}", lang.pick("Classement", "Rangliste"), r.class.number), headers, rows)
}
