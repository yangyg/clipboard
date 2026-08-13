/**
 * Shared Rust/JS source-scanning helpers for the consistency-check scripts.
 * Both `check-ipc-contract.mjs` and (potentially) other scanners need to read
 * Rust sources while ignoring string literals, comments and nested delimiters;
 * keeping these in one place means a parsing bug is fixed once.
 */

/**
 * Mask string/template literals and comments with spaces (length-preserving,
 * newlines kept) so that regex scans below cannot match inside them. Applies
 * to both Rust sources (doc comments mention `#[tauri::command]`) and
 * frontend sources. Includes a light regex-literal heuristic so
 * `split(/['"]/g)`-style code doesn't desynchronise the scanner.
 */
export function maskSource(content, lang) {
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

/** Extract the contents of a balanced `open…close` pair starting at `openIdx`. */
export function extractBalanced(source, openIdx, open, close) {
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
 * Split a string by a delimiter, respecting angle-bracket and paren nesting.
 * E.g. "State<'_, AppState>, id: i64" splits into two parts, not three.
 */
export function splitTopLevel(str, delim) {
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
