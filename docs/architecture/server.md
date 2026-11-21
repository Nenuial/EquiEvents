# equievents.ch — the server

Status, 2026-11-21: the master copy of every show, accounts (anyone makes their own on the site),
who owns a show (its key, checked with Swiss Equestrian), the nightly backups, the desktop's sync
with it, the desktop app's downloads and updates ([releases.md](releases.md)), and Swiss
Equestrian from the server, downloads only. Why it works this way:
[data-and-sync.md](data-and-sync.md).

## What this first version does (the user, 2026-11-21)

- **equievents.ch is the master.** It keeps every show; the desktop apps (macOS, Windows) are
  downloaded from it, sync only with it, and **never talk to Swiss Equestrian**.
- **An account first.** Anyone makes one, on the site or in the app; the apps do nothing
  without one (signed out, they offer only signing in and making an account). A computer that
  signed in keeps the account, also offline.
- **Downloads from Swiss Equestrian, with a key**: an event (its show made on the server, its
  first enterer the owner), its updates, and the **riders and horses** (with the key of one of
  the account's shows; kept per account, shared by all its shows and computers).
- **Nothing is ever sent to Swiss Equestrian**: no start lists, results or anything else. The
  apps' « Envoyer » goes to the server's simulation, which shows what would have been sent.
- **The online version** (decided the same day): a show office that never works offline works in
  the browser or on a phone, at `/app/`, signed in on the site; the downloads page too only
  signed in (below).

## The online version

The same interface as the desktop app, at `www.equievents.ch/app/` (`crates/server/src/web.rs`):

- **Signed in on the site** (`/connexion`, `/anmelden`; a cookie: `session.rs`). Without it,
  `/app/` sends to the sign-in and back. Signing out on the site signs the online version out.
- **One office per account, on the server** (the office core as the desktop app runs it,
  `crates/office`), its shows' copies under `OFFICES_DIR/<account>` (the `offices` volume). It
  works **straight on the master**: each change goes into the show's log in Postgres at once,
  what others (laptops, other browsers) did comes down before a command, two seconds apart at
  most; every show of the account is there. One copy per account, not per show: undo, the
  author of each change and the conflicts already seen stay each person's.
- The interface's commands: `POST /app/api/<command>`; its files (a Rosson export imported, a
  show exported, a print): `POST /app/files/<name>`, `GET /app/files/<n>/<name>`, the
  account's own only, and a command's `path` only among them. The office never signs in or
  out by itself nor reaches another server (`set_server`, `sign_in`… refused). Requests from
  other sites are refused (the cookie is `SameSite=Lax`, and the origin is checked).
- **« Comptes », for staff** (the user, 2026-11-21; `crates/server/src/admin.rs`,
  `/app/admin/…`, in the online version only). **Support** finds an account and **acts as it**
  to help it: the session then works on that account's office (its shows, as it), every change
  signed « Name (via Staff) », a band on top of every screen and the way back; never as an
  administrator or another support account. **Administrators** do the same and administer: a
  new password (made and shown once, or typed; the account signed out everywhere), a reset link
  by e-mail (said when e-mail is off), the roles (administrator, support; never their own), and
  deleting an account with all its data (never their own): the shows it owns released (deleted
  for everyone, who worked on them told by e-mail), its memberships, sessions, riders and horses,
  its online version's files; its changes in others' shows stay in their history. A second
  tab, « Concours »: the shows on equievents.ch, found by name, number or owner, and who works on
  each; administrators give a show **another owner** (the user, 2026-11-21), any account, the
  previous one staying as a collaborator or leaving, both told by e-mail. What staff may do is
  always the session's own account's, never the one it acts as; each action is logged.
- **Online only**, never offline in the browser (the user, 2026-11-21): that is the desktop
  apps' job. A show imported in the online version stays in its account's copy until put on
  equievents.ch with its key (and is then in the nightly backup like every show).

## What runs

One program, `equievents-server` (`crates/server`), in one image, as Gestion Concours':

| Container | Command | Does |
|---|---|---|
| `eq-db` | Postgres 18.4 | accounts, shows, their operation logs |
| `eq-web` | `web` | the API the apps sync with (`/api/…`), the site (accounts, signing in, the desktop app's downloads), the online version (`/app/`, the accounts' offices in the `offices` volume); port 3000, on the `npm` network for Nginx Proxy Manager |
| `eq-worker` | `worker` | the nightly backup (`pg_dump`, 02:30, 14 kept); Swiss Equestrian's jobs next |

Both apply the migrations when they start (`crates/server/migrations`, each once).

## Data

- `users`: personal accounts (name, e-mail, Argon2id password hash, administrator, support or
  neither). Anyone makes their own (decision 2026-11-14): an account alone opens nothing, a show
  needs its Swiss Equestrian keys. The first administrator: `create-admin`; then roles in the
  online version's « Comptes », or `set-role e-mail admin|support|none`; `create-user` and
  `set-password` (a new password printed once, the account signed out everywhere) from the
  command line too. Each has a language (`fr`, `de`, `it`) for their e-mails.
- `sessions.acting_as`: the account a staff session acts as (`admin.rs`), none when it works
  for itself.
- `password_resets`: a forgotten password's link, its token kept as a SHA-256 hash, valid an
  hour, used once; asking again replaces it.
- `sessions`: a signed-in app's token, kept as a SHA-256 hash; valid a year after its last use.
- `shows`: a show (its own id, from the event number and 8 random hex digits: local ids like
  `2026.8352` repeat across rehearsals), its name, its Swiss Equestrian event id and key (an
  event has one show).
- `show_members`: who works on a show — its owner (one) and its collaborators.
- `show_requests`: asking a show's owner for access or for the show; one open request per
  person and show; pending, accepted, refused or withdrawn, with when and by whom.
- `registries`: Swiss Equestrian's riders and horses per account, as delivered (with the
  riders' contact data), and when.
- `operations`: a show's log. Each operation once (by its id), as the show core writes it
  (`show::oplog::Operation`), with who pushed it and when it arrived; `seq` is the arrival
  order. The state is computed from the log, by the apps as by the server, never stored.

## The API

JSON over HTTPS; `Authorization: Bearer <token>` after signing in (`crates/server/src/api.rs`):

| | |
|---|---|
| `GET /api/health` | the server and its database answer (Docker's health check) |
| `POST /api/register` | `{name, email, password, device}` → `{token, user}`: a new account, signed in |
| `POST /api/login` | `{email, password, device}` → `{token, user}` |
| `POST /api/password-reset` | `{email, language}`: a link by e-mail if the account exists; the same answer either way |
| `POST /api/logout` | the token signed out |
| `GET /api/me` | who is signed in, and `archive_after_weeks` |
| `POST /api/me` | `{archive_after_weeks}` (1–104, or `null`: by hand only): the account archives the shows it owns that many weeks after their last day |
| `GET /api/shows` | the shows one works on: id, name, event, role, owner, requests waiting (for the owner), operations, cursor, `archived`, `last_day` (from the pushed operations); first archives those due (their owner's setting) |
| `POST /api/shows` | `{access_key, event_id?, name?, number?}`: an event's show (below), checked with Swiss Equestrian: 202 `{job}`; the key alone downloads the event; `{name, number?}`: a show without an event, its maker's |
| `GET /api/jobs/{id}` | what waits for Swiss Equestrian (one's own): 202 `{working}`, then the answer the request would have given |
| `GET /api/swiss-equestrian` | `{downloads, sends}`: which Swiss Equestrian (`production`, `stage`, `none`); where « Envoyer » goes (`simulation`, always) |
| `POST /api/shows/{id}/send` | `{what, class?, published?, id?, language?, through?}`: « Envoyer », to the simulation (202 `{job}` → `{sent, server, warnings}`), once the server has the app's operation `through` (409 `not_synced`) |
| `POST /api/shows/{id}/registry` | the riders and horses, downloaded with the show's key, kept for one's account (202 `{job}` → `{persons, horses, at}`) |
| `GET /api/registry` | one's account's riders and horses (`/api/registry/about`: how many, of when) |
| `POST /api/demo` | `{driving}`: one of Swiss Equestrian's demo events, its three answers, for an app's training show (202 `{job}`) |
| `POST /api/shows/{id}/update` | what Swiss Equestrian changed since the event was loaded (202 `{job}` → `{added}`) |
| `GET /api/shows/{id}/received` | what the simulation received for the show, the latest first |
| `GET /api/shows/{id}/members` | who works on it |
| `POST /api/shows/{id}/handover` | `{to}`: the owner gives the show to a collaborator and leaves it |
| `DELETE /api/shows/{id}` | the owner releases the event: the show deleted from equievents.ch for everyone, its key free again |
| `POST /api/shows/{id}/members` | `{email}`: the owner lets someone with an account work on it |
| `DELETE /api/shows/{id}/members/{user}` | the owner takes a collaborator off; a collaborator leaves |
| `GET /api/shows/{id}/requests` | the requests waiting for the owner |
| `POST /api/shows/{id}/archive` | `{archived}`: the owner archives the show or takes it out (then not archived on its own again); 403 `not_owner` for a collaborator. Only how every member's library lists it |
| `POST /api/shows/{id}/requests` | `{kind: access\|ownership, access_key}`: ask the owner |
| `GET /api/requests` | one's own requests and their answers |
| `POST /api/requests/{id}` | `{accept, stay?}`: the owner answers |
| `DELETE /api/requests/{id}` | who asked takes it back |
| `POST /api/shows/{id}/ops` | push `{ops}` (≤ 5000) → `{added, known, cursor}` |
| `GET /api/shows/{id}/ops?after=n` | pull → `{ops, cursor, more}` (≤ 2000 at a time) |
| `GET /api/updates/{target}/{arch}/{version}` | the desktop app's update check, no account: the newer release in Tauri's format, else 204 ([releases.md](releases.md)) |
| `GET /downloads/{file}` | an installer or update `latest.json` names (ranges: a download picked up again) |

- **Push** stores the operations the server does not have; the same push again changes nothing.
  Each must be an operation the show core reads (else nothing is stored, and the error says
  which). Which operations apply and which are set aside is decided the same way on every
  device (the reconciliation rules), so none is refused for that.
- **Pull** gives the operations after the app's cursor, in arrival order; the app applies them
  in the operations' own order (their clocks), so every device reaches the same state.
- Pushes to one show are taken one at a time (the show's row is locked), so `seq` follows the
  commits: an operation never appears behind a cursor an app already passed.
- A show seen by someone who does not work on it is "not found"; administrators see every show.
- Refused account details: 400 (409 for an address with an account) and a `code` the apps
  translate: `name`, `email`, `password` (10 to 200 characters), `taken`.

## Who owns a show

Decided 2026-11-14 (`crates/server/src/shows.rs`):

- **The key decides.** The first account to enter an event's Swiss Equestrian key owns its show
  on the server (`POST /api/shows` → made, `owner`). Entering it again: one's role.
- **The others ask.** Another account entering the key learns who owns the show (409,
  `claimed`, the owner's name, their open request if any) and asks, with the key: for
  **access** (to work on it as a collaborator) or for the **ownership**. The owner gets an
  e-mail, sees the requests (`GET …/requests`, and their count in the list of shows) and
  accepts or refuses; who asked gets an e-mail. A wrong key opens nothing (403, `key`).
- **A transfer:** accepting, the owner says whether they stay on the show as a collaborator or
  leave it (`stay`; without it, 400 `stay`). The show has one owner at every moment (the
  database refuses two).
- **Without a request:** the owner lets someone with an account work on the show (they get an
  e-mail), and takes collaborators off; a collaborator can leave. The owner hands the show
  over before leaving.
- Only the owner answers requests; who asked can take a request back while it waits. Entering
  keys is limited (30 an hour per account): keys are not guessed.
- **Swiss Equestrian checks the key** (decided 2026-11-21: real events may be downloaded now):
  before anyone owns a show, the server asks Swiss Equestrian which event the key opens
  (`getVeranstaltung`). A key it does not know opens nothing (403, `key`); a show put online
  from the app (its `event_id`) must be that event's. An event never changes its key (the user,
  2026-11-21): a show keeps the key it was made with, and no other opens it. Swiss Equestrian's demo keys are no one's (400,
  `training`: the app's settings load them).
- **The key alone downloads the event**: the server asks Swiss Equestrian for the officials,
  the master data and the event, as Rosson does, and a show made now starts with them
  (operations of the server, device `equievents.ch`, by the person who entered the key); the
  app then downloads it like any show of the server (*Importer › Charger de Swiss
  Equestrian…* in the library).
- Swiss Equestrian may take minutes to prepare an event (202, asked again every 4 s, 10
  minutes at most), and Nginx Proxy Manager waits a minute at most: the server answers 202
  with a job, and the app asks `GET /api/jobs/{id}` until it is done (in memory, kept an hour).
- `SE_SERVER` says which Swiss Equestrian: `production` (the default), `stage`, or `none`
  (development: the app says which event a key opens, unchecked; nothing downloaded). Only
  downloads go there (`crates/server/src/se.rs`: a GET of a loading endpoint with one key, else
  refused). To try it without a real key: `cargo run -p server --example se_stand_in` (a
  stand-in answering with the practice event to `PRACTICE-KEY`).

## Swiss Equestrian

Only the server talks to it (`crates/server/src/se.rs`; the contract: [se-api.md](se-api.md)):

- **Downloads only**: GETs of the loading and registry endpoints with one key; anything else is
  refused before it leaves (`se::ENDPOINTS`). Downloads an account asks for (updates, riders
  and horses, demo events) are limited to 30 an hour.
- **Loading and updating**: an event's show from its key (above); its update (the app's *Mise
  à jour de Swiss Equestrian*: the officials, the master data and what changed in the event
  since its last delivery, Rosson's update), into the show's log.
- **The riders and horses** (`getPersonen`, `getPferde`: those with a valid status, with their
  contact data), downloaded with the key of a show one works on and kept for one's account
  (`registries`, one row per account, replaced at each download); the apps copy them for
  searching offline, after signing in and whenever the account's copy is newer; the copy goes
  when the account signs out.
- **Swiss Equestrian's demo events**: downloaded with their demo keys and handed to the app,
  which makes a training show of them on the computer; nothing kept on the server.
- **« Envoyer » goes to the simulation, never to Swiss Equestrian** (the user, 2026-11-21): the
  app pushes its changes, then asks the server to send, naming its latest operation
  (`through`); the server builds Rosson's requests from what the show is there, the simulation
  (`se_api::simulation`, per show, in memory: a restart empties it) answers, and the answer
  goes into the log (`Uploaded`), by the one who sent. The server has no code that uploads;
  `SE_SEND` set to anything but `simulation` stops it at start. The app's rehearsals and
  training shows send to the simulation on the computer.
- Requests that wait for Swiss Equestrian are jobs (above). The Rosson version sent
  (`rosson_version`, `app_version`) is read from Rosson's update feed once a day, as Rosson
  does; today's `rosson_key` is the server's date (`TZ=Europe/Zurich`).
- Shows without an event (rehearsals) belong to the account that made them; no key.
- **Deleting a show** (the user, 2026-11-21; in the desktop app when equievents.ch answers, and
  in the online version), its owner chooses: **keep the rights** (only that copy goes; the key
  brings the show back), **give the event to a collaborator** (they become its owner, he leaves
  it; they get an e-mail), or **release it**: the show is deleted from equievents.ch for everyone
  (its log, who worked on it, the requests; they get an e-mail), and the next to enter the key
  downloads the event fresh from Swiss Equestrian and owns it. A collaborator keeps their access
  or leaves. Offline, only that copy goes. The copy's last changes go up first. The online
  version keeps no copy of a show its account no longer works on.
- E-mails go in the recipient's language (the page they signed up on, or the app's).

## The site

Plain HTML forms (no script, nothing loaded from elsewhere), in French, German and Italian, for
phones too, in the app's look (the navy band with the logo, the language switch and the title,
the card below, Inter, light or dark as the system is; signing in and making an account as the
app's welcome screen): `crates/server/src/site.rs`.

**One language per page** (the user, 2026-11-21), the others a click away in the switch. The
home page is signing in, in the visitor's language: the one last chosen on the site (a page seen
in it, or signing out of the app in it: the `equievents_lang` cookie, a preference the online
version reads and writes too), else the first of the three the browser asks for, else German.
The online version starts in the same one until a language is chosen in it.

| French | German | Italian | |
|---|---|---|---|
| `/` | | | signing in, in the visitor's language; signed in: the online version |
| `/connexion` | `/anmelden` | `/accedi` | signing in (`?next=`: a page of the site to go back to); a year's cookie |
| `/deconnexion` | `/abmelden` | `/esci` | signing out (the session ends everywhere it was used: the online version) |
| `/inscription` | `/registrieren` | `/registrati` | name, e-mail, password: the account is made at once, signed in |
| `/mot-de-passe` | `/passwort` | `/password-dimenticata` | a forgotten password: the link by e-mail |
| `/nouveau-mot-de-passe?t=…` | `/neues-passwort?t=…` | `/nuova-password?t=…` | the link: a new password; signed out everywhere |
| `/telecharger` | `/herunterladen` | `/scarica` | the desktop app, Mac and Windows (this computer's first), signed in only (the online version's account dialog links to it): [releases.md](releases.md); the files themselves stay reachable for the apps' updates |
| `/app/` | | | the online version (above), signed in only |

What it says is the same for an address with or without an account (no list of who has one to
be had by asking), except when making an account with an address that has one ("forgot your
password?"). The pages pass no address on to other sites (the reset link) and cannot be framed.

## Abuse

No approval and no e-mail check in the way (an account alone opens nothing). Instead, limits in
memory per client address (Nginx Proxy Manager's `X-Real-IP`) and per account
(`crates/server/src/limit.rs`): signing in 30 per 15 minutes per address and 10 per account;
new accounts 10 an hour per address; reset links 10 an hour per address and 3 per account.
Beyond: 429, "try again later". The sign-up form has a hidden field robots fill: they are told
it worked, nothing is made.

## E-mail

As Gestion Concours sends its own (`crates/server/src/mail.rs`), with the same settings in
`.env`: `MAIL_MODE` `off` (nothing sent, the recipient and subject logged), `redirect` (all to
`MAIL_REDIRECT_TO`, a test server) or `send`; `SMTP_HOST`, `SMTP_PORT` (465: TLS, else
STARTTLS), `SMTP_USER`, `SMTP_PASS`, `SMTP_FROM`, `MAIL_FROM_NAME`, `MAIL_REPLY_TO`. Links point
to `APP_URL`. Sent: reset links; requests to a show's owner, their answers, access given
(`crates/server/src/notices.rs`). Until the mailbox (info@equievents.ch) and the domain's SPF / DKIM / DMARC exist,
`off`: a forgotten password then needs an administrator (« Comptes »: a new password) or
`set-password`.

Checked against a real Postgres (`crates/server/tests/api.rs`: making accounts from the apps and
the site, the limits, a forgotten password from the form to the new password; the key and the
owner, requests answered and taken back, transfers staying and leaving, people added and taken
off; rights, push, pull, signing out; a whole corpus show up and back, identical; staff:
support refused what only administrators do, acting as an account and back, a new password,
a reset link, roles, a show's owner changed and back, deleting an account and what goes with it).

## Backups

The worker runs `pg_dump --format=custom` every night (`BACKUP_AT`, local time) into the
`backups` volume, `equievents-YYYY-MM-DD-HHMM.dump`, the newest `BACKUP_KEEP` (14) kept; a
failure is logged and tried again the next night. `equievents-server backup` makes one now.
Copy them off the server as the other apps' dumps. Restore: `pg_restore --clean --no-owner -d
equievents <file>` into the database container. The `offices` volume holds the online
version's copies of shows that are in the database anyway (made again from it when lost), apart
from shows imported there and not put on equievents.ch.

## Deployment

The whole order, once (GitHub, the app's keys, the server, checks, the pilot):
[../deployment.md](../deployment.md). On the server, in a folder with this repository cloned
from GitHub (its `Dockerfile`, `docker-compose.yml` and sources):

```
cp .env.production.example .env        # set DB_PASSWORD (letters and digits); e-mail later;
                                       # RELEASES_REPO, RELEASE_TAG: the desktop app (releases.md);
                                       # SE_SERVER=production
docker compose build
docker compose up -d
docker compose exec web equievents-server create-admin "Prénom Nom" adresse@exemple.ch   # support
```

An update: `git pull && docker compose build && docker compose up -d` (a new release of the
desktop app: its tag in `RELEASE_TAG` first).

Nginx Proxy Manager: a proxy host `www.equievents.ch` (and `equievents.ch`) → `eq-web:3000`,
Let's Encrypt, websockets not needed. DNS: A/AAAA of the apex and `www` to the server.
Health: `https://www.equievents.ch/api/health`.

## The apps' sync

The desktop (`crates/office/src/sync.rs`, in the app: the badge in a show's bar, its panel, the
account in the library):

- **The account**: made or signed in from the app (or the site); nothing works without one (the
  app shows only signing in and making an account). The token kept with the access keys (the
  system keychain), never in a file of the app; the account stays signed in, also offline.
- **Putting a show online** with its event's key: made and owned, or someone owns it (ask them for
  access or for the show; the owner answers in the show's panel). A rehearsal goes up without the
  event's key (its event is real: it stays free for its show office).
- **One copy per show**: a show the server has is downloaded from there (the library lists those
  not on this computer), never merged with a copy imported here on its own: two imports of one
  event are two different logs (Rosson imports even reuse operation ids).
- **The sync**: what was made here goes up (batches of 1000), what others made comes down after
  the show's cursor; every 20 s with a show open, shortly after each change, every minute from the
  library. The show file keeps which operations the server has (they wait there while offline,
  across restarts) and its link (the server, the show's id there, the cursor): `show::Store`
  schema 2.
- **Clocks** (rule R5): the server refuses operations stamped more than 5 minutes in its future
  and gives its time; the app says when the computer's clock runs ahead (those changes wait).
- States shown: on this computer only, online (with changes waiting), offline, signed out, no
  longer on the show (the copy stays here; it can be detached), clock ahead, error.
- Checked end to end (`crates/server/tests/api.rs`, `two_laptops_share_a_show`): two laptops
  through a real server, offline then back, a transfer leaving.
- The server for development: `EQUIEVENTS_SERVER` or the library's settings (`set_server`); a
  local one with a stand-in Swiss Equestrian: `cargo run -p server --example se_stand_in`.

## Next

1. Sending to Swiss Equestrian, in a later version, when the user decides.
