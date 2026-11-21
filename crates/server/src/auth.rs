//! Accounts and sign-in. Anyone makes their own account (the site, the apps): an account alone
//! opens nothing, a show needs its Swiss Equestrian keys. Passwords are kept as Argon2id hashes;
//! an app signs in with an e-mail and password and gets a token (32 random bytes), which the
//! server keeps only as a SHA-256 hash, valid a year from its last use. A forgotten password is
//! reset with a link sent by e-mail (its token hashed too, valid an hour, used once).

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use deadpool_postgres::Pool;
use rand::RngCore;
use sha2::{Digest, Sha256};
use tokio_postgres::error::SqlState;

use crate::db::Error;
use crate::lang::Lang;

/// A signed-in person.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub admin: bool,
    /// Of their e-mails: `fr` or `de`.
    pub language: String,
}

impl User {
    pub fn lang(&self) -> Lang {
        Lang::of(&self.language)
    }
}

/// How long a token stays valid after its last use.
const TOKEN_DAYS: i32 = 365;
/// How long a password-reset link is valid.
pub const RESET_MINUTES: i32 = 60;
pub const PASSWORD_MIN: usize = 10;
pub const PASSWORD_MAX: usize = 200;
const NAME_MAX: usize = 100;

/// Why an account's details are refused (the site and the apps say it in their language).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
    /// No name, or a very long one.
    Name,
    /// Not an e-mail address.
    Email,
    /// Fewer than 10 characters (or more than 200).
    Password,
    /// The address has an account already.
    Taken,
    /// A password-reset link unknown, used or expired.
    Link,
}

impl Invalid {
    /// For the apps: `name`, `email`, `password`, `taken`, `link`.
    pub fn code(self) -> &'static str {
        match self {
            Invalid::Name => "name",
            Invalid::Email => "email",
            Invalid::Password => "password",
            Invalid::Taken => "taken",
            Invalid::Link => "link",
        }
    }
}

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Invalid::Name => "a name is needed",
            Invalid::Email => "not an e-mail address",
            Invalid::Password => "a password has 10 to 200 characters",
            Invalid::Taken => "this address has an account already",
            Invalid::Link => "this link is no longer valid",
        })
    }
}

/// A name as kept (trimmed).
pub fn check_name(name: &str) -> Result<String, Invalid> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(Invalid::Name);
    }
    Ok(name)
}

/// An e-mail address as kept (trimmed, lower case): something@domain.tld, nothing that would
/// break an e-mail's headers.
pub fn check_email(email: &str) -> Result<String, Invalid> {
    let email = email.trim().to_lowercase();
    let plain = !email.chars().any(|c| c.is_whitespace() || c.is_control() || "<>,;:\"()[]\\".contains(c));
    let shaped = match email.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty() && !domain.contains('@') && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.') && !domain.contains("..")
        }
        None => false,
    };
    if plain && shaped && email.len() <= 254 {
        Ok(email)
    } else {
        Err(Invalid::Email)
    }
}

pub fn check_password(password: &str) -> Result<(), Invalid> {
    let n = password.chars().count();
    if (PASSWORD_MIN..=PASSWORD_MAX).contains(&n) {
        Ok(())
    } else {
        Err(Invalid::Password)
    }
}

pub fn hash_password(password: &str) -> Result<String, Error> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default().hash_password(password.as_bytes(), &salt).map_err(|e| e.to_string())?.to_string())
}

fn password_ok(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes.iter().flat_map(|b| [DIGITS[(b >> 4) as usize] as char, DIGITS[(b & 15) as usize] as char]).collect()
}

/// `n` random bytes, as hexadecimal.
pub fn random_hex(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    OsRng.fill_bytes(&mut bytes);
    hex(&bytes)
}

fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

/// A readable password: 4 groups of 4 letters and digits (no 0/O, 1/l).
pub fn new_password() -> String {
    const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    let chars: Vec<char> = bytes.iter().map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char).collect();
    chars.chunks(4).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-")
}

/// Make an account (the site, the apps, the administrator's commands). An e-mail address has
/// one account.
pub async fn create_user(pool: &Pool, name: &str, email: &str, password: &str, admin: bool, lang: Lang) -> Result<Result<User, Invalid>, Error> {
    let (name, email) = match (check_name(name), check_email(email), check_password(password)) {
        (Ok(name), Ok(email), Ok(())) => (name, email),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return Ok(Err(e)),
    };
    let hash = hash_password(password)?;
    let client = pool.get().await?;
    let language = lang.code();
    match client.query_one("INSERT INTO users (name, email, password_hash, admin, language) VALUES ($1, $2, $3, $4, $5) RETURNING id", &[&name, &email, &hash, &admin, &language]).await {
        Ok(row) => Ok(Ok(User { id: row.get(0), name, email, admin, language: language.into() })),
        Err(e) if e.code() == Some(&SqlState::UNIQUE_VIOLATION) => Ok(Err(Invalid::Taken)),
        Err(e) => Err(e.into()),
    }
}

/// Make an account and sign in with it (the apps): its token.
pub async fn register(pool: &Pool, name: &str, email: &str, password: &str, device: &str, lang: Lang) -> Result<Result<(String, User), Invalid>, Error> {
    let user = match create_user(pool, name, email, password, false, lang).await? {
        Ok(user) => user,
        Err(e) => return Ok(Err(e)),
    };
    let token = new_session(pool, user.id, device).await?;
    Ok(Ok((token, user)))
}

/// A new password for an account (the administrator's command); signed out everywhere.
pub async fn set_password(pool: &Pool, email: &str, password: &str) -> Result<(), Error> {
    check_password(password).map_err(|e| e.to_string())?;
    let client = pool.get().await?;
    let Some(row) = client.query_opt("UPDATE users SET password_hash = $2 WHERE email = $1 RETURNING id", &[&email.trim().to_lowercase(), &hash_password(password)?]).await? else {
        return Err(format!("no account for {email}").into());
    };
    let id: i64 = row.get(0);
    client.execute("DELETE FROM sessions WHERE user_id = $1", &[&id]).await?;
    client.execute("DELETE FROM password_resets WHERE user_id = $1", &[&id]).await?;
    Ok(())
}

/// A password-reset link's token for the account of `email`, if there is one (an earlier link
/// no longer valid).
pub async fn reset_start(pool: &Pool, email: &str) -> Result<Option<(String, User)>, Error> {
    let Ok(email) = check_email(email) else { return Ok(None) };
    let client = pool.get().await?;
    let Some(row) = client.query_opt("SELECT id, name, email, admin, language FROM users WHERE email = $1", &[&email]).await? else {
        return Ok(None);
    };
    let user = User { id: row.get(0), name: row.get(1), email: row.get(2), admin: row.get(3), language: row.get(4) };
    let token = random_hex(32);
    client.execute("DELETE FROM password_resets WHERE user_id = $1 OR expires_at < now()", &[&user.id]).await?;
    client
        .execute("INSERT INTO password_resets (token_hash, user_id, expires_at) VALUES ($1, $2, now() + make_interval(mins => $3))", &[&token_hash(&token), &user.id, &RESET_MINUTES])
        .await?;
    Ok(Some((token, user)))
}

/// Whether a password-reset link is still valid (before showing its form).
pub async fn reset_valid(pool: &Pool, token: &str) -> Result<bool, Error> {
    let client = pool.get().await?;
    Ok(client.query_opt("SELECT 1 FROM password_resets WHERE token_hash = $1 AND expires_at > now()", &[&token_hash(token)]).await?.is_some())
}

/// A new password with a reset link: the link used up, the account signed out everywhere.
pub async fn reset_finish(pool: &Pool, token: &str, password: &str) -> Result<Result<User, Invalid>, Error> {
    if let Err(e) = check_password(password) {
        return Ok(Err(e));
    }
    let hash = hash_password(password)?;
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let Some(row) = tx.query_opt("DELETE FROM password_resets WHERE token_hash = $1 AND expires_at > now() RETURNING user_id", &[&token_hash(token)]).await? else {
        return Ok(Err(Invalid::Link));
    };
    let id: i64 = row.get(0);
    let row = tx.query_one("UPDATE users SET password_hash = $2 WHERE id = $1 RETURNING name, email, admin, language", &[&id, &hash]).await?;
    tx.execute("DELETE FROM sessions WHERE user_id = $1", &[&id]).await?;
    tx.execute("DELETE FROM password_resets WHERE user_id = $1", &[&id]).await?;
    tx.commit().await?;
    Ok(Ok(User { id, name: row.get(0), email: row.get(1), admin: row.get(2), language: row.get(3) }))
}

/// A token for a device signed in as `user`.
async fn new_session(pool: &Pool, user: i64, device: &str) -> Result<String, Error> {
    let token = random_hex(32);
    pool.get()
        .await?
        .execute(
            "INSERT INTO sessions (token_hash, user_id, device, expires_at) VALUES ($1, $2, $3, now() + make_interval(days => $4))",
            &[&token_hash(&token), &user, &device.chars().take(100).collect::<String>(), &TOKEN_DAYS],
        )
        .await?;
    Ok(token)
}

/// Sign in: a token for the device, and who it is; `None` for a wrong e-mail or password.
pub async fn sign_in(pool: &Pool, email: &str, password: &str, device: &str) -> Result<Option<(String, User)>, Error> {
    let client = pool.get().await?;
    let Some(row) = client.query_opt("SELECT id, name, email, admin, language, password_hash FROM users WHERE email = $1", &[&email.trim().to_lowercase()]).await? else {
        // The same work as for a known address: no hint which addresses exist.
        static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        let _ = password_ok(password, DUMMY.get_or_init(|| hash_password("no account here").unwrap_or_default()));
        return Ok(None);
    };
    if !password_ok(password, row.get(5)) {
        return Ok(None);
    }
    let user = User { id: row.get(0), name: row.get(1), email: row.get(2), admin: row.get(3), language: row.get(4) };
    drop(client);
    let token = new_session(pool, user.id, device).await?;
    Ok(Some((token, user)))
}

/// Who a token belongs to (a valid one); its use is noted, which keeps it valid.
pub async fn user_of(pool: &Pool, token: &str) -> Result<Option<User>, Error> {
    let client = pool.get().await?;
    let row = client
        .query_opt(
            "UPDATE sessions SET last_used = now(), expires_at = now() + make_interval(days => $2)
             WHERE token_hash = $1 AND expires_at > now() RETURNING user_id",
            &[&token_hash(token), &TOKEN_DAYS],
        )
        .await?;
    let Some(row) = row else { return Ok(None) };
    let id: i64 = row.get(0);
    let row = client.query_one("SELECT name, email, admin, language FROM users WHERE id = $1", &[&id]).await?;
    Ok(Some(User { id, name: row.get(0), email: row.get(1), admin: row.get(2), language: row.get(3) }))
}

/// Sign out a token.
pub async fn sign_out(pool: &Pool, token: &str) -> Result<(), Error> {
    pool.get().await?.execute("DELETE FROM sessions WHERE token_hash = $1", &[&token_hash(token)]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_and_tokens() {
        let hash = hash_password("correct horse battery").unwrap();
        assert!(password_ok("correct horse battery", &hash) && !password_ok("wrong", &hash));
        assert_eq!(random_hex(32).len(), 64);
        assert_ne!(random_hex(8), random_hex(8));
        let p = new_password();
        assert_eq!((p.len(), p.matches('-').count()), (19, 3));
        assert_eq!(token_hash("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn what_an_account_needs() {
        assert_eq!(check_name("  Anne   Office "), Ok("Anne Office".into()));
        assert_eq!(check_name("   "), Err(Invalid::Name));
        assert_eq!(check_email(" Anne.Office@Example.CH "), Ok("anne.office@example.ch".into()));
        for bad in ["anne", "anne@", "@example.ch", "anne@example", "anne@@example.ch", "an ne@example.ch", "anne@example..ch", "Anne <anne@example.ch>", "anne@example.ch\r\nBcc: x@y.ch"] {
            assert_eq!(check_email(bad), Err(Invalid::Email), "{bad}");
        }
        assert_eq!(check_password("short"), Err(Invalid::Password));
        assert_eq!(check_password("ten chars!"), Ok(()));
        assert_eq!(check_password(&"x".repeat(201)), Err(Invalid::Password));
    }
}
