//! equievents.ch: the master copy of every show (its operation log), the apps' sync, accounts
//! (anyone makes their own on the site), the desktop apps' downloads and updates, the nightly
//! backups (docs/architecture/server.md).
//! Swiss Equestrian: only the server talks to it (`se`).

pub mod api;
pub mod auth;
pub mod backup;
pub mod db;
pub mod downloads;
pub mod lang;
pub mod limit;
pub mod mail;
pub mod notices;
pub mod registry;
pub mod se;
pub mod session;
pub mod shows;
pub mod site;
pub mod web;
