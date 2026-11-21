# Releases of the desktop app

Status, 2026-11-14: the download page on equievents.ch, the installers served from a folder on
the server, the app's own updates (checked against the app's update key), and the script that
makes a release. Decided: the Mac app is signed with the Developer ID and notarized by Apple;
the Windows app is not signed for the pilot (SmartScreen warns once when it is installed; the
page says what to click). A Windows certificate later changes nothing else.

## How it fits

```
a tag vX.Y.Z pushed to GitHub ─> .github/workflows/release.yml
   Mac runner:      node scripts/release.mjs ─┐
   Windows runner:  node scripts/release.mjs ─┴─> the GitHub release vX.Y.Z
                                                   (latest.json, the installers, the updates)
                                          │  the server: RELEASE_TAG in .env, docker compose build
                                          │  (scripts/fetch-release.sh puts them in the image)
                                          ▼
equievents.ch   /telecharger, /herunterladen    the page: this computer's system first
                /downloads/<file>               the files latest.json names, nothing else
                /api/updates/<os>/<cpu>/<ver>   the app's question: the newer release, else 204
                                          ▲
the app         asks a moment after the library opens and from ⚙; a button in the library's
                bar when there is a newer version; installed only on a click, then it starts again
```

- **`latest.json`**, one entry per system, each built on its own computer:
  `{"macos": {version, date, installer, installer_size, update, signature, arch}, "windows": {…}}`.
  The server turns it into Tauri's format for the app (`crates/server/src/downloads.rs`), with
  the address from `APP_URL`.
- **The update key** (minisign, Tauri's `signer`): its public half is in the app
  (`app/src-tauri/tauri.conf.json`, `plugins.updater.pubkey`); an update signed with another key,
  or changed after signing, is refused. Each update's signature also carries its version
  (`tauri build` writes it): the app refuses an older release offered as a newer one
  (`requireSignedVersion`).
- **Which server**: the library's (equievents.ch, or `EQUIEVENTS_SERVER` / the settings in
  development). Development builds (`tauri dev`, debug) never update themselves.
- **Mac**: one app for Apple chips and Intel when both Rust targets are installed (else the
  computer's own; the page then says "Mac with Apple chip"), macOS 11 or newer. The update
  replaces the app where it is: in Applications (the page says to drag it there; an app opened
  from the disk image or the Downloads folder cannot replace itself).
- **Windows**: the NSIS setup, installed for the user (no administrator), Windows 10 and 11;
  the update is the setup again, run without questions (`passive`), and it starts the app.

## Once

In order with the server's own steps: [../deployment.md](../deployment.md). On GitHub: the secrets of the environment `release` (Settings › Environments; its deployment
tags `v*` and branch `main`, so no other branch's workflow reads them), listed at the top of
`.github/workflows/release.yml`: the update key and its password; the Developer ID
certificate (exported from the keychain as a .p12, then `base64 -i cert.p12 | pbcopy`) and its
password; for the notarization an App Store Connect API key (appstoreconnect.apple.com › Users
and Access › Integrations › App Store Connect API: the key's ID, the issuer ID, the .p8 file) or
the Apple ID with an app-specific password and the team ID. The update key's public half must be
committed in `tauri.conf.json` before the first release from GitHub (the workflow refuses
otherwise): the first `node scripts/release.mjs` on the Mac writes it, or paste
`~/.tauri/equievents.key.pub` into `plugins.updater.pubkey`.

To build on a computer instead (or to try updates, `--test`), on the Mac:

1. **The Developer ID certificate** in the keychain: Xcode › Settings › Accounts › (the team) ›
   Manage Certificates › + › *Developer ID Application*.
2. **Notarization**: an app-specific password (account.apple.com › Sign-In and Security ›
   App-Specific Passwords), kept in the keychain, never in a file:
   `security add-generic-password -s equievents-notarization -a <Apple ID> -w`
   (it asks for the password). The team ID comes from the certificate's name. Alternatively
   Tauri's own variables (`APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`, or an App Store Connect
   API key: `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_PATH`).
3. **The update key**: `cd app && npx tauri signer generate -w ~/.tauri/equievents.key`, with a
   password kept in the password manager. **Keep the key and its password safe**: without them,
   installed apps get no more updates (people would have to download the app again by hand).
   The first release writes its public half into `tauri.conf.json`: commit it.
4. For every Mac (Apple chips and Intel): `rustup target add x86_64-apple-darwin`.

On the Windows computer: Rust (rustup, the project's 1.84), Node, Visual Studio's C++ build
tools, `npm ci` in `app/`; the update key's two files in `%USERPROFILE%\.tauri\`. If the
project comes there through Dropbox, keep `target\` and `app\node_modules\` out of its sync
(`Set-Content -Path <folder> -Stream com.dropbox.ignored -Value 1`); `release\` stays in it, so
both systems fill the same folder.

On the server, in `.env`: `RELEASES_REPO` (owner/repository) and, for a private repository,
`GITHUB_TOKEN` (a fine-grained token with read-only access to the repository's contents; a build
secret, never in the image). The release is in the image (`/data/downloads`, the Dockerfile's
`downloads` stage): no folder to copy.

## Each release

1. The version in `app/src-tauri/tauri.conf.json` (`version`, e.g. `0.2.0`), committed and pushed.
2. The tag: `git tag v0.2.0 && git push origin v0.2.0`. The workflow checks the tag is the app's
   version, builds both systems (the notarization takes a few minutes; Gatekeeper's check as on
   the Mac) and publishes the GitHub release `v0.2.0` with `latest.json` and the files. Both
   systems or none: a failed build publishes nothing.
3. On the server: `RELEASE_TAG=v0.2.0` in `.env`, then `git pull && docker compose build web &&
   docker compose up -d`. The image fetches that release's `latest.json` and the files it names
   (`scripts/fetch-release.sh`); a tag without a release stops the build. Nothing else changes:
   the page and the apps read `latest.json` each time.

A release from a computer (`node scripts/release.mjs` on the Mac and on Windows, into `release/`)
works as before; to serve it, attach its files to a GitHub release (`gh release create`), as the
workflow does.

**Trying updates without a real release**: `node scripts/release.mjs --test` (into
`release-test/`, not signed by Apple, a server over http accepted, with any key: e.g. one made
with `npx tauri signer generate --ci -w /tmp/test.key`, `TAURI_SIGNING_PRIVATE_KEY_PATH` pointing
to it). Then a local server with `DOWNLOADS_DIR=release-test`, the app from
`target/<target>/release/bundle/macos/` started with `EQUIEVENTS_SERVER` set to it, and a higher
version in `release-test/latest.json`.

## Checked

`crates/server/tests/downloads.rs` (the page in both languages, this computer's system first; the
files, whole and in part; nothing else in the folder served; the update answer and its 204) and
`app/src-tauri/tests/updates.rs` (Tauri's updater against our server: the newer version found,
downloaded, its signature checked; another key, a changed file and an older release offered as
newer refused; on a Mac, the app replaced, in a stand-in folder).

Not checked here: a notarized build (it needs the certificate and the password above) and the
Windows setup and its update (a Windows computer) — the workflow's first run on GitHub does both.
`scripts/fetch-release.sh` was tried against a stand-in for GitHub's API (`GITHUB_API`): the
release's `latest.json` and the files it names, nothing else; a missing release stops the build.
