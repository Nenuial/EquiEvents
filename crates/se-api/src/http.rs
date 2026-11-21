//! Sending requests to Swiss Equestrian.
//!
//! The requests go out as Rosson's HTTP layer (AngularJS `$http`) sends them: the query sorted by
//! name and encoded like `$httpParamSerializer` (a space as `+`, `:` `@` `$` `,` `;` as they
//! are), `Accept: application/json, text/plain, */*`, and with a body
//! `Content-Type: application/json;charset=utf-8` (checked against Rosson:
//! `golden/corpus/http-get.json`, `http-send.json`). Object keys in a body may come in another
//! order than Rosson's; JSON does not give them one.
//!
//! What happens around them is done properly (docs/deviations.md E42):
//!
//! - while Swiss Equestrian answers 202 (preparing), a polled request is asked again every
//!   4 seconds, as Rosson does, but not forever (Rosson: until the app is closed);
//! - a read that gets no answer, or a gateway error, is tried again a few times; a write is not
//!   (the office decides);
//! - an error says what Swiss Equestrian said (its `message`), not only the HTTP status.
//!
//! No I/O here: a [`Call`] says what to do next ([`Next`]); the app sends and waits, with
//! whatever it has (an HTTP client on the desktop, `fetch` in the browser). [`run`] drives a
//! call with any async `send` and `wait`.

use std::future::Future;
use std::time::Duration;

use serde_json::Value;

use crate::{Endpoint, Method, Request};

/// What goes over the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(&'static str, &'static str)>,
    pub body: Option<Vec<u8>>,
}

/// What comes back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    /// The reason phrase, e.g. "Not Found" (may be empty).
    pub status_text: String,
    pub body: Vec<u8>,
}

/// Why a request did not succeed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// Swiss Equestrian answered with an error.
    Refused { status: u16, message: String },
    /// No answer (no connection, timeout, gateway errors), after the retries.
    Unreachable(String),
    /// Still preparing the answer after the time allowed.
    StillPreparing,
    /// An answer that is not JSON.
    BadAnswer(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Refused { status, message } => write!(f, "Swiss Equestrian refused the request ({status}): {message}"),
            ApiError::Unreachable(why) => write!(f, "Swiss Equestrian could not be reached: {why}"),
            ApiError::StillPreparing => f.write_str("Swiss Equestrian is still preparing the data; try again in a few minutes"),
            ApiError::BadAnswer(why) => write!(f, "Swiss Equestrian sent an answer that cannot be read: {why}"),
        }
    }
}

/// How long to keep trying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Asking again while the server prepares (Rosson: 4 s).
    pub poll_every: Duration,
    /// How many times to ask again while it prepares.
    pub max_polls: u32,
    /// Further attempts for a read without answer.
    pub retries: u32,
    /// Wait before the n-th retry: n times this.
    pub backoff: Duration,
}

impl Default for Limits {
    /// Polling for 10 minutes; 3 more attempts after 2, 4 and 6 seconds.
    fn default() -> Limits {
        Limits { poll_every: Duration::from_secs(4), max_polls: 150, retries: 3, backoff: Duration::from_secs(2) }
    }
}

/// What to do next.
#[derive(Debug, Clone, PartialEq)]
pub enum Next {
    Send(HttpRequest),
    Wait(Duration),
    Done(Value),
    Failed(ApiError),
}

/// One request, from the first send to its outcome.
#[derive(Debug, Clone)]
pub struct Call {
    request: HttpRequest,
    polled: bool,
    limits: Limits,
    polls: u32,
    attempts: u32,
}

impl Call {
    pub fn new(base: &str, request: &Request, limits: Limits) -> Call {
        let polled = Endpoint::of_path(&request.path).is_some_and(Endpoint::polled);
        Call { request: http_request(base, request), polled, limits, polls: 0, attempts: 0 }
    }

    pub fn first(&self) -> Next {
        Next::Send(self.request.clone())
    }

    /// After a [`Next::Wait`].
    pub fn waited(&self) -> Next {
        Next::Send(self.request.clone())
    }

    /// The server answered.
    pub fn answered(&mut self, response: HttpResponse) -> Next {
        match response.status {
            202 if self.polled => {
                self.polls += 1;
                if self.polls > self.limits.max_polls {
                    Next::Failed(ApiError::StillPreparing)
                } else {
                    Next::Wait(self.limits.poll_every)
                }
            }
            200..=299 => {
                if response.body.iter().all(u8::is_ascii_whitespace) {
                    return Next::Done(Value::Null);
                }
                match serde_json::from_slice(&response.body) {
                    Ok(value) => Next::Done(value),
                    Err(e) => Next::Failed(ApiError::BadAnswer(e.to_string())),
                }
            }
            502..=504 => self.retry(format!("{} {}", response.status, response.status_text)),
            status => {
                let message = serde_json::from_slice::<Value>(&response.body).ok().and_then(|v| v["message"].as_str().map(str::to_string)).filter(|m| !m.is_empty());
                Next::Failed(ApiError::Refused { status, message: message.unwrap_or(response.status_text) })
            }
        }
    }

    /// No answer (connection failed, timed out).
    pub fn unanswered(&mut self, why: String) -> Next {
        self.retry(why)
    }

    fn retry(&mut self, why: String) -> Next {
        if self.request.method == Method::Get && self.attempts < self.limits.retries {
            self.attempts += 1;
            Next::Wait(self.limits.backoff * self.attempts)
        } else {
            Next::Failed(ApiError::Unreachable(why))
        }
    }
}

/// Send a request until it has an outcome. `send` does the HTTP request (an error: no answer);
/// `wait` waits.
pub async fn run<S, SF, W, WF>(base: &str, request: &Request, limits: Limits, mut send: S, mut wait: W) -> Result<Value, ApiError>
where
    S: FnMut(HttpRequest) -> SF,
    SF: Future<Output = Result<HttpResponse, String>>,
    W: FnMut(Duration) -> WF,
    WF: Future<Output = ()>,
{
    let mut call = Call::new(base, request, limits);
    let mut next = call.first();
    loop {
        next = match next {
            Next::Send(r) => match send(r).await {
                Ok(response) => call.answered(response),
                Err(why) => call.unanswered(why),
            },
            Next::Wait(d) => {
                wait(d).await;
                call.waited()
            }
            Next::Done(value) => return Ok(value),
            Next::Failed(error) => return Err(error),
        }
    }
}

/// The request as AngularJS sends it.
pub fn http_request(base: &str, request: &Request) -> HttpRequest {
    let mut query: Vec<&(String, String)> = request.query.iter().collect();
    query.sort_by(|a, b| a.0.cmp(&b.0));
    let query: Vec<String> = query.iter().map(|(k, v)| format!("{}={}", encode_query(k), encode_query(v))).collect();
    let url = if query.is_empty() { format!("{base}{}", request.path) } else { format!("{base}{}?{}", request.path, query.join("&")) };
    let mut headers = vec![("Accept", "application/json, text/plain, */*")];
    let body = request.body.as_ref().map(|b| serde_json::to_vec(b).unwrap_or_default());
    if body.is_some() {
        headers.push(("Content-Type", "application/json;charset=utf-8"));
    }
    HttpRequest { method: request.method, url, headers, body }
}

/// The access key of a request that only downloads: a GET to `base`, of one of `endpoints`, with
/// exactly one access key and nothing to upload. Anything else: `None` (not to be sent as a
/// download).
pub fn download_key<'a>(request: &'a HttpRequest, base: &str, endpoints: &[&str]) -> Option<&'a str> {
    let rest = request.url.strip_prefix(base)?.strip_prefix('/')?;
    let (endpoint, query) = rest.split_once('?').unwrap_or((rest, ""));
    let keys: Vec<&str> = query.split('&').filter_map(|q| q.strip_prefix("access_key=")).collect();
    let ok = request.method == Method::Get && request.body.is_none() && endpoints.contains(&endpoint) && keys.len() == 1 && !keys[0].is_empty();
    ok.then(|| keys[0])
}

/// AngularJS's `encodeUriQuery`: `encodeURIComponent`, then `@ : $ , ;` kept and a space as `+`.
fn encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' | b'@' | b':' | b'$' | b',' | b';' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::pin::pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    /// Runs a future whose parts are all ready at once (the fakes below).
    fn block_on<F: Future>(f: F) -> F::Output {
        fn raw() -> RawWaker {
            fn clone(_: *const ()) -> RawWaker {
                raw()
            }
            fn noop(_: *const ()) {}
            static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        let waker = unsafe { Waker::from_raw(raw()) };
        let mut f = pin!(f);
        loop {
            if let Poll::Ready(v) = f.as_mut().poll(&mut Context::from_waker(&waker)) {
                return v;
            }
        }
    }

    fn answer(status: u16, body: &str) -> Result<HttpResponse, String> {
        Ok(HttpResponse { status, status_text: String::new(), body: body.as_bytes().to_vec() })
    }

    /// Run a request against answers given in turn; returns the outcome, the requests sent and the waits.
    fn against(request: &Request, answers: Vec<Result<HttpResponse, String>>) -> (Result<Value, ApiError>, usize, Vec<Duration>) {
        let answers = RefCell::new(answers.into_iter());
        let sent = RefCell::new(0);
        let waits = RefCell::new(Vec::new());
        let outcome = block_on(run(
            "https://api.invalid/v2",
            request,
            Limits { max_polls: 3, ..Limits::default() },
            |_| {
                *sent.borrow_mut() += 1;
                std::future::ready(answers.borrow_mut().next().expect("asked more than expected"))
            },
            |d| {
                waits.borrow_mut().push(d);
                std::future::ready(())
            },
        ));
        (outcome, sent.into_inner(), waits.into_inner())
    }

    fn get(path: &str) -> Request {
        Request { method: Method::Get, path: path.into(), query: vec![], body: None }
    }

    /// Only a GET of one of the endpoints, with one access key, is a download.
    #[test]
    fn what_a_download_is() {
        const BASE: &str = "https://se.invalid/v2";
        let request = |method: Method, url: &str, body: Option<Vec<u8>>| HttpRequest { method, url: format!("{BASE}{url}"), headers: Vec::new(), body };
        let endpoints = ["getVeranstaltung"];
        assert_eq!(download_key(&request(Method::Get, "/getVeranstaltung?access_key=K&app_version=3.3.15", None), BASE, &endpoints), Some("K"));
        for refused in [
            request(Method::Post, "/putVeranstaltung", Some(b"{}".to_vec())),
            request(Method::Get, "/getVeranstaltung", None),
            request(Method::Get, "/getVeranstaltung?access_key=", None),
            request(Method::Get, "/getVeranstaltung?access_key=K&access_key=L", None),
            request(Method::Get, "/getPersonen?access_key=K", None),
            request(Method::Get, "/getVeranstaltung?access_key=K", Some(Vec::new())),
            HttpRequest { url: "https://elsewhere.invalid/v2/getVeranstaltung?access_key=K".into(), ..request(Method::Get, "", None) },
        ] {
            assert_eq!(download_key(&refused, BASE, &endpoints), None, "{}", refused.url);
        }
    }

    #[test]
    fn urls_and_headers_as_rossons() {
        let fixtures: Vec<Value> = ["http-get.json", "http-send.json"]
            .iter()
            .flat_map(|f| serde_json::from_str::<Vec<Value>>(&std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus").join(f)).unwrap()).unwrap())
            .collect();
        let keys = crate::Keys { access_key: "<access_key>".into(), rosson_key: "<rosson_key>".into(), rosson_version: "3.3.15".into() };
        let theirs = |method: &str, start: &str| fixtures.iter().find(|r| r["method"] == method && r["url"].as_str().unwrap().starts_with(start)).unwrap().clone();
        let check = |ours: HttpRequest, theirs: Value| {
            assert_eq!(ours.url, theirs["url"].as_str().unwrap());
            let headers: Vec<(String, String)> = theirs["headers"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
            assert_eq!(ours.headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>(), headers);
        };
        check(http_request("", &crate::load::get_veranstaltung(&keys, Some("2025-06-15 08:25:07"))), theirs("GET", "/getVeranstaltung?access_key=%3Caccess_key%3E&app_version=3.3.15&geaendert_nach"));
        check(http_request("", &crate::load::get_stammdaten(&keys)), theirs("GET", "/getStammdaten"));
        let redacted = crate::Keys { access_key: "REDACTED".into(), ..keys };
        check(http_request("", &crate::event::delete_startlisten(&show::model::PublishedClassId(103857), &redacted)), theirs("DELETE", "/deleteStartlisten"));
        let post = Request { method: Method::Post, path: "/putVeranstaltung".into(), query: vec![], body: Some(serde_json::json!({})) };
        check(http_request("", &post), theirs("POST", "/putVeranstaltung"));
        assert_eq!(http_request(crate::PRODUCTION, &post).url, "https://my.swiss-equestrian.ch/api/veranstaltersoftware/v2/putVeranstaltung");
    }

    #[test]
    fn polls_while_preparing_but_not_forever() {
        let (outcome, sent, waits) = against(&get("/getVeranstaltung"), vec![answer(202, ""), answer(202, ""), answer(200, r#"{"data":1}"#)]);
        assert_eq!(outcome, Ok(serde_json::json!({ "data": 1 })));
        assert_eq!((sent, waits), (3, vec![Duration::from_secs(4); 2]));
        let (outcome, sent, _) = against(&get("/getVeranstaltung"), vec![answer(202, ""); 4]);
        assert_eq!((outcome, sent), (Err(ApiError::StillPreparing), 4));
        // Not polled: 202 is an answer like 200 (Rosson's Xhr.get).
        assert_eq!(against(&get("/getStammdaten"), vec![answer(202, "{}")]).0, Ok(serde_json::json!({})));
    }

    #[test]
    fn reads_are_tried_again_writes_are_not() {
        let (outcome, sent, waits) = against(&get("/getStammdaten"), vec![Err("timeout".into()), answer(503, ""), answer(200, "{}")]);
        assert_eq!(outcome, Ok(serde_json::json!({})));
        assert_eq!((sent, waits), (3, vec![Duration::from_secs(2), Duration::from_secs(4)]));
        let post = Request { method: Method::Post, path: "/putResultate".into(), query: vec![], body: Some(serde_json::json!({})) };
        let (outcome, sent, _) = against(&post, vec![Err("timeout".into())]);
        assert_eq!((outcome, sent), (Err(ApiError::Unreachable("timeout".into())), 1));
    }

    #[test]
    fn errors_say_what_swiss_equestrian_said() {
        let (outcome, _, _) = against(&get("/getVeranstaltung"), vec![answer(403, r#"{"message":"Ungültiger Zugangsschlüssel"}"#)]);
        assert_eq!(outcome, Err(ApiError::Refused { status: 403, message: "Ungültiger Zugangsschlüssel".into() }));
        let (outcome, _, _) = against(&get("/getVeranstaltung"), vec![Ok(HttpResponse { status: 500, status_text: "Internal Server Error".into(), body: b"<html>".to_vec() })]);
        assert_eq!(outcome, Err(ApiError::Refused { status: 500, message: "Internal Server Error".into() }));
        assert!(matches!(against(&get("/getStammdaten"), vec![answer(200, "<html>")]).0, Err(ApiError::BadAnswer(_))));
        assert_eq!(against(&get("/lookupZuweisung"), vec![answer(200, "")]).0, Ok(Value::Null));
    }
}
