//! Comparing our requests with Rosson's, field by field (shared by the contract tests).
#![allow(dead_code)]

use std::collections::BTreeMap;

use serde_json::Value;

/// Fields compared by value only (Rosson sends text or numbers depending on how they were entered).
pub fn loose(key: &str) -> bool {
    key.starts_with("punkte_") && !matches!(key, "punkte_total" | "punkte_prozent") || key.starts_with("zeit_") || key == "aufgabegrund_id"
}

/// Classes whose ranking differs from Rosson 3.3.15's by a decided rule (docs/deviations.md);
/// the golden tests check our ranking there. Any other ranking difference is a failure.
pub const DECIDED_RANKINGS: &[(&str, &str)] = &[
    ("2025.8118 P7", "S16 round 2"),
    ("2025.8158 P1", "S11 tie"),
    ("2025.8161 P5", "S2 special two-phase"),
    ("2025.8161 P6", "S2 special two-phase"),
    ("2025.8161 P7", "S1 points, highest first"),
    ("2025.8161 P8", "S15 ghost team"),
    ("2025.8161 P9b", "S16 round 2"),
    ("2025.8161 P9c", "S16 round 2"),
    ("2026.8352 P5", "S2 special two-phase"),
    ("2026.8352 P6", "S1 points, highest first"),
    ("2026.8352 P7", "S7 team out"),
    ("2026.8352 P8", "S16 round 2"),
    ("2026.8352 P11", "S16 round 2"),
];

/// Fields that follow from the ranking (golden tests).
pub fn ranking_field(key: &str) -> bool {
    matches!(key, "rang" | "klassiert" | "anzahl_klassierte_reiter")
}

/// Rosson's per-judge percentage, `((points / max) * 100).toFixed(2)` in JavaScript: the
/// double's exact value rounded half up (toFixed takes the larger on an exact tie).
pub fn rosson_percent(points: f64, max: f64) -> String {
    let x = points / max * 100.0;
    let exact = rust_decimal::Decimal::from_f64_retain(x).unwrap();
    format!("{:.2}", exact.round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero))
}

/// Ours exact, Rosson's the floating-point slip, for the same judge's points.
pub fn float_slip_fixed(theirs: &Value, ours: &Value) -> bool {
    let (Some(points), Some(t), Some(o)) = (ours["punkte"].as_f64(), theirs["prozent"].as_str(), ours["prozent"].as_str()) else { return false };
    let Ok(o_value) = o.parse::<f64>() else { return false };
    // The judge's maximum: a whole number, points / percentage × 100.
    let max = (points / o_value * 100.0).round();
    if !max.is_finite() || max <= 0.0 {
        return false;
    }
    let exact = rust_decimal::Decimal::from_f64_retain(points).unwrap() / rust_decimal::Decimal::from_f64_retain(max).unwrap() * rust_decimal::Decimal::ONE_HUNDRED;
    t == rosson_percent(points, max) && o == format!("{:.2}", scoring::number::round(exact, 2)) && t != o
}

pub fn as_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// How a value is written: "text", "number", "\"\"", "none".
pub fn form(v: Option<&Value>) -> String {
    match v {
        None => "no field".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::String(s)) if s.is_empty() => "\"\"".into(),
        Some(Value::String(_)) => "text".into(),
        Some(Value::Number(n)) => format!("number {n}"),
        Some(other) => other.to_string(),
    }
}

/// "Nothing", as Rosson writes it in several ways.
pub fn nothing(key: &str, v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) if s.is_empty() => loose(key),
        Some(v) if key == "aufgabegrund_id" => as_number(v) == Some(0.0),
        _ => false,
    }
}

/// Differences between two JSON values, as "path: Rosson … ours …".
pub fn diff(path: &str, theirs: &Value, ours: &Value, out: &mut Vec<(String, String)>, counts: &mut Counts) {
    let key = path.rsplit('.').next().unwrap_or("");
    if key == "updated_at" {
        if as_number(theirs).is_none() || as_number(ours).is_none() {
            out.push((path.to_string(), format!("Rosson {theirs}, ours {ours}: not a time")));
        }
        return;
    }
    match (theirs, ours) {
        (Value::Object(a), Value::Object(b)) => {
            for k in a.keys().chain(b.keys()).collect::<std::collections::BTreeSet<_>>() {
                match (a.get(k), b.get(k)) {
                    // E34: "nothing" is left out; Rosson also sends null.
                    (None | Some(Value::Null), None | Some(Value::Null)) => {
                        if a.get(k) != b.get(k) {
                            counts.loose_types += 1;
                            *counts.forms.entry((k.clone(), form(a.get(k)), form(b.get(k)))).or_default() += 1;
                        }
                    }
                    (x, y) if loose(k) && nothing(k, x) && nothing(k, y) => {
                        if x != y {
                            counts.loose_types += 1;
                            *counts.forms.entry((k.clone(), form(x), form(y))).or_default() += 1;
                        }
                    }
                    (Some(x), Some(y)) => diff(&format!("{path}.{k}"), x, y, out, counts),
                    (x, y) => out.push((format!("{path}.{k}"), format!("Rosson {x:?}, ours {y:?}"))),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if key == "pro_richter" => {
            // Judges in any order, matched by position letter.
            let sorted = |v: &Vec<Value>| {
                let mut v = v.clone();
                v.sort_by_key(|j| (j["buchstabe"].to_string(), j["person_id"].to_string()));
                v
            };
            let (theirs_sorted, mut ours_sorted) = (sorted(a), sorted(b));
            for (t, o) in theirs_sorted.iter().zip(ours_sorted.iter_mut()) {
                if float_slip_fixed(t, o) {
                    counts.float_fixed += 1;
                    o["prozent"] = t["prozent"].clone();
                }
            }
            diff(&format!("{path}(sorted)"), &Value::Array(theirs_sorted), &Value::Array(ours_sorted), out, counts);
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                diff(&format!("{path}[{i}]"), x, y, out, counts);
            }
        }
        (Value::Number(a), Value::Number(b)) if a.as_f64() == b.as_f64() => {}
        (a, b) if a == b => {}
        (a, b) if loose(key) && as_number(a).is_some() && as_number(a) == as_number(b) => {
            counts.loose_types += 1;
            *counts.forms.entry((key.to_string(), form(Some(a)), form(Some(b)))).or_default() += 1;
        }
        (a, b) => out.push((path.to_string(), format!("Rosson {a}, ours {b}"))),
    }
}

#[derive(Default)]
pub struct Counts {
    /// Same value, other form (text or number, the forms of "nothing").
    pub loose_types: usize,
    /// (field, Rosson's form, ours) → how often.
    pub forms: BTreeMap<(String, String, String), usize>,
    /// Per-judge percentages Rosson rounded wrongly in floating point, exact in ours.
    pub float_fixed: usize,
}


/// Runs a future whose parts are all ready at once (fake servers in tests).
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    fn raw() -> RawWaker {
        fn clone(_: *const ()) -> RawWaker {
            raw()
        }
        fn noop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    let waker = unsafe { Waker::from_raw(raw()) };
    let mut f = std::pin::pin!(f);
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut Context::from_waker(&waker)) {
            return v;
        }
    }
}
