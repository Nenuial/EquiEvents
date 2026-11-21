//! Every template lays out its data: a PDF comes out, and its pages say what the data says.

use serde_json::{json, Value};
use typeset::{render, text, Template};

fn base(title: &str) -> Value {
    json!({
        "title": title,
        "language": "fr",
        "printed": "13.11.2026 18:30",
        "labels": {
            "printed": "Imprimé le", "page": "Page", "time": "Heure", "number": "N°", "rider": "Cavalier", "horse": "Cheval",
            "carried": "Qualif.", "rank": "Rang", "class": "Épreuve", "judge": "Juge", "movement": "Mouvement", "mark": "Note",
            "remarks": "Remarques", "penalties": "Pénalités", "total": "Total", "signature": "Signature", "tie_break": "Départage",
            "participants": "Participants", "classes": "Épreuves", "schedule": "Horaire", "starters": "Partants", "judges": "Juges"
        },
        "event": { "title": "Championnat vaudois", "place": "Le Chalet-à-Gobet", "dates": "25–27 septembre 2026", "number": "2026.8352" }
    })
}

fn with(mut data: Value, key: &str, value: Value) -> Value {
    data[key] = value;
    data
}

fn all_text(template: Template, data: &Value) -> String {
    let pdf = render(template, data).unwrap_or_else(|e| panic!("{template:?}: {e}"));
    assert!(pdf.bytes.starts_with(b"%PDF"));
    assert!(pdf.pages >= 1);
    text(template, data).unwrap().join("\n")
}

fn class(lines: Value) -> Value {
    json!({
        "number": "12", "name": "R/N120 · A au chrono",
        "info": [["Date", "vendredi 25 septembre"], ["Barème", "A au chrono"]],
        "officials": [["Président", "Odile Fasel"]],
        "columns": { "time": true, "number": true, "carried": true },
        "lines": lines
    })
}

#[test]
fn start_list() {
    let lines = json!([
        { "kind": "heading", "text": "Niveau 2" },
        { "kind": "starter", "position": 1, "time": "08:00", "number": 101, "rider": "Lucien Amstutz", "rider_sub": "Villars-le-Test · Club Equestre du Test",
          "horse": "PRAIRIE BLEUE", "horse_sub": "Bai, 2015", "mark": "", "not_starting": false, "carried": "4" },
        { "kind": "note", "text": "Pause 10:00 – 10:30" },
        { "kind": "starter", "position": 2, "time": null, "number": null, "rider": "Noémie Rochat-Exemple", "rider_sub": "", "horse": "ORAGE II", "horse_sub": "", "mark": "NS", "not_starting": true, "carried": null }
    ]);
    let data = with(base("Liste de départ 12"), "classes", json!([class(lines)]));
    let text = all_text(Template::StartList, &data);
    for wanted in ["Lucien Amstutz", "PRAIRIE BLEUE", "Niveau 2", "Pause 10:00", "NS", "Odile Fasel", "2026.8352", "Page"] {
        assert!(text.contains(wanted), "{wanted} not in {text}");
    }
}

#[test]
fn ranking() {
    let mut c = class(json!([
        { "rank": 1, "placed": true, "team": false, "rider": "Bastien Morand-Exemple", "rider_sub": "Exempleville", "horse": "TORNADE DU TEST", "horse_sub": "", "cells": ["0", "69.80"] },
        { "rank": null, "placed": false, "team": false, "rider": "Yann Exemple", "rider_sub": "", "horse": "CALL ME", "horse_sub": "", "cells": ["éliminé", ""] }
    ]));
    c["columns"] = json!(["Pén.", "Temps"]);
    c["summary"] = json!("4 partants, 1 classé");
    let text = all_text(Template::Ranking, &with(base("Classement 12"), "classes", json!([c])));
    for wanted in ["Bastien Morand-Exemple", "69.80", "éliminé", "4 partants"] {
        assert!(text.contains(wanted), "{wanted} not in {text}");
    }
}

#[test]
fn judge_sheets() {
    let test = json!({
        "title": "Reprise de dressage L 12/60", "subtitle": "Carré 20 x 60m - Temps 4'00", "version": "Édition 2021", "fei": false, "kuer": false, "children": false,
        "tables": [
            { "layout": "lektionen", "title": "Reprise", "before": "", "after": "Quitter le carré au pas rênes longues", "rows": [
                { "number": "1", "items": [{ "position": "A", "text": "Entrer au trot rassemblé", "ideas": "La rectitude" }, { "position": "X", "text": "Arrêt, immobilité, salut", "ideas": "" }], "mark": "", "correction": "", "coefficient": "", "remarks": "" },
                { "number": "2", "items": [{ "position": "C", "text": "Piste à main gauche", "ideas": "L'incurvation" }], "mark": "", "correction": "", "coefficient": "2", "remarks": "" }
            ] },
            { "layout": "standard", "title": "Impression générale", "before": "", "after": "", "rows": [{ "number": "20", "text": "Allures (Franchise et régularité)", "mark": "", "correction": "", "coefficient": "2", "remarks": "" }] },
            { "layout": "strafpunkte", "title": "Points de pénalisation", "before": "", "after": "", "rows": [{ "text": "Première erreur", "penalty": "2 points de pénalisation", "received": "", "remarks": "" }] }
        ],
        "result_after": "", "max_points": "260"
    });
    let sheet = json!({ "test": "7", "memo": false, "number": "7", "rider": "Sandra Imhof-Exemple (123)", "horse": "BELLE EPOQUE CH (456)", "class": "12", "judge": "Hélène Exemple, Bulle (C)", "date_place": "26.09.2026 Le Chalet-à-Gobet" });
    let memo = json!({ "test": "7", "memo": true, "number": "", "rider": "", "horse": "", "class": "12", "judge": "Hélène Exemple, Bulle (C)", "date_place": "26.09.2026 Le Chalet-à-Gobet" });
    let mut data = with(with(base("Feuilles de juge 12"), "sheets", json!([memo, sheet.clone(), sheet])), "tests", json!({ "7": test }));
    data["year"] = json!("2026");
    for (k, v) in [
        ("memo", "JUGE"), ("subtotal", "Total intermédiaire:"), ("result", "Résultat"), ("max_points", "Max. de points"), ("signature", "Signature de la/du juge:"),
        ("start_number", "Numéro de start"), ("rider", "Cavalier/ère"), ("horse", "Cheval"), ("class_number", "Épreuve n°"), ("judge", "Juge en (Position)"), ("date_place", "Date/Lieu"),
        ("number", "N°"), ("criteria", "Critères de jugement"), ("mark", "Note"), ("correction", "Corr."), ("coefficient", "Coeff."), ("remarks", "Remarques"),
        ("movements", "Figures"), ("ideas", "Idées directrices"), ("total_points", "Total des points"), ("percent", "Résultat en pourcent"),
        ("total_a", "Total note A / 2"), ("total_b", "Total note B / 2"), ("total_a_children", "Total note A"), ("total_b_children", "Total note B"),
        ("carried_from", "Report de la page"), ("carried_to", "Report à la page"),
    ] {
        data["labels"][k] = json!(v);
    }
    let text = all_text(Template::JudgeSheets, &data);
    for wanted in ["SWISS EQUESTRIAN", "Reprise de dressage L 12/60", "Sandra Imhof-Exemple (123)", "BELLE EPOQUE CH (456)", "Arrêt, immobilité, salut", "Total intermédiaire: Reprise", "Première erreur", "Max. de points 260 = 100%", "Hélène Exemple, Bulle (C)", "Signature de la/du juge:", "JUGE", "© 2026 Swiss Equestrian"] {
        assert!(text.contains(wanted), "{wanted} not in {text}");
    }
    // Each sheet on its own page(s).
    let pdf = render(Template::JudgeSheets, &data).unwrap();
    assert!(pdf.pages >= 3);
    if let Ok(dir) = std::env::var("EQUIEVENTS_PRINTS") {
        std::fs::write(std::path::Path::new(&dir).join("judge-sheets-template.pdf"), pdf.bytes).unwrap();
    }
}

#[test]
fn combined_participants_schedule() {
    let ranking = json!({
        "name": "Championnat R", "rule": "Pénalités additionnées",
        "classes": ["1", "2"],
        "lines": [{ "rank": 1, "rider": "Lucien Amstutz", "rider_sub": "", "horse": "PRAIRIE BLEUE", "cells": [["0", "71.20"], ["0", "64.90"]], "total": "0", "tie_break": "64.90" }]
    });
    let text = all_text(Template::Combined, &with(base("Championnat R"), "ranking", ranking));
    assert!(text.contains("Championnat R") && text.contains("64.90"), "{text}");

    let riders = json!([{ "rider": "Noémie Rochat-Exemple", "rider_sub": "Exempleville", "horses": [{ "horse": "ORAGE II", "classes": "1 (#12) · 2" }, { "horse": "ALIZEE C CH", "classes": "1" }] }]);
    let text = all_text(Template::Participants, &with(base("Participants"), "riders", riders));
    assert!(text.contains("ALIZEE C CH") && text.contains("#12"), "{text}");

    let days = json!([{ "date": "vendredi 25 septembre", "classes": [{ "time": "08:00", "number": "1", "name": "R/N120", "sub": "A au chrono", "starters": 43, "judges": "Odile Fasel" }] }]);
    let text = all_text(Template::Schedule, &with(base("Horaire"), "days", days));
    assert!(text.contains("R/N120") && text.contains("08:00"), "{text}");
}

/// A class's logo, as Rosson keeps it (a PNG data URL), at the top of its start list; something
/// that is not an image is left out, and the print still comes.
#[test]
fn a_class_logo() {
    let logo = "data:image/octet-stream;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEElEQVR4nGP4z8AARAwQCgAf7gP9i18U1AAAAABJRU5ErkJggg==";
    let mut with_logo = class(json!([]));
    with_logo["logo"] = json!(logo);
    let data = with(base("Liste de départ"), "classes", json!([with_logo]));
    let pdf = render(Template::StartList, &data).unwrap();
    let mut without = class(json!([]));
    without["logo"] = json!("data:image/png;base64,bm90IGFuIGltYWdl");
    let plain = render(Template::StartList, &with(base("Liste de départ"), "classes", json!([without]))).unwrap();
    // The image is in the PDF; the other is not.
    assert!(pdf.bytes.len() > plain.bytes.len());
    assert!(String::from_utf8_lossy(&pdf.bytes).contains("/Image"));
    assert!(!String::from_utf8_lossy(&plain.bytes).contains("/Image"));
}

