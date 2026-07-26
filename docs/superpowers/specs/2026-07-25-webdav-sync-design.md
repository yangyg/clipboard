# WebDAV 同步设计

日期：2026-07-25  
状态：已实现（P0 手动整包 + P1 manifest 增量）

## 目标

多台 Windows PC 通过用户自备 WebDAV（坚果云 / Nextcloud / 群晖等）备份式同步剪贴板历史：本地仍只读写 SQLite + `media/`，远端只存约定好的同步包。

## 非目标

- 不托管账号服务、不做实时 CRDT
- 不同步 `clipvault.db` / WAL（禁止盲同步数据库文件）
- P0/P1 不做墓碑删除同步、不做定时调度 UI（P2）
- 不同步应用设置（主题、快捷键等）

## 远端目录约定

根路径由设置 `webdav_remote_path` 指定（默认 `ClipVaultSync`），相对 WebDAV 根 URL：

```
{remote_path}/
  manifest.json
  records/bundle.jsonl
  media/{hash}.png
  media/thumbs/{hash}.jpg
```

### `manifest.json`

```json
{
  "version": 1,
  "protocol": "clipvault-webdav-v1",
  "updated_at": "ISO-8601",
  "device_id": "uuid",
  "entries": [
    {
      "hash": "sha256-hex",
      "updated_at": "ISO-8601",
      "has_media": true,
      "media_path": "media/{hash}.png",
      "thumb_path": "media/thumbs/{hash}.jpg",
      "content_type": "image"
    }
  ]
}
```

### `records/bundle.jsonl`

每行一条记录 JSON（字段同 `ClipboardRecord`；`id` 可忽略；不含 `media_abs` / `thumb_abs`）。

## 同步流程

1. **Pull**：GET `manifest.json` + `bundle.jsonl`；对 `has_media` 且本地缺失的对象 GET 落盘  
2. **Merge**：按 `hash` 插入缺失行；已存在则浅合并（见下）  
3. **Push**：上传本地相对远端缺失的 media（HEAD/GET 探测，已存在则跳过）；合并 entry 集合后 PUT `bundle.jsonl` + `manifest.json`  
4. **调度（P0/P1）**：仅设置页「测试连接 / 拉取合并 / 推送 / 立即同步」；不阻塞剪贴板捕获线程

## 冲突与范围

| 规则 | 约定 |
|------|------|
| 同 hash | 不插入重复行 |
| `updated_at` | 取较新 |
| `is_favorite` / `is_pinned` | OR（任一端为真则保留） |
| `copy_count` | 取较大值 |
| 删除 | P0/P1：只增合并，删除不同步；远端独有条目在 push 时保留在 manifest/bundle |
| 敏感 | 默认不同步（`webdav_sync_sensitive = false`） |
| 设置 | 不同步 |
| 凭证 | 存本机 settings（不进 JSON 导出）；仅本机使用 |

## 设置字段

- `webdav_url` / `webdav_username` / `webdav_password`
- `webdav_remote_path`（默认 `ClipVaultSync`）
- `webdav_sync_sensitive`（默认 `false`）
- `webdav_device_id`（首次生成 UUID）
- `webdav_last_sync_at`（成功同步后更新）

## 分期

- **P0**：凭证配置 + 手动 pull / push / sync（整包 bundle + media）
- **P1**：manifest 条目对比增量；media 按 hash 跳过已存在对象
- **P2**（未做）：墓碑删除、定时同步、冲突报告 UI
