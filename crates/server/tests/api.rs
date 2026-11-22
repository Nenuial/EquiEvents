//! The server's API against a real Postgres: a database of its own for each test, dropped at
//! the end. Postgres from `TEST_DATABASE_URL` (default: the local one, as the current user);
//! without one the tests say so and pass (the server's tests need a database).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use server::lang::Lang;
use server::{api, auth, db, mail};
use tower::ServiceExt;

struct TestDb {
    admin: String,
    name: String,
    url: String,
}

impl TestDb {
    async fn new(tag: &str) -> Option<TestDb> {
        let user = std::env::var("USER").unwrap_or_else(|_| "postgres".into());
        let admin = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| format!("host=localhost user={user} dbname=postgres"));
        let Ok((client, conn)) = tokio_postgres::connect(&admin, tokio_postgres::NoTls).await else {
            eprintln!("skipped: no Postgres at {admin}");
            return None;
        };
        tokio::spawn(conn);
        let name = format!("equievents_test_{tag}_{}", std::process::id());
        client.batch_execute(&format!("DROP DATABASE IF EXISTS {name}")).await.unwrap();
        client.batch_execute(&format!("CREATE DATABASE {name}")).await.unwrap();
        let url = if admin.contains("dbname=") { admin.replace("dbname=postgres", &format!("dbname={name}")) } else { format!("{}/{name}", admin.rsplit_once('/').unwrap().0) };
        Some(TestDb { admin, name, url })
    }

    async fn drop(self) {
        let (client, conn) = tokio_postgres::connect(&self.admin, tokio_postgres::NoTls).await.unwrap();
        tokio::spawn(conn);
        client.batch_execute(&format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", self.name)).await.unwrap();
    }
}

struct Server {
    router: axum::Router,
    pool: deadpool_postgres::Pool,
    mailer: mail::Mailer,
}

impl Server {
    async fn call(&self, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
        self.call_from("10.0.0.1", method, path, token, body).await
    }

    /// From a client address (as Nginx Proxy Manager passes it on).
    async fn call_from(&self, client: &str, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
        self.call_as_app(client, Some(show::oplog::SYNC_VERSION), method, path, token, body).await
    }

    /// From an app speaking this sync version (`None`: one from before it said).
    async fn call_as_app(&self, client: &str, sync: Option<u32>, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(path).header("content-type", "application/json").header("x-real-ip", client);
        if let Some(v) = sync {
            req = req.header("x-equievents-sync", v.to_string());
        }
        if let Some(t) = token {
            req = req.header("authorization", format!("Bearer {t}"));
        }
        let req = req.body(body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty)).unwrap();
        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    /// A page of the site (`form`: a form sent, urlencoded): its status and HTML.
    async fn page(&self, path: &str, form: Option<&[(&str, &str)]>) -> (StatusCode, String) {
        let req = Request::builder().uri(path).header("x-real-ip", "10.0.0.2");
        let req = match form {
            Some(fields) => {
                let body = fields.iter().map(|(k, v)| format!("{k}={}", urlencode(v))).collect::<Vec<_>>().join("&");
                req.method("POST").header("content-type", "application/x-www-form-urlencoded").body(Body::from(body)).unwrap()
            }
            None => req.method("GET").body(Body::empty()).unwrap(),
        };
        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    async fn sign_in(&self, email: &str, password: &str) -> String {
        let (status, body) = self.call("POST", "/api/login", None, Some(json!({ "email": email, "password": password, "device": "test" }))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["token"].as_str().unwrap().to_string()
    }
}

async fn server(db: &TestDb) -> Server {
    let pool = db::pool(&db.url).unwrap();
    db::migrate(&pool).await.unwrap();
    // Twice: nothing more to do.
    assert!(db::migrate(&pool).await.unwrap().is_empty());
    let mailer = mail::Mailer::off();
    let app = api::App { mail_in_background: false, ..api::App::new(pool.clone(), mailer.clone(), "https://www.equievents.ch/") };
    Server { router: api::router(app), pool, mailer }
}

fn urlencode(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// A show's operations, as an app has them: a whole corpus show imported from Rosson.
fn corpus_ops() -> Vec<Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus/2026.8352/snapshot.json");
    let snapshot: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    rosson_import::ops::to_operations(&snapshot).unwrap().into_iter().map(|o| serde_json::to_value(o).unwrap()).collect()
}

#[tokio::test]
async fn accounts_shows_and_sync() {
    let Some(test_db) = TestDb::new("sync").await else { return };
    let s = server(&test_db).await;

    // Health.
    let (status, health) = s.call("GET", "/api/health", None, None).await;
    assert_eq!((status, health["database"].clone()), (StatusCode::OK, json!(true)));

    // Accounts: made by the administrator; signing in gives a token.
    auth::create_user(&s.pool, "Anne Office", "Anne@Example.ch", "first-password-1", false, Lang::Fr).await.unwrap().unwrap();
    auth::create_user(&s.pool, "Bruno Second", "bruno@example.ch", "second-password-2", false, Lang::Fr).await.unwrap().unwrap();
    assert_eq!(auth::create_user(&s.pool, "Anne again", "anne@example.ch", "another-password", false, Lang::Fr).await.unwrap(), Err(auth::Invalid::Taken));
    assert_eq!(auth::create_user(&s.pool, "Short", "short@example.ch", "short", false, Lang::Fr).await.unwrap(), Err(auth::Invalid::Password));
    let (status, _) = s.call("POST", "/api/login", None, Some(json!({ "email": "anne@example.ch", "password": "wrong" }))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let anne = s.sign_in("anne@example.ch", "first-password-1").await;
    let bruno = s.sign_in("bruno@example.ch", "second-password-2").await;
    assert_eq!(s.call("GET", "/api/me", Some(&anne), None).await.1["name"], "Anne Office");
    assert_eq!(s.call("GET", "/api/shows", None, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(s.call("GET", "/api/shows", Some("nonsense"), None).await.0, StatusCode::UNAUTHORIZED);

    // A show: Anne's; Bruno does not see it until she adds him.
    let (_, made) = s.call("POST", "/api/shows", Some(&anne), Some(json!({ "name": "CSN Chalet-à-Gobet", "event_id": 8352, "number": "2026.8352", "access_key": "KEY-8352" }))).await;
    let show = made["id"].as_str().unwrap().to_string();
    assert!(show.starts_with("2026.8352-"));
    assert_eq!(s.call("GET", &format!("/api/shows/{show}/ops"), Some(&bruno), None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(s.call("POST", &format!("/api/shows/{show}/members"), Some(&bruno), Some(json!({ "email": "bruno@example.ch" }))).await.0, StatusCode::NOT_FOUND);
    assert_eq!(s.call("POST", &format!("/api/shows/{show}/members"), Some(&anne), Some(json!({ "email": "bruno@example.ch" }))).await.0, StatusCode::OK);
    assert_eq!(s.call("POST", &format!("/api/shows/{show}/members"), Some(&anne), Some(json!({ "email": "nobody@example.ch" }))).await.0, StatusCode::BAD_REQUEST);
    let (_, listed) = s.call("GET", "/api/shows", Some(&bruno), None).await;
    assert_eq!((listed[0]["id"].as_str(), listed[0]["role"].as_str()), (Some(show.as_str()), Some("collaborator")));
    // Bruno works on it, but cannot hand it to others.
    assert_eq!(s.call("POST", &format!("/api/shows/{show}/members"), Some(&bruno), Some(json!({ "email": "anne@example.ch" }))).await.0, StatusCode::FORBIDDEN);

    // Sync: Anne pushes the show's operations; again, nothing new; Bruno pulls them.
    let ops = make_ops(5);
    // An app from before what changes saw was said (R12): told to update, nothing stored.
    let (status, old) = s.call_as_app("10.0.0.1", None, "POST", &format!("/api/shows/{show}/ops"), Some(&anne), Some(json!({ "ops": ops }))).await;
    assert_eq!((status, old["code"].as_str()), (StatusCode::UPGRADE_REQUIRED, Some("update")), "{old}");
    assert!(old["error"].as_str().unwrap().starts_with("Mettez EquiEvents à jour"), "{old}");
    assert_eq!(s.call_as_app("10.0.0.1", Some(1), "POST", &format!("/api/shows/{show}/ops"), Some(&anne), Some(json!({ "ops": ops }))).await.0, StatusCode::UPGRADE_REQUIRED);
    let (status, pushed) = s.call("POST", &format!("/api/shows/{show}/ops"), Some(&anne), Some(json!({ "ops": ops }))).await;
    assert_eq!(status, StatusCode::OK, "{pushed}");
    assert_eq!((pushed["added"].clone(), pushed["known"].clone()), (json!(5), json!(0)));
    let (_, again) = s.call("POST", &format!("/api/shows/{show}/ops"), Some(&anne), Some(json!({ "ops": ops[..3] }))).await;
    assert_eq!((again["added"].clone(), again["known"].clone(), again["cursor"].clone()), (json!(0), json!(3), pushed["cursor"].clone()));
    let (_, pulled) = s.call("GET", &format!("/api/shows/{show}/ops"), Some(&bruno), None).await;
    assert_eq!(pulled["ops"].as_array().unwrap().len(), 5);
    assert_eq!(pulled["ops"][0]["id"], ops[0]["id"]);
    assert_eq!(pulled["more"], false);
    // Bruno's own change; Anne pulls only what is new to her.
    let cursor = pulled["cursor"].as_i64().unwrap();
    let mine = make_ops(1).into_iter().map(|mut o| {
        o["id"] = json!("bruno:1:0");
        o
    });
    s.call("POST", &format!("/api/shows/{show}/ops"), Some(&bruno), Some(json!({ "ops": mine.collect::<Vec<_>>() }))).await;
    let (_, news) = s.call("GET", &format!("/api/shows/{show}/ops?after={cursor}"), Some(&anne), None).await;
    assert_eq!(news["ops"].as_array().unwrap().len(), 1);
    assert_eq!(news["ops"][0]["id"], "bruno:1:0");

    // From a clock running ahead (rule R5): not stored, said which; the server's clock given.
    let mut ahead = make_ops(1);
    ahead[0]["id"] = json!("fast:1");
    ahead[0]["stamp"]["millis"] = json!(4_000_000_000_000u64);
    let (_, pushed) = s.call("POST", &format!("/api/shows/{show}/ops"), Some(&anne), Some(json!({ "ops": ahead }))).await;
    assert_eq!((pushed["added"].clone(), pushed["refused"].clone()), (json!(0), json!(["fast:1"])));
    assert!(pushed["now"].as_u64().unwrap() > 1_700_000_000_000);

    // Not an operation: nothing stored, said why.
    let (status, err) = s.call("POST", &format!("/api/shows/{show}/ops"), Some(&anne), Some(json!({ "ops": [{ "id": "x" }] }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err["error"].as_str().unwrap().contains("operation 1"));

    // Signed out: the token no longer works; a new password signs out everywhere.
    s.call("POST", "/api/logout", Some(&bruno), None).await;
    assert_eq!(s.call("GET", "/api/me", Some(&bruno), None).await.0, StatusCode::UNAUTHORIZED);
    auth::set_password(&s.pool, "anne@example.ch", "a-new-password-3").await.unwrap();
    assert_eq!(s.call("GET", "/api/me", Some(&anne), None).await.0, StatusCode::UNAUTHORIZED);
    s.sign_in("anne@example.ch", "a-new-password-3").await;

    drop(s);
    test_db.drop().await;
}

/// Archives: the owner archives a show (every member sees it), or their account does it some
/// weeks after the show's last day, which the server takes from the operations; a show taken out
/// of the archives stays out.
#[tokio::test]
async fn shows_are_archived_by_their_owner() {
    let Some(test_db) = TestDb::new("archives").await else { return };
    let s = server(&test_db).await;
    auth::create_user(&s.pool, "Anne Office", "anne@example.ch", "first-password-1", false, Lang::Fr).await.unwrap().unwrap();
    auth::create_user(&s.pool, "Bruno Second", "bruno@example.ch", "second-password-2", false, Lang::Fr).await.unwrap().unwrap();
    let anne = s.sign_in("anne@example.ch", "first-password-1").await;
    let bruno = s.sign_in("bruno@example.ch", "second-password-2").await;
    let (_, made) = s.call("POST", "/api/shows", Some(&anne), Some(json!({ "name": "CSN Chalet-à-Gobet", "event_id": 8352, "number": "2026.8352", "access_key": "KEY-8352" }))).await;
    let show = made["id"].as_str().unwrap().to_string();
    s.call("POST", &format!("/api/shows/{show}/members"), Some(&anne), Some(json!({ "email": "bruno@example.ch" }))).await;
    let listed = |token: String| {
        let s = &s;
        async move { s.call("GET", "/api/shows", Some(&token), None).await.1 }
    };

    // The last day comes with the event.
    let (status, _) = s.call("POST", &format!("/api/shows/{show}/ops"), Some(&anne), Some(json!({ "ops": corpus_ops() }))).await;
    assert_eq!(status, StatusCode::OK);
    let mine = listed(anne.clone()).await;
    assert_eq!((mine[0]["last_day"].as_str(), mine[0]["archived"].as_bool()), (Some("2026-09-27"), Some(false)));

    // Only the owner archives; every member sees it archived.
    let (status, refused) = s.call("POST", &format!("/api/shows/{show}/archive"), Some(&bruno), Some(json!({ "archived": true }))).await;
    assert_eq!((status, refused["code"].as_str()), (StatusCode::FORBIDDEN, Some("not_owner")));
    assert_eq!(s.call("POST", &format!("/api/shows/{show}/archive"), Some(&anne), Some(json!({ "archived": true }))).await.0, StatusCode::OK);
    assert_eq!(listed(bruno.clone()).await[0]["archived"], true);
    assert_eq!(s.call("POST", &format!("/api/shows/{show}/archive"), Some(&anne), Some(json!({ "archived": false }))).await.0, StatusCode::OK);
    assert_eq!(listed(bruno.clone()).await[0]["archived"], false);

    // The account's setting: some weeks after the last day (1 to 104, or none).
    assert_eq!(s.call("GET", "/api/me", Some(&anne), None).await.1["archive_after_weeks"], Value::Null);
    assert_eq!(s.call("POST", "/api/me", Some(&anne), Some(json!({ "archive_after_weeks": 0 }))).await.0, StatusCode::BAD_REQUEST);
    let (_, me) = s.call("POST", "/api/me", Some(&anne), Some(json!({ "archive_after_weeks": 2 }))).await;
    assert_eq!((me["archive_after_weeks"].as_i64(), me["name"].as_str()), (Some(2), Some("Anne Office")));
    // A rehearsal that ended 10 days ago: not yet after two weeks, archived after one, also
    // when a member lists it. The show taken out of the archives by hand stays out.
    let (_, rehearsal) = s.call("POST", "/api/shows", Some(&anne), Some(json!({ "name": "Rehearsal", "number": "2026.8352" }))).await;
    let rehearsal = rehearsal["id"].as_str().unwrap().to_string();
    s.call("POST", &format!("/api/shows/{rehearsal}/members"), Some(&anne), Some(json!({ "email": "bruno@example.ch" }))).await;
    let client = s.pool.get().await.unwrap();
    client.execute("UPDATE shows SET last_day = current_date - 10", &[]).await.unwrap();
    let archived = |list: &Value, id: &str| list.as_array().unwrap().iter().find(|x| x["id"] == id).unwrap()["archived"].as_bool().unwrap();
    assert!(!archived(&listed(bruno.clone()).await, &rehearsal));
    s.call("POST", "/api/me", Some(&anne), Some(json!({ "archive_after_weeks": 1 }))).await;
    let now = listed(bruno.clone()).await;
    assert!(archived(&now, &rehearsal) && !archived(&now, &show));
    // By hand only again.
    s.call("POST", "/api/me", Some(&anne), Some(json!({ "archive_after_weeks": null }))).await;
    assert_eq!(s.call("GET", "/api/me", Some(&anne), None).await.1["archive_after_weeks"], Value::Null);

    drop(client);
    drop(s);
    test_db.drop().await;
}

/// `n` operations as an app makes them (a class's options changed n times).
fn make_ops(n: usize) -> Vec<Value> {
    (0..n)
        .map(|i| {
            json!({
                "id": format!("laptop:{}:{i}", 1_700_000_000_000u64 + i as u64),
                "stamp": { "millis": 1_700_000_000_000u64 + i as u64, "counter": 0, "device": "laptop" },
                "author": "Anne Office",
                "change": { "kind": "ClassReopened", "class": "1", "reason": format!("correction {i}") }
            })
        })
        .collect()
}

#[tokio::test]
async fn a_whole_show_goes_up_and_comes_back_the_same() {
    let ops = corpus_ops();
    assert!(ops.len() > 100);
    let Some(test_db) = TestDb::new("whole").await else { return };
    let s = server(&test_db).await;
    auth::create_user(&s.pool, "Anne Office", "anne@example.ch", "first-password-1", false, Lang::Fr).await.unwrap().unwrap();
    let anne = s.sign_in("anne@example.ch", "first-password-1").await;
    let (_, made) = s.call("POST", "/api/shows", Some(&anne), Some(json!({ "name": "CSN" }))).await;
    let show = made["id"].as_str().unwrap().to_string();
    s.call("POST", &format!("/api/shows/{show}/ops"), Some(&anne), Some(json!({ "ops": ops }))).await;
    let mut back = Vec::new();
    let mut after = 0;
    loop {
        let (_, pulled) = s.call("GET", &format!("/api/shows/{show}/ops?after={after}"), Some(&anne), None).await;
        back.extend(pulled["ops"].as_array().unwrap().iter().cloned());
        after = pulled["cursor"].as_i64().unwrap();
        if pulled["more"] == false {
            break;
        }
    }
    assert_eq!(back, ops);
    drop(s);
    test_db.drop().await;
}

#[tokio::test]
async fn anyone_makes_an_account() {
    let Some(test_db) = TestDb::new("register").await else { return };
    let s = server(&test_db).await;

    // From an app: made and signed in.
    let (status, made) = s.call("POST", "/api/register", None, Some(json!({ "name": " Carla  Neu ", "email": "Carla@Example.ch", "password": "carla-password-1", "device": "laptop" }))).await;
    assert_eq!(status, StatusCode::OK, "{made}");
    let token = made["token"].as_str().unwrap();
    let (_, me) = s.call("GET", "/api/me", Some(token), None).await;
    assert_eq!((me["name"].as_str(), me["email"].as_str(), me["admin"].as_bool()), (Some("Carla Neu"), Some("carla@example.ch"), Some(false)));
    // An account opens no show.
    assert_eq!(s.call("GET", "/api/shows", Some(token), None).await.1, json!([]));
    // Refused, with a code the apps translate.
    for (body, status, code) in [
        (json!({ "name": "Carla", "email": "carla@example.ch", "password": "another-password" }), StatusCode::CONFLICT, "taken"),
        (json!({ "name": "Dora", "email": "dora@example", "password": "dora-password-1" }), StatusCode::BAD_REQUEST, "email"),
        (json!({ "name": "Dora", "email": "dora@example.ch", "password": "short" }), StatusCode::BAD_REQUEST, "password"),
        (json!({ "name": "  ", "email": "dora@example.ch", "password": "dora-password-1" }), StatusCode::BAD_REQUEST, "name"),
    ] {
        let (got, err) = s.call("POST", "/api/register", None, Some(body)).await;
        assert_eq!((got, err["code"].as_str()), (status, Some(code)), "{err}");
    }

    // Limits: so many new accounts from one address, then a pause.
    for i in 0..10 {
        let (status, _) = s.call_from("10.9.9.9", "POST", "/api/register", None, Some(json!({ "name": "Bot", "email": format!("bot{i}@example.ch"), "password": "bot-password-1" }))).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, _) = s.call_from("10.9.9.9", "POST", "/api/register", None, Some(json!({ "name": "Bot", "email": "bot-x@example.ch", "password": "bot-password-1" }))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    // Guessing a password: so many tries per account, then a pause (the right one included).
    for _ in 0..10 {
        assert_eq!(s.call_from("10.8.8.8", "POST", "/api/login", None, Some(json!({ "email": "carla@example.ch", "password": "guess" }))).await.0, StatusCode::UNAUTHORIZED);
    }
    let (status, _) = s.call_from("10.7.7.7", "POST", "/api/login", None, Some(json!({ "email": "carla@example.ch", "password": "carla-password-1" }))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

    drop(s);
    test_db.drop().await;
}

#[tokio::test]
async fn the_site_signs_people_up_and_resets_passwords() {
    let Some(test_db) = TestDb::new("site").await else { return };
    let s = server(&test_db).await;

    // Home is signing in (the user, 2026-11-21), in one language: the browser's (else German),
    // the one chosen last on the site first; the others a click away.
    let (status, home) = s.page("/", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(home.contains("<html lang=\"de\"") && home.contains("action=\"/anmelden\"") && home.contains("href=\"/registrieren\""), "{home}");
    assert!(home.contains("href=\"/connexion\"") && home.contains("href=\"/accedi\"") && !home.contains("Bienvenue"));
    let home = |headers: &'static [(&'static str, &'static str)]| {
        let router = s.router.clone();
        async move {
            let mut req = Request::builder().uri("/").header("x-real-ip", "10.0.0.2");
            for (k, v) in headers {
                req = req.header(*k, *v);
            }
            let res = router.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
            let cookie = res.headers().get_all("set-cookie").iter().map(|v| v.to_str().unwrap().to_string()).collect::<Vec<_>>().join(" | ");
            (String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap(), cookie)
        }
    };
    let (page, remembered) = home(&[("accept-language", "it-CH,it;q=0.9,de;q=0.8")]).await;
    assert!(page.contains("<html lang=\"it\"") && page.contains("Benvenuti in EquiEvents") && page.contains("action=\"/accedi\"") && remembered.contains("equievents_lang=it"), "{remembered}");
    let (page, _) = home(&[("accept-language", "fr-CH,fr;q=0.9"), ("cookie", "equievents_lang=de")]).await;
    assert!(page.contains("Willkommen bei EquiEvents"));
    let (page, _) = home(&[("accept-language", "fr-CH,fr;q=0.9")]).await;
    assert!(page.contains("Bienvenue dans EquiEvents"));
    let (_, form) = s.page("/registrieren", None).await;
    assert!(form.contains("Konto erstellen") && form.contains("lang=\"de\"") && form.contains("href=\"/inscription\"") && form.contains("href=\"/registrati\""));
    let (_, form) = s.page("/registrati", None).await;
    assert!(form.contains("Crea un account") && form.contains("<html lang=\"it\""));

    // Signing up.
    let fields = [("name", "Émile Büro"), ("email", "emile@example.ch"), ("password", "emile-password-1"), ("website", "")];
    let (status, done) = s.page("/inscription", Some(&fields)).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert!(done.contains("Votre compte est prêt") && done.contains("emile@example.ch"));
    s.sign_in("emile@example.ch", "emile-password-1").await;
    // Again: the address has an account; the form keeps what was typed (not the password).
    let (status, again) = s.page("/inscription", Some(&fields)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(again.contains("déjà un compte") && again.contains("value=\"Émile Büro\"") && !again.contains("emile-password-1"));
    // What is typed is shown as text, never as HTML.
    let (_, odd) = s.page("/inscription", Some(&[("name", "<script>x</script>"), ("email", "nope"), ("password", "whatever-123")])).await;
    assert!(odd.contains("&lt;script&gt;") && !odd.contains("<script>x"));
    // A robot fills the hidden field: told it worked, nothing made.
    let (status, _) = s.page("/inscription", Some(&[("name", "Bot"), ("email", "bot@example.ch"), ("password", "bot-password-1"), ("website", "http://spam")])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s.call("POST", "/api/login", None, Some(json!({ "email": "bot@example.ch", "password": "bot-password-1" }))).await.0, StatusCode::UNAUTHORIZED);

    // A forgotten password: the same answer with or without an account; an e-mail only for one.
    let app_token = s.sign_in("emile@example.ch", "emile-password-1").await;
    let (_, unknown) = s.page("/passwort", Some(&[("email", "nobody@example.ch")])).await;
    assert!(unknown.contains("Schauen Sie in Ihre E-Mails"));
    assert!(s.mailer.kept().is_empty());
    let (_, sent) = s.page("/passwort", Some(&[("email", " Emile@Example.ch ")])).await;
    assert!(sent.contains("Schauen Sie in Ihre E-Mails"));
    let mail = s.mailer.kept().pop().expect("the reset e-mail");
    assert_eq!((mail.to.as_str(), mail.subject.as_str()), ("emile@example.ch", "EquiEvents: neues Passwort"));
    let link = mail.body.lines().find(|l| l.starts_with("https://www.equievents.ch/neues-passwort?t=")).expect("the link").to_string();
    let path = link.trim_start_matches("https://www.equievents.ch");
    let token = path.split("t=").nth(1).unwrap();
    // The link: a form; a short password refused; then the new one, everywhere signed out.
    let (status, form) = s.page(path, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(form.contains("Neues Passwort") && form.contains(token));
    let (status, _) = s.page("/neues-passwort", Some(&[("t", token), ("password", "short")])).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, changed) = s.page("/neues-passwort", Some(&[("t", token), ("password", "emile-new-password")])).await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert!(changed.contains("Passwort geändert"));
    assert_eq!(s.call("GET", "/api/me", Some(&app_token), None).await.0, StatusCode::UNAUTHORIZED);
    s.sign_in("emile@example.ch", "emile-new-password").await;
    // Used once.
    assert_eq!(s.page(path, None).await.0, StatusCode::GONE);
    assert_eq!(s.page("/neues-passwort", Some(&[("t", token), ("password", "yet-another-one")])).await.0, StatusCode::GONE);

    // From an app: the same, in French.
    let (status, _) = s.call("POST", "/api/password-reset", None, Some(json!({ "email": "emile@example.ch", "language": "fr" }))).await;
    assert_eq!(status, StatusCode::OK);
    let mail = s.mailer.kept().pop().unwrap();
    assert!(mail.subject.contains("nouveau mot de passe") && mail.body.contains("https://www.equievents.ch/nouveau-mot-de-passe?t="));
    // Asking again: only the newest link works.
    s.call("POST", "/api/password-reset", None, Some(json!({ "email": "emile@example.ch", "language": "fr" }))).await;
    let newest = s.mailer.kept().pop().unwrap();
    assert_ne!(newest.body, mail.body);
    let old_path = mail.body.lines().find(|l| l.contains("?t=")).unwrap().trim_start_matches("https://www.equievents.ch").to_string();
    assert_eq!(s.page(&old_path, None).await.0, StatusCode::GONE);
    // So many e-mails per account in an hour, then none (the answer still the same).
    s.call("POST", "/api/password-reset", None, Some(json!({ "email": "emile@example.ch" }))).await;
    let before = s.mailer.kept().len();
    assert_eq!(s.call("POST", "/api/password-reset", None, Some(json!({ "email": "emile@example.ch" }))).await.0, StatusCode::OK);
    assert_eq!(s.mailer.kept().len(), before);

    drop(s);
    test_db.drop().await;
}

/// A show's key decides who owns it: the first to enter it; the others ask the owner for access
/// or for the show, the owner answers (a transfer: staying on as a collaborator or leaving),
/// gives access without a request, takes it away. One owner at every moment.
#[tokio::test]
async fn the_key_decides_who_owns_a_show() {
    let Some(test_db) = TestDb::new("owners").await else { return };
    let s = server(&test_db).await;
    let mut tokens = std::collections::BTreeMap::new();
    for (name, lang) in [("Anne", Lang::De), ("Bruno", Lang::Fr), ("Carla", Lang::Fr), ("Dora", Lang::Fr), ("Erik", Lang::De)] {
        let email = format!("{}@example.ch", name.to_lowercase());
        auth::create_user(&s.pool, name, &email, "a-good-password", false, lang).await.unwrap().unwrap();
        tokens.insert(name, s.sign_in(&email, "a-good-password").await);
    }
    let t = |name: &str| tokens[name].clone();
    let event = json!({ "name": "CSN Chalet-à-Gobet", "event_id": 8352, "number": "2026.8352", "access_key": "KEY-8352" });
    let owners = || async {
        let client = s.pool.get().await.unwrap();
        client.query_one("SELECT count(*) FROM show_members WHERE role = 'owner'", &[]).await.unwrap().get::<_, i64>(0)
    };
    let mails_to = |email: &str| s.mailer.kept().into_iter().filter(|m| m.to == email).collect::<Vec<_>>();

    // Anne enters the key first: the show is made, hers. Again: hers still.
    let (status, made) = s.call("POST", "/api/shows", Some(&t("Anne")), Some(event.clone())).await;
    assert_eq!((status, made["role"].as_str(), made["created"].as_bool()), (StatusCode::OK, Some("owner"), Some(true)), "{made}");
    let show = made["id"].as_str().unwrap().to_string();
    let (_, again) = s.call("POST", "/api/shows", Some(&t("Anne")), Some(event.clone())).await;
    assert_eq!((again["id"].as_str(), again["created"].as_bool()), (Some(show.as_str()), Some(false)));

    // Bruno: a wrong key opens nothing; the right one says who owns the show.
    let wrong = json!({ "name": "CSN", "event_id": 8352, "access_key": "guess" });
    let (status, err) = s.call("POST", "/api/shows", Some(&t("Bruno")), Some(wrong)).await;
    assert_eq!((status, err["code"].as_str()), (StatusCode::FORBIDDEN, Some("key")));
    let (status, owned) = s.call("POST", "/api/shows", Some(&t("Bruno")), Some(event.clone())).await;
    assert_eq!((status, owned["code"].as_str(), owned["id"].as_str(), owned["owner"].as_str()), (StatusCode::CONFLICT, Some("claimed"), Some(show.as_str()), Some("Anne")));
    assert_eq!(s.call("GET", &format!("/api/shows/{show}/ops"), Some(&t("Bruno")), None).await.0, StatusCode::NOT_FOUND);
    // An event's show needs its key.
    assert_eq!(s.call("POST", "/api/shows", Some(&t("Bruno")), Some(json!({ "name": "CSN", "event_id": 8352 }))).await.0, StatusCode::BAD_REQUEST);

    // Bruno asks for access (with the key); Anne gets an e-mail, in German.
    let ask = |kind: &str, key: &str| json!({ "kind": kind, "access_key": key });
    let (status, _) = s.call("POST", &format!("/api/shows/{show}/requests"), Some(&t("Bruno")), Some(ask("access", "guess"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, bruno_asks) = s.call("POST", &format!("/api/shows/{show}/requests"), Some(&t("Bruno")), Some(ask("access", "KEY-8352"))).await;
    assert_eq!((status, bruno_asks["status"].as_str(), bruno_asks["kind"].as_str()), (StatusCode::OK, Some("pending"), Some("access")), "{bruno_asks}");
    let mail = mails_to("anne@example.ch").pop().expect("Anne's e-mail");
    assert_eq!(mail.subject, "EquiEvents: Zugriffsanfrage für CSN Chalet-à-Gobet");
    assert!(mail.body.contains("Bruno (bruno@example.ch) möchte am Turnier CSN Chalet-à-Gobet mitarbeiten"));
    // Entering the key again shows the open request.
    let (_, owned) = s.call("POST", "/api/shows", Some(&t("Bruno")), Some(event.clone())).await;
    assert_eq!(owned["request"]["id"], bruno_asks["id"]);
    // Carla asks for the show itself, takes it back, asks again.
    let (_, carla_asks) = s.call("POST", &format!("/api/shows/{show}/requests"), Some(&t("Carla")), Some(ask("ownership", "KEY-8352"))).await;
    let (status, back) = s.call("DELETE", &format!("/api/requests/{}", carla_asks["id"]), Some(&t("Carla")), None).await;
    assert_eq!((status, back["status"].as_str()), (StatusCode::OK, Some("withdrawn")));
    assert_eq!(s.call("DELETE", &format!("/api/requests/{}", carla_asks["id"]), Some(&t("Carla")), None).await.0, StatusCode::CONFLICT);
    let (_, carla_asks) = s.call("POST", &format!("/api/shows/{show}/requests"), Some(&t("Carla")), Some(ask("ownership", "KEY-8352"))).await;
    assert!(mails_to("anne@example.ch").pop().unwrap().subject.contains("Übernahmeanfrage"));

    // Anne sees them (the list counts them); nobody else answers them.
    let (_, waiting) = s.call("GET", &format!("/api/shows/{show}/requests"), Some(&t("Anne")), None).await;
    assert_eq!(waiting.as_array().unwrap().iter().map(|r| r["person"]["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Bruno", "Carla"]);
    assert_eq!(s.call("GET", "/api/shows", Some(&t("Anne")), None).await.1[0]["requests"], 2);
    assert_eq!(s.call("POST", &format!("/api/requests/{}", carla_asks["id"]), Some(&t("Bruno")), Some(json!({ "accept": true, "stay": true }))).await.0, StatusCode::NOT_FOUND);

    // Anne lets Bruno in: he works on it now, and is told so (in French).
    let (status, answered) = s.call("POST", &format!("/api/requests/{}", bruno_asks["id"]), Some(&t("Anne")), Some(json!({ "accept": true }))).await;
    assert_eq!((status, answered["status"].as_str()), (StatusCode::OK, Some("accepted")));
    assert_eq!(s.call("GET", &format!("/api/shows/{show}/ops"), Some(&t("Bruno")), None).await.0, StatusCode::OK);
    assert_eq!(mails_to("bruno@example.ch").pop().unwrap().subject, "EquiEvents : accès à CSN Chalet-à-Gobet accordé");
    assert_eq!(s.call("POST", &format!("/api/requests/{}", bruno_asks["id"]), Some(&t("Anne")), Some(json!({ "accept": false }))).await.0, StatusCode::CONFLICT);
    // Asking for access when one has it.
    let (status, err) = s.call("POST", &format!("/api/shows/{show}/requests"), Some(&t("Bruno")), Some(ask("access", "KEY-8352"))).await;
    assert_eq!((status, err["code"].as_str()), (StatusCode::CONFLICT, Some("member")));

    // Carla's transfer: Anne must say whether she stays; she leaves.
    let (status, err) = s.call("POST", &format!("/api/requests/{}", carla_asks["id"]), Some(&t("Anne")), Some(json!({ "accept": true }))).await;
    assert_eq!((status, err["code"].as_str()), (StatusCode::BAD_REQUEST, Some("stay")));
    let (status, _) = s.call("POST", &format!("/api/requests/{}", carla_asks["id"]), Some(&t("Anne")), Some(json!({ "accept": true, "stay": false }))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s.call("GET", &format!("/api/shows/{show}/ops"), Some(&t("Anne")), None).await.0, StatusCode::NOT_FOUND, "Anne left");
    let (_, carla_shows) = s.call("GET", "/api/shows", Some(&t("Carla")), None).await;
    assert_eq!((carla_shows[0]["role"].as_str(), carla_shows[0]["owner"].as_str()), (Some("owner"), Some("Carla")));
    assert_eq!(mails_to("carla@example.ch").pop().unwrap().subject, "EquiEvents : vous êtes responsable de CSN Chalet-à-Gobet");
    assert_eq!(owners().await, 1);
    // Anne entering the key now: Carla owns it.
    assert_eq!(s.call("POST", "/api/shows", Some(&t("Anne")), Some(event.clone())).await.1["owner"], "Carla");

    // Carla lets Dora in without a request; Dora is told.
    let (status, added) = s.call("POST", &format!("/api/shows/{show}/members"), Some(&t("Carla")), Some(json!({ "email": "Dora@example.ch" }))).await;
    assert_eq!((status, added["role"].as_str()), (StatusCode::OK, Some("collaborator")));
    assert!(mails_to("dora@example.ch").pop().unwrap().body.contains("Carla vous a donné accès au concours CSN Chalet-à-Gobet"));
    let (status, err) = s.call("POST", &format!("/api/shows/{show}/members"), Some(&t("Carla")), Some(json!({ "email": "nobody@example.ch" }))).await;
    assert_eq!((status, err["code"].as_str()), (StatusCode::BAD_REQUEST, Some("account")));
    // Collaborators do not let others in.
    assert_eq!(s.call("POST", &format!("/api/shows/{show}/members"), Some(&t("Bruno")), Some(json!({ "email": "erik@example.ch" }))).await.0, StatusCode::FORBIDDEN);

    // Bruno, a collaborator, asks for the show; Carla hands it over and stays.
    let (_, bruno_takes) = s.call("POST", &format!("/api/shows/{show}/requests"), Some(&t("Bruno")), Some(ask("ownership", "KEY-8352"))).await;
    s.call("POST", &format!("/api/requests/{}", bruno_takes["id"]), Some(&t("Carla")), Some(json!({ "accept": true, "stay": true }))).await;
    let (_, members) = s.call("GET", &format!("/api/shows/{show}/members"), Some(&t("Dora")), None).await;
    let roles: Vec<(&str, &str)> = members.as_array().unwrap().iter().map(|m| (m["name"].as_str().unwrap(), m["role"].as_str().unwrap())).collect();
    assert_eq!(roles, [("Bruno", "owner"), ("Carla", "collaborator"), ("Dora", "collaborator")]);
    assert_eq!(owners().await, 1);

    // Taking people off: the owner removes Dora; Carla leaves herself; the owner stays.
    let id_of = |name: &str| members.as_array().unwrap().iter().find(|m| m["name"] == name).unwrap()["id"].as_i64().unwrap();
    assert_eq!(s.call("DELETE", &format!("/api/shows/{show}/members/{}", id_of("Bruno")), Some(&t("Carla")), None).await.1["code"], "owner");
    assert_eq!(s.call("DELETE", &format!("/api/shows/{show}/members/{}", id_of("Bruno")), Some(&t("Bruno")), None).await.1["code"], "owner");
    assert_eq!(s.call("DELETE", &format!("/api/shows/{show}/members/{}", id_of("Dora")), Some(&t("Carla")), None).await.0, StatusCode::NOT_FOUND, "not the owner");
    assert_eq!(s.call("DELETE", &format!("/api/shows/{show}/members/{}", id_of("Dora")), Some(&t("Bruno")), None).await.0, StatusCode::OK);
    assert_eq!(s.call("DELETE", &format!("/api/shows/{show}/members/{}", id_of("Carla")), Some(&t("Carla")), None).await.0, StatusCode::OK);
    assert_eq!(s.call("GET", &format!("/api/shows/{show}/members"), Some(&t("Bruno")), None).await.1.as_array().unwrap().len(), 1);

    // Erik is refused: told so; his requests show it.
    let (_, erik_asks) = s.call("POST", &format!("/api/shows/{show}/requests"), Some(&t("Erik")), Some(ask("access", "KEY-8352"))).await;
    s.call("POST", &format!("/api/requests/{}", erik_asks["id"]), Some(&t("Bruno")), Some(json!({ "accept": false }))).await;
    let (_, mine) = s.call("GET", "/api/requests", Some(&t("Erik")), None).await;
    assert_eq!((mine[0]["status"].as_str(), mine[0]["show_name"].as_str()), (Some("refused"), Some("CSN Chalet-à-Gobet")));
    assert_eq!(mails_to("erik@example.ch").pop().unwrap().subject, "EquiEvents: Anfrage für CSN Chalet-à-Gobet abgelehnt");

    // A show without an event: its maker's, no key involved.
    let (status, own) = s.call("POST", "/api/shows", Some(&t("Erik")), Some(json!({ "name": "Répétition" }))).await;
    assert_eq!((status, own["role"].as_str()), (StatusCode::OK, Some("owner")));

    drop(s);
    test_db.drop().await;
}

// --- Two laptops through the server: the office's sync (crates/office/src/sync.rs) ------------

/// A show's main view on a laptop, without its id there (each laptop names its own copy).
async fn view(o: &std::sync::Arc<office::Office>, show: &str) -> Value {
    let mut v = office(o, "show", json!({ "show": show })).await;
    v.as_object_mut().unwrap().remove("id");
    v
}

/// The server on a port of this computer, stopped by sending on the channel.
async fn serve(router: axum::Router, at: Option<std::net::SocketAddr>) -> (String, std::net::SocketAddr, tokio::sync::oneshot::Sender<()>) {
    let listener = tokio::net::TcpListener::bind(at.unwrap_or_else(|| "127.0.0.1:0".parse().unwrap())).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service_with_connect_info::<std::net::SocketAddr>()).with_graceful_shutdown(async { stopped.await.ok(); }).await.unwrap();
    });
    (format!("http://{addr}"), addr, stop)
}

/// An office command, on a thread of its own (its HTTP calls block).
async fn office(o: &std::sync::Arc<office::Office>, command: &str, args: Value) -> Value {
    let (o, command) = (o.clone(), command.to_string());
    tokio::task::spawn_blocking(move || o.call(&command, &args)).await.unwrap().unwrap_or_else(|e| panic!("{e}"))
}

/// A corpus show as a Rosson export file.
fn rosson_file(dir: &std::path::Path) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus/2026.8352/snapshot.json");
    let snapshot: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let encoded: String = snapshot
        .to_string()
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect();
    let file = dir.join("2026.8352_export.txt");
    std::fs::write(&file, format!("Rosson: 3.3.15\n{encoded}")).unwrap();
    file.to_string_lossy().to_string()
}

/// Anne's laptop puts the show on the server with its key; Bruno's, with the same key, asks
/// her for access, downloads the show, changes it; the change reaches Anne. Offline, changes
/// wait in the show file and go up when the server is back. Then Anne hands the show to Bruno
/// and leaves it.
#[tokio::test(flavor = "multi_thread")]
async fn two_laptops_share_a_show() {
    let Some(test_db) = TestDb::new("laptops").await else { return };
    let s = server(&test_db).await;
    let (url, addr, stop) = serve(s.router.clone(), None).await;
    let dir = tempfile::tempdir().unwrap();
    let file = rosson_file(dir.path());
    let laptop = |name: &str| std::sync::Arc::new(office::Office::new(&dir.path().join(name), name).unwrap());
    let (anne, bruno) = (laptop("anne"), laptop("bruno"));
    for (o, name) in [(&anne, "Anne"), (&bruno, "Bruno")] {
        office(o, "set_server", json!({ "url": url })).await;
        assert_eq!(office(o, "account", json!({})).await["signed_in"], false);
        let made = office(o, "sign_up", json!({ "name": name, "email": format!("{}@example.ch", name.to_lowercase()), "password": "a-good-password", "language": "fr" })).await;
        assert_eq!(made["ok"], true, "{made}");
    }
    assert_eq!(office(&anne, "account", json!({})).await["account"]["name"], "Anne");
    let import = |o: std::sync::Arc<office::Office>| {
        let file = file.clone();
        async move {
            let event = office(&o, "rosson_events", json!({ "path": file })).await[0]["id"].as_str().unwrap().to_string();
            office(&o, "import_rosson", json!({ "path": file, "event": event, "mode": "ended" })).await["show"].as_str().unwrap().to_string()
        }
    };

    // Anne: the show goes up with the event's key, and is hers.
    let a_show = import(anne.clone()).await;
    let status = office(&anne, "sync_status", json!({ "show": a_show })).await;
    assert_eq!((status["remote"].clone(), status["needs_key"].clone()), (Value::Null, json!(true)));
    assert_eq!(office(&anne, "put_online", json!({ "show": a_show })).await["outcome"], "need_key");
    let put = office(&anne, "put_online", json!({ "show": a_show, "access_key": "KEY-8352" })).await;
    assert_eq!(put["outcome"], "made", "{put}");
    let remote = put["remote"].as_str().unwrap().to_string();
    // The whole import went up.
    assert!(put["sync"]["pushed"].as_u64().unwrap() > 100, "{put}");
    assert_eq!((put["sync"]["status"].as_str(), put["sync"]["role"].as_str()), (Some("synced"), Some("owner")));
    assert_eq!(office(&anne, "sync_status", json!({ "show": a_show })).await["pending"], 0);

    // Bruno, with his own import and the key: Anne owns it; he asks her for access.
    let b_own = import(bruno.clone()).await;
    let put = office(&bruno, "put_online", json!({ "show": b_own, "access_key": "KEY-8352" })).await;
    assert_eq!((put["outcome"].as_str(), put["owner"].as_str(), put["remote"].as_str()), (Some("owned"), Some("Anne"), Some(remote.as_str())), "{put}");
    let asked = office(&bruno, "ask_owner", json!({ "show": b_own, "remote": remote, "kind": "access" })).await;
    assert_eq!(asked["request"]["status"], "pending", "{asked}");
    let waiting = office(&bruno, "online_shows", json!({})).await;
    assert_eq!((waiting["shows"].as_array().unwrap().len(), waiting["requests"][0]["status"].as_str()), (0, Some("pending")));

    // Anne sees the request at her next sync, and lets him in.
    let note = office(&anne, "sync", json!({ "show": a_show })).await;
    assert_eq!(note["note"]["requests"], 1);
    let requests = office(&anne, "show_requests", json!({ "show": a_show })).await;
    let request = requests["requests"][0]["id"].as_i64().unwrap();
    assert_eq!(office(&anne, "answer_request", json!({ "show": a_show, "id": request, "accept": true })).await["ok"], true);

    // Bruno downloads the show (his own import stays apart): the same show as Anne's.
    let online = office(&bruno, "online_shows", json!({})).await;
    assert_eq!((online["shows"][0]["id"].as_str(), online["shows"][0]["local"].clone()), (Some(remote.as_str()), Value::Null));
    let down = office(&bruno, "download_show", json!({ "remote": remote })).await;
    let b_show = down["show"].as_str().unwrap().to_string();
    assert_ne!(b_show, b_own);
    assert_eq!(view(&bruno, &b_show).await, view(&anne, &a_show).await);
    assert_eq!(office(&bruno, "online_shows", json!({})).await["shows"][0]["local"], json!(b_show));
    assert_eq!(office(&bruno, "download_show", json!({ "remote": remote })).await["show"], json!(b_show), "once");

    // Bruno reopens a class; it reaches Anne.
    office(&bruno, "reopen", json!({ "show": b_show, "class": "108196", "reason": "protest" })).await;
    assert_eq!(office(&bruno, "sync_status", json!({ "show": b_show })).await["pending"], 1);
    assert_eq!(office(&bruno, "sync", json!({ "show": b_show })).await["note"]["pushed"], 1);
    assert_eq!(office(&anne, "sync", json!({ "show": a_show })).await["note"]["pulled"], 1);
    let a_view = view(&anne, &a_show).await;
    assert_eq!(a_view["days"][0]["classes"][0]["status"], "running", "{}", a_view["days"][0]["classes"][0]);
    assert_eq!(view(&bruno, &b_show).await, a_view);

    // Archives: Anne, the responsable, archives the show; Bruno cannot. His library lists it
    // archived after his next sync, offline too; out of the archives the same way. A show only
    // on his laptop is archived there. The account archives on its own after some weeks.
    let library = |o: std::sync::Arc<office::Office>, show: String| async move {
        let all = office(&o, "library", json!({})).await;
        all.as_array().unwrap().iter().find(|x| x["id"] == show.as_str()).unwrap().clone()
    };
    assert_eq!(office(&bruno, "archive_show", json!({ "show": b_show, "archived": true })).await["code"], "not_owner");
    assert_eq!(office(&anne, "archive_show", json!({ "show": a_show, "archived": true })).await["ok"], true);
    let a = library(anne.clone(), a_show.clone()).await;
    assert_eq!((a["archived"].as_bool(), a["online"].as_bool(), a["role"].as_str()), (Some(true), Some(true), Some("owner")));
    assert_eq!(library(bruno.clone(), b_show.clone()).await["archived"], false);
    office(&bruno, "sync_all", json!({})).await;
    let b = library(bruno.clone(), b_show.clone()).await;
    assert_eq!((b["archived"].as_bool(), b["role"].as_str()), (Some(true), Some("collaborator")));
    assert_eq!(office(&bruno, "online_shows", json!({})).await["shows"][0]["archived"], true);
    office(&anne, "archive_show", json!({ "remote": remote, "archived": false })).await;
    office(&bruno, "sync", json!({ "show": b_show })).await;
    assert_eq!(library(bruno.clone(), b_show.clone()).await["archived"], false);
    assert_eq!(office(&bruno, "archive_show", json!({ "show": b_own, "archived": true })).await["ok"], true);
    let own = library(bruno.clone(), b_own.clone()).await;
    assert_eq!((own["archived"].as_bool(), own["online"].as_bool(), own["role"].clone()), (Some(true), Some(false), Value::Null));
    assert_eq!(office(&anne, "set_account_settings", json!({ "archive_after_weeks": 3 })).await["account"]["archive_after_weeks"], 3);
    assert_eq!(office(&anne, "account_settings", json!({})).await["account"]["archive_after_weeks"], 3);

    // The server goes away: Anne's change waits in the file; it goes up when it is back.
    stop.send(()).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    office(&anne, "reopen", json!({ "show": a_show, "class": "108197", "reason": "correction" })).await;
    let offline = office(&anne, "sync", json!({ "show": a_show })).await;
    assert_eq!((offline["note"]["status"].as_str(), offline["pending"].as_u64()), (Some("offline"), Some(1)), "{offline}");
    let (_, _, stop) = serve(s.router.clone(), Some(addr)).await;
    let back = office(&anne, "sync", json!({ "show": a_show })).await;
    assert_eq!((back["note"]["status"].as_str(), back["pending"].as_u64()), (Some("synced"), Some(0)), "{back}");
    let all = office(&bruno, "sync_all", json!({})).await;
    assert_eq!(all[&b_show]["pulled"], 1);

    // Bruno asks for the show itself; Anne hands it over and leaves: her laptop is told.
    let asked = office(&bruno, "ask_owner", json!({ "show": b_show, "remote": remote, "kind": "ownership" })).await;
    let id = asked["request"]["id"].as_i64().unwrap();
    let stay_needed = office(&anne, "answer_request", json!({ "show": a_show, "id": id, "accept": true })).await;
    assert_eq!(stay_needed["code"], "stay");
    assert_eq!(office(&anne, "answer_request", json!({ "show": a_show, "id": id, "accept": true, "stay": false })).await["ok"], true);
    assert_eq!(office(&anne, "sync_status", json!({ "show": a_show })).await["note"]["status"], "no_access");
    assert_eq!(office(&bruno, "sync", json!({ "show": b_show })).await["note"]["role"], "owner");
    // Her copy stays on her laptop, no longer following the server.
    let unlinked = office(&anne, "unlink_show", json!({ "show": a_show })).await;
    assert_eq!(unlinked["remote"], Value::Null);

    // Signed out: the token is gone from the laptop.
    office(&bruno, "sign_out", json!({})).await;
    assert_eq!(office(&bruno, "sync", json!({ "show": b_show })).await["note"]["status"], "signed_out");

    stop.send(()).ok();
    drop(s);
    test_db.drop().await;
}

/// Riders and horses in Swiss Equestrian's registry on a laptop (as `getPersonen` and `getPferde`
/// deliver them): late entries come from there.
fn registry(library: &std::path::Path, rider: (u64, &str, &str), horse: (u64, &str)) {
    let dir = library.join("registry");
    std::fs::create_dir_all(&dir).unwrap();
    let persons = json!([{ "id": rider.0, "vorname": rider.1, "nachname": rider.2, "ort": "Bulle", "vereinsname": "", "lizenzen": "R", "land": "CH", "strasse": "", "postleitzahl": "1630", "email": "" }]);
    let horses = json!([{ "id": horse.0, "name": horse.1, "signalement_label": "Fuchs, Stute, 2015", "besitzer": "", "zuechter": "", "bezahlte_sportjahre": [2025, 2026] }]);
    std::fs::write(dir.join("persons.json"), persons.to_string()).unwrap();
    std::fs::write(dir.join("horses.json"), horses.to_string()).unwrap();
    std::fs::write(dir.join("about.json"), json!({ "persons": 1, "horses": 1, "persons_at": 1_794_000_000_000u64, "horses_at": 1_794_000_000_000u64, "simulated": false }).to_string()).unwrap();
}

/// Imports go through equievents.ch (the user, 2026-11-22: a request, then the same event
/// imported a second time, was a copy apart): the first to bring an event owns it; someone
/// else's is never imported, only asked for, and once let in it comes from equievents.ch; the
/// same file again opens the show one has; no connection, no import. EquiEvents show files too.
#[tokio::test(flavor = "multi_thread")]
async fn imports_go_through_equievents() {
    let Some(test_db) = TestDb::new("imports").await else { return };
    let s = server(&test_db).await;
    let (url, _, stop) = serve(s.router.clone(), None).await;
    let dir = tempfile::tempdir().unwrap();
    let file = rosson_file(dir.path());
    let laptop = |name: &str| std::sync::Arc::new(office::Office::new(&dir.path().join(name), name).unwrap());
    let (anne, bruno, carla) = (laptop("anne"), laptop("bruno"), laptop("carla"));
    for (o, name) in [(&anne, "Anne"), (&bruno, "Bruno"), (&carla, "Carla")] {
        office(o, "set_server", json!({ "url": url })).await;
        office(o, "sign_up", json!({ "name": name, "email": format!("{}@example.ch", name.to_lowercase()), "password": "a-good-password", "language": "fr" })).await;
    }
    let event = office(&anne, "rosson_events", json!({ "path": file })).await[0]["id"].as_str().unwrap().to_string();
    let import = |o: &std::sync::Arc<office::Office>, key: Option<&str>| {
        let (o, args) = (o.clone(), json!({ "path": file, "event": event, "access_key": key }));
        async move { office(&o, "import_online", args).await }
    };
    let shows = |o: std::sync::Arc<office::Office>| async move { office(&o, "library", json!({})).await.as_array().unwrap().len() };

    // This export has no key: one to type; nothing imported meanwhile.
    let asked = import(&anne, None).await;
    assert_eq!((asked["outcome"].as_str(), asked["number"].as_str()), (Some("need_key"), Some("2026.8352")), "{asked}");
    assert_eq!(shows(anne.clone()).await, 0);
    // With it: Anne's, imported and on equievents.ch at once.
    let made = import(&anne, Some("KEY-8352")).await;
    assert_eq!(made["outcome"], "made", "{made}");
    let (a_show, remote) = (made["show"].as_str().unwrap().to_string(), made["remote"].as_str().unwrap().to_string());
    assert_eq!((made["sync"]["status"].as_str(), made["sync"]["role"].as_str()), (Some("synced"), Some("owner")));
    assert!(made["sync"]["pushed"].as_u64().unwrap() > 100, "{made}");
    // The same file again (its key kept): the show she has, no copy.
    let again = import(&anne, None).await;
    assert_eq!((again["outcome"].as_str(), again["show"].as_str()), (Some("member"), Some(a_show.as_str())), "{again}");
    assert_eq!(shows(anne.clone()).await, 1);

    // Bruno, with the same export and key: Anne's. Nothing imported; he asks her (the key
    // given back for it).
    let owned = import(&bruno, Some("KEY-8352")).await;
    assert_eq!((owned["outcome"].as_str(), owned["owner"].as_str(), owned["remote"].as_str(), owned["access_key"].as_str()), (Some("owned"), Some("Anne"), Some(remote.as_str()), Some("KEY-8352")), "{owned}");
    assert_eq!(shows(bruno.clone()).await, 0);
    let asked = office(&bruno, "ask_owner", json!({ "remote": remote, "kind": "access", "access_key": owned["access_key"] })).await;
    assert_eq!(asked["request"]["status"], "pending", "{asked}");
    // Asked again before she answers: still nothing here, the request waiting.
    let waiting = import(&bruno, None).await;
    assert_eq!((waiting["outcome"].as_str(), waiting["request"]["status"].as_str()), (Some("owned"), Some("pending")), "{waiting}");
    assert_eq!(shows(bruno.clone()).await, 0);
    // She lets him in: the file now brings him equievents.ch's show, the same as hers.
    let requests = office(&anne, "show_requests", json!({ "show": a_show })).await;
    let request = requests["requests"][0]["id"].as_i64().unwrap();
    assert_eq!(office(&anne, "answer_request", json!({ "show": a_show, "id": request, "accept": true })).await["ok"], true);
    let member = import(&bruno, None).await;
    assert_eq!((member["outcome"].as_str(), member["role"].as_str()), (Some("member"), Some("collaborator")), "{member}");
    let b_show = member["show"].as_str().unwrap().to_string();
    assert_eq!(view(&bruno, &b_show).await, view(&anne, &a_show).await);
    assert_eq!(import(&bruno, None).await["show"], json!(b_show), "once");
    assert_eq!(shows(bruno.clone()).await, 1);

    // An EquiEvents show file of the event: the same rule (its key is not in the file).
    let saved = dir.path().join("2026.8352.equievents");
    office(&anne, "export_show", json!({ "show": a_show, "path": saved, "private": true })).await;
    let from_file = |key: Option<&str>| {
        let (o, args) = (carla.clone(), json!({ "path": saved, "access_key": key }));
        async move { office(&o, "import_online", args).await }
    };
    assert_eq!(from_file(None).await["outcome"], "need_key");
    let owned = from_file(Some("KEY-8352")).await;
    assert_eq!((owned["outcome"].as_str(), owned["owner"].as_str()), (Some("owned"), Some("Anne")), "{owned}");
    assert_eq!(shows(carla.clone()).await, 0);

    // Without equievents.ch: no import.
    stop.send(()).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(import(&carla, Some("KEY-8352")).await["outcome"], "offline");
    assert_eq!(shows(carla.clone()).await, 0);
    test_db.drop().await;
}

/// The office's question of 2026-11-16: Anne and Bruno, each on a laptop, offline (the server
/// is down), both rename class 1, move a rider and give start numbers; Anne adds a late entry.
/// When the server is back and both have synced: the same show on both, both moves kept, the
/// later numbers and name kept, the late entry at the end with the next number, and both told
/// what replaced what. Then an undo after the sync takes back only one's own change.
#[tokio::test(flavor = "multi_thread")]
async fn two_laptops_offline_on_the_same_class() {
    let Some(test_db) = TestDb::new("offline").await else { return };
    let s = server(&test_db).await;
    let (url, addr, stop) = serve(s.router.clone(), None).await;
    let dir = tempfile::tempdir().unwrap();
    let file = rosson_file(dir.path());
    let laptop = |name: &str| std::sync::Arc::new(office::Office::new(&dir.path().join(name), name).unwrap());
    registry(&dir.path().join("anne"), (999001, "Léa", "Nouvelle"), (888001, "QUICK STAR"));
    registry(&dir.path().join("bruno"), (999002, "Noé", "Tardif"), (888002, "LATE SHOW"));
    let (anne, bruno) = (laptop("anne"), laptop("bruno"));
    for (o, name) in [(&anne, "Anne"), (&bruno, "Bruno")] {
        office(o, "set_server", json!({ "url": url })).await;
        office(o, "sign_up", json!({ "name": name, "email": format!("{}@example.ch", name.to_lowercase()), "password": "a-good-password", "language": "fr" })).await;
    }
    let event = office(&anne, "rosson_events", json!({ "path": file })).await[0]["id"].as_str().unwrap().to_string();
    let a_show = office(&anne, "import_rosson", json!({ "path": file, "event": event, "mode": "delivered" })).await["show"].as_str().unwrap().to_string();
    let published = office(&anne, "published", json!({ "show": a_show })).await.as_array().unwrap().iter().find(|p| p["number"] == "1").unwrap()["id"].as_u64().unwrap();
    let class = office(&anne, "set_up_class", json!({ "show": a_show, "published": published })).await["class"].as_str().unwrap().to_string();
    let put = office(&anne, "put_online", json!({ "show": a_show })).await;
    let remote = put["remote"].as_str().unwrap_or_else(|| panic!("{put}")).to_string();
    // A rehearsal (no event key): Anne adds Bruno; he downloads it.
    let added = office(&anne, "add_person", json!({ "show": a_show, "email": "bruno@example.ch" })).await;
    assert_eq!(added["ok"], true, "{added}");
    let b_show = office(&bruno, "download_show", json!({ "remote": remote })).await["show"].as_str().unwrap().to_string();

    let starters = |o: std::sync::Arc<office::Office>, show: String| {
        let class = class.clone();
        async move {
            let list = office(&o, "start_list", json!({ "show": show, "class": class })).await;
            list["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| (l["entry"].as_u64().unwrap(), l["start_number"].as_u64())).collect::<Vec<_>>()
        }
    };
    let drawn: Vec<u64> = starters(anne.clone(), a_show.clone()).await.iter().map(|(e, _)| *e).collect();
    assert_eq!(drawn, starters(bruno.clone(), b_show.clone()).await.iter().map(|(e, _)| *e).collect::<Vec<_>>());
    let step = || tokio::time::sleep(std::time::Duration::from_millis(3));

    // Offline.
    stop.send(()).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    office(&anne, "edit_class", json!({ "show": a_show, "class": class, "name": "Nom d'Anne" })).await;
    step().await;
    office(&anne, "move_start", json!({ "show": a_show, "class": class, "entry": drawn[0], "to": 3 })).await;
    step().await;
    office(&anne, "number", json!({ "show": a_show, "class": class, "first": 1 })).await;
    step().await;
    let late = office(&anne, "add_entry", json!({ "show": a_show, "published": published, "rider": { "person": 999001 }, "horse": { "horse": 888001 } })).await["entry"].as_u64().unwrap();
    step().await;
    office(&bruno, "edit_class", json!({ "show": b_show, "class": class, "name": "Nom de Bruno" })).await;
    step().await;
    office(&bruno, "move_start", json!({ "show": b_show, "class": class, "entry": drawn[42], "to": 1 })).await;
    step().await;
    office(&bruno, "number", json!({ "show": b_show, "class": class, "first": 101 })).await;
    assert_eq!(office(&anne, "sync", json!({ "show": a_show })).await["note"]["status"], "offline");

    // Back online: both sync (Anne once more, to get what Bruno sent after her).
    let (_, _, stop) = serve(s.router.clone(), Some(addr)).await;
    office(&anne, "sync", json!({ "show": a_show })).await;
    office(&bruno, "sync", json!({ "show": b_show })).await;
    office(&anne, "sync", json!({ "show": a_show })).await;
    assert_eq!(view(&anne, &a_show).await, view(&bruno, &b_show).await, "the same show on both laptops");
    let list = starters(anne.clone(), a_show.clone()).await;
    let places: Vec<(usize, Option<u64>)> = list.iter().map(|(e, n)| (drawn.iter().position(|d| d == e).map_or(0, |i| i + 1), *n)).collect();
    // Bruno's last rider first, Anne's first rider after her second and third; Bruno's numbers
    // (he numbered last); the late entry at the end with the next one.
    assert_eq!(&places[..5], &[(43, Some(101)), (2, Some(103)), (3, Some(104)), (1, Some(102)), (4, Some(105))]);
    assert_eq!(list.last(), Some(&(late, Some(144))));
    let row = |v: Value| v["days"].as_array().unwrap().iter().flat_map(|d| d["classes"].as_array().unwrap().clone()).find(|c| c["id"] == class.as_str()).unwrap();
    assert_eq!(row(view(&anne, &a_show).await)["name"], "Nom de Bruno");
    // Both told, by name: Bruno's numbers and name kept, Anne's replaced.
    for (o, show, here) in [(&anne, &a_show, "anne"), (&bruno, &b_show, "bruno")] {
        let c = office(o, "conflicts", json!({ "show": show })).await;
        assert_eq!(c["unseen"], 2, "{c:#}");
        let kinds: Vec<(&str, &str, &str)> = c["conflicts"].as_array().unwrap().iter().map(|c| (c["kind"].as_str().unwrap(), c["kept"]["author"].as_str().unwrap(), c["replaced"]["author"].as_str().unwrap())).collect();
        assert_eq!(kinds, vec![("numbers", "Bruno", "Anne"), ("class", "Bruno", "Anne")]);
        assert_eq!(c["conflicts"][0]["kept"]["here"], here == "bruno");
    }

    // Anne moves a rider while Bruno adds a late entry, both offline again; after the sync Anne
    // undoes her move: her rider is back, Bruno's late entry stays.
    let before = starters(anne.clone(), a_show.clone()).await;
    stop.send(()).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    office(&anne, "move_start", json!({ "show": a_show, "class": class, "entry": before[0].0, "to": 6 })).await;
    step().await;
    let b_late = office(&bruno, "add_entry", json!({ "show": b_show, "published": published, "rider": { "person": 999002 }, "horse": { "horse": 888002 } })).await["entry"].as_u64().unwrap();
    let (_, _, stop) = serve(s.router.clone(), Some(addr)).await;
    office(&anne, "sync", json!({ "show": a_show })).await;
    office(&bruno, "sync", json!({ "show": b_show })).await;
    office(&anne, "sync", json!({ "show": a_show })).await;
    assert_eq!(office(&anne, "undo", json!({ "show": a_show })).await["undone"], "move_start");
    let list = starters(anne.clone(), a_show.clone()).await;
    assert_eq!(list[..before.len()], before[..]);
    assert_eq!(list.last(), Some(&(b_late, Some(145))));
    office(&anne, "sync", json!({ "show": a_show })).await;
    office(&bruno, "sync", json!({ "show": b_show })).await;
    assert_eq!(starters(bruno.clone(), b_show.clone()).await, list);

    stop.send(()).ok();
    drop(s);
    test_db.drop().await;
}

/// A stand-in for Swiss Equestrian, answering with the practice event (every person invented)
/// to the key `PRACTICE-KEY` (and `NEW-KEY`, another key it accepts for the same event, which
/// equievents.ch must refuse: an event never changes its key; and the demo keys),
/// its riders and horses as the registry, 404 to other keys, a 500 to `BROKEN`; it answers 202
/// (preparing) to the first `preparing` loads of the event. Every request it got, for the test
/// to look at; and a switch: the event is over (its keys refused, as Swiss Equestrian does).
#[allow(clippy::type_complexity)]
fn stand_in_se(preparing: usize) -> (server::se::Se, std::sync::Arc<std::sync::Mutex<Vec<String>>>, std::sync::Arc<std::sync::atomic::AtomicBool>) {
    use se_api::http::{HttpRequest, HttpResponse};
    const BASE: &str = "https://se.invalid/api/veranstaltersoftware/v2";
    let practice: Value = serde_json::from_str(include_str!("../../office/training/practice.json")).unwrap();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let over = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (log, polls, ended) = (seen.clone(), std::sync::Mutex::new(0usize), over.clone());
    let transport = std::sync::Arc::new(move |request: &HttpRequest| {
        log.lock().unwrap().push(request.url.clone());
        let key = se_api::http::download_key(request, BASE, &server::se::ENDPOINTS).expect("only downloads").to_string();
        let endpoint = request.url[BASE.len() + 1..].split('?').next().unwrap().to_string();
        let answer = |status: u16, body: Value| Ok(HttpResponse { status, status_text: String::new(), body: body.to_string().into_bytes() });
        match key.as_str() {
            "PRACTICE-KEY" | "NEW-KEY" if ended.load(std::sync::atomic::Ordering::Relaxed) => return answer(404, json!({ "message": format!("access_key '{key}' not found") })),
            "PRACTICE-KEY" | "NEW-KEY" | "demoveranstaltung" | "demoveranstaltung_fa" => {}
            "BROKEN" => return answer(500, json!({ "message": "Internal Server Error" })),
            other => return answer(404, json!({ "message": format!("access_key '{other}' not found") })),
        }
        if endpoint == "getVeranstaltung" {
            let mut n = polls.lock().unwrap();
            if *n < preparing {
                *n += 1;
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
    let limits = se_api::http::Limits { poll_every: std::time::Duration::from_millis(1), max_polls: 10, retries: 0, backoff: std::time::Duration::from_millis(1) };
    (server::se::Se::with(Some(BASE), transport, limits), seen, over)
}

impl Server {
    /// What a job answered, once done (asked again while it works); an answer given at once
    /// (a key known here already) as it is.
    async fn job(&self, token: &str, answer: (StatusCode, Value)) -> (StatusCode, Value) {
        if answer.0 != StatusCode::ACCEPTED {
            return answer;
        }
        let job = answer.1["job"].as_str().unwrap().to_string();
        for _ in 0..500 {
            let (status, body) = self.call("GET", &format!("/api/jobs/{job}"), Some(token), None).await;
            if status != StatusCode::ACCEPTED {
                return (status, body);
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("job {job} still working");
    }
}

/// With Swiss Equestrian: the key alone downloads the event, and the first to enter it owns the
/// show made from it; the key is checked before anyone owns a show (another event's key, an
/// unknown key, a new key of the event); Swiss Equestrian's demo events are no one's; while it
/// prepares the event, the app asks again.
#[tokio::test]
async fn events_come_from_swiss_equestrian() {
    let Some(test_db) = TestDb::new("se").await else { return };
    let pool = db::pool(&test_db.url).unwrap();
    db::migrate(&pool).await.unwrap();
    let (se, seen, _) = stand_in_se(2);
    let app = api::App { mail_in_background: false, se, ..api::App::new(pool.clone(), mail::Mailer::off(), "https://www.equievents.ch/") };
    let s = Server { router: api::router(app), pool, mailer: mail::Mailer::off() };
    for (name, email) in [("Anne Office", "anne@example.ch"), ("Bruno Second", "bruno@example.ch")] {
        auth::create_user(&s.pool, name, email, "a-good-password", false, Lang::Fr).await.unwrap().unwrap();
    }
    let anne = s.sign_in("anne@example.ch", "a-good-password").await;
    let bruno = s.sign_in("bruno@example.ch", "a-good-password").await;

    // Anne enters the key: Swiss Equestrian prepares the event (twice 202), then the show is made
    // from its answers, and is hers.
    let claim = |key: &str| json!({ "access_key": key });
    let (status, made) = s.job(&anne, s.call("POST", "/api/shows", Some(&anne), Some(claim("PRACTICE-KEY"))).await).await;
    assert_eq!((status, made["created"].clone(), made["role"].clone(), made["event_id"].clone()), (StatusCode::OK, json!(true), json!("owner"), json!(1)), "{made}");
    let show = made["id"].as_str().unwrap().to_string();
    assert!(show.starts_with("EXERCICE-"), "{show}");
    let asked: Vec<String> = seen.lock().unwrap().iter().map(|u| u.split('?').next().unwrap().rsplit('/').next().unwrap().to_string()).collect();
    assert_eq!(asked, ["getVeranstaltung", "getVeranstaltung", "getVeranstaltung", "getOffizielle", "getStammdaten"]);
    assert!(seen.lock().unwrap().iter().all(|u| u.contains("access_key=PRACTICE-KEY") && u.contains("app_version=3.3.15")));
    // The show, as Swiss Equestrian delivered it: what the apps pull.
    let (_, pulled) = s.call("GET", &format!("/api/shows/{show}/ops"), Some(&anne), None).await;
    let ops: Vec<show::oplog::Operation> = serde_json::from_value(pulled["ops"].clone()).unwrap();
    assert!(ops.iter().all(|o| o.author == "Anne Office" && o.id.starts_with("equievents.ch:")));
    let (state, ignored) = show::State::from_log(ops);
    assert!(ignored.is_empty(), "{ignored:?}");
    let event = state.event.as_ref().unwrap();
    assert_eq!((event.id.0, event.number.as_str()), (1, "EXERCICE"));
    assert!(state.entries.len() > 100, "{}", state.entries.len());
    let (_, listed) = s.call("GET", "/api/shows", Some(&anne), None).await;
    assert_eq!((listed[0]["name"].as_str(), listed[0]["last_day"].as_str()), (event.title.first().map(String::as_str), Some(event.to.get(..10).unwrap())));

    // Again: hers already, known at once by its key, nothing asked of Swiss Equestrian.
    seen.lock().unwrap().clear();
    let (status, again) = s.call("POST", "/api/shows", Some(&anne), Some(claim("PRACTICE-KEY"))).await;
    assert_eq!((status, again["id"].as_str(), again["created"].clone()), (StatusCode::OK, Some(show.as_str()), json!(false)));
    assert!(seen.lock().unwrap().is_empty());

    // Bruno, with the key: Anne's; he asks to join. An event never changes its key: another key
    // Swiss Equestrian were to accept for it opens nothing here.
    let (status, owned) = s.call("POST", "/api/shows", Some(&bruno), Some(claim("PRACTICE-KEY"))).await;
    assert_eq!((status, owned["code"].as_str(), owned["owner"].as_str(), owned["id"].as_str()), (StatusCode::CONFLICT, Some("claimed"), Some("Anne Office"), Some(show.as_str())));
    assert!(seen.lock().unwrap().is_empty(), "Bruno's key is the show's: known at once");
    let (status, other) = s.job(&bruno, s.call("POST", "/api/shows", Some(&bruno), Some(claim("NEW-KEY"))).await).await;
    assert_eq!((status, other["code"].as_str()), (StatusCode::FORBIDDEN, Some("key")));
    let ask = |key: &str| json!({ "kind": "access", "access_key": key });
    assert_eq!(s.call("POST", &format!("/api/shows/{show}/requests"), Some(&bruno), Some(ask("NEW-KEY"))).await.0, StatusCode::FORBIDDEN);
    assert_eq!(s.call("POST", &format!("/api/shows/{show}/requests"), Some(&bruno), Some(ask("PRACTICE-KEY"))).await.0, StatusCode::OK);

    // Refused: a key Swiss Equestrian does not know, another event's key for a show here, a demo
    // key; Swiss Equestrian failing says so.
    let (status, wrong) = s.job(&bruno, s.call("POST", "/api/shows", Some(&bruno), Some(claim("GUESSED"))).await).await;
    assert_eq!((status, wrong["code"].as_str()), (StatusCode::FORBIDDEN, Some("key")), "{wrong}");
    let (status, other) = s.job(&bruno, s.call("POST", "/api/shows", Some(&bruno), Some(json!({ "access_key": "NEW-KEY", "event_id": 8352 }))).await).await;
    assert_eq!((status, other["error"].as_str()), (StatusCode::FORBIDDEN, Some("this key opens another event")));
    let (status, demo) = s.call("POST", "/api/shows", Some(&bruno), Some(claim("demoveranstaltung"))).await;
    assert_eq!((status, demo["code"].as_str()), (StatusCode::BAD_REQUEST, Some("training")));
    let (status, broken) = s.job(&bruno, s.call("POST", "/api/shows", Some(&bruno), Some(claim("BROKEN"))).await).await;
    assert_eq!((status, broken["code"].as_str()), (StatusCode::BAD_GATEWAY, Some("swiss_equestrian")), "{broken}");

    // A job is its owner's only (a key not known here yet: asked of Swiss Equestrian).
    let (_, started) = s.call("POST", "/api/shows", Some(&anne), Some(claim("NEW-KEY"))).await;
    assert_eq!(s.call("GET", &format!("/api/jobs/{}", started["job"].as_str().unwrap()), Some(&bruno), None).await.0, StatusCode::NOT_FOUND);

    drop(s);
    test_db.drop().await;
}

/// Without Swiss Equestrian (development), the key alone cannot download an event.
#[tokio::test]
async fn without_swiss_equestrian_the_app_names_the_event() {
    let Some(test_db) = TestDb::new("nose").await else { return };
    let s = server(&test_db).await;
    auth::create_user(&s.pool, "Anne Office", "anne@example.ch", "a-good-password", false, Lang::Fr).await.unwrap().unwrap();
    let anne = s.sign_in("anne@example.ch", "a-good-password").await;
    let (status, answer) = s.call("POST", "/api/shows", Some(&anne), Some(json!({ "access_key": "KEY" }))).await;
    assert_eq!((status, answer["code"].as_str()), (StatusCode::BAD_REQUEST, Some("no_swiss_equestrian")));
    drop(s);
    test_db.drop().await;
}

/// From the office: the key alone brings the event, through equievents.ch, as a show here (and
/// hers on the server); Bruno, with the key, asks her, is let in, and gets the same show. A
/// show imported here goes up only with its own event's key; demo keys stay in the settings.
#[tokio::test(flavor = "multi_thread")]
async fn an_office_opens_an_event_with_its_key() {
    let Some(test_db) = TestDb::new("openkey").await else { return };
    let pool = db::pool(&test_db.url).unwrap();
    db::migrate(&pool).await.unwrap();
    let (se, _, _) = stand_in_se(1);
    let app = api::App { se, ..api::App::new(pool.clone(), mail::Mailer::off(), "https://www.equievents.ch/") };
    let (url, _, stop) = serve(api::router(app), None).await;
    let dir = tempfile::tempdir().unwrap();
    let laptop = |name: &str| std::sync::Arc::new(office::Office::new(&dir.path().join(name), name).unwrap());
    let (anne, bruno) = (laptop("anne"), laptop("bruno"));
    for (o, name) in [(&anne, "Anne"), (&bruno, "Bruno")] {
        office(o, "set_server", json!({ "url": url })).await;
        assert_eq!(office(o, "open_with_key", json!({ "access_key": "PRACTICE-KEY" })).await["outcome"], "signed_out");
        office(o, "sign_up", json!({ "name": name, "email": format!("{}@example.ch", name.to_lowercase()), "password": "a-good-password", "language": "fr" })).await;
    }

    // Anne: the event comes, its show is hers, here and on the server; the key is kept.
    let opened = office(&anne, "open_with_key", json!({ "access_key": " PRACTICE-KEY " })).await;
    assert_eq!(opened["outcome"], "made", "{opened}");
    let a_show = opened["show"].as_str().unwrap().to_string();
    let a_view = view(&anne, &a_show).await;
    assert_eq!(a_view["event"]["number"], "EXERCICE");
    assert_eq!(office(&anne, "connection", json!({ "show": a_show })).await["has_key"], true);
    let status = office(&anne, "sync_status", json!({ "show": a_show })).await;
    assert_eq!((status["remote"].clone(), status["note"]["role"].as_str()), (opened["remote"].clone(), Some("owner")));
    // Again: the same show here.
    assert_eq!(office(&anne, "open_with_key", json!({ "access_key": "PRACTICE-KEY" })).await["show"], json!(a_show));

    // Bruno: Anne's; he asks for access with the key, she lets him in, he gets the same show.
    let owned = office(&bruno, "open_with_key", json!({ "access_key": "PRACTICE-KEY" })).await;
    assert_eq!((owned["outcome"].as_str(), owned["owner"].as_str()), (Some("owned"), Some("Anne")), "{owned}");
    let asked = office(&bruno, "ask_owner", json!({ "remote": owned["remote"], "kind": "access", "access_key": "PRACTICE-KEY" })).await;
    assert_eq!(asked["request"]["status"], "pending", "{asked}");
    let requests = office(&anne, "show_requests", json!({ "show": a_show })).await;
    office(&anne, "answer_request", json!({ "show": a_show, "id": requests["requests"][0]["id"], "accept": true })).await;
    let member = office(&bruno, "open_with_key", json!({ "access_key": "PRACTICE-KEY" })).await;
    assert_eq!(member["outcome"], "member", "{member}");
    assert_eq!(view(&bruno, member["show"].as_str().unwrap()).await, a_view);

    // Refused: an unknown key; a demo key (the settings' Exercice); a show imported here with
    // a key Swiss Equestrian gives for another event.
    assert_eq!(office(&bruno, "open_with_key", json!({ "access_key": "GUESSED" })).await["outcome"], "key");
    assert_eq!(office(&bruno, "open_with_key", json!({ "access_key": "demoveranstaltung" })).await["outcome"], "training");
    let file = rosson_file(dir.path());
    let event = office(&bruno, "rosson_events", json!({ "path": file })).await[0]["id"].as_str().unwrap().to_string();
    let imported = office(&bruno, "import_rosson", json!({ "path": file, "event": event, "mode": "ended" })).await["show"].as_str().unwrap().to_string();
    assert_eq!(office(&bruno, "put_online", json!({ "show": imported, "access_key": "PRACTICE-KEY" })).await["outcome"], "key");
    // A past event from its Rosson export (the user, 2026-11-21): Swiss Equestrian no longer
    // knows its key, taken on trust: Bruno's, the first to put it up, and the whole import with it.
    let past = office(&bruno, "put_online", json!({ "show": imported, "access_key": "KEY-8352" })).await;
    assert_eq!((past["outcome"].as_str(), past["sync"]["role"].as_str()), (Some("made"), Some("owner")), "{past}");
    assert!(past["sync"]["pushed"].as_u64().unwrap() > 100, "{past}");

    stop.send(()).ok();
    test_db.drop().await;
}

/// A show of equievents.ch sends from there (the server's simulation until the pilot): the
/// office's changes go up first, what was sent comes down as the show's own record, and the
/// simulation's log is the server's; the event is updated from Swiss Equestrian there too.
#[tokio::test(flavor = "multi_thread")]
async fn a_show_of_the_server_sends_and_updates_from_there() {
    let Some(test_db) = TestDb::new("sends").await else { return };
    let pool = db::pool(&test_db.url).unwrap();
    db::migrate(&pool).await.unwrap();
    let (se, seen, over) = stand_in_se(0);
    let app = api::App { se, ..api::App::new(pool.clone(), mail::Mailer::off(), "https://www.equievents.ch/") };
    let (url, _, stop) = serve(api::router(app), None).await;
    let dir = tempfile::tempdir().unwrap();
    let anne = std::sync::Arc::new(office::Office::new(&dir.path().join("anne"), "anne").unwrap());
    office(&anne, "set_server", json!({ "url": url })).await;
    office(&anne, "sign_up", json!({ "name": "Anne", "email": "anne@example.ch", "password": "a-good-password", "language": "fr" })).await;
    let show = office(&anne, "open_with_key", json!({ "access_key": "PRACTICE-KEY" })).await["show"].as_str().unwrap().to_string();
    let connection = office(&anne, "connection", json!({ "show": show })).await;
    assert_eq!((connection["online"].as_bool(), connection["server"].as_str()), (Some(true), Some("simulation")), "{connection}");

    // The event sent: by the server, to its simulation; recorded in the show, here too.
    let sent = office(&anne, "send", json!({ "show": show, "what": "event" })).await;
    assert_eq!((sent["sent"].as_str(), sent["server"].as_str()), (Some("event"), Some("simulation")), "{sent}");
    let sending = office(&anne, "sending", json!({ "show": show })).await;
    assert_eq!(sending["event_sent"]["author"], "Anne");
    let log = office(&anne, "simulation_log", json!({ "show": show })).await;
    let put = log.as_array().unwrap().iter().find(|r| r["path"] == "putVeranstaltung").expect("the server's simulation got it");
    assert_eq!((put["status"].as_u64(), put["body"]["access_key"].as_str()), (Some(200), Some("•••")));

    // Updated from Swiss Equestrian (here: the same delivery again), then synced.
    seen.lock().unwrap().clear();
    let updated = office(&anne, "update_from_se", json!({ "show": show })).await;
    assert_eq!((updated["ok"].clone(), updated["sync"]["status"].as_str()), (json!(true), Some("synced")), "{updated}");
    assert!(updated["added"].is_number());
    let asked: Vec<String> = seen.lock().unwrap().iter().map(|u| u.split('?').next().unwrap().rsplit('/').next().unwrap().to_string()).collect();
    assert_eq!(asked, ["getOffizielle", "getStammdaten", "getVeranstaltung"]);

    // The riders and horses: downloaded by the server with the show's key, kept for Anne's
    // account; her other computer gets them with its sync; signing out takes them off.
    let about = office(&anne, "registry_download", json!({ "show": show })).await;
    assert!(about["persons"].as_u64().unwrap() > 100 && about["horses"].as_u64().unwrap() > 100, "{about}");
    assert_eq!(about["simulated"], false);
    let other = std::sync::Arc::new(office::Office::new(&dir.path().join("anne-2"), "anne-2").unwrap());
    office(&other, "set_server", json!({ "url": url })).await;
    office(&other, "sign_in", json!({ "email": "anne@example.ch", "password": "a-good-password" })).await;
    assert_eq!(office(&other, "registry", json!({})).await["persons"], about["persons"]);
    office(&other, "sign_out", json!({})).await;
    assert_eq!(office(&other, "registry", json!({})).await["persons"], 0);
    // Swiss Equestrian's demo event, through the server: a training show here, never online.
    let demo = office(&anne, "load_demo", json!({ "driving": false })).await["show"].as_str().unwrap().to_string();
    assert_eq!(office(&anne, "sync_status", json!({ "show": demo })).await["training"], true);
    let asked: Vec<String> = seen.lock().unwrap().iter().filter(|u| u.contains("demoveranstaltung")).map(|u| u.split('?').next().unwrap().rsplit('/').next().unwrap().to_string()).collect();
    assert_eq!(asked, ["getOffizielle", "getStammdaten", "getVeranstaltung"]);

    // The event over: Swiss Equestrian refuses its key, and the office is told so.
    over.store(true, std::sync::atomic::Ordering::Relaxed);
    let (o, show2) = (anne.clone(), show.clone());
    let refused = tokio::task::spawn_blocking(move || o.call("registry_download", &json!({ "show": show2 }))).await.unwrap();
    assert_eq!(refused.unwrap_err(), "Swiss Equestrian does not know the show's key: it only accepts the key of an event that is not over yet");
    assert_eq!(office(&anne, "update_from_se", json!({ "show": show })).await["code"], "key");
    over.store(false, std::sync::atomic::Ordering::Relaxed);

    // Offline: nothing is sent, and the office is told why.
    stop.send(()).ok();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let (o, show2) = (anne.clone(), show.clone());
    let refused = tokio::task::spawn_blocking(move || o.call("send", &json!({ "show": show2, "what": "event" }))).await.unwrap();
    assert_eq!(refused.unwrap_err(), "equievents.ch does not answer: sending to Swiss Equestrian waits for the connection");
    test_db.drop().await;
}

/// The server's own way to Swiss Equestrian, for real: its demo event (the one download allowed
/// before anything else, the user, 2026-11-15), as `SE_SERVER=production` reaches it. Run by
/// hand (`--ignored`); nothing of the answer is printed or kept.
#[tokio::test]
#[ignore = "reaches Swiss Equestrian"]
async fn the_server_downloads_from_swiss_equestrian() {
    let se = server::se::Se::from_env(|name| (name == "SE_SERVER").then(|| "production".to_string())).unwrap();
    let keys = se.keys("demoveranstaltung").await;
    let event = se.get(&se_api::load::get_veranstaltung(&keys, None)).await.unwrap();
    let delivered = &event["data"]["veranstaltung"];
    assert!(se_api::load::number(&delivered["id"]).is_some());
    assert_eq!(delivered["nr"], "2024.0001");
    // Anything but a download is refused before it leaves.
    let mut upload = se_api::actions::event(&show::State::default(), &keys);
    let refused = se.get(&upload.remove(0).request).await.unwrap_err();
    assert!(refused.to_string().contains("only an event's download goes to Swiss Equestrian"), "{refused}");
}

/// This version never sends to Swiss Equestrian: a setting asking for it stops the server, and
/// « Envoyer » goes to the simulation whatever Swiss Equestrian is set.
#[test]
fn the_server_never_sends_to_swiss_equestrian() {
    let env = |send: &'static str| move |name: &str| match name {
        "SE_SEND" => Some(send.to_string()),
        _ => None,
    };
    for send in ["live", "production", "stage"] {
        assert!(server::se::Se::from_env(env(send)).err().unwrap().contains("never sends to Swiss Equestrian"), "{send}");
    }
    let se = server::se::Se::from_env(env("simulation")).unwrap();
    assert_eq!((se.name(), se.sends_to()), ("production", "simulation"));
}

/// One request with its own headers (a browser's cookie and origin): status, headers, body.
async fn browser(router: &axum::Router, method: &str, path: &str, headers: &[(&str, &str)], body: Option<String>) -> (StatusCode, axum::http::HeaderMap, String) {
    let mut req = Request::builder().method(method).uri(path).header("x-real-ip", "10.0.0.9").header("host", "www.equievents.ch");
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let req = req.body(body.map(Body::from).unwrap_or_else(Body::empty)).unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&bytes).into_owned())
}

/// The online version: signed in on the site (a cookie), the interface at /app/, its commands
/// run by the account's office straight on the master (a change there at once, no sync asked),
/// its files the account's own, nothing it must not do; the download page only signed in.
#[tokio::test(flavor = "multi_thread")]
async fn the_online_version_works_on_the_master() {
    let Some(test_db) = TestDb::new("online").await else { return };
    let pool = db::pool(&test_db.url).unwrap();
    db::migrate(&pool).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let interface = dir.path().join("web");
    std::fs::create_dir_all(interface.join("assets")).unwrap();
    std::fs::write(interface.join("index.html"), "<!doctype html><title>EquiEvents</title>").unwrap();
    std::fs::write(interface.join("assets/app.js"), "console.log(1)").unwrap();
    // The server on a port, its offices reaching it there.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (se, _, _) = stand_in_se(0);
    let web = server::web::Web::new(dir.path().join("offices"), &url, Some(interface));
    let app = api::App { se, web: Some(std::sync::Arc::new(web)), ..api::App::new(pool.clone(), mail::Mailer::off(), "https://www.equievents.ch") };
    let router = api::router(app);
    let served = router.clone();
    tokio::spawn(async move { axum::serve(listener, served.into_make_service_with_connect_info::<std::net::SocketAddr>()).await.unwrap() });
    for (name, email) in [("Anne Office", "anne@example.ch"), ("Bruno Second", "bruno@example.ch")] {
        auth::create_user(&pool, name, email, "a-good-password", false, Lang::Fr).await.unwrap().unwrap();
    }

    // Signed out: the interface and the downloads send to the sign-in (the browser's language,
    // else German), and come back after.
    let (status, headers, _) = browser(&router, "GET", "/app/", &[], None).await;
    assert_eq!((status, headers["location"].to_str().unwrap()), (StatusCode::SEE_OTHER, "/anmelden?next=/app/"));
    let (_, headers, _) = browser(&router, "GET", "/app/", &[("accept-language", "fr-CH,fr")], None).await;
    assert_eq!(headers["location"].to_str().unwrap(), "/connexion?next=/app/");
    let (status, headers, _) = browser(&router, "GET", "/herunterladen", &[], None).await;
    assert_eq!((status, headers["location"].to_str().unwrap()), (StatusCode::SEE_OTHER, "/anmelden?next=/herunterladen"));
    assert_eq!(browser(&router, "POST", "/app/api/library", &[], Some("{}".into())).await.0, StatusCode::UNAUTHORIZED);

    // Signing in on the site: a wrong password, then the right one (the cookie, back to /app/).
    let form = |password: &str| Some(format!("email=anne%40example.ch&password={password}&next=%2Fapp%2F"));
    let urlencoded = [("content-type", "application/x-www-form-urlencoded")];
    assert_eq!(browser(&router, "POST", "/connexion", &urlencoded, form("wrong")).await.0, StatusCode::UNAUTHORIZED);
    let (status, headers, _) = browser(&router, "POST", "/connexion", &urlencoded, form("a-good-password")).await;
    assert_eq!((status, headers["location"].to_str().unwrap()), (StatusCode::SEE_OTHER, "/app/"));
    let set = headers["set-cookie"].to_str().unwrap().to_string();
    assert!(set.contains("HttpOnly") && set.contains("SameSite=Lax") && set.contains("Secure"), "{set}");
    let cookie = set.split(';').next().unwrap().to_string();
    let anne = [("cookie", cookie.as_str()), ("sec-fetch-site", "same-origin"), ("content-type", "application/json")];
    let (status, _, page) = browser(&router, "GET", "/app/", &anne, None).await;
    assert_eq!((status, page.contains("<title>EquiEvents</title>")), (StatusCode::OK, true));
    assert_eq!(browser(&router, "GET", "/app/assets/app.js", &[], None).await.0, StatusCode::OK);
    assert_eq!(browser(&router, "GET", "/app/../Cargo.toml", &anne, None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(browser(&router, "GET", "/telecharger", &anne, None).await.0, StatusCode::OK);

    // The account's office: signed in as Anne, on this server; what it must not do, refused.
    let api = |command: &str, args: Value| {
        let router = router.clone();
        let command = command.to_string();
        async move {
            let (status, _, body) = browser(&router, "POST", &format!("/app/api/{command}"), &anne, Some(args.to_string())).await;
            (status, serde_json::from_str::<Value>(&body).unwrap_or(Value::Null))
        }
    };
    let (status, account) = api("account", json!({})).await;
    assert_eq!((status, account["hosted"].clone(), account["account"]["name"].as_str()), (StatusCode::OK, json!(true), Some("Anne Office")), "{account}");
    let (status, refused) = api("set_server", json!({ "url": "https://evil.example" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(refused["error"].as_str().unwrap().contains("not in the online version"), "{refused}");
    assert_eq!(api("sign_out", json!({})).await.0, StatusCode::BAD_REQUEST);
    let (status, refused) = api("import_show", json!({ "path": "/etc/passwd" })).await;
    assert_eq!((status, refused["code"].as_str()), (StatusCode::BAD_REQUEST, Some("path")));
    let elsewhere = [("cookie", cookie.as_str()), ("origin", "https://evil.example")];
    assert_eq!(browser(&router, "POST", "/app/api/library", &elsewhere, Some("{}".into())).await.0, StatusCode::FORBIDDEN);

    // An event from Swiss Equestrian, then a change: on the master at once.
    let (_, opened) = api("open_with_key", json!({ "access_key": "PRACTICE-KEY" })).await;
    assert_eq!(opened["outcome"], "made", "{opened}");
    let (show, remote) = (opened["show"].as_str().unwrap().to_string(), opened["remote"].as_str().unwrap().to_string());
    let before = server::shows::log(&pool, &remote).await.unwrap().len();
    let (_, view) = api("show", json!({ "show": show })).await;
    let published = view["not_set_up"][0]["id"].clone();
    let (status, set_up) = api("set_up_class", json!({ "show": show, "published": published })).await;
    assert_eq!(status, StatusCode::OK, "{set_up}");
    let after = server::shows::log(&pool, &remote).await.unwrap();
    assert!(after.len() > before && after.iter().any(|o| o.author == "Anne Office" && o.id.starts_with(&format!("web-{}:", 1))), "{} → {}", before, after.len());

    // Files: the account's own, nobody else's.
    let (status, _, kept) = browser(&router, "POST", "/app/files/note.txt", &anne, Some("hello".into())).await;
    assert_eq!(status, StatusCode::OK, "{kept}");
    let kept: Value = serde_json::from_str(&kept).unwrap();
    let link = kept["url"].as_str().unwrap().to_string();
    assert!(link.starts_with("/app/files/") && kept["path"].as_str().unwrap().starts_with(dir.path().join("offices/1/files").to_str().unwrap()));
    assert_eq!(browser(&router, "GET", &link, &anne, None).await.2, "hello");
    assert_eq!(browser(&router, "GET", &link, &[], None).await.0, StatusCode::UNAUTHORIZED);
    let (_, headers, _) = browser(&router, "POST", "/connexion", &urlencoded, Some("email=bruno%40example.ch&password=a-good-password".into())).await;
    let bruno = headers["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_string();
    assert_eq!(browser(&router, "GET", &link, &[("cookie", bruno.as_str())], None).await.0, StatusCode::NOT_FOUND);

    // Signed out on the site: the cookie gone, the token too.
    let (status, headers, _) = browser(&router, "GET", "/deconnexion", &anne, None).await;
    assert_eq!((status, headers["location"].to_str().unwrap()), (StatusCode::SEE_OTHER, "/"));
    assert!(headers["set-cookie"].to_str().unwrap().contains("Max-Age=0"));
    // The home page after it in the language signed out in.
    assert!(headers.get_all("set-cookie").iter().any(|c| c.to_str().unwrap().starts_with("equievents_lang=fr;")));
    assert_eq!(api("account", json!({})).await.0, StatusCode::UNAUTHORIZED);

    // An account made on the site is signed in at once.
    let (status, headers, _) = browser(&router, "POST", "/inscription", &urlencoded, Some("name=Carla+Third&email=carla%40example.ch&password=a-good-password".into())).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.get_all("set-cookie").iter().any(|c| c.to_str().unwrap().starts_with("equievents_session=")));
    test_db.drop().await;
}

/// Deleting an event (the user, 2026-11-21): a collaborator leaves it; the owner keeps the
/// rights (only the copy goes), gives it to a collaborator (and leaves it), or releases it
/// (deleted from equievents.ch for everyone: the next to enter the key downloads it fresh and
/// owns it). An event never changes its key.
#[tokio::test(flavor = "multi_thread")]
async fn an_owner_deletes_an_event() {
    let Some(test_db) = TestDb::new("deletes").await else { return };
    let pool = db::pool(&test_db.url).unwrap();
    db::migrate(&pool).await.unwrap();
    let (se, _, _) = stand_in_se(0);
    let app = api::App { se, ..api::App::new(pool.clone(), mail::Mailer::off(), "https://www.equievents.ch/") };
    let (url, _, stop) = serve(api::router(app), None).await;
    let dir = tempfile::tempdir().unwrap();
    let mut laptops = Vec::new();
    for name in ["Anne", "Bruno", "Carla"] {
        let o = std::sync::Arc::new(office::Office::new(&dir.path().join(name), name).unwrap());
        office(&o, "set_server", json!({ "url": url })).await;
        office(&o, "sign_up", json!({ "name": name, "email": format!("{}@example.ch", name.to_lowercase()), "password": "a-good-password", "language": "fr" })).await;
        laptops.push(o);
    }
    let (anne, bruno, carla) = (laptops[0].clone(), laptops[1].clone(), laptops[2].clone());
    let open = |o: std::sync::Arc<office::Office>| async move { office(&o, "open_with_key", json!({ "access_key": "PRACTICE-KEY" })).await };

    let a_show = open(anne.clone()).await["show"].as_str().unwrap().to_string();
    office(&anne, "add_person", json!({ "show": a_show, "email": "bruno@example.ch" })).await;
    let b_show = open(bruno.clone()).await["show"].as_str().unwrap().to_string();
    let people = |o: std::sync::Arc<office::Office>, show: String| async move { office(&o, "show_people", json!({ "show": show })).await["people"].as_array().unwrap().iter().map(|p| (p["name"].as_str().unwrap().to_string(), p["role"].as_str().unwrap().to_string())).collect::<Vec<_>>() };
    assert_eq!(people(anne.clone(), a_show.clone()).await, [("Anne".into(), "owner".into()), ("Bruno".into(), "collaborator".into())]);

    // Bruno leaves: his copy goes, he no longer works on it.
    assert_eq!(office(&bruno, "delete_show", json!({ "show": b_show, "choice": "leave" })).await["ok"], true);
    assert!(!office(&bruno, "library", json!({})).await.as_array().unwrap().iter().any(|s| s["id"] == json!(b_show)));
    assert_eq!(people(anne.clone(), a_show.clone()).await, [("Anne".into(), "owner".into())]);

    // Anne keeps her rights: only her copy goes; the key brings it back, hers.
    assert_eq!(office(&anne, "delete_show", json!({ "show": a_show, "choice": "keep" })).await["ok"], true);
    let again = open(anne.clone()).await;
    assert_eq!(again["outcome"], "member", "{again}");
    let a_show = again["show"].as_str().unwrap().to_string();

    // Anne gives it to Bruno and leaves it.
    office(&anne, "add_person", json!({ "show": a_show, "email": "bruno@example.ch" })).await;
    let b_show = open(bruno.clone()).await["show"].as_str().unwrap().to_string();
    let bruno_id = office(&anne, "show_people", json!({ "show": a_show })).await["people"].as_array().unwrap().iter().find(|p| p["name"] == "Bruno").unwrap()["id"].clone();
    assert_eq!(office(&anne, "delete_show", json!({ "show": a_show, "choice": "give", "to": bruno_id })).await["ok"], true);
    assert_eq!(people(bruno.clone(), b_show.clone()).await, [("Bruno".into(), "owner".into())]);
    assert_eq!(office(&anne, "online_shows", json!({})).await["shows"].as_array().unwrap().len(), 0);
    // Only the owner gives the event away or releases it.
    let c_owned = open(carla.clone()).await;
    assert_eq!((c_owned["outcome"].as_str(), c_owned["owner"].as_str()), (Some("owned"), Some("Bruno")));

    // Bruno releases it: deleted from equievents.ch; Carla, with the key, gets it fresh and owns it.
    assert_eq!(office(&bruno, "delete_show", json!({ "show": b_show, "choice": "release" })).await["ok"], true);
    assert_eq!(server::shows::of_event(&pool, 1).await.unwrap(), None);
    let made = open(carla.clone()).await;
    assert_eq!(made["outcome"], "made", "{made}");
    assert_eq!(office(&carla, "show", json!({ "show": made["show"] })).await["event"]["number"], "EXERCICE");
    // An event never changes its key.
    assert_eq!(office(&anne, "open_with_key", json!({ "access_key": "NEW-KEY" })).await["outcome"], "key");

    stop.send(()).ok();
    test_db.drop().await;
}

/// Signed in on the site: the session's cookie.
async fn site_cookie(router: &axum::Router, email: &str, password: &str) -> String {
    let form = format!("email={}&password={}", urlencode(email), urlencode(password));
    let (status, headers, _) = browser(router, "POST", "/connexion", &[("content-type", "application/x-www-form-urlencoded")], Some(form)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{email}");
    headers["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_string()
}

/// A call of the online version, signed in with `cookie`.
async fn online(router: &axum::Router, cookie: &str, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let headers = [("cookie", cookie), ("sec-fetch-site", "same-origin"), ("content-type", "application/json")];
    let (status, _, body) = browser(router, method, path, &headers, body.map(|b| b.to_string())).await;
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

/// Staff (the user, 2026-11-21), in the online version only: support sees the accounts and acts
/// as one (its events, as it, its changes signed with both names), never as staff; it does not
/// administer. Administrators also set passwords, send reset links, give roles (never their
/// own) and delete accounts with all their data: the shows they own released (who worked on
/// them told), their online version's files gone.
#[tokio::test(flavor = "multi_thread")]
async fn staff_help_and_administer_accounts() {
    let Some(test_db) = TestDb::new("staff").await else { return };
    let pool = db::pool(&test_db.url).unwrap();
    db::migrate(&pool).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (se, _, _) = stand_in_se(0);
    let mailer = mail::Mailer::off();
    let web = server::web::Web::new(dir.path().join("offices"), &url, None);
    let app = api::App { se, web: Some(std::sync::Arc::new(web)), mail_in_background: false, ..api::App::new(pool.clone(), mailer.clone(), "https://www.equievents.ch") };
    let router = api::router(app);
    let served = router.clone();
    tokio::spawn(async move { axum::serve(listener, served.into_make_service_with_connect_info::<std::net::SocketAddr>()).await.unwrap() });
    let mut ids = Vec::new();
    for (name, email, admin) in [("Ada Admin", "ada@example.ch", true), ("Sam Support", "sam@example.ch", false), ("Olga Owner", "olga@example.ch", false), ("Carl Collab", "carl@example.ch", false)] {
        ids.push(auth::create_user(&pool, name, email, "a-good-password", admin, Lang::Fr).await.unwrap().unwrap().id);
    }
    let (ada_id, sam_id, olga_id) = (ids[0], ids[1], ids[2]);
    let r = &router;
    let ada = site_cookie(r, "ada@example.ch", "a-good-password").await;
    let sam = site_cookie(r, "sam@example.ch", "a-good-password").await;
    let olga = site_cookie(r, "olga@example.ch", "a-good-password").await;
    let names = |list: &Value| list.as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();

    // Not staff: no panel.
    let (status, me) = online(r, &olga, "GET", "/app/admin/me", None).await;
    assert_eq!((status, me["role"].as_str(), me["acting_as"].clone()), (StatusCode::OK, Some(""), Value::Null), "{me}");
    let (status, refused) = online(r, &olga, "GET", "/app/admin/accounts", None).await;
    assert_eq!((status, refused["code"].as_str()), (StatusCode::FORBIDDEN, Some("not_staff")));

    // The administrator: the accounts, found by name or address; Sam made support.
    assert_eq!(online(r, &ada, "GET", "/app/admin/me", None).await.1["role"], "admin");
    let (status, all) = online(r, &ada, "GET", "/app/admin/accounts", None).await;
    assert_eq!((status, names(&all)), (StatusCode::OK, vec!["Ada Admin".to_string(), "Carl Collab".into(), "Olga Owner".into(), "Sam Support".into()]), "{all}");
    assert_eq!(names(&online(r, &ada, "GET", "/app/admin/accounts?q=OLGA%40", None).await.1), ["Olga Owner"]);
    assert_eq!(online(r, &ada, "POST", &format!("/app/admin/accounts/{sam_id}/role"), Some(json!({ "role": "support" }))).await.0, StatusCode::OK);
    let (status, refused) = online(r, &ada, "POST", &format!("/app/admin/accounts/{ada_id}/role"), Some(json!({ "role": "none" }))).await;
    assert_eq!((status, refused["code"].as_str()), (StatusCode::BAD_REQUEST, Some("self")));
    assert_eq!(online(r, &ada, "POST", &format!("/app/admin/accounts/{sam_id}/role"), Some(json!({ "role": "king" }))).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(online(r, &sam, "GET", "/app/admin/me", None).await.1["role"], "support");

    // Support: sees the accounts, does not administer, never acts as staff.
    assert_eq!(online(r, &sam, "GET", "/app/admin/accounts", None).await.0, StatusCode::OK);
    for (method, path, body) in [("POST", "password", Some(json!({}))), ("POST", "reset-link", None), ("POST", "role", Some(json!({ "role": "support" }))), ("DELETE", "", None)] {
        let path = format!("/app/admin/accounts/{olga_id}/{path}");
        let (status, refused) = online(r, &sam, method, path.trim_end_matches('/'), body).await;
        assert_eq!((status, refused["code"].as_str()), (StatusCode::FORBIDDEN, Some("not_staff")), "{method} {path}");
    }
    assert_eq!(online(r, &sam, "POST", &format!("/app/admin/accounts/{ada_id}/act"), None).await.1["code"], "not_staff");
    assert_eq!(online(r, &sam, "POST", &format!("/app/admin/accounts/{sam_id}/act"), None).await.1["code"], "self");

    // Olga's event, Carl working on it.
    let (_, opened) = online(r, &olga, "POST", "/app/api/open_with_key", Some(json!({ "access_key": "PRACTICE-KEY" }))).await;
    let (show, remote) = (opened["show"].as_str().unwrap().to_string(), opened["remote"].as_str().unwrap().to_string());
    let (status, added) = online(r, &olga, "POST", "/app/api/add_person", Some(json!({ "show": show, "email": "carl@example.ch" }))).await;
    assert_eq!(status, StatusCode::OK, "{added}");

    // Sam acts as Olga: her events, her changes signed with both names; then back.
    let elsewhere = [("cookie", sam.as_str()), ("origin", "https://evil.example")];
    assert_eq!(browser(r, "POST", &format!("/app/admin/accounts/{olga_id}/act"), &elsewhere, None).await.0, StatusCode::FORBIDDEN);
    let (status, acting) = online(r, &sam, "POST", &format!("/app/admin/accounts/{olga_id}/act"), None).await;
    assert_eq!((status, acting["acting_as"]["name"].as_str()), (StatusCode::OK, Some("Olga Owner")), "{acting}");
    assert_eq!(online(r, &sam, "GET", "/app/admin/me", None).await.1["acting_as"]["name"], "Olga Owner");
    assert_eq!(online(r, &sam, "POST", "/app/api/account", Some(json!({}))).await.1["account"]["name"], "Olga Owner");
    let (_, library) = online(r, &sam, "POST", "/app/api/library", Some(json!({}))).await;
    assert!(library.as_array().unwrap().iter().any(|s| s["id"] == json!(show)), "{library}");
    let (_, view) = online(r, &sam, "POST", "/app/api/show", Some(json!({ "show": show }))).await;
    let (status, set_up) = online(r, &sam, "POST", "/app/api/set_up_class", Some(json!({ "show": show, "published": view["not_set_up"][0]["id"] }))).await;
    assert_eq!(status, StatusCode::OK, "{set_up}");
    assert!(server::shows::log(&pool, &remote).await.unwrap().iter().any(|o| o.author == "Olga Owner (via Sam Support)"));
    // Developer mode: an administrator's only, also acting as an account (the user, 2026-11-22);
    // the scoring tables only, never the dressage tests.
    let reference = Some(json!({ "show": show }));
    assert_eq!(online(r, &sam, "POST", "/app/api/reference_data", reference.clone()).await.1["code"], "not_admin");
    assert_eq!(online(r, &olga, "POST", "/app/api/reference_data", reference.clone()).await.1["code"], "not_admin");
    assert_eq!(online(r, &ada, "POST", &format!("/app/admin/accounts/{olga_id}/act"), None).await.0, StatusCode::OK);
    let (status, data) = online(r, &ada, "POST", "/app/api/reference_data", reference).await;
    assert!(status == StatusCode::OK && data["scoring_tables"].is_array() && data.get("programmes").is_none(), "{data}");
    assert_eq!(online(r, &ada, "POST", "/app/admin/stop", None).await.0, StatusCode::OK);
    assert_eq!(online(r, &sam, "POST", "/app/admin/stop", None).await.0, StatusCode::OK);
    assert_eq!(online(r, &sam, "POST", "/app/api/account", Some(json!({}))).await.1["account"]["name"], "Sam Support");
    assert!(online(r, &sam, "POST", "/app/api/library", Some(json!({}))).await.1.as_array().unwrap().is_empty());

    // A new password: typed (Olga signed out everywhere), made (shown once), or refused.
    let password = format!("/app/admin/accounts/{olga_id}/password");
    let (status, set) = online(r, &ada, "POST", &password, Some(json!({ "password": "another-good-password" }))).await;
    assert_eq!((status, set["password"].clone()), (StatusCode::OK, Value::Null), "{set}");
    assert_eq!(online(r, &olga, "POST", "/app/api/account", Some(json!({}))).await.0, StatusCode::UNAUTHORIZED);
    site_cookie(r, "olga@example.ch", "another-good-password").await;
    let (status, made) = online(r, &ada, "POST", &password, Some(json!({}))).await;
    let made = made["password"].as_str().unwrap().to_string();
    assert!(status == StatusCode::OK && made.len() >= 10);
    let olga = site_cookie(r, "olga@example.ch", &made).await;
    assert_eq!(online(r, &olga, "POST", "/app/api/account", Some(json!({}))).await.1["account"]["name"], "Olga Owner");
    let (status, refused) = online(r, &ada, "POST", &password, Some(json!({ "password": "short" }))).await;
    assert_eq!((status, refused["code"].as_str()), (StatusCode::BAD_REQUEST, Some("password")));

    // A reset link, as the site sends it (kept: e-mail is off).
    let (status, sent) = online(r, &ada, "POST", &format!("/app/admin/accounts/{olga_id}/reset-link"), None).await;
    assert_eq!((status, sent["mail"].as_str()), (StatusCode::OK, Some("off")), "{sent}");
    assert_eq!(mailer.kept().last().unwrap().to, "olga@example.ch");

    // The shows: found by name or owner; another owner (administrators): any account, the
    // previous one stays or leaves; both told.
    let (status, found) = online(r, &sam, "GET", "/app/admin/shows?q=olga", None).await;
    assert_eq!((status, found[0]["id"].as_str(), found[0]["owner"]["name"].as_str(), found[0]["collaborators"].as_i64()), (StatusCode::OK, Some(remote.as_str()), Some("Olga Owner"), Some(1)), "{found}");
    let owner = format!("/app/admin/shows/{remote}/owner");
    let carl_id = ids[3];
    assert_eq!(online(r, &sam, "POST", &owner, Some(json!({ "account": carl_id, "previous": "stay" }))).await.1["code"], "not_staff");
    let (status, set) = online(r, &ada, "POST", &owner, Some(json!({ "account": carl_id, "previous": "stay" }))).await;
    assert_eq!((status, set["owner"]["name"].as_str()), (StatusCode::OK, Some("Carl Collab")), "{set}");
    let (_, details) = online(r, &ada, "GET", &format!("/app/admin/shows/{remote}"), None).await;
    let people = |d: &Value| d["people"].as_array().unwrap().iter().map(|p| format!("{} {}", p["name"].as_str().unwrap(), p["role"].as_str().unwrap())).collect::<Vec<_>>();
    assert_eq!(people(&details), ["Carl Collab owner", "Olga Owner collaborator"]);
    let mails = mailer.kept();
    assert!(mails.iter().any(|m| m.to == "carl@example.ch" && m.subject.contains("vous êtes responsable") && m.body.contains("L’équipe d’equievents.ch vous a nommé·e responsable")));
    assert!(mails.iter().any(|m| m.to == "olga@example.ch" && m.body.contains("a nommé Carl Collab responsable") && m.body.contains("Vous y restez comme collaborateur·trice")));
    assert_eq!(online(r, &ada, "POST", &owner, Some(json!({ "account": carl_id, "previous": "stay" }))).await.1["code"], "owner");
    assert_eq!(online(r, &ada, "POST", &owner, Some(json!({ "account": olga_id, "previous": "maybe" }))).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(online(r, &ada, "POST", "/app/admin/shows/nothing/owner", Some(json!({ "account": olga_id, "previous": "stay" }))).await.1["code"], "not_found");
    // Back to Olga, Carl staying.
    assert_eq!(online(r, &ada, "POST", &owner, Some(json!({ "account": olga_id, "previous": "stay" }))).await.0, StatusCode::OK);
    assert_eq!(people(&online(r, &ada, "GET", &format!("/app/admin/shows/{remote}"), None).await.1), ["Olga Owner owner", "Carl Collab collaborator"]);

    // Deleting Olga's account: her show released (Carl told), her files gone, Sam no longer
    // acting as her; never one's own.
    assert_eq!(online(r, &sam, "POST", &format!("/app/admin/accounts/{olga_id}/act"), None).await.0, StatusCode::OK);
    let (_, details) = online(r, &ada, "GET", &format!("/app/admin/accounts/{olga_id}"), None).await;
    assert_eq!((details["owns"][0]["name"].clone(), details["owns"][0]["collaborators"].clone(), details["account"]["owns"].clone()), (json!("CONCOURS D'EXERCICE EQUIEVENTS"), json!(1), json!(1)), "{details}");
    assert!(dir.path().join(format!("offices/{olga_id}")).exists());
    assert_eq!(online(r, &ada, "DELETE", &format!("/app/admin/accounts/{ada_id}"), None).await.1["code"], "self");
    let (status, deleted) = online(r, &ada, "DELETE", &format!("/app/admin/accounts/{olga_id}"), None).await;
    assert_eq!((status, deleted["released"].clone()), (StatusCode::OK, json!(1)), "{deleted}");
    let shows: i64 = pool.get().await.unwrap().query_one("SELECT count(*) FROM shows", &[]).await.unwrap().get(0);
    assert_eq!(shows, 0);
    let told = mailer.kept().into_iter().rev().find(|m| m.to == "carl@example.ch").unwrap();
    assert!(told.subject.contains("supprimé") && told.body.contains("Le compte de Olga Owner"), "{told:?}");
    // As HTML too, in the site's look, saying whom it went to.
    let html = told.html.as_deref().unwrap();
    assert!(html.contains("Le compte de Olga Owner") && html.contains("carl@example.ch") && html.contains("cid:mark@equievents.ch"));
    assert!(!dir.path().join(format!("offices/{olga_id}")).exists());
    assert_eq!(online(r, &sam, "GET", "/app/admin/me", None).await.1["acting_as"], Value::Null);
    assert_eq!(online(r, &olga, "POST", "/app/api/account", Some(json!({}))).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(names(&online(r, &ada, "GET", "/app/admin/accounts", None).await.1), ["Ada Admin", "Carl Collab", "Sam Support"]);
    assert_eq!(online(r, &ada, "GET", &format!("/app/admin/accounts/{olga_id}"), None).await.0, StatusCode::NOT_FOUND);

    // Sam no longer support.
    assert_eq!(online(r, &ada, "POST", &format!("/app/admin/accounts/{sam_id}/role"), Some(json!({ "role": "none" }))).await.0, StatusCode::OK);
    assert_eq!(online(r, &sam, "GET", "/app/admin/accounts", None).await.0, StatusCode::FORBIDDEN);
    test_db.drop().await;
}

/// From a Rosson export (the user, 2026-11-21): the first to put an event up owns it, a past one
/// too, whose key Swiss Equestrian no longer accepts: taken on trust ("if they have the export,
/// they have the key"), and only with the export. A key Swiss Equestrian accepts wins over one
/// taken on trust: that show stays, no longer the event's, its owner told.
#[tokio::test]
async fn rosson_exports_go_up_with_their_key() {
    let Some(test_db) = TestDb::new("export").await else { return };
    let pool = db::pool(&test_db.url).unwrap();
    db::migrate(&pool).await.unwrap();
    let (se, _, _) = stand_in_se(0);
    let mailer = mail::Mailer::off();
    let app = api::App { mail_in_background: false, se, ..api::App::new(pool.clone(), mailer.clone(), "https://www.equievents.ch/") };
    let s = Server { router: api::router(app), pool, mailer: mailer.clone() };
    for (name, email) in [("Anne Office", "anne@example.ch"), ("Bruno Second", "bruno@example.ch"), ("Carla Third", "carla@example.ch")] {
        auth::create_user(&s.pool, name, email, "a-good-password", false, Lang::Fr).await.unwrap().unwrap();
    }
    let anne = s.sign_in("anne@example.ch", "a-good-password").await;
    let bruno = s.sign_in("bruno@example.ch", "a-good-password").await;
    let carla = s.sign_in("carla@example.ch", "a-good-password").await;
    let past = |key: &str, from_export: bool| json!({ "access_key": key, "event_id": 8352, "name": "CSN d’antan", "number": "2025.8352", "from_export": from_export });
    let put = |token: &String, body: Value| {
        let token = token.clone();
        let s = &s;
        async move { s.job(&token, s.call("POST", "/api/shows", Some(&token), Some(body)).await).await }
    };

    // A past event's key: refused without the export; with it, Anne's, the first.
    let (status, refused) = put(&anne, past("OLD-KEY", false)).await;
    assert_eq!((status, refused["code"].as_str()), (StatusCode::FORBIDDEN, Some("key")), "{refused}");
    let (status, made) = put(&anne, past("OLD-KEY", true)).await;
    assert_eq!((status, made["created"].clone(), made["role"].as_str(), made["event_id"].as_i64()), (StatusCode::OK, json!(true), Some("owner"), Some(8352)), "{made}");
    let show = made["id"].as_str().unwrap().to_string();
    assert!(show.starts_with("2025.8352-"), "{show}");
    // Again: hers. Bruno, with the same export: Anne's, he asks her; with another key: not this
    // event's.
    assert_eq!(put(&anne, past("OLD-KEY", true)).await.1["created"], json!(false));
    let (status, owned) = put(&bruno, past("OLD-KEY", true)).await;
    assert_eq!((status, owned["code"].as_str(), owned["owner"].as_str(), owned["id"].as_str()), (StatusCode::CONFLICT, Some("claimed"), Some("Anne Office"), Some(show.as_str())));
    let (status, other) = put(&bruno, past("ANOTHER-KEY", true)).await;
    assert_eq!((status, other["code"].as_str()), (StatusCode::FORBIDDEN, Some("key")));

    // Carla puts the practice event (1) up with a key taken on trust; Bruno comes with the key
    // Swiss Equestrian accepts: the event's show is made for him, Carla's stays, unlinked, and
    // she is told.
    let (_, squatted) = put(&carla, json!({ "access_key": "MADE-UP", "event_id": 1, "name": "Exercice", "number": "EXERCICE", "from_export": true })).await;
    assert_eq!(squatted["created"], json!(true), "{squatted}");
    let carlas = squatted["id"].as_str().unwrap().to_string();
    let (status, real) = put(&bruno, json!({ "access_key": "PRACTICE-KEY" })).await;
    assert_eq!((status, real["created"].clone(), real["role"].as_str()), (StatusCode::OK, json!(true), Some("owner")), "{real}");
    assert_ne!(real["id"].as_str(), Some(carlas.as_str()));
    let client = s.pool.get().await.unwrap();
    let row = client.query_one("SELECT event_id, access_key FROM shows WHERE id = $1", &[&carlas]).await.unwrap();
    assert_eq!((row.get::<_, Option<i64>>(0), row.get::<_, Option<String>>(1)), (None, None));
    let told = mailer.kept().into_iter().find(|m| m.to == "carla@example.ch").unwrap();
    assert!(told.subject.contains("n’est plus lié à son événement") && told.body.contains("Swiss Equestrian a accepté une autre clé"), "{told:?}");
    // A key taken on trust never wins over one Swiss Equestrian accepted.
    let (status, again) = put(&carla, json!({ "access_key": "MADE-UP", "event_id": 1, "name": "Exercice", "number": "EXERCICE", "from_export": true })).await;
    assert_eq!((status, again["code"].as_str()), (StatusCode::FORBIDDEN, Some("key")), "{again}");

    drop(client);
    drop(s);
    test_db.drop().await;
}

/// The online version, a command that takes longer than the proxy in front waits (the user,
/// 2026-11-22: an import waiting on Swiss Equestrian spun on): it answers with a job, and the
/// browser asks again until it is done; the job is its account's only.
#[tokio::test(flavor = "multi_thread")]
async fn slow_online_commands_answer_with_a_job() {
    let Some(test_db) = TestDb::new("slow").await else { return };
    let pool = db::pool(&test_db.url).unwrap();
    db::migrate(&pool).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let mut web = server::web::Web::new(dir.path().join("offices"), &url, None);
    // No time at all: every command answers with a job.
    web.answer_within = std::time::Duration::ZERO;
    let app = api::App { web: Some(std::sync::Arc::new(web)), ..api::App::new(pool.clone(), mail::Mailer::off(), "https://www.equievents.ch") };
    let router = api::router(app);
    let served = router.clone();
    tokio::spawn(async move { axum::serve(listener, served.into_make_service_with_connect_info::<std::net::SocketAddr>()).await.unwrap() });
    for (name, email) in [("Anne Office", "anne@example.ch"), ("Bruno Second", "bruno@example.ch")] {
        auth::create_user(&pool, name, email, "a-good-password", false, Lang::Fr).await.unwrap().unwrap();
    }
    let r = &router;
    let anne = site_cookie(r, "anne@example.ch", "a-good-password").await;
    let bruno = site_cookie(r, "bruno@example.ch", "a-good-password").await;

    let (status, started) = online(r, &anne, "POST", "/app/api/account", Some(json!({}))).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{started}");
    let job = format!("/app/api/jobs/{}", started["job"].as_str().unwrap());
    assert_eq!(online(r, &bruno, "GET", &job, None).await.0, StatusCode::NOT_FOUND);
    let mut answer = online(r, &anne, "GET", &job, None).await;
    for _ in 0..200 {
        if answer.0 != StatusCode::ACCEPTED {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        answer = online(r, &anne, "GET", &job, None).await;
    }
    assert_eq!((answer.0, answer.1["account"]["name"].as_str()), (StatusCode::OK, Some("Anne Office")), "{}", answer.1);
    // Given once.
    assert_eq!(online(r, &anne, "GET", &job, None).await.0, StatusCode::NOT_FOUND);
    test_db.drop().await;
}
