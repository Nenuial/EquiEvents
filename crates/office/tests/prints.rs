//! What the office prints, from a show: start lists, rankings, judges' sheets, a ranking over
//! several classes, the participants, the timetable. Checked on what the pages say.

use office::prints::{self, Language};
use office::Office;
use serde_json::{json, Value};

mod common;

struct Show {
    office: Office,
    id: String,
    dir: tempfile::TempDir,
}

impl Show {
    fn rehearsal() -> Show {
        let dir = tempfile::tempdir().unwrap();
        let office = Office::new(&dir.path().join("library"), "test").unwrap();
        let file = common::rosson_file(dir.path(), "2026.8352");
        let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
        let id = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
        Show { office, id, dir }
    }

    fn call(&self, command: &str, mut args: Value) -> Result<Value, String> {
        args["show"] = json!(self.id);
        self.office.call(command, &args)
    }

    fn set_up(&self, number: &str) -> String {
        let show = self.call("show", json!({})).unwrap();
        let published = show["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == number).unwrap()["id"].as_u64().unwrap();
        self.call("set_up_class", json!({ "published": published })).unwrap()["class"].as_str().unwrap().to_string()
    }

    fn starters(&self, class: &str) -> Vec<Value> {
        self.call("results", json!({ "class": class })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["not_starting"] == false).cloned().collect()
    }

    /// The text of a print made by the office.
    fn text(&self, args: Value) -> String {
        let answer = self.call("print", args.clone()).unwrap_or_else(|e| panic!("{args}: {e}"));
        let path = answer["path"].as_str().unwrap();
        assert!(std::fs::read(path).unwrap().starts_with(b"%PDF"));
        // To look at them: EQUIEVENTS_PRINTS=<folder> cargo test -p office --test prints
        if let Ok(dir) = std::env::var("EQUIEVENTS_PRINTS") {
            std::fs::copy(path, std::path::Path::new(&dir).join(answer["name"].as_str().unwrap())).unwrap();
        }
        assert!(path.starts_with(self.dir.path().to_str().unwrap()));
        // The same print's data, laid out again, read back.
        let store = show::Store::open(&self.dir.path().join("library").join(format!("{}.show", self.id))).unwrap();
        let state = store.state();
        let lang = Language::of(args["language"].as_str().unwrap_or("fr"));
        let print = match args["what"].as_str().unwrap() {
            "start_list" => {
                let kind = match args["variant"].as_str() {
                    Some("lite") => prints::StartListKind::Lite,
                    Some("judges") => prints::StartListKind::Judges,
                    _ => prints::StartListKind::Full { pedigree: args["pedigree"] == true },
                };
                prints::start_lists_as(state, &[show::model::ClassId(args["class"].as_str().unwrap().into())], lang, kind, "now").unwrap()
            }
            "ranking" => prints::rankings_with(state, &[show::model::ClassId(args["class"].as_str().unwrap().into())], lang, args["by_judge"] == true, "now").unwrap(),
            "judge_sheets" => prints::judge_sheets(state, &show::model::ClassId(args["class"].as_str().unwrap().into()), None, args["memo"] == true, lang, "now").unwrap(),
            "combined" => prints::combined(state, args["id"].as_str().unwrap(), lang, "now").unwrap(),
            "participants" => prints::participants(state, lang, "now"),
            "schedule" => prints::schedule(state, lang, "now"),
            "entries" => prints::entries_detail(state, args["published"].as_u64().map(show::model::PublishedClassId).as_ref(), lang, "now"),
            _ => unreachable!(),
        };
        typeset::text(print.template, &print.data).unwrap().join("\n")
    }
}

impl Show {
    /// The text of a print made through the office's command.
    fn text_of(&self, mut args: Value) -> String {
        args["inline"] = json!(false);
        let answer = self.call("print", args.clone()).unwrap_or_else(|e| panic!("{args}: {e}"));
        let state = show::Store::open(&self.dir.path().join("library").join(format!("{}.show", self.id))).unwrap().state().clone();
        let kind = match args["kind"].as_str().unwrap() {
            "judges" => prints::Labels::Judges(show::model::ClassId(args["class"].as_str().unwrap().into())),
            "prizes" => prints::Labels::Prizes(show::model::ClassId(args["class"].as_str().unwrap().into())),
            _ => prints::Labels::Addresses,
        };
        assert!(answer["pages"].as_u64().unwrap() >= 1);
        if let Ok(dir) = std::env::var("EQUIEVENTS_PRINTS") {
            std::fs::copy(answer["path"].as_str().unwrap(), std::path::Path::new(&dir).join(answer["name"].as_str().unwrap())).unwrap();
        }
        let print = prints::labels(&state, &kind, prints::Sheet::of(args["sheet"].as_str().unwrap_or("3475")), 0, Language::Fr, "now").unwrap();
        typeset::text(print.template, &print.data).unwrap().join("\n")
    }
}

fn has(text: &str, wanted: &[&str]) {
    for w in wanted {
        assert!(text.contains(w), "{w:?} not in:\n{text}");
    }
}

#[test]
fn start_list_and_ranking_of_a_jumping_class() {
    let show = Show::rehearsal();
    let class = show.set_up("1");
    show.call("number", json!({ "class": class, "first": 1 })).unwrap();
    let riders = show.starters(&class);
    let (a, b) = (&riders[0], &riders[1]);
    show.call("edit_result", json!({ "class": class, "entry": a["entry"], "values": { "strafpunkte_initial_1": "0", "zeit_initial_1": "61.2" } })).unwrap();
    show.call("edit_result", json!({ "class": class, "entry": b["entry"], "values": { "strafpunkte_initial_1": "4", "zeit_initial_1": "58.03" } })).unwrap();

    let list = show.text(json!({ "what": "start_list", "class": class, "language": "fr" }));
    has(&list, &["Liste de départ 1", a["rider"].as_str().unwrap(), a["horse"].as_str().unwrap(), "R/N120", "2026.8352", "PARTANTS"]);
    let with_pedigree = show.text(json!({ "what": "start_list", "class": class, "language": "de", "pedigree": true }));
    // The pedigree on a line of its own: sire, dam, dam's sire; the breeder when listed.
    has(&with_pedigree, &["Startliste 1", "Seite"]);
    let pedigree = regex::Regex::new(r"Vater [A-Z][^·\n]* · Mutter [A-Z][^·\n]* · Muttervater [A-Z]").unwrap();
    assert!(pedigree.is_match(&with_pedigree) && with_pedigree.contains("Züchter "), "no pedigree in:\n{with_pedigree}");

    let ranking = show.text(json!({ "what": "ranking", "class": class, "language": "fr" }));
    has(&ranking, &["Classement 1", a["rider"].as_str().unwrap(), "61.20", "58.03", "PÉN.", "TEMPS", "partants"]);
    // The faster rider with 4 penalties after the clear round.
    assert!(ranking.find(a["rider"].as_str().unwrap()).unwrap() < ranking.find(b["rider"].as_str().unwrap()).unwrap());
}

#[test]
fn judges_sheets_of_a_dressage_class() {
    let show = Show::rehearsal();
    let class = show.set_up("14");
    // Without judges: said so.
    assert!(show.call("print", json!({ "what": "judge_sheets", "class": class, "language": "fr" })).unwrap_err().contains("juge"));
    // Two judges, C and H; the class's programme chosen.
    let programmes = show.call("programmes", json!({})).unwrap();
    let programme = programmes.as_array().unwrap().iter().find(|p| p["name"] == "L12/60").unwrap_or(&programmes[0])["id"].clone();
    show.call("edit_class", json!({ "class": class, "programme": programme })).unwrap();
    let people: Vec<Value> = show.call("find_officials", json!({ "query": "a" })).unwrap().as_array().unwrap().iter().take(2).cloned().collect();
    show.call("set_role", json!({ "class": class, "person": people[0]["person"], "role": "DRI", "letter": "C" })).unwrap();
    show.call("set_role", json!({ "class": class, "person": people[1]["person"], "role": "DRI", "letter": "H" })).unwrap();

    let starters = show.starters(&class).len();
    let answer = show.call("print", json!({ "what": "judge_sheets", "class": class, "language": "fr" })).unwrap();
    assert!(answer["pages"].as_u64().unwrap() as usize >= 2 * starters, "{answer}");
    // Swiss Equestrian's form: its head, the test's tables, the result, the signature.
    let text = show.text(json!({ "what": "judge_sheets", "class": class, "language": "fr" }));
    has(&text, &[people[0]["name"].as_str().unwrap(), "SWISS EQUESTRIAN", "N° de départ", "Juge (position)", "Remarques", "Total intermédiaire", "Résultat en pourcentage", "Signature du juge / de la juge :", "Swiss Equestrian"]);
    // All judges in one document, judge C's sheets first; with a memo sheet for each, marked.
    let memo = show.text(json!({ "what": "judge_sheets", "class": class, "language": "de", "memo": true }));
    has(&memo, &["RICHTER", "Unterschrift des Richters/der Richterin:"]);
    let pages = |args: Value| show.call("print", args).unwrap()["pages"].as_u64().unwrap();
    assert!(pages(json!({ "what": "judge_sheets", "class": class, "memo": true })) > pages(json!({ "what": "judge_sheets", "class": class })));

    // The ranking: each judge's total and percentage; by judge, that judge's rank too (Rosson's
    // "Avec détail par juge"). Three riders, every movement the same mark on a sheet: C gives 7,
    // 6, 6.5 (70, 60, 65 %), H 6, 7.5, 6.5 (60, 75, 65 %).
    let sheets = show.call("sheets", json!({ "class": class })).unwrap();
    let judge = |letter: &str| sheets["judges"].as_array().unwrap().iter().find(|j| j["letter"] == letter).unwrap()["assignment"].clone();
    let movements: Vec<String> = sheets["programmes"].as_object().unwrap().values().next().unwrap()["sections"].as_array().unwrap().iter().flat_map(|s| s["movements"].as_array().unwrap().clone()).map(|m| m["number"].as_str().unwrap().to_string()).collect();
    let riders: Vec<Value> = show.starters(&class).iter().take(3).map(|l| l["entry"].clone()).collect();
    for (rider, (c, h)) in riders.iter().zip([("7", "6"), ("6", "7.5"), ("6.5", "6.5")]) {
        for (letter, mark) in [("C", c), ("H", h)] {
            let marks: serde_json::Map<String, Value> = movements.iter().map(|m| (m.clone(), json!(mark))).collect();
            show.call("edit_sheet", json!({ "class": class, "entry": rider, "judge": judge(letter), "marks": marks })).unwrap();
        }
    }
    let plain = show.text(json!({ "what": "ranking", "class": class, "language": "fr" }));
    has(&plain, &["Classement", "70.00 %", "75.00 %"]);
    assert!(!plain.contains("% ("), "{plain}");
    let by_judge = show.text(json!({ "what": "ranking", "class": class, "language": "fr", "by_judge": true }));
    has(&by_judge, &["70.00 % (1)", "60.00 % (3)", "75.00 % (1)", "60.00 % (3)", "65.00 % (2)"]);
    assert_eq!(show.call("print", json!({ "what": "ranking", "class": class, "by_judge": true })).unwrap()["name"], format!("Classement par juge {}.pdf", 14));
}

#[test]
fn the_show_prints() {
    let show = Show::rehearsal();
    let (one, two) = (show.set_up("1"), show.set_up("2"));
    show.call("edit_class", json!({ "class": one, "time": "08:00" })).unwrap();
    let schedule = show.text(json!({ "what": "schedule", "language": "de" }));
    has(&schedule, &["Zeitplan", "Freitag, 25. September 2026", "08:00", "R/N120"]);

    let participants = show.text(json!({ "what": "participants", "language": "fr" }));
    let first = &show.starters(&one)[0];
    has(&participants, &["Participants", first["rider"].as_str().unwrap(), first["horse"].as_str().unwrap()]);

    let id = show.call("set_combined", json!({ "name": "Championnat R", "classes": [one, two], "method": "penalties" })).unwrap()["id"].as_str().unwrap().to_string();
    let combined = show.text(json!({ "what": "combined", "id": id, "language": "fr" }));
    has(&combined, &["Championnat R", "Pénalités additionnées"]);

    assert!(show.call("print", json!({ "what": "nonsense" })).is_err());
    // An answer for a browser carries the PDF.
    let inline = show.call("print", json!({ "what": "schedule", "inline": true })).unwrap();
    assert!(inline["pdf"].as_str().unwrap().starts_with("JVBERi")); // "%PDF"
}

#[test]
fn labels_on_sheets() {
    let show = Show::rehearsal();
    let store = || show::Store::open(&show.dir.path().join("library").join(format!("{}.show", show.id))).unwrap();
    // Every rider's address, 24 to a 3475 sheet; 5 places skipped.
    let state = store().state().clone();
    let print = prints::labels(&state, &prints::Labels::Addresses, prints::Sheet::of("3475"), 5, Language::Fr, "now").unwrap();
    let n = print.data["labels"].as_array().unwrap().len();
    assert!(n > 200, "{n}");
    let pdf = typeset::render(print.template, &print.data).unwrap();
    assert_eq!(pdf.pages, (n + 5).div_ceil(24));
    // The name in bold (after the salutation, when there is one).
    assert!(print.data["labels"][0]["lines"].as_array().unwrap().iter().any(|l| l["bold"] == true));

    // A sheet of the office's own (Rosson's custom templates): 4 × 10 with gaps, centred; kept
    // with the library; one too large for A4 refused.
    let own = json!({ "name": " Herma 4×10 ", "cols": 4, "rows": 10, "width_mm": 48.5, "height_mm": 25.4, "top_mm": 13.5, "left_mm": 8, "gap_x_mm": 0, "gap_y_mm": 2, "padding_mm": 2, "font_pt": 8, "align": "center" });
    let saved = show.office.call("save_label_sheet", &json!({ "sheet": own })).unwrap();
    let id = saved["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("own-") && saved["name"] == "Herma 4×10");
    let sheets = show.office.call("label_sheets", &json!({})).unwrap();
    assert_eq!(sheets.as_array().unwrap().iter().map(|s| (s["sheet"]["id"].as_str().unwrap(), s["builtin"].as_bool().unwrap())).collect::<Vec<_>>(), [("3475", true), ("3484", true), (id.as_str(), false)]);
    let answer = show.call("print", json!({ "what": "labels", "kind": "addresses", "sheet": id })).unwrap();
    assert_eq!(answer["pages"].as_u64().unwrap() as usize, n.div_ceil(40));
    let mut too_big = own.clone();
    too_big["rows"] = json!(12);
    assert!(show.office.call("save_label_sheet", &json!({ "sheet": too_big })).unwrap_err().contains("A4"));
    show.office.call("remove_label_sheet", &json!({ "id": id })).unwrap();
    assert_eq!(show.office.call("label_sheets", &json!({})).unwrap().as_array().unwrap().len(), 2);

    // For the dressage judges: one per judge and rider who starts.
    let class = show.set_up("14");
    let people: Vec<Value> = show.call("find_officials", json!({ "query": "a" })).unwrap().as_array().unwrap().iter().take(2).cloned().collect();
    show.call("set_role", json!({ "class": class, "person": people[0]["person"], "role": "DRI", "letter": "C" })).unwrap();
    show.call("set_role", json!({ "class": class, "person": people[1]["person"], "role": "DRI", "letter": "M" })).unwrap();
    let starters = show.starters(&class).len();
    let text = show.text_of(json!({ "what": "labels", "kind": "judges", "class": class, "sheet": "3484" }));
    assert_eq!(text.matches("Juge C").count(), starters);
    assert_eq!(text.matches("Juge M").count(), starters);

    // Prizes: one per rank.
    let jumping = show.set_up("1");
    assert!(show.call("print", json!({ "what": "labels", "kind": "prizes", "class": jumping })).unwrap_err().contains("prix"));
    let published = show.call("results", json!({ "class": jumping })).unwrap()["class"]["published"].as_u64().unwrap();
    show.call("set_prizes", json!({ "published": published, "prizes": { "minimum": "0", "amounts": ["200", "160"], "natural": ["", "Couverture"] } })).unwrap();
    let text = show.text_of(json!({ "what": "labels", "kind": "prizes", "class": jumping }));
    has(&text, &["1er rang", "CHF 200.–", "2e rang", "Couverture"]);
}

#[test]
fn excel_exports() {
    use calamine::{open_workbook_auto, Reader};
    let show = Show::rehearsal();
    let class = show.set_up("1");
    let rows = |what: &str, extra: Value| -> Vec<Vec<String>> {
        let path = show.dir.path().join(format!("{what}.xlsx"));
        let mut args = json!({ "what": what, "path": path, "language": "fr" });
        for (k, v) in extra.as_object().unwrap() {
            args[k] = v.clone();
        }
        show.call("export_excel", args).unwrap();
        let mut book = open_workbook_auto(&path).unwrap();
        let sheet = book.sheet_names()[0].clone();
        book.worksheet_range(&sheet).unwrap().rows().map(|r| r.iter().map(|c| c.to_string()).collect()).collect()
    };
    let participants = rows("participants", json!({}));
    assert_eq!(participants[0][0], "Licence");
    assert!(participants.len() > 200);
    let entries = rows("entries", json!({}));
    assert_eq!(entries.len(), 1 + show.call("participants", json!({})).unwrap().as_array().unwrap().len());
    let list = rows("start_list", json!({ "class": class }));
    assert_eq!(list.len(), 1 + show.starters(&class).len() + show.call("start_list", json!({ "class": class })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter" && l["not_starting"] == true).count());
    let ranking = rows("ranking", json!({ "class": class }));
    assert_eq!(ranking[0][0], "Rang");
}

#[test]
fn letters_to_the_riders() {
    let show = Show::rehearsal();
    show.set_up("1");
    let answer = show.call("print", json!({ "what": "letters", "text": "Bienvenue au Chalet-à-Gobet !", "window": "right", "language": "fr" })).unwrap();
    let state = show::Store::open(&show.dir.path().join("library").join(format!("{}.show", show.id))).unwrap().state().clone();
    let print = prints::letters(&state, "Bienvenue au Chalet-à-Gobet !", true, "Le Chalet-à-Gobet, 13 novembre 2026", Language::Fr, "now");
    let letters = print.data["letters"].as_array().unwrap();
    // One page per rider.
    assert_eq!(answer["pages"].as_u64().unwrap() as usize, letters.len());
    // A rider with several entries: all of them, and the total of their fees.
    let several = letters.iter().find(|l| l["entries"].as_array().unwrap().len() > 2).unwrap();
    let sum: f64 = several["entries"].as_array().unwrap().iter().map(|e| e["fee"].as_str().unwrap().parse::<f64>().unwrap()).sum();
    assert_eq!(several["total"].as_str().unwrap().parse::<f64>().unwrap(), sum);
    let text = typeset::text(print.template, &print.data).unwrap();
    has(&text[0], &["Bienvenue au Chalet-à-Gobet !", "Total CHF", "FINANCE"]);
}

#[test]
fn start_cards_and_style_sheets() {
    let show = Show::rehearsal();
    let class = show.set_up("6");
    let starters = show.starters(&class).len();
    let state = || show::Store::open(&show.dir.path().join("library").join(format!("{}.show", show.id))).unwrap().state().clone();
    // Two cards to a page.
    let answer = show.call("print", json!({ "what": "start_cards", "class": class, "language": "fr" })).unwrap();
    assert_eq!(answer["pages"].as_u64().unwrap() as usize, starters.div_ceil(2));
    let print = prints::start_cards(&state(), &show::model::ClassId(class.clone()), Language::Fr, "now").unwrap();
    let text = typeset::text(print.template, &print.data).unwrap().join("\n");
    has(&text, &["LICENCE", "PASSEPORT", "Parcours", show.starters(&class)[0]["horse"].as_str().unwrap()]);
    // The style judges (EquiEvents' own role), not the jumping judge (R, Rosson's choice).
    let people = show.call("find_officials", json!({ "query": "er" })).unwrap();
    let name = |i: usize| people[i]["name"].as_str().unwrap().to_string();
    show.call("set_role", json!({ "class": class, "person": people[0]["person"], "role": "R" })).unwrap();
    show.call("set_role", json!({ "class": class, "person": people[1]["person"], "role": show::model::STYLE_JUDGE })).unwrap();
    show.call("set_role", json!({ "class": class, "person": people[2]["person"], "role": show::model::STYLE_JUDGE })).unwrap();
    let officials = show.call("officials", json!({})).unwrap();
    assert!(officials["roles"].as_array().unwrap().iter().any(|r| r["code"] == show::model::STYLE_JUDGE && r["fr"] == "Juge de style"));
    // A style sheet per starter, in German.
    let answer = show.call("print", json!({ "what": "style_sheets", "class": class, "language": "de" })).unwrap();
    if let Ok(dir) = std::env::var("EQUIEVENTS_PRINTS") {
        let cards = show.call("print", json!({ "what": "start_cards", "class": class, "language": "fr" })).unwrap();
        for a in [&answer, &cards] {
            std::fs::copy(a["path"].as_str().unwrap(), std::path::Path::new(&dir).join(a["name"].as_str().unwrap())).unwrap();
        }
    }
    // A licence class (B100): two judges, a page per rider each.
    assert_eq!(answer["pages"].as_u64().unwrap() as usize, 2 * starters);
    let id = show::model::ClassId(class);
    let print = prints::style_sheets(&state(), &id, Language::De, "now").unwrap();
    let text = typeset::text(print.template, &print.data).unwrap().join("\n");
    has(&text, &["RICHTERBLATT · RICHTER 1", "RICHTER 2", "Stilprüfung B100", "Schenkellage", "Schlussresultat", "ausgezeichnet", "Korrekte Hilfengebung", &name(1), &name(2)]);
    assert!(!text.contains(&name(0)), "the jumping judge has no style sheet");
    // The judges' start list names them as style judges; the other lists name only the judges at
    // a position (dressage, driving).
    has(&show.text(json!({ "what": "start_list", "variant": "judges", "class": id.0, "language": "fr" })), &["Juge de style", &name(1)]);
    assert!(!show.text(json!({ "what": "start_list", "class": id.0, "language": "fr" })).contains("Juge de style"));
    // E and ECav: Swiss Equestrian's own sheets, one judge, half marks.
    for (category, words, lang) in [
        ("E50", vec!["Einsteigerprüfung Springen · E50", "halbe Noten möglich", "Gefühlvolles Zusammenwirken der Hilfen", "Punkte Richter:in"], Language::De),
        ("Rookie E50", vec!["Épreuve d’initiation saut · Rookie E50", "demi-notes possibles", "Coordination sensible des aides", "Assez mal"], Language::Fr),
        ("ECav", vec!["Épreuve accompagnée sur cavalettis · ECav", "Exécution", "accompagnateur/trice", "Points juge"], Language::Fr),
        ("ECav", vec!["Führzügel-Cavaletti-Prüfung · ECav", "Führperson", "Ausführung"], Language::De),
    ] {
        let mut state = state();
        let published = state.classes[&id].published.clone();
        state.published_classes.get_mut(&published).unwrap().category_text = category.into();
        let print = prints::style_sheets(&state, &id, lang, "now").unwrap();
        let pdf = typeset::render(print.template, &print.data).unwrap();
        assert_eq!(pdf.pages, starters, "{category}: one judge");
        let text = typeset::text(print.template, &print.data).unwrap().join("\n");
        has(&text, &words);
        if let Ok(dir) = std::env::var("EQUIEVENTS_PRINTS") {
            std::fs::write(std::path::Path::new(&dir).join(format!("style-{category}-{}.pdf", if lang == Language::Fr { "fr" } else { "de" })), pdf.bytes).unwrap();
        }
    }
    assert_eq!((prints::StyleSheet::of("B100"), prints::StyleSheet::of("P55 E"), prints::StyleSheet::of("E70")), (prints::StyleSheet::Licence, prints::StyleSheet::Licence, prints::StyleSheet::Initiation));
}

#[test]
fn the_officials_plan() {
    let show = Show::rehearsal();
    let class = show.set_up("14");
    let person = show.call("find_officials", json!({ "query": "Pfister" })).unwrap()[0]["person"].clone();
    show.call("set_role", json!({ "class": class, "person": person, "role": "DRI", "letter": "C" })).unwrap();
    let answer = show.call("print", json!({ "what": "officials", "language": "fr" })).unwrap();
    assert!(answer["pages"].as_u64().unwrap() >= 1);
    let state = show::Store::open(&show.dir.path().join("library").join(format!("{}.show", show.id))).unwrap().state().clone();
    let print = prints::officials_plan(&state, Language::Fr, "now");
    let text = typeset::text(print.template, &print.data).unwrap().join("\n");
    // The event's officials and the class's judge, with their day and class.
    has(&text, &["Plan des officiels", "Pfister", "dimanche 27 septembre", "14 (C)", "Vétérinaire"]);
}

#[test]
fn the_short_and_the_judges_start_lists() {
    let show = Show::rehearsal();
    let class = show.set_up("1");
    show.call("number", json!({ "class": class, "first": 1 })).unwrap();
    let full = show.text(json!({ "what": "start_list", "class": class, "language": "fr" }));
    let lite = show.text(json!({ "what": "start_list", "class": class, "variant": "lite", "language": "fr" }));
    let first = &show.starters(&class)[0];
    has(&lite, &["SOP", "PROPRIÉTAIRE", "SIGNALEMENT", first["rider"].as_str().unwrap(), first["horse"].as_str().unwrap()]);
    // One line a starter: fewer pages than the full list, or as many.
    let pages = |args: Value| show.call("print", args).unwrap()["pages"].as_u64().unwrap();
    assert!(pages(json!({ "what": "start_list", "class": class, "variant": "lite" })) <= pages(json!({ "what": "start_list", "class": class })));
    let judges = show.text(json!({ "what": "start_list", "class": class, "variant": "judges", "language": "de" }));
    has(&judges, &["Startliste Richter", "RESULTAT"]);
    // What the judge fills in, in Rosson's order and words, before the starters (beside the
    // class's head): Swiss Equestrian's office looks for them there.
    let fields = ["Von", "bis", "Anz. Gestartete (ohne HC)", "Anz. Gestartete HC", "Anz. beendet", "Anz. klassiert", "Visum Richter", "REITER"];
    let at: Vec<usize> = fields.iter().map(|f| judges.find(f).unwrap_or_else(|| panic!("{f} not in the judges' list"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{fields:?} at {at:?}");
    assert!(!full.contains("Visa du juge"));
}


#[test]
fn the_entries_in_detail() {
    let show = Show::rehearsal();
    let class = show.set_up("1");
    let published = show.call("show", json!({})).unwrap()["days"][0]["classes"][0]["published"].clone();
    let text = show.text(json!({ "what": "entries", "language": "fr" }));
    // Its riders by last name first.
    let first = &show.starters(&class)[0];
    let last_name = first["rider"].as_str().unwrap().rsplit(' ').next().unwrap();
    has(&text, &["Inscriptions en détail", "FINANCE 1", "PAYÉ", last_name, first["horse"].as_str().unwrap()]);
    let one = show.text(json!({ "what": "entries", "published": published, "language": "de" }));
    has(&one, &["Nennungen im Detail · Prüfung 1", "NENNGELD 1"]);
    assert!(one.len() < text.len());
}

#[test]
fn team_start_cards() {
    let show = Show::rehearsal();
    let class = show.set_up("7");
    show.call("number", json!({ "class": class, "first": 1 })).unwrap();
    let answer = show.call("print", json!({ "what": "team_cards", "class": class, "language": "fr" })).unwrap();
    let teams = show.call("start_list", json!({ "class": class })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "team").count();
    // One page per team.
    assert_eq!(answer["pages"].as_u64().unwrap() as usize, teams);
    // The riders' own cards too (numbered: the number in its box).
    let cards = show.call("print", json!({ "what": "start_cards", "class": class, "language": "fr" })).unwrap();
    if let Ok(dir) = std::env::var("EQUIEVENTS_PRINTS") {
        for a in [&answer, &cards] {
            std::fs::copy(a["path"].as_str().unwrap(), std::path::Path::new(&dir).join(a["name"].as_str().unwrap())).unwrap();
        }
    }
    let store = show::Store::open(&show.dir.path().join("library").join(format!("{}.show", show.id))).unwrap();
    let print = prints::start_cards(store.state(), &show::model::ClassId(class.clone()), Language::Fr, "now").unwrap();
    // Its scoring table's two rounds, a row each.
    has(&typeset::text(print.template, &print.data).unwrap().join("\n"), &["1re manche", "2e manche"]);
    let print = prints::team_cards(store.state(), &show::model::ClassId(class.clone()), Language::Fr, "now").unwrap();
    let text = typeset::text(print.template, &print.data).unwrap().join("\n");
    // Class 7: its scoring table's two rounds, and a jump-off (always, for teams), a block each.
    has(&text, &["ÉQUIPE 1", "RANG", "1re manche", "2e manche", "Barrage", "Total de l’équipe", "SOP CAV."]);
    assert!(!text.contains("REMARQUES"));
    // One team's card, on A5 (the same card, smaller); a team not in the class: refused.
    let team = show.call("start_list", json!({ "class": class })).unwrap()["lines"].as_array().unwrap().iter().find(|l| l["kind"] == "team").unwrap()["team"].as_str().unwrap().to_string();
    let one_team = show.call("print", json!({ "what": "team_cards", "class": class, "team": team, "paper": "a5" })).unwrap();
    assert_eq!(one_team["pages"], 1);
    assert!(one_team["name"].as_str().unwrap().starts_with("Carte de départ de"), "{one_team}");
    assert!(show.call("print", json!({ "what": "team_cards", "class": class, "team": "nobody" })).unwrap_err().contains("not in class"));
    // Not a team class: said so.
    let one = show.set_up("1");
    assert!(show.call("print", json!({ "what": "team_cards", "class": one })).unwrap_err().contains("équipes"));
}

/// Start cards follow the class's scoring table: its rounds, phases, jump-offs (7 obstacles)
/// and winning round, and the sums the jury makes, carrying over what the table adds up.
#[test]
fn start_cards_follow_the_scoring_table() {
    let show = Show::rehearsal();
    let class = show.set_up("7");
    let id = show::model::ClassId(class.clone());
    let store = show::Store::open(&show.dir.path().join("library").join(format!("{}.show", show.id))).unwrap();
    let rounds_with = |code: &str, team: bool| -> Value {
        let mut state = store.state().clone();
        let table = state.scoring_tables.values().find(|t| t.code == code).unwrap_or_else(|| panic!("no table {code}")).id;
        state.classes.get_mut(&id).unwrap().scoring_table = Some(table);
        let print = if team { prints::team_cards(&state, &id, Language::Fr, "now") } else { prints::start_cards(&state, &id, Language::Fr, "now") }.unwrap();
        if let Ok(dir) = std::env::var("EQUIEVENTS_PRINTS") {
            let pdf = typeset::render(print.template, &print.data).unwrap();
            std::fs::write(std::path::Path::new(&dir).join(format!("cards-{code}{}.pdf", if team { "-team" } else { "" })), pdf.bytes).unwrap();
        }
        print.data["rounds"].clone()
    };
    let labels = |r: &Value| r.as_array().unwrap().iter().map(|r| r["label"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    // A sum: what is added up, then its total.
    let boxes = |r: &Value, round: usize, group: usize| {
        let g = &r[round]["groups"][group];
        g["boxes"].as_array().unwrap().iter().chain([&g["total"]]).map(|b| b.as_str().unwrap().to_string()).collect::<Vec<_>>()
    };

    // A round and a jump-off of at most 7 obstacles; penalties + time penalties = total, time +
    // correction = total time.
    let r = rounds_with("AZ1AZ", false);
    assert_eq!(labels(&r), ["Parcours", "Barrage"]);
    assert_eq!((r[0]["obstacles"].as_u64(), r[1]["obstacles"].as_u64()), (Some(14), Some(7)));
    assert_eq!(boxes(&r, 0, 0), ["Pén. obstacles", "Pén. temps", "Total pén."]);
    assert_eq!(boxes(&r, 0, 1), ["Temps", "Correction de temps", "Temps total"]);
    // No jump-off in the table: none on the card (team cards: always two rounds and one).
    assert_eq!(labels(&rounds_with("AZ", false)), ["Parcours"]);
    assert_eq!(labels(&rounds_with("AZ", true)), ["1re manche", "2e manche", "Barrage"]);
    // Two rounds: round 1's penalties carried into round 2; its time too when both add up.
    let r = rounds_with("AZ2RUZ2", false);
    assert_eq!(boxes(&r, 1, 0), ["Pén. 1re m.", "Pén. obstacles", "Pén. temps", "Total pén."]);
    assert_eq!(boxes(&r, 1, 1), ["Temps", "Correction de temps", "Temps total"]);
    let r = rounds_with("AZ2RUZ12", false);
    assert_eq!(boxes(&r, 1, 1), ["Temps 1re m.", "Temps", "Correction de temps", "Temps total"]);
    // Two phases: normal ones apart, the special ones with phase 1's penalties carried over.
    let r = rounds_with("2PHAZ", false);
    assert_eq!(labels(&r), ["Phase 1", "Phase 2"]);
    assert_eq!(r[1]["obstacles"].as_u64(), Some(7));
    assert_eq!(boxes(&r, 1, 0), ["Pén. obstacles", "Pén. temps", "Total pén."]);
    let r = rounds_with("2PHAZPAZP", false);
    assert_eq!(boxes(&r, 1, 0), ["Pén. phase 1", "Pén. obstacles", "Pén. temps", "Total pén."]);
    // Table C (phase 2 here): time, knock-downs in seconds, correction.
    let r = rounds_with("2PHAC", false);
    assert_eq!(boxes(&r, 1, 0), ["Temps", "Pén. en secondes", "Correction de temps", "Temps total"]);
    // Winning rounds: with the earlier rounds' penalties, or from zero.
    let r = rounds_with("AZ2RUSAZ2", false);
    assert_eq!(labels(&r), ["1re manche", "2e manche", "Round des gagnants"]);
    assert_eq!(boxes(&r, 2, 0)[0], "Pén. 1re + 2e m.");
    let r = rounds_with("AZAZSC0", false);
    assert_eq!(boxes(&r, 1, 0), ["Temps", "Pén. en secondes", "Correction de temps", "Temps total"]);
    // Style: the style judges' note less the penalties; points classes: points less the time
    // penalties. The totals side by side, penalties (or points) left, time right.
    let r = rounds_with("STIL", false);
    assert_eq!(boxes(&r, 0, 0), ["Note de style", "Pén. obstacles", "Pén. temps", "Total des points"]);
    assert_eq!(r[0]["groups"][0]["minus"], true);
    assert_eq!((r[0]["groups"][0]["slot"].as_u64(), r[0]["groups"][1]["slot"].as_u64()), (Some(0), Some(1)));
    assert_eq!(boxes(&rounds_with("PUNKT", false), 0, 0), ["Points", "Pén. temps", "Total des points"]);
    // The ideal-time table: no time penalties (S20).
    assert_eq!(boxes(&rounds_with("AZI", false), 0, 0), ["Pén. obstacles", "Total pén."]);
    // Several jump-offs, numbered; and a card with many rounds still lays out.
    assert_eq!(labels(&rounds_with("AZ2AZ", false)), ["Parcours", "1er barrage", "2e barrage"]);
    assert_eq!(rounds_with("BARR", false).as_array().unwrap().len(), 5);
    rounds_with("AZ2RU1AZZ12", true);
}

/// Start cards as the office chooses: two to an A4 page or one to an A5 page; the whole class or
/// one starter's.
#[test]
fn start_cards_a4_or_a5_the_class_or_one() {
    let show = Show::rehearsal();
    let class = show.set_up("1");
    let starters: Vec<u64> = show.call("start_list", json!({ "class": class })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter" && l["not_starting"] == false).map(|l| l["entry"].as_u64().unwrap()).collect();
    let pages = |args: Value| show.call("print", args).unwrap()["pages"].as_u64().unwrap() as usize;
    assert_eq!(pages(json!({ "what": "start_cards", "class": class })), starters.len().div_ceil(2));
    assert_eq!(pages(json!({ "what": "start_cards", "class": class, "paper": "a5" })), starters.len());
    let one = show.call("print", json!({ "what": "start_cards", "class": class, "paper": "a5", "entry": starters[2] })).unwrap();
    assert_eq!(one["pages"], 1);
    assert!(one["name"].as_str().unwrap().starts_with("Carte de départ"), "{one}");
    assert!(show.call("print", json!({ "what": "start_cards", "class": class, "entry": 1 })).unwrap_err().contains("does not start"));
}
