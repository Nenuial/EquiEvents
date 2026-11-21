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

/// The visitor's language, from the browser (German if it comes before French).
pub fn lang_of(headers: &HeaderMap) -> Lang {
    let accept = headers.get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok()).unwrap_or("").to_lowercase();
    match (accept.find("de"), accept.find("fr")) {
        (Some(de), Some(fr)) if de < fr => Lang::De,
        (Some(_), None) => Lang::De,
        _ => Lang::Fr,
    }
}

pub fn sign_in(lang: Lang) -> &'static str {
    lang.pick("/connexion", "/anmelden")
}

pub fn sign_out(lang: Lang) -> &'static str {
    lang.pick("/deconnexion", "/abmelden")
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
}
