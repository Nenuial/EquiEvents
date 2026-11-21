//! Swiss Equestrian's registry on this computer: the riders and horses with a valid status, as
//! `getPersonen` and `getPferde` deliver them (Rosson's `alle_personen` and `pferde`), with the
//! event's access key. A rider or a horse the office puts on an entry comes from the show or from
//! here, never typed in: the office does not make up or change riders and horses (P. Burkhard,
//! 2026-11-14).
//!
//! Only these official lists, never the public directory behind info.swiss-equestrian.ch (the
//! user, 2026-11-15): it also has riders whose licence is no longer active, and an office must
//! only replace a rider with someone whose licence is.
//!
//! Downloaded by equievents.ch (never by the app: only the server talks to Swiss Equestrian) with
//! the key of one of the account's shows, and kept there for the account (the user,
//! 2026-11-21); this computer keeps the signed-in account's copy in `<library>/registry/`
//! (`persons.json`, `horses.json`, `about.json`), for every show, searched offline. The persons
//! carry contact data: the copy goes when the account signs out, or another signs in.
//!
//! Searching, as Rosson: words of three letters or more, each the start of the id, a name or the
//! town (persons), of the id, the name or a word of it (horses); accents and case do not count.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// When each list arrived, and how many.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct About {
    pub persons: usize,
    pub horses: usize,
    /// Milliseconds since 1970.
    pub persons_at: Option<u64>,
    pub horses_at: Option<u64>,
    /// From the simulation (tests: the library's own riders and horses), not Swiss Equestrian.
    #[serde(default)]
    pub simulated: bool,
    /// Whose copy (`<server>#<account id>`), when it came from equievents.ch.
    #[serde(default)]
    pub account: Option<String>,
}

struct Loaded {
    persons: Vec<Value>,
    horses: Vec<Value>,
    person_terms: Vec<Vec<String>>,
    horse_terms: Vec<Vec<String>>,
    person_at: HashMap<u64, usize>,
    horse_at: HashMap<u64, usize>,
}

/// The registry of a library: read from disk the first time it is searched.
pub struct Registry {
    dir: PathBuf,
    loaded: Mutex<Option<Arc<Loaded>>>,
}

/// Lower case, without accents: what is compared.
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            c => c,
        })
        .collect()
}

fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| c.is_whitespace() || c == '-' || c == ',' || c == '/').filter(|w| !w.is_empty()).map(fold)
}

fn id_text(v: &Value) -> String {
    se_api::load::number(&v["id"]).map(|n| n.to_string()).unwrap_or_default()
}

fn person_terms(p: &Value) -> Vec<String> {
    let mut terms = vec![id_text(p)];
    for k in ["nachname", "vorname", "ort"] {
        terms.extend(words(p[k].as_str().unwrap_or("")));
    }
    terms
}

fn horse_terms(h: &Value) -> Vec<String> {
    let name = h["name"].as_str().unwrap_or("");
    let mut terms = vec![id_text(h), fold(name)];
    terms.extend(words(name));
    terms
}

/// The words searched for (three letters or more; Rosson's rule).
pub fn query_words(query: &str) -> Vec<String> {
    words(query).filter(|w| w.chars().count() >= 3).collect()
}

/// Whether every word starts one of the terms.
pub fn matches(terms: &[String], wanted: &[String]) -> bool {
    !wanted.is_empty() && wanted.iter().all(|w| terms.iter().any(|t| t.starts_with(w.as_str())))
}

/// A horse Swiss Equestrian lets start in `year`: its fee paid for that sport year (Rosson's
/// `pferdHinzufuegbar`; records without the list: yes).
pub fn paid_for(horse: &Value, year: Option<u32>) -> bool {
    match (horse["bezahlte_sportjahre"].as_array(), year) {
        (Some(years), Some(year)) => years.iter().any(|y| se_api::load::number(y) == Some(year as u64)),
        _ => true,
    }
}

impl Registry {
    pub fn new(library: &Path) -> Registry {
        Registry { dir: library.join("registry"), loaded: Mutex::new(None) }
    }

    pub fn about(&self) -> About {
        std::fs::read(self.dir.join("about.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    /// Forget this computer's copy (an account signed out, another signed in).
    pub fn clear(&self) -> Result<(), String> {
        if self.dir.exists() {
            std::fs::remove_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        }
        *self.loaded.lock().unwrap_or_else(|e| e.into_inner()) = None;
        Ok(())
    }

    /// Keep what arrived: the persons, the horses (either may be missing: the other stays), of
    /// `account` (`None`: the simulation's, in tests).
    pub fn store(&self, persons: Option<Vec<Value>>, horses: Option<Vec<Value>>, millis: u64, simulated: bool, account: Option<&str>) -> Result<About, String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let mut about = self.about();
        let write = |name: &str, list: &[Value]| {
            let path = self.dir.join(name);
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, serde_json::to_vec(list).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", tmp.display()))?;
            std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
        };
        if let Some(p) = &persons {
            write("persons.json", p)?;
            (about.persons, about.persons_at) = (p.len(), Some(millis));
        }
        if let Some(h) = &horses {
            write("horses.json", h)?;
            (about.horses, about.horses_at) = (h.len(), Some(millis));
        }
        about.simulated = simulated;
        about.account = account.map(str::to_string);
        std::fs::write(self.dir.join("about.json"), serde_json::to_vec_pretty(&about).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        *self.loaded.lock().unwrap_or_else(|e| e.into_inner()) = None;
        Ok(about)
    }

    fn loaded(&self) -> Arc<Loaded> {
        let mut slot = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(l) = slot.as_ref() {
            return Arc::clone(l);
        }
        let read = |name: &str| -> Vec<Value> { std::fs::read(self.dir.join(name)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default() };
        let (persons, horses) = (read("persons.json"), read("horses.json"));
        let index = |list: &[Value]| list.iter().enumerate().filter_map(|(i, v)| Some((se_api::load::number(&v["id"])?, i))).collect();
        let loaded = Arc::new(Loaded {
            person_terms: persons.iter().map(person_terms).collect(),
            horse_terms: horses.iter().map(horse_terms).collect(),
            person_at: index(&persons),
            horse_at: index(&horses),
            persons,
            horses,
        });
        *slot = Some(Arc::clone(&loaded));
        loaded
    }

    /// The persons matching `query`, at most `limit`.
    pub fn find_persons(&self, query: &str, limit: usize) -> Vec<Value> {
        let wanted = query_words(query);
        let l = self.loaded();
        l.persons.iter().zip(&l.person_terms).filter(|(_, t)| matches(t, &wanted)).take(limit).map(|(p, _)| p.clone()).collect()
    }

    /// The horses matching `query`, at most `limit`.
    pub fn find_horses(&self, query: &str, limit: usize) -> Vec<Value> {
        let wanted = query_words(query);
        let l = self.loaded();
        l.horses.iter().zip(&l.horse_terms).filter(|(_, t)| matches(t, &wanted)).take(limit).map(|(h, _)| h.clone()).collect()
    }

    pub fn person(&self, id: u64) -> Option<Value> {
        let l = self.loaded();
        l.person_at.get(&id).map(|&i| l.persons[i].clone())
    }

    pub fn horse(&self, id: u64) -> Option<Value> {
        let l = self.loaded();
        l.horse_at.get(&id).map(|&i| l.horses[i].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn searched_as_rosson_without_accents() {
        let dir = tempfile::tempdir().unwrap();
        let r = Registry::new(dir.path());
        assert_eq!(r.about(), About::default());
        let persons = vec![
            json!({ "id": 123456, "vorname": "Léa", "nachname": "Dupont-Moret", "ort": "Bulle" }),
            json!({ "id": "234567", "vorname": "Hans", "nachname": "Müller", "ort": "Bern" }),
        ];
        let horses = vec![json!({ "id": 7001, "name": "BRUYERE TS", "bezahlte_sportjahre": [2025, 2026] }), json!({ "id": 7002, "name": "NORDIC STAR", "bezahlte_sportjahre": [2025] })];
        let about = r.store(Some(persons), Some(horses), 1_794_000_000_000, false, None).unwrap();
        assert_eq!((about.persons, about.horses, about.persons_at), (2, 2, Some(1_794_000_000_000)));
        assert_eq!(r.about(), about);
        let ids = |found: Vec<Value>| found.iter().map(id_text).collect::<Vec<_>>();
        assert_eq!(ids(r.find_persons("lea dupont", 10)), ["123456"]);
        assert_eq!(ids(r.find_persons("mor", 10)), ["123456"]);
        assert_eq!(ids(r.find_persons("muller bern", 10)), ["234567"]);
        assert_eq!(ids(r.find_persons("2345", 10)), ["234567"]);
        // Shorter words do not count; nothing asked, nothing found.
        assert!(r.find_persons("le", 10).is_empty());
        assert!(r.find_persons("lea zz", 10).len() == 1);
        assert_eq!(ids(r.find_horses("bruy", 10)), ["7001"]);
        assert_eq!(ids(r.find_horses("star", 10)), ["7002"]);
        assert_eq!(r.person(234567).unwrap()["nachname"], "Müller");
        assert!(r.horse(9).is_none());
        // Paid for the show's year.
        assert!(paid_for(&r.horse(7001).unwrap(), Some(2026)) && !paid_for(&r.horse(7002).unwrap(), Some(2026)));
        assert!(paid_for(&json!({ "id": 1 }), Some(2026)));
        // A new list replaces the old; the other stays.
        r.store(None, Some(vec![json!({ "id": 7003, "name": "NEW ONE" })]), 1_794_000_100_000, false, None).unwrap();
        assert_eq!(ids(r.find_horses("new", 10)), ["7003"]);
        assert_eq!(ids(r.find_persons("bern", 10)), ["234567"]);
        assert_eq!(r.about().horses_at, Some(1_794_000_100_000));
    }
}
