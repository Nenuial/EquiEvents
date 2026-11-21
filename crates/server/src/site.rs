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

const STYLE: &str = r#"
:root{--ink:#14202b;--ink-2:#4b5966;--line:#d5dde4;--page:#f4f7f9;--card:#fff;--accent:#00283c;--accent-ink:#fff;--bad:#a4262c;--good:#1d6b3a;color-scheme:light}
@media (prefers-color-scheme:dark){:root{--ink:#e8eef3;--ink-2:#a6b4c0;--line:#33424f;--page:#0f1820;--card:#17232d;--accent:#7fb6d6;--accent-ink:#0f1820;--bad:#ff8a8a;--good:#7fd69a;color-scheme:dark}}
*{box-sizing:border-box}
body{margin:0;background:var(--page);color:var(--ink);font:16px/1.5 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;padding:max(1rem,env(safe-area-inset-top)) 1rem max(1rem,env(safe-area-inset-bottom))}
main{max-width:28rem;margin:2rem auto}
header{display:flex;align-items:baseline;justify-content:space-between;gap:1rem;margin-bottom:1.25rem}
.brand{font-weight:700;font-size:1.35rem;letter-spacing:.01em;color:var(--ink);text-decoration:none}
.lang{color:var(--ink-2);font-size:.9rem}
.card{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:1.5rem}
h1{font-size:1.3rem;margin:0 0 .5rem}
p{margin:.5rem 0}
.muted{color:var(--ink-2);font-size:.92rem}
label{display:block;font-weight:600;margin:1rem 0 .3rem}
input{width:100%;font:inherit;padding:.65rem .75rem;border:1px solid var(--line);border-radius:8px;background:var(--page);color:var(--ink)}
input:focus{outline:2px solid var(--accent);outline-offset:1px}
.hint{color:var(--ink-2);font-size:.85rem;margin-top:.25rem}
button,.button{display:inline-block;width:100%;margin-top:1.25rem;padding:.75rem 1rem;border:0;border-radius:8px;background:var(--accent);color:var(--accent-ink);font:inherit;font-weight:600;text-align:center;text-decoration:none;cursor:pointer}
.second{background:transparent;color:var(--accent);border:1px solid var(--line)}
.error{color:var(--bad);font-weight:600}
.done{color:var(--good);font-weight:600}
.trap{position:absolute;left:-10000px;width:1px;height:1px;overflow:hidden}
a{color:var(--accent)}
footer{margin-top:1rem;text-align:center}
h2{font-size:1.1rem;margin:0}
.platform{margin-top:1.5rem;padding-top:1.25rem;border-top:1px solid var(--line)}
.platform .button{margin-top:.75rem}
.steps{padding-left:1.25rem;margin:.75rem 0 0;color:var(--ink-2);font-size:.92rem}
.steps li{margin:.3rem 0}
.then{margin-top:1.5rem;padding-top:1.25rem;border-top:1px solid var(--line)}
.group{margin-top:1.25rem}
.group .button{margin-top:.6rem}
"#;

/// A whole page: the site's head, the language switch, `body` in a card.
pub(crate) fn page(lang: Lang, title: &str, switch_to: Option<&str>, body: &str) -> Response {
    let switch = switch_to.map(|href| format!(r#"<a class="lang" href="{href}" hreflang="{0}" lang="{0}">{1}</a>"#, lang.other().code(), lang.pick("Deutsch", "Français"))).unwrap_or_default();
    let html = format!(
        r#"<!doctype html><html lang="{code}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover"><title>{title} · EquiEvents</title><style>{STYLE}</style></head><body><main><header><a class="brand" href="/">EquiEvents</a>{switch}</header><div class="card">{body}</div></main></body></html>"#,
        code = lang.code(),
        title = esc(title),
    );
    let mut res = (StatusCode::OK, html).into_response();
    let headers = res.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    // Nothing from elsewhere, no frames, no address (reset links!) passed on to other sites.
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'"));
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
        r#"<h1>{title}</h1>
{error}
<form method="post" action="{action}">
<input type="hidden" name="next" value="{next}">
<label for="email">{email_label}</label><input id="email" name="email" type="email" autocomplete="email" required maxlength="254" value="{email}">
<label for="password">{password}</label><input id="password" name="password" type="password" autocomplete="current-password" required maxlength="200">
<button>{submit}</button>
</form>
<p class="muted"><a href="{forgot}">{forgot_label}</a> · <a href="{register}">{register_label}</a></p>"#,
        title = lang.pick("Se connecter", "Anmelden"),
        action = session::sign_in(lang),
        next = esc(next),
        email_label = lang.pick("Adresse e-mail", "E-Mail-Adresse"),
        email = esc(email),
        password = lang.pick("Mot de passe", "Passwort"),
        submit = lang.pick("Se connecter", "Anmelden"),
        forgot = Path::Forgot.of(lang),
        forgot_label = lang.pick("Mot de passe oublié ?", "Passwort vergessen?"),
        register = Path::Register.of(lang),
        register_label = lang.pick("Créer un compte", "Konto erstellen"),
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
        r#"<h1>{title}</h1>
<p class="muted">{intro}</p>
{error}
<form method="post" action="{action}">
<label for="name">{name}</label><input id="name" name="name" autocomplete="name" required maxlength="100" value="{name_v}">
<label for="email">{email}</label><input id="email" name="email" type="email" autocomplete="email" required maxlength="254" value="{email_v}">
<label for="password">{password}</label><input id="password" name="password" type="password" autocomplete="new-password" required minlength="10" maxlength="200">
<p class="hint">{password_hint}</p>
<div class="trap" aria-hidden="true"><label for="website">Website</label><input id="website" name="website" tabindex="-1" autocomplete="off"></div>
<button>{submit}</button>
</form>
<p class="muted">{privacy}</p>"#,
        title = lang.pick("Créer un compte", "Konto erstellen"),
        intro = lang.pick(
            "Votre compte vous connecte à EquiEvents sur vos ordinateurs et appareils. Pour travailler sur un concours, il vous faut sa clé d’accès, fournie par Swiss Equestrian.",
            "Mit Ihrem Konto melden Sie sich in EquiEvents auf Ihren Computern und Geräten an. Um an einem Turnier zu arbeiten, brauchen Sie seinen Zugangsschlüssel von Swiss Equestrian."
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
