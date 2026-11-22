//! A local equievents.ch whose Swiss Equestrian is a stand-in, to try loading an event with its
//! key from the app without a real key: it answers with EquiEvents' practice event (every
//! person invented, `crates/office/training/practice.json`) to the key `PRACTICE-KEY` (and the
//! demo keys), its riders and horses as the registry, after a few seconds of "preparing" (202),
//! and 404 to any other key. Nothing goes to Swiss Equestrian.
//!
//! ```text
//! createdb equievents_try
//! DATABASE_URL="host=localhost dbname=equievents_try" cargo run -p server --example se_stand_in
//! ```
//!
//! Then the app's server is `http://localhost:3999` (`PORT`): the library's settings, or
//! `EQUIEVENTS_SERVER`. Accounts are made from the app as on equievents.ch. The online version
//! too, at `http://localhost:3999/app/`, when `WEB_DIR` names the interface built for it
//! (`cd app && npx vite build --base /app/ --outDir <folder>`); its offices in `OFFICES_DIR`
//! (else a folder in the system's temporary one); `ANSWER_WITHIN_MS`: how long a command of the
//! online version may take before it answers with a job (20 s).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use se_api::http::{HttpRequest, HttpResponse, Limits};
use serde_json::{json, Value};
use server::{api, db, mail, se::Se};

const BASE: &str = "https://se-stand-in.invalid/api/veranstaltersoftware/v2";
const KEY: &str = "PRACTICE-KEY";

#[tokio::main]
async fn main() -> Result<(), db::Error> {
    let url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set")?;
    let pool = db::pool(&url)?;
    db::migrate(&pool).await?;
    let practice: Value = serde_json::from_str(include_str!("../../office/training/practice.json"))?;
    // Preparing: 202 to the first two loads of each request for the event.
    let polls = Mutex::new(0u32);
    let transport = Arc::new(move |request: &HttpRequest| {
        let key = se_api::http::download_key(request, BASE, &server::se::ENDPOINTS).ok_or("only downloads")?;
        let endpoint = request.url[BASE.len() + 1..].split('?').next().unwrap_or("").to_string();
        let answer = |status: u16, body: Value| Ok(HttpResponse { status, status_text: String::new(), body: body.to_string().into_bytes() });
        println!("stand-in Swiss Equestrian: {endpoint} (key {key})");
        if key != KEY && !se_api::DEMO_KEYS.contains(&key) {
            return answer(404, json!({ "message": format!("access_key '{key}' not found") }));
        }
        if endpoint == "getVeranstaltung" {
            let mut n = polls.lock().unwrap_or_else(|e| e.into_inner());
            *n += 1;
            if *n % 3 != 0 {
                return answer(202, json!({}));
            }
        }
        let delivered = &practice["getVeranstaltung"]["data"];
        match endpoint.as_str() {
            "getPersonen" => answer(200, json!({ "data": { "personen": delivered["personen"] } })),
            "getPferde" => answer(200, json!({ "data": { "pferde": delivered["pferde"] } })),
            _ => answer(200, practice[&endpoint].clone()),
        }
    });
    let se = Se::with(Some(BASE), transport, Limits { poll_every: Duration::from_secs(2), ..Limits::default() });
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3999);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    println!("equievents with a stand-in Swiss Equestrian on http://localhost:{port} (key {KEY})");
    let site = format!("http://localhost:{port}");
    let offices = std::env::var("OFFICES_DIR").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir().join("equievents-offices"));
    let interface = std::env::var("WEB_DIR").ok().map(std::path::PathBuf::from).filter(|d| d.join("index.html").is_file());
    println!("online version: {}", if interface.is_some() { format!("{site}/app/ (offices in {})", offices.display()) } else { "not served (WEB_DIR)".into() });
    let mut web = server::web::Web::new(offices, &format!("http://127.0.0.1:{port}"), interface);
    // To try the online version's jobs (a command slower than this answers with one).
    if let Some(ms) = std::env::var("ANSWER_WITHIN_MS").ok().and_then(|v| v.parse().ok()) {
        web.answer_within = Duration::from_millis(ms);
    }
    let app = api::router(api::App { se, web: Some(std::sync::Arc::new(web)), ..api::App::new(pool, mail::Mailer::off(), &site) });
    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await?;
    Ok(())
}
