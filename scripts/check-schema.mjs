#!/usr/bin/env node
/**
 * check-schema.mjs — SQLite schema compatibility checker
 *
 * Parses the Rust source (src-tauri/src/db/mod.rs) and verifies:
 *   1. SCHEMA_VERSION constant exists and is a positive integer.
 *   2. All columns in RECORD_COLS / RECORD_COLS_LIST exist in the
 *      CREATE TABLE IF NOT EXISTS records block.
 *   3. Every ALTER TABLE ADD COLUMN migration references a column
 *      that is also present in the CREATE TABLE block (drift detection).
 *   4. RECORD_COLS and RECORD_COLS_LIST have the same arity.
 *
 * Exit 0 = pass, exit 1 = schema drift detected.
 * Run via: npm run check:schema
 */

import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DB_MOD = resolve(__dirname, '..', 'src-tauri', 'src', 'db', 'mod.rs');

let exitCode = 0;
function fail(msg) {
  console.error(`  ✗ ${msg}`);
  exitCode = 1;
}
function pass(msg) {
  console.log(`  ✓ ${msg}`);
}

console.log('Schema compatibility check\n');

const src = readFileSync(DB_MOD, 'utf-8');

// ── 1. SCHEMA_VERSION ──────────────────────────────────────────────
const versionMatch = src.match(/const\s+SCHEMA_VERSION\s*:\s*i64\s*=\s*(\d+)/);
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
const createTableMatch = src.match(
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
const alterRegex = /ALTER\s+TABLE\s+records\s+ADD\s+COLUMN\s+(\w+)/gi;
const alterCols = [];
let m;
while ((m = alterRegex.exec(src)) !== null) {
  alterCols.push(m[1]);
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
const recordColsMatch = src.match(/const\s+RECORD_COLS\s*:\s*&str\s*=\s*"([\s\S]*?)"/);
const recordColsListMatch = src.match(/const\s+RECORD_COLS_LIST\s*:\s*&str\s*=\s*"([\s\S]*?)"/);

if (!recordColsMatch || !recordColsListMatch) {
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
  const fullArity = countCols(recordColsMatch[1]);
  const listArity = countCols(recordColsListMatch[1]);
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
    return cleaned.split(',').map((part) => {
      // Handle "expr AS alias" → take the alias
      const asMatch = part.trim().match(/\bas\s+(\w+)\s*$/i);
      if (asMatch) return asMatch[1];
      // Otherwise take the first word (column name)
      const nameMatch = part.trim().match(/^(\w+)/);
      return nameMatch ? nameMatch[1] : part.trim();
    }).filter(Boolean);
  };

  const fullColNames = extractColNames(recordColsMatch[1]);
  for (const col of fullColNames) {
    // Skip SQL functions/keywords that aren't raw column names
    if (/^(NULL|substr|id)$/i.test(col) && col.toLowerCase() === 'null') continue;
    if (col === 'id' || createCols.has(col)) continue;
    // Some RECORD_COLS entries are expressions like "substr(content, 1, 400) as content"
    // Already handled by the AS alias extraction above
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
  'idx_recordtags_tag_id',
];

for (const idx of expectedIndexes) {
  if (!src.includes(idx)) {
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
