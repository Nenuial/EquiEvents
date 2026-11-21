//! Every print and export in Italian: no French text left without its Italian
//! (`office::italian`). `EQUIEVENTS_MISSING_IT=<file>` writes the missing ones (French, German).

use office::prints::{self, Language};
use serde_json::json;
use show::model::*;

mod common;

#[test]
fn every_print_in_italian() {
    let it = Language::It;
    for corpus in ["2026.8352", "2025.8161"] {
        let dir = tempfile::tempdir().unwrap();
        let office = office::Office::new(&dir.path().join("library"), "test").unwrap();
        let file = common::rosson_file(dir.path(), corpus);
        let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
        let show = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
        let state = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap().state().clone();
        let mut printed = Vec::new();
        let classes: Vec<ClassId> = state.classes.keys().cloned().collect();
        for kind in [prints::StartListKind::Full { pedigree: true }, prints::StartListKind::Lite, prints::StartListKind::Judges] {
            printed.extend(prints::start_lists_as(&state, &classes, it, kind, "now").ok());
        }
        printed.extend(prints::rankings(&state, &classes, it, "now").ok());
        for c in &classes {
            printed.extend(prints::judge_sheets(&state, c, None, true, it, "now").ok());
            printed.extend(prints::start_cards(&state, c, it, "now").ok());
            printed.extend(prints::team_cards(&state, c, it, "now").ok());
            printed.extend(prints::style_sheets(&state, c, it, "now").ok());
            printed.extend(prints::labels(&state, &prints::Labels::Judges(c.clone()), prints::Sheet::of("3475"), 0, it, "now").ok());
            printed.extend(prints::labels(&state, &prints::Labels::Prizes(c.clone()), prints::Sheet::of("3475"), 0, it, "now").ok());
            let _ = office::exports::start_list(&state, c, it);
            let _ = office::exports::ranking(&state, c, it);
        }
        for id in state.combined.keys() {
            printed.extend(prints::combined(&state, id, it, "now").ok());
        }
        printed.push(prints::participants(&state, it, "now"));
        printed.push(prints::schedule(&state, it, "now"));
        printed.push(prints::officials_plan(&state, it, "now"));
        printed.push(prints::entries_detail(&state, None, it, "now"));
        printed.push(prints::letters(&state, "Gentile cavaliere", false, "Lugano, 1 ottobre 2026", it, "now"));
        printed.extend(prints::labels(&state, &prints::Labels::Addresses, prints::Sheet::of("3475"), 0, it, "now").ok());
        let _ = office::exports::participants(&state, it);
        let _ = office::exports::entries(&state, it);
        // The templates take Italian too.
        for p in &printed {
            typeset::text(p.template, &p.data).unwrap_or_else(|e| panic!("{}: {e}", p.name));
        }
        assert!(printed.len() > 20, "{corpus}: {} prints", printed.len());
    }
    let missing = office::italian::missing();
    if let Ok(path) = std::env::var("EQUIEVENTS_MISSING_IT") {
        std::fs::write(path, serde_json::to_string_pretty(&missing).unwrap()).unwrap();
    }
    assert!(missing.is_empty(), "{} French texts without Italian, first: {:?}", missing.len(), &missing[..missing.len().min(10)]);
}
