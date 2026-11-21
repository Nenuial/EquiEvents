//! The nightly backup (the worker): `pg_dump` of the whole database into the backups folder
//! (Postgres' compressed custom format, restored with `pg_restore`), the newest `keep` kept.

use std::path::{Path, PathBuf};

use chrono::{Local, NaiveTime, TimeDelta};

use crate::db::Error;

/// Where and when backups are made.
#[derive(Debug, Clone)]
pub struct Backups {
    pub dir: PathBuf,
    pub keep: usize,
    /// Local time of day (the container's TZ).
    pub at: NaiveTime,
    pub database_url: String,
}

/// Make one backup now; its file.
pub async fn run(b: &Backups) -> Result<PathBuf, Error> {
    tokio::fs::create_dir_all(&b.dir).await?;
    let name = format!("equievents-{}.dump", Local::now().format("%Y-%m-%d-%H%M"));
    let file = b.dir.join(&name);
    let partial = b.dir.join(format!("{name}.partial"));
    let out = tokio::process::Command::new("pg_dump").arg("--format=custom").arg("--no-owner").arg("--file").arg(&partial).arg("--dbname").arg(&b.database_url).output().await?;
    if !out.status.success() {
        let _ = tokio::fs::remove_file(&partial).await;
        return Err(format!("pg_dump failed: {}", String::from_utf8_lossy(&out.stderr).trim()).into());
    }
    tokio::fs::rename(&partial, &file).await?;
    prune(&b.dir, b.keep)?;
    Ok(file)
}

/// Keep the newest `keep` backups (by name: they sort by date).
pub fn prune(dir: &Path, keep: usize) -> Result<Vec<PathBuf>, Error> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("equievents-") && n.ends_with(".dump")))
        .collect();
    files.sort();
    let old = files.len().saturating_sub(keep);
    let removed: Vec<PathBuf> = files.into_iter().take(old).collect();
    for f in &removed {
        std::fs::remove_file(f)?;
    }
    Ok(removed)
}

/// How long until the next `at` (today's, or tomorrow's when it is past).
pub fn until_next(at: NaiveTime) -> std::time::Duration {
    let now = Local::now().naive_local();
    let mut next = now.date().and_time(at);
    if next <= now {
        next += TimeDelta::days(1);
    }
    (next - now).to_std().unwrap_or_default()
}

/// The worker's loop: a backup every night at `at`; a failure is reported and tried again the
/// next night.
pub async fn every_night(b: Backups) {
    loop {
        tokio::time::sleep(until_next(b.at)).await;
        match run(&b).await {
            Ok(file) => println!("backup: {}", file.display()),
            Err(e) => eprintln!("backup failed: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_are_kept() {
        let dir = std::env::temp_dir().join(format!("eq-backups-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for day in 1..=5 {
            std::fs::write(dir.join(format!("equievents-2026-11-0{day}-0230.dump")), b"x").unwrap();
        }
        std::fs::write(dir.join("notes.txt"), b"kept").unwrap();
        let removed = prune(&dir, 3).unwrap();
        assert_eq!(removed.len(), 2);
        assert!(dir.join("equievents-2026-11-05-0230.dump").exists() && !dir.join("equievents-2026-11-01-0230.dump").exists());
        assert!(dir.join("notes.txt").exists());
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(until_next(NaiveTime::from_hms_opt(2, 30, 0).unwrap()) <= std::time::Duration::from_secs(24 * 3600));
    }
}
