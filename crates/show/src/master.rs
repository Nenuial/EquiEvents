//! Swiss Equestrian master data: reading the published definitions into the scoring model.

use rust_decimal::Decimal;
use scoring::dressage::{Movement, Part, Programme, ProgrammeKind};
use scoring::jumping::OutKind;
use scoring::number;
use serde_json::Value;

use crate::model::ProgrammeDef;

/// Programme names that mean "every rider chooses their own programme".
pub const INDIVIDUAL_PROGRAMME: [&str; 2] = ["AlaCarte", "AChoix"];

/// A dressage programme from its published definition: the scored movements (all tables except
/// penalties and movements without marks) with their coefficients (1 when not given).
pub fn programme(def: &ProgrammeDef) -> Result<Programme, String> {
    let json: Value = serde_json::from_str(&def.definition).map_err(|e| format!("programme {}: {e}", def.name))?;
    let kind = if json["kuer"] == true {
        ProgrammeKind::Freestyle
    } else if json["children"] == true {
        ProgrammeKind::Children
    } else {
        ProgrammeKind::Standard
    };
    let tables = json["tabellen"].as_array().ok_or_else(|| format!("programme {}: no tables", def.name))?;
    let mut movements = Vec::new();
    for t in tables {
        let layout = t["layout"].as_str().unwrap_or("");
        if layout == "strafpunkte" || layout == "lektionen_ohne_noten" {
            continue;
        }
        for f in t["felder"].as_array().into_iter().flatten() {
            let coefficient = decimal(&f["koeffizient"]).filter(|c| !c.is_zero()).unwrap_or(Decimal::ONE);
            let number = match &f["nummer"] {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            };
            movements.push(Movement { number, coefficient, part: part(kind, t, f) });
        }
    }
    Ok(Programme { name: def.name.clone(), kind, movements, max_points: def.max_points, final_ties: None })
}

/// The part a movement counts for, as Rosson reads it: freestyle, from its table's German title
/// ("Technische Noten (A)", "Künstlerische Noten (B)"); Children, from its table's `teilnote`
/// ("A", "B"), else its own.
fn part(kind: ProgrammeKind, table: &Value, field: &Value) -> Option<Part> {
    match kind {
        ProgrammeKind::Standard => None,
        ProgrammeKind::Freestyle => {
            let title = table["titel_de"].as_str().unwrap_or("");
            if title.contains("(A)") {
                Some(Part::A)
            } else if title.contains("(B)") {
                Some(Part::B)
            } else {
                None
            }
        }
        ProgrammeKind::Children => match table["teilnote"].as_str().filter(|t| !t.is_empty()).or_else(|| field["teilnote"].as_str()) {
            Some("A") => Some(Part::A),
            Some("B") => Some(Part::B),
            _ => None,
        },
    }
}

/// OKV team dressage (the programme's `okv_mannschaftsdressur`).
pub fn is_okv_team(def: &ProgrammeDef) -> bool {
    serde_json::from_str::<Value>(&def.definition).is_ok_and(|json| json["okv_mannschaftsdressur"] == true)
}

/// The value of a penalty in a programme's penalty table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenaltyValue {
    /// Points taken off the judge's total (standard programmes).
    Points(Decimal),
    /// Percent taken off (freestyle).
    Percent(Decimal),
}

/// One penalty of a programme, e.g. "Erster Programmfehler: 2 Strafpunkte".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PenaltyField {
    /// German description, e.g. "Erster Programmfehler".
    pub label: String,
    /// What it costs, as German text, e.g. "2 Strafpunkte" or "Ausschluss".
    pub cost: String,
    /// `None`: no value (e.g. the third error, which means elimination).
    pub value: Option<PenaltyValue>,
}

/// A programme's penalty tables (layout `strafpunkte`), in order, each with its penalties in order.
pub fn penalty_tables(def: &ProgrammeDef) -> Result<Vec<Vec<PenaltyField>>, String> {
    let json: Value = serde_json::from_str(&def.definition).map_err(|e| format!("programme {}: {e}", def.name))?;
    let tables = json["tabellen"].as_array().ok_or_else(|| format!("programme {}: no tables", def.name))?;
    Ok(tables
        .iter()
        .filter(|t| t["layout"] == "strafpunkte")
        .map(|t| {
            t["felder"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|f| PenaltyField {
                    label: f["beschreibung_de"].as_str().unwrap_or("").to_string(),
                    cost: f["strafe_de"].as_str().unwrap_or("").to_string(),
                    value: decimal(&f["strafpunkte"])
                        .filter(|d| !d.is_zero())
                        .map(PenaltyValue::Points)
                        .or_else(|| decimal(&f["strafprozente"]).filter(|d| !d.is_zero()).map(PenaltyValue::Percent)),
                })
                .collect()
        })
        .collect())
}

/// A programme's title in a language (`de`, `fr`, `it`), from its definition (`titel_de`, …):
/// e.g. "Dressuraufgabe GA 06/60". Empty when the definition has none.
pub fn programme_title(def: &ProgrammeDef, language: &str) -> String {
    let Ok(json) = serde_json::from_str::<Value>(&def.definition) else { return String::new() };
    // Italian is not always there (Children's tests): German, as Swiss Equestrian always gives it.
    [language, "de"].iter().filter_map(|l| json[format!("titel_{l}")].as_str()).map(str::trim).find(|t| !t.is_empty()).unwrap_or_default().to_string()
}

/// Tag der Jugend finals: the movements whose marks by judges C and H break ties in ranks 1–3,
/// in order (RL Tag der Jugend 10.3; S38). `None`: not a Tag der Jugend programme.
pub fn youth_final_movements(def: &ProgrammeDef) -> Option<[&'static str; 2]> {
    match def.name.as_str() {
        "TdJ Fuerz" => Some(["2", "3"]),
        "TdJ EinRei" => Some(["6", "3"]),
        "TdJ Einste" => Some(["15", "17"]),
        // The regulation's movement 24 is not in the 2018 programme (23 movements): it separates
        // nobody, lots decide.
        "TdJ DresPr" => Some(["21", "24"]),
        _ => None,
    }
}

/// Young horses' programmes (JP…) are judged in tenths; every other programme in halves
/// (Dressurreglement 8.2 (2)).
pub fn in_tenths(def: &ProgrammeDef) -> bool {
    def.name.starts_with("JP")
}

/// Young horses' programmes (JP…) are judged jointly: one sheet for the jury, entered as the
/// first judge's and counted for every judge of the class (Dressurreglement 8.1; S39).
pub fn judged_jointly(def: &ProgrammeDef) -> bool {
    def.name.starts_with("JP")
}

/// Every rider chooses their own programme (the class's programme is "AlaCarte" or "AChoix").
pub fn is_individual(def: &ProgrammeDef) -> bool {
    INDIVIDUAL_PROGRAMME.contains(&def.name.as_str())
}

fn decimal(v: &Value) -> Option<Decimal> {
    match v {
        Value::Number(n) => number::parse(&n.to_string()).ok().flatten(),
        Value::String(s) => number::parse(s).ok().flatten(),
        _ => None,
    }
}

/// The kind of a Swiss Equestrian reason (Aufgabegrund), from its German label: "verzichtet" or
/// "zurückgezogen" is withdrawn, "aufgegeben" retired, "ausgeschieden", "eliminiert" or
/// "disqualifiziert" eliminated.
pub fn reason_kind(label: &str) -> OutKind {
    let l = label.to_lowercase();
    if l.contains("verzichtet") || l.contains("zurückgezogen") {
        OutKind::Withdrawn
    } else if l.contains("aufgegeben") {
        OutKind::Retired
    } else if l.contains("ausgeschieden") || l.contains("eliminiert") || l.contains("disqualifiziert") {
        OutKind::Eliminated
    } else {
        OutKind::Other
    }
}
