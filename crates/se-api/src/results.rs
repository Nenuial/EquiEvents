//! `putResultate`: a class's results, as Rosson's results screen uploads them
//! (`UploadService.uploadResultate`).
//!
//! The ranking comes from [`show::ranking`] (the engine, ranks set by hand, placed riders); the
//! rest is the class, its officials and the entries as Swiss Equestrian delivered them, in Rosson's shape.
//!
//! Where Rosson itself is not consistent, one of its forms is used: entered jumping values are
//! sent as text ("61.08"; Rosson sends numbers for some entry paths), reasons as numbers.
//! Rosson's slips that reach the server are kept, since the server is used to them:
//! `kategorie_id` of riders in classes with chosen programmes is the whole category record.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use scoring::dressage::ProgrammeKind;
use scoring::jumping::team;
use scoring::number::round;
use scoring::ranking::Placement;
use serde_json::{json, Map, Value};
use show::input::ScoringInput;
use show::master;
use show::model::*;
use show::ranking::Line;
use show::State;

use crate::{Keys, Method, Request};

/// Rosson's rank for riders who are out.
pub const OUT_RANK: u32 = 99;

/// A decimal as a JSON number, integral values without decimals (235, 71.5).
pub(crate) fn number(d: Decimal) -> Value {
    let d = d.normalize();
    if d.fract().is_zero() {
        if let Ok(i) = i64::try_from(d) {
            return json!(i);
        }
    }
    d.to_string().parse::<f64>().ok().and_then(serde_json::Number::from_f64).map_or(Value::Null, Value::Number)
}

/// An id that Swiss Equestrian gives as a number, as a JSON number when it is one.
fn id_value(id: &str) -> Value {
    id.parse::<u64>().map_or_else(|_| json!(id), |n| json!(n))
}

/// The upload of a class's results.
pub fn put_resultate(state: &State, class_id: &ClassId, keys: &Keys) -> Result<Request, String> {
    let class = state.classes.get(class_id).ok_or_else(|| format!("unknown class {class_id}"))?;
    let published = state.published_classes.get(&class.published).ok_or("no published class")?;
    let ranking = state.ranking(class_id)?;
    let input = state.scoring_input(class_id);
    let class_programme = class.programme.and_then(|p| state.programmes.get(&p));
    let individual = class_programme.is_some_and(master::is_individual);
    let dressage = matches!(input, ScoringInput::Dressage { .. } | ScoringInput::DressageTeams { .. });
    // Team dressage: the team gets the result; its riders are listed (Rosson's
    // `getNennungDetailsVonEquipenResultat`: no rank, no points).
    let team_dressage = matches!(input, ScoringInput::DressageTeams { .. });
    let three_two = team_dressage && class.options.team_3_2;
    let teams_published = published.team_size.is_some();

    // Officials of the class, as Rosson lists them (by assignment id).
    let officials: Vec<Value> = state
        .assignments
        .values()
        .filter(|a| a.class.as_ref() == Some(class_id) && a.for_swiss_equestrian())
        .map(|a| json!({ "id": a.id.0, "person_id": a.person.0, "funktion_id": a.role_id, "buchstabe": a.letter.clone().unwrap_or_default() }))
        .collect();

    // Team values (team classes): per field, the sum of the counted results.
    let team_values: BTreeMap<String, (BTreeMap<String, Decimal>, Option<u32>)> = match &input {
        ScoringInput::JumpingTeams { table, teams, count_all, .. } => teams.iter().map(|t| {
            (t.id.clone(), (team::team_values(table, t, *count_all), team::team_entry(table, t, *count_all).out.map(|o| o.code)))
        }).collect(),
        _ => BTreeMap::new(),
    };
    let per_judge = if dressage { per_judge_results(state, class_id, &ranking.lines, individual, team_dressage) } else { BTreeMap::new() };
    // 3/2: each rider's judge (the team's H, C, M), by rider.
    let rider_judge: BTreeMap<String, String> = ranking.lines.iter().filter(|l| three_two && !l.team).filter_map(|l| l.judge_totals.keys().next().map(|j| (l.id.clone(), j.clone()))).collect();

    // Eventing: each rider's penalties per phase and where they went out (Rosson's CC branch).
    let overall = phase_results(state, class);

    let mut resultate = Vec::new();
    let (mut started, mut placed, mut finished) = (0usize, 0usize, 0usize);
    let counts_as_started = |l: &Line| l.team == matches!(input, ScoringInput::JumpingTeams { .. } | ScoringInput::DressageTeams { .. });
    for line in &ranking.lines {
        let rang = match line.placement {
            Placement::Rank(r) => r,
            Placement::Out => OUT_RANK,
        };
        let mut args = Map::new();
        args.insert("pruefungs_nummer".into(), json!(class.number));
        args.insert("rang".into(), json!(rang));
        args.insert("klassiert".into(), json!(line.placed));
        let out;
        if line.team {
            let (values, team_out) = team_values.get(&line.id).cloned().unwrap_or_default();
            out = team_out;
            if let Some(code) = out {
                args.insert("aufgabegrund_id".into(), json!(code));
            }
            args.insert("reiter_id".into(), Value::Null);
            args.insert("pferd_id".into(), Value::Null);
            args.insert("team_resultat".into(), json!(true));
            args.insert("reiter_name".into(), json!(state.teams.get(&TeamId(line.id.clone())).map(|t| t.name.clone()).unwrap_or_default()));
            args.insert("team_id".into(), id_value(&line.id));
            for (field, v) in &values {
                args.insert(field.replacen("strafpunkte_", "punkte_", 1), number(*v));
            }
            if team_dressage {
                // 3/2: the team's judges are E and B (H, C, M judged its riders).
                let judges: Vec<Value> = per_judge.get(&line.id).cloned().unwrap_or_default().into_iter().filter(|j| !three_two || matches!(j["buchstabe"].as_str(), Some("E" | "B"))).collect();
                args.insert("pro_richter".into(), Value::Array(judges));
                args.insert("punkte_total".into(), number(line.judge_totals.values().sum()));
                args.insert("punkte_prozent".into(), json!(format!("{:.2}", round(line.percentage.unwrap_or_default(), 2))));
            }
        } else {
            let entry_id = EntryId(line.id.parse().map_err(|_| format!("entry id {}", line.id))?);
            let entry = state.entries.get(&entry_id).ok_or_else(|| format!("unknown entry {entry_id}"))?;
            let person = state.persons.get(&entry.rider);
            let card = state.results.get(&(class_id.clone(), entry_id.clone()));
            out = match overall.get(&line.id) {
                Some(phases) => phases.iter().find_map(|p| p.out),
                None => card.and_then(|c| c.out),
            };
            if team_dressage {
                args.remove("rang");
                args.remove("klassiert");
            } else if !dressage || out.is_some() {
                args.insert("aufgabegrund_id".into(), json!(out.unwrap_or(0)));
            }
            args.insert("reiter_id".into(), json!(entry.rider.0));
            // The rider as written on the entry (Rosson sends the entry's copy).
            let (first, last, town) = match (&entry.rider_on_entry, person) {
                (Some(r), _) => (r.first_name.clone(), r.last_name.clone(), r.town.clone()),
                (None, Some(p)) => (p.first_name.clone(), p.last_name.clone(), p.town.clone()),
                (None, None) => Default::default(),
            };
            args.insert("reiter_name".into(), json!(format!("{last} {first}")));
            args.insert("reiter_ort".into(), json!(town));
            if let Some(horse) = entry.horses_on_entry.first() {
                args.insert("pferd_id".into(), horse["pferd_id"].clone());
                args.insert("pferd_name".into(), horse["pferdename"].clone());
            }
            args.insert("stufe".into(), entry.stage.map_or(Value::Null, |s| json!(s)));
            let driving_class = state.published_classes.get(&class.published).is_some_and(|p| p.discipline == "CA");
            // Eventing: the total and each phase's penalties (dressage 100 − %).
            if let (Some(phases), false) = (overall.get(&line.id), driving_class) {
                args.insert("punkte_total".into(), number(line.faults.unwrap_or_default()));
                for p in phases {
                    let field = match p.kind {
                        PhaseKind::Dressage => "punkte_dressur",
                        PhaseKind::Jumping => "strafpunkte_springen_hindernis",
                        _ => "strafpunkte_cross_hindernis",
                    };
                    args.insert(field.into(), number(p.penalties.unwrap_or_default()));
                }
            }
            // Driving: each phase's penalties (or rank), named after it, and why out of it.
            if let (Some(phases), true) = (overall.get(&line.id), driving_class) {
                for p in phases {
                    let name = rosson_discipline(p.kind);
                    if state.overall_by_rank(class) {
                        args.insert(format!("rang_{name}"), json!(p.rank));
                    } else {
                        args.insert(format!("strafpunkte_{name}"), number(p.penalties.unwrap_or_default()));
                    }
                    if let Some(code) = p.out {
                        args.insert(format!("aufgabegrund_{name}_id"), json!(code));
                    }
                }
                let total = if state.overall_by_rank(class) { "rangpunkte_total" } else { "strafpunkte_total" };
                args.insert(total.into(), number(line.faults.unwrap_or_default()));
            }
            // Driving: only the horses that start in a phase (Rosson).
            let horses: Vec<Value> = entry.horses_on_entry.iter().filter(|h| !driving_class || h["einsaetze"].as_array().is_some_and(|e| !e.is_empty())).cloned().collect();
            args.insert("pferde".into(), Value::Array(horses));
            if teams_published {
                args.insert("team_mitglied".into(), json!(true));
                args.insert("team_id".into(), entry.team.as_ref().map_or(Value::Null, |t| id_value(&t.0)));
            }
            if team_dressage {
                // 3/2: the rider's own judge, as the team's.
                let team = entry.team.as_ref().map(|t| t.0.clone()).unwrap_or_default();
                if let Some(judge) = rider_judge.get(&line.id) {
                    let letter = state.assignments.get(&AssignmentId(judge.clone())).and_then(|a| a.letter.clone()).unwrap_or_default();
                    let mine: Vec<Value> = per_judge.get(&team).into_iter().flatten().filter(|j| j["buchstabe"] == json!(letter)).cloned().collect();
                    args.insert("pro_richter".into(), Value::Array(mine));
                }
            } else if dressage {
                let judges = per_judge.get(&line.id).cloned().unwrap_or_default();
                args.insert("pro_richter".into(), Value::Array(judges));
                args.insert("punkte_total".into(), number(line.judge_totals.values().sum()));
                // Rosson: the percentage as text, "0.00" when there is none.
                args.insert("punkte_prozent".into(), json!(format!("{:.2}", round(line.percentage.unwrap_or_default(), 2))));
                // Freestyle: the parts (Children's go in `pro_richter`).
                if programme_kind(state, class_id, &line.id, individual) == Some(ProgrammeKind::Freestyle) {
                    if let Some(a) = line.part_a {
                        args.insert("punkte_prozent_a".into(), json!(format!("{a:.2}")));
                    }
                    if let Some(b) = line.part_b {
                        args.insert("punkte_prozent_b".into(), json!(format!("{b:.2}")));
                    }
                }
                if individual {
                    if let Some(category) = entry.programme.and_then(|p| state.categories.values().find(|c| c.programme == Some(p))) {
                        args.insert("kategorie_id".into(), category.record.clone()); // sic: Rosson sends the record
                    }
                    if let Some(p) = entry.programme.and_then(|p| state.programmes.get(&p)) {
                        args.insert("kategorie_code".into(), json!(p.name));
                    }
                }
            } else if let Some(card) = card {
                for (field, v) in &card.values {
                    args.insert(field.replacen("strafpunkte_", "punkte_", 1), json!(v.to_string()));
                }
            }
        }
        if counts_as_started(line) {
            started += 1;
            placed += line.placed as usize;
            finished += out.is_none() as usize;
        }
        resultate.push(Value::Object(args));
    }

    let category_id = class.programme.and_then(|p| state.categories.values().find(|c| c.programme == Some(p))).map(|c| c.id).or(published.category_id);
    let mut pruefung = json!({
        "ausschreibung_pruefung_id": class.published.0,
        "nummer": class.number,
        "bezeichnung": class.name,
        "datum": format!("{}T00:00:00.000Z", class.date), // Rosson: the date as a JavaScript date, UTC midnight
        "disziplin_id": published.discipline_id,
        "kategorie_id": category_id,
        // Eventing: the riders of the first phase (the class made of phases has no list).
        "anzahl_gemeldete_reiter": state.starts.get(class.phases.first().unwrap_or(class_id)).map_or(0, |s| s.iter().filter(|s| state.entries.contains_key(&s.entry)).count()),
        "anzahl_gestartete_reiter": started,
        "anzahl_klassierte_reiter": placed,
        "anzahl_vollendete_reiter": finished,
        "offizielle": officials,
    });
    if let Some(table) = class.scoring_table {
        pruefung["wertung_id"] = json!(table);
    }
    if let Some(fields) = &class.extra_fields {
        pruefung["weitere_felder"] = Value::Array(fields.iter().map(|f| json!({ "key": f.key, "value": f.value })).collect());
    }

    let mut body = keys.envelope();
    body["resultate"] = Value::Array(resultate);
    body["pruefungen"] = json!([pruefung]);
    body["veranstaltungsbeteiligungen"] = Value::Array(event_officials(state));
    body["delete_veranstaltungsbeteiligungen"] = json!([]);
    Ok(Request { method: Method::Post, path: "/putResultate".into(), query: Vec::new(), body: Some(body) })
}

/// `putResultate` for a ranking over several classes (a final), as Rosson's finals screen sends
/// it (`UploadService.uploadFinalResultate`): one class, numbered as the ranking (6.1), with a
/// row per rider and horse. A final over jumping and dressage classes goes as Swiss Equestrian's
/// combined discipline (4) and special category (462), each row with its ranks per class.
/// Rosson's ways kept: riders missing a class get rank 99; riders out in one get no rank; the
/// points are the dressage classes' judges' means added (0 for jumping); the percentage the
/// mean of the classes' percentages (none for jumping); the result as Rosson words it
/// (`resultat_layout`). Rosson's own extra lines never reached the server (it read a field that
/// does not exist): the ranking's lines are sent.
pub fn put_final(state: &State, id: &str, keys: &Keys) -> Result<Request, String> {
    let ranking = state.combined.get(id).ok_or_else(|| format!("unknown ranking {id}"))?;
    let first = ranking.classes.first().and_then(|c| state.classes.get(c)).ok_or("the ranking has no class")?;
    let published = state.published_classes.get(&first.published).ok_or("unknown published class")?;
    let lines = state.combined_ranking(id)?;
    let discipline_of = |c: &ClassId| state.classes.get(c).and_then(|k| state.published_classes.get(&k.published)).map(|p| p.discipline.clone()).unwrap_or_default();
    let disciplines: Vec<String> = ranking.classes.iter().map(discipline_of).collect();
    let mixed = disciplines.iter().any(|d| *d != disciplines[0]);
    let all_dressage = disciplines.iter().all(|d| d == "CD");
    let percentages = matches!(ranking.method, CombinedMethod::Percentages { .. });
    let number_text = state.combined_number(id);
    let class_numbers: Vec<String> = ranking.classes.iter().map(|c| state.classes.get(c).map(|k| k.number.clone()).unwrap_or_default()).collect();

    // Per class: each entry's ranking line (its judges' totals and percentage, dressage).
    let class_lines: Vec<BTreeMap<String, Line>> = ranking
        .classes
        .iter()
        .map(|c| state.ranking(c).map(|r| r.lines.into_iter().filter(|l| !l.team).map(|l| (l.id.clone(), l)).collect()).unwrap_or_default())
        .collect();
    let (mut placed, mut out_count) = (0usize, 0usize);
    let mut rows = Vec::new();
    // Team finals: a row per team, then a row per rider of each team (Rosson's order).
    let mut members_rows = Vec::new();
    for line in &lines {
        let Some(result) = line.results.iter().flatten().next() else { continue };
        if let show::combined::Who::Team(name) = &line.who {
            let team = result.team.clone().ok_or("a team without its id")?;
            let mut row = Map::new();
            match (line.rank, line.complete()) {
                (Some(rank), _) => {
                    row.insert("rang".into(), json!(rank));
                }
                (None, false) => {
                    row.insert("rang".into(), json!(OUT_RANK));
                }
                (None, true) => {}
            }
            let ranks_total: u32 = line.results.iter().flatten().filter_map(|r| r.rank).sum();
            row.insert("punkte_total".into(), json!(ranks_total));
            row.insert("pruefungs_nummer".into(), json!(number_text));
            row.insert("klassiert".into(), json!(line.placed));
            placed += line.placed as usize;
            if line.complete() && line.rank.is_none() {
                out_count += 1;
            }
            row.insert("team_id".into(), id_value(&team.0));
            row.insert("team_resultat".into(), json!(true));
            row.insert("reiter_name".into(), json!(name));
            if mixed {
                let per: Vec<Value> = line.results.iter().zip(&class_numbers).map(|(r, n)| json!({ "punkte": r.as_ref().and_then(|r| r.rank).map_or_else(|| json!("-"), |k| json!(k)), "pruefung_nummer": n })).collect();
                row.insert("pro_disziplin".into(), Value::Array(per));
            }
            rows.push(Value::Object(row));
            for entry in state.entries.values().filter(|e| e.team.as_ref() == Some(&team) && !e.waiting_list) {
                let mut m = Map::new();
                if let Some(rank) = line.rank {
                    m.insert("rang".into(), json!(rank));
                }
                m.insert("pruefungs_nummer".into(), json!(number_text));
                let (first_name, last_name, town) = match (&entry.rider_on_entry, state.persons.get(&entry.rider)) {
                    (Some(r), _) => (r.first_name.clone(), r.last_name.clone(), r.town.clone()),
                    (None, Some(p)) => (p.first_name.clone(), p.last_name.clone(), p.town.clone()),
                    (None, None) => Default::default(),
                };
                m.insert("reiter_id".into(), json!(entry.rider.0));
                m.insert("reiter_name".into(), json!(format!("{last_name} {first_name}")));
                m.insert("reiter_ort".into(), json!(town));
                if let Some(horse) = entry.horses_on_entry.first() {
                    m.insert("pferd_id".into(), horse["pferd_id"].clone());
                    m.insert("pferd_name".into(), horse["pferdename"].clone());
                }
                m.insert("stufe".into(), entry.stage.map_or(Value::Null, |s| json!(s)));
                m.insert("pferde".into(), Value::Array(entry.horses_on_entry.clone()));
                m.insert("team_mitglied".into(), json!(true));
                m.insert("team_id".into(), id_value(&team.0));
                members_rows.push(Value::Object(m));
            }
            continue;
        }
        let entry = state.entries.get(&result.entry).ok_or_else(|| format!("unknown entry {}", result.entry))?;
        let mut row = Map::new();
        // Rank: in every class and not out, its rank; missing a class, 99; out, none.
        let out_in = ranking.classes.iter().zip(&line.results).find(|(_, r)| r.as_ref().is_some_and(|r| r.rank.is_none()));
        match (line.rank, line.complete()) {
            (Some(rank), _) => {
                row.insert("rang".into(), json!(rank));
            }
            (None, false) => {
                row.insert("rang".into(), json!(OUT_RANK));
            }
            (None, true) => {}
        }
        row.insert("pruefungs_nummer".into(), json!(number_text));
        row.insert("klassiert".into(), json!(line.placed));
        placed += line.placed as usize;
        if let Some((class, r)) = out_in {
            out_count += 1;
            let reason = r.as_ref().and_then(|r| state.results.get(&(class.clone(), r.entry.clone()))).and_then(|c| c.out);
            if let Some(reason) = reason {
                row.insert("aufgabegrund_id".into(), json!(reason));
            }
        }
        // Points: each dressage class's judges' mean, added; percentage: the mean.
        let mut points = Decimal::ZERO;
        let mut percents: Vec<Decimal> = Vec::new();
        for (k, r) in line.results.iter().enumerate() {
            let Some(r) = r else { continue };
            if let Some(l) = class_lines[k].get(&r.entry.0.to_string()) {
                if disciplines[k] == "CD" && !l.judge_totals.is_empty() {
                    points += l.judge_totals.values().sum::<Decimal>() / Decimal::from(l.judge_totals.len() as u32);
                }
            }
            percents.extend(r.percentage);
        }
        let ranks_total: u32 = line.results.iter().flatten().filter_map(|r| r.rank).sum();
        row.insert("punkte_total".into(), if mixed { json!(ranks_total) } else { number(round(points, 2)) });
        row.insert(
            "punkte_prozent".into(),
            if all_dressage && !percents.is_empty() { number(round(percents.iter().sum::<Decimal>() / Decimal::from(percents.len() as u32), 4)) } else { Value::Null },
        );
        row.insert("resultat_layout".into(), json!(final_layout(line, percentages && all_dressage, all_dressage)));
        let (first_name, last_name, town) = match (&entry.rider_on_entry, state.persons.get(&entry.rider)) {
            (Some(r), _) => (r.first_name.clone(), r.last_name.clone(), r.town.clone()),
            (None, Some(p)) => (p.first_name.clone(), p.last_name.clone(), p.town.clone()),
            (None, None) => Default::default(),
        };
        row.insert("reiter_id".into(), json!(entry.rider.0));
        row.insert("reiter_name".into(), json!(format!("{last_name} {first_name}")));
        row.insert("reiter_ort".into(), json!(town));
        if let Some(horse) = entry.horses_on_entry.first() {
            row.insert("pferd_id".into(), horse["pferd_id"].clone());
            row.insert("pferd_name".into(), horse["pferdename"].clone());
        }
        row.insert("stufe".into(), entry.stage.map_or(Value::Null, |s| json!(s)));
        row.insert("pferde".into(), Value::Array(entry.horses_on_entry.clone()));
        if mixed {
            let per: Vec<Value> = line
                .results
                .iter()
                .zip(&class_numbers)
                .map(|(r, n)| json!({ "punkte": r.as_ref().and_then(|r| r.rank).map_or_else(|| json!("-"), |k| json!(k)), "pruefung_nummer": n }))
                .collect();
            row.insert("pro_disziplin".into(), Value::Array(per));
        }
        rows.push(Value::Object(row));
    }

    let teams_counted = rows.len();
    rows.extend(members_rows);
    let category_id = if mixed { Some(462) } else { first.programme.and_then(|p| state.categories.values().find(|c| c.programme == Some(p))).map(|c| c.id).or(published.category_id) };
    let date = ranking.record["datum"].as_str().map(str::to_string).unwrap_or_else(|| format!("{}T00:00:00.000Z", first.date));
    let mut pruefung = json!({
        "ausschreibung_pruefung_id": ranking.record["ausschreibung_pruefung_id"].as_u64().unwrap_or(first.published.0),
        "nummer": number_text,
        "bezeichnung": ranking.name,
        "datum": date,
        "disziplin_id": if mixed { 4 } else { published.discipline_id },
        "anzahl_gemeldete_reiter": state.starts.get(&first.id).map_or(0, |s| s.iter().filter(|s| state.entries.contains_key(&s.entry)).count()),
        // Rosson counts the ranking's lines (teams, in a team final).
        "anzahl_gestartete_reiter": teams_counted,
        "anzahl_klassierte_reiter": placed,
        "anzahl_vollendete_reiter": teams_counted - out_count,
        "kategorie_id": category_id,
    });
    if !ranking.extra_fields.is_empty() {
        pruefung["weitere_felder"] = Value::Array(ranking.extra_fields.iter().map(|f| json!({ "key": f.key, "value": f.value })).collect());
    }
    let mut body = keys.envelope();
    body["pruefungen"] = json!([pruefung]);
    body["resultate"] = Value::Array(rows);
    Ok(Request { method: Method::Post, path: "/putResultate".into(), query: Vec::new(), body: Some(body) })
}

/// A final's result as Rosson words it (`layoutFinaldressurFuer`): dressage, the classes'
/// percentages (or ranks) and the total, "(64.5 + 66.2) / 2 = <b>65.35%</b>"; otherwise the
/// ranks, "3 + 5 = <b>8</b>"; "-" when a class is missing.
fn final_layout(line: &show::combined::CombinedLine, percentages: bool, dressage: bool) -> String {
    let n = line.results.len();
    if dressage {
        let values: Vec<String> = line
            .results
            .iter()
            .map(|r| match r {
                Some(r) if r.rank.is_some() => {
                    if percentages {
                        r.percentage.map(|p| p.normalize().to_string())
                    } else {
                        r.rank.map(|k| k.to_string())
                    }
                }
                _ => None,
            })
            .map(|v| v.unwrap_or_else(|| " - ".into()))
            .collect();
        let whole = line.results.iter().all(|r| r.as_ref().is_some_and(|r| r.rank.is_some()));
        if percentages {
            let total = if whole { line.total.map(|t| format!("{}%", round(t, 2).normalize())).unwrap_or_else(|| " - ".into()) } else { " - ".into() };
            format!("({}) / {n} = <b>{total}</b>", values.join(" + "))
        } else {
            let total = if whole { line.results.iter().flatten().filter_map(|r| r.rank).sum::<u32>().to_string() } else { " - ".into() };
            format!("{} = <b>{total}</b>", values.join(" + "))
        }
    } else {
        if !line.complete() || line.results.iter().any(|r| r.as_ref().is_some_and(|r| r.rank.is_none())) {
            return "-".into();
        }
        let ranks: Vec<u32> = line.results.iter().flatten().filter_map(|r| r.rank).collect();
        format!("{} = <b>{}</b>", ranks.iter().map(u32::to_string).collect::<Vec<_>>().join(" + "), ranks.iter().sum::<u32>())
    }
}

/// The event's officials (assigned to the event, not to a class), as Rosson stores and sends them.
pub fn event_officials(state: &State) -> Vec<Value> {
    state
        .assignments
        .values()
        .filter(|a| a.class.is_none() && a.on_event == Some(true) && a.for_swiss_equestrian())
        .map(|a| {
            let mut o = Map::new();
            o.insert("id".into(), json!(a.id.0));
            let mut put = |k: &str, v: Option<Value>| {
                if let Some(v) = v {
                    o.insert(k.into(), v);
                }
            };
            put("readonly", a.readonly.map(|b| json!(b)));
            put("updated_at", a.updated_at.map(|t| json!(t)));
            put("veranstaltung_id", a.event.as_ref().map(|e| json!(e.0)));
            put("person_id", Some(json!(a.person.0)));
            put("rolle_id", a.role_id.map(|r| json!(r)));
            put("verantwortlicher", a.responsible.map(|b| json!(b)));
            put("equis_id", a.se_id.map(|i| json!(i)));
            put("auf_veranstaltung", a.on_event.map(|b| json!(b)));
            put("anzahl_einsatztage", a.days.map(|d| json!(d)));
            Value::Object(o)
        })
        .collect()
}

/// A rider's result in one phase of an eventing class, as the overall upload sends it.
struct PhaseResult {
    kind: PhaseKind,
    /// Dressage 100 − % (driving: its total), the others their penalties.
    penalties: Option<Decimal>,
    rank: Option<u32>,
    out: Option<u32>,
}

/// Rosson's name of a phase in uploads (`appConfig.disziplinCodes`).
fn rosson_discipline(kind: PhaseKind) -> &'static str {
    kind.se_name()
}

/// Eventing: each rider's results in the class's phases (none for other classes).
fn phase_results(state: &State, class: &Class) -> BTreeMap<String, Vec<PhaseResult>> {
    let mut out: BTreeMap<String, Vec<PhaseResult>> = BTreeMap::new();
    for phase in class.phases.iter().filter_map(|p| state.classes.get(p)) {
        let Some(kind) = phase.phase.as_ref().map(|p| p.kind) else { continue };
        let Ok(ranking) = state.ranking(&phase.id) else { continue };
        for l in ranking.lines.iter().filter(|l| !l.team) {
            let penalties = match kind {
                PhaseKind::Dressage if l.percentage.is_some() => l.percentage.map(|p| round(Decimal::ONE_HUNDRED - p, 2)),
                _ => l.faults,
            };
            // Eventing: the jury's penalties carried in the phase's number (S41).
            let penalties = penalties.map(|p| p + state.jury_penalty(&phase.id, &l.id));
            let code = l.id.parse().ok().and_then(|e| state.out_reason(&phase.id, &EntryId(e)));
            out.entry(l.id.clone()).or_default().push(PhaseResult { kind, penalties, rank: l.placement.rank(), out: code });
        }
    }
    out
}

/// `putZwischenresultate`: one phase of an eventing class, as Rosson sends a phase's results
/// (`upload.service.js` 474–582): the phase, its class, a line per rider with the rank and the
/// result as words (dressage: each judge's total).
pub fn put_zwischenresultate(state: &State, phase_id: &ClassId, keys: &Keys) -> Result<Request, String> {
    let phase = state.classes.get(phase_id).ok_or_else(|| format!("unknown class {phase_id}"))?;
    let kind = phase.phase.as_ref().map(|p| p.kind).ok_or("not a phase")?;
    let parent = phase.phase.as_ref().and_then(|p| state.classes.get(&p.of)).ok_or("the phase's class is gone")?;
    let published = state.published_classes.get(&parent.published).ok_or("no published class")?;
    let ranking = state.ranking(phase_id)?;
    let letters: BTreeMap<String, String> = state.dressage_judges(phase_id).into_iter().map(|a| (a.id.0.clone(), a.letter.clone().unwrap_or_default())).collect();
    let clock = |t: Decimal| {
        let s = t.trunc().to_string().parse::<u64>().unwrap_or(0);
        format!("{}:{:02}", s / 60, s % 60)
    };
    let mut zeilen = Vec::new();
    // Team cones (OKV Fahrcup): a row per team, then its drivers, each with their points
    // without the team's struck round (Rosson's `berechneFahrenStrafpunkte`).
    if ranking.lines.iter().any(|l| l.team) && kind == PhaseKind::Cones {
        let details = state.cones_team_details(phase)?;
        for (i, t) in ranking.lines.iter().filter(|l| l.team).enumerate() {
            let rang = match t.placement {
                Placement::Rank(r) => r,
                Placement::Out => OUT_RANK,
            };
            let name = state.teams.get(&show::model::TeamId(t.id.clone())).map(|x| x.name.clone()).unwrap_or_default();
            zeilen.push(json!({
                "position": i + 1, "rang": rang, "reiter_id": null, "cache_reiter_name": name, "cache_reiter_ort": "", "pferde": [],
                "resultat_details": "", "resultat_layout": format!("P: {}", t.faults.map(|f| format!("{f:.2}")).unwrap_or_default()),
                "team_resultat": true, "team_id": id_value(&t.id),
            }));
            let Some(detail) = details.iter().find(|d| d.team.0 == t.id) else { continue };
            let rounds: Vec<scoring::driving::TeamRound> = detail.rounds.iter().map(|r| scoring::driving::TeamRound { points: r.points, time: r.time }).collect();
            let struck = scoring::driving::struck_index(&rounds);
            for e in detail.riders.iter().filter_map(|e| state.entries.get(e)) {
                let (first, last, town) = match (&e.rider_on_entry, state.persons.get(&e.rider)) {
                    (Some(r), _) => (r.first_name.clone(), r.last_name.clone(), r.town.clone()),
                    (None, Some(p)) => (p.first_name.clone(), p.last_name.clone(), p.town.clone()),
                    (None, None) => Default::default(),
                };
                let mine: Vec<(usize, &show::TeamConesRound)> = detail.rounds.iter().enumerate().filter(|(_, r)| r.entry == e.id).collect();
                let points: Decimal = mine.iter().filter(|(k, _)| Some(*k) != struck).map(|(_, r)| r.points).sum();
                let details: Map<String, Value> = mine.iter().map(|(k, r)| (format!("Umgang {}", r.round), json!(if Some(*k) == struck { format!("<s>{:.2} {:.2}</s>", r.points, r.time) } else { format!("{:.2} {:.2}", r.points, r.time) }))).collect();
                zeilen.push(json!({
                    "position": i + 1, "rang": rang, "reiter_id": e.rider.0, "cache_reiter_name": format!("{last} {first}"), "cache_reiter_ort": town,
                    "pferde": show::model::horses_in_phase(e, kind).into_iter().cloned().collect::<Vec<_>>(),
                    "resultat_details": details, "resultat_layout": format!("P: {points:.2}"),
                    "team_mitglied": true, "team_id": id_value(&t.id),
                }));
            }
        }
    }
    for (i, l) in ranking.lines.iter().filter(|l| !l.team && !ranking.lines.iter().any(|t| t.team)).enumerate() {
        let Some(entry) = l.id.parse().ok().and_then(|e| state.entries.get(&EntryId(e))) else { continue };
        let (first, last, town) = match (&entry.rider_on_entry, state.persons.get(&entry.rider)) {
            (Some(r), _) => (r.first_name.clone(), r.last_name.clone(), r.town.clone()),
            (None, Some(p)) => (p.first_name.clone(), p.last_name.clone(), p.town.clone()),
            (None, None) => Default::default(),
        };
        let rang = match l.placement {
            Placement::Rank(r) => r,
            Placement::Out => OUT_RANK,
        };
        let details: Map<String, Value> = l.judge_totals.iter().map(|(j, t)| (letters.get(j).cloned().unwrap_or_else(|| j.clone()), number(*t))).collect();
        let layout = match kind {
            // Driving dressage: its total on 160, as Rosson's "P: x".
            PhaseKind::Dressage if l.percentage.is_none() => format!("P: {}", l.faults.map(|f| format!("{f:.2}")).unwrap_or_default()),
            PhaseKind::Dressage => l.percentage.map(|p| format!("{p:.2}%")).unwrap_or_default(),
            PhaseKind::Jumping => [l.faults.map(|f| f.normalize().to_string()), l.time.filter(|t| !t.is_zero()).map(|t| format!("{:.2}", t))].into_iter().flatten().collect::<Vec<_>>().join(" / "),
            PhaseKind::Cross => format!("P: {}, T: {}", l.faults.map(|f| format!("{f:.2}")).unwrap_or_default(), l.time.map(clock).unwrap_or_default()),
            PhaseKind::Cones | PhaseKind::Open => format!("P: {}, T: {}", l.faults.map(|f| format!("{f:.2}")).unwrap_or_default(), l.time.map(|t| format!("{t:.2}")).unwrap_or_default()),
            PhaseKind::Marathon | PhaseKind::Derby | PhaseKind::Multitest | PhaseKind::Presentation => format!("P: {}", l.faults.map(|f| format!("{f:.2}")).unwrap_or_default()),
        };
        zeilen.push(json!({
            "position": i + 1,
            "rang": rang,
            "reiter_id": entry.rider.0,
            "cache_reiter_name": format!("{last} {first}"),
            "cache_reiter_ort": town,
            // Driving: only the horses that start in the phase (Rosson's `getPferdeMitEinsatz`).
            "pferde": if state.driving(phase) { show::model::horses_in_phase(entry, kind).into_iter().cloned().collect::<Vec<_>>() } else { entry.horses_on_entry.clone() },
            "resultat_details": details,
            "resultat_layout": layout,
        }));
    }
    let name = crate::startlists::word(crate::startlists::Language::De, kind.discipline());
    let programme = phase.programme.and_then(|p| state.programmes.get(&p)).map(|p| show::master::programme_title(p, "de")).unwrap_or_default();
    let mut body = keys.envelope();
    body["kopf"] = json!({
        "rosson_zwischenpruefung_id": phase.id.0,
        "position": phase.position,
        "code": kind.code(),
        "name": name,
        "datum": format!("{}T00:00:00.000Z", phase.date),
        "uhrzeit": phase.time_text,
        "wertung_id": phase.scoring_table,
        "wertung_text": phase.scoring_text,
        "programm_name": programme,
    });
    body["pruefung"] = json!({
        "nummer": parent.number,
        "ausschreibung_pruefung_id": parent.published.0,
        "bezeichnung": parent.name,
        "datum": format!("{}T00:00:00.000Z", parent.date),
        "disziplin_id": published.discipline_id,
        "kategorie_id": published.category_id,
        "weitere_felder": parent.extra_fields.iter().flatten().map(|f| json!({ "key": f.key, "value": f.value })).collect::<Vec<_>>(),
    });
    body["zeilen"] = Value::Array(zeilen);
    Ok(Request { method: Method::Post, path: "/putZwischenresultate".into(), query: Vec::new(), body: Some(body) })
}

/// A rider's dressage programme kind (the class's programme, or the rider's own).
fn programme_kind(state: &State, class_id: &ClassId, entry_id: &str, individual: bool) -> Option<ProgrammeKind> {
    let programme = if individual {
        let entry = state.entries.get(&EntryId(entry_id.parse().ok()?))?;
        entry.programme
    } else {
        state.classes.get(class_id)?.programme
    };
    let def = state.programmes.get(&programme?)?;
    master::programme(def).ok().map(|p| p.kind)
}

/// Dressage: each rider's result per judge (`pro_richter`): the judge's total, percentage
/// (of the judge's maximum, 2 decimals) and rank among the class by that percentage. Freestyle:
/// the judge's parts in percent too. Children (one sheet): two rows, `C (A)` and `E (B)`, the
/// parts' points and percentages, as Rosson.
fn per_judge_results(state: &State, class_id: &ClassId, lines: &[Line], individual: bool, teams: bool) -> BTreeMap<String, Vec<Value>> {
    let class = &state.classes[class_id];
    let judges = state.dressage_judges(class_id);
    let max_of = |entry_id: &str| -> Option<Decimal> {
        let programme = if individual {
            let entry = state.entries.get(&EntryId(entry_id.parse().ok()?))?;
            entry.programme.and_then(|p| state.programmes.get(&p))
        } else {
            class.programme.and_then(|p| state.programmes.get(&p))
        };
        programme.map(|p| p.max_points).filter(|m| !m.is_zero())
    };
    // (rider, judge, total, percentage of the judge's maximum, exact to 2 decimals)
    let mut table: Vec<(String, AssignmentId, Decimal, Decimal)> = Vec::new();
    for line in lines.iter().filter(|l| l.team == teams) {
        let Some(max) = (if teams { class.programme.and_then(|p| state.programmes.get(&p)).map(|p| p.max_points).filter(|m| !m.is_zero()) } else { max_of(&line.id) }) else { continue };
        for judge in &judges {
            if let Some(total) = line.judge_totals.get(&judge.id.0) {
                table.push((line.id.clone(), judge.id.clone(), *total, round(*total / max * Decimal::ONE_HUNDRED, 2)));
            }
        }
    }
    let rank_of = |judge: &AssignmentId, percentage: Decimal| -> u32 {
        1 + table.iter().filter(|(_, j, _, p)| j == judge && *p > percentage).count() as u32
    };
    let mut out: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (rider, judge_id, total, percentage) in &table {
        let judge = &state.assignments[judge_id];
        let mut result = json!({
            "person_id": judge.person.0,
            "buchstabe": judge.letter.clone().unwrap_or_default(),
            "punkte": number(*total),
            "prozent": format!("{percentage:.2}"),
        });
        // Rosson ranks per judge inside its sort; with a single result the sort never runs and
        // the rank stays unset (docs/deviations.md E33). Kept: it is what the server receives.
        if table.iter().filter(|(_, j, _, _)| j == judge_id).count() > 1 {
            result["rang"] = json!(rank_of(judge_id, *percentage));
        }
        let children = !teams && programme_kind(state, class_id, rider, individual) == Some(ProgrammeKind::Children);
        if let Some(parts) = lines.iter().find(|l| l.team == teams && l.id == *rider).and_then(|l| l.judge_parts.get(&judge_id.0)) {
            if let Some(a) = parts.percent_a {
                result["prozent_a"] = json!(format!("{a:.2}"));
            }
            if let Some(b) = parts.percent_b {
                result["prozent_b"] = json!(format!("{b:.2}"));
            }
            if children {
                result["punkte_a"] = json!(format!("{:.1}", parts.a));
                result["punkte_b"] = json!(format!("{:.1}", parts.b));
            }
        }
        out.entry(rider.clone()).or_default().push(result);
    }
    // Children: one result for the judges, sent as part A by C and part B by E (the person is
    // the judge whose sheet it is).
    for (rider, results) in out.iter_mut() {
        if teams || programme_kind(state, class_id, rider, individual) != Some(ProgrammeKind::Children) || results.is_empty() {
            continue;
        }
        let one = results.iter().find(|r| r["punkte"].as_f64().is_some_and(|p| p != 0.0)).unwrap_or(&results[0]).clone();
        *results = [("C (A)", "a"), ("E (B)", "b")]
            .iter()
            .map(|(letter, part)| {
                let mut row = Map::new();
                row.insert("buchstabe".into(), json!(letter));
                row.insert("person_id".into(), one["person_id"].clone());
                for (to, from) in [("prozent", format!("prozent_{part}")), ("punkte", format!("punkte_{part}")), ("rang", "rang".to_string())] {
                    if let Some(v) = one.get(&from) {
                        row.insert(to.into(), v.clone());
                    }
                }
                Value::Object(row)
            })
            .collect();
    }
    out
}
