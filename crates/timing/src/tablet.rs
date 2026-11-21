//! Dressage: the file for the judges' tablets.
//!
//! The show office exports a class's start list; each judge's tablet app fills in the marks and
//! sends the file back; the office imports it. Like the timekeeping files, the layout is an
//! external contract (tablet apps read and write it) and is kept as Rosson 3.3.15 writes it;
//! `tests/tablet_contract.rs` checks it against files produced by Rosson itself
//! (`golden/corpus/<show>/timing/*-tablet*`).
//!
//! # Layout
//!
//! Excel, one sheet named `Prüfung_<class number>`. Columns:
//!
//! | Column | Content |
//! |---|---|
//! | `p_number` | Swiss Equestrian id of the published class |
//! | `category` | programme name (e.g. `L12/60`), despite the column name |
//! | `programm_id` | Swiss Equestrian programme id |
//! | `starting_number`, `rider_id`, `rider_name`, `horse_id`, `horse_name` | the starter |
//! | `ranking` | empty |
//! | `status` | filled in by the tablet: `OK`, or a code as in the timekeeping files (`DISQUAL`, `RET`, `DNS`, `DSQ`) |
//! | `judge` | the judge's position (E, H, C, M, B) |
//! | `V<n>` | the mark for movement *n* |
//! | `penalty_<i>` | penalty *i* of the programme, its value when it was given (`penalty_<t>_<i>` when the programme has several penalty tables, and always in classes where every rider chooses the programme) |
//!
//! One row per starter **and judge**, judges in score-sheet order (E, H, C, M, B). Riders who do
//! not start are left out.
//!
//! - **Classes where every rider chooses the programme** (AlaCarte, AChoix): the movement columns
//!   are `V1` … up to the longest programme, and the cells beyond a rider's own programme hold
//!   `-`; the same for penalty columns. *Better than Rosson:* Rosson marks unused penalty cells
//!   only when a rider's programme has no penalty table at all (a variable shadowing slip), so a
//!   tablet could offer a penalty the rider's programme does not have.
//! - **Children programmes**: one row per starter; the judges mark together, their positions
//!   joined in the `judge` cell (`E / C`).
//! - **Classes where judges enter only their total**: no `programm_id` and `judge` columns, no
//!   marks; instead `points_total`, then `ranking_<position>` and `points_<position>` per judge.
//!   *Better than Rosson:* Rosson writes one cell more per row than there are columns.
//!
//! # Import
//!
//! Excel, or text separated by tabs, commas or semicolons; columns are found by name, the first
//! two must be `p_number` and `category`. What is read, *done by the book* where Rosson was not:
//!
//! - **The rider** by `rider_id` and `horse_id` among the starters, like the timekeeping files;
//!   when the same rider and horse start twice, by `starting_number`.
//! - **The judge** by the `judge` cell. *Rosson* ignored it and cut the rows into blocks of as
//!   many rows as the class has judges, so one missing or extra row shifted every following
//!   rider's marks to the wrong judge or rider. When two judges sit at the same position, their
//!   rows are taken in the order of the export.
//! - **The status** per rider, from its own rows. *Rosson* carried the last status it had seen
//!   over to the following riders: a rider without status after an eliminated one was imported
//!   as eliminated. Rows with neither status nor marks are not judged yet and are skipped; marks
//!   without a status are reported, not imported. When a rider's rows disagree, the rider is out
//!   (the strictest reading) and it is reported.
//! - **Marks** by the movement number in the column name (`V12` is movement 12). *Rosson* took the
//!   position of the cell instead, which only works while programmes number their movements
//!   1, 2, 3, … Marks must be numbers from 0 to 10 (`7,5` is accepted); anything else is reported.
//! - **Penalties** by column: `penalty_2` is the programme's second penalty, ticked when the cell
//!   holds its value. *Rosson* ticked every penalty *with the same value*: in a programme whose
//!   three errors each cost 2 points, one error became three. A different value is reported,
//!   not guessed. A penalty without a value (the third error, elimination) is reported: the
//!   status decides.
//! - **Text files** with any line ends. *Rosson* split on both CR and LF and then dropped every
//!   second line, which only works with Windows line ends (CRLF); a file with LF lost half its
//!   riders. Windows-1252 and UTF-8 are both decoded.
//!
//! Not covered yet, refused with an error: importing Children programmes and judges' totals
//! (their scoring is not decided, see `scoring::dressage`), and team classes (quadrilles), whose
//! programme belongs to the team.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use show::master::{self, PenaltyField, PenaltyValue};
use show::model::*;
use show::oplog::Change;
use show::State;

use crate::{entry_horse_name, find_entry, read_table, reasons, started, status, Cell, Error, Import, Row, Status};

/// The columns before the marks.
pub const FIXED_COLUMNS: [&str; 11] =
    ["p_number", "category", "programm_id", "starting_number", "rider_id", "rider_name", "horse_id", "horse_name", "ranking", "status", "judge"];

/// Written when a class has no programme (as Rosson).
const NO_PROGRAMME: &str = "Kein Programm ausgewählt";

/// A tablet file: its column names and rows.
#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub head: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
}

/// How a programme looks in the file.
struct Layout<'a> {
    def: &'a ProgrammeDef,
    /// Movement numbers, in order.
    movements: Vec<String>,
    penalties: Vec<Vec<PenaltyField>>,
    children: bool,
}

fn layout(def: &ProgrammeDef) -> Result<Layout<'_>, Error> {
    let programme = master::programme(def).map_err(Error)?;
    Ok(Layout {
        def,
        movements: programme.movements.iter().map(|m| m.number.clone()).collect(),
        penalties: master::penalty_tables(def).map_err(Error)?,
        children: programme.kind == scoring::dressage::ProgrammeKind::Children,
    })
}

/// The class and its starters with their programme (`None`: the class has none).
struct ClassFile<'a> {
    class: &'a Class,
    individual: bool,
    class_layout: Option<Layout<'a>>,
    starters: Vec<(&'a Start, &'a Entry, Option<Layout<'a>>)>,
}

fn class_file<'a>(state: &'a State, class_id: &ClassId) -> Result<ClassFile<'a>, Error> {
    let class = state.classes.get(class_id).ok_or_else(|| Error(format!("unknown class {class_id}")))?;
    if state.published_classes.get(&class.published).is_some_and(|p| p.team_size.is_some_and(|n| n > 0)) {
        return Err(Error(format!("class {}: team classes (quadrilles) are not covered yet", class.number)));
    }
    let class_def = class.programme.and_then(|p| state.programmes.get(&p));
    let individual = class_def.is_some_and(master::is_individual);
    let class_layout = match class_def {
        Some(def) if !individual => Some(layout(def)?),
        _ => None,
    };
    let mut starters = Vec::new();
    for start in state.starts.get(class_id).into_iter().flatten() {
        let Some(entry) = state.entries.get(&start.entry).filter(|e| e.status == EntryStatus::Entered) else { continue };
        let own = if individual {
            let def = entry.programme.and_then(|p| state.programmes.get(&p)).ok_or_else(|| Error(format!("class {}: entry {} has no programme", class.number, entry.id)))?;
            Some(layout(def)?)
        } else {
            None
        };
        starters.push((start, entry, own));
    }
    starters.sort_by_key(|(s, _, _)| s.order);
    Ok(ClassFile { class, individual, class_layout, starters })
}

/// Penalty column names for a programme's tables (`always_numbered`: `penalty_<t>_<i>` even for one table).
fn penalty_columns(counts: &[usize], always_numbered: bool) -> Vec<Vec<String>> {
    let numbered = always_numbered || counts.len() > 1;
    counts.iter().enumerate().map(|(t, n)| (0..*n).map(|i| if numbered { format!("penalty_{}_{}", t + 1, i + 1) } else { format!("penalty_{}", i + 1) }).collect()).collect()
}

// ─── Export ──────────────────────────────────────────────────────────────────

/// The tablet file of a class.
pub fn tablet_sheet(state: &State, class_id: &ClassId) -> Result<Sheet, Error> {
    let file = class_file(state, class_id)?;
    let judges = state.dressage_judges(class_id);
    let starter_cells = |start: &Start, entry: &Entry| -> [Cell; 5] {
        [
            start.start_number.map_or(Cell::Empty, |n| Cell::Number(n as u64)),
            Cell::Number(entry.rider.0),
            Cell::Text(state.persons.get(&entry.rider).map(|p| format!("{} {}", p.first_name, p.last_name)).unwrap_or_default()),
            entry.horse.as_ref().map_or(Cell::Empty, |h| Cell::Number(h.0)),
            Cell::Text(entry_horse_name(state, entry)),
        ]
    };
    let programme_name = |l: Option<&Layout>| Cell::Text(l.map_or(NO_PROGRAMME.to_string(), |l| l.def.name.clone()));
    let published = Cell::Number(file.class.published.0);

    if file.class.options.totals_per_judge {
        let mut head: Vec<String> = FIXED_COLUMNS.iter().filter(|c| !matches!(**c, "programm_id" | "judge")).map(|c| c.to_string()).collect();
        head.push("points_total".into());
        for j in &judges {
            let letter = j.letter.clone().unwrap_or_default();
            head.extend([format!("ranking_{letter}"), format!("points_{letter}")]);
        }
        let rows = file
            .starters
            .iter()
            .map(|(start, entry, own)| {
                let mut row = vec![published.clone(), programme_name(own.as_ref().or(file.class_layout.as_ref()))];
                row.extend(starter_cells(start, entry));
                row.resize(head.len(), Cell::Empty);
                row
            })
            .collect();
        return Ok(Sheet { head, rows });
    }

    // Movement and penalty columns.
    let (movement_columns, penalty_columns): (Vec<String>, Vec<Vec<String>>) = if file.individual {
        let longest = file.starters.iter().filter_map(|(_, _, l)| l.as_ref()).map(|l| l.movements.len()).max().unwrap_or(0);
        let tables = file.starters.iter().filter_map(|(_, _, l)| l.as_ref()).map(|l| l.penalties.len()).max().unwrap_or(0);
        let counts: Vec<usize> = (0..tables).map(|t| file.starters.iter().filter_map(|(_, _, l)| l.as_ref()?.penalties.get(t).map(Vec::len)).max().unwrap_or(0)).collect();
        ((1..=longest).map(|n| format!("V{n}")).collect(), penalty_columns(&counts, true))
    } else {
        match &file.class_layout {
            Some(l) => (l.movements.iter().map(|n| format!("V{n}")).collect(), penalty_columns(&l.penalties.iter().map(Vec::len).collect::<Vec<_>>(), false)),
            None => (Vec::new(), Vec::new()),
        }
    };
    let mut head: Vec<String> = FIXED_COLUMNS.iter().map(|c| c.to_string()).collect();
    head.extend(movement_columns.iter().cloned());
    head.extend(penalty_columns.iter().flatten().cloned());

    // Children: the judges mark together, on one row.
    let children = file.class_layout.as_ref().is_some_and(|l| l.children);
    let letters: Vec<String> = if children {
        let mut unique: Vec<String> = Vec::new();
        for l in judges.iter().filter_map(|j| j.letter.clone()) {
            if !unique.contains(&l) {
                unique.push(l);
            }
        }
        vec![unique.join(" / ")]
    } else {
        judges.iter().map(|j| j.letter.clone().unwrap_or_default()).collect()
    };

    let mut rows = Vec::new();
    for (start, entry, own) in &file.starters {
        let programme = own.as_ref().or(file.class_layout.as_ref());
        let [number, rider, name, horse, horse_name] = starter_cells(start, entry);
        // Cells beyond the rider's own programme (individual programmes only).
        let unused = |used: bool| if used || !file.individual { Cell::Empty } else { Cell::Text("-".into()) };
        let movements = programme.map_or(0, |l| l.movements.len());
        for letter in &letters {
            let mut row = vec![
                published.clone(),
                programme_name(programme),
                programme.map_or(Cell::Empty, |l| Cell::Number(l.def.id as u64)),
                number.clone(),
                rider.clone(),
                name.clone(),
                horse.clone(),
                horse_name.clone(),
                Cell::Empty,
                Cell::Empty,
                Cell::Text(letter.clone()),
            ];
            row.extend((0..movement_columns.len()).map(|i| unused(i < movements)));
            for (t, columns) in penalty_columns.iter().enumerate() {
                let fields = programme.and_then(|l| l.penalties.get(t)).map_or(0, Vec::len);
                row.extend((0..columns.len()).map(|i| unused(i < fields)));
            }
            rows.push(row);
        }
    }
    Ok(Sheet { head, rows })
}

/// The tablet file as Excel.
pub fn tablet_xlsx(sheet: &Sheet, class_number: &str) -> Result<Vec<u8>, Error> {
    crate::xlsx(&sheet.head.iter().map(String::as_str).collect::<Vec<_>>(), &sheet.rows, class_number)
}

// ─── Import ──────────────────────────────────────────────────────────────────

/// Read a tablet file, Excel or text.
pub fn read_tablet(bytes: &[u8]) -> Result<Vec<Row>, Error> {
    read_table(bytes, ["p_number", "category"])
}

/// The marks, penalties and status in a filled-in tablet file, as changes to a class.
pub fn import_tablet(state: &State, class_id: &ClassId, rows: &[Row]) -> Result<Import, Error> {
    let file = class_file(state, class_id)?;
    let class = file.class;
    if class.options.totals_per_judge {
        return Err(Error(format!("class {}: judges' totals are not covered yet", class.number)));
    }
    if file.class_layout.as_ref().is_some_and(|l| l.children) {
        return Err(Error(format!("class {}: Children programmes are not covered yet", class.number)));
    }
    let discipline = state.published_classes.get(&class.published).map(|p| p.discipline.clone()).unwrap_or_default();
    let reasons = reasons(state, &discipline);
    let started = started(state, class_id);
    let judges = state.dressage_judges(class_id);
    let mut result = Import::default();

    // The rows of each rider, in file order.
    let mut riders: Vec<(&Entry, Vec<&Row>)> = Vec::new();
    for row in rows {
        match find_entry(state, class, &started, row.get("rider_id"), row.get("horse_id"), row.get("horse_name"), row.get("starting_number")) {
            Ok(entry) => match riders.iter_mut().find(|(e, _)| e.id == entry.id) {
                Some((_, rows)) => rows.push(row),
                None => riders.push((entry, vec![row])),
            },
            Err(note) => result.notes.push(format!("line {}: {note}", row.line)),
        }
    }

    for (entry, rows) in riders {
        let Some(programme) = file.starters.iter().find(|(_, e, _)| e.id == entry.id).and_then(|(_, _, own)| own.as_ref().or(file.class_layout.as_ref())) else {
            result.notes.push(format!("line {}: class {} has no programme, rider not read", rows[0].line, class.number));
            continue;
        };
        let who = format!("start number {}", rows[0].get("starting_number"));

        // Status: from the rider's own rows.
        let mut statuses = Vec::new();
        for row in &rows {
            match status(row.get("status"), &reasons)? {
                Status::Empty => {}
                Status::Unknown(code) => result.notes.push(format!("line {}: unknown status {code:?}", row.line)),
                s => statuses.push(s),
            }
        }
        let mut sheets: Vec<(&Row, JudgeSheet)> = rows.iter().map(|row| (*row, sheet(row, programme, file.individual, &mut result.notes))).collect();
        one_set(&mut sheets);
        let anything = sheets.iter().any(|(_, s)| !is_empty(s));
        let out = match statuses.as_slice() {
            [] => {
                if anything {
                    result.notes.push(format!("line {}: {who}: marks but no status (OK or a reason), not read", rows[0].line));
                }
                continue;
            }
            [first, rest @ ..] => {
                if rest.iter().any(|s| s != first) {
                    result.notes.push(format!("line {}: {who}: the judges' rows give different statuses; taken as out", rows[0].line));
                }
                statuses.iter().find_map(|s| if let Status::Out(code) = s { Some(*code) } else { None })
            }
        };
        // The exclusion box ticked (more errors than the programme allows): eliminated (S40).
        let excluded = sheets.iter().any(|(_, s)| s.excluded);
        let out = match out {
            None if excluded => {
                let eliminated = reasons.iter().find(|r| r.name == "ausgeschieden").map(|r| r.id);
                result.notes.push(format!("line {}: {who}: the exclusion box is ticked: eliminated", rows[0].line));
                eliminated
            }
            out => out,
        };
        result.changes.push(Change::ResultEntered { class: class_id.clone(), entry: entry.id.clone(), values: BTreeMap::new(), out });

        // Each row to its judge: the n-th row at a position to the n-th judge there.
        let mut used: Vec<&AssignmentId> = Vec::new();
        for (row, sheet) in sheets {
            let letter = row.get("judge").trim();
            let Some(judge) = judges.iter().find(|j| j.letter.as_deref() == Some(letter) && !used.contains(&&j.id)) else {
                result.notes.push(format!("line {}: {who}: no (further) judge at {letter:?} in class {}, row not read", row.line, class.number));
                continue;
            };
            used.push(&judge.id);
            if out.is_none() {
                let missing: Vec<&str> = programme.movements.iter().filter(|m| !sheet.marks.contains_key(*m)).map(String::as_str).collect();
                if !missing.is_empty() && !is_empty(&sheet) {
                    result.notes.push(format!("line {}: {who}, judge {letter}: no mark for movement {}", row.line, missing.join(", ")));
                }
            }
            if !is_empty(&sheet) {
                result.changes.push(Change::JudgeSheetEntered { class: class_id.clone(), entry: entry.id.clone(), judge: judge.id.clone(), sheet });
            }
        }
    }
    Ok(result)
}

fn is_empty(sheet: &JudgeSheet) -> bool {
    sheet.marks.is_empty() && sheet.penalty_points.is_empty() && sheet.penalty_percent.is_empty() && !sheet.excluded
}

/// One set of penalties for every judge (S40): what any judge's row ticks — each value as many
/// times as the row that ticks it most, the exclusion box if any row ticks it.
fn one_set(sheets: &mut [(&Row, JudgeSheet)]) {
    let most = |values: Vec<&Vec<Decimal>>| -> Vec<Decimal> {
        let mut distinct: Vec<Decimal> = values.iter().flat_map(|v| v.iter().copied()).collect();
        distinct.sort();
        distinct.dedup();
        distinct.iter().flat_map(|d| std::iter::repeat_n(*d, values.iter().map(|v| v.iter().filter(|x| *x == d).count()).max().unwrap_or(0))).collect()
    };
    let points = most(sheets.iter().map(|(_, s)| &s.penalty_points).collect());
    let percent = most(sheets.iter().map(|(_, s)| &s.penalty_percent).collect());
    let excluded = sheets.iter().any(|(_, s)| s.excluded);
    for (_, s) in sheets.iter_mut() {
        s.penalty_points = points.clone();
        s.penalty_percent = percent.clone();
        s.excluded = excluded;
    }
}

/// One judge's marks and penalties from a row.
fn sheet(row: &Row, programme: &Layout, individual: bool, notes: &mut Vec<String>) -> JudgeSheet {
    let line = row.line;
    let mut sheet = JudgeSheet::default();
    let blank = |text: &str| text.trim().is_empty() || text.trim() == "-";

    // Marks: V<number>, or V<position> where every rider chooses the programme.
    for (i, number) in programme.movements.iter().enumerate() {
        let column = if individual { format!("V{}", i + 1) } else { format!("V{number}") };
        let text = row.get(&column);
        if blank(text) {
            continue;
        }
        match scoring::number::parse(text) {
            Ok(Some(mark)) if (Decimal::ZERO..=Decimal::TEN).contains(&mark) => {
                sheet.marks.insert(number.clone(), mark);
            }
            _ => notes.push(format!("line {line}: {column}: {text:?} is not a mark from 0 to 10, not read")),
        }
    }
    if individual {
        let extra: Vec<&String> = row.cells.iter().filter(|(c, v)| c.strip_prefix('V').and_then(|n| n.parse::<usize>().ok()).is_some_and(|n| n > programme.movements.len()) && !blank(v)).map(|(c, _)| c).collect();
        if !extra.is_empty() {
            notes.push(format!("line {line}: the rider's programme {} has {} movements; {} not read", programme.def.name, programme.movements.len(), extra.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ")));
        }
    }

    // Penalties: ticked when the cell holds the penalty's value.
    let columns = penalty_columns(&programme.penalties.iter().map(Vec::len).collect::<Vec<_>>(), individual);
    for (t, fields) in programme.penalties.iter().enumerate() {
        for (i, field) in fields.iter().enumerate() {
            let column = &columns[t][i];
            // Accept both namings: penalty_<i> and penalty_1_<i> for a single table.
            let text = match row.get(column) {
                "" if programme.penalties.len() == 1 => row.get(&format!("penalty_{}_{}", t + 1, i + 1)),
                text => text,
            };
            if blank(text) {
                continue;
            }
            let given = scoring::number::parse(text).ok().flatten();
            if given == Some(Decimal::ZERO) {
                continue; // nothing given
            }
            match (field.value, given) {
                // The exclusion box: the rider is eliminated (S40).
                (None, _) => sheet.excluded = true,
                (Some(PenaltyValue::Points(v)), Some(g)) if g == v => sheet.penalty_points.push(v),
                (Some(PenaltyValue::Percent(v)), Some(g)) if g == v => sheet.penalty_percent.push(v),
                (Some(_), _) => notes.push(format!("line {line}: {column}: {text:?} is not the value of {} ({}), not read", field.label, field.cost)),
            }
        }
    }
    sheet
}
