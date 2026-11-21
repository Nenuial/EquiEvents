#!/usr/bin/env node
// A release of the desktop app for this computer's system (docs/architecture/releases.md): built,
// its update signed with the app's update key, the Mac app signed with the Developer ID and
// notarized by Apple (Windows: not signed). Its files and the manifest (latest.json) go into
// release/, the folder equievents.ch serves (each system adds its own: build on the Mac, then on
// the Windows computer, into the same folder).
//
//   node scripts/release.mjs           the release (the version: app/src-tauri/tauri.conf.json)
//   node scripts/release.mjs --test    to try updates on this computer only: not signed by Apple,
//                                      a server over http accepted, into release-test/
//
// The update key: ~/.tauri/equievents.key (or TAURI_SIGNING_PRIVATE_KEY_PATH), its password asked
// (or TAURI_SIGNING_PRIVATE_KEY_PASSWORD). Mac: the Developer ID from the keychain (or
// APPLE_SIGNING_IDENTITY); notarization with the keychain item `equievents-notarization` (or
// Tauri's APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID, or APPLE_API_KEY / APPLE_API_ISSUER /
// APPLE_API_KEY_PATH).

import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const app = join(root, 'app');
const cli = join(app, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
const confPath = join(app, 'src-tauri', 'tauri.conf.json');
const args = process.argv.slice(2);
const test = args.includes('--test');
const out = join(root, test ? 'release-test' : 'release');
const NOTARY_ITEM = 'equievents-notarization';

function fail(message) {
  console.error(`\n✗ ${message}`);
  process.exit(1);
}

function run(command, commandArgs, options = {}) {
  const r = spawnSync(command, commandArgs, { encoding: 'utf8', ...options });
  if (r.error) throw r.error;
  return r;
}

/** A password typed without showing it. */
function askHidden(question) {
  if (!process.stdin.isTTY) fail(`${question.trim()} — no terminal to ask in: set TAURI_SIGNING_PRIVATE_KEY_PASSWORD.`);
  process.stdout.write(question);
  return new Promise(done => {
    const stdin = process.stdin;
    let text = '';
    const onData = chunk => {
      for (const c of chunk) {
        if (c === '\r' || c === '\n') {
          stdin.setRawMode(false);
          stdin.pause();
          stdin.off('data', onData);
          process.stdout.write('\n');
          return done(text);
        }
        if (c === '\u0003') process.exit(130);
        text = c === '\u007f' || c === '\b' ? text.slice(0, -1) : text + c;
      }
    };
    stdin.setRawMode(true);
    stdin.setEncoding('utf8');
    stdin.resume();
    stdin.on('data', onData);
  });
}

if (!existsSync(cli)) fail('The Tauri CLI is missing: run `npm ci` in app/ first.');
if (!['darwin', 'win32'].includes(process.platform)) fail('Releases are built on a Mac or on Windows.');
const confText = readFileSync(confPath, 'utf8');
const conf = JSON.parse(confText);
const version = conf.version;
console.log(`EquiEvents ${version} for ${process.platform === 'darwin' ? 'macOS' : 'Windows'}${test ? ' (test build)' : ''}`);
if (!test) {
  // A release is built from what is committed (it can be built again).
  const status = spawnSync('git', ['status', '--porcelain'], { cwd: root, encoding: 'utf8' });
  if (status.status === 0 && status.stdout.trim()) console.log('• Uncommitted changes: this release is not what is committed.');
}

// --- The update key: the one the apps know -----------------------------------------------------

const keyPath = process.env.TAURI_SIGNING_PRIVATE_KEY_PATH || join(homedir(), '.tauri', 'equievents.key');
if (!existsSync(keyPath) || !existsSync(`${keyPath}.pub`))
  fail(`No update key at ${keyPath}. Once, on the first computer (then copy both files to the other):\n  cd app && npx tauri signer generate -w ${join(homedir(), '.tauri', 'equievents.key')}\nKeep the password in your password manager: without key and password, installed apps get no more updates.`);
const pubkey = readFileSync(`${keyPath}.pub`, 'utf8').trim();
const known = conf.plugins?.updater?.pubkey ?? '';
if (!test) {
  if (!known) {
    // The first release: the apps learn the key (committed with the release).
    writeFileSync(confPath, confText.replace('"pubkey": ""', `"pubkey": "${pubkey}"`));
    if (JSON.parse(readFileSync(confPath, 'utf8')).plugins.updater.pubkey !== pubkey) fail('Could not write the public key into tauri.conf.json.');
    console.log('• The update key\'s public half is now in app/src-tauri/tauri.conf.json: commit it.');
  } else if (known !== pubkey) {
    fail(`${keyPath} is not the app's update key (tauri.conf.json has another public key): use the same key on every computer.`);
  }
}
let password = process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD;
if (password === undefined) password = await askHidden('Password of the update key: ');
{
  // The password checked now, not after a long build.
  const dir = mkdtempSync(join(tmpdir(), 'equievents-key-'));
  writeFileSync(join(dir, 'check'), 'check');
  const r = run(process.execPath, [cli, 'signer', 'sign', '-f', keyPath, join(dir, 'check')], { env: { ...process.env, TAURI_SIGNING_PRIVATE_KEY_PASSWORD: password } });
  rmSync(dir, { recursive: true, force: true });
  if (r.status !== 0) fail(`The update key could not sign (wrong password?):\n${r.stderr || r.stdout}`);
}

// --- The system's build ------------------------------------------------------------------------

const env = { ...process.env, TAURI_SIGNING_PRIVATE_KEY: readFileSync(keyPath, 'utf8'), TAURI_SIGNING_PRIVATE_KEY_PASSWORD: password };
delete env.TAURI_SIGNING_PRIVATE_KEY_PATH;
const config = { bundle: { createUpdaterArtifacts: true } };
if (test) config.plugins = { updater: { pubkey, dangerousInsecureTransportProtocol: true } };
let target = null;
let arch;
let bundles;
let platform;

if (process.platform === 'darwin') {
  platform = 'macos';
  bundles = 'app,dmg';
  const installed = run('rustup', ['target', 'list', '--installed']).stdout.split('\n');
  if (installed.includes('aarch64-apple-darwin') && installed.includes('x86_64-apple-darwin')) {
    target = 'universal-apple-darwin';
    arch = ['aarch64', 'x86_64'];
  } else {
    const host = process.arch === 'arm64' ? 'aarch64' : 'x86_64';
    target = `${host}-apple-darwin`;
    arch = [host];
    console.log(`• For Macs with ${host === 'aarch64' ? 'Apple chips' : 'Intel processors'} only. For every Mac, once: rustup target add ${host === 'aarch64' ? 'x86_64' : 'aarch64'}-apple-darwin`);
  }
  if (test) {
    delete env.APPLE_SIGNING_IDENTITY;
    for (const name of ['APPLE_ID', 'APPLE_PASSWORD', 'APPLE_TEAM_ID', 'APPLE_API_KEY', 'APPLE_API_ISSUER', 'APPLE_API_KEY_PATH']) delete env[name];
    console.log('• Not signed by Apple: for this computer only.');
  } else {
    if (!env.APPLE_SIGNING_IDENTITY) {
      const ids = [...run('security', ['find-identity', '-v', '-p', 'codesigning']).stdout.matchAll(/"(Developer ID Application: [^"]+)"/g)].map(m => m[1]);
      const unique = [...new Set(ids)];
      if (unique.length === 0) fail('No "Developer ID Application" certificate in the keychain: Xcode › Settings › Accounts › Manage Certificates › + › Developer ID Application.');
      if (unique.length > 1) fail(`Several Developer ID certificates: choose one with APPLE_SIGNING_IDENTITY:\n  ${unique.join('\n  ')}`);
      env.APPLE_SIGNING_IDENTITY = unique[0];
    }
    console.log(`• Signed by ${env.APPLE_SIGNING_IDENTITY}`);
    const hasApi = env.APPLE_API_KEY && env.APPLE_API_ISSUER && env.APPLE_API_KEY_PATH;
    if (!hasApi && !(env.APPLE_ID && env.APPLE_PASSWORD)) {
      // The app-specific password in the keychain, never in a file.
      const item = run('security', ['find-generic-password', '-s', NOTARY_ITEM]);
      const account = item.stdout.match(/"acct"<blob>="([^"]+)"/)?.[1];
      const secret = run('security', ['find-generic-password', '-s', NOTARY_ITEM, '-w']).stdout.trim();
      if (item.status !== 0 || !account || !secret)
        fail(`No notarization password. Once: make an app-specific password at account.apple.com (Sign-In and Security › App-Specific Passwords), then\n  security add-generic-password -s ${NOTARY_ITEM} -a <your Apple ID> -w\n(it asks for the password and keeps it in the keychain).`);
      env.APPLE_ID = account;
      env.APPLE_PASSWORD = secret;
    }
    if (!hasApi && !env.APPLE_TEAM_ID) {
      const team = env.APPLE_SIGNING_IDENTITY.match(/\(([A-Z0-9]{10})\)\s*$/)?.[1];
      if (!team) fail('No team ID: set APPLE_TEAM_ID.');
      env.APPLE_TEAM_ID = team;
    }
    console.log(`• Notarized by Apple (${hasApi ? 'API key' : env.APPLE_ID}): this takes a few minutes.`);
  }
} else {
  platform = 'windows';
  bundles = 'nsis';
  arch = ['x86_64'];
  if (process.arch !== 'x64') fail('Windows releases are built on a 64-bit Intel or AMD computer.');
  console.log('• Not signed for Windows: SmartScreen warns once when it is installed (see the download page).');
}

const buildArgs = [cli, 'build', '--bundles', bundles, '--config', JSON.stringify(config)];
if (target) buildArgs.push('--target', target);
console.log(`\n$ tauri ${buildArgs.slice(1).join(' ')}\n`);
const built = spawnSync(process.execPath, buildArgs, { cwd: app, env, stdio: 'inherit' });
if (built.status !== 0) fail('The build failed (above).');

// --- Into the folder for equievents.ch ---------------------------------------------------------

const bundleDir = join(root, 'target', ...(target ? [target] : []), 'release', 'bundle');
const pick = (dir, wanted) => {
  const found = existsSync(dir) ? readdirSync(dir).filter(wanted) : [];
  if (found.length !== 1) fail(`Expected one file in ${dir}, found: ${found.join(', ') || 'none'}`);
  return join(dir, found[0]);
};
let installer, update;
if (platform === 'macos') {
  installer = pick(join(bundleDir, 'dmg'), f => f.endsWith('.dmg') && f.includes(`_${version}_`));
  update = pick(join(bundleDir, 'macos'), f => f.endsWith('.app.tar.gz'));
} else {
  installer = pick(join(bundleDir, 'nsis'), f => f.endsWith('-setup.exe') && f.includes(`_${version}_`));
  update = installer;
}
const signature = readFileSync(`${update}.sig`, 'utf8').trim();
const signedFor = Buffer.from(signature, 'base64').toString('utf8').match(/version:([^\s]+)/)?.[1];
if (signedFor && signedFor !== version) fail(`The update is signed for ${signedFor}, not ${version}.`);

if (platform === 'macos' && !test) {
  const app = join(bundleDir, 'macos', 'EquiEvents.app');
  const assess = run('spctl', ['--assess', '--type', 'execute', '-vv', app]);
  const said = `${assess.stdout}${assess.stderr}`.trim();
  if (assess.status !== 0 || !/Notarized Developer ID/.test(said)) fail(`Gatekeeper does not accept the app:\n${said}`);
  console.log(`• Gatekeeper: ${said.split('\n').find(l => l.startsWith('source=')) ?? 'accepted'}`);
}

mkdirSync(out, { recursive: true });
const names =
  platform === 'macos'
    ? { installer: `EquiEvents-${version}-macos.dmg`, update: `EquiEvents-${version}-macos.app.tar.gz` }
    : { installer: `EquiEvents-${version}-windows-setup.exe`, update: `EquiEvents-${version}-windows-setup.exe` };
// This system's earlier files go; the other system's stay.
for (const f of readdirSync(out)) if (f.startsWith('EquiEvents-') && f.includes(`-${platform}`) && f !== names.installer && f !== names.update) rmSync(join(out, f));
copyFileSync(installer, join(out, names.installer));
if (update !== installer) copyFileSync(update, join(out, names.update));
const manifestPath = join(out, 'latest.json');
const manifest = existsSync(manifestPath) ? JSON.parse(readFileSync(manifestPath, 'utf8')) : {};
manifest[platform] = {
  version,
  date: new Date().toISOString(),
  installer: names.installer,
  installer_size: statSync(join(out, names.installer)).size,
  update: names.update,
  signature,
  arch,
};
writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

console.log(`\n✓ ${out}`);
for (const f of readdirSync(out).sort()) console.log(`    ${f}`);
console.log(
  test
    ? '\nFor this computer only: DOWNLOADS_DIR=release-test for a local server, its address in the library (EQUIEVENTS_SERVER).'
    : '\nNext: the same on the other system (into this folder), then its files as the GitHub release of the version (gh release create v<version> release/*), and RELEASE_TAG in the server\'s .env. Usually the tag does it all: .github/workflows/release.yml.',
);
