#!/usr/bin/env node
/**
 * check-schema.mjs — SQLite schema compatibility checker
 *
 * Parses the Rust source (src-tauri/src/db/mod.rs + schema.rs + types.rs) and
 * verifies:
 *   1. SCHEMA_VERSION constant exists and is a positive integer.
 *   2. All columns in RECORD_COLS / RECORD_COLS_LIST exist in the
 *      CREATE TABLE IF NOT EXISTS records block.
 *   3. Every ALTER TABLE ADD COLUMN migration (literal statements or the
 *      dynamic MIGRATE_COLUMNS array) references a column that is also present
 *      in the CREATE TABLE block (drift detection).
 *   4. RECORD_COLS and RECORD_COLS_LIST have the same arity.
 *
 * Exit 0 = pass, exit 1 = schema drift detected, exit 2 = input files
 * missing/unreadable (not drift — likely a refactor/rename).
 * Run via: npm run check:schema
 */

import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DB_DIR = resolve(__dirname, '..', 'src-tauri', 'src', 'db');
const DB_MOD = resolve(DB_DIR, 'mod.rs');
const DB_SCHEMA = resolve(DB_DIR, 'schema.rs');
const DB_TYPES = resolve(DB_DIR, 'types.rs');

let exitCode = 0;
function fail(msg) {
  console.error(`  ✗ ${msg}`);
  exitCode = 1;
}
function pass(msg) {
  console.log(`  ✓ ${msg}`);
}

console.log('Schema compatibility check\n');

let src, schemaSrc, typesSrc;
try {
  src = readFileSync(DB_MOD, 'utf-8');
  schemaSrc = readFileSync(DB_SCHEMA, 'utf-8');
  typesSrc = readFileSync(DB_TYPES, 'utf-8');
} catch (e) {
  console.error(`  ✗ Cannot read schema source files: ${e.message}`);
  console.error('    (This is an environment/refactor problem, not schema drift.)');
  process.exit(2);
}
const schemaSources = `${src}\n${schemaSrc}`;

// ── 1. SCHEMA_VERSION ──────────────────────────────────────────────
const versionMatch = schemaSrc.match(/const\s+SCHEMA_VERSION\s*:\s*i64\s*=\s*(\d+)/);
if (!versionMatch) {
  fail('SCHEMA_VERSION constant not found');
} else {
  const v = Number(versionMatch[1]);
  if (v < 1) {
    fail(`SCHEMA_VERSION must be ≥ 1, got ${v}`);
  } else {
    pass(`SCHEMA_VERSION = ${v}`);
  }
}

// ── 2. Extract CREATE TABLE records columns ────────────────────────
const createTableMatch = schemaSources.match(
  /CREATE\s+TABLE\s+IF\s+NOT\s+EXISTS\s+records\s*\(([\s\S]*?)\);/
);
if (!createTableMatch) {
  fail('CREATE TABLE IF NOT EXISTS records block not found');
  console.error('\nSchema check FAILED');
  process.exit(1);
}

const createBlock = createTableMatch[1];
// Parse column definitions: lines starting with a column name (not CONSTRAINT, PRIMARY, FOREIGN, etc.)
const createCols = new Set();
for (const line of createBlock.split('\n')) {
  const trimmed = line.trim().replace(/,$/, '');
  if (!trimmed || trimmed.startsWith('--')) continue;
  // Skip table-level constraints
  if (/^(PRIMARY|FOREIGN|UNIQUE|CHECK|CONSTRAINT)/i.test(trimmed)) continue;
  const colMatch = trimmed.match(/^(\w+)\s+/);
  if (colMatch) {
    createCols.add(colMatch[1]);
  }
}
pass(`CREATE TABLE records defines ${createCols.size} columns`);

// ── 3. Extract ALTER TABLE ADD COLUMN migrations ───────────────────
// Literal ALTER statements (legacy layout) plus the dynamic MIGRATE_COLUMNS
// array in schema.rs (migrate_schema builds `ALTER TABLE records ADD COLUMN
// {name} {ddl}` from it).
const alterCols = [];
const alterRegex = /ALTER\s+TABLE\s+records\s+ADD\s+COLUMN\s+(\w+)/gi;
let m;
while ((m = alterRegex.exec(src)) !== null) {
  alterCols.push(m[1]);
}
const migrateBlock = schemaSrc.match(/MIGRATE_COLUMNS\s*:\s*&\[\(&str,\s*&str\)\]\s*=\s*&\[([\s\S]*?)\];/);
if (migrateBlock) {
  const pairRegex = /\(\s*"(\w+)"\s*,\s*"/g;
  let p;
  while ((p = pairRegex.exec(migrateBlock[1])) !== null) {
    alterCols.push(p[1]);
  }
}
pass(`Found ${alterCols.length} ALTER TABLE ADD COLUMN migration(s)`);

for (const col of alterCols) {
  if (!createCols.has(col)) {
    fail(
      `ALTER TABLE ADD COLUMN '${col}' exists but is NOT in the CREATE TABLE block. ` +
      `Add it to the CREATE TABLE to prevent schema drift.`
    );
  }
}
if (exitCode === 0) {
  pass('All ALTER TABLE columns are present in CREATE TABLE block');
}

// ── 4. RECORD_COLS / RECORD_COLS_LIST arity check ──────────────────
// Constants moved to db/types.rs during the module split; fall back to
// mod.rs for older layouts.
function extractConstString(source, name) {
  const match = source.match(
    new RegExp(`const\\s+${name}\\s*:\\s*&str\\s*=\\s*"([\\s\\S]*?)"`)
  );
  return match ? match[1] : null;
}
const recordCols =
  extractConstString(typesSrc, 'RECORD_COLS') ?? extractConstString(src, 'RECORD_COLS');
const recordColsList =
  extractConstString(typesSrc, 'RECORD_COLS_LIST') ??
  extractConstString(src, 'RECORD_COLS_LIST');

if (!recordCols || !recordColsList) {
  fail('RECORD_COLS or RECORD_COLS_LIST constant not found');
} else {
  const countCols = (s) => {
    // Remove SQL comments, then count top-level commas (skip commas inside parentheses)
    const cleaned = s.replace(/--.*$/gm, '').replace(/\n/g, ' ');
    let depth = 0, count = 1;
    for (const ch of cleaned) {
      if (ch === '(') depth++;
      else if (ch === ')') depth--;
      else if (ch === ',' && depth === 0) count++;
    }
    return count;
  };
  const fullArity = countCols(recordCols);
  const listArity = countCols(recordColsList);
  if (fullArity !== listArity) {
    fail(
      `RECORD_COLS has ${fullArity} columns but RECORD_COLS_LIST has ${listArity}. ` +
      `They must match 1:1 for map_record_row.`
    );
  } else {
    pass(`RECORD_COLS and RECORD_COLS_LIST both have ${fullArity} columns`);
  }

  // Verify all columns in RECORD_COLS exist in CREATE TABLE
  const extractColNames = (s) => {
    const cleaned = s.replace(/--.*$/gm, '').replace(/\n/g, ' ');
    // Split on top-level commas only — expressions like
    // "substr(content, 1, 400) as content" contain inner commas that a naive
    // split would turn into phantom columns.
    const parts = [];
    let depth = 0;
    let current = '';
    for (const ch of cleaned) {
      if (ch === '(') depth++;
      else if (ch === ')') depth--;
      if (ch === ',' && depth === 0) {
        parts.push(current);
        current = '';
      } else {
        current += ch;
      }
    }
    if (current.trim()) parts.push(current);
    return parts
      .map((part) => {
        // Handle "expr AS alias" → take the alias
        const asMatch = part.trim().match(/\bas\s+(\w+)\s*$/i);
        if (asMatch) return asMatch[1];
        // Otherwise take the first word (column name)
        const nameMatch = part.trim().match(/^(\w+)/);
        return nameMatch ? nameMatch[1] : part.trim();
      })
      .filter(Boolean);
  };

  const fullColNames = extractColNames(recordCols);
  for (const col of fullColNames) {
    // Skip SQL keywords/expressions that aren't raw column names.
    if (col.toLowerCase() === 'null') continue;
    if (col === 'id' || createCols.has(col)) continue;
    // Some RECORD_COLS entries are expressions like "substr(content, 1, 400) as content"
    // Already handled by the AS alias extraction above
    fail(`RECORD_COLS references column '${col}' which is NOT in the CREATE TABLE block`);
  }

  // Positional binding: map_record_row reads by index, so the *order* of
  // RECORD_COLS and RECORD_COLS_LIST must match exactly (same alias at each
  // position). A reorder with equal arity would otherwise silently mis-map.
  const listColNames = extractColNames(recordColsList);
  if (fullColNames.length !== listColNames.length) {
    fail(
      `RECORD_COLS (${fullColNames.length}) and RECORD_COLS_LIST (${listColNames.length}) ` +
      `resolve to different column-name counts`
    );
  } else {
    let mismatch = -1;
    for (let i = 0; i < fullColNames.length; i++) {
      if (fullColNames[i] !== listColNames[i]) {
        mismatch = i;
        break;
      }
    }
    if (mismatch >= 0) {
      fail(
        `Column #${mismatch + 1} order differs: RECORD_COLS '${fullColNames[mismatch]}' vs ` +
        `RECORD_COLS_LIST '${listColNames[mismatch]}' (map_record_row reads positionally)`
      );
    } else {
      pass(`RECORD_COLS and RECORD_COLS_LIST column order match (${fullColNames.length} columns)`);
    }
  }
}

// ── 5. Verify expected indexes exist in source ─────────────────────
const expectedIndexes = [
  'idx_records_updated_at',
  'idx_records_hash',
  'idx_records_content_type',
  'idx_records_is_favorite',
  'idx_records_trashed_updated',
  'idx_records_trashed_pinned_updated',
  'idx_records_hash_active',
  'idx_records_auto_expire',
  'idx_record_tags_tag_id',
];

for (const idx of expectedIndexes) {
if (!schemaSources.includes(idx)) {
    fail(`Expected index '${idx}' not found in source`);
  }
}
pass(`All ${expectedIndexes.length} expected indexes found in source`);

// ── Summary ────────────────────────────────────────────────────────
console.log('');
if (exitCode === 0) {
  console.log('Schema check passed ✓');
} else {
  console.error('Schema check FAILED ✗');
}
process.exit(exitCode);
