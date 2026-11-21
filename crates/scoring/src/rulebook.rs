//! The rulebook: how every ranking works, in words, made from what the engine itself uses — the
//! rules written at the top of each module, and for show jumping each scoring table as the engine
//! reads it (its blocks, what they add up, what they compare, in which order).
//!
//! `docs/rulebook.md` is this text. A test rebuilds it from the code and fails when it differs
//! (`EQUIEVENTS_UPDATE_RULEBOOK=1` writes it anew), so any change to how a class is ranked shows
//! in the rulebook, where a person who knows the regulations can check it.

use crate::jumping::table::{regulation_override, Phases, ScoringTable, TableError};

/// The rules written at the top of each module, in the order the rulebook gives them.
const MODULES: [(&str, &str); 7] = [
    ("Ranks, ties and placed riders (all disciplines)", include_str!("ranking.rs")),
    ("Numbers and rounding", include_str!("number.rs")),
    ("Show jumping", include_str!("jumping/mod.rs")),
    ("Show jumping: teams", include_str!("jumping/team.rs")),
    ("Dressage", include_str!("dressage.rs")),
    ("Driving", include_str!("driving.rs")),
    ("Eventing", include_str!("eventing.rs")),
];

/// A scoring table as Swiss Equestrian publishes it, read or not.
pub struct TableSource<'a> {
    pub id: u32,
    pub code: &'a str,
    pub name: &'a str,
    pub table: Result<ScoringTable, TableError>,
}

/// The whole rulebook: the general rules, then show jumping's scoring tables one by one.
pub fn rulebook(tables: &[TableSource]) -> String {
    let mut out = String::from(
        "# Rulebook\n\n\
         How EquiEvents ranks every class, in words. **Generated from the code** \
         (`crates/scoring/src/rulebook.rs`): the rules as written at the top of each module of the \
         engine, and every show-jumping scoring table as the engine reads it. Do not edit by hand: \
         `EQUIEVENTS_UPDATE_RULEBOOK=1 cargo test -p scoring --test rulebook` writes it anew, and \
         the test fails whenever the code says something else than this text.\n\n\
         Decisions where EquiEvents differs from Rosson or settles a point the regulations leave \
         open are numbered S1, S2, … and explained in `docs/deviations.md`. Each rider's rank also \
         comes with why (`scoring::jumping::Why`, `scoring::ranking::Reason`): the values the \
         ranking compared, checked against the ranking itself in every test.\n",
    );
    for (title, source) in MODULES {
        out.push_str(&format!("\n## {title}\n\n{}\n", module_rules(source)));
    }
    out.push_str(
        "\n## Show jumping: every scoring table\n\n\
         A table is a list of parts (blocks), the most decisive first. A rider is ranked in the \
         first part for which all its results are entered; everyone ranked in a part is ahead of \
         everyone ranked in a part below it. In each part the values listed are added up and \
         compared in the order given. Times count to the hundredth, finer digits cut (S33); a \
         time that is missing or zero counts as the slowest. Riders equal in everything compared \
         have the same rank and are listed by horse name (S4). Out without a jump-off or a second \
         round to be equal last of: out, equal last of the class (S31).\n",
    );
    for t in tables {
        out.push_str(&format!("\n{}", table_rules(t)));
    }
    out
}

/// One scoring table, in words.
pub fn table_rules(source: &TableSource) -> String {
    let mut out = format!("### {} · {} — {}\n\n", source.id, source.code, source.name.trim());
    let table = match &source.table {
        Ok(t) => t,
        Err(e) => {
            out.push_str(&format!("**Not usable as published:** {}.\n", match e {
                TableError::Empty => "the definition is empty".to_string(),
                TableError::InvalidJson(why) => format!("the definition cannot be read ({why})"),
                TableError::Missing(what) => format!("the definition has no `{what}`"),
                TableError::NotCovered(what) => format!("not covered: {what}"),
            }));
            return out;
        }
    };
    if table.knock_out {
        let rounds = (1..=table.blocks.len()).map(|r| format!("round {r}")).collect::<Vec<_>>().join(", ");
        out.push_str(&format!(
            "**Knock-out** (S35): duels under Wertung C, {rounds} in riding order; each round's field \
             (`strafpunkte_initial_1`, then `strafpunkte_stechen_1` …) holds the duel's result, the time \
             with 3 s per fault. The rider who reached the furthest round ranks first; in the furthest \
             round the faster result wins; the losers of the same round are equal, eliminated or not \
             (1st, 2nd, two 3rd, four 5th). Eight are placed (Springreglement 4.5).\n"
        ));
        return out;
    }
    if let Some((decision, article)) = regulation_override(source.code) {
        out.push_str(&format!("**Defined by EquiEvents** ({decision}): the published definition contradicts the Springreglement ({article}); the regulation is followed.\n\n"));
    }
    out.push_str("| Part | Results needed | Compared, in order |\n|---|---|---|\n");
    for (i, b) in table.blocks.iter().enumerate() {
        // A points-based part does not need its time: missing, it is the slowest (S32).
        let needed = b.required.iter().filter(|f| !(b.points && b.time.contains(f))).map(|f| table.field_label(f, b.points)).collect::<Vec<_>>().join(", ");
        let sum = |fields: &[String], points: bool| fields.iter().map(|f| table.field_label(f, points)).collect::<Vec<_>>().join(" + ");
        let mut compared: Vec<String> = Vec::new();
        if !b.faults.is_empty() {
            compared.push(if b.points { format!("more points ({})", sum(&b.faults, true)) } else { format!("fewer faults ({})", sum(&b.faults, false)) });
        }
        if !b.time.is_empty() {
            compared.push(if b.ideal_time { format!("closer to the class's ideal time, over or under the same ({}) (S20)", sum(&b.time, false)) } else { format!("faster time ({})", sum(&b.time, false)) });
        }
        if compared.is_empty() {
            compared.push("nothing: everyone in this part is tied".into());
        }
        out.push_str(&format!("| {} | {needed} | {} |\n", table.block_label(i), compared.join("; then ")));
    }
    let mut notes: Vec<String> = Vec::new();
    // "round 2", "the jump-off".
    let the = |label: String| if label.ends_with(char::is_numeric) { label } else { format!("the {label}") };
    let jump_offs = table.blocks.iter().take_while(|b| b.required.iter().any(|f| f.contains("stechen"))).count();
    if table.winning_round {
        notes.push(format!(
            "**Winning round:** ridden by the placed riders after {rounds} (30 % of the entries, or the number the organiser set; S17). What it adds up is the table's own: the winning round part above. Withdrawn, retired or eliminated in it: equal last of the winning round. The others are ranked by {rounds}.",
            rounds = the(table.block_label(1)),
        ));
    } else if jump_offs > 0 {
        notes.push(format!(
            "**Jump-off{s}:** ridden by the riders equal first after {rounds} on {score} — time does not count for this (S13); with the championship option, also every group equal on score that includes a medal place (S14). Within such a group {first} decides{more}. Withdrawn, retired or eliminated in a jump-off: equal last of that jump-off, keeping those completed before it, ahead of the next group (S8, S9, S29); everyone out of it: nobody wins, all equal last of it (S30). No result and not out: after those who rode it; a jump-off nobody rode: all equal first. Everyone else is ranked by {rounds}.",
            s = if jump_offs > 1 { "s" } else { "" },
            rounds = the(table.block_label(jump_offs)),
            score = if table.blocks[jump_offs].faults.is_empty() { "time (table C: the time is the score)".to_string() } else if table.blocks[jump_offs].points { "points".into() } else { "faults".into() },
            first = the(table.block_label(jump_offs - 1)),
            more = match (jump_offs > 1, table.blocks[..jump_offs].iter().any(|b| !b.time.is_empty())) {
                (true, true) => ", then the next jump-off among those still equal, and so on; the time of the last jump-off they both completed comes last",
                (true, false) => ", then the next jump-off among those still equal, and so on (no time is taken)",
                (false, _) => "",
            },
        ));
    }
    match table.phases {
        Phases::Normal => notes.push(
            "**Two phases (normal):** only riders clear in phase 1 go on to phase 2, without a break. Eliminated or retired in phase 2: equal last of phase 2, ahead of the riders ranked by phase 1 — with faults in phase 1, out (S2, S16). Phase 2 follows without a break, so withdrawing is not possible: entered as withdrawn, it counts as retired.".into(),
        ),
        Phases::Special => notes.push("**Two phases (special):** everyone rides both phases. Eliminated, retired or withdrawn in either phase: out of the class (S2).".into()),
        Phases::None => {}
    }
    let second_round = table.phases == Phases::None && table.blocks.len() > 1 && table.blocks.iter().any(|b| b.required.iter().any(|f| f.ends_with("initial_2")));
    if second_round {
        notes.push("**Round 2:** eliminated or retired in round 2: equal last of round 2, ahead of the riders ranked by round 1. Withdrawn before round 2: ranked by round 1 (S16).".into());
    }
    if table.reduced_second_round {
        notes.push("**Placed:** only riders who completed both rounds (Springreglement 3.4.2.1; S34); if nobody rode round 2 (cancelled), round 1 places.".into());
    }
    if table.blocks.iter().any(|b| b.points) {
        notes.push("**Points:** the most points rank first (S1); a missing time is the slowest of equal points (S32).".into());
    }
    for n in notes {
        out.push_str(&format!("\n- {n}"));
    }
    out.push('\n');
    out
}

/// The `//!` lines at the top of a module, as Markdown: its headings one level down, rustdoc
/// links as plain code.
fn module_rules(source: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in source.lines() {
        let Some(text) = line.strip_prefix("//!") else {
            if lines.is_empty() {
                continue;
            }
            break;
        };
        let text = text.strip_prefix(' ').unwrap_or(text);
        let text = if text.starts_with('#') { format!("###{text}") } else { text.to_string() };
        lines.push(plain_links(&text));
    }
    lines.join("\n").trim().to_string()
}

/// "[`rank`]" → "`rank`", "[`x`](y)" → "`x`".
fn plain_links(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[`") {
        let Some(end) = rest[start..].find("`]") else { break };
        out.push_str(&rest[..start]);
        out.push_str(&rest[start + 1..start + end + 1]);
        rest = &rest[start + end + 2..];
        if let Some(stripped) = rest.strip_prefix('(') {
            if let Some(close) = stripped.find(')') {
                rest = &stripped[close + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_rules_read_the_header() {
        let text = module_rules("//! Title.\n//!\n//! # The rule\n//! See [`rank`] and [`x`](y).\n\nuse a;\n//! not this\n");
        assert_eq!(text, "Title.\n\n#### The rule\nSee `rank` and `x`.");
    }
}
