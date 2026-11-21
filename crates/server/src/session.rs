//! Signed in on the site (the user, 2026-11-21: the online version and the download page only
//! for signed-in people): a cookie holding a token of the account (`auth`, a session like an
//! app's, valid a year after its last use), set when signing in or making an account on the
//! site. `HttpOnly` (no script reads it), `SameSite=Lax` (another site's form or script does not
//! send it), `Secure` when the site is served over HTTPS.

use axum::http::{header, HeaderMap, HeaderValue};

use crate::lang::Lang;

pub const COOKIE: &str = "equievents_session";
/// A year, as the token.
const MAX_AGE: u32 = 365 * 24 * 3600;

/// The session's token, from the request's cookies.
pub fn token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.trim().to_string())
        .filter(|t| !t.is_empty() && t.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// The cookie that signs in (`secure`: the site is on HTTPS).
pub fn set(token: &str, secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={MAX_AGE}{secure}")).unwrap_or(HeaderValue::from_static(""))
}

/// The cookie that signs out.
pub fn clear(secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}")).unwrap_or(HeaderValue::from_static(""))
}

/// The language last chosen on the site (a page in that language seen: `lang_cookie`), for the
/// home page and the online version. Not a secret: the app reads it too.
pub const LANG_COOKIE: &str = "equievents_lang";

/// The visitor's language (the user, 2026-11-21): the one they last chose on the site, else the
/// first of French, German and Italian their browser asks for, else German.
pub fn lang_of(headers: &HeaderMap) -> Lang {
    let chosen = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(name, _)| *name == LANG_COOKIE)
        .and_then(|(_, value)| Lang::parse(value));
    let accept = || headers.get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok()).and_then(Lang::from_accept);
    chosen.or_else(accept).unwrap_or(Lang::De)
}

/// The cookie keeping the language of the page seen (a year; a preference, nothing secret).
pub fn lang_cookie(lang: Lang) -> HeaderValue {
    HeaderValue::from_str(&format!("{LANG_COOKIE}={}; Path=/; SameSite=Lax; Max-Age={MAX_AGE}", lang.code())).unwrap_or(HeaderValue::from_static(""))
}

pub fn sign_in(lang: Lang) -> &'static str {
    lang.pick("/connexion", "/anmelden", "/accedi")
}

pub fn sign_out(lang: Lang) -> &'static str {
    lang.pick("/deconnexion", "/abmelden", "/esci")
}

/// The sign-in, coming back to `next` after.
pub fn sign_in_path(lang: Lang, next: &str) -> String {
    format!("{}?next={}", sign_in(lang), encode(next))
}

/// Where to go after signing in: a page of this site only (never another site), else the
/// online version.
pub fn next_or_app(next: Option<&str>) -> String {
    match next {
        Some(n) if n.starts_with('/') && !n.starts_with("//") && !n.contains(['\\', '\r', '\n']) => n.to_string(),
        _ => "/app/".into(),
    }
}

fn encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cookie_and_where_to_go() {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static("other=1; equievents_session=ab12cd; x=y"));
        assert_eq!(token(&headers).as_deref(), Some("ab12cd"));
        headers.insert(header::COOKIE, HeaderValue::from_static("equievents_session=<script>"));
        assert_eq!(token(&headers), None);
        assert_eq!(next_or_app(Some("/telecharger")), "/telecharger");
        for elsewhere in ["//evil.example", "https://evil.example", "/\\evil", ""] {
            assert_eq!(next_or_app(Some(elsewhere)), "/app/", "{elsewhere}");
        }
        assert_eq!(sign_in_path(Lang::De, "/app/"), "/anmelden?next=/app/");
    }

    #[test]
    fn the_visitors_language() {
        let with = |pairs: &[(header::HeaderName, &'static str)]| {
            let mut h = HeaderMap::new();
            for (k, v) in pairs {
                h.append(k.clone(), HeaderValue::from_static(v));
            }
            lang_of(&h)
        };
        assert_eq!(with(&[]), Lang::De);
        assert_eq!(with(&[(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")]), Lang::De);
        assert_eq!(with(&[(header::ACCEPT_LANGUAGE, "it-CH,it;q=0.9,de;q=0.8")]), Lang::It);
        assert_eq!(with(&[(header::ACCEPT_LANGUAGE, "fr-CH"), (header::COOKIE, "equievents_lang=de")]), Lang::De);
        assert_eq!(with(&[(header::ACCEPT_LANGUAGE, "fr-CH"), (header::COOKIE, "equievents_lang=xx")]), Lang::Fr);
    }
}
