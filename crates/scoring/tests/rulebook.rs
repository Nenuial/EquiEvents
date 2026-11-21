//! The rulebook (`docs/rulebook.md`) is what the code says: rebuilt here from the engine and
//! Swiss Equestrian's scoring tables, it must be the committed text. Any change to how a class is
//! ranked therefore shows in the rulebook's diff, where it can be checked against the
//! regulations. `EQUIEVENTS_UPDATE_RULEBOOK=1 cargo test -p scoring --test rulebook` writes it.

use std::path::Path;

use scoring::jumping::table::ScoringTable;
use scoring::rulebook::{rulebook, TableSource};

#[test]
fn the_rulebook_is_what_the_code_says() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("fixtures/scoring-tables.json")).unwrap();
    let sources: Vec<TableSource> = fixture["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            let id = t["id"].as_u64().unwrap() as u32;
            let (code, name) = (t["code"].as_str().unwrap(), t["name"].as_str().unwrap_or(""));
            TableSource { id, code, name, table: ScoringTable::from_definition(id, code, name, t["definition"].as_str().unwrap_or("")) }
        })
        .collect();
    let text = rulebook(&sources);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/rulebook.md");
    if std::env::var("EQUIEVENTS_UPDATE_RULEBOOK").is_ok() {
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == text,
        "docs/rulebook.md is not what the code says: run EQUIEVENTS_UPDATE_RULEBOOK=1 cargo test -p scoring --test rulebook, then review the difference"
    );
}
