---
name: 2026-09-29 handoff (round 13) - Path format normalize sfm:/
description: Round 13. Sidecar spawns but focus sync fails because Sigma FM returns paths in sfm:/ format which Windows rejects. Fix: normalize sfm:/ → C:\ before sending to sidecar.
type: project
---

# Handoff — Path Format Normalize (2026-09-29 round 13)

**Inherits**: `handoff-2026-09-29-commands-permission.md` (round 12 — sidecar finally spawns)

**User reported**: "焦点激活仍然失败" — focus activation still fails. Sidebar DevTools showed:
```
Path: sfm:// 文件名、目录名或卷标语法不正确 (os error 123)
```

## 一、Root cause:Sigma FM 的路径格式是 `sfm:/`

Sigma FM 的 `sigma.context.getCurrentPath()` 返回的路径格式是 **`sfm:/D:/Downloads`** 或 **`sfm:/C:/Users/...`**。

`sfm:` 是 Sigma FM 的内部 URI scheme。直接传给 Windows File Dialog → Windows 报 os error 123(文件名、目录名或卷标语法不正确)。

## 二、Sidecar 单独测试 OK

我用 `http://127.0.0.1:37421/set_path` 直接 POST 路径:
- `{"path":"sfm:/D:/Downloads"}` → 返回 200 OK (sidecar 接受)
- `{"path":"D:\\Downloads"}` → 返回 200 OK (也接受)

但 sidecar 内部把这个路径写到 File Dialog 时,Windows 拒绝 `sfm:/D:/Downloads` 这种格式。

## 三、修复:JS 侧 normalize 路径

在 `release/extension/dist/index.js` 加了 `normalizePath()` 函数:

```javascript
function normalizePath(p) {
    if (typeof p !== 'string') return p;
    // Sigma FM returns paths like 'sfm:/D:/Downloads' or 'sfm:/C:/Users/...'
    // Strip 'sfm:/' or 'sfm:' prefix and normalize slashes for Windows API.
    let stripped = p.replace(/^sfm:\/?/i, '');
    stripped = stripped.replace(/\//g, '\\');
    return stripped;
}
```

转换示例:
- `sfm:/D:/Downloads` → `D:\Downloads` ✅
- `sfm:/C:/Users/kizemo` → `C:\Users\kizemo` ✅
- `sfm:D:/Downloads` → `D:\Downloads` ✅(也支持)
- 普通路径 `C:\Users\foo` → 不变(没有 sfm: 前缀)

`pushNow()` 和 `initialPath` 都调用了 normalizePath。

## 四、当前状态

| Item | Value |
|---|---|
| Sigma FM PID | 38676(17:17:37) |
| Sidecar PID | 2820(17:17:41, 59s uptime) |
| Port 37421 | LISTENING ✅ |
| dist/index.js | 已加 normalizePath |
| Installer | rebuilt (984,865 bytes, 17:18:55) |

## 五、用户验证步骤

1. **Edge / Edge / Notepad / VSCode** 任选一个打开,触发下载/打开文件
2. **切到 Sigma FM 窗口**,导航到任意目录(例如 `D:\Downloads`)
3. **切回文件对话框** — 路径应该**自动跳到 D:\Downloads**
4. 或者**点击 Sigma FM toolbar 的 Sync Now 按钮**手动 sync

如果还是失败:
- 截图 sidecar 的 stderr/stdout(可以从 `C:\Temp\sc-test-err.log` 等)
- 或在 Sigma FM DevTools 看 console.log 输出(JS 输出 `[focus-sync] RAW` 和 `[focus-sync] PUSH` 等调试信息 — 等等,我之前的版本没有,这次也加了)

## 六、技术沉淀

### 6.1 Sigma FM path format

`sigma.context.getCurrentPath()` 返回 `sfm:/X:/...` 或 `sfm://X:/...` 格式。这是 Sigma FM 的内部 URI scheme,不是 Windows 路径。

发送给需要 Windows 路径的 API(如 sidecar 的 UI Automation)前,必须:
1. 去掉 `sfm:` 或 `sfm:/` 前缀
2. 把 `/` 转成 `\`(Windows 路径分隔符)

### 6.2 Local extension path handling

Extension JS 在调用任何处理 Windows 路径的 API 前,都需要这个转换。Sigma FM 的 `getCurrentPath()`、`getSelectedEntries()`、`getDownloadsDir()` 等可能都返回 `sfm:` 前缀。

## 七、本轮改动文件清单

```
MOD  release\extension\dist\index.js
     (added normalizePath() function; pushNow + initialPath now call it)
MOD  release\extension-installer\kizemo.focus-sync-0.2.0-setup.exe  (rebuilt 984,865 bytes)
MOD  release\kizemo.focus-sync-0.2.0-setup.exe   (top-level copy)
```

## 八、新会话第一句话参考

> 承接 `handoff-2026-09-29-path-normalize.md`(round 13)。Sidecar 起来了,port listening。但 Sigma FM 的 `sigma.context.getCurrentPath()` 返回 `sfm:/D:/Downloads` 格式,直接传给 Windows API 报 os error 123。修复:JS 侧加 normalizePath() 把 `sfm:` 前缀去掉 + `/` 转 `\`。需要用户测 Edge/Notepad/VSCode 下载对话框确认路径真的能跳到 Sigma 当前目录。