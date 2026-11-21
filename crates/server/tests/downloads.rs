//! The desktop apps' downloads and update check, through the server's router (no database: these
//! pages never use it).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use server::{api, db, mail};
use tower::ServiceExt;

/// A server whose downloads folder holds a Mac release (and stray files that are not served).
fn server(dir: &std::path::Path) -> axum::Router {
    std::fs::write(dir.join("EquiEvents-0.2.0-macos.dmg"), b"the disk image").unwrap();
    std::fs::write(dir.join("EquiEvents-0.2.0-macos.app.tar.gz"), b"the update").unwrap();
    std::fs::write(dir.join("notes.txt"), b"not for anyone").unwrap();
    std::fs::write(
        dir.join("latest.json"),
        r#"{"macos":{"version":"0.2.0","date":"2026-11-14T09:30:00.000Z","installer":"EquiEvents-0.2.0-macos.dmg","installer_size":14,
        "update":"EquiEvents-0.2.0-macos.app.tar.gz","signature":"c2lnbmF0dXJl","arch":["aarch64","x86_64"]}}"#,
    )
    .unwrap();
    // Never connected: these routes do not use the database.
    let pool = db::pool("host=127.0.0.1 port=1 user=nobody dbname=none").unwrap();
    api::router(api::App { downloads: Some(dir.to_path_buf()), ..api::App::new(pool, mail::Mailer::off(), "https://www.equievents.ch/") })
}

async fn get(router: &axum::Router, path: &str, headers: &[(&str, &str)]) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut req = Request::builder().uri(path);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let res = router.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    (status, headers, res.into_body().collect().await.unwrap().to_bytes().to_vec())
}

#[tokio::test]
async fn the_page_the_files_and_the_update_check() {
    let dir = tempfile::tempdir().unwrap();
    let router = server(dir.path());

    // The page, in each language, linked from the home page.
    let windows = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
    let (status, _, html) = get(&router, "/telecharger", &[("user-agent", windows)]).await;
    let html = String::from_utf8(html).unwrap();
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Télécharger pour Mac") && html.contains(r#"href="/downloads/EquiEvents-0.2.0-macos.dmg""#));
    assert!(html.find("<h2>Windows</h2>").unwrap() < html.find("<h2>Mac</h2>").unwrap(), "this computer's system first");
    assert!(html.contains(r#"href="/herunterladen""#));
    let (_, _, html) = get(&router, "/herunterladen", &[]).await;
    assert!(String::from_utf8(html).unwrap().contains("Für Mac herunterladen"));
    let (_, _, home) = get(&router, "/", &[]).await;
    let home = String::from_utf8(home).unwrap();
    assert!(home.contains(r#"href="/telecharger""#) && home.contains(r#"href="/herunterladen""#));

    // The files the manifest names, whole or in part (a download picked up again).
    let (status, headers, body) = get(&router, "/downloads/EquiEvents-0.2.0-macos.dmg", &[]).await;
    assert_eq!((status, body.as_slice()), (StatusCode::OK, b"the disk image".as_slice()));
    assert_eq!(headers["content-disposition"], r#"attachment; filename="EquiEvents-0.2.0-macos.dmg""#);
    let (status, _, body) = get(&router, "/downloads/EquiEvents-0.2.0-macos.dmg", &[("range", "bytes=4-")]).await;
    assert_eq!((status, body.as_slice()), (StatusCode::PARTIAL_CONTENT, b"disk image".as_slice()));
    // Nothing else in the folder.
    for path in ["/downloads/notes.txt", "/downloads/latest.json", "/downloads/..%2Flatest.json", "/downloads/EquiEvents-0.1.0-macos.dmg"] {
        assert_eq!(get(&router, path, &[]).await.0, StatusCode::NOT_FOUND, "{path}");
    }

    // The update check, as Tauri's updater asks.
    let (status, headers, body) = get(&router, "/api/updates/darwin/aarch64/0.1.0", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["cache-control"], "no-store");
    let update: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(update["version"], "0.2.0");
    assert_eq!(update["url"], "https://www.equievents.ch/downloads/EquiEvents-0.2.0-macos.app.tar.gz");
    assert_eq!(update["signature"], "c2lnbmF0dXJl");
    for path in ["/api/updates/darwin/aarch64/0.2.0", "/api/updates/windows/x86_64/0.1.0"] {
        assert_eq!(get(&router, path, &[]).await.0, StatusCode::NO_CONTENT, "{path}");
    }
}

#[tokio::test]
async fn no_release_yet() {
    let pool = db::pool("host=127.0.0.1 port=1 user=nobody dbname=none").unwrap();
    let empty = tempfile::tempdir().unwrap();
    for downloads in [None, Some(empty.path().to_path_buf())] {
        let router = api::router(api::App { downloads, ..api::App::new(pool.clone(), mail::Mailer::off(), "https://www.equievents.ch") });
        let (status, _, html) = get(&router, "/telecharger", &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(String::from_utf8(html).unwrap().matches("Pas encore disponible").count(), 2);
        assert_eq!(get(&router, "/api/updates/darwin/aarch64/0.1.0", &[]).await.0, StatusCode::NO_CONTENT);
        assert_eq!(get(&router, "/downloads/EquiEvents-0.2.0-macos.dmg", &[]).await.0, StatusCode::NOT_FOUND);
    }
}
