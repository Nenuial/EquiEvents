//! Scoring tables (Wertungen): how a jumping class is judged.
//!
//! Swiss Equestrian publishes each table as a small definition. A table is a list of **blocks**, most
//! decisive first. For example table 17, "A mit Zeitmessung und 1 Stechen", has two blocks:
//!
//! | Block | Rider counts here when entered | Ranked on |
//! |---|---|---|
//! | 1 jump-off | jump-off faults and time | jump-off faults, then jump-off time |
//! | 2 round 1 | round-1 faults and time | round-1 faults, then round-1 time |
//!
//! Every rider is ranked in the first block they have complete results for, so everyone who
//! jumped off ranks ahead of everyone who did not.
//!
//! A block can be **points-based** (`prog` in the definition): style, Punktespringen,
//! progressive classes. There the highest points rank first (docs/deviations.md S1).
//!
//! Two-phase tables are recognised by their code (the `ist_phase` flag only exists in the 2026
//! master data). Two kinds are ranked differently when a rider is eliminated or retires (S2): the
//! **special** ones (codes `2PHAZPAZP`, `2PHCAZP`), where everyone rides both phases, and the
//! **normal** ones (`2PHAC`, `2PHAOAZ`, `2PHAOC`, `2PHAZ`).

use serde_json::Value;

/// A jumping scoring table.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoringTable {
    pub id: u32,
    /// Swiss Equestrian code of the table record (e.g. `AZ1AZ`).
    pub code: String,
    pub name: String,
    /// Most decisive first.
    pub blocks: Vec<Block>,
    pub phases: Phases,
    /// The class ends with a winning round (Siegerrunde).
    pub winning_round: bool,
    /// Knock-out (55): duels, round after round (Springreglement 4.5).
    pub knock_out: bool,
    /// Two rounds, the second reduced (codes `…2RU…`): only riders who completed both rounds are
    /// placed (Springreglement 3.4.2.1).
    pub reduced_second_round: bool,
}

/// One block of a table.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Fields that must all be entered for a rider to be ranked in this block.
    pub required: Vec<String>,
    /// Time fields, added up.
    pub time: Vec<String>,
    /// Fault fields (or point fields in a points-based block), added up.
    pub faults: Vec<String>,
    /// Points-based: highest first.
    pub points: bool,
    /// Ideal time (table AZI): faults, then closest to the class's ideal time, over or under the
    /// same (S20).
    pub ideal_time: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phases {
    /// Not a two-phase class.
    None,
    /// Two phases; only some riders go on to phase 2.
    Normal,
    /// Two phases; everybody rides both.
    Special,
}

/// Codes of the special two-phase tables (S2).
pub const SPECIAL_TWO_PHASE: [&str; 2] = ["2PHAZPAZP", "2PHCAZP"];
/// Codes of the normal two-phase tables. Recognised by code because the `ist_phase` flag only
/// appeared in the 2026 master data.
pub const NORMAL_TWO_PHASE: [&str; 4] = ["2PHAC", "2PHAOAZ", "2PHAOC", "2PHAZ"];

#[derive(Debug, Clone, PartialEq)]
pub enum TableError {
    /// The published definition is empty (tables 66, 101, 103 at the time of writing, which are
    /// defined here instead: `definition_text`).
    Empty,
    /// The published definition is not valid JSON (table 30 at the time of writing).
    InvalidJson(String),
    /// A required part of the definition is missing.
    Missing(&'static str),
    /// The table uses a rule that is not implemented yet.
    NotCovered(String),
}

/// Tables Swiss Equestrian publishes without a definition, defined here (docs/deviations.md S5).
/// All three are scored like the style table (63, STIL): the most points first.
/// - STILMIDEAL (103), a style class with an ideal time: then the rider closest to the ideal
///   time, over or under the same (P. Burkhard, 2026-11-14).
/// - Gymk (66), Gymkhana, scoring B of the ZKV and Swiss Freiberger rules: then the faster time.
/// - Rücke (101), Rückeprüfung of the Swiss Freiberger and Haflinger rules: then the faster time.
const POINTS_THEN_TIME: &str = r#"{"sichtbare_felder": ["zeit_initial_1", "strafpunkte_initial_1"],
  "bloecke": [{"prog": true, "definierte_felder": ["zeit_initial_1", "strafpunkte_initial_1"],
  "zeit": ["zeit_initial_1"], "strafpunkte": ["strafpunkte_initial_1"], "resultate": []}]}"#;
const POINTS_THEN_IDEAL_TIME: &str = r#"{"sichtbare_felder": ["zeit_initial_1", "strafpunkte_initial_1"],
  "bloecke": [{"prog": true, "idealzeit": true, "definierte_felder": ["zeit_initial_1", "strafpunkte_initial_1"],
  "zeit": ["zeit_initial_1"], "strafpunkte": ["strafpunkte_initial_1"], "resultate": []}]}"#;

/// Zeitspringen (64, ZEIT): good points (two per obstacle cleared, one off per refusal), the
/// most first, then the time (Springreglement 4.3.4; the published table has the time only:
/// S26).
const ZEIT: &str = POINTS_THEN_TIME;
/// Puissance (61, PUIS): the normal round is judged under Wertung A *with* time (Springreglement
/// 4.1.2; the published table has no time there: S27); up to four jump-offs without time.
const PUIS: &str = r#"{"sichtbare_felder": ["strafpunkte_initial_1", "zeit_initial_1", "strafpunkte_stechen_1", "strafpunkte_stechen_2", "strafpunkte_stechen_3", "strafpunkte_stechen_4"],
  "bloecke": [
    {"definierte_felder": ["strafpunkte_stechen_4"], "zeit": [], "strafpunkte": ["strafpunkte_stechen_4"], "resultate": []},
    {"definierte_felder": ["strafpunkte_stechen_3"], "zeit": [], "strafpunkte": ["strafpunkte_stechen_3"], "resultate": []},
    {"definierte_felder": ["strafpunkte_stechen_2"], "zeit": [], "strafpunkte": ["strafpunkte_stechen_2"], "resultate": []},
    {"definierte_felder": ["strafpunkte_stechen_1"], "zeit": [], "strafpunkte": ["strafpunkte_stechen_1"], "resultate": []},
    {"definierte_felder": ["strafpunkte_initial_1", "zeit_initial_1"], "zeit": ["zeit_initial_1"], "strafpunkte": ["strafpunkte_initial_1"], "resultate": []}]}"#;
/// Wertung C with a jump-off under A with time (41, C1AZ): the jump-off's own time breaks its
/// ties (Springreglement 2.3.2; the published table times it with round 1's: S28).
const C1AZ: &str = r#"{"sichtbare_felder": ["zeit_initial_1", "strafpunkte_stechen_1", "zeit_stechen_1"],
  "bloecke": [
    {"definierte_felder": ["zeit_stechen_1", "strafpunkte_stechen_1"], "zeit": ["zeit_stechen_1"], "strafpunkte": ["strafpunkte_stechen_1"], "resultate": []},
    {"definierte_felder": ["zeit_initial_1"], "zeit": ["zeit_initial_1"], "strafpunkte": [], "resultate": []}]}"#;

/// The tables whose published definition contradicts the Springreglement, defined here instead:
/// the decision (docs/deviations.md) and the article followed.
pub fn regulation_override(code: &str) -> Option<(&'static str, &'static str)> {
    match code {
        "ZEIT" => Some(("S26", "4.3.4")),
        "PUIS" => Some(("S27", "4.1.2")),
        "C1AZ" => Some(("S28", "2.3.2")),
        "AZ2RUSAZ2" => Some(("S37", "3.1.4")),
        _ => None,
    }
}

/// Two rounds (reduced) and a winning round under A with time, time of round 2 (24, AZ2RUSAZ2):
/// the winning round adds its own faults to those of rounds 1 and 2 (Springreglement 3.1.4: a
/// jump-off, the normal round's faults carried; the published table leaves its faults out, E20:
/// S37), as table 29 does.
const AZ2RUSAZ2: &str = r#"{"siegerrunde": true,
  "sichtbare_felder": ["zeit_initial_1", "strafpunkte_initial_1", "zeit_initial_2", "strafpunkte_initial_2", "strafpunkte_stechen_1", "zeit_stechen_1"],
  "bloecke": [
    {"definierte_felder": ["strafpunkte_stechen_1", "zeit_stechen_1"], "zeit": ["zeit_stechen_1"], "strafpunkte": ["strafpunkte_initial_1", "strafpunkte_initial_2", "strafpunkte_stechen_1"], "resultate": []},
    {"definierte_felder": ["strafpunkte_initial_1", "strafpunkte_initial_2", "zeit_initial_2"], "zeit": ["zeit_initial_2"], "strafpunkte": ["strafpunkte_initial_1", "strafpunkte_initial_2"], "resultate": []},
    {"definierte_felder": ["strafpunkte_initial_1", "zeit_initial_1"], "zeit": ["zeit_initial_1"], "strafpunkte": ["strafpunkte_initial_1"], "resultate": []}]}"#;

/// A table's definition: as published, or ours when it is published empty (S5) or contradicts
/// the Springreglement (S26–S28, S37, [`regulation_override`]).
pub fn definition_text<'a>(code: &str, definition: &'a str) -> &'a str {
    let empty = definition.trim().is_empty() || definition.trim() == "null";
    match code {
        "STILMIDEAL" if empty => POINTS_THEN_IDEAL_TIME,
        "Gymk" | "Rücke" if empty => POINTS_THEN_TIME,
        "ZEIT" => ZEIT,
        "PUIS" => PUIS,
        "C1AZ" => C1AZ,
        "AZ2RUSAZ2" => AZ2RUSAZ2,
        _ => definition,
    }
}

/// A definition as Swiss Equestrian publishes it, read even with a comma before a closing
/// bracket (table 30, AZAZSA0, has one: docs/deviations.md S5).
pub fn parse_definition(text: &str) -> serde_json::Result<Value> {
    serde_json::from_str(text).or_else(|e| {
        // Drop a comma that only whitespace separates from `]` or `}`, outside strings.
        let mut clean = String::with_capacity(text.len());
        let (mut in_string, mut escaped) = (false, false);
        let chars: Vec<char> = text.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            if in_string {
                escaped = !escaped && c == '\\';
                if c == '"' && !escaped {
                    in_string = false;
                }
            } else if c == '"' {
                in_string = true;
            } else if c == ',' && chars[i + 1..].iter().find(|x| !x.is_whitespace()).is_some_and(|x| *x == ']' || *x == '}') {
                continue;
            }
            clean.push(c);
        }
        serde_json::from_str(&clean).map_err(|_| e)
    })
}

impl ScoringTable {
    /// Read a table from its published definition (`bewertung_json`).
    pub fn from_definition(id: u32, code: &str, name: &str, definition: &str) -> Result<Self, TableError> {
        let definition = definition_text(code, definition);
        if definition.trim().is_empty() || definition.trim() == "null" {
            return Err(TableError::Empty);
        }
        let json: Value = parse_definition(definition).map_err(|e| TableError::InvalidJson(e.to_string()))?;
        let blocks_json = json.get("bloecke").and_then(Value::as_array).ok_or(TableError::Missing("bloecke"))?;

        let mut blocks = Vec::with_capacity(blocks_json.len());
        for b in blocks_json {
            blocks.push(Block {
                required: strings(b, "definierte_felder")?,
                time: strings(b, "zeit")?,
                faults: strings(b, "strafpunkte")?,
                points: b.get("prog").and_then(Value::as_bool).unwrap_or(false),
                ideal_time: b.get("idealzeit").and_then(Value::as_bool).unwrap_or(false),
            });
        }
        if blocks.is_empty() {
            return Err(TableError::Missing("bloecke"));
        }

        let two_phase = json.get("ist_phase").and_then(Value::as_bool).unwrap_or(false);
        let phases = if SPECIAL_TWO_PHASE.contains(&code) {
            Phases::Special
        } else if two_phase || NORMAL_TWO_PHASE.contains(&code) {
            Phases::Normal
        } else {
            Phases::None
        };

        Ok(ScoringTable {
            id,
            code: code.to_string(),
            name: name.to_string(),
            blocks,
            phases,
            winning_round: json.get("siegerrunde").and_then(Value::as_bool).unwrap_or(false),
            knock_out: code == "KNOCK",
            reduced_second_round: code.contains("2RU"),
        })
    }

    /// The table has a jump-off (a block that needs jump-off fields).
    pub fn has_jump_off(&self) -> bool {
        self.blocks.iter().flat_map(|b| &b.required).any(|f| is_jump_off_field(f))
    }
}

pub(crate) fn is_jump_off_field(field: &str) -> bool {
    field.contains("stechen")
}

impl ScoringTable {
    /// What a block is, in words: "round 1", "round 2", "rounds 1 and 2", "phase 2",
    /// "both phases", "jump-off", "jump-off 2", "winning round".
    pub fn block_label(&self, block: usize) -> String {
        let Some(b) = self.blocks.get(block) else { return format!("block {}", block + 1) };
        // Knock-out: duel rounds in riding order, the last block being round 1.
        if self.knock_out {
            return format!("round {}", self.blocks.len() - block);
        }
        let phases = self.phases != Phases::None;
        let has = |part: &str| b.required.iter().any(|f| f.contains(part));
        if let Some(n) = b.required.iter().find_map(|f| f.split("stechen_").nth(1)) {
            if self.winning_round {
                return "winning round".into();
            }
            let jump_offs = self.blocks.iter().filter(|b| b.required.iter().any(|f| is_jump_off_field(f))).count();
            return if jump_offs > 1 { format!("jump-off {n}") } else { "jump-off".into() };
        }
        let (unit, units, both) = if phases { ("phase", "phases", "both phases") } else { ("round", "rounds", "rounds 1 and 2") };
        let _ = units;
        match (has("initial_1"), has("initial_2")) {
            // A single block of a class of two rounds or phases: both count.
            (_, true) if self.blocks.len() == 1 => both.into(),
            (_, true) => format!("{unit} 2"),
            _ => format!("{unit} 1"),
        }
    }

    /// A field in words: "round 1 faults", "jump-off 2 time", "phase 2 points".
    pub fn field_label(&self, field: &str, points: bool) -> String {
        let what = if field.starts_with("zeit") { "time" } else if points { "points" } else { "faults" };
        if self.knock_out {
            if let Some(block) = self.blocks.iter().position(|b| b.required.iter().any(|f| f == field)) {
                return format!("{} result", self.block_label(block));
            }
        }
        let phases = self.phases != Phases::None;
        let part = if let Some(n) = field.split("stechen_").nth(1) {
            if self.winning_round { "winning round".to_string() } else { format!("jump-off {n}") }
        } else if let Some(n) = field.split("initial_").nth(1) {
            format!("{} {n}", if phases { "phase" } else { "round" })
        } else {
            field.to_string()
        };
        format!("{part} {what}")
    }
}

fn strings(block: &Value, key: &'static str) -> Result<Vec<String>, TableError> {
    block
        .get(key)
        .and_then(Value::as_array)
        .ok_or(TableError::Missing(key))?
        .iter()
        .map(|v| v.as_str().map(str::to_string).ok_or(TableError::Missing(key)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const AZ1AZ: &str = r#"{"code":"AZ1AZ","sichtbare_felder":["zeit_stechen_1","strafpunkte_stechen_1","strafpunkte_initial_1","zeit_initial_1"],"bloecke":[{"definierte_felder":["zeit_stechen_1","strafpunkte_stechen_1"],"zeit":["zeit_stechen_1"],"strafpunkte":["strafpunkte_stechen_1"],"resultate":[]},{"definierte_felder":["zeit_initial_1","strafpunkte_initial_1"],"zeit":["zeit_initial_1"],"strafpunkte":["strafpunkte_initial_1"],"resultate":[]}]}"#;

    #[test]
    fn reads_a_published_definition() {
        let t = ScoringTable::from_definition(17, "AZ1AZ", "A mit Stechen", AZ1AZ).unwrap();
        assert_eq!(t.blocks.len(), 2);
        assert_eq!(t.blocks[0].faults, vec!["strafpunkte_stechen_1"]);
        assert!(t.has_jump_off());
        assert_eq!(t.phases, Phases::None);
    }

    #[test]
    fn rejects_broken_definitions_with_the_reason() {
        assert_eq!(ScoringTable::from_definition(999, "NEU", "x", ""), Err(TableError::Empty));
        // Tables published empty that are defined here (S5).
        for code in ["Gymk", "Rücke", "STILMIDEAL"] {
            assert!(ScoringTable::from_definition(1, code, "x", "").unwrap().blocks[0].points, "{code}");
        }
        assert!(matches!(ScoringTable::from_definition(30, "AZAZSA0", "x", r#"{"bloecke":[1,],"#), Err(TableError::InvalidJson(_))));
        // A comma before a closing bracket is read (table 30), not one inside a string.
        assert_eq!(parse_definition(r#"{"a": ["x,]", "y",], }"#).unwrap(), serde_json::json!({ "a": ["x,]", "y"] }));
    }
}
