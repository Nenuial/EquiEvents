//! The interface entirely in French, German and Italian (the user, 2026-11-15): every refusal the
//! core can give the office (an `Err`, a change the show ignores, a ranking it cannot make…) has
//! its French, German and Italian in the app's catalogue (`app/src/refusals.json`, used by
//! `app/src/refusals.ts`). Found in the sources of the crates the office calls; a message made
//! where no pattern sees it is listed here by hand.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use regex::Regex;

/// Messages made where the patterns below do not see them (a closure with a block, a lock's
/// reason), as the catalogue writes them (values as `{}`).
const BY_HAND: &[&str] = &[
    "class {}: {} has marks on the judges' sheets; delete those results first",
    "team classes are divided by teams",
    "eventing classes are not divided",
    // Why a show of equievents.ch cannot send now (`Office::send_online`).
    "equievents.ch does not answer: sending to Swiss Equestrian waits for the connection",
    "sign in to equievents.ch to send to Swiss Equestrian",
    "you no longer work on this show on equievents.ch",
    "this computer's clock runs ahead: some changes wait; set the computer's time, then send",
    "the show has no event's key on equievents.ch",
    "the server could not do it",
    // Why a download through equievents.ch failed (`Office::not_downloaded`).
    "equievents.ch does not answer; try again once the connection is back",
    "sign in to equievents.ch first",
    "Swiss Equestrian does not know the show's key: it only accepts the key of an event that is not over yet",
    "too many downloads from Swiss Equestrian for this account; try again in an hour",
];

/// A literal turned into a message: its values (`{…}`) as `{}`.
fn pattern(literal: &str) -> String {
    let text = literal.replace("\\\"", "\"").replace("\\'", "'").replace("\\n", " ");
    let text = text.replace("{{", "\u{1}").replace("}}", "\u{2}");
    let text = Regex::new(r"\{[^}]*\}").unwrap().replace_all(&text, "{}").to_string();
    text.replace('\u{1}', "{").replace('\u{2}', "}")
}

/// The messages of a crate's sources (its tests left out), with where each is; `only`: those
/// files alone (equievents.ch: what its API answers, not its administrator's commands).
fn messages(root: &Path, krate: &str, only: &[&str], found: &mut BTreeMap<String, String>) {
    let heads = [
        r"Err\(\s*format!\(\s*",
        r"Err\(\s*",
        r"ok_or\(\s*",
        r"ok_or_else\(\s*\|\|\s*format!\(\s*",
        r"ok_or_else\(\s*\|\|\s*",
        r"Ignored\(\s*format!\(\s*",
        r"Ignored\(\s*",
        r"map_err\(\s*\|\w*\|\s*format!\(\s*",
        r"map_err\(\s*\|\w*\|\s*",
        r"\bbad\(\s*format!\(\s*",
        r"\bbad\(\s*",
        r"\brefuse\(\s*format!\(\s*",
        r"\brefuse\(\s*",
        r"NotCovered\(\s*format!\(\s*",
        r"NotCovered\(\s*",
        r"let (?:bad|refused|why|error) = \|\|\s*\{?[^;]*?format!\(\s*",
        // An error's own words (`impl Display`).
        r"write!\(\s*f,\s*",
        r"f\.write_str\(\s*",
    ];
    let re = Regex::new(&format!(r#"(?:{})"((?:[^"\\]|\\.)*)""#, heads.join("|"))).unwrap();
    let words = Regex::new(r"[A-Za-z]{2}").unwrap();
    let mut stack = vec![root.join("crates").join(krate).join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            if !only.is_empty() && !only.iter().any(|f| path.file_name().and_then(|n| n.to_str()) == Some(f)) {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let body = text.split("#[cfg(test)]").next().unwrap_or_default();
            for m in re.captures_iter(body) {
                let message = pattern(&m[1]);
                if !words.is_match(&message.replace("{}", "")) {
                    continue;
                }
                let line = body[..m.get(0).unwrap().start()].matches('\n').count() + 1;
                found.entry(message).or_insert_with(|| format!("{}:{line}", path.strip_prefix(root).unwrap().display()));
            }
        }
    }
}

#[test]
fn every_refusal_speaks_the_offices_languages() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let catalogue: BTreeMap<String, BTreeMap<String, String>> = serde_json::from_str(&std::fs::read_to_string(root.join("app/src/refusals.json")).unwrap()).unwrap();
    let mut found = BTreeMap::new();
    for krate in ["office", "show", "rosson-import", "se-api"] {
        messages(&root, krate, &[], &mut found);
    }
    messages(&root, "server", &["api.rs", "shows.rs", "auth.rs"], &mut found);
    for m in BY_HAND {
        found.insert((*m).to_string(), "listed by hand".into());
    }
    assert!(found.len() > 200, "{} messages: the patterns no longer find them", found.len());

    let missing: Vec<String> = found.iter().filter(|(m, _)| !catalogue.contains_key(*m)).map(|(m, at)| format!("{m:?} ({at})")).collect();
    assert!(missing.is_empty(), "not in app/src/refusals.json:\n{}", missing.join("\n"));

    // Each in the three languages, with its values ({1}, {2}…) and nothing left of the English's.
    let holes = Regex::new(r"\{(\d+)\}").unwrap();
    let mut wrong = Vec::new();
    for (english, texts) in &catalogue {
        let count = english.matches("{}").count();
        for language in ["fr", "de", "it"] {
            let text = texts.get(language).map(String::as_str).unwrap_or_default();
            let used: BTreeSet<usize> = holes.captures_iter(text).map(|c| c[1].parse().unwrap()).collect();
            if text.trim().is_empty() || text.contains("{}") || used.iter().any(|n| *n == 0 || *n > count) {
                wrong.push(format!("{english:?} in {language}: {text:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "wrong translations:\n{}", wrong.join("\n"));
}

/// What the office sees after a change (« Fait : … », and the undo it offers) is the change's
/// name in its language (`change_<name>` in app/src/i18n.tsx and i18n-it.ts), never the core's
/// command name.
#[test]
fn every_change_has_a_name() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source: String = ["crates/office/src/lib.rs", "crates/office/src/sync.rs"].iter().map(|f| std::fs::read_to_string(root.join(f)).unwrap()).collect();
    let body = source.split("#[cfg(test)]").next().unwrap_or_default().to_string();
    let mut names: BTreeSet<String> = Regex::new(r#"self\.change\([^;]*?,\s*"([a-z_]+)",\s*\|"#).unwrap().captures_iter(&body).map(|c| c[1].to_string()).collect();
    names.extend(Regex::new(r#"=> "([a-z_]+_sheet)""#).unwrap().captures_iter(&body).map(|c| c[1].to_string()));
    assert!(names.len() > 40, "{names:?}");
    let i18n = std::fs::read_to_string(root.join("app/src/i18n.tsx")).unwrap();
    let (fr, de) = i18n.split_once("const de: typeof fr").unwrap();
    let it = std::fs::read_to_string(root.join("app/src/i18n-it.ts")).unwrap();
    for (language, dictionary) in [("fr", fr), ("de", de), ("it", it.as_str())] {
        let missing: Vec<&String> = names.iter().filter(|n| !dictionary.contains(&format!("\n  change_{n}: '"))).collect();
        assert!(missing.is_empty(), "{language}: no change_… for {missing:?}");
    }
}
