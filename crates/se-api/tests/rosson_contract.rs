//! The API contract against Rosson 3.3.15's own requests (golden/corpus, made by
//! golden/src/api-fixtures.mjs).

use std::fs;
use std::path::Path;

use se_api::{rosson_key, Endpoint};
use serde_json::Value;

fn corpus() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus")
}

#[test]
fn the_date_based_key_is_rossons() {
    let keys: Value = serde_json::from_str(&fs::read_to_string(corpus().join("rosson-key.json")).unwrap()).unwrap();
    let keys = keys.as_object().unwrap();
    assert!(keys.len() >= 5);
    for (day, key) in keys {
        let parts: Vec<u32> = day.split('-').map(|p| p.parse().unwrap()).collect();
        assert_eq!(rosson_key(parts[0], parts[1], parts[2]), key.as_str().unwrap(), "{day}");
    }
}

#[test]
fn every_request_rosson_makes_is_a_known_endpoint() {
    let mut seen = std::collections::BTreeMap::new();
    for show in fs::read_dir(corpus()).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("api").is_dir()) {
        for file in fs::read_dir(show.join("api")).unwrap().map(|e| e.unwrap().path()) {
            let fixture: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
            assert!(fixture.get("rosson_failed").is_none(), "{}: Rosson failed", file.display());
            for r in fixture["requests"].as_array().unwrap() {
                let path = r["path"].as_str().unwrap();
                let endpoint = Endpoint::of_path(path).unwrap_or_else(|| panic!("{}: unknown endpoint {path}", file.display()));
                *seen.entry(format!("{} {endpoint:?}", r["method"].as_str().unwrap())).or_insert(0) += 1;
                // Every request carries the event key; uploads also the date-based key and the version.
                let carries = |k: &str| r["query"].get(k).is_some() || r["body"].get(k).is_some();
                assert!(carries("access_key"), "{}: {path} without access_key", file.display());
                if r["method"] != "GET" {
                    assert!(carries("rosson_key"), "{}: {path} without rosson_key", file.display());
                }
            }
        }
    }
    println!("{seen:#?}");
    assert!(seen.len() >= 5);
}

#[test]
fn the_version_is_read_from_rossons_update_feed() {
    // The feeds as read on 2026-11-13 (tests/fixtures).
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for file in ["rosson-latest.yml", "rosson-latest-mac.yml"] {
        let feed = fs::read_to_string(dir.join(file)).unwrap();
        assert_eq!(se_api::version::from_feed(&feed).as_deref(), Some("3.3.15"), "{file}");
    }
    // Rosson's own requests carry its version.
    let fixture: Value = serde_json::from_str(&fs::read_to_string(corpus().join("2026.8352/api/event.json")).unwrap()).unwrap();
    assert_eq!(fixture["requests"][0]["body"]["rosson_version"], se_api::version::FALLBACK);
}
