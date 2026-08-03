#!/usr/bin/env node
/**
 * IPC Contract Validation Script
 * 
 * Parses Rust command sources (src-tauri/src/commands/ directory, or the
 * legacy single commands.rs) to extract #[tauri::command] signatures,
 * then compares against TypeScript contract definition.
 * 
 * Exit codes:
 *   0 - All commands match
 *   1 - Mismatches found
 *   2 - Parse error
 */

import { existsSync, readdirSync, readFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));

const COMMANDS_DIR = 'src-tauri/src/commands';
const COMMANDS_RS = 'src-tauri/src/commands.rs';
const CONTRACT_SPEC = 'src/stores/invoke-contract.spec.ts';

// Tauri-injected params that don't appear in frontend invoke calls
const TAURI_INTERNAL_PARAMS = new Set(['state', 'app']);

/**
 * Parse Rust commands.rs to extract command signatures
 */
function parseRustCommands(content) {
  const commands = new Map();
  
  // Match #[tauri::command(...)] followed by pub async fn
  const commandRegex = /#\[tauri::command(?:\(([^)]*)\))?\]\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*\(([^)]*)\)/g;
  
  let match;
  while ((match = commandRegex.exec(content)) !== null) {
    const attrs = match[1] || '';
    const fnName = match[2];
    const paramsStr = match[3];
    
    // Parse rename_all attribute
    const renameAll = attrs.match(/rename_all\s*=\s*"(\w+)"/)?.[1];
    
    // Determine command name (may be renamed)
    let cmdName = fnName;
    if (renameAll === 'snake_case') {
      cmdName = toSnakeCase(fnName);
    }
    
    // Parse parameters
    const rawParams = parseParams(paramsStr);
    // Tauri v2 default: without rename_all, frontend sends camelCase params
    const params = renameAll === 'snake_case'
      ? rawParams
      : rawParams.map(toCamelCase);
    
    commands.set(cmdName, {
      fnName,
      params,
      renameAll
    });
  }
  
  return commands;
}

/**
 * Parse parameter list, filtering out Tauri internal params.
 * Handles Rust generics with nested angle brackets like State<'_, AppState>.
 */
function parseParams(paramsStr) {
  const params = [];
  // Split respecting angle-bracket nesting
  const parts = splitTopLevel(paramsStr, ',');

  for (const rawPart of parts) {
    const part = rawPart.trim();
    if (!part) continue;

    // Strip `mut ` prefix (Rust mutability keyword)
    const stripped = part.replace(/^mut\s+/, '');

    // Match param name (before the first top-level colon)
    const paramMatch = stripped.match(/^(\w+)\s*:/);
    if (!paramMatch) continue;

    const paramName = paramMatch[1];

    // Skip Tauri-injected params
    if (TAURI_INTERNAL_PARAMS.has(paramName)) continue;

    params.push(paramName);
  }

  return params;
}

/**
 * Split a string by a delimiter, respecting angle-bracket nesting.
 * E.g. "State<'_, AppState>, id: i64" splits into two parts, not three.
 */
function splitTopLevel(str, delim) {
  const result = [];
  let depth = 0;
  let current = '';
  for (const ch of str) {
    if (ch === '<' || ch === '(') depth++;
    else if (ch === '>' || ch === ')') depth--;
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
  return str.replace(/[A-Z]/g, letter => `_${letter.toLowerCase()}`);
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
      .map(s => s.trim().replace(/['"]/g, ''))
      .filter(Boolean);
    
    contracts.set(cmdName, { params });
  }
  
  return contracts;
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
        message: `Rust command "${cmdName}" not found in TypeScript contract`
      });
      continue;
    }
    
    const tsDef = tsContracts.get(cmdName);
    
    // Compare parameter lists
    const rustParams = new Set(rustDef.params);
    const tsParams = new Set(tsDef.params);
    
    // Check for missing params in TypeScript
    for (const param of rustParams) {
      if (!tsParams.has(param)) {
        errors.push({
          type: 'PARAM_MISSING_IN_TS',
          command: cmdName,
          param,
          message: `Command "${cmdName}": param "${param}" missing in TypeScript contract`
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
          message: `Command "${cmdName}": param "${param}" not in Rust signature`
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
        message: `TypeScript contract references command "${cmdName}" not found in Rust`
      });
    }
  }
  
  return { errors, warnings };
}

/**
 * Main validation logic
 */
function main() {
  try {
    const commandsDir = join(__dirname, '..', COMMANDS_DIR);
    const commandsFile = join(__dirname, '..', COMMANDS_RS);
    const contractPath = join(__dirname, '..', CONTRACT_SPEC);

    // Commands were split into src-tauri/src/commands/*.rs (module split);
    // fall back to the legacy single file.
    let rustContent;
    if (existsSync(commandsDir)) {
      const files = readdirSync(commandsDir).filter((f) => f.endsWith('.rs')).sort();
      rustContent = files
        .map((f) => readFileSync(join(commandsDir, f), 'utf-8'))
        .join('\n');
    } else {
      rustContent = readFileSync(commandsFile, 'utf-8');
    }
    const tsContent = readFileSync(contractPath, 'utf-8');
    
    const rustCommands = parseRustCommands(rustContent);
    const tsContracts = parseContractSpec(tsContent);
    
    console.log(`Found ${rustCommands.size} Rust commands`);
    console.log(`Found ${tsContracts.size} TypeScript contracts`);
    console.log('');
    
    const { errors, warnings } = validateContracts(rustCommands, tsContracts);
    
    // Report warnings
    if (warnings.length > 0) {
      console.log('⚠️  Warnings:');
      for (const w of warnings) {
        console.log(`  ${w.message}`);
      }
      console.log('');
    }
    
    // Report errors
    if (errors.length > 0) {
      console.log('❌ Errors:');
      for (const e of errors) {
        console.log(`  ${e.message}`);
      }
      console.log('');
      console.log(`Found ${errors.length} error(s)`);
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
