//! Files exchanged with the timekeepers in show jumping, and with the judges' tablets in
//! dressage ([`tablet`]).
//!
//! The format is an external contract, like the Swiss Equestrian API: timekeeping software reads the start
//! list and sends the results back in this layout. It is kept exactly as Rosson 3.3.15 writes
//! and reads it; `tests/rosson_contract.rs` checks that against files produced by Rosson itself
//! (`golden/corpus/<show>/timing/`).
//!
//! # Start list, to the timekeepers
//!
//! One row per starter in start order (riders who do not start are left out), 25 columns
//! ([`COLUMNS`]): start number, horse, rider, class, licence and passport numbers, signalement,
//! empty result columns, `hc`, start order. Team classes list all teams' first riders in team
//! order, then the second riders, … and number the start order 1, 2, 3, …
//! As text: tab-separated, Windows-1252, lines ending in LF, no line end after the last row. As
//! Excel: one sheet named `Prüfung_<class number>`.
//!
//! # Results, from the timekeepers
//!
//! Excel (.xlsx, .xls) or text separated by tabs, commas or semicolons; columns are found by name,
//! the first two must be `Start-Nr` and `Name Pferd`. Per row:
//!
//! - **Aufgabe** says how the rider finished: `OK`; `DISQUAL`, `DISQUAL.` or `DNF` eliminated; `RET`
//!   retired; `DNS` withdrawn; `DSQ` other (the discipline's first reason if it has no "other").
//!   Rows with an empty Aufgabe are not read. *Better than Rosson:* an unknown code is reported
//!   instead of silently skipping the row.
//! - The rider is found by **licence** (`Lizenz Nr`) and **horse passport** (`Pass Nr Pferd`) among
//!   the entries started in the class; Rosson's placeholder horse 5 also needs the horse name.
//!   *Better than Rosson:* when the same rider and horse start more than once in the class (relay
//!   teams), the start number decides; Rosson took the first match and could put the results on
//!   the wrong entry.
//! - The fields of the class's scoring table are taken from their columns. In two-phase classes
//!   phase 2 may come in the jump-off columns (`Strafpunkte 1 St`, `Zeit 1 St`), as timekeeping
//!   software writes it. *Better than Rosson:* two-phase classes are recognised by the scoring
//!   table, not by the word "Phase" in its name.
//! - **hc** and **Nichtstarter** are reported when they disagree with the entry, not applied:
//!   changing an entry is the show office's decision. (Rosson changed the entry.)
//! - **Rang** is only read when the office chooses to take the timekeeper's ranks over
//!   ([`Options::adopt_ranks`], Rosson's "Rang übernehmen"): each rank becomes a rank set by hand
//!   (docs/deviations.md S18), the others' ranks skip it. A rider without rank in the file loses
//!   a rank taken over from an earlier file, but not one the office set itself. *Better than
//!   Rosson:* a rank that is not a number is reported (Rosson read "2nd" as 2 and "-" as nothing).

pub mod tablet;

use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use rust_decimal::Decimal;
use serde_json::Value;
use show::model::*;
use show::oplog::Change;
use show::State;

/// The columns, in order, as Rosson writes them.
pub const COLUMNS: [&str; 25] = [
    "Start-Nr", "Name Pferd", "Reiter", "Pr Bezeichnung", "PSerie", "Rang", "Lizenz Nr", "Pass Nr Pferd", "Signalement",
    "Strafpunkte NP", "Zeit NP", "Aufgabe", "Strafpunkte NP 2 Umg", "Zeit NP 2 Umg", "Strafpunkte 1 St", "Zeit 1 St",
    "Strafpunkte 2 St", "Zeit 2 St", "Strafpunkte 3 St", "Zeit 3 St", "Strafpunkte 4 St", "Zeit 4 St", "hc", "Nichtstarter",
    "Reihenfolge-Nr",
];

/// Scoring-table field → column of the results file.
pub const FIELD_COLUMNS: [(&str, &str); 12] = [
    ("strafpunkte_initial_1", "Strafpunkte NP"),
    ("zeit_initial_1", "Zeit NP"),
    ("strafpunkte_initial_2", "Strafpunkte NP 2 Umg"),
    ("zeit_initial_2", "Zeit NP 2 Umg"),
    ("strafpunkte_stechen_1", "Strafpunkte 1 St"),
    ("zeit_stechen_1", "Zeit 1 St"),
    ("strafpunkte_stechen_2", "Strafpunkte 2 St"),
    ("zeit_stechen_2", "Zeit 2 St"),
    ("strafpunkte_stechen_3", "Strafpunkte 3 St"),
    ("zeit_stechen_3", "Zeit 3 St"),
    ("strafpunkte_stechen_4", "Strafpunkte 4 St"),
    ("zeit_stechen_4", "Zeit 4 St"),
];

/// Codes in the Aufgabe column → the Swiss Equestrian reason label they stand for.
const REASON_CODES: [(&str, &str); 6] = [
    ("DISQUAL.", "ausgeschieden"),
    ("DISQUAL", "ausgeschieden"),
    ("DNF", "ausgeschieden"),
    ("RET", "aufgegeben"),
    ("DNS", "verzichtet"),
    ("DSQ", "andere"),
];

/// Rosson's placeholder horse for horses without Swiss Equestrian id.
const PLACEHOLDER_HORSE: u64 = 5;

/// One cell of the start list.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Text(String),
    Number(u64),
    Empty,
}

impl Cell {
    fn text(&self) -> String {
        match self {
            Cell::Text(s) => s.clone(),
            Cell::Number(n) => n.to_string(),
            Cell::Empty => String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Error(pub String);

// ─── Start list ──────────────────────────────────────────────────────────────

/// The start list of a class, one row of [`COLUMNS`] per starter.
pub fn start_list(state: &State, class_id: &ClassId) -> Result<Vec<Vec<Cell>>, Error> {
    let class = state.classes.get(class_id).ok_or_else(|| Error(format!("unknown class {class_id}")))?;
    let team_class = state.published_classes.get(&class.published).is_some_and(|p| p.team_size.is_some());
    let starts = state.starts.get(class_id).map(Vec::as_slice).unwrap_or(&[]);
    let mut starts: Vec<&Start> = starts.iter().collect();
    if team_class {
        // Rosson's getGesamtreihenfolge; teams without a place in the rotation start after the others.
        starts.sort_by_key(|s| (s.order, s.team_order.unwrap_or(u32::MAX)));
    }
    let mut rows = Vec::new();
    for start in starts.into_iter().filter(|s| state.entries.get(&s.entry).is_some_and(|e| e.status == EntryStatus::Entered)) {
        let entry = &state.entries[&start.entry];
        let horse = entry.horse.as_ref().and_then(|h| state.horses.get(h));
        let rider = state.persons.get(&entry.rider);
        let mut row = vec![
            start.start_number.map_or(Cell::Empty, |n| Cell::Number(n as u64)),
            Cell::Text(entry_horse_name(state, entry)),
            Cell::Text(rider.map(|p| format!("{} {}", p.first_name, p.last_name)).unwrap_or_default()),
            Cell::Text(class.number.clone()),
            Cell::Empty,
            Cell::Text(String::new()),
            Cell::Number(entry.rider.0),
            entry.horse.as_ref().map_or(Cell::Empty, |h| Cell::Number(h.0)),
            Cell::Text(horse.map(|h| h.signalement.clone()).unwrap_or_default()),
        ];
        row.extend((0..13).map(|_| Cell::Text(String::new())));
        row.push(Cell::Text(if entry.hors_concours { "hc".into() } else { String::new() }));
        row.push(Cell::Text(String::new()));
        row.push(Cell::Number(start.order as u64));
        rows.push(row);
    }
    if team_class {
        for (i, row) in rows.iter_mut().enumerate() {
            row[24] = Cell::Number(i as u64 + 1);
        }
    }
    Ok(rows)
}

/// The start list as text: tab-separated, Windows-1252.
pub fn to_text(rows: &[Vec<Cell>]) -> Vec<u8> {
    let mut lines = vec![COLUMNS.join("\t")];
    lines.extend(rows.iter().map(|r| r.iter().map(Cell::text).collect::<Vec<_>>().join("\t")));
    let text = lines.join("\n");
    let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(&text);
    bytes.into_owned()
}

/// The start list as an Excel file.
pub fn to_xlsx(rows: &[Vec<Cell>], class_number: &str) -> Result<Vec<u8>, Error> {
    xlsx(&COLUMNS, rows, class_number)
}

/// An Excel file with one sheet, `Prüfung_<class number>`, as Rosson names it.
pub(crate) fn xlsx(columns: &[&str], rows: &[Vec<Cell>], class_number: &str) -> Result<Vec<u8>, Error> {
    let mut book = rust_xlsxwriter::Workbook::new();
    let sheet = book.add_worksheet();
    let name: String = format!("Prüfung_{class_number}").chars().filter(|c| !"[]:*?/\\".contains(*c)).take(31).collect();
    sheet.set_name(&name).map_err(|e| Error(e.to_string()))?;
    let err = |e: rust_xlsxwriter::XlsxError| Error(e.to_string());
    for (col, head) in columns.iter().enumerate() {
        sheet.write_string(0, col as u16, *head).map_err(err)?;
    }
    for (r, row) in rows.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            match cell {
                Cell::Text(s) if !s.is_empty() => {
                    sheet.write_string(r as u32 + 1, c as u16, s).map_err(err)?;
                }
                Cell::Number(n) => {
                    sheet.write_number(r as u32 + 1, c as u16, *n as f64).map_err(err)?;
                }
                _ => {}
            }
        }
    }
    book.save_to_buffer().map_err(err)
}

// ─── Results ─────────────────────────────────────────────────────────────────

/// One row of a results file: cell text by column name (empty cells left out).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    /// Line in the file (the header is line 1), for notes to the office.
    pub line: usize,
    pub cells: HashMap<String, String>,
}

impl Row {
    fn get(&self, column: &str) -> &str {
        self.cells.get(column).map(String::as_str).unwrap_or("")
    }
}

/// Read a results file, Excel or text.
pub fn read_results(bytes: &[u8]) -> Result<Vec<Row>, Error> {
    read_table(bytes, ["Start-Nr", "Name Pferd"])
}

/// Read a table file (Excel, or text separated by tabs, commas or semicolons) whose first two
/// columns are `first`. Excel sheets with other columns are skipped.
pub(crate) fn read_table(bytes: &[u8], first: [&str; 2]) -> Result<Vec<Row>, Error> {
    let tables = if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) { read_excel(bytes)? } else { vec![read_text(bytes)?] };
    let wanted: Vec<&Table> = tables.iter().filter(|(header, _)| header.len() >= 2 && header[0] == first[0] && header[1] == first[1]).collect();
    if wanted.is_empty() {
        return Err(Error(format!("not the expected file: the first columns must be {} and {}", first[0], first[1])));
    }
    Ok(wanted
        .into_iter()
        .flat_map(|(header, lines)| {
            lines.iter().filter(|(_, cells)| cells.iter().any(|c| !c.is_empty())).map(move |(line, cells)| Row {
                line: *line,
                cells: header.iter().zip(cells).filter(|(_, v)| !v.is_empty()).map(|(h, v)| (h.clone(), v.clone())).collect(),
            })
        })
        .collect())
}

/// A sheet or text file: header, and the lines below it with their line numbers.
type Table = (Vec<String>, Vec<(usize, Vec<String>)>);

/// Every sheet, as text.
fn read_excel(bytes: &[u8]) -> Result<Vec<Table>, Error> {
    let mut book = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec())).map_err(|e| Error(e.to_string()))?;
    let mut tables = Vec::new();
    for name in book.sheet_names() {
        let range = book.worksheet_range(&name).map_err(|e| Error(e.to_string()))?;
        let first_row = range.start().map_or(0, |(row, _)| row as usize);
        let mut lines = range.rows().enumerate();
        let Some((_, header)) = lines.next() else { continue };
        tables.push((header.iter().map(data_text).collect(), lines.map(|(i, l)| (first_row + i + 1, l.iter().map(data_text).collect())).collect()));
    }
    Ok(tables)
}

fn data_text(d: &Data) -> String {
    match d {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string().to_uppercase(),
        other => other.to_string(),
    }
}

/// Text: UTF-8 or Windows-1252, any line ends (CRLF, LF, CR), separated by tabs, commas or
/// semicolons (whichever the header uses).
fn read_text(bytes: &[u8]) -> Result<Table, Error> {
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s.trim_start_matches('\u{feff}').to_string(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    };
    // A line ends with CRLF, LF or CR.
    let mut lines = text.split("\r\n").flat_map(|l| l.split(['\r', '\n'])).enumerate().map(|(i, l)| (i + 1, l)).filter(|(_, l)| !l.is_empty());
    let (_, header) = lines.next().ok_or_else(|| Error("empty file".into()))?;
    let separator = if header.contains('\t') { '\t' } else if header.contains(',') { ',' } else { ';' };
    let split = |l: &str| l.split(separator).map(str::to_string).collect::<Vec<_>>();
    Ok((split(header), lines.map(|(i, l)| (i, split(l))).collect()))
}

/// What importing a results file does: the score cards to enter, and what to tell the office.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Import {
    pub changes: Vec<Change>,
    pub notes: Vec<String>,
}

/// How a results file is read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// Take the timekeeper's ranks (`Rang` column) over as ranks set by hand (S18). Off by
    /// default: ranks are computed from the results.
    pub adopt_ranks: bool,
}

/// The reason recorded on ranks taken over from a timekeeper's file.
pub const TIMEKEEPER_RANK: &str = "rank from the timekeeper's file";

/// Turn the rows of a results file into score cards for a class.
pub fn import(state: &State, class_id: &ClassId, rows: &[Row]) -> Result<Import, Error> {
    import_with(state, class_id, rows, Options::default())
}

/// [`import`], with options.
pub fn import_with(state: &State, class_id: &ClassId, rows: &[Row], options: Options) -> Result<Import, Error> {
    let class = state.classes.get(class_id).ok_or_else(|| Error(format!("unknown class {class_id}")))?;
    let discipline = state.published_classes.get(&class.published).map(|p| p.discipline.clone()).unwrap_or_default();
    let def = class.scoring_table.and_then(|t| state.scoring_tables.get(&t)).ok_or_else(|| Error("the class has no scoring table".into()))?;
    let definition: Value = scoring::jumping::table::parse_definition(scoring::jumping::table::definition_text(&def.code, &def.definition)).map_err(|e| Error(format!("scoring table {}: {e}", def.code)))?;
    let visible: Vec<String> = definition["sichtbare_felder"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(str::to_string)).collect();
    let two_phase = scoring::jumping::table::ScoringTable::from_definition(def.id, &def.code, &def.name, &def.definition)
        .is_ok_and(|t| t.phases != scoring::jumping::table::Phases::None);

    let reasons = reasons(state, &discipline);
    let started = started(state, class_id);

    let mut result = Import::default();
    for row in rows {
        let line = row.line;
        let out = match status(row.get("Aufgabe"), &reasons)? {
            Status::Empty => {
                note_entry_flags(&mut result, row, line, None, &started);
                continue;
            }
            Status::Finished => None,
            Status::Out(code) => Some(code),
            Status::Unknown(code) => {
                result.notes.push(format!("line {line}: unknown code {code:?} in Aufgabe, row not read"));
                continue;
            }
        };
        let entry = match find_entry(state, class, &started, row.get("Lizenz Nr"), row.get("Pass Nr Pferd"), row.get("Name Pferd"), row.get("Start-Nr")) {
            Ok(e) => e,
            Err(note) => {
                result.notes.push(format!("line {line}: {note}"));
                continue;
            }
        };
        note_entry_flags(&mut result, row, line, Some(entry), &started);

        let mut values = BTreeMap::new();
        for field in &visible {
            let Some((_, column)) = FIELD_COLUMNS.iter().find(|(f, _)| f == field) else { continue };
            let mut text = row.get(column);
            if two_phase && field.ends_with("initial_2") && text.is_empty() {
                // Timekeeping software writes phase 2 in the jump-off columns.
                let alternative = FIELD_COLUMNS.iter().find(|(f, _)| *f == field.replace("initial_2", "stechen_1")).map(|(_, c)| *c).unwrap_or("");
                text = row.get(alternative);
            }
            if two_phase && field.contains("stechen") {
                continue;
            }
            match scoring::number::parse(text) {
                Ok(Some(v)) => {
                    values.insert(field.clone(), v);
                }
                Ok(None) => {}
                Err(e) => result.notes.push(format!("line {line}: {column}: {e}")),
            }
        }
        // Wertung C: a round with a time and no faults field — the time includes the penalty
        // seconds. Its faults column is not read, as Rosson does; filled in, it is said, in case
        // the timekeeper wrote the faults apart from the time.
        for time_field in visible.iter().filter(|f| f.starts_with("zeit_") && !two_phase) {
            let faults_field = time_field.replacen("zeit_", "strafpunkte_", 1);
            if visible.contains(&faults_field) {
                continue;
            }
            let Some((_, column)) = FIELD_COLUMNS.iter().find(|(f, _)| *f == faults_field) else { continue };
            if matches!(scoring::number::parse(row.get(column)), Ok(Some(v)) if !v.is_zero()) {
                result.notes.push(format!("line {line}: {column} {}: not read — in a Wertung C round the time includes the penalty seconds", row.get(column)));
            }
        }
        result.changes.push(Change::ResultEntered { class: class_id.clone(), entry: entry.id.clone(), values, out });
        if options.adopt_ranks {
            adopt_rank(state, class_id, entry, row, &mut result);
        }
    }
    Ok(result)
}

/// The `Rang` cell as a rank set by hand.
fn adopt_rank(state: &State, class: &ClassId, entry: &Entry, row: &Row, result: &mut Import) {
    let competitor = Competitor::Entry(entry.id.clone());
    let current = state.manual_ranks.get(&(class.clone(), competitor.clone()));
    let text = row.get("Rang").trim();
    let rank = match text.trim_end_matches('.') {
        "" => None,
        n => match n.parse::<u32>() {
            Ok(r) if r > 0 => Some(r),
            _ => {
                result.notes.push(format!("line {}: Rang {text:?} is not a rank, not taken over", row.line));
                return;
            }
        },
    };
    let change = |rank: Option<u32>| Change::RankSet { class: class.clone(), competitor: competitor.clone(), rank, reason: TIMEKEEPER_RANK.into() };
    match (rank, current) {
        (Some(r), Some(m)) if m.rank == r && m.reason == TIMEKEEPER_RANK => {}
        (Some(r), _) => result.changes.push(change(Some(r))),
        // No rank in the file: a rank from an earlier file goes, one set by the office stays.
        (None, Some(m)) if m.reason == TIMEKEEPER_RANK => result.changes.push(change(None)),
        (None, _) => {}
    }
}

// ─── Shared by both imports ──────────────────────────────────────────────────

/// The discipline's reasons for being out (or the general ones), as Rosson's getAufgabegruende.
pub(crate) fn reasons<'a>(state: &'a State, discipline: &str) -> Vec<&'a ReasonDef> {
    let mut reasons: Vec<&ReasonDef> = state.reasons.values().filter(|r| r.discipline.as_deref() == Some(discipline)).collect();
    if reasons.is_empty() {
        reasons = state.reasons.values().filter(|r| r.discipline.is_none()).collect();
    }
    reasons
}

/// The entries in a class's start list.
pub(crate) fn started<'a>(state: &'a State, class: &ClassId) -> Vec<&'a Entry> {
    state.starts.get(class).into_iter().flatten().filter_map(|s| state.entries.get(&s.entry)).collect()
}

/// What a status cell (Aufgabe, status) says.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Status {
    Empty,
    Finished,
    /// Out, with the Swiss Equestrian reason code.
    Out(u32),
    Unknown(String),
}

/// `OK`; `DISQUAL`, `DISQUAL.` or `DNF` eliminated; `RET` retired; `DNS` withdrawn; `DSQ` other —
/// mapped to the discipline's reason with that label (its first reason if it has none).
pub(crate) fn status(text: &str, reasons: &[&ReasonDef]) -> Result<Status, Error> {
    let code = text.trim().to_uppercase();
    Ok(match code.as_str() {
        "" => Status::Empty,
        "OK" => Status::Finished,
        code => match REASON_CODES.iter().find(|(c, _)| *c == code) {
            Some((_, label)) => Status::Out(reasons.iter().find(|r| r.name == *label).or(reasons.first()).map(|r| r.id).ok_or_else(|| Error("no reasons for the discipline".into()))?),
            None => Status::Unknown(code.to_string()),
        },
    })
}

/// The started entry a row is about: by licence and horse passport (Rosson's placeholder horse 5
/// also by horse name, whatever its case); when that pair starts more than once in the class, by start number.
/// `Err`: the note for the office.
pub(crate) fn find_entry<'a>(state: &State, class: &Class, started: &[&'a Entry], licence: &str, passport: &str, horse_name: &str, start_no: &str) -> Result<&'a Entry, String> {
    let licence_id = licence.trim().parse::<u64>().ok();
    let passport_id = passport.trim().parse::<u64>().ok();
    let candidates: Vec<&'a Entry> = started
        .iter()
        .copied()
        .filter(|e| {
            Some(e.rider.0) == licence_id
                && match passport_id {
                    Some(PLACEHOLDER_HORSE) => e.horse == Some(HorseId(PLACEHOLDER_HORSE)) && entry_horse_name(state, e).to_uppercase() == horse_name.to_uppercase(),
                    Some(p) => e.horse == Some(HorseId(p)),
                    None => e.horse.is_none() && e.guest_horse.as_deref().is_some_and(|g| guest_horse_name(g) == guest_horse_name(horse_name)),
                }
        })
        .collect();
    let entry = match candidates.len() {
        0 => return Err(format!("no starter in class {} with licence {licence} and horse {passport}", class.number)),
        1 => candidates[0],
        n => {
            let number = start_no.trim().parse::<u32>().ok();
            let by_number: Vec<&'a Entry> = candidates.into_iter().filter(|e| number.is_some() && start_number(state, &class.id, e) == number).collect();
            if by_number.len() != 1 {
                return Err(format!("rider {licence} with horse {passport} starts {n} times in class {}, and the start number does not tell which — row not read", class.number));
            }
            by_number[0]
        }
    };
    if entry.status != EntryStatus::Entered {
        return Err(format!("entry {} does not start (withdrawn from the entry list), row not read", entry.id));
    }
    Ok(entry)
}

fn start_number(state: &State, class: &ClassId, e: &Entry) -> Option<u32> {
    state.starts.get(class).and_then(|s| s.iter().find(|s| s.entry == e.id)).and_then(|s| s.start_number)
}

/// A guest horse's own name wins (in capitals, as shown everywhere): Rosson's placeholder horse 5
/// is shared by all of them.
fn entry_horse_name(state: &State, e: &Entry) -> String {
    e.guest_horse_shown().or_else(|| e.horse.as_ref().and_then(|h| state.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default()
}

/// hc and Nichtstarter are reported, not applied.
fn note_entry_flags(result: &mut Import, row: &Row, line: usize, entry: Option<&Entry>, _started: &[&Entry]) {
    let flag = |c: &str| row.get(c).trim().to_uppercase();
    if let Some(e) = entry {
        if flag("hc") == "HC" && !e.hors_concours {
            result.notes.push(format!("line {line}: marked hc, but entry {} is not hors concours — not changed", e.id));
        }
    }
    if matches!(flag("Nichtstarter").as_str(), "NS" | "TRUE") {
        result.notes.push(format!("line {line}: marked as not starting (Nichtstarter) — not changed; withdraw the entry in the office"));
    }
}

/// Decimal of a value as Rosson stored it (text or number), for comparisons.
pub fn decimal(text: &str) -> Option<Decimal> {
    scoring::number::parse(text).ok().flatten()
}
