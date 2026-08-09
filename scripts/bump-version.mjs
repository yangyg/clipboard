#!/usr/bin/env node
/**
 * bump-version.mjs — single source of truth version propagation
 *
 * package.json owns the app version; this script mirrors it into every
 * other app-owned version source:
 *
 *   - package-lock.json      (root + packages[""], only in arg mode;
 *                             npm version maintains these itself in hook mode)
 *   - src-tauri/tauri.conf.json
 *   - src-tauri/Cargo.toml   ([package] section)
 *   - src-tauri/Cargo.lock   (the `clipboard` package entry only)
 *   - src/locales/{zh-CN,en-US}.ts (about.version copy)
 *
 * Usage:
 *   node scripts/bump-version.mjs            # sync all targets to package.json's version
 *   node scripts/bump-version.mjs 1.2.3      # also rewrite package.json + package-lock.json
 *
 * Wired as the npm `version` lifecycle hook, so the recommended flow is:
 *   npm version patch|minor|major|x.y.z
 *
 * Every target is edited structurally (JSON keys / anchored patterns),
 * never with a blind global replace, so third-party dependencies that
 * happen to share the same version number are left untouched.
 *
 * Exit 0 = all targets synced, exit 1 = a target could not be located
 * (likely a refactor — re-anchor the patterns below).
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const SEMVER = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;

const read = (rel) => readFileSync(resolve(ROOT, rel), 'utf8');
const write = (rel, text) => writeFileSync(resolve(ROOT, rel), text);

function report(rel, from, to) {
  console.log(from === to ? `  = ${rel} (already ${to})` : `  ~ ${rel}: ${from} -> ${to}`);
}

function fail(msg) {
  console.error(`bump-version: ${msg}`);
  process.exit(1);
}

// --- 1. Source of truth ---------------------------------------------------
const pkg = JSON.parse(read('package.json'));
let version = process.argv[2];

if (version) {
  if (!SEMVER.test(version)) fail(`invalid version "${version}" (expected semver, e.g. 0.3.0)`);
  const from = pkg.version;
  pkg.version = version;
  write('package.json', JSON.stringify(pkg, null, 2) + '\n');
  report('package.json', from, version);

  const lock = JSON.parse(read('package-lock.json'));
  lock.version = version;
  if (lock.packages?.['']) lock.packages[''].version = version;
  write('package-lock.json', JSON.stringify(lock, null, 2) + '\n');
  report('package-lock.json', from, version);
} else {
  version = pkg.version;
}
console.log(`App version: ${version}`);

// --- 2. Tauri config --------------------------------------------------------
{
  const rel = 'src-tauri/tauri.conf.json';
  const conf = JSON.parse(read(rel));
  report(rel, conf.version, version);
  conf.version = version;
  write(rel, JSON.stringify(conf, null, 2) + '\n');
}

// --- 3. Cargo.toml — first `version = "..."` line belongs to [package] ------
{
  const rel = 'src-tauri/Cargo.toml';
  const text = read(rel);
  const m = text.match(/^version = "([^"]*)"/m);
  if (!m) fail(`could not locate [package] version in ${rel}`);
  report(rel, m[1], version);
  write(rel, text.replace(/^version = "[^"]*"/m, `version = "${version}"`));
}

// --- 4. Cargo.lock — only the app's own `clipboard` package entry ------------
{
  const rel = 'src-tauri/Cargo.lock';
  const text = read(rel);
  const m = text.match(/name = "clipboard"\nversion = "([^"]*)"/);
  if (!m) {
    console.log(`  ! ${rel}: no clipboard entry found; run \`cargo check\` to regenerate`);
  } else {
    report(rel, m[1], version);
    write(rel, text.replace(/(name = "clipboard"\nversion = ")[^"]*"/, `$1${version}"`));
  }
}

// --- 5. About-page copy — labelled patterns, never a bare version string ----
const locales = [
  ['src/locales/zh-CN.ts', /version: '版本 ([\d.]+)'/, (v) => `version: '版本 ${v}'`],
  ['src/locales/en-US.ts', /version: 'Version ([\d.]+)'/, (v) => `version: 'Version ${v}'`],
];
for (const [rel, re, render] of locales) {
  const text = read(rel);
  const m = text.match(re);
  if (!m) fail(`could not locate about.version in ${rel}`);
  report(rel, m[1], version);
  write(rel, text.replace(re, render(version)));
}

console.log('\nDone. Suggested checks: git diff, git grep <old-version>, npm run validate');
