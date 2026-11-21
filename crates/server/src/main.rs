//! `equievents-server <command>`: one program, as Gestion Concours' image, for every container.
//!
//! - `web`: the API and site on `PORT` (3000), behind Nginx Proxy Manager; e-mail (reset links)
//!   as `mail::Mailer::from_env` says, links to `APP_URL`; the desktop apps' releases from
//!   `DOWNLOADS_DIR` (`/data/downloads`; docs/architecture/releases.md); Swiss Equestrian as
//!   `SE_SERVER` says (`se::Se::from_env`: production, stage, none); the online version
//!   (`web`): the accounts' offices in `OFFICES_DIR` (`/data/offices`), the interface from
//!   `WEB_DIR` (`/srv/app`);
//! - `worker`: the nightly backup (`BACKUP_AT`, `BACKUP_DIR`, `BACKUP_KEEP`);
//! - `migrate`: bring the database up to date (`web` and `worker` do it when they start);
//! - `create-admin "Prénom Nom" e-mail [fr|de]`: an administrator's account, its password printed once
//!   (people make their own accounts on the site);
//! - `create-user "Prénom Nom" e-mail`, `set-password e-mail`: for support;
//! - `set-role e-mail admin|support|none`: an account's role (administrators also give them in
//!   the online version's « Comptes »);
//! - `backup`: a backup now;
//! - `mail-preview [file.html]`: every e-mail it sends, light and dark, in the three languages, on
//!   one page to look at (nothing sent, no database).
//!
//! The database: `DATABASE_URL`.

use std::net::SocketAddr;
use std::process::ExitCode;

use chrono::NaiveTime;
use server::{api, auth, backup, db, mail};

fn env(name: &str, default: &str) -> String {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| default.to_string())
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: &[String]) -> Result<(), db::Error> {
    let command = args.first().map(String::as_str).unwrap_or("web");
    // Every e-mail, to look at before any is sent (no database needed).
    if command == "mail-preview" {
        let file = std::path::PathBuf::from(args.get(1).cloned().unwrap_or_else(|| "equievents-mails.html".into()));
        std::fs::write(&file, server::notices::preview(&env("APP_URL", "https://www.equievents.ch")))?;
        println!("every e-mail, light and dark, in the three languages: {}", file.display());
        return Ok(());
    }
    let database_url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set")?;
    let pool = db::pool(&database_url)?;
    let migrated = db::migrate(&pool).await?;
    if !migrated.is_empty() {
        println!("migrations applied: {}", migrated.join(", "));
    }
    let arg = |i: usize, what: &str| args.get(i).cloned().ok_or_else(|| format!("{command}: {what} missing"));
    let backups = || -> Result<backup::Backups, db::Error> {
        Ok(backup::Backups {
            dir: env("BACKUP_DIR", "/data/backups").into(),
            keep: env("BACKUP_KEEP", "14").parse()?,
            at: NaiveTime::parse_from_str(&env("BACKUP_AT", "02:30"), "%H:%M")?,
            database_url: database_url.clone(),
        })
    };
    match command {
        "web" => {
            let port: u16 = env("PORT", "3000").parse()?;
            let mailer = mail::Mailer::from_env(|name| std::env::var(name).ok())?;
            let site = env("APP_URL", "https://www.equievents.ch");
            let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
            println!("equievents web on :{port} ({site}; e-mail {:?})", mailer.mode);
            let downloads = server::downloads::dir_from_env(Some(env("DOWNLOADS_DIR", "/data/downloads")));
            let se = server::se::Se::from_env(|name| std::env::var(name).ok())?;
            println!("Swiss Equestrian: {} (downloads)", se.name());
            // The online version: each account's office under OFFICES_DIR, reaching this server
            // on its own port; the interface from WEB_DIR (the image's /srv/app).
            let offices = std::path::PathBuf::from(env("OFFICES_DIR", "/data/offices"));
            let interface = Some(std::path::PathBuf::from(env("WEB_DIR", "/srv/app"))).filter(|d| d.join("index.html").is_file());
            println!("online version: offices in {}, interface {}", offices.display(), interface.as_ref().map_or("not served".into(), |d| d.display().to_string()));
            let web = server::web::Web::new(offices, &format!("http://127.0.0.1:{port}"), interface);
            let app = api::router(api::App { downloads, se, web: Some(std::sync::Arc::new(web)), ..api::App::new(pool, mailer, &site) });
            axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(stopped()).await?;
        }
        "worker" => {
            let b = backups()?;
            println!("equievents worker: backups at {} into {} ({} kept)", b.at.format("%H:%M"), b.dir.display(), b.keep);
            tokio::select! {
                _ = backup::every_night(b) => {}
                _ = stopped() => {}
            }
        }
        "migrate" => println!("database up to date"),
        "create-admin" | "create-user" => {
            let password = auth::new_password();
            let lang = server::lang::Lang::of(args.get(3).map(String::as_str).unwrap_or("fr"));
            auth::create_user(&pool, &arg(1, "the name")?, &arg(2, "the e-mail address")?, &password, command == "create-admin", lang).await?.map_err(|e| e.to_string())?;
            println!("account made; its password (shown once): {password}");
        }
        "set-password" => {
            let password = auth::new_password();
            auth::set_password(&pool, &arg(1, "the e-mail address")?, &password).await?;
            println!("new password (shown once; signed out everywhere): {password}");
        }
        "set-role" => {
            let email = arg(1, "the e-mail address")?;
            let (admin, support) = match arg(2, "the role (admin, support or none)")?.as_str() {
                "admin" => (true, false),
                "support" => (false, true),
                "none" => (false, false),
                other => return Err(format!("set-role: {other}: admin, support or none").into()),
            };
            let changed = pool.get().await?.execute("UPDATE users SET admin = $2, support = $3 WHERE lower(email) = lower($1)", &[&email, &admin, &support]).await?;
            if changed == 0 {
                return Err(format!("set-role: no account {email}").into());
            }
            println!("{email}: {}", if admin { "administrator" } else if support { "support" } else { "no role" });
        }
        "backup" => println!("backup: {}", backup::run(&backups()?).await?.display()),
        other => return Err(format!("unknown command {other} (web, worker, migrate, create-admin, create-user, set-password, set-role, backup, mail-preview)").into()),
    }
    Ok(())
}

/// Ctrl-C or the container's SIGTERM.
async fn stopped() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("signal handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = ctrl_c.await;
}
