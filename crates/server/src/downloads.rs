//! The desktop apps on equievents.ch (docs/architecture/releases.md): the download page, the
//! installers, and the apps' question whether a newer version is out.
//!
//! | | |
//! |---|---|
//! | `/telecharger`, `/herunterladen` | the page, signed in on the site only: Mac and Windows, this computer's system first |
//! | `/downloads/{file}` | an installer or an update, as the manifest names it |
//! | `/api/updates/{target}/{arch}/{version}` | Tauri's updater: the newer release (its format), else 204 |
//!
//! The files and their manifest (`latest.json`, written by `scripts/release.mjs`) are in a folder
//! the deployment copies each release into (`DOWNLOADS_DIR`); nothing else in it is served.

use std::path::{Path as FsPath, PathBuf};

use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::api::App;
use crate::site::{esc, page, Lang};

/// The page in each language.
pub fn path(lang: Lang) -> &'static str {
    lang.pick("/telecharger", "/herunterladen")
}

pub fn routes() -> Router<App> {
    Router::new()
        .route("/telecharger", get(|s, h| download_page(Lang::Fr, s, h)))
        .route("/herunterladen", get(|s, h| download_page(Lang::De, s, h)))
        .route("/downloads/{file}", get(file))
        .route("/api/updates/{target}/{arch}/{version}", get(update))
}

/// The two systems the desktop app is made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Windows,
}

impl Platform {
    /// The computer a browser runs on, from its `User-Agent` (phones and others: none).
    pub fn of_agent(agent: &str) -> Option<Platform> {
        if ["iPhone", "iPad", "Android"].iter().any(|p| agent.contains(p)) {
            None
        } else if agent.contains("Windows") {
            Some(Platform::Windows)
        } else if agent.contains("Macintosh") || agent.contains("Mac OS X") {
            Some(Platform::Macos)
        } else {
            None
        }
    }

    /// Tauri's `{{target}}`.
    fn of_target(target: &str) -> Option<Platform> {
        match target {
            "darwin" => Some(Platform::Macos),
            "windows" => Some(Platform::Windows),
            _ => None,
        }
    }
}

/// One system's latest release, as `scripts/release.mjs` describes it.
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub version: String,
    /// When it was built (RFC 3339).
    pub date: String,
    /// What people download: the disk image (Mac), the setup program (Windows).
    pub installer: String,
    #[serde(default)]
    pub installer_size: u64,
    /// What the app downloads to update itself (Windows: the setup program again).
    pub update: String,
    /// The update's signature (minisign, as Tauri's signer writes it).
    pub signature: String,
    /// The processors it runs on: `aarch64`, `x86_64`.
    #[serde(default)]
    pub arch: Vec<String>,
}

/// `latest.json`: each system's latest release (each built on its own computer).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Manifest {
    pub macos: Option<Release>,
    pub windows: Option<Release>,
}

impl Manifest {
    /// The folder's manifest; none yet (or not readable): no release.
    pub async fn read(dir: Option<&FsPath>) -> Manifest {
        let Some(dir) = dir else { return Manifest::default() };
        let file = dir.join("latest.json");
        match tokio::fs::read(&file).await {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                eprintln!("error: {}: {e}", file.display());
                Manifest::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Manifest::default(),
            Err(e) => {
                eprintln!("error: {}: {e}", file.display());
                Manifest::default()
            }
        }
    }

    pub fn release(&self, platform: Platform) -> Option<&Release> {
        match platform {
            Platform::Macos => self.macos.as_ref(),
            Platform::Windows => self.windows.as_ref(),
        }
    }

    /// Whether `name` is one of the files it names (only those are served).
    fn names(&self, name: &str) -> bool {
        [&self.macos, &self.windows].into_iter().flatten().any(|r| r.installer == name || r.update == name)
    }

    /// The update for an app at `current` on `target`/`arch` (Tauri's names), in the updater's
    /// format, when a newer release is out.
    pub fn update_for(&self, target: &str, arch: &str, current: &str, site: &str) -> Option<Value> {
        let release = self.release(Platform::of_target(target)?)?;
        if !release.arch.iter().any(|a| a == arch) {
            return None;
        }
        let newer = match (semver::Version::parse(&release.version), semver::Version::parse(current.trim_start_matches('v'))) {
            (Ok(out), Ok(current)) => out > current,
            _ => false,
        };
        newer.then(|| {
            json!({
                "version": release.version,
                "pub_date": release.date,
                "url": format!("{site}/downloads/{}", release.update),
                "signature": release.signature,
                "notes": "",
            })
        })
    }
}

/// A file name alone: no folder, nothing hidden.
fn plain_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\']) && FsPath::new(name).file_name().is_some_and(|n| n == name)
}

async fn file(State(app): State<App>, Path(name): Path<String>, req: Request) -> Response {
    let Some(dir) = app.downloads.as_deref() else { return StatusCode::NOT_FOUND.into_response() };
    if !plain_name(&name) || !Manifest::read(Some(dir)).await.names(&name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let mut res = match ServeFile::new(dir.join(&name)).oneshot(req).await {
        Ok(res) => res.into_response(),
        Err(e) => match e {},
    };
    if res.status().is_success() {
        let headers = res.headers_mut();
        if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
            headers.insert(header::CONTENT_DISPOSITION, v);
        }
        // A file's name carries its version; a version built again keeps its name.
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=3600"));
        headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    }
    res
}

async fn update(State(app): State<App>, Path((target, arch, version)): Path<(String, String, String)>) -> Response {
    let manifest = Manifest::read(app.downloads.as_deref()).await;
    let mut res = match manifest.update_for(&target, &arch, &version, &app.site_url) {
        Some(release) => Json(release).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    };
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

// --- The page ----------------------------------------------------------------------------------

/// `2026-11-14T…` → `14.11.2026`.
fn day(date: &str) -> String {
    match (date.get(0..4), date.get(5..7), date.get(8..10)) {
        (Some(y), Some(m), Some(d)) => format!("{d}.{m}.{y}"),
        _ => String::new(),
    }
}

/// Megabytes, rounded (`18 Mo`, `18 MB`).
fn size(lang: Lang, bytes: u64) -> Option<String> {
    (bytes > 0).then(|| format!("{} {}", (bytes + 500_000) / 1_000_000, lang.pick("Mo", "MB")))
}

fn platform_section(lang: Lang, platform: Platform, release: Option<&Release>) -> String {
    let (name, button, system) = match platform {
        Platform::Macos => ("Mac", lang.pick("Télécharger pour Mac", "Für Mac herunterladen"), lang.pick("macOS 11 ou plus récent", "macOS 11 oder neuer")),
        Platform::Windows => ("Windows", lang.pick("Télécharger pour Windows", "Für Windows herunterladen"), "Windows 10, 11"),
    };
    let Some(r) = release else {
        return format!(r#"<section class="platform"><h2>{name}</h2><p class="muted">{}</p></section>"#, lang.pick("Pas encore disponible : bientôt ici.", "Noch nicht verfügbar: bald hier."));
    };
    let mut facts = vec![format!("{} {}", lang.pick("Version", "Version"), esc(&r.version))];
    let built = day(&r.date);
    if !built.is_empty() {
        facts[0] = format!("{} {} {built}", facts[0], lang.pick("du", "vom"));
    }
    facts.extend(size(lang, r.installer_size));
    facts.push(system.to_string());
    if platform == Platform::Macos && !r.arch.iter().any(|a| a == "x86_64") {
        facts.push(lang.pick("Mac avec puce Apple (M1 ou plus récent)", "Mac mit Apple-Chip (M1 oder neuer)").into());
    }
    let steps: &[&str] = match platform {
        Platform::Macos => &[
            lang.pick("Ouvrez le fichier téléchargé.", "Öffnen Sie die heruntergeladene Datei."),
            lang.pick("Glissez EquiEvents dans le dossier Applications.", "Ziehen Sie EquiEvents in den Ordner Programme."),
            lang.pick("Ouvrez EquiEvents depuis Applications : c’est là qu’il se met à jour.", "Öffnen Sie EquiEvents im Ordner Programme: Dort aktualisiert es sich."),
        ],
        Platform::Windows => &[
            lang.pick("Ouvrez le fichier téléchargé.", "Öffnen Sie die heruntergeladene Datei."),
            lang.pick(
                "Windows ne connaît pas encore EquiEvents et affiche « Windows a protégé votre ordinateur » : cliquez sur « Informations complémentaires », puis sur « Exécuter quand même ».",
                "Windows kennt EquiEvents noch nicht und zeigt «Der Computer wurde durch Windows geschützt»: Klicken Sie auf «Weitere Informationen», dann auf «Trotzdem ausführen».",
            ),
            lang.pick("Suivez l’installation : EquiEvents s’ouvre à la fin.", "Folgen Sie der Installation: Am Ende öffnet sich EquiEvents."),
        ],
    };
    format!(
        r#"<section class="platform"><h2>{name}</h2><a class="button" href="/downloads/{file}" download>{button}</a><p class="muted">{facts}</p><ol class="steps">{steps}</ol></section>"#,
        file = esc(&r.installer),
        facts = facts.join(" · "),
        steps = steps.iter().map(|s| ["<li>", s, "</li>"].concat()).collect::<String>(),
    )
}

/// The page for a browser on `platform` (its system first; a phone: both, and where to go).
pub fn page_body(lang: Lang, manifest: &Manifest, platform: Option<Platform>) -> String {
    let order = match platform {
        Some(Platform::Windows) => [Platform::Windows, Platform::Macos],
        _ => [Platform::Macos, Platform::Windows],
    };
    let phone = platform.is_none().then(|| format!(r#"<p class="muted">{}</p>"#, lang.pick("EquiEvents s’installe sur l’ordinateur du secrétariat de concours, Mac ou Windows : ouvrez cette page sur celui-ci.", "EquiEvents wird auf dem Computer des Turniersekretariats installiert, Mac oder Windows: Öffnen Sie diese Seite dort.")));
    let account = crate::site::register_path(lang);
    format!(
        r#"<h1>{title}</h1>
<p class="muted">{intro}</p>
{phone}{first}{second}
<p class="muted then">{then} <a href="{account}">{make}</a></p>"#,
        title = lang.pick("Télécharger EquiEvents", "EquiEvents herunterladen"),
        intro = lang.pick(
            "L’application du secrétariat de concours. Elle fonctionne aussi sans internet ; les nouvelles versions s’installent depuis l’application.",
            "Die App für das Turniersekretariat. Sie funktioniert auch ohne Internet; neue Versionen werden in der App installiert."
        ),
        phone = phone.unwrap_or_default(),
        first = platform_section(lang, order[0], manifest.release(order[0])),
        second = platform_section(lang, order[1], manifest.release(order[1])),
        then = lang.pick(
            "Pour travailler sur un concours : un compte EquiEvents et la clé d’accès du concours, fournie par Swiss Equestrian.",
            "Um an einem Turnier zu arbeiten: ein EquiEvents-Konto und der Zugangsschlüssel des Turniers von Swiss Equestrian."
        ),
        make = lang.pick("Créer un compte", "Konto erstellen"),
    )
}

/// The page, for signed-in people only (the user, 2026-11-21); the files themselves stay
/// reachable for installed apps' updates.
async fn download_page(lang: Lang, State(app): State<App>, headers: HeaderMap) -> Response {
    let signed_in = match crate::session::token(&headers) {
        Some(token) => crate::auth::user_of(&app.pool, &token).await.ok().flatten().is_some(),
        None => false,
    };
    if !signed_in {
        let to = crate::session::sign_in_path(lang, path(lang));
        return (StatusCode::SEE_OTHER, [(header::LOCATION, to)]).into_response();
    }
    let agent = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or("");
    let manifest = Manifest::read(app.downloads.as_deref()).await;
    let body = page_body(lang, &manifest, Platform::of_agent(agent));
    page(lang, lang.pick("Télécharger EquiEvents", "EquiEvents herunterladen"), Some(path(lang.other())), &body)
}

/// The folder from `DOWNLOADS_DIR` (none: no downloads yet).
pub fn dir_from_env(value: Option<String>) -> Option<PathBuf> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str, arch: &[&str]) -> Release {
        Release {
            version: version.into(),
            date: "2026-11-14T09:30:00.000Z".into(),
            installer: format!("EquiEvents-{version}-macos.dmg"),
            installer_size: 18_400_000,
            update: format!("EquiEvents-{version}-macos.app.tar.gz"),
            signature: "c2ln".into(),
            arch: arch.iter().map(|a| a.to_string()).collect(),
        }
    }

    #[test]
    fn browsers_by_system() {
        let mac = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15";
        let windows = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
        let iphone = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1";
        let android = "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36";
        assert_eq!(Platform::of_agent(mac), Some(Platform::Macos));
        assert_eq!(Platform::of_agent(windows), Some(Platform::Windows));
        assert_eq!(Platform::of_agent(iphone), None);
        assert_eq!(Platform::of_agent(android), None);
        assert_eq!(Platform::of_agent(""), None);
    }

    #[test]
    fn only_a_newer_release_for_the_same_processor() {
        let manifest = Manifest { macos: Some(release("0.2.0", &["aarch64", "x86_64"])), windows: None };
        let site = "https://www.equievents.ch";
        let update = manifest.update_for("darwin", "aarch64", "0.1.0", site).unwrap();
        assert_eq!(update["version"], "0.2.0");
        assert_eq!(update["url"], "https://www.equievents.ch/downloads/EquiEvents-0.2.0-macos.app.tar.gz");
        assert_eq!(update["signature"], "c2ln");
        assert_eq!(update["pub_date"], "2026-11-14T09:30:00.000Z");
        assert!(manifest.update_for("darwin", "x86_64", "0.1.9", site).is_some());
        // Not newer, another system, another processor, nonsense.
        assert!(manifest.update_for("darwin", "aarch64", "0.2.0", site).is_none());
        assert!(manifest.update_for("darwin", "aarch64", "0.10.0", site).is_none());
        assert!(manifest.update_for("windows", "x86_64", "0.1.0", site).is_none());
        assert!(manifest.update_for("linux", "x86_64", "0.1.0", site).is_none());
        let apple_only = Manifest { macos: Some(release("0.2.0", &["aarch64"])), windows: None };
        assert!(apple_only.update_for("darwin", "x86_64", "0.1.0", site).is_none());
        assert!(manifest.update_for("darwin", "aarch64", "nonsense", site).is_none());
        // A test version is older than its release.
        assert!(manifest.update_for("darwin", "aarch64", "0.2.0-beta.1", site).is_some());
    }

    #[test]
    fn only_the_files_it_names() {
        let manifest = Manifest { macos: Some(release("0.2.0", &["aarch64"])), windows: None };
        assert!(manifest.names("EquiEvents-0.2.0-macos.dmg") && manifest.names("EquiEvents-0.2.0-macos.app.tar.gz"));
        assert!(!manifest.names("latest.json") && !manifest.names("EquiEvents-0.1.0-macos.dmg"));
        assert!(plain_name("EquiEvents-0.2.0-macos.dmg"));
        for bad in ["", ".env", "../latest.json", "a/b", "a\\b", ".."] {
            assert!(!plain_name(bad), "{bad}");
        }
    }

    #[test]
    fn the_page() {
        let manifest = Manifest { macos: Some(release("0.2.0", &["aarch64"])), windows: None };
        let fr = page_body(Lang::Fr, &manifest, Some(Platform::Windows));
        // This computer's system first; one not out yet says so.
        assert!(fr.find("<h2>Windows</h2>").unwrap() < fr.find("<h2>Mac</h2>").unwrap());
        assert!(fr.contains("Pas encore disponible"));
        assert!(fr.contains(r#"href="/downloads/EquiEvents-0.2.0-macos.dmg""#));
        assert!(fr.contains("Version 0.2.0 du 14.11.2026 · 18 Mo · macOS 11 ou plus récent · Mac avec puce Apple"));
        assert!(!fr.contains("sur l’ordinateur du bureau"));
        let de = page_body(Lang::De, &Manifest { macos: Some(release("0.2.0", &["aarch64", "x86_64"])), windows: None }, None);
        assert!(de.find("<h2>Mac</h2>").unwrap() < de.find("<h2>Windows</h2>").unwrap());
        assert!(de.contains("Version 0.2.0 vom 14.11.2026 · 18 MB · macOS 11 oder neuer</p>"));
        assert!(de.contains("Öffnen Sie diese Seite dort") && de.contains(r#"href="/registrieren""#));
        let none = page_body(Lang::Fr, &Manifest::default(), Some(Platform::Macos));
        assert_eq!(none.matches("Pas encore disponible").count(), 2);
        assert!(!none.contains("/downloads/"));
    }

    #[test]
    fn a_manifest_as_the_release_script_writes_it() {
        let manifest: Manifest = serde_json::from_str(
            r#"{"macos":{"version":"0.2.0","date":"2026-11-14T09:30:00.000Z","installer":"EquiEvents-0.2.0-macos.dmg","installer_size":18400000,
            "update":"EquiEvents-0.2.0-macos.app.tar.gz","signature":"c2ln","arch":["aarch64","x86_64"]},
            "windows":{"version":"0.1.9","date":"2026-11-10T09:30:00.000Z","installer":"EquiEvents-0.1.9-windows-setup.exe","installer_size":9100000,
            "update":"EquiEvents-0.1.9-windows-setup.exe","signature":"c2ln","arch":["x86_64"]}}"#,
        )
        .unwrap();
        assert_eq!(manifest.release(Platform::Windows).unwrap().version, "0.1.9");
        assert_eq!(manifest.update_for("windows", "x86_64", "0.1.0", "https://x.ch").unwrap()["url"], "https://x.ch/downloads/EquiEvents-0.1.9-windows-setup.exe");
    }
}
