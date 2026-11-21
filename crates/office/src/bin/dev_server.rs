//! The office over HTTP on this computer, for working on the UI in a browser:
//! `POST /api/<command>` with JSON arguments → JSON answer (`{"error": …}` with status 400).
//!
//! Files, as the desktop app has them from the system's dialogs: the browser picks or saves them,
//! the office reads and writes paths. `POST /files/<name>` keeps the body (a file the browser
//! picked; empty: a file the office is to write) in a folder of its own under the system's
//! temporary folder and answers `{"path", "url"}`; `GET <url>` gives the file back (to save it
//! where the browser chose; a print, to open it under its own name). Nothing else is served.
//!
//! `cargo run -p office --bin office-dev-server -- <library folder> [port]` (default 7878);
//! the UI's development server (`npm run dev` in app/ui) forwards `/api` and `/files` to it.
//! Only on 127.0.0.1: it is a development tool, not the web app.

use std::path::{Path, PathBuf};

fn main() {
    let mut args = std::env::args().skip(1);
    let library = args.next().unwrap_or_else(|| "library".into());
    let port = args.next().unwrap_or_else(|| "7878".into());
    let office = std::sync::Arc::new(office::Office::new(Path::new(&library), "dev").unwrap_or_else(|e| panic!("{e}")));
    let server = tiny_http::Server::http(format!("127.0.0.1:{port}")).unwrap_or_else(|e| panic!("port {port}: {e}"));
    let files = std::env::temp_dir().join("equievents-dev-files");
    eprintln!("office on http://127.0.0.1:{port}/api/<command>, library {library}, files in {}", files.display());
    let json = tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap();
    let mut kept = 0u64;
    for mut request in server.incoming_requests() {
        let url = request.url().split('?').next().unwrap_or("").to_string();
        if let Some(rest) = url.strip_prefix("/files/") {
            let answer = if *request.method() == tiny_http::Method::Get {
                match file(&files, rest).and_then(|p| std::fs::read(&p).ok().map(|bytes| (p, bytes))) {
                    // A print opens in the browser's viewer and is saved under its own name.
                    Some((path, bytes)) => {
                        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        let kind = if name.to_lowercase().ends_with(".pdf") { "application/pdf" } else { "application/octet-stream" };
                        let disposition = format!("inline; filename*=UTF-8''{}", encode(&name));
                        tiny_http::Response::from_data(bytes)
                            .with_status_code(200)
                            .with_header(tiny_http::Header::from_bytes("Content-Type", kind).unwrap())
                            .with_header(tiny_http::Header::from_bytes("Content-Disposition", disposition.as_bytes()).unwrap())
                    }
                    None => tiny_http::Response::from_data(b"no such file".to_vec()).with_status_code(404),
                }
            } else {
                kept += 1;
                let name = file_name(&decode(rest));
                let folder = format!("{}-{kept}", std::process::id());
                let mut body = Vec::new();
                let _ = request.as_reader().read_to_end(&mut body);
                let dir = files.join(&folder);
                let path = dir.join(&name);
                let written = std::fs::create_dir_all(&dir).and_then(|_| if body.is_empty() { Ok(()) } else { std::fs::write(&path, &body) });
                match written {
                    Ok(()) => {
                        let link = format!("/files/{folder}/{}", encode(&name));
                        eprintln!("file {} ({} bytes)", path.display(), body.len());
                        tiny_http::Response::from_data(serde_json::json!({ "path": path, "url": link }).to_string().into_bytes()).with_status_code(200).with_header(json.clone())
                    }
                    Err(e) => tiny_http::Response::from_data(serde_json::json!({ "error": e.to_string() }).to_string().into_bytes()).with_status_code(400).with_header(json.clone()),
                }
            };
            let _ = request.respond(answer);
            continue;
        }
        // Each call on its own thread, as the desktop app runs them: a long one (Swiss
        // Equestrian's riders and horses) does not hold the others.
        let command = url.trim_start_matches("/api/").to_string();
        let mut body = String::new();
        let _ = request.as_reader().read_to_string(&mut body);
        let args: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
        let (office, json) = (std::sync::Arc::clone(&office), json.clone());
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let (status, answer) = match office.call(&command, &args) {
                Ok(v) => (200, v),
                Err(e) => (400, serde_json::json!({ "error": e })),
            };
            eprintln!("{command} {status} {} ms", started.elapsed().as_millis());
            let _ = request.respond(tiny_http::Response::from_string(answer.to_string()).with_status_code(status).with_header(json));
        });
    }
}

/// A kept file, `<folder>/<name>` as given out: only there, never elsewhere.
fn file(files: &Path, rest: &str) -> Option<PathBuf> {
    let (folder, name) = rest.split_once('/')?;
    let ok = |s: &str| !s.is_empty() && s != "." && s != ".." && !s.contains(['/', '\\']);
    let name = decode(name);
    (ok(folder) && ok(&name) && folder.chars().all(|c| c.is_ascii_digit() || c == '-')).then(|| files.join(folder).join(name))
}

/// The name of a file without any folder in it.
fn file_name(name: &str) -> String {
    let name = name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if name.is_empty() || name == "." || name == ".." {
        "file".into()
    } else {
        name.into()
    }
}

/// `%xx` in a URL's path, back to UTF-8.
fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A file's name in a URL's path.
fn encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}
