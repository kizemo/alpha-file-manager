# Task — Installer A 真实修复 (round 19d v2 — NSIS 变量 bug)

## Status: ✅ completed + verified

## 关键发现(NSIS 3.x bug)
- `$APPDATA` 在 elevated context 返回 `C:\ProgramData` (那应该是 `$PROGRAMDATA` 的值) — **BUG,不可信**
- `$USERPROFILE` 在 StrCpy RHS 不展开,字面量保留 — **不可用**
- `$PROFILE` (NSIS 2.x 老 synonym) **能展开** — 兜底用
- **PowerShell `$env:USERPROFILE` 是 fallback 唯一可靠来源** — admin context 下仍是当前用户

## 实施
- `hooks.nsh` Stage 0 fallback 改用 PowerShell `$env:USERPROFILE` one-liner 计算路径
- 最终兜底用 `$PROFILE\AppData\Roaming`
- POSTUNINSTALL 同步修复

## 验证
- Silent install 在此机(管理员,elevated) PASS
- Marker 显示 `RESOLVED=C:\Users\Duanyi\AppData\Roaming` ✓
- `extensions\kizemo.focus-sync\` 全部文件已落地 ✓
- `binaries\focus-sync-sidecar\` 二进制已落地 ✓
- Sigma FM 启动后 user-extensions.json 保留 entry ✓

## Build artifact (current working)
- `release\Sigma-File-Manager-2.2.0-focus-6-setup.exe`
- 16,946,584 bytes (16.16 MiB)
- SHA1: `6F7938EB6159513097E6FBB20985B664FC9CF386`
- Built: 2026-09-30 16:30:11

## 待用户验证 + 清理
- focus-6 是当前 working build
- focus-2/3/4/5 都是 BROKEN 中间产物,等用户验证通过后清理
- hooks.nsh 内的 debug FileWrite (POSTINSTALL_STARTED, RESOLVED=, INSTDIR=) 等用户验证后也要清掉

## Handoff
- `handoff-2026-09-30-installer-a-userprofile-fix.md`(本轮全过程 + 5 个 NSIS gotcha 警示)
- `prompt-2026-09-30-installer-a-userprofile-next.md`(下一步 prompt)

## Memory 已更新
- `memory/focus_sync_status_2026-09-29.md` 加 round 19d-final + 4 个 NSIS gotcha (decisions #7-#10)