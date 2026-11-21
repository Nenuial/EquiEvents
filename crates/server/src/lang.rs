//! The languages of the site and the e-mails: French, German, Italian (the app's three).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Fr,
    De,
    It,
}

impl Lang {
    /// All of them, in the language switch's order.
    pub const ALL: [Lang; 3] = [Lang::Fr, Lang::De, Lang::It];

    pub fn pick(self, fr: &'static str, de: &'static str, it: &'static str) -> &'static str {
        match self {
            Lang::Fr => fr,
            Lang::De => de,
            Lang::It => it,
        }
    }

    pub fn code(self) -> &'static str {
        self.pick("fr", "de", "it")
    }

    /// `fr`, `de` or `it` (any case); anything else: none.
    pub fn parse(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| code.trim().eq_ignore_ascii_case(l.code()))
    }

    /// A stored or sent language (`fr`, `de`, `it`); anything else French, as accounts made
    /// before it was asked.
    pub fn of(code: &str) -> Lang {
        Lang::parse(code).unwrap_or(Lang::Fr)
    }

    /// The first of the three a browser asks for (`Accept-Language`, by preference: `q`), if any.
    pub fn from_accept(accept: &str) -> Option<Lang> {
        let mut asked: Vec<(f32, Lang)> = accept
            .split(',')
            .filter_map(|part| {
                let mut bits = part.split(';');
                let tag = bits.next()?.trim();
                let lang = Lang::parse(tag.split(['-', '_']).next()?)?;
                let q = bits.find_map(|b| b.trim().strip_prefix("q=")).map_or(Some(1.0), |q| q.trim().parse::<f32>().ok())?;
                (q > 0.0).then_some((q, lang))
            })
            .collect();
        // Stable: equal preferences keep the browser's order.
        asked.sort_by(|a, b| b.0.total_cmp(&a.0));
        asked.first().map(|(_, l)| *l)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_browser_asks_for() {
        assert_eq!(Lang::from_accept("fr-CH,fr;q=0.9,en;q=0.8"), Some(Lang::Fr));
        assert_eq!(Lang::from_accept("en-US,en;q=0.9,it-CH;q=0.8,de;q=0.7"), Some(Lang::It));
        assert_eq!(Lang::from_accept("de;q=0.5, fr;q=0.8"), Some(Lang::Fr));
        assert_eq!(Lang::from_accept("en-GB,en;q=0.9"), None);
        assert_eq!(Lang::from_accept("fr;q=0, de"), Some(Lang::De));
        assert_eq!(Lang::from_accept(""), None);
        assert_eq!((Lang::of("IT"), Lang::of("xx")), (Lang::It, Lang::Fr));
    }
}
