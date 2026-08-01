#!/usr/bin/env node
/**
 * Clipboard 环境诊断
 *
 * 输出本项目（Tauri 2 + Vue 3 + Rust + SQLite）关键组件的状态，并对异常状态
 * 给出修复建议。运行方式：
 *
 *   npm run doctor
 *
 * 退出码：0 = 所有关键组件正常；1 = 存在关键组件缺失或损坏。
 */

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readSync, statSync, closeSync, openSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";

const isWin = process.platform === "win32";
const failures = [];
const warnings = [];
const notes = [];

function run(cmd, args, options = {}) {
  try {
    const r = spawnSync(cmd, args, {
      encoding: "utf8",
      windowsHide: true,
      timeout: 20000,
      shell: options.shell ?? false,
    });
    if (r.error) return { error: r.error };
    return {
      status: r.status,
      stdout: (r.stdout ?? "").trim(),
      stderr: (r.stderr ?? "").trim(),
    };
  } catch (e) {
    return { error: e };
  }
}

function semverGte(a, b) {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    if ((pa[i] ?? 0) !== (pb[i] ?? 0)) return (pa[i] ?? 0) > (pb[i] ?? 0);
  }
  return true;
}

console.log("== Clipboard 环境诊断 ==");

// --- Node.js ---
{
  const v = process.versions.node;
  if (semverGte(v, "18.0.0")) {
    console.log(`  [OK] Node.js ${v} (需要 >= 18)`);
  } else {
    failures.push("Node.js 版本过低");
    console.log(`  [X] Node.js ${v} (需要 >= 18)`);
    console.log("      修复: 从 https://nodejs.org/ 安装 LTS 版本后重试");
  }

  // 通过 `npm run doctor` 启动时用 npm 自带的 CLI 入口（避免 .cmd 的 EINVAL 问题）。
  const npmCli = process.env.npm_execpath;
  const npm = npmCli && existsSync(npmCli)
    ? run(process.execPath, [npmCli, "--version"])
    : run(isWin ? "npm.cmd" : "npm", ["--version"], { shell: isWin });
  if (npm.error || npm.status !== 0) {
    failures.push("npm 不可用");
    console.log("  [X] npm 不可用");
    console.log("      修复: 重新安装 Node.js LTS（自带 npm），或检查 PATH");
  } else {
    console.log(`  [OK] npm ${npm.stdout}`);
  }
}

// --- Rust 工具链 ---
{
  const rustc = run("rustc", ["--version"]);
  const cargo = run("cargo", ["--version"]);
  if (rustc.error || rustc.status !== 0 || cargo.error || cargo.status !== 0) {
    failures.push("Rust 工具链缺失");
    console.log("  [X] Rust 工具链缺失（rustc / cargo）");
    console.log("      修复: 使用 rustup 安装 stable — https://rustup.rs/");
  } else {
    const version = (rustc.stdout.match(/\d+\.\d+\.\d+/) || ["?"])[0];
    console.log(`  [OK] ${rustc.stdout} (cargo ${(cargo.stdout.match(/\d+\.\d+\.\d+/) || ["?"])[0]})`);
    if (!semverGte(version, "1.70.0")) {
      warnings.push("rustc 过旧");
      console.log("  [!] rustc 低于 1.70，依赖可能无法编译");
      console.log("      建议: rustup update stable");
    }
  }
}

// --- Tauri CLI ---
{
  const tauriJs = path.join(process.cwd(), "node_modules", "@tauri-apps", "cli", "tauri.js");
  const bin = path.join(process.cwd(), "node_modules", ".bin", isWin ? "tauri.cmd" : "tauri");
  let t;
  if (existsSync(tauriJs)) {
    t = run(process.execPath, [tauriJs, "--version"]);
  } else if (existsSync(bin)) {
    t = run(bin, ["--version"], { shell: isWin });
  } else {
    warnings.push("Tauri CLI 未安装");
    console.log("  [!] Tauri CLI 未安装（node_modules/.bin/tauri 不存在）");
    console.log("      修复: npm install");
    t = null;
  }
  if (t) {
    if (t.error || t.status !== 0) {
      warnings.push("Tauri CLI 无法运行");
      console.log(`  [!] Tauri CLI 无法运行: ${t.error?.message ?? t.stderr}`);
      console.log("      建议: 重新执行 npm install");
    } else {
      console.log(`  [OK] ${t.stdout.split(/\r?\n/)[0]}`);
    }
  }
}

// --- WebView2 Runtime ---
{
  if (!isWin) {
    notes.push("WebView2 检查跳过（非 Windows）");
    console.log("  [-] WebView2: 非 Windows 平台，跳过");
  } else {
    const SDK_GUID = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    const regKeys = [
      `HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\${SDK_GUID}`,
      `HKLM\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\${SDK_GUID}`,
      `HKCU\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\${SDK_GUID}`,
    ];
    let version = null;
    for (const key of regKeys) {
      const r = run("reg", ["query", key, "/v", "pv"]);
      if (r.status === 0) {
        const m = r.stdout.match(/\d+\.\d+(\.\d+)*/);
        if (m) version = m[0];
        if (version) break;
      }
    }
    if (!version) {
      for (const dir of [
        "C:\\Program Files (x86)\\Microsoft\\EdgeWebView\\Application",
        "C:\\Program Files\\Microsoft\\EdgeWebView\\Application",
      ]) {
        if (!existsSync(dir)) continue;
        const versions = readdirSync(dir).filter((d) => /^\d+\.\d+/.test(d));
        if (versions.length) {
          version = versions.sort(semverGte).pop();
          break;
        }
      }
    }
    if (version) {
      console.log(`  [OK] WebView2 Runtime ${version}`);
    } else {
      failures.push("WebView2 Runtime 缺失");
      console.log("  [X] WebView2 Runtime 未检测到");
      console.log("      修复: 安装 Evergreen Runtime — https://developer.microsoft.com/microsoft-edge/webview2/");
      console.log("      说明: Windows 10/11 通常已内置；仅在精简系统/离线环境缺失");
    }
  }
}

// --- SQLite 数据库 ---
{
  const appData = process.env.LOCALAPPDATA || path.join(homedir(), "AppData", "Local");
  const dataDir = path.join(appData, "ClipVault");
  const dbPath = path.join(dataDir, "clipvault.db");

  if (!existsSync(dbPath)) {
    notes.push("数据库未初始化");
    console.log("  [-] SQLite 数据库尚未初始化（首次启动应用时自动创建）");
    console.log(`      路径: ${dbPath}`);
  } else {
    const size = statSync(dbPath).size;
    let headerOk = false;
    try {
      const fd = openSync(dbPath, "r");
      const buf = Buffer.alloc(16);
      readSync(fd, buf, 0, 16, 0);
      closeSync(fd);
      headerOk = buf.equals(Buffer.from("SQLite format 3\u0000"));
    } catch {
      headerOk = false;
    }

    if (!headerOk) {
      failures.push("SQLite 数据库损坏");
      console.log("  [X] SQLite 数据库文件头无效，疑似损坏");
      console.log(`      路径: ${dbPath}`);
      console.log("      修复: 退出应用后备份该文件，再从「设置 → 数据」导入的备份恢复，或删除后重新开始");
    } else {
      console.log(`  [OK] SQLite 数据库存在（${(size / 1024).toFixed(1)} KB），文件头校验通过`);
      console.log(`      路径: ${dbPath}`);
      for (const suffix of ["-wal", "-shm"]) {
        if (existsSync(dbPath + suffix)) notes.push(`WAL 文件存在: clipvault.db${suffix}`);
      }

      const sqlite3 = run("sqlite3", [dbPath, "PRAGMA integrity_check;"]);
      if (sqlite3.error) {
        notes.push("sqlite3 CLI 不可用");
        console.log("  [-] sqlite3 CLI 未安装，跳过完整 integrity_check（文件头校验已通过）");
        console.log("      提示: 安装 SQLite CLI 后可获得更完整的校验");
      } else if (sqlite3.status === 0 && sqlite3.stdout.trim() === "ok") {
        console.log("  [OK] SQLite PRAGMA integrity_check = ok");
      } else {
        failures.push("SQLite 完整性校验失败");
        console.log("  [X] SQLite PRAGMA integrity_check 未通过:");
        console.log(`      ${sqlite3.stdout || sqlite3.stderr}`);
        console.log("      修复: 退出应用，使用 sqlite3 的 .recover 导出并重建数据库，或从备份恢复");
      }
    }
  }

  const mediaDir = path.join(dataDir, "media");
  if (existsSync(mediaDir)) {
    console.log(`  [OK] 媒体目录存在: ${mediaDir}`);
  } else {
    notes.push("媒体目录未创建");
    console.log("  [-] 媒体目录尚未创建（首次保存图片时自动创建）");
  }
}

// --- 汇总 ---
console.log("");
if (failures.length) {
  console.log(`发现 ${failures.length} 个问题: ${failures.join("、")}`);
  console.log("请按上方 [X] 后的修复建议处理后重新运行 npm run doctor。");
} else {
  console.log("所有关键组件正常。");
  if (warnings.length) console.log(`提示（${warnings.length}）: ${warnings.join("、")}`);
  if (notes.length) console.log(`备注: ${notes.join("；")}`);
}

process.exit(failures.length ? 1 : 0);
