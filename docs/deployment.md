# Going live: equievents.ch and the desktop app

Status, 2026-11-21: everything below is built and tested on this computer; nothing is deployed
yet. equievents.ch is the master of every show ([architecture/data-and-sync.md](architecture/data-and-sync.md)):
the apps sync only with it, and only it talks to Swiss Equestrian, downloads only. What this
first version does: [architecture/server.md](architecture/server.md#what-this-first-version-does-the-user-2026-11-21).
The server is set up in
[architecture/server.md](architecture/server.md), the app's releases in
[architecture/releases.md](architecture/releases.md). This page is the order to do it in, once.

## What is ready

- **The server** (`docker compose`: Postgres, the web container, the worker): accounts made on
  the site or in the app (the app does nothing without one), shows and their sync, who owns a
  show (its key), nightly backups, the app's download page and updates.
- **Swiss Equestrian from the server, downloads only**: an event downloaded with its key
  (*Importer › Charger de Swiss Equestrian…* in the app), the key checked with Swiss Equestrian
  before anyone owns a show, the event's updates, the riders and horses (per account), the demo
  events. From Swiss Equestrian's production (`SE_SERVER=production`) from the first day.
- **Nothing is sent to Swiss Equestrian**: the server has no code that uploads. « Envoyer »
  goes to the server's simulation.
- **The online version** at `/app/` for signed-in people (the same interface, its office on the
  server, straight on the master); the download page also only signed in.
- **GitHub Actions**: `ci.yml` (every push: clippy, unit tests, the server's API against
  Postgres, the interface's build, the server's image) and `release.yml` (a tag `vX.Y.Z`: the
  Mac app signed and notarized, the Windows setup, published as a GitHub release; run by hand:
  the same builds, published nowhere).

What needs the golden corpus (real shows, personal data) only runs on this computer: it is never
in the repository.

## 1. The repository on GitHub

1. A new public repository, with a fresh history: one commit of the tree as it is, checked
   against the corpus and the Rosson exports (`node scripts/corpus-guard.mjs --tree`). The
   history before it (where tests once held real riders' and horses' names) stays in its
   private backups (`EquiEvents-git-backup-2026-11-19.git`, `…-2026-11-21.git`), never pushed.
2. The corpus guard on: `git config core.hooksPath .githooks` (it checks every commit and every
   push; without the corpus on a computer, only the file names).
3. Push `main`. CI runs; all three jobs should pass.

## 2. The app's update key (once, on the Mac)

1. `cd app && npx tauri signer generate -w ~/.tauri/equievents.key`, with a password kept in
   the password manager. **Keep the key and its password safe**: without them, installed apps
   get no more updates.
2. Its public half into `app/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`: the content
   of `~/.tauri/equievents.key.pub`), committed and pushed. The release workflow refuses to run
   without it.

## 3. The repository's secrets

Settings › Environments › `release`, its deployment branches and tags limited to the tag rule
`v*` and the branch `main` (the trial build); its secrets (the list is also at the top of
`release.yml`; only the build job, which names the environment, can read them):

| Secret | What |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | the update key's file, as text |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | its password |
| `APPLE_CERTIFICATE` | the Developer ID Application certificate with its key, as base64: made in Xcode › Settings › Accounts › (the team) › Manage Certificates › + › *Developer ID Application* (the account holder only); exported from Keychain Access › My Certificates › (it) › Export… as a .p12 with a password; `base64 -i DeveloperID.p12 \| pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | the password the .p12 was exported with |
| `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_P8` | notarization with an App Store Connect API key (recommended): appstoreconnect.apple.com › Users and Access › Integrations › App Store Connect API › Team Keys › + (access: Developer); the Key ID, the Issuer ID shown above the list, the .p8 file (downloadable once: `pbcopy < AuthKey_<ID>.p8`) |
| or `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | notarization with the Apple ID, an app-specific password (account.apple.com › Sign-In and Security) and the team ID (developer.apple.com › Membership) |

Keep the .p12 and the .p8 in the password manager, then delete the files.

Windows is not signed for the pilot (SmartScreen warns once; the download page says what to
click).

## 4. A trial build, then the first release

1. Actions › Release › *Run workflow*: both systems are built, signed and notarized, kept as
   the run's artifacts, published nowhere. Download them; the Mac app should open without a
   warning, the Windows setup install for the user.
2. The first release: the version in `tauri.conf.json` (`0.1.0`, or the next), committed, then
   `git tag v0.1.0 && git push origin v0.1.0`. The workflow publishes the GitHub release
   `v0.1.0` with `latest.json` and the installers.

## 5. The server

On the server, as the other projects (Docker Compose behind Nginx Proxy Manager):

1. DNS: A (and AAAA) of `equievents.ch` and `www.equievents.ch` to the server.
2. `git clone` the repository into its folder; `cp .env.production.example .env` and set:
   - `DB_PASSWORD` (letters and digits);
   - `RELEASES_REPO` (owner/repository) and `RELEASE_TAG` (`v0.1.0`);
   - `SE_SERVER=production`;
   - e-mail: `MAIL_MODE=off` for now (step 6).
3. `docker compose build && docker compose up -d`. The build compiles the server (a few minutes,
   about 2 GB of memory) and puts the release's installers in the image.
4. Nginx Proxy Manager: a proxy host `www.equievents.ch` and `equievents.ch` → `eq-web`, port
   3000, Let's Encrypt, Force SSL. No timeout to raise: what waits for Swiss Equestrian runs in
   the background, and the app asks how it goes.
5. The first administrator: `docker compose exec web equievents-server create-admin
   "Prénom Nom" adresse@exemple.ch` (the password is printed once). Others get their role in
   the online version (« Comptes ») or with `equievents-server set-role adresse@exemple.ch
   admin|support|none`.
6. Backups: the worker writes one every night into the `backups` volume (14 kept); copy them
   off the server as the other projects' dumps.

One web container: the jobs waiting for Swiss Equestrian and the simulation's log are in its
memory (a restart forgets them; the app asks again).

## 6. E-mail

Until the mailbox `info@equievents.ch` exists with the domain's SPF, DKIM and DMARC, `MAIL_MODE=off`
(a forgotten password then needs an administrator: « Comptes », a new password). Then `MAIL_MODE=redirect` with
`MAIL_REDIRECT_TO` to try it, then `send`. What the e-mails look like (HTML and text, light and
dark, the three languages), before any is sent: `docker compose exec web equievents-server
mail-preview /tmp/mails.html`, then `docker compose cp web:/tmp/mails.html .` and open it.

## 7. Checking it works

1. `https://www.equievents.ch/api/health` → `{"ok": true}`; the server's log says where the
   online version's offices are and that its interface is served (`/srv/app`).
2. `https://www.equievents.ch/api/swiss-equestrian` → `{"downloads": "production", "sends": "simulation"}`.
3. `/telecharger` and `/herunterladen`: the installers, this computer's first.
4. An account made on the site (signed in at once): *Ouvrir EquiEvents en ligne* opens
   `/app/`; then the app installed from the download page, signed in with the same account.
5. In the app: *Importer › Charger de Swiss Equestrian…* with the key of a **current** event
   (Swiss Equestrian refuses a past event's key): the show comes, its responsable is that
   account. A second account with the same key: asked to request access; the first answers.
6. In the show, *Swiss Equestrian*: *Mettre à jour*; the riders and horses (*Télécharger*),
   then on a second computer signed in with the same account, the same riders and horses;
   *Envoyer* the event: « Reçu par la simulation » shows what would have gone.
7. Settings › Exercice: Swiss Equestrian's demo event comes (through the server).
8. An update of the app: a release `v0.1.1`, `RELEASE_TAG=v0.1.1`, `docker compose build web &&
   docker compose up -d`; the app offers it in the library's bar.

## Later: sending to Swiss Equestrian

This version never sends: there is no code for it on the server, and a setting asking for it
(`SE_SEND`) stops the server at start. Sending start lists and results for real comes with a
later version, when the user decides (it was planned for the pilot, April 2027).
