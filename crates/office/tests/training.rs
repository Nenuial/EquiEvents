//! Training shows: the practice event (« Concours d'exercice ») is what its maker makes from the
//! corpus, with no name of the real shows in it; made from the settings it starts today, works as
//! any show (a class of each discipline, sending to the simulation), and never goes online, nor
//! to Swiss Equestrian whatever the server set.

use office::Office;
use serde_json::{json, Value};

#[allow(dead_code)]
#[path = "../examples/practice_event.rs"]
mod maker;

#[test]
fn the_practice_event_is_made_of_invented_people() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (practice, originals) = maker::make(&root);
    assert!(originals.len() > 1000);
    assert_eq!(maker::leaks(&practice, &originals), Vec::<String>::new());
    let kept: Value = serde_json::from_str(&std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("training/practice.json")).unwrap()).unwrap();
    assert!(kept == practice, "training/practice.json is not what examples/practice_event.rs makes: run it again");
}

#[test]
fn a_practice_event_from_the_settings() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let show = office.call("create_practice", &json!({ "language": "de" })).unwrap()["show"].as_str().unwrap().to_string();
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };

    // In the library: a training show, from today, in the office's language.
    let library = office.call("library", &json!({})).unwrap();
    let summary = &library[0];
    assert_eq!((summary["training"].as_bool(), summary["rehearsal"].as_bool(), summary["online"].as_bool()), (Some(true), Some(false), Some(false)));
    assert_eq!((summary["event"]["number"].as_str(), summary["event"]["title"][0].as_str()), (Some("ÜBUNG"), Some("ÜBUNGSTURNIER EQUIEVENTS")));
    let today = rosson_import::swiss_time::date(&chrono_free_now()).unwrap();
    assert_eq!(summary["event"]["from"], today);
    assert!(summary["entries"].as_u64().unwrap() > 300);

    // A class of each discipline sets up: jumping, dressage, driving (its phases).
    let view = call("show", json!({})).unwrap();
    let not_set_up = view["not_set_up"].as_array().unwrap().clone();
    for discipline in ["CS", "CD", "CA"] {
        let published = not_set_up.iter().find(|p| p["discipline"] == discipline).unwrap_or_else(|| panic!("no {discipline} class"));
        let class = call("set_up_class", json!({ "published": published["id"] })).unwrap();
        assert!(class["class"].is_string(), "{discipline}: {class}");
    }

    // Never online; sending reaches the simulation only, whatever the app's server.
    assert_eq!(office.call("put_online", &json!({ "show": show })).unwrap()["outcome"], "training");
    let connection = call("connection", json!({})).unwrap();
    assert_eq!((connection["training"].as_bool(), connection["server"].as_str(), connection["has_key"].as_bool()), (Some(true), Some("simulation"), Some(true)));
    let jumping = call("show", json!({})).unwrap()["days"].as_array().unwrap().iter().flat_map(|d| d["classes"].as_array().unwrap().clone()).find(|c| c["discipline"] == "CS").unwrap();
    call("send", json!({ "what": "start_lists", "published": jumping["published"], "language": "de" })).unwrap();
    let received = call("simulation_log", json!({})).unwrap();
    assert!(received.as_array().unwrap().iter().any(|r| r["path"] == "putStartlisten"), "{received}");
}

/// Now, as the office stamps it (RFC 3339, UTC).
fn chrono_free_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // 1970-01-01 + days (Hinnant's civil_from_days).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest / 60 % 60, rest % 60)
}
