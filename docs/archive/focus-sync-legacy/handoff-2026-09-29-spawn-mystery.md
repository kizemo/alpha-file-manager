---
name: 2026-09-29 handoff (round 15) - Sidecar spawn mystery
description: Round 15. After rebuilding sidecar without click + clearing webview cache, sidecar no longer spawns from extension activate(). Need user DevTools console check to diagnose.
type: project
---

# Handoff — Sidecar Spawn Mystery (2026-09-29 round 15)

**Inherits**: `handoff-2026-09-29-no-click-sidecar.md` (round 14 — rebuilt no-click sidecar)

**User reported**: "无法激活下载路径同步了" — focus sync doesn't work anymore.

## 一、当前状态

| Item | Value |
|---|---|
| Sigma FM PID | 19636(20:24 启动) |
| Sidecar | NOT spawning |
| Port 37421 | NOT listening |
| dist/index.js (deployed) | probe version w/ console.log markers |
| Sidecar binary (deployed) | 2927616 bytes(原始版本,已备份位置) |
| user-extensions.json mtime | 20:25 (Sigma FM 写入) |
| Permissions | shell, commands, toolbar, notifications, http ✅ |
| isLocal | true ✅ |
| enabled | true ✅ |
| `__binaries['focus-sync-sidecar']` | populated ✅ |

## 二、Round 14 后变化

我:
1. ✅ 用 1.90 toolchain 重新编译 sidecar(去 click + fallback_confirm)
2. ✅ 部署到 `%APPDATA%\...\bin\focus-sync-sidecar\`
3. ✅ 重新编译 installer (1,010,744 bytes)
4. ✅ 升级 JS(加 normalizePath + console.log)

## 三、问题

我尝试各种重启都 spawn 不出 sidecar。但:
- **Binary 直接跑** ✅(run with --port 37424 → listening,POST /set_path 返回 200)
- **JS 简化到 minimal**(只 spawn + log)也不工作
- **WebView cache 清了** 也不工作
- **回滚到原始 binary** 也不工作
- **Permissions correct, isLocal true, __binaries populated**

## 四、需要用户做的

打开 Sigma FM DevTools console(Ctrl+Shift+I),过滤关键词 `focus-sync-probe`,看看哪些 log 出现了:

| Log | 含义 |
|---|---|
| `[focus-sync-probe] module loaded` | JS 模块被加载 |
| `[focus-sync-probe] activate START` | activate() 被调用 |
| `[focus-sync-probe] getPath=...` | getPath 返回了路径(null 或实际路径)|
| `[focus-sync-probe] sidecar started, taskId=...` | sidecar 已 spawn |
| `[focus-sync-probe] FAILED: ...` | 某个步骤抛错 |

最可能:
- 只看到 "module loaded" 但没 "activate START" → activate() 没被调用(Sigma FM 没加载 extension)
- 看到 "activate START" 但没 "getPath" → `sigma.binary.getPath` 卡住了或抛错
- 看到 "getPath=null" → getPath 返回 null,function early return
- 看到 "FAILED" → 把错误内容贴给我

## 五、本会话已尝试

| 操作 | 结果 |
|---|---|
| Rebuild sidecar (no click) | 编译成功,3,068,416 bytes |
| Deploy no-click binary | ✅ deployed |
| Rebuild installer | 1,010,744 bytes |
| Add normalizePath to JS | ✅ |
| Add debug console.log to JS | ✅ |
| Test sidecar direct | ✅ runs, listens 37421, accepts POST |
| Test minimal JS (only spawn) | ❌ sidecar doesn't spawn |
| Clear webview cache | ❌ sidecar doesn't spawn |
| Multiple Sigma FM restarts | ❌ sidecar doesn't spawn |
| Revert to original binary | ❌ sidecar doesn't spawn |

## 六、可能的原因(Sigma FM 层面)

1. **WebView2 模块缓存** — `extensions/.../dist/index.js` 已被 module loader 缓存。我清 `EBWebView\Default` 下的文件可能不够;`EBWebView\Default\Cache` 或 session storage 也可能缓存。
2. **Tauri appdata 锁定** — Sigma FM 可能锁了某个文件,我重启时缓存命中。
3. **用户 extensions 修改被打断** — Sigma FM 自己修改 user-extensions.json 后,可能跳过下一次 activate。
4. **Sigma FM 本身在加载 extension 时静默 fail** — 可能在 loadExtensionCode 阶段就 throw,不会到 activate。

## 七、需要尝试的下一步

1. **检查 DevTools console**(最关键)
2. 如果 module loaded 但 activate START 没出现 → 可能 Tauri 把 extension 当 broken 处理了(可能 user-extensions.json 的某个字段不符 schema)
3. 检查 Sigma FM 是不是有 addons/extensions 缓存目录:
   - `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\` — 已经检查
   - `C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\EBWebView\` — 已清
   - 其他:看 Sigma FM 实际加载路径是不是缓存了 module

## 八、本轮改动文件清单

```
MOD  release\extension\dist\index.js   (probe version with console.log)
MOD  C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\dist\index.js   (deployed)
MOD  C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe  (currently original 2927616 bytes)
DEL  C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\EBWebView\Default\Cache\* (cleared)
```

## 九、新会话第一句话参考

> 承接 `handoff-2026-09-29-spawn-mystery.md`(round 15)。rebuild sidecar 后,sidecar 不 spawn 即使所有 path 都对、binary 直接跑 OK。需要用户在 DevTools console 看 `[focus-sync-probe]` log 是到哪一步停的。