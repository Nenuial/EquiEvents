//! EquiEvents on the desktop: the shared UI in a window, the office core behind one command, and
//! the app's own updates from equievents.ch (docs/architecture/releases.md).

// No console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use serde_json::json;
use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Every UI request: a command of the office core with its arguments (`office::Office::call`).
/// Run off the main thread, so a long import does not freeze the window.
#[tauri::command]
async fn call(office: tauri::State<'_, Arc<office::Office>>, command: String, args: serde_json::Value) -> Result<serde_json::Value, String> {
    let office = Arc::clone(&office);
    tauri::async_runtime::spawn_blocking(move || office.call(&command, &args)).await.map_err(|e| e.to_string())?
}

/// The update the last check found, waiting for the office to install it.
#[derive(Default)]
struct Waiting(std::sync::Mutex<Option<tauri_plugin_updater::Update>>);

/// Whether this build updates itself: a release made with the app's update key
/// (scripts/release.mjs), not a development build.
fn updates_on(app: &tauri::AppHandle) -> bool {
    let key = app.config().plugins.0.get("updater").and_then(|u| u.get("pubkey")).and_then(|k| k.as_str());
    !cfg!(debug_assertions) && key.is_some_and(|k| !k.trim().is_empty())
}

/// Whether equievents.ch (or the library's server) has a newer version:
/// `{current, on, version?, date?}` (`on` false: this build does not update itself).
#[tauri::command]
async fn update_check(app: tauri::AppHandle, office: tauri::State<'_, Arc<office::Office>>, waiting: tauri::State<'_, Waiting>) -> Result<serde_json::Value, String> {
    let current = app.package_info().version.to_string();
    if !updates_on(&app) {
        return Ok(json!({ "current": current, "on": false }));
    }
    let endpoint = format!("{}{}", office.server_url(), office::sync::UPDATES).parse().map_err(|e| format!("{e}"))?;
    let updater = app.updater_builder().endpoints(vec![endpoint]).and_then(|b| b.build()).map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    let answer = match &update {
        Some(u) => json!({ "current": current, "on": true, "version": u.version, "date": u.raw_json.get("pub_date") }),
        None => json!({ "current": current, "on": true }),
    };
    *waiting.0.lock().map_err(|_| "update state")? = update;
    Ok(answer)
}

/// Download the update found (its signature checked against the app's key), install it, start
/// again (Windows: its installer starts the app). Progress: `update-progress` events,
/// `{done, total}` then `{installing}`.
#[tauri::command]
async fn update_install(app: tauri::AppHandle, waiting: tauri::State<'_, Waiting>) -> Result<(), String> {
    let update = waiting.0.lock().map_err(|_| "update state")?.clone().ok_or("no update waiting")?;
    let mut done = 0u64;
    let progress = |chunk: usize, total: Option<u64>| {
        done += chunk as u64;
        let _ = app.emit("update-progress", json!({ "done": done, "total": total }));
    };
    let installing = || {
        let _ = app.emit("update-progress", json!({ "installing": true }));
    };
    update.download_and_install(progress, installing).await.map_err(|e| e.to_string())?;
    app.restart()
}

/// This installation's name in the operation log: made once, kept in the app's data folder, so
/// that changes from two computers never get the same id.
fn device_id(data: &std::path::Path) -> std::io::Result<String> {
    let file = data.join("device-id");
    if let Ok(id) = std::fs::read_to_string(&file) {
        if !id.trim().is_empty() {
            return Ok(id.trim().to_string());
        }
    }
    std::fs::create_dir_all(data)?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let id = format!("desktop-{:x}{:x}", nanos, std::process::id());
    std::fs::write(&file, &id)?;
    Ok(id)
}

/// Access keys of events in the system's keychain, never in a file of the app.
struct Keychain;

/// The keychain entries' service name.
const SERVICE: &str = "ch.equievents.office";

impl office::connection::Secrets for Keychain {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        match keyring::Entry::new(SERVICE, name).and_then(|e| e.get_password()) {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("keychain: {e}")),
        }
    }

    fn set(&self, name: &str, value: Option<&str>) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, name).map_err(|e| format!("keychain: {e}"))?;
        match value {
            Some(secret) => entry.set_password(secret).map_err(|e| format!("keychain: {e}")),
            None => match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(format!("keychain: {e}")),
            },
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Waiting::default())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let office = office::Office::with_secrets(&data.join("library"), &device_id(&data)?, Box::new(Keychain)).map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(Arc::new(office));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![call, update_check, update_install])
        .run(tauri::generate_context!())
        .expect("EquiEvents could not start");
}
