#!/usr/bin/env node
/**
 * IPC Contract Validation Script
 *
 * Parses Rust command sources (src-tauri/src/commands/*.rs) to extract
 * #[tauri::command] signatures, then compares against the TypeScript contract
 * definition, the invoke_handler registration list in lib.rs, and every
 * literal `invoke(...)` payload on the frontend.
 *
 * Checks performed:
 *   1. Rust commands ↔ TS COMMAND_CONTRACTS param lists (both directions)
 *   2. Every Rust command registered in generate_handler! (and vice versa)
 *   3. Frontend literal payloads: no unknown keys, no missing required
 *      (non-Option) Rust params — Tauri silently ignores unknown keys, so
 *      this catches silent contract breakage
 *
 * Exit codes:
 *   0 - All commands match
 *   1 - Mismatches found
 *   2 - Parse error
 */

import { readdirSync, readFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));

const COMMANDS_DIR = 'src-tauri/src/commands';
const LIB_RS = 'src-tauri/src/lib.rs';
const CONTRACT_SPEC = 'src/stores/invoke-contract.spec.ts';
const FRONTEND_DIR = 'src';

// Tauri-injected params that don't appear in frontend invoke calls
const TAURI_INTERNAL_PARAMS = new Set(['state', 'app', 'window', 'webview']);
// Tauri-injected param types (by leading path segment)
const TAURI_INTERNAL_TYPES = /^(tauri::)?(State|AppHandle|Window|WebviewWindow|Webview)\b/;

/**
 * Parse Rust command sources to extract command signatures.
 * Uses balanced-paren scanning so param types containing parens
 * (e.g. `Option<(u32, u32)>` or fn pointers) don't truncate the list.
 */
function parseRustCommands(maskedContent, rawContent) {
  const commands = new Map();

  const attrRegex = /#\[tauri::command(?:\(([^)]*)\))?\]/g;
  let match;
  while ((match = attrRegex.exec(maskedContent)) !== null) {
    // Attr text from the RAW source — masking blanks out its string literals.
    const attrText = rawContent.slice(match.index, match.index + match[0].length);
    const attrs = attrText.match(/tauri::command(?:\((.*)\))?/)?.[1] || '';
    const rest = maskedContent.slice(match.index + match[0].length);

    // Allow intervening attributes/doc lines between the command attr and fn.
    const fnMatch = rest.match(
      /^\s*(?:#[^\r\n]*\r?\n\s*)*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*\(/
    );
    if (!fnMatch) {
      throw new Error(
        `#[tauri::command] at offset ${match.index} is not followed by a fn`
      );
    }

    const fnName = fnMatch[1];
    const openParen = match.index + match[0].length + fnMatch[0].length - 1;
    const paramsStr = extractBalanced(maskedContent, openParen, '(', ')');
    if (paramsStr === null) {
      throw new Error(`Unbalanced parameter list on command fn "${fnName}"`);
    }

    // Parse rename_all attribute
    const renameAll = attrs.match(/rename_all\s*=\s*"(\w+)"/)?.[1];

    // Determine command name (all current fns are snake_case, so this is a
    // no-op in practice; kept for parity with Tauri's naming behaviour).
    let cmdName = fnName;
    if (renameAll === 'snake_case') {
      cmdName = toSnakeCase(fnName);
    }

    const rawParams = parseParams(paramsStr);
    // Tauri v2 default: without rename_all, frontend sends camelCase params.
    // With rename_all = "snake_case" the Rust param names are used verbatim.
    const params = rawParams.map((p) => ({
      ...p,
      name: renameAll === 'snake_case' ? p.name : toCamelCase(p.name),
    }));

    commands.set(cmdName, { fnName, params, renameAll });
  }

  return commands;
}

/** Extract text between a balanced open/close pair starting at openIdx. */
function extractBalanced(source, openIdx, open, close) {
  let depth = 0;
  for (let i = openIdx; i < source.length; i++) {
    const ch = source[i];
    if (ch === open) depth++;
    else if (ch === close) {
      depth--;
      if (depth === 0) return source.slice(openIdx + 1, i);
    }
  }
  return null;
}

/**
 * Parse parameter list, filtering out Tauri-injected params by name AND by
 * injected types. Returns { name, optional } (optional = Option<T> type).
 */
function parseParams(paramsStr) {
  const params = [];
  // Split respecting angle-bracket/paren nesting
  const parts = splitTopLevel(paramsStr, ',');

  for (const rawPart of parts) {
    const part = rawPart.trim();
    if (!part) continue;

    // Strip `mut ` prefix (Rust mutability keyword)
    const stripped = part.replace(/^mut\s+/, '');

    // Match param name (before the first top-level colon)
    const paramMatch = stripped.match(/^(\w+)\s*:\s*/);
    if (!paramMatch) continue;

    const paramName = paramMatch[1];
    const paramType = stripped.slice(paramMatch[0].length);

    // Skip Tauri-injected params (by conventional name or injected type)
    if (TAURI_INTERNAL_PARAMS.has(paramName)) continue;
    if (TAURI_INTERNAL_TYPES.test(paramType)) continue;

    params.push({ name: paramName, optional: /^Option\s*</.test(paramType) });
  }

  return params;
}

/**
 * Split a string by a delimiter, respecting angle-bracket and paren nesting.
 * E.g. "State<'_, AppState>, id: i64" splits into two parts, not three.
 */
function splitTopLevel(str, delim) {
  const result = [];
  let depth = 0;
  let current = '';
  for (const ch of str) {
    if (ch === '<' || ch === '(' || ch === '[') depth++;
    else if (ch === '>' || ch === ')' || ch === ']') depth--;
    if (ch === delim && depth === 0) {
      result.push(current);
      current = '';
    } else {
      current += ch;
    }
  }
  if (current.trim()) result.push(current);
  return result;
}

/**
 * Convert snake_case to camelCase
 */
function toCamelCase(str) {
  return str.replace(/_([a-z])/g, (_, ch) => ch.toUpperCase());
}

/**
 * Convert camelCase to snake_case
 */
function toSnakeCase(str) {
  return str.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

/**
 * Parse lib.rs invoke_handler registration list. Returns null when the
 * generate_handler! macro cannot be located (reported as a parse error).
 */
function parseRegisteredHandlers(libContent) {
  const m = libContent.match(/generate_handler!\s*\[([\s\S]*?)\]/);
  if (!m) return null;
  return m[1]
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)
    .map((s) => s.split('::').pop());
}

/**
 * Parse TypeScript contract spec to extract COMMAND_CONTRACTS
 */
function parseContractSpec(content) {
  const contracts = new Map();

  // Match command definitions like: command_name: { params: [...] }
  const contractRegex = /(\w+):\s*\{\s*params:\s*\[([^\]]*)\]/g;

  let match;
  while ((match = contractRegex.exec(content)) !== null) {
    const cmdName = match[1];
    const paramsStr = match[2];

    // Parse param list
    const params = paramsStr
      .split(',')
      .map((s) => s.trim().replace(/['"]/g, ''))
      .filter(Boolean);

    contracts.set(cmdName, { params });
  }

  return contracts;
}

function collectRustFiles(dir) {
  const files = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectRustFiles(path));
    } else if (entry.name.endsWith('.rs')) {
      files.push(path);
    }
  }
  return files;
}

function collectSourceFiles(dir) {
  const files = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (!['assets', 'locales'].includes(entry.name)) files.push(...collectSourceFiles(path));
      continue;
    }
    if (/\.(ts|vue)$/.test(entry.name) && !/\.(spec|test)\.ts$/.test(entry.name)) {
      files.push(path);
    }
  }
  return files;
}

/**
 * Mask string/template literals and comments with spaces (length-preserving,
 * newlines kept) so that regex scans below cannot match inside them. Applies
 * to both Rust sources (doc comments mention `#[tauri::command]`) and
 * frontend sources. Includes a light regex-literal heuristic so
 * `split(/['"]/g)`-style code doesn't desynchronise the scanner.
 */
function maskSource(content, lang) {
  let out = '';
  let i = 0;
  const n = content.length;
  const lastSignificant = () => {
    for (let j = out.length - 1; j >= 0; j--) {
      if (!/\s/.test(out[j])) return out[j];
    }
    return '';
  };
  while (i < n) {
    const ch = content[i];
    if (ch === '/' && content[i + 1] === '/') {
      while (i < n && content[i] !== '\n') {
        out += ' ';
        i++;
      }
    } else if (ch === '/' && content[i + 1] === '*') {
      out += '  ';
      i += 2;
      while (i < n && !(content[i] === '*' && content[i + 1] === '/')) {
        out += content[i] === '\n' ? '\n' : ' ';
        i++;
      }
      if (i < n) {
        out += '  ';
        i += 2;
      }
    } else if (ch === '/' && '([{,=:+-*/%&|!<>?;~^'.includes(lastSignificant())) {
      // Probable regex literal: mask its body (bail at a newline).
      out += ch;
      i++;
      let inClass = false;
      while (i < n) {
        const c = content[i];
        if (c === '\\') {
          out += '  ';
          i += 2;
          continue;
        }
        if (c === '\n') break;
        if (c === '[') inClass = true;
        else if (c === ']') inClass = false;
        else if (c === '/' && !inClass) {
          out += c;
          i++;
          break;
        }
        out += ' ';
        i++;
      }
    } else if (ch === "'" || ch === '"' || ch === '`') {
      if (
        ch === "'" &&
        lang === 'rust' &&
        !/^'(?:\\.|[^'\\])'/.test(content.slice(i, i + 10))
      ) {
        // Rust lifetime ('static, '_) rather than a char literal — emit as-is.
        out += ch;
        i++;
        continue;
      }
      const quote = ch;
      out += ch;
      i++;
      while (i < n) {
        const c = content[i];
        if (c === '\\') {
          out += '  ';
          i += 2;
          continue;
        }
        if (c === quote) {
          out += c;
          i++;
          break;
        }
        if (c === '\n' && quote !== '`') {
          break; // unterminated single/double quote — bail out
        }
        out += c === '\n' ? '\n' : ' ';
        i++;
      }
    } else {
      out += ch;
      i++;
    }
  }
  return out;
}

/**
 * Scan frontend sources for invoke call sites.
 * Returns [{ command, file, keys }] where keys is:
 *   - Set<string> of top-level payload keys when the payload is a plain
 *     object literal (or absent → empty set),
 *   - null when the payload is dynamic (variable/spread) and uncheckable.
 */
function parseFrontendInvokes(files) {
  const sites = [];
  const invokeRegex = /\binvoke(?:<[^()]*>)?\s*\(\s*["']([^"']+)["']/g;
  for (const file of files) {
    const raw = readFileSync(file, 'utf8');
    const masked = maskSource(raw, 'js');
    let match;
    invokeRegex.lastIndex = 0;
    while ((match = invokeRegex.exec(masked)) !== null) {
      // Command name from the RAW source — masking blanks string contents.
      const rawSegment = raw.slice(match.index, match.index + match[0].length);
      const command = rawSegment.match(/["']([^"']+)["']/)[1];
      const after = masked.slice(match.index + match[0].length);
      const payloadMatch = after.match(/^\s*,\s*\{/);
      if (!payloadMatch) {
        // No payload argument (or payload built elsewhere) — zero keys.
        sites.push({ command, file, keys: new Set() });
        continue;
      }
      const braceIdx = match.index + match[0].length + payloadMatch[0].length - 1;
      sites.push({ command, file, keys: extractObjectKeys(masked, braceIdx) });
    }
  }
  return sites;
}

/**
 * Extract top-level keys of an object literal. Returns null when the literal
 * contains spread/computed entries (uncheckable statically).
 */
function extractObjectKeys(source, openBraceIdx) {
  let depth = 0;
  const segments = [];
  let current = '';
  let closed = false;
  for (let i = openBraceIdx; i < source.length; i++) {
    const ch = source[i];
    if (ch === '{' || ch === '(' || ch === '[') {
      depth++;
      if (depth > 1) current += ch;
    } else if (ch === '}' || ch === ')' || ch === ']') {
      depth--;
      if (depth === 0) {
        segments.push(current);
        closed = true;
        break;
      }
      current += ch;
    } else if (ch === ',' && depth === 1) {
      segments.push(current);
      current = '';
    } else if (depth >= 1) {
      current += ch;
    }
  }
  if (!closed) return null;

  const keys = new Set();
  for (const segRaw of segments) {
    const seg = segRaw.trim();
    if (!seg) continue;
    if (seg.startsWith('...') || seg.startsWith('[')) return null;
    const keyMatch = seg.match(/^["']?(\w+)["']?\s*:/) || seg.match(/^["']?(\w+)["']?$/);
    if (!keyMatch) return null;
    keys.add(keyMatch[1]);
  }
  return keys;
}

/**
 * Compare Rust commands against TypeScript contracts
 */
function validateContracts(rustCommands, tsContracts) {
  const errors = [];
  const warnings = [];

  // Check for missing commands in TypeScript
  for (const [cmdName, rustDef] of rustCommands) {
    if (!tsContracts.has(cmdName)) {
      warnings.push({
        type: 'MISSING_IN_TS',
        command: cmdName,
        message: `Rust command "${cmdName}" not found in TypeScript contract`,
      });
      continue;
    }

    const tsDef = tsContracts.get(cmdName);

    // Compare parameter lists
    const rustParams = new Set(rustDef.params.map((p) => p.name));
    const tsParams = new Set(tsDef.params);

    // Check for missing params in TypeScript
    for (const param of rustParams) {
      if (!tsParams.has(param)) {
        errors.push({
          type: 'PARAM_MISSING_IN_TS',
          command: cmdName,
          param,
          message: `Command "${cmdName}": param "${param}" missing in TypeScript contract`,
        });
      }
    }

    // Check for extra params in TypeScript
    for (const param of tsParams) {
      if (!rustParams.has(param)) {
        errors.push({
          type: 'PARAM_EXTRA_IN_TS',
          command: cmdName,
          param,
          message: `Command "${cmdName}": param "${param}" not in Rust signature`,
        });
      }
    }
  }

  // Check for commands in TypeScript that don't exist in Rust
  for (const [cmdName] of tsContracts) {
    if (!rustCommands.has(cmdName)) {
      errors.push({
        type: 'COMMAND_NOT_IN_RUST',
        command: cmdName,
        message: `TypeScript contract references command "${cmdName}" not found in Rust`,
      });
    }
  }

  return { errors, warnings };
}

/**
 * Cross-check parsed Rust commands against the generate_handler! list.
 */
function validateRegistration(rustCommands, registered) {
  const errors = [];
  const registeredSet = new Set(registered);
  const fnNames = new Set([...rustCommands.values()].map((c) => c.fnName));

  for (const [cmdName, def] of rustCommands) {
    if (!registeredSet.has(def.fnName)) {
      errors.push(`Command "${cmdName}" is not registered in generate_handler! (lib.rs)`);
    }
  }
  for (const name of registered) {
    if (!fnNames.has(name)) {
      errors.push(`generate_handler! entry "${name}" does not match any #[tauri::command] fn`);
    }
  }
  return errors;
}

/**
 * Validate literal invoke payloads against Rust signatures.
 * Tauri silently ignores unknown keys → extra keys are contract breakage;
 * missing non-Option params fail at runtime.
 */
function validatePayloadKeys(rustCommands, sites) {
  const errors = [];
  for (const site of sites) {
    const def = rustCommands.get(site.command);
    if (!def) continue; // unknown command is reported separately
    if (site.keys === null) continue; // dynamic payload — not statically checkable

    const valid = new Set(def.params.map((p) => p.name));
    for (const key of site.keys) {
      if (!valid.has(key)) {
        errors.push(
          `Payload key "${key}" sent to "${site.command}" in ${site.file} matches no Rust param (valid: ${[...valid].join(', ') || 'none'})`
        );
      }
    }
    for (const p of def.params) {
      if (!p.optional && !site.keys.has(p.name)) {
        errors.push(
          `Payload for "${site.command}" in ${site.file} is missing required param "${p.name}"`
        );
      }
    }
  }
  return errors;
}

/**
 * Main validation logic
 */
function main() {
  try {
    const commandsDir = join(__dirname, '..', COMMANDS_DIR);
    const contractPath = join(__dirname, '..', CONTRACT_SPEC);
    const libPath = join(__dirname, '..', LIB_RS);

    const files = collectRustFiles(commandsDir).sort();
    const rawParts = files.map((f) => readFileSync(f, 'utf-8'));
    const rustRaw = rawParts.join('\n');
    const rustContent = rawParts.map((c) => maskSource(c, 'rust')).join('\n');
    const tsContent = readFileSync(contractPath, 'utf-8');
    const libContent = readFileSync(libPath, 'utf-8');

    const rustCommands = parseRustCommands(rustContent, rustRaw);
    const tsContracts = parseContractSpec(tsContent);
    const registered = parseRegisteredHandlers(libContent);
    if (registered === null) {
      throw new Error('generate_handler! list not found in lib.rs');
    }
    const invokeSites = parseFrontendInvokes(
      collectSourceFiles(join(__dirname, '..', FRONTEND_DIR))
    );

    console.log(`Found ${rustCommands.size} Rust commands`);
    console.log(`Found ${tsContracts.size} TypeScript contracts`);
    console.log(`Found ${registered.length} registered handlers`);
    console.log(`Found ${invokeSites.length} frontend invoke call sites`);
    console.log('');

    const { errors: contractErrors, warnings } = validateContracts(rustCommands, tsContracts);
    const registrationErrors = validateRegistration(rustCommands, registered);
    const payloadErrors = validatePayloadKeys(rustCommands, invokeSites);
    const unknownCommandSites = invokeSites.filter((s) => !rustCommands.has(s.command));

    // Report warnings
    if (warnings.length > 0) {
      console.log('⚠️  Warnings:');
      for (const w of warnings) {
        console.log(`  ${w.message}`);
      }
      console.log('');
    }

    // Report errors
    const frontendErrors = [
      ...unknownCommandSites.map(
        (s) => `Frontend invoke "${s.command}" in ${s.file} is not a Rust command`
      ),
      ...payloadErrors,
    ];
    if (frontendErrors.length > 0) {
      console.log('❌ Frontend invoke errors:');
      for (const error of frontendErrors) console.log(`  ${error}`);
      console.log('');
    }
    if (registrationErrors.length > 0) {
      console.log('❌ Registration errors:');
      for (const error of registrationErrors) console.log(`  ${error}`);
      console.log('');
    }

    const totalErrors = contractErrors.length + frontendErrors.length + registrationErrors.length;
    if (totalErrors > 0) {
      console.log('❌ Errors:');
      for (const e of contractErrors) {
        console.log(`  ${e.message}`);
      }
      console.log('');
      console.log(`Found ${totalErrors} error(s)`);
      process.exit(1);
    }

    // Missing commands are also errors (contract must be complete)
    if (warnings.length > 0) {
      console.log(`❌ Found ${warnings.length} missing command(s) in TypeScript contract`);
      process.exit(1);
    }

    console.log('✅ All commands match!');
    process.exit(0);
  } catch (err) {
    console.error('❌ Parse error:', err.message);
    process.exit(2);
  }
}

main();
