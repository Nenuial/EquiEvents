//! Read Rosson data — its exports, and the golden corpus built from them — into the scoring
//! model.
//!
//! Rosson's shapes are loose: ids are sometimes strings and sometimes numbers, numbers are often
//! text, the entry is copied into every result. This crate is the only place that knows about
//! that. It decides, per class, which results Rosson would rank (the same selection as
//! `RanglisteService.getResultate`), and returns what Rosson stored next to the scoring input so
//! the two can be compared.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub mod delivered;
pub mod export;
pub mod ops;
pub mod swiss_time;
pub mod to_rosson;

use rust_decimal::Decimal;
use scoring::dressage::{self, JudgeSheet, Penalty, Programme};
use scoring::jumping::{self, table::ScoringTable, team::Team};
use scoring::number;
use serde_json::{Map, Value};

/// One show, ready for scoring.
#[derive(Debug)]
pub struct Show {
    pub nr: String,
    pub classes: Vec<Class>,
}

/// One class (Prüfung).
#[derive(Debug)]
pub struct Class {
    pub pruefung_id: String,
    pub number: String,
    pub name: String,
    pub kind: ClassKind,
    /// What Rosson stored for each result it ranks in this class.
    pub stored: Vec<Stored>,
    /// Team classes: what Rosson stored for each team (keyed by team id).
    pub stored_teams: Vec<Stored>,
    /// Number of placed riders if the organiser set it by hand.
    pub placed_count_override: Option<usize>,
    /// Number of results Rosson counts for the default placed count.
    pub results_counted: usize,
    /// What Rosson stored as the placed count.
    pub stored_placed_count: Option<usize>,
}

/// The scoring input of a class (shared with the show data model).
pub use show::ScoringInput as ClassKind;

/// What Rosson stored for one result.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stored {
    /// The id the engine uses for this result (Rosson's result id, or the entry id via the store).
    pub id: String,
    /// Rosson's result record id.
    pub record: String,
    pub rank: Option<Decimal>,
    pub placed: Option<bool>,
    pub faults: Option<Decimal>,
    pub time: Option<Decimal>,
    pub percentage: Option<Decimal>,
    pub judge_totals: BTreeMap<String, Decimal>,
}

impl Stored {
    /// Read the computed fields of a Rosson result record.
    pub fn from_record(id: &str, r: &Value) -> Stored {
        let judge_totals = r
            .get("total")
            .and_then(Value::as_object)
            .map(|m| m.iter().filter_map(|(k, v)| decimal(v).map(|d| (k.clone(), d))).collect())
            .unwrap_or_default();
        // An overall row (eventing, driving) keeps its total by how the class is ranked.
        let overall = || {
            let total = |k: &str| r.get(k).and_then(decimal);
            ["rangpunkte_total", "punkte_total"].iter().filter_map(|k| total(k)).find(|t| !t.is_zero()).or_else(|| total("strafpunkte_total"))
        };
        Stored {
            id: id.to_string(),
            record: id.to_string(),
            rank: r.get("rang").and_then(decimal),
            placed: r.get("klassiert").and_then(Value::as_bool),
            faults: if r["typ"] == "gesamt" { overall() } else { r.get("strafpunkte").filter(|v| !v.is_object()).and_then(decimal) },
            time: r.get("zeit").and_then(decimal),
            percentage: r.get("gesamttotal").and_then(decimal),
            judge_totals,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportError(pub String);

/// Statuses with which Rosson marks an entry as not starting (`nennungAnreichern`).
pub(crate) const NOT_STARTING: [&str; 3] = ["abgemeldet", "zurueckgewiesen", "falschnennung"];
/// Programme names that mean "every rider chooses their own programme" (appConfig).
use show::master::INDIVIDUAL_PROGRAMME;

/// Read a show from a corpus snapshot (Rosson's stores, cut to one event).
pub fn read_show(snapshot: &Value) -> Result<Show, ImportError> {
    let store = |name: &str| snapshot.get(name).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let by_id = |name: &str| -> HashMap<String, &Value> { store(name).iter().map(|r| (id(&r["id"]), r)).collect() };

    let nr = store("veranstaltungen").first().map(|v| id(&v["nr"])).ok_or_else(|| ImportError("no event".into()))?;
    let disziplin: HashMap<String, String> = store("disziplinen").iter().map(|d| (id(&d["id"]), text(&d["code"]))).collect();
    let ausschreibung = by_id("ausschreibung_pruefungen");
    let wertungen = by_id("wertungen");
    let programme = by_id("programme");
    let nennungen = by_id("nennungen");
    let starter = by_id("starter");
    let reasons: HashMap<u32, jumping::OutKind> = store("aufgabegruende").iter().filter_map(|g| Some((g["id"].as_u64()? as u32, show::master::reason_kind(&text(&g["bezeichnung_de"]))))).collect();
    let dri_role = store("pruefungsfunktionen").iter().find(|f| f["code"] == "DRI").map(|f| id(&f["id"]));

    let mut classes = Vec::new();
    for p in store("pruefungen") {
        let pruefung_id = id(&p["id"]);
        let ap = ausschreibung.get(&id(&p["ausschreibung_pruefung_id"])).copied().unwrap_or(&Value::Null);
        let code = disziplin.get(&id(&ap["disziplin_id"])).cloned().unwrap_or_default();

        // The results Rosson ranks in this class (RanglisteService.getResultate), team rows aside.
        let mut rows: Vec<(&Value, &Value)> = Vec::new(); // (result, entry)
        let mut results_counted = 0;
        for r in store("resultate") {
            if id(&r["pruefung_id"]) != pruefung_id || !r["zwischenpruefung_id"].is_null() || r["gestrichen"] == true {
                continue;
            }
            if r.get("starter_id").map_or(true, Value::is_null) {
                continue; // team row (S7)
            }
            let Some(s) = starter.get(&id(&r["starter_id"])) else { continue };
            let Some(n) = nennungen.get(&id(&s["nennung_id"])) else { continue };
            // The default placed count counts every result except hors concours, eliminated and
            // retired included (ResultatService.resultateVonPruefung).
            if n["hors_concours"] != true {
                results_counted += 1;
            }
            let not_starting = n["kein_start"] == true || NOT_STARTING.contains(&text(&n["status"]).as_str());
            if not_starting || n["hors_concours"] == true {
                continue;
            }
            rows.push((r, n));
        }

        let stored = rows.iter().map(|(r, _)| Stored::from_record(&id(&r["id"]), r)).collect();
        let has_teams = p.get("equipen").and_then(Value::as_array).is_some_and(|e| !e.is_empty());

        // Team rows: one per team, no starter (S7).
        let team_rows: Vec<&Value> = store("resultate")
            .iter()
            .filter(|r| id(&r["pruefung_id"]) == pruefung_id && r.get("starter_id").map_or(true, Value::is_null) && !r["equipe_id"].is_null())
            .collect();
        let stored_teams: Vec<Stored> = team_rows.iter().map(|r| Stored::from_record(&id(&r["equipe_id"]), r)).collect();
        if has_teams {
            results_counted = team_rows.len();
        }

        let kind = if has_teams && code == "CS" {
            match jumping_class(p, &rows, &wertungen, &reasons) {
                ClassKind::Jumping { table, entries } => {
                    let class = team_class(table, entries, &rows, &team_rows, &by_id("equipen"));
                    if let ClassKind::JumpingTeams { teams, .. } = &class {
                        // Teams without any starting rider have no result (S15).
                        results_counted = teams.iter().filter(|t| !t.riders.is_empty()).count();
                    }
                    class
                }
                other => other,
            }
        } else if has_teams {
            ClassKind::NotCovered(format!("team class in discipline {code}"))
        } else if code == "CD" {
            dressage_class(p, &rows, &programme, dri_role.as_deref(), snapshot)
        } else if code == "CS" {
            jumping_class(p, &rows, &wertungen, &reasons)
        } else {
            ClassKind::NotCovered(format!("discipline {code}"))
        };

        classes.push(Class {
            pruefung_id,
            number: text(&p["nummer"]),
            name: text(&p["name"]),
            kind,
            stored,
            stored_teams,
            placed_count_override: if p["customKlassierte"] == true { p["anzahlKlassiert"].as_u64().map(|n| n as usize) } else { None },
            results_counted,
            stored_placed_count: p["anzahlKlassiert"].as_u64().map(|n| n as usize),
        });
    }

    // Phases of eventing and driving: each one a class of its own, with the rows Rosson keeps for
    // it. They are scored only through the show file (`read_show_via_store`).
    let pruefungen = by_id("pruefungen");
    for z in store("zwischenpruefungen") {
        let phase_id = id(&z["id"]);
        let parent = pruefungen.get(&id(&z["parent_pruefung_id"])).copied().unwrap_or(&Value::Null);
        let driving = parent["fahren"] == true;
        let mut stored = Vec::new();
        let mut results_counted = 0;
        for r in store("resultate") {
            if id(&r["zwischenpruefung_id"]) != phase_id || r["gestrichen"] == true {
                continue;
            }
            let Some(n) = r.get("starter_id").and_then(|s| starter.get(&id(s))).and_then(|s| nennungen.get(&id(&s["nennung_id"]))) else { continue };
            if n["hors_concours"] != true {
                results_counted += 1;
            }
            if n["kein_start"] == true || NOT_STARTING.contains(&text(&n["status"]).as_str()) || n["hors_concours"] == true {
                continue;
            }
            let mut s = Stored::from_record(&id(&r["id"]), r);
            if driving {
                // Driving dressage keeps the judges' mean on 160 as `gesamttotal`; cones, marathon and
                // derby their total time as `total_zeit`.
                if r["typ"] == "dressur" {
                    s.faults = s.percentage.take();
                } else {
                    s.time = r.get("total_zeit").and_then(decimal).or(s.time);
                }
            }
            stored.push(s);
        }
        classes.push(Class {
            pruefung_id: phase_id.clone(),
            number: text(&z["nummer"]),
            name: text(&z["name"]),
            kind: ClassKind::NotCovered(format!("phase {phase_id}")),
            stored,
            stored_teams: Vec::new(),
            placed_count_override: if z["customKlassierte"] == true { z["anzahlKlassiert"].as_u64().map(|n| n as usize) } else { None },
            results_counted,
            stored_placed_count: z["anzahlKlassiert"].as_u64().map(|n| n as usize),
        });
    }
    Ok(Show { nr, classes })
}

fn jumping_class(p: &Value, rows: &[(&Value, &Value)], wertungen: &HashMap<String, &Value>, reasons: &HashMap<u32, jumping::OutKind>) -> ClassKind {
    for flag in ["idealzeit_berechnen", "max_zeit_berechnen", "kombination", "fahren"] {
        if p[flag] == true {
            return ClassKind::NotCovered(format!("class option {flag}"));
        }
    }
    let Some(w) = wertungen.get(&id(&p["wertung_id"])) else {
        return ClassKind::NotCovered("no scoring table".into());
    };
    let table = match ScoringTable::from_definition(
        w["id"].as_u64().unwrap_or(0) as u32,
        &text(&w["code"]),
        &text(&w["bezeichnung_de"]),
        w["bewertung_json"].as_str().unwrap_or(""),
    ) {
        Ok(t) => t,
        Err(e) => return ClassKind::NotCovered(format!("scoring table {}: {e:?}", text(&w["code"]))),
    };

    let mut entries = Vec::new();
    for (r, n) in rows {
        let details = r["details"].as_object();
        let mut values = BTreeMap::new();
        for (field, v) in details.into_iter().flatten() {
            if !(field.starts_with("zeit_") || field.starts_with("strafpunkte_")) {
                continue;
            }
            match value_decimal(v) {
                Ok(Some(d)) => {
                    values.insert(field.clone(), d);
                }
                Ok(None) => {}
                Err(e) => return ClassKind::NotCovered(format!("result {}: field {field}: {e}", id(&r["id"]))),
            }
        }
        entries.push(jumping::Entry {
            id: id(&r["id"]),
            horse: horse_name(n),
            values,
            out: details.and_then(|d| d.get("aufgabe")).and_then(decimal).filter(|a| a > &Decimal::ZERO).map(|a| {
                let code: u32 = a.try_into().unwrap_or(0);
                jumping::Out { code, kind: reasons.get(&code).copied().unwrap_or(jumping::OutKind::Other) }
            }),
        });
    }
    ClassKind::Jumping { table, entries }
}

/// Group the riders of a team class into their teams (the entry's `equipe_id`).
fn team_class(table: ScoringTable, entries: Vec<jumping::Entry>, rows: &[(&Value, &Value)], team_rows: &[&Value], equipen: &HashMap<String, &Value>) -> ClassKind {
    let team_of_entry: HashMap<String, String> = rows.iter().map(|(r, n)| (id(&r["id"]), id(&n["equipe_id"]))).collect();
    let mut teams: Vec<Team> = team_rows
        .iter()
        .map(|t| {
            let team_id = id(&t["equipe_id"]);
            let name = equipen.get(&team_id).map(|e| text(&e["name"])).unwrap_or_else(|| team_id.clone());
            Team { id: team_id, name, riders: Vec::new() }
        })
        .collect();
    let mut team_of = HashMap::new();
    for entry in entries {
        let Some(team_id) = team_of_entry.get(&entry.id) else { continue };
        if let Some(team) = teams.iter_mut().find(|t| &t.id == team_id) {
            team_of.insert(entry.id.clone(), team_id.clone());
            team.riders.push(entry);
        }
    }
    // Rosson always dropped the worst of a team of four (S7).
    ClassKind::JumpingTeams { table, teams, team_of, count_all: false }
}

fn dressage_class(p: &Value, rows: &[(&Value, &Value)], programme: &HashMap<String, &Value>, dri_role: Option<&str>, snapshot: &Value) -> ClassKind {
    let pruefung_id = id(&p["id"]);
    // Judges: assignments to this class with the dressage-judge role, in letter order E H C M B.
    let mut judges: Vec<(usize, String)> = snapshot["offizielle_zuweisungen"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|z| id(&z["pruefung_id"]) == pruefung_id && dri_role.is_some_and(|r| id(&z["rolle_id"]) == r))
        .map(|z| ("EHCMB".find(&text(&z["buchstabe"])).unwrap_or(9), id(&z["id"])))
        .collect();
    judges.sort();
    let judges: Vec<String> = judges.into_iter().map(|(_, j)| j).collect();

    let class_programme = programme.get(&id(&p["programm_id"])).copied();
    let individual = class_programme.is_some_and(|pr| INDIVIDUAL_PROGRAMME.contains(&text(&pr["name"]).as_str()));

    let mut entries = Vec::new();
    for (r, n) in rows {
        let prog_record = if individual { programme.get(&id(&n["programm_id"])).copied() } else { class_programme };
        let Some(prog_record) = prog_record else {
            return ClassKind::NotCovered("no programme".into());
        };
        let prog = match read_programme(prog_record) {
            Ok(p) => p,
            Err(e) => return ClassKind::NotCovered(e.0),
        };

        let details = r["details"].as_object();
        let mut sheets = BTreeMap::new();
        for judge in &judges {
            // Totals entered per judge (manuelle_eingabe): the judge's total, a number.
            if let Some(total) = details.and_then(|d| d.get(judge)).filter(|v| v.is_number() || v.is_string()).and_then(|v| value_decimal(v).ok().flatten()) {
                sheets.insert(judge.clone(), JudgeSheet { marks: BTreeMap::new(), penalties: ticked_penalties(r, judge), total: Some(total) });
                continue;
            }
            let Some(marks_json) = details.and_then(|d| d.get(judge)).and_then(Value::as_object) else { continue };
            let mut marks = BTreeMap::new();
            for (movement, v) in marks_json {
                if movement.starts_with("_origin_") {
                    continue;
                }
                if let Ok(Some(mark)) = value_decimal(v) {
                    marks.insert(movement.clone(), mark);
                }
            }
            sheets.insert(judge.clone(), JudgeSheet { marks, penalties: ticked_penalties(r, judge), total: None });
        }
        entries.push((prog, dressage::Entry {
            id: id(&r["id"]),
            horse: horse_name(n),
            out: details.and_then(|d| d.get("aufgabe")).and_then(decimal).is_some_and(|a| a > Decimal::ZERO),
            sheets,
        }));
    }
    // Rosson ranks every dressage the same way (the eventing average is ours, E11).
    ClassKind::Dressage { judges, entries, judges_mean: false }
}

/// Penalties a judge ticked: `strafpunkte[judge][table] = [{strafpunkte | strafprozente, checked}]`.
fn ticked_penalties(r: &Value, judge: &str) -> Vec<Penalty> {
    let mut out = Vec::new();
    let Some(tables) = r["strafpunkte"].get(judge).and_then(Value::as_object) else { return out };
    for list in tables.values().filter_map(Value::as_array) {
        for p in list.iter().filter(|p| p["checked"] == true) {
            if let Some(points) = decimal(&p["strafpunkte"]).filter(|d| !d.is_zero()) {
                out.push(Penalty::Points(points));
            } else if let Some(percent) = decimal(&p["strafprozente"]).filter(|d| !d.is_zero()) {
                out.push(Penalty::Percent(percent));
            }
        }
    }
    out
}

/// A dressage programme record (`programm_json` + `max_punkte`), via the shared master-data reader.
pub fn read_programme(record: &Value) -> Result<Programme, ImportError> {
    let def = show::model::ProgrammeDef {
        id: record["id"].as_u64().unwrap_or(0) as u32,
        name: text(&record["name"]),
        max_points: decimal(&record["max_punkte"]).ok_or_else(|| ImportError(format!("programme {}: no maximum", text(&record["name"]))))?,
        definition: record["programm_json"].as_str().unwrap_or("").to_string(),
        discipline_id: record["disziplin_id"].as_u64().map(|d| d as u32),
    };
    show::master::programme(&def).map_err(ImportError)
}

fn horse_name(nennung: &Value) -> String {
    nennung["pferde"].get(0).map(|p| text(&p["pferdename"])).filter(|s| !s.is_empty()).unwrap_or_else(|| text(&nennung["gastpferd_name"]))
}

/// An id as text, whether Rosson stored it as a number or a string.
pub fn id(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

pub(crate) fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// A number Rosson stored as a JSON number or as text; empty or invalid → `None`.
pub fn decimal(v: &Value) -> Option<Decimal> {
    value_decimal(v).ok().flatten()
}

pub(crate) fn value_decimal(v: &Value) -> Result<Option<Decimal>, String> {
    match v {
        Value::Number(n) => number::parse(&n.to_string()),
        Value::String(s) => number::parse(s),
        Value::Null => Ok(None),
        other => Err(format!("not a number: {other}")),
    }
}

/// Rosson's recomputed values for a show (golden/corpus/<nr>/oracle.json → `recomputed`).
pub fn read_recomputed(oracle: &Value) -> HashMap<String, Stored> {
    oracle["recomputed"]
        .as_object()
        .map(|m: &Map<String, Value>| m.iter().map(|(k, v)| (k.clone(), Stored::from_record(k, v))).collect())
        .unwrap_or_default()
}

/// Read a show the way the app will: Rosson export → operations → show file (SQLite) →
/// state → scoring input. The stored Rosson values are taken from the export, for comparison.
pub fn read_show_via_store(snapshot: &Value, file: &Path) -> Result<Show, ImportError> {
    let direct = read_show(snapshot)?;
    let operations = ops::to_operations(snapshot)?;
    {
        let mut store = show::Store::open(file).map_err(|e| ImportError(e.0))?;
        store.append(&operations).map_err(|e| ImportError(e.0))?;
    }
    let store = show::Store::open(file).map_err(|e| ImportError(e.0))?; // reopen: state rebuilt from the file
    if let Some((op, why)) = store.ignored().first() {
        return Err(ImportError(format!("operation {op} ignored: {why}")));
    }
    let state = store.state();
    // In the show file riders are identified by their entry, not by Rosson's result record.
    let starter_entry: HashMap<String, String> = snapshot["starter"].as_array().into_iter().flatten().map(|s| (id(&s["id"]), id(&s["nennung_id"]))).collect();
    let entry_of_record: HashMap<String, String> = snapshot["resultate"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| Some((id(&r["id"]), starter_entry.get(&id(&r["starter_id"]))?.clone())))
        .collect();
    let classes = direct
        .classes
        .into_iter()
        .map(|c| {
            let class_id = show::model::ClassId(c.pruefung_id.clone());
            let stored = c.stored.iter().map(|s| Stored { id: entry_of_record.get(&s.record).cloned().unwrap_or_else(|| s.id.clone()), ..s.clone() }).collect();
            Class {
                stored,
                kind: state.scoring_input(&class_id),
                results_counted: state.results_counted(&class_id),
                placed_count_override: state.classes.get(&class_id).and_then(|k| k.options.placed_count).map(|n| n as usize),
                ..c
            }
        })
        .collect();
    Ok(Show { nr: direct.nr, classes })
}
