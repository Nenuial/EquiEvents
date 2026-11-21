//! Ranks, ties and placed riders, shared by all disciplines.
//!
//! - Entries are ranked in order; the first gets rank 1.
//! - An entry tied with the one before it gets the same rank, and the next rank skips:
//!   1, 2, 2, 4 ("competition ranking").
//! - Eliminated or retired entries are *out*: listed after everyone who is ranked, all equal.
//!   Rosson stored them as rank 99; that number only survives at the API boundary.
//! - Tied entries are listed alphabetically by horse name (docs/deviations.md S4).
//! - "Placed" (klassiert) entries are those ranked within the placed count: by default 30 % of
//!   the results, rounded up, unless the organiser sets the number (S10).
//! - The show office can set a rank by hand ([`with_manual_ranks`], S18); the others' ranks skip
//!   the numbers given by hand.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::number::Decimal;

/// One thing a ranking compares, with an entry's value: what it is, the value, and which way
/// ranks first. An entry's reasons, in the order compared, are why it stands where it does
/// (dressage, driving, eventing; jumping has its own, [`crate::jumping::Why`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Reason {
    pub what: &'static str,
    pub value: Decimal,
    pub higher_first: bool,
}

/// Reasons in words: "percentage 71.72; then artistic (B) percentage 72.50".
pub fn reasons_text(reasons: &[Reason]) -> String {
    reasons.iter().map(|r| format!("{} {}", r.what, r.value)).collect::<Vec<_>>().join("; then ")
}

/// Where an entry ends up in a class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Placement {
    /// Ranked; 1 is the winner. Tied entries share the rank.
    Rank(u32),
    /// Eliminated, retired or otherwise out of the class: listed after all ranked entries.
    Out,
}

impl Placement {
    pub fn rank(self) -> Option<u32> {
        match self {
            Placement::Rank(r) => Some(r),
            Placement::Out => None,
        }
    }
}

/// Competition ranks for entries that are already in ranking order.
///
/// `tied(previous, current)` says whether `current` has exactly the same result as the entry
/// listed before it. Returns one rank per entry, in the same order.
pub fn competition_ranks<T>(ordered: &[T], tied: impl Fn(&T, &T) -> bool) -> Vec<u32> {
    let mut ranks: Vec<u32> = Vec::with_capacity(ordered.len());
    for (position, entry) in ordered.iter().enumerate() {
        let rank = match position.checked_sub(1) {
            Some(prev) if tied(&ordered[prev], entry) => ranks[prev],
            _ => position as u32 + 1,
        };
        ranks.push(rank);
    }
    ranks
}

/// The office's manual ranks (docs/deviations.md S18) applied to a ranking in listing order.
///
/// - An entry with a manual rank gets that rank, also when it was out.
/// - The other ranked entries keep their order and their ties; their ranks skip every number
///   given by hand. Amigo 1, Bijou 2, Cassis 2, Dandy 4, and the office gives Dandy rank 2:
///   Amigo 1, Dandy 2, Bijou 3, Cassis 3. A rank given to two riders takes two places, as any
///   tie: two riders set to 1, the next is 3.
/// - Entries that are out stay out.
///
/// `manual` maps entry ids to ranks; ids not in the listing are ignored. Returns the listing in
/// its new order: by rank (an entry ranked by hand first among equal ranks), then those out.
pub fn with_manual_ranks(listing: &[(String, Placement)], manual: &BTreeMap<String, u32>) -> Vec<(String, Placement)> {
    let manual: BTreeMap<&String, u32> = listing.iter().filter_map(|(id, _)| manual.get(id).map(|r| (id, *r))).collect();
    if manual.is_empty() {
        return listing.to_vec();
    }
    // A rank set by hand for n riders takes n places, as any tie: two riders set to 1 take 1 and 2.
    let mut taken: BTreeSet<u32> = BTreeSet::new();
    for r in manual.values() {
        let mut place = *r;
        while !taken.insert(place) {
            place += 1;
        }
    }
    let others: Vec<&(String, Placement)> = listing.iter().filter(|(id, _)| !manual.contains_key(id)).collect();

    let mut result: Vec<(String, Placement, bool)> = Vec::with_capacity(listing.len());
    let mut next = 1u32;
    let mut k = 0;
    while k < others.len() {
        let placement = others[k].1;
        let end = k + others[k..].iter().take_while(|o| o.1 == placement).count();
        let new = match placement {
            Placement::Rank(_) => {
                while taken.contains(&next) {
                    next += 1;
                }
                let rank = next;
                next += (end - k) as u32; // competition ranking: 1, 1, 3
                Placement::Rank(rank)
            }
            Placement::Out => Placement::Out,
        };
        result.extend(others[k..end].iter().map(|o| (o.0.clone(), new, false)));
        k = end;
    }
    result.extend(manual.iter().map(|(id, r)| ((*id).clone(), Placement::Rank(*r), true)));

    let position = |id: &String| listing.iter().position(|(i, _)| i == id).unwrap_or(usize::MAX);
    result.sort_by_key(|(id, p, by_hand)| (*p, !*by_hand, position(id)));
    result.into_iter().map(|(id, p, _)| (id, p)).collect()
}

/// Within each group of tied entries (same rank), list them alphabetically by horse name
/// (docs/deviations.md S4). Entries must already be in ranking order; ranks are unchanged.
pub fn order_ties_by_horse<T>(entries: &mut [T], rank: impl Fn(&T) -> u32, horse: impl Fn(&T) -> &str) {
    let mut start = 0;
    while start < entries.len() {
        let r = rank(&entries[start]);
        let mut end = start + 1;
        while end < entries.len() && rank(&entries[end]) == r {
            end += 1;
        }
        entries[start..end].sort_by(|a, b| compare_names(horse(a), horse(b)));
        start = end;
    }
}

/// Alphabetical order for names as people expect it: case-insensitive, accents ignored for the
/// comparison ("Éclair" next to "Eclair"), ties broken by the exact text so the order is stable.
pub fn compare_names(a: &str, b: &str) -> Ordering {
    fold(a).cmp(&fold(b)).then_with(|| a.cmp(b))
}

fn fold(name: &str) -> String {
    name.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            other => other,
        })
        .collect()
}

/// Default number of placed entries: 30 % of the results, rounded up (Rosson's rule,
/// `ResultatService.anzahlKlassiertFuer`; docs/deviations.md S10).
pub fn default_placed_count(results: usize) -> usize {
    (results * 3).div_ceil(10)
}

/// Whether a placement is within the placed count.
pub fn is_placed(placement: Placement, placed_count: usize) -> bool {
    matches!(placement, Placement::Rank(r) if (r as usize) <= placed_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ties_share_a_rank_and_the_next_rank_skips() {
        let scores = [10, 9, 9, 7, 7, 7, 1];
        assert_eq!(competition_ranks(&scores, |a, b| a == b), vec![1, 2, 2, 4, 4, 4, 7]);
    }

    #[test]
    fn placed_count_is_30_percent_rounded_up() {
        assert_eq!(default_placed_count(39), 12);
        assert_eq!(default_placed_count(10), 3);
        assert_eq!(default_placed_count(1), 1);
        assert_eq!(default_placed_count(0), 0);
    }

    #[test]
    fn tied_entries_are_listed_by_horse_name() {
        let mut entries = vec![(1, "Zorro"), (2, "Éclair"), (2, "Amigo"), (2, "elf"), (5, "Bijou")];
        order_ties_by_horse(&mut entries, |e| e.0, |e| e.1);
        assert_eq!(entries, vec![(1, "Zorro"), (2, "Amigo"), (2, "Éclair"), (2, "elf"), (5, "Bijou")]);
    }
}
