//! The app's updates end to end (docs/architecture/releases.md): a release in equievents.ch's
//! downloads folder, Tauri's updater asking our server as the app does, the download checked
//! against the app's key, and (macOS) the new app put in place of the old one, here a stand-in
//! in a temporary folder.

use std::io::Write;
use std::net::SocketAddr;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::json;
use tauri_plugin_updater::UpdaterExt;

/// An app bundle as the release's `.app.tar.gz` holds it: `EquiEvents.app/Contents/…`.
fn app_archive(text: &str) -> Vec<u8> {
    let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default()));
    for (path, body) in [("EquiEvents.app/Contents/Info.plist", "<plist/>"), ("EquiEvents.app/Contents/MacOS/EquiEvents", text)] {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, path, body.as_bytes()).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

/// As `tauri build` signs: the signature's minisign text in base64, the version it is for in its
/// trusted comment.
fn sign(keys: &minisign::KeyPair, data: &[u8], version: &str) -> String {
    let comment = format!("timestamp:1794689810\tfile:EquiEvents.app.tar.gz\tversion:{version}");
    let signature = minisign::sign(Some(&keys.pk), &keys.sk, data, Some(&comment), Some("signature from tauri secret key")).unwrap();
    B64.encode(signature.into_string())
}

/// The server on a free port, its downloads folder `dir`.
async fn serve(dir: &std::path::Path) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let at = listener.local_addr().unwrap();
    // Never connected: the update check and the downloads do not use the database.
    let pool = server::db::pool("host=127.0.0.1 port=1 user=nobody dbname=none").unwrap();
    let app = server::api::App { downloads: Some(dir.to_path_buf()), ..server::api::App::new(pool, server::mail::Mailer::off(), &format!("http://{at}")) };
    let router = server::api::router(app);
    tokio::spawn(async move { axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap() });
    at
}

/// `latest.json` as scripts/release.mjs writes it, for this computer's system.
fn manifest(dir: &std::path::Path, version: &str, update: &str, signature: &str) {
    let release = json!({
        "version": version, "date": "2026-11-14T09:30:00.000Z", "installer": "EquiEvents-0.2.0-macos.dmg", "installer_size": 1,
        "update": update, "signature": signature, "arch": ["aarch64", "x86_64"],
    });
    let system = if cfg!(target_os = "windows") { "windows" } else { "macos" };
    std::fs::write(dir.join("latest.json"), json!({ system: release }).to_string()).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_update_from_the_server_replaces_the_app() {
    let keys = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
    let pubkey = B64.encode(keys.pk.to_box().unwrap().into_string());
    let downloads = tempfile::tempdir().unwrap();
    let archive = app_archive("the new app");
    std::fs::write(downloads.path().join("EquiEvents-0.2.0-macos.app.tar.gz"), &archive).unwrap();
    manifest(downloads.path(), "0.2.0", "EquiEvents-0.2.0-macos.app.tar.gz", &sign(&keys, &archive, "0.2.0"));
    let at = serve(downloads.path()).await;

    // The installed app (0.1.0, the mock's version), a stand-in.
    let installed = tempfile::tempdir().unwrap();
    let bundle = installed.path().join("EquiEvents.app");
    std::fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
    std::fs::File::create(bundle.join("Contents/MacOS/EquiEvents")).unwrap().write_all(b"the old app").unwrap();

    // The app's configuration (tauri.conf.json), its key.
    let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let mut updater_conf = conf["plugins"]["updater"].clone();
    assert_eq!(updater_conf["requireSignedVersion"], true);
    updater_conf["pubkey"] = pubkey.into();
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().plugins.0.insert("updater".into(), updater_conf);
    let app = tauri::test::mock_builder().plugin(tauri_plugin_updater::Builder::new().build()).build(context).unwrap();
    // As the app asks (main.rs: the library's server and `office::sync::UPDATES`).
    let endpoint: tauri::Url = format!("http://{at}{}", office::sync::UPDATES).parse().unwrap();
    let updater = || app.updater_builder().endpoints(vec![endpoint.clone()]).unwrap().executable_path(bundle.join("Contents/MacOS/EquiEvents")).build().unwrap();

    let update = updater().check().await.unwrap().expect("a newer version");
    assert_eq!((update.current_version.as_str(), update.version.as_str()), ("0.1.0", "0.2.0"));
    assert_eq!(update.download_url.as_str(), format!("http://{at}/downloads/EquiEvents-0.2.0-macos.app.tar.gz"));
    let bytes = update.download(|_, _| {}, || {}).await.unwrap();
    assert_eq!(bytes, archive);

    #[cfg(target_os = "macos")]
    {
        update.install(&bytes).unwrap();
        assert_eq!(std::fs::read_to_string(bundle.join("Contents/MacOS/EquiEvents")).unwrap(), "the new app");
        assert!(bundle.join("Contents/Info.plist").exists());
    }

    // Refused, nothing installed: signed with another key; not what was signed; an older release
    // offered as a newer one (its signature says which it is); a signature without its version.
    let refused = || async { updater().check().await.unwrap().expect("offered").download(|_, _| {}, || {}).await.is_err() };
    let other = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
    manifest(downloads.path(), "0.2.0", "EquiEvents-0.2.0-macos.app.tar.gz", &sign(&other, &archive, "0.2.0"));
    assert!(refused().await);
    manifest(downloads.path(), "0.3.0", "EquiEvents-0.2.0-macos.app.tar.gz", &sign(&keys, &archive, "0.2.0"));
    assert!(refused().await);
    let unversioned = B64.encode(minisign::sign(Some(&keys.pk), &keys.sk, archive.as_slice(), Some("timestamp:1794689810\tfile:x"), None).unwrap().into_string());
    manifest(downloads.path(), "0.2.0", "EquiEvents-0.2.0-macos.app.tar.gz", &unversioned);
    assert!(refused().await);
    std::fs::write(downloads.path().join("EquiEvents-0.2.0-macos.app.tar.gz"), app_archive("something else")).unwrap();
    manifest(downloads.path(), "0.2.0", "EquiEvents-0.2.0-macos.app.tar.gz", &sign(&keys, &archive, "0.2.0"));
    assert!(refused().await);

    // Already the latest: nothing.
    std::fs::write(downloads.path().join("latest.json"), json!({ "macos": null, "windows": null }).to_string()).unwrap();
    assert!(updater().check().await.unwrap().is_none());
}
