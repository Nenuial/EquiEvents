//! Training shows (decided by the user, 2026-11-15), made from the app's settings:
//!
//! - **The practice event** (« Concours d'exercice »): EquiEvents' own, real shows' classes and
//!   entries with every person invented (`training/practice.json`, made by
//!   `examples/practice_event.rs`), its days moved to start the day it is made.
//! - **Swiss Equestrian's demo event** (Rosson's « Manifestation test »): downloaded from Swiss
//!   Equestrian with its demo key (`connection::DEMO_KEYS`), the one real call allowed before the
//!   pilot, and only to download.
//!
//! Both are shows like any other, loaded through the same code as a real event
//! (`se_api::load`), but never synced: no event key, never on equievents.ch, and they send only
//! to the simulation (to practise sending), whatever server is set.

use serde_json::Value;

/// The author of a training show's first operations (how it is known as one).
pub const PRACTICE: &str = "training: EquiEvents' practice event";
pub const DEMO: &str = "training: Swiss Equestrian's demo event";

/// The access key a training show sends to the simulation with.
pub const SIMULATION_KEY: &str = "exercice";

const PRACTICE_DATA: &str = include_str!("../training/practice.json");

/// A show whose first operation has this author is a training show.
pub fn is_training(first_author: Option<&str>) -> bool {
    first_author.is_some_and(|a| a.starts_with("training: "))
}

/// The practice event as Swiss Equestrian's three answers (`getStammdaten`, `getOffizielle`,
/// `getVeranstaltung`), its first day `today` (`YYYY-MM-DD`), named in `language`.
pub fn practice(today: &str, language: &str) -> Result<(Value, Value, Value), String> {
    let mut all: Value = serde_json::from_str(PRACTICE_DATA).map_err(|e| format!("practice event: {e}"))?;
    let event = &mut all["getVeranstaltung"]["data"]["veranstaltung"];
    let shift = days(today)? - days(event["von"].as_str().unwrap_or(""))?;
    for key in ["von", "bis", "nennschluss"] {
        moved(&mut event[key], shift)?;
    }
    for class in event["ausschreibung_pruefungen"].as_array_mut().into_iter().flatten() {
        moved(&mut class["datum"], shift)?;
    }
    let (number, title) = match language {
        "de" => ("ÜBUNG", "ÜBUNGSTURNIER EQUIEVENTS"),
        "it" => ("ESERCIZIO", "CONCORSO DI ESERCITAZIONE EQUIEVENTS"),
        _ => ("EXERCICE", "CONCOURS D'EXERCICE EQUIEVENTS"),
    };
    event["nr"] = Value::from(number);
    event["erste_titelzeile"] = Value::from(title);
    Ok((all["getStammdaten"].take(), all["getOffizielle"].take(), all["getVeranstaltung"].take()))
}

/// A date (`YYYY-MM-DD`, maybe with a time after it) moved by some days; `null` stays.
fn moved(value: &mut Value, shift: i64) -> Result<(), String> {
    let Some(text) = value.as_str() else { return Ok(()) };
    if text.len() < 10 {
        return Ok(());
    }
    *value = Value::from(format!("{}{}", date(days(&text[..10])? + shift), &text[10..]));
    Ok(())
}

/// Days since 1970-01-01 (the proleptic Gregorian calendar).
fn days(date: &str) -> Result<i64, String> {
    let parts: Vec<i64> = date.get(..10).unwrap_or("").split('-').filter_map(|p| p.parse().ok()).collect();
    let [y, m, d] = parts[..] else { return Err(format!("{date} is not a date (YYYY-MM-DD)")) };
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146_097 + doe - 719_468)
}

/// `YYYY-MM-DD` of days since 1970-01-01.
fn date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_move_by_days() {
        for (d, n) in [("1970-01-01", 0), ("2000-02-29", 11_016), ("2026-11-15", 20_772), ("2027-01-01", 20_819)] {
            assert_eq!((days(d).unwrap(), date(n)), (n, d.to_string()));
        }
        let mut v = Value::from("2026-01-02T00:00:00");
        moved(&mut v, 365).unwrap();
        assert_eq!(v, "2027-01-02T00:00:00");
    }
}
