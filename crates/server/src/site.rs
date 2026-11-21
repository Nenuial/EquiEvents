//! The site at www.equievents.ch: anyone makes their own account there (an account alone opens
//! nothing: a show needs its Swiss Equestrian keys), signs in (`session`: for the online version
//! and the desktop app's downloads), and resets a forgotten password with a link sent by
//! e-mail. French and German pages, plain HTML forms (no script), for phones too:
//!
//! | French | German | |
//! |---|---|---|
//! | `/` | | home, both languages: signed in, the online version and the downloads |
//! | `/connexion` | `/anmelden` | sign in (`?next=`: a page of the site to go to after) |
//! | `/deconnexion` | `/abmelden` | sign out |
//! | `/inscription` | `/registrieren` | make an account (signed in at once) |
//! | `/mot-de-passe` | `/passwort` | a forgotten password: the link by e-mail |
//! | `/nouveau-mot-de-passe?t=…` | `/neues-passwort?t=…` | the link: a new password |
//! | `/telecharger` | `/herunterladen` | the desktop app (`downloads`) |

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Form, Router};
use serde::Deserialize;

use crate::api::{App, Client};
use crate::auth::{self, Invalid};
use crate::limit;
use crate::session;

pub use crate::lang::Lang;

/// The pages' paths in each language.
#[derive(Debug, Clone, Copy)]
enum Path {
    Register,
    Forgot,
    Reset,
}

impl Path {
    fn of(self, lang: Lang) -> &'static str {
        match self {
            Path::Register => lang.pick("/inscription", "/registrieren"),
            Path::Forgot => lang.pick("/mot-de-passe", "/passwort"),
            Path::Reset => lang.pick("/nouveau-mot-de-passe", "/neues-passwort"),
        }
    }
}

pub fn routes() -> Router<App> {
    Router::new()
        .route("/", get(home))
        .route("/site/inter.woff2", get(font))
        .route("/connexion", get(|s, h, q| sign_in_form(Lang::Fr, s, h, q)).post(|s, c, f| sign_in(Lang::Fr, s, c, f)))
        .route("/anmelden", get(|s, h, q| sign_in_form(Lang::De, s, h, q)).post(|s, c, f| sign_in(Lang::De, s, c, f)))
        .route("/deconnexion", get(|s, h| sign_out(Lang::Fr, s, h)))
        .route("/abmelden", get(|s, h| sign_out(Lang::De, s, h)))
        .route("/inscription", get(|| register_form(Lang::Fr)).post(|s, c, f| register(Lang::Fr, s, c, f)))
        .route("/registrieren", get(|| register_form(Lang::De)).post(|s, c, f| register(Lang::De, s, c, f)))
        .route("/mot-de-passe", get(|| forgot_form(Lang::Fr)).post(|s, c, f| forgot(Lang::Fr, s, c, f)))
        .route("/passwort", get(|| forgot_form(Lang::De)).post(|s, c, f| forgot(Lang::De, s, c, f)))
        .route("/nouveau-mot-de-passe", get(|s, q| reset_form(Lang::Fr, s, q)).post(|s, c, f| reset(Lang::Fr, s, c, f)))
        .route("/neues-passwort", get(|s, q| reset_form(Lang::De, s, q)).post(|s, c, f| reset(Lang::De, s, c, f)))
}

/// The page to make an account, in each language.
pub fn register_path(lang: Lang) -> &'static str {
    Path::Register.of(lang)
}

/// Text for HTML.
pub fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// The site's look is the app's (docs/ux/design.md; the user, 2026-11-21: "like the app"): the
/// navy band with the logo, the language switch and the page's title, its card below; Inter (the
/// app's typeface, `/site/inter.woff2`); light or dark as the system is. No script.
const STYLE: &str = r#"
@font-face{font-family:Inter;font-style:normal;font-weight:100 900;font-display:swap;src:url(/site/inter.woff2) format("woff2")}
:root{--bg:#f3f5f8;--surface:#fff;--line:#e4e9f0;--line-strong:#d3dbe5;--ink:#0d1b2a;--ink-2:#3a4859;--ink-3:#66768a;--ink-4:#98a5b5;--hover:#eef2f6;--accent:#e5484d;--accent-strong:#cc3a3f;--on-accent:#fff;--mark:#e5484d;--ok:#138a4c;--danger:#d32f3a;--info:#1f6fc4;--nav:#00283c;--shadow:16 32 48;color-scheme:light}
@media (prefers-color-scheme:dark){:root{--bg:#060f17;--surface:#0c1824;--line:#1b3043;--line-strong:#264057;--ink:#e9eff6;--ink-2:#b8c6d4;--ink-3:#8195a9;--ink-4:#5d7186;--hover:#132637;--accent:#ef5559;--accent-strong:#ff7477;--ok:#3ccf8a;--danger:#ff6370;--info:#5aa8ff;--nav:#0a1b29;--shadow:0 0 0;color-scheme:dark}}
*{box-sizing:border-box}
html,body{margin:0}
body{background:var(--bg);color:var(--ink);font:15px/1.5 Inter,ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;-webkit-font-smoothing:antialiased}
.band{position:relative;overflow:hidden;background:var(--nav);color:#fff;padding:max(1rem,env(safe-area-inset-top)) 1rem 6.5rem}
.band::before{content:"";position:absolute;inset:0;pointer-events:none;background:radial-gradient(80% 120% at 100% 0%,rgb(242 90 93/.22),transparent 55%),radial-gradient(60% 90% at 0% 100%,rgb(31 111 196/.18),transparent 60%)}
.bar,.head{position:relative;margin:0 auto}
.bar{max-width:72rem;display:flex;align-items:center;justify-content:space-between;gap:1rem;padding:.25rem 0}
.brand{display:inline-flex;align-items:center;gap:.6rem;color:#fff;text-decoration:none;font-weight:700;font-size:17px;letter-spacing:-.01em}
.brand svg{width:2rem;height:2rem;flex:none}
.brand span{color:var(--mark)}
.langs{display:inline-flex;gap:2px;padding:3px;border-radius:9px;background:rgb(255 255 255/.08)}
.langs a,.langs span{display:inline-block;min-width:2.4rem;padding:.35rem .55rem;border-radius:7px;font-size:12px;font-weight:700;text-align:center;color:rgb(255 255 255/.55);text-decoration:none}
.langs a:hover{color:#fff}
.langs span{background:#fff;color:#0d1b2a}
.head{max-width:28rem;padding-top:3.25rem}
.head h1{margin:0;font-size:32px;line-height:1.15;font-weight:700;letter-spacing:-.025em;text-wrap:balance}
.head p{margin:.65rem 0 0;color:rgb(255 255 255/.72);font-size:15px}
main{position:relative;max-width:28rem;margin:-4rem auto 0;padding:0 1rem max(2.5rem,env(safe-area-inset-bottom))}
.card{background:var(--surface);border:1px solid var(--line);border-radius:16px;padding:1.6rem;box-shadow:0 1px 2px rgb(var(--shadow)/.06),0 10px 28px -14px rgb(var(--shadow)/.22)}
.card>:first-child{margin-top:0}
p{margin:.6rem 0}
.muted{color:var(--ink-2);font-size:15px}
.tabs{display:flex;gap:4px;padding:4px;border-radius:10px;background:var(--hover);margin:1rem 0}
.tabs a,.tabs span{flex:1;padding:.55rem .75rem;border-radius:7px;text-align:center;font-weight:600;font-size:15px;color:var(--ink-3);text-decoration:none}
.tabs a:hover{color:var(--ink)}
.tabs span{background:var(--surface);color:var(--ink);box-shadow:0 1px 3px rgb(var(--shadow)/.14)}
label{display:block;font-weight:600;font-size:15px;margin:1rem 0 .4rem}
input{width:100%;height:2.85rem;font:inherit;padding:0 .8rem;border:1px solid var(--line-strong);border-radius:9px;background:var(--surface);color:var(--ink)}
input:focus{outline:none;border-color:var(--info);box-shadow:0 0 0 3px rgb(31 111 196/.2)}
.hint{color:var(--ink-3);font-size:13px;margin-top:.35rem}
button,.button{display:flex;align-items:center;justify-content:center;width:100%;min-height:2.85rem;margin-top:1.25rem;padding:.6rem 1rem;border:0;border-radius:9px;background:var(--accent);color:var(--on-accent);font:inherit;font-weight:600;text-align:center;text-decoration:none;cursor:pointer}
button:hover,.button:hover{background:var(--accent-strong)}
.second{background:transparent;color:var(--ink);border:1px solid var(--line-strong)}
.second:hover{background:var(--hover)}
.error{color:var(--danger);font-weight:600}
.done{color:var(--ok);font-weight:600}
.trap{position:absolute;left:-10000px;width:1px;height:1px;overflow:hidden}
a{color:var(--accent)}
.links{margin:1rem 0 0;font-size:15px}
.links a{color:var(--ink-3);text-decoration:none}
.links a:hover{color:var(--ink)}
footer{margin-top:1rem;text-align:center;color:var(--ink-4);font-size:13px}
footer a{color:var(--ink-3)}
h2{font-size:16px;margin:0}
.platform{margin-top:1.5rem;padding-top:1.25rem;border-top:1px solid var(--line)}
.platform .button{margin-top:.75rem}
.steps{padding-left:1.25rem;margin:.75rem 0 0;color:var(--ink-2);font-size:14px}
.steps li{margin:.3rem 0}
.then{margin-top:1.5rem;padding-top:1.25rem;border-top:1px solid var(--line)}
.group{margin-top:1.25rem}
.group .button{margin-top:.6rem}
:focus-visible{outline:2px solid var(--info);outline-offset:2px}
"#;

/// The app's logo (`app/src/ui/Logo.tsx`): the E on its tile.
const LOGO: &str = r##"<svg viewBox="0 0 32 32" aria-hidden="true"><rect width="32" height="32" rx="9" style="fill:var(--mark)"/><path d="M22.5 9.5H16a6.5 6.5 0 0 0 0 13h6.5M13 16h7.5" fill="none" stroke="#fff" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;

/// The app's typeface (Inter, SIL Open Font License: `assets/inter-LICENSE.txt`).
const INTER: &[u8] = include_bytes!("../assets/inter-latin.woff2");

async fn font() -> Response {
    ([(header::CONTENT_TYPE, "font/woff2"), (header::CACHE_CONTROL, "public, max-age=31536000, immutable")], INTER).into_response()
}

/// A whole page: the band (the logo, the language switch, the page's heading and the line under
/// it: the body's first `<h1>` and `<p class="muted">`, else `title`), the rest in the card.
pub(crate) fn page(lang: Lang, title: &str, switch_to: Option<&str>, body: &str) -> Response {
    let (heading, rest) = match body.trim_start().strip_prefix("<h1>").and_then(|b| b.split_once("</h1>")) {
        Some((h, rest)) => (h.to_string(), rest.trim_start()),
        None => (esc(title), body),
    };
    let (under, rest) = match rest.strip_prefix(r#"<p class="muted">"#).and_then(|b| b.split_once("</p>")) {
        Some((p, rest)) => (format!("<p>{p}</p>"), rest.trim_start()),
        None => (String::new(), rest),
    };
    // FR · DE, this page's language marked; the other one when the page has it.
    let langs = [Lang::Fr, Lang::De]
        .iter()
        .map(|l| match (*l == lang, switch_to) {
            (true, _) => format!(r#"<span lang="{0}">{1}</span>"#, l.code(), l.code().to_uppercase()),
            (false, Some(href)) => format!(r#"<a href="{href}" hreflang="{0}" lang="{0}">{1}</a>"#, l.code(), l.code().to_uppercase()),
            (false, None) => String::new(),
        })
        .collect::<String>();
    let html = format!(
        r##"<!doctype html><html lang="{code}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover"><meta name="theme-color" content="#00283c"><title>{title} · EquiEvents</title><style>{STYLE}</style></head><body><header class="band"><div class="bar"><a class="brand" href="/">{LOGO}<b>Equi<span>Events</span></b></a><nav class="langs">{langs}</nav></div><div class="head"><h1>{heading}</h1>{under}</div></header><main><div class="card">{rest}</div></main></body></html>"##,
        code = lang.code(),
        title = esc(title),
    );
    let mut res = (StatusCode::OK, html).into_response();
    let headers = res.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    // Nothing from elsewhere, no frames, no address (reset links!) passed on to other sites.
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; font-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

fn with_status(status: StatusCode, mut res: Response) -> Response {
    *res.status_mut() = status;
    res
}

fn invalid_text(lang: Lang, e: Invalid) -> String {
    let forgot = Path::Forgot.of(lang);
    match e {
        Invalid::Name => lang.pick("Votre nom, s’il vous plaît.", "Bitte Ihren Namen angeben.").into(),
        Invalid::Email => lang.pick("Cette adresse e-mail n’est pas valable.", "Diese E-Mail-Adresse ist ungültig.").into(),
        Invalid::Password => lang.pick("Le mot de passe doit compter de 10 à 200 caractères.", "Das Passwort hat 10 bis 200 Zeichen.").into(),
        Invalid::Taken => format!(
            "{} <a href=\"{forgot}\">{}</a>",
            lang.pick("Cette adresse a déjà un compte.", "Für diese Adresse besteht schon ein Konto."),
            lang.pick("Mot de passe oublié ?", "Passwort vergessen?")
        ),
        Invalid::Link => lang.pick("Ce lien n’est plus valable (une heure, une seule fois).", "Dieser Link ist nicht mehr gültig (eine Stunde, nur einmal).").into(),
    }
}

fn too_many(lang: Lang) -> Response {
    let body = format!(r#"<h1>{}</h1><p>{}</p>"#, lang.pick("Trop d’essais", "Zu viele Versuche"), lang.pick("Réessayez dans un moment.", "Bitte versuchen Sie es später wieder."));
    with_status(StatusCode::TOO_MANY_REQUESTS, page(lang, lang.pick("Trop d’essais", "Zu viele Versuche"), None, &body))
}

pub(crate) fn server_error(lang: Lang, e: crate::db::Error) -> Response {
    eprintln!("error: {e}");
    let body = format!(r#"<h1>{}</h1><p>{}</p>"#, lang.pick("Un problème est survenu", "Ein Problem ist aufgetreten"), lang.pick("Le serveur n’a pas pu traiter la demande. Réessayez plus tard.", "Der Server konnte es nicht ausführen. Bitte später wieder versuchen."));
    with_status(StatusCode::INTERNAL_SERVER_ERROR, page(lang, lang.pick("Un problème est survenu", "Ein Problem ist aufgetreten"), None, &body))
}

async fn home(State(app): State<App>, headers: HeaderMap) -> Response {
    let signed_in = match session::token(&headers) {
        Some(token) => auth::user_of(&app.pool, &token).await.ok().flatten(),
        None => None,
    };
    let group = |lang: Lang| match &signed_in {
        // Signed in: the online version, the desktop app, signing out.
        Some(_) => format!(
            r#"<div class="group" lang="{code}"><a class="button" href="/app/">{open}</a><a class="button second" href="{download}">{download_label}</a></div>"#,
            code = lang.code(),
            open = lang.pick("Ouvrir EquiEvents en ligne", "EquiEvents online öffnen"),
            download = crate::downloads::path(lang),
            download_label = lang.pick("Télécharger l’application", "App herunterladen"),
        ),
        None => format!(
            r#"<div class="group" lang="{code}"><a class="button" href="{sign_in}">{sign_in_label}</a><a class="button second" href="{register}">{register_label}</a></div>"#,
            code = lang.code(),
            sign_in = session::sign_in(lang),
            sign_in_label = lang.pick("Se connecter", "Anmelden"),
            register = Path::Register.of(lang),
            register_label = lang.pick("Créer un compte", "Konto erstellen"),
        ),
    };
    let footer = match &signed_in {
        Some(user) => format!(
            r#"<footer class="muted">{} · <a href="{}" lang="fr">Se déconnecter</a> · <a href="{}" lang="de">Abmelden</a></footer>"#,
            esc(&user.name),
            session::sign_out(Lang::Fr),
            session::sign_out(Lang::De),
        ),
        None => format!(
            r#"<footer class="muted"><a href="{}" lang="fr">Mot de passe oublié ?</a> · <a href="{}" lang="de">Passwort vergessen?</a></footer>"#,
            Path::Forgot.of(Lang::Fr),
            Path::Forgot.of(Lang::De),
        ),
    };
    let body = format!(
        r#"<h1>Le secrétariat de concours · Das Turniersekretariat</h1>
<p class="muted">Concours hippiques de Swiss Equestrian · Pferdesportturniere von Swiss Equestrian</p>
{}{}
{footer}"#,
        group(Lang::Fr),
        group(Lang::De),
    );
    page(Lang::Fr, "EquiEvents", None, &body)
}

// --- Signing in and out --------------------------------------------------------------------------

/// The head of signing in and making an account, as the app's welcome screen: its title, the
/// line under it, what an account is for.
fn welcome(lang: Lang) -> String {
    format!(
        r#"<h1>{}</h1>
<p class="muted">{}</p>
<p class="muted">{}</p>"#,
        lang.pick("Bienvenue dans EquiEvents", "Willkommen bei EquiEvents"),
        lang.pick("EquiEvents s’utilise avec un compte : connectez-vous, ou créez le vôtre.", "EquiEvents braucht ein Konto: Melden Sie sich an, oder erstellen Sie Ihres."),
        lang.pick(
            "Avec un compte, vos concours sont sur equievents.ch : sauvegardés, partagés avec votre équipe, et vous y travaillez en ligne, ici, ou dans l’application.",
            "Mit einem Konto sind Ihre Turniere auf equievents.ch: gesichert, mit Ihrem Team geteilt, und Sie arbeiten online damit, hier, oder in der App."
        ),
    )
}

/// Signing in, making an account: the two tabs (`next`: where signing in goes after).
fn tabs(lang: Lang, signing_in: bool, next: &str) -> String {
    let sign_in = lang.pick("Se connecter", "Anmelden");
    let register = lang.pick("Créer un compte", "Konto erstellen");
    if signing_in {
        format!(r#"<nav class="tabs"><span>{sign_in}</span><a href="{}">{register}</a></nav>"#, Path::Register.of(lang))
    } else {
        format!(r#"<nav class="tabs"><a href="{}">{sign_in}</a><span>{register}</span></nav>"#, esc(&session::sign_in_path(lang, next)))
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct SignInForm {
    #[serde(default)]
    email: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NextQuery {
    next: Option<String>,
}

fn sign_in_page(lang: Lang, email: &str, next: &str, error: Option<&str>) -> Response {
    let error = error.map(|e| format!(r#"<p class="error" role="alert">{e}</p>"#)).unwrap_or_default();
    let body = format!(
        r#"{welcome}
{tabs}
{error}
<form method="post" action="{action}">
<input type="hidden" name="next" value="{next}">
<label for="email">{email_label}</label><input id="email" name="email" type="email" autocomplete="email" required maxlength="254" value="{email}">
<label for="password">{password}</label><input id="password" name="password" type="password" autocomplete="current-password" required maxlength="200">
<button>{submit}</button>
</form>
<p class="links"><a href="{forgot}">{forgot_label}</a></p>"#,
        welcome = welcome(lang),
        tabs = tabs(lang, true, next),
        action = session::sign_in(lang),
        next = esc(next),
        email_label = lang.pick("Adresse e-mail", "E-Mail-Adresse"),
        email = esc(email),
        password = lang.pick("Mot de passe", "Passwort"),
        submit = lang.pick("Se connecter", "Anmelden"),
        forgot = Path::Forgot.of(lang),
        forgot_label = lang.pick("Mot de passe oublié ?", "Passwort vergessen?"),
    );
    page(lang, lang.pick("Se connecter", "Anmelden"), Some(&session::sign_in_path(lang.other(), next)), &body)
}

/// The sign-in; already signed in: straight on.
async fn sign_in_form(lang: Lang, State(app): State<App>, headers: HeaderMap, Query(q): Query<NextQuery>) -> Response {
    let next = session::next_or_app(q.next.as_deref());
    if let Some(token) = session::token(&headers) {
        if auth::user_of(&app.pool, &token).await.ok().flatten().is_some() {
            return redirect(&next, None);
        }
    }
    sign_in_page(lang, "", &next, None)
}

/// See `next`, with the session's cookie when there is one to set.
fn redirect(to: &str, cookie: Option<HeaderValue>) -> Response {
    let mut res = (StatusCode::SEE_OTHER, [(header::LOCATION, to.to_string())]).into_response();
    if let Some(cookie) = cookie {
        res.headers_mut().append(header::SET_COOKIE, cookie);
    }
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

async fn sign_in(lang: Lang, State(app): State<App>, Client(client): Client, Form(form): Form<SignInForm>) -> Response {
    let email = form.email.trim().to_lowercase();
    let next = session::next_or_app(form.next.as_deref());
    if !app.limits.allow(&format!("login:{client}"), limit::LOGIN_CLIENT) || !app.limits.allow(&format!("login-email:{email}"), limit::LOGIN_EMAIL) {
        return too_many(lang);
    }
    match auth::sign_in(&app.pool, &email, &form.password, "equievents.ch (site)").await {
        Ok(Some((token, _))) => redirect(&next, Some(session::set(&token, app.site_url.starts_with("https://")))),
        Ok(None) => with_status(
            StatusCode::UNAUTHORIZED,
            sign_in_page(lang, &form.email, &next, Some(lang.pick("Adresse e-mail ou mot de passe incorrect.", "E-Mail-Adresse oder Passwort falsch."))),
        ),
        Err(e) => server_error(lang, e),
    }
}

async fn sign_out(_lang: Lang, State(app): State<App>, headers: HeaderMap) -> Response {
    if let Some(token) = session::token(&headers) {
        let _ = auth::sign_out(&app.pool, &token).await;
    }
    redirect("/", Some(session::clear(app.site_url.starts_with("https://"))))
}

// --- Making an account -------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct RegisterForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    password: String,
    /// Left empty by people (hidden); filled by robots.
    #[serde(default)]
    website: String,
}

fn register_page(lang: Lang, form: &RegisterForm, error: Option<String>) -> Response {
    let error = error.map(|e| format!(r#"<p class="error" role="alert">{e}</p>"#)).unwrap_or_default();
    let body = format!(
        r#"{welcome}
{tabs}
{error}
<form method="post" action="{action}">
<label for="name">{name}</label><input id="name" name="name" autocomplete="name" required maxlength="100" value="{name_v}">
<label for="email">{email}</label><input id="email" name="email" type="email" autocomplete="email" required maxlength="254" value="{email_v}">
<label for="password">{password}</label><input id="password" name="password" type="password" autocomplete="new-password" required minlength="10" maxlength="200">
<p class="hint">{password_hint}</p>
<div class="trap" aria-hidden="true"><label for="website">Website</label><input id="website" name="website" tabindex="-1" autocomplete="off"></div>
<button>{submit}</button>
</form>
<p class="hint">{key}</p>
<p class="hint">{privacy}</p>"#,
        welcome = welcome(lang),
        tabs = tabs(lang, false, "/app/"),
        key = lang.pick(
            "Pour travailler sur un concours, il vous faut sa clé d’accès, fournie par Swiss Equestrian.",
            "Um an einem Turnier zu arbeiten, brauchen Sie seinen Zugangsschlüssel von Swiss Equestrian."
        ),
        action = Path::Register.of(lang),
        name = lang.pick("Prénom et nom", "Vorname und Name"),
        name_v = esc(&form.name),
        email = lang.pick("Adresse e-mail", "E-Mail-Adresse"),
        email_v = esc(&form.email),
        password = lang.pick("Mot de passe", "Passwort"),
        password_hint = lang.pick("Au moins 10 caractères.", "Mindestens 10 Zeichen."),
        submit = lang.pick("Créer le compte", "Konto erstellen"),
        privacy = lang.pick(
            "Nous gardons votre nom et votre adresse e-mail pour votre compte, rien d’autre, et ne les donnons à personne.",
            "Wir speichern Ihren Namen und Ihre E-Mail-Adresse für Ihr Konto, nichts anderes, und geben sie niemandem weiter."
        ),
    );
    page(lang, lang.pick("Créer un compte", "Konto erstellen"), Some(Path::Register.of(lang.other())), &body)
}

async fn register_form(lang: Lang) -> Response {
    register_page(lang, &RegisterForm::default(), None)
}

fn registered(lang: Lang, email: &str) -> Response {
    let body = format!(
        r#"<h1>{}</h1><p class="done">{}</p><p>{}</p><a class="button" href="/app/">{}</a><a class="button second" href="{}">{}</a>"#,
        lang.pick("Votre compte est prêt", "Ihr Konto ist bereit"),
        esc(email),
        lang.pick(
            "Travaillez en ligne, ici, ou installez l’application sur vos ordinateurs et connectez-vous-y avec cette adresse. Pour travailler sur un concours, il vous faut sa clé d’accès, fournie par Swiss Equestrian.",
            "Arbeiten Sie online, hier, oder installieren Sie die App auf Ihren Computern und melden Sie sich dort mit dieser Adresse an. Um an einem Turnier zu arbeiten, brauchen Sie seinen Zugangsschlüssel von Swiss Equestrian."
        ),
        lang.pick("Ouvrir EquiEvents en ligne", "EquiEvents online öffnen"),
        crate::downloads::path(lang),
        lang.pick("Télécharger l’application", "App herunterladen"),
    );
    page(lang, lang.pick("Votre compte est prêt", "Ihr Konto ist bereit"), None, &body)
}

async fn register(lang: Lang, State(app): State<App>, Client(client): Client, Form(form): Form<RegisterForm>) -> Response {
    if !app.limits.allow(&format!("register:{client}"), limit::REGISTER_CLIENT) {
        return too_many(lang);
    }
    // A robot filled the hidden field: it is told it worked, nothing is made.
    if !form.website.is_empty() {
        return registered(lang, &form.email);
    }
    match auth::create_user(&app.pool, &form.name, &form.email, &form.password, false, lang).await {
        Ok(Ok(user)) => {
            println!("account made: {}", user.email);
            // Signed in on the site at once.
            let mut res = registered(lang, &user.email);
            if let Ok(token) = auth::new_session(&app.pool, user.id, "equievents.ch (site)").await {
                res.headers_mut().append(header::SET_COOKIE, session::set(&token, app.site_url.starts_with("https://")));
            }
            res
        }
        Ok(Err(e)) => with_status(StatusCode::BAD_REQUEST, register_page(lang, &form, Some(invalid_text(lang, e)))),
        Err(e) => server_error(lang, e),
    }
}

// --- A forgotten password ----------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct ForgotForm {
    #[serde(default)]
    email: String,
}

fn forgot_page(lang: Lang, email: &str) -> Response {
    let body = format!(
        r#"<h1>{title}</h1>
<p class="muted">{intro}</p>
<form method="post" action="{action}">
<label for="email">{label}</label><input id="email" name="email" type="email" autocomplete="email" required maxlength="254" value="{email}">
<button>{submit}</button>
</form>"#,
        title = lang.pick("Mot de passe oublié", "Passwort vergessen"),
        intro = lang.pick("Nous vous envoyons un lien pour en choisir un nouveau.", "Wir senden Ihnen einen Link, um ein neues zu wählen."),
        action = Path::Forgot.of(lang),
        label = lang.pick("Adresse e-mail de votre compte", "E-Mail-Adresse Ihres Kontos"),
        email = esc(email),
        submit = lang.pick("Envoyer le lien", "Link senden"),
    );
    page(lang, lang.pick("Mot de passe oublié", "Passwort vergessen"), Some(Path::Forgot.of(lang.other())), &body)
}

async fn forgot_form(lang: Lang) -> Response {
    forgot_page(lang, "")
}

/// The e-mail with the reset link.
pub fn reset_mail(lang: Lang, site: &str, name: &str, email: &str, token: &str) -> (String, String) {
    let link = format!("{}{}?t={token}", site.trim_end_matches('/'), Path::Reset.of(lang));
    match lang {
        Lang::Fr => (
            "EquiEvents : nouveau mot de passe".into(),
            format!("Bonjour {name},\n\nPour choisir un nouveau mot de passe pour votre compte EquiEvents ({email}), ouvrez ce lien dans l’heure :\n\n{link}\n\nSi vous n’avez rien demandé, ignorez cet e-mail : votre mot de passe ne change pas.\n\nEquiEvents\n"),
        ),
        Lang::De => (
            "EquiEvents: neues Passwort".into(),
            format!("Guten Tag {name}\n\nUm ein neues Passwort für Ihr EquiEvents-Konto ({email}) zu wählen, öffnen Sie diesen Link innerhalb einer Stunde:\n\n{link}\n\nFalls Sie nichts angefordert haben, ignorieren Sie diese E-Mail: Ihr Passwort bleibt unverändert.\n\nEquiEvents\n"),
        ),
    }
}

/// Ask for a reset link: sent if the address has an account; the answer is the same either way
/// (no hint which addresses have one).
pub async fn send_reset_link(app: &App, lang: Lang, email: &str) -> Result<(), crate::db::Error> {
    let Ok(email) = auth::check_email(email) else { return Ok(()) };
    if !app.limits.allow(&format!("reset-email:{email}"), limit::RESET_EMAIL) {
        return Ok(());
    }
    let Some((token, user)) = auth::reset_start(&app.pool, &email).await? else { return Ok(()) };
    let (subject, body) = reset_mail(lang, &app.site_url, &user.name, &user.email, &token);
    let mailer = app.mailer.clone();
    // Sent in the background: the answer comes as fast for an address without an account.
    let send = async move {
        if let Err(e) = mailer.send(&user.email, &subject, &body).await {
            eprintln!("reset e-mail to {} not sent: {e}", user.email);
        }
    };
    if app.mail_in_background {
        tokio::spawn(send);
    } else {
        send.await;
    }
    Ok(())
}

async fn forgot(lang: Lang, State(app): State<App>, Client(client): Client, Form(form): Form<ForgotForm>) -> Response {
    if !app.limits.allow(&format!("reset:{client}"), limit::RESET_CLIENT) {
        return too_many(lang);
    }
    if auth::check_email(&form.email).is_err() {
        return with_status(StatusCode::BAD_REQUEST, forgot_page(lang, &form.email));
    }
    if let Err(e) = send_reset_link(&app, lang, &form.email).await {
        return server_error(lang, e);
    }
    let body = format!(
        r#"<h1>{}</h1><p>{} <strong>{}</strong>{}</p><p class="muted">{}</p>"#,
        lang.pick("Regardez vos e-mails", "Schauen Sie in Ihre E-Mails"),
        lang.pick("Si un compte existe pour", "Falls für"),
        esc(form.email.trim()),
        lang.pick(", un e-mail avec un lien vient de partir.", " ein Konto besteht, ist eine E-Mail mit einem Link unterwegs."),
        lang.pick("Le lien est valable une heure. Rien reçu ? Regardez dans les indésirables.", "Der Link ist eine Stunde gültig. Nichts erhalten? Schauen Sie im Spam-Ordner nach."),
    );
    page(lang, lang.pick("Regardez vos e-mails", "Schauen Sie in Ihre E-Mails"), None, &body)
}

// --- The link: a new password ------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct ResetQuery {
    #[serde(default)]
    t: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ResetForm {
    #[serde(default)]
    t: String,
    #[serde(default)]
    password: String,
}

fn link_gone(lang: Lang) -> Response {
    let body = format!(r#"<h1>{}</h1><p class="error">{}</p><a class="button" href="{}">{}</a>"#, lang.pick("Lien expiré", "Link abgelaufen"), invalid_text(lang, Invalid::Link), Path::Forgot.of(lang), lang.pick("Demander un nouveau lien", "Neuen Link anfordern"));
    with_status(StatusCode::GONE, page(lang, lang.pick("Lien expiré", "Link abgelaufen"), None, &body))
}

fn reset_page(lang: Lang, token: &str, error: Option<String>) -> Response {
    let error = error.map(|e| format!(r#"<p class="error" role="alert">{e}</p>"#)).unwrap_or_default();
    let body = format!(
        r#"<h1>{title}</h1>
{error}
<form method="post" action="{action}">
<input type="hidden" name="t" value="{token}">
<label for="password">{label}</label><input id="password" name="password" type="password" autocomplete="new-password" required minlength="10" maxlength="200">
<p class="hint">{hint}</p>
<button>{submit}</button>
</form>"#,
        title = lang.pick("Nouveau mot de passe", "Neues Passwort"),
        action = Path::Reset.of(lang),
        token = esc(token),
        label = lang.pick("Nouveau mot de passe", "Neues Passwort"),
        hint = lang.pick("Au moins 10 caractères. Vos appareils seront déconnectés.", "Mindestens 10 Zeichen. Ihre Geräte werden abgemeldet."),
        submit = lang.pick("Enregistrer", "Speichern"),
    );
    page(lang, lang.pick("Nouveau mot de passe", "Neues Passwort"), None, &body)
}

async fn reset_form(lang: Lang, State(app): State<App>, Query(q): Query<ResetQuery>) -> Response {
    match auth::reset_valid(&app.pool, &q.t).await {
        Ok(true) => reset_page(lang, &q.t, None),
        Ok(false) => link_gone(lang),
        Err(e) => server_error(lang, e),
    }
}

async fn reset(lang: Lang, State(app): State<App>, Client(client): Client, Form(form): Form<ResetForm>) -> Response {
    if !app.limits.allow(&format!("reset:{client}"), limit::RESET_CLIENT) {
        return too_many(lang);
    }
    match auth::reset_finish(&app.pool, &form.t, &form.password).await {
        Ok(Ok(user)) => {
            let body = format!(
                r#"<h1>{}</h1><p class="done">{}</p><p>{}</p>"#,
                lang.pick("Mot de passe changé", "Passwort geändert"),
                esc(&user.email),
                lang.pick("Vos appareils sont déconnectés : connectez-vous avec le nouveau mot de passe.", "Ihre Geräte sind abgemeldet: Melden Sie sich mit dem neuen Passwort an."),
            );
            page(lang, lang.pick("Mot de passe changé", "Passwort geändert"), None, &body)
        }
        Ok(Err(Invalid::Link)) => link_gone(lang),
        Ok(Err(e)) => with_status(StatusCode::BAD_REQUEST, reset_page(lang, &form.t, Some(invalid_text(lang, e)))),
        Err(e) => server_error(lang, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaped() {
        assert_eq!(esc(r#"<a href="x">Anne & 'Bruno'</a>"#), "&lt;a href=&quot;x&quot;&gt;Anne &amp; &#39;Bruno&#39;&lt;/a&gt;");
    }

    #[test]
    fn the_reset_mail() {
        let (subject, body) = reset_mail(Lang::De, "https://www.equievents.ch/", "Anne Office", "anne@example.ch", "abc123");
        assert_eq!(subject, "EquiEvents: neues Passwort");
        assert!(body.contains("https://www.equievents.ch/neues-passwort?t=abc123") && body.starts_with("Guten Tag Anne Office"));
        let (_, body) = reset_mail(Lang::Fr, "https://www.equievents.ch", "Anne", "anne@example.ch", "abc123");
        assert!(body.contains("https://www.equievents.ch/nouveau-mot-de-passe?t=abc123"));
    }
}
