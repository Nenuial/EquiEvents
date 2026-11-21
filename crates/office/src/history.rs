//! The show's history, as the office reads it: every change in the operation log, latest first,
//! said in the office's language, with who made it, when and on which device. A run of the same
//! kind of change by the same author (an import, a timekeeper's file) is one line.

use serde::Serialize;
use show::model::*;
use show::oplog::{Change, Operation};
use show::State;

use crate::prints::Language;

/// A line of the history.
#[derive(Debug, Clone, Serialize)]
pub struct HistoryLine {
    pub millis: u64,
    pub author: String,
    pub device: String,
    /// The kind of change (`ResultEdited`, …), for filtering.
    pub kind: String,
    pub text: String,
    /// How many changes the line stands for.
    pub count: usize,
}

/// The latest lines (at most `limit`), whose text contains every word of `query`.
pub fn history(log: &[Operation], state: &State, lang: Language, query: &str, limit: usize) -> Vec<HistoryLine> {
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_string).collect();
    let mut lines: Vec<HistoryLine> = Vec::new();
    for op in log.iter().rev() {
        let kind = kind(&op.change);
        let text = describe(&op.change, state, lang);
        if !words.is_empty() {
            let haystack = format!("{text} {}", op.author).to_lowercase();
            if !words.iter().all(|w| haystack.contains(w.as_str())) {
                continue;
            }
        }
        match lines.last_mut() {
            // Runs: the same kind by the same author, and either the same text or a bulk kind.
            Some(last) if last.kind == kind && last.author == op.author && (last.text == text || bulk(&kind)) => {
                last.count += 1;
                last.millis = op.stamp.millis;
            }
            _ => {
                if lines.len() == limit {
                    break;
                }
                lines.push(HistoryLine { millis: op.stamp.millis, author: op.author.clone(), device: op.stamp.device.clone(), kind, text, count: 1 });
            }
        }
    }
    for line in &mut lines {
        if line.count > 1 && bulk(&line.kind) {
            line.text = format!("{} (×{})", collective(&line.kind, lang).unwrap_or(&line.text), line.count);
        }
    }
    lines
}

fn kind(change: &Change) -> String {
    serde_json::to_value(change).ok().and_then(|v| v["kind"].as_str().map(str::to_string)).unwrap_or_default()
}

/// Kinds that come many at once (imports, deliveries, files).
fn bulk(kind: &str) -> bool {
    matches!(kind, "EntryDelivered" | "PersonsDelivered" | "AssignmentSet" | "ResultEntered" | "AssignmentLookedUp" | "PrivateDataDelivered" | "OfficialsDelivered")
}

fn collective(kind: &str, lang: Language) -> Option<&'static str> {
    let pick = |fr, de| Some(lang.pick(fr, de));
    match kind {
        "EntryDelivered" => pick("Inscriptions reçues de Swiss Equestrian", "Nennungen von Swiss Equestrian erhalten"),
        "AssignmentSet" => pick("Officiels attribués", "Offizielle zugeteilt"),
        "ResultEntered" => pick("Résultats saisis", "Resultate erfasst"),
        "AssignmentLookedUp" => pick("Officiels identifiés par Swiss Equestrian", "Offizielle von Swiss Equestrian identifiziert"),
        "PrivateDataDelivered" => pick("Coordonnées reçues", "Kontaktdaten erhalten"),
        _ => None,
    }
}

fn class_number(state: &State, class: &ClassId) -> String {
    state.classes.get(class).map_or_else(|| class.to_string(), |c| c.number.clone())
}

fn entry_name(state: &State, entry: &EntryId) -> String {
    let Some(e) = state.entries.get(entry) else { return format!("#{entry}") };
    let rider = state.persons.get(&e.rider).map(|p| format!("{} {}", p.first_name, p.last_name)).unwrap_or_default();
    let horse = e.guest_horse.clone().or_else(|| e.horse.as_ref().and_then(|h| state.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default();
    format!("{rider} / {horse}")
}

/// One change, said in the language.
pub fn describe(change: &Change, state: &State, lang: Language) -> String {
    let text = describe_fr_spaced(change, state, lang);
    // Written with French spacing (" : "); German and Italian put no space before a colon.
    if lang == Language::Fr { text } else { text.replace(" : ", ": ") }
}

fn describe_fr_spaced(change: &Change, state: &State, lang: Language) -> String {
    let t = |f: &str, d: &str| lang.pick(f, d).to_string();
    let class = |c: &ClassId| format!("{} {}", t("épreuve", "Prüfung"), class_number(state, c));
    match change {
        Change::EventImported { .. } => t("Concours importé", "Turnier importiert"),
        Change::RolesDelivered { .. } => t("Rôles des officiels reçus de Swiss Equestrian", "Funktionen der Offiziellen von Swiss Equestrian erhalten"),
        Change::OfficialsDelivered { .. } => t("Liste nationale des officiels reçue", "Nationale Liste der Offiziellen erhalten"),
        Change::PrivateDataDelivered { .. } => t("Coordonnées des participants reçues", "Kontaktdaten der Teilnehmer erhalten"),
        Change::EntryDelivered { entry } => format!("{} : {}", t("Inscription reçue", "Nennung erhalten"), entry_name(state, &entry.id)),
        Change::EntryEdited { entry, edit } => {
            let mut what = Vec::new();
            if let Some(s) = edit.status {
                what.push(if s == EntryStatus::NotStarting { t("ne part pas", "startet nicht") } else { t("part", "startet") });
            }
            if let Some(hc) = edit.hors_concours {
                what.push(if hc { "hc".into() } else { t("pas hc", "nicht hc") });
            }
            if edit.horse.is_some() {
                what.push(t("cheval", "Pferd"));
            }
            if edit.rider.is_some() {
                what.push(t("cavalier", "Reiter"));
            }
            if let Some(p) = &edit.published {
                what.push(format!("{} {}", t("vers l'épreuve", "in Prüfung"), state.published_classes.get(p).map_or_else(|| p.to_string(), |p| p.number.clone())));
            }
            if edit.stage.is_some() {
                what.push(t("niveau", "Stufe"));
            }
            if edit.comment.is_some() {
                what.push(t("note", "Notiz"));
            }
            if edit.horse_phases.is_some() {
                what.push(t("chevaux par épreuve partielle", "Pferde je Teilprüfung"));
            }
            format!("{} {} : {}", t("Inscription", "Nennung"), entry_name(state, entry), what.join(", "))
        }
        Change::EntryAdded { entry } => {
            // Said from the entry itself: it may have been taken back since.
            let rider = entry.rider_on_entry.as_ref().map(|r| format!("{} {}", r.first_name, r.last_name)).unwrap_or_default();
            let horse = entry.guest_horse.clone().or_else(|| entry.horses_on_entry.first().and_then(|h| h["pferdename"].as_str()).map(str::to_string)).unwrap_or_default();
            let class = state.published_classes.get(&entry.published).map(|p| p.number.clone()).unwrap_or_default();
            format!("{} : {rider} / {horse} ({} {class})", t("Inscription tardive", "Nachnennung"), t("épreuve", "Prüfung"))
        }
        Change::EntryRemoved { entry } => format!("{} #{entry}", t("Inscription retirée", "Nennung zurückgenommen")),
        Change::PersonAdded { person } => format!("{} : {} {}", t("Cavalier saisi", "Reiter erfasst"), person.first_name, person.last_name),
        Change::HorseAdded { horse } => format!("{} : {}", t("Cheval saisi", "Pferd erfasst"), horse.name),
        Change::ClassSet { class: c, .. } => format!("{} {} : {}", t("Épreuve", "Prüfung"), c.number, t("préparée ou modifiée", "vorbereitet oder geändert")),
        Change::ClassRemoved { class: c } => format!("{} {}", t("Épreuve supprimée", "Prüfung gelöscht"), class_number(state, c)),
        Change::StartsSet { class: c, .. } => format!("{} : {}", t("Liste de départ", "Startliste"), class(c)),
        Change::AssignmentSet { assignment } => {
            let who = state.persons.get(&assignment.person).map(|p| format!("{} {}", p.first_name, p.last_name)).or_else(|| state.officials.get(&assignment.person).map(|o| format!("{} {}", o.first_name, o.last_name))).unwrap_or_default();
            let place = assignment.class.as_ref().map(|c| format!(", {}", class(c))).unwrap_or_default();
            format!("{} : {who} ({}{}){place}", t("Officiel", "Offizielle:r"), assignment.role, assignment.letter.as_deref().map(|l| format!(" {l}")).unwrap_or_default())
        }
        Change::AssignmentRemoved { .. } => t("Rôle d'un officiel retiré", "Funktion einer/eines Offiziellen entfernt"),
        Change::ResultEntered { class: c, entry, .. } | Change::ResultEdited { class: c, entry, .. } => format!("{} {} : {}", t("Résultat", "Resultat"), class(c), entry_name(state, entry)),
        Change::ResultRemoved { class: c, entry } => format!("{} {} : {}", t("Résultat effacé", "Resultat gelöscht"), class(c), entry_name(state, entry)),
        Change::JudgeSheetEntered { class: c, entry, judge, .. } | Change::SheetEdited { class: c, entry, judge, .. } => {
            let letter = state.assignments.get(judge).and_then(|a| a.letter.clone()).unwrap_or_default();
            format!("{} {letter} {} : {}", t("Feuille du juge", "Richterblatt"), class(c), entry_name(state, entry))
        }
        Change::TeamSheetEdited { class: c, team, judge, .. } => {
            let letter = state.assignments.get(judge).and_then(|a| a.letter.clone()).unwrap_or_default();
            format!("{} {letter} {} : {}", t("Feuille du juge", "Richterblatt"), class(c), state.teams.get(team).map(|t| t.name.clone()).unwrap_or_default())
        }
        Change::SheetMoved { class: c, from, to } => {
            let what = match (from, to) {
                (_, SheetPlace::Aside { .. }) => t("Notes de juge coupées", "Richternoten ausgeschnitten"),
                (SheetPlace::Aside { .. }, _) => t("Notes de juge collées", "Richternoten eingefügt"),
                _ => t("Notes de juge déplacées", "Richternoten verschoben"),
            };
            format!("{what} : {}", class(c))
        }
        Change::SheetSet { class: c, sheet: None, .. } => format!("{} : {}", t("Notes de juge supprimées", "Richternoten gelöscht"), class(c)),
        Change::SheetSet { class: c, .. } => format!("{} : {}", t("Notes de juge remises", "Richternoten zurückgelegt"), class(c)),
        Change::TeamResultEntered { class: c, .. } => format!("{} {}", t("Résultat d'équipe", "Mannschaftsresultat"), class(c)),
        Change::JuryPenaltySet { class: c, entry, points } => format!("{} {} : {} ({})", t("Pénalité du jury", "Strafpunkte der Jury"), class(c), entry_name(state, entry), points.map_or_else(|| "–".into(), |p| p.normalize().to_string())),
        Change::RankSet { class: c, rank, reason, .. } => format!("{} {} : {} ({reason})", t("Rang attribué à la main", "Rang von Hand"), class(c), rank.map_or_else(|| "–".into(), |r| r.to_string())),
        Change::ClassFinalised { class: c, .. } => format!("{} {}", class(c), t("clôturée", "abgeschlossen")),
        Change::ClassReopened { class: c, reason } => format!("{} {} ({reason})", class(c), t("rouverte", "wieder geöffnet")),
        Change::Uploaded { upload } => {
            let what = match upload {
                Upload::Results(c) => format!("{} {}", t("résultats", "Resultate"), class(c)),
                Upload::StartLists(p) => format!("{} {}", t("listes de départ", "Startlisten"), state.published_classes.get(p).map_or_else(|| p.to_string(), |p| p.number.clone())),
                Upload::StartListsWithdrawn(p) => format!("{} {}", t("listes de départ retirées", "Startlisten zurückgezogen"), state.published_classes.get(p).map_or_else(|| p.to_string(), |p| p.number.clone())),
                Upload::Event => t("le concours", "das Turnier"),
                Upload::Combined(id) => format!("{} {}", t("classement", "Wertung"), state.combined.get(id).map_or_else(|| id.clone(), |r| r.name.clone())),
            };
            format!("{} : {what}", t("Envoyé à Swiss Equestrian", "An Swiss Equestrian gesendet"))
        }
        Change::CombinedSet { ranking } => format!("{} : {}", t("Classement sur plusieurs épreuves", "Klassement über mehrere Prüfungen"), ranking.name),
        Change::AddressSet { person, .. } => format!("{} : {}", t("Adresse", "Adresse"), state.persons.get(person).map(|p| format!("{} {}", p.first_name, p.last_name)).unwrap_or_default()),
        Change::TeamSet { team } => format!("{} : {}", t("Équipe", "Mannschaft"), team.name),
        Change::TeamRemoved { .. } => t("Équipe retirée", "Mannschaft entfernt"),
        Change::ProgrammeSet { programme } => format!("{} : {}", t("Définition du programme", "Definition der Aufgabe"), programme.name),
        Change::ScoringTableSet { table } => format!("{} : {}", t("Définition du barème", "Definition der Wertung"), table.code),
        Change::PublishedClassEdited { published, category_text, levels } => {
            let mut what = Vec::new();
            if let Some(text) = category_text {
                what.push(format!("{} « {text} »", t("catégorie", "Kategorie")));
            }
            if let Some(levels) = levels {
                what.push(format!("{levels} {}", t("niveaux", "Stufen")));
            }
            let number = state.published_classes.get(published).map_or_else(|| published.to_string(), |p| p.number.clone());
            format!("{} {number} : {}", t("Épreuve publiée", "Ausgeschriebene Prüfung"), what.join(", "))
        }
        Change::CombinedRemoved { .. } => t("Classement sur plusieurs épreuves supprimé", "Klassement über mehrere Prüfungen gelöscht"),
        other => kind(other),
    }
}
