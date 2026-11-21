//! Rebuilding what Swiss Equestrian delivered from a Rosson export: the same responses as the
//! fixtures' (golden/src/load-fixtures.mjs), which Rosson and `se_api::load` were checked on.

use std::fs;
use std::path::Path;

use serde_json::Value;

#[test]
fn rebuilt_responses_are_the_fixtures() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let mut dirs: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("api/load.json").is_file()).collect();
    dirs.sort();
    assert!(dirs.len() >= 6);
    for dir in dirs {
        let snapshot: Value = serde_json::from_str(&fs::read_to_string(dir.join("snapshot.json")).unwrap()).unwrap();
        let fixture: Value = serde_json::from_str(&fs::read_to_string(dir.join("api/load.json")).unwrap()).unwrap();
        let ours = rosson_import::delivered::event(&snapshot);
        let show = dir.file_name().unwrap().to_string_lossy().to_string();
        for part in ["personen", "pferde"] {
            assert_eq!(ours["data"][part], fixture["response"]["data"][part], "{show}: {part}");
        }
        for (key, value) in fixture["response"]["data"]["veranstaltung"].as_object().unwrap() {
            assert_eq!(&ours["data"]["veranstaltung"][key], value, "{show}: veranstaltung.{key}");
        }
        assert_eq!(ours, fixture["response"], "{show}");
        let master = rosson_import::delivered::master_data(&snapshot);
        assert!(master["data"]["stammdaten"]["wertungen"].as_array().unwrap().iter().all(|w| w.get("updated_at").is_none()));
    }
}
