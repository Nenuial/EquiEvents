//! Limits on attempts (signing in, making accounts, asking for reset links, entering events'
//! keys), per client address, account or e-mail address, in memory: guessing passwords or keys,
//! or filling the database with accounts, is slowed down without bothering anyone at a show. A
//! restart forgets them.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A limit: at most `max` in `window`.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub max: usize,
    pub window: Duration,
}

const fn per_minutes(max: usize, minutes: u64) -> Rule {
    Rule { max, window: Duration::from_secs(minutes * 60) }
}

/// Signing in: per client, per account.
pub const LOGIN_CLIENT: Rule = per_minutes(30, 15);
pub const LOGIN_EMAIL: Rule = per_minutes(10, 15);
/// New accounts per client.
pub const REGISTER_CLIENT: Rule = per_minutes(10, 60);
/// Reset links: asked per client and per account; used (a new password) per client.
pub const RESET_CLIENT: Rule = per_minutes(10, 60);
pub const RESET_EMAIL: Rule = per_minutes(3, 60);
/// Entering events' keys (a show, a request) per account: keys are not guessed.
pub const KEY_USER: Rule = per_minutes(30, 60);
/// Downloads from Swiss Equestrian an account asks for (updates, riders and horses, demo events).
pub const SE_USER: Rule = per_minutes(30, 60);

#[derive(Default)]
pub struct Limiter {
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl Limiter {
    /// One more attempt for `key`: whether it is allowed (a refused one is not counted).
    pub fn allow(&self, key: &str, rule: Rule) -> bool {
        self.allow_at(key, rule, Instant::now())
    }

    fn allow_at(&self, key: &str, rule: Rule, now: Instant) -> bool {
        let Ok(mut hits) = self.hits.lock() else { return true };
        // Now and then, forget what is too old to count.
        if hits.len() > 10_000 {
            hits.retain(|_, times| times.last().is_some_and(|t| now.duration_since(*t) < Duration::from_secs(3600)));
        }
        let times = hits.entry(key.to_string()).or_default();
        times.retain(|t| now.duration_since(*t) < rule.window);
        if times.len() >= rule.max {
            return false;
        }
        times.push(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_most_so_many_in_a_while() {
        let l = Limiter::default();
        let rule = Rule { max: 3, window: Duration::from_secs(60) };
        let t0 = Instant::now();
        assert!((0..3).all(|_| l.allow_at("a", rule, t0)));
        assert!(!l.allow_at("a", rule, t0 + Duration::from_secs(30)));
        assert!(l.allow_at("b", rule, t0), "per key");
        assert!(l.allow_at("a", rule, t0 + Duration::from_secs(61)), "the window has passed");
    }
}
