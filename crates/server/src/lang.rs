//! The two languages of the site and the e-mails.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Fr,
    De,
}

impl Lang {
    pub fn pick(self, fr: &'static str, de: &'static str) -> &'static str {
        match self {
            Lang::Fr => fr,
            Lang::De => de,
        }
    }

    pub fn code(self) -> &'static str {
        self.pick("fr", "de")
    }

    pub fn other(self) -> Lang {
        match self {
            Lang::Fr => Lang::De,
            Lang::De => Lang::Fr,
        }
    }

    /// `de` for German, else French.
    pub fn of(code: &str) -> Lang {
        if code.trim().eq_ignore_ascii_case("de") {
            Lang::De
        } else {
            Lang::Fr
        }
    }
}
