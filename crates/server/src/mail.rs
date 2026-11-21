//! E-mail (password-reset links), through SMTP as Gestion Concours sends its own (the same
//! settings, `MAIL_MODE`, `SMTP_*`):
//!
//! - `off`: nothing is sent; the recipient and subject are logged, the e-mail is kept in memory
//!   (the tests read it there);
//! - `redirect`: everything goes to `MAIL_REDIRECT_TO` only (a test server);
//! - `send`: to the real recipients.
//!
//! Port 465: TLS from the start (SMTPS); another port: STARTTLS.

use std::sync::{Arc, Mutex};

use lettre::message::{header::ContentType, Mailbox};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::db::Error;

/// An e-mail as it was (or would have been) sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    pub to: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Off,
    /// Everything to this address.
    Redirect(String),
    Send,
}

#[derive(Debug, Clone)]
pub struct Smtp {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

#[derive(Clone)]
pub struct Mailer {
    pub mode: Mode,
    smtp: Option<Smtp>,
    from: Mailbox,
    reply_to: Option<Mailbox>,
    /// `off`: the e-mails not sent, newest last (at most 50).
    kept: Arc<Mutex<Vec<Mail>>>,
}

impl Mailer {
    /// Nothing sent (tests, a server without e-mail yet).
    pub fn off() -> Mailer {
        Mailer { mode: Mode::Off, smtp: None, from: "EquiEvents <info@equievents.ch>".parse().expect("address"), reply_to: None, kept: Arc::default() }
    }

    /// From the environment: `MAIL_MODE` (off), `MAIL_REDIRECT_TO`, `SMTP_HOST`, `SMTP_PORT`
    /// (465), `SMTP_USER`, `SMTP_PASS`, `SMTP_FROM`, `MAIL_FROM_NAME` (EquiEvents),
    /// `MAIL_REPLY_TO`.
    pub fn from_env(env: impl Fn(&str) -> Option<String>) -> Result<Mailer, Error> {
        let get = |name: &str, default: &str| env(name).map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).unwrap_or_else(|| default.to_string());
        let mode = match get("MAIL_MODE", "off").as_str() {
            "off" => Mode::Off,
            "redirect" => Mode::Redirect(get("MAIL_REDIRECT_TO", "")),
            "send" => Mode::Send,
            other => return Err(format!("MAIL_MODE {other}: off, redirect or send").into()),
        };
        if let Mode::Redirect(to) = &mode {
            to.parse::<Mailbox>().map_err(|e| format!("MAIL_REDIRECT_TO: {e}"))?;
        }
        let from_address = get("SMTP_FROM", "info@equievents.ch");
        let from: Mailbox = format!("{} <{from_address}>", get("MAIL_FROM_NAME", "EquiEvents")).parse().map_err(|e| format!("SMTP_FROM: {e}"))?;
        let reply_to = Some(get("MAIL_REPLY_TO", "")).filter(|r| !r.is_empty()).map(|r| r.parse::<Mailbox>()).transpose().map_err(|e| format!("MAIL_REPLY_TO: {e}"))?;
        let smtp = match mode {
            Mode::Off => None,
            _ => Some(Smtp {
                host: Some(get("SMTP_HOST", "")).filter(|h| !h.is_empty()).ok_or("SMTP_HOST is needed to send e-mail")?,
                port: get("SMTP_PORT", "465").parse().map_err(|_| "SMTP_PORT: a port number")?,
                user: get("SMTP_USER", ""),
                password: get("SMTP_PASS", ""),
            }),
        };
        Ok(Mailer { mode, smtp, from, reply_to, kept: Arc::default() })
    }

    /// What `off` kept, newest last.
    pub fn kept(&self) -> Vec<Mail> {
        self.kept.lock().map(|k| k.clone()).unwrap_or_default()
    }

    /// Send a plain-text e-mail (or keep it, `off`).
    pub async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), Error> {
        let mail = Mail { to: to.to_string(), subject: subject.to_string(), body: body.to_string() };
        let (Some(smtp), recipient) = (&self.smtp, match &self.mode {
            Mode::Off => {
                println!("mail off, not sent: to {to}: {subject}");
                if let Ok(mut kept) = self.kept.lock() {
                    kept.push(mail);
                    let extra = kept.len().saturating_sub(50);
                    kept.drain(..extra);
                }
                return Ok(());
            }
            Mode::Redirect(other) => other.clone(),
            Mode::Send => to.to_string(),
        }) else {
            return Err("no SMTP server".into());
        };
        let mut message = Message::builder().from(self.from.clone()).to(recipient.parse()?).subject(subject).header(ContentType::TEXT_PLAIN);
        if let Some(r) = &self.reply_to {
            message = message.reply_to(r.clone());
        }
        let message = message.body(body.to_string())?;
        let builder = if smtp.port == 465 {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp.host)?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.host)?
        };
        let mut builder = builder.port(smtp.port).timeout(Some(std::time::Duration::from_secs(30)));
        if !smtp.user.is_empty() {
            builder = builder.credentials(Credentials::new(smtp.user.clone(), smtp.password.clone()));
        }
        builder.build().send(message).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings() {
        let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        assert_eq!(Mailer::from_env(env(&[])).unwrap().mode, Mode::Off);
        let m = Mailer::from_env(env(&[("MAIL_MODE", "redirect"), ("MAIL_REDIRECT_TO", "test@example.ch"), ("SMTP_HOST", "mail.example.ch")])).unwrap();
        assert_eq!(m.mode, Mode::Redirect("test@example.ch".into()));
        assert_eq!(m.smtp.unwrap().port, 465);
        assert!(Mailer::from_env(env(&[("MAIL_MODE", "send")])).is_err(), "no SMTP host");
        assert!(Mailer::from_env(env(&[("MAIL_MODE", "loud")])).is_err());
    }

    #[tokio::test]
    async fn off_keeps_them() {
        let m = Mailer::off();
        m.send("anne@example.ch", "Hello", "Body").await.unwrap();
        assert_eq!(m.kept(), vec![Mail { to: "anne@example.ch".into(), subject: "Hello".into(), body: "Body".into() }]);
    }
}
