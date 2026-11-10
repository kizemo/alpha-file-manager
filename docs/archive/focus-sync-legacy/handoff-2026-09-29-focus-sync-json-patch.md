---
name: 2026-09-29 handoff (round 6) - Focus Sync registered via JSON patch
description: Round 6. Manually added kizemo.focus-sync entry to user-extensions.json. Disk files were already in place from round 4. Edit verified, Sigma FM did not overwrite after 5s.
type: project
---

# Handoff — Focus Sync JSON Patch (2026-09-29 round 6)

**Inherits**: `handoff-2026-09-29-focus-sync-registration.md` (round 5 — root cause confirmed)
**Inherits chain**: round 4 POSTINSTALL hook fix → round 3 build success → round 2 smart upgrade → round 1 base build

**Status**: ✅ **REGISTERED** — kizemo.focus-sync entry added to `user-extensions.json`. Awaiting user verification after Sigma FM restart.

---

## 一、本轮动作摘要

**采用方案 A:手动 patch `user-extensions.json`**(handoff round 5 列出的快路径)。

### 1.1 备份

- 备份文件: `C:\Temp\user-extensions.json.bak-kizemo-20260929-132845` (9644 bytes,与原文件一致)
- 备份命令: `Copy-Item` from `%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json`

### 1.2 编辑

- 文件: `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-extensions.json`
- 旧大小: 9644 bytes
- 新大小: 13241 bytes (+3597 bytes for kizemo entry)
- LastWriteTime: 2026/9/29 13:29:17
- 方法: Edit 工具插入 hetima entry 之后、`installedExtensions` 闭合 `},` 之前

### 1.3 验证

```powershell
$json = Get-Content '...' -Raw | ConvertFrom-Json
# ✅ JSON 解析成功
# ✅ installedExtensions count = 2 (hetima + kizemo)
# ✅ kizemo entry 字段完整:autoUpdate=true, enabled=true, installPendingDependencies=false,
#    installedAt=1790659734628, version=0.2.0
# ✅ manifest.id=kizemo.focus-sync, name="Focus Sync", version=0.2.0
# ✅ engines.sigmaFileManager=^2.2.0, main=dist/index.js
# ✅ settings.customSettings.__binaries={}, scopedDirectories=[]
```

5 秒后再读:文件未变化,Sigma FM 没有自动重写。说明 Sigma FM 不持续监测此文件(只在 startup / UI 操作时读写)。

---

## 二、新插入的 kizemo entry 结构(完整)

字段顺序遵循 kizemo 的 package.json。**未包含**:
- `devDependencies` / `scripts`(运行时无关)
- `$schema`(原 manifest 没有,不强加)

**包含**:
- `id`, `name`, `version`, `description`, `publisher`, `repository`, `license`, `icon`
- `categories`, `tags`
- `extensionType`, `main`, `type`, `engines`, `permissions`, `activationEvents`
- `binaries`(包含 sidecar 完整 metadata + sha256 integrity)
- `contributes.commands` + `contributes.toolbar`
- `settings.customSettings.__binaries = {}`(匹配 hetima 模式)
- `settings.scopedDirectories = []`(匹配 hetima 模式)

**`__binaries: {}` 含义**: Sigma FM 内部用此 track extension 实际安装的 binary 版本/路径。空对象意味着"无 overrides,完全用 manifest.binaries 解析"。Sigma FM 启动时 reconcile 阶段会填充实际安装的 binary 路径。

---

## 三、Sigma FM 当前进程状态

```
Id               : 2480
ProcessName      : sigma-file-manager
Mem(MB)          : 78.5
Threads          : 65
Handles          : 718
StartTime        : 2026/9/29 13:08:45
MainWindowHandle : 14880992
```

**Sigma FM 自 13:08:45 启动至今未写 user-extensions.json**(我的 edit @ 13:29:17 仍在),说明:
- Sigma FM 启动时读一次
- 后续只在 UI 操作时才写回
- 编辑后用户**无需重启 Sigma FM**,只要在 UI 中触发 extensions 数据 reload 即可(打开 Ctrl+Shift+X 应该就触发)

**但保险起见,推荐重启 Sigma FM 后再验证**,以避免 in-memory cache 与磁盘不一致。

---

## 四、用户验证 checklist(本会话后用户做)

### 4.1 重启 Sigma FM(推荐)

```
右键任务栏 Sigma → 退出(或直接关窗口)
双击 D:\Program Files\Sigma File Manager\sigma-file-manager.exe 重启
```

### 4.2 打开 extensions 页面

`Ctrl+Shift+X` → "已安装" tab

### 4.3 期望看到

- **Focus Sync**(作者 kizemo,大小 96.x KB,version 0.2.0)
- toggle 启用按钮可用
- 启用后 toolbar 出现 Sync Now / Enable/Disable 按钮

### 4.4 功能验证

打开任意 Windows 应用的 "Save As" 对话框(Notepad / Chrome 下载对话框 / Word 等),在 Sigma FM 中切换目录,看对话框是否自动跳到当前目录(参考 `docs/superpowers/test-plans/2026-09-27-listary-focus-sync-manual-checklist.md`)。

---

## 五、技术沉淀(给未来会话)

### 5.1 Sigma FM extension 注册机制(确认)

- **数据源**: `%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json` 的 `installedExtensions` 字段
- **不依赖**: disk 扫描(Rust `get_installed_extensions` 只用于检测 broken extensions)
- **不要相信**: "复制 extension 文件到 `extensions/` 目录就自动注册"
- **真正注册路径**:
  1. 用户在 UI 操作(Ctrl+Shift+X → 本地安装)→ Sigma FM 调内部 install 逻辑 → 写 `user-extensions.json`
  2. Marketplace install → 同上
  3. **绕过 UI 的 patch**(本轮做法)→ 直接编辑 `user-extensions.json`

### 5.2 manifest entry 的最小字段集

`installedExtensions[id]`:
- `autoUpdate`: bool (默认 true)
- `enabled`: bool (默认 true)
- `installPendingDependencies`: bool (默认 false)
- `installedAt`: Unix ms timestamp
- `manifest`: 完整 manifest 对象(必须含 `id`, `name`, `version`, `engines`)
- `settings.customSettings.__binaries`: object (空 `{}` 也行)
- `settings.scopedDirectories`: array
- `version`: 当前安装的 version(可与 manifest.version 不同,如升级后)

### 5.3 与 Plan C(installer POSTINSTALL 自动注册)的关系

本轮方案 A 是**手动 workaround**。Plan C(installer hook 自动写 user-extensions.json)仍是更彻底的方案:

- 当前 POSTINSTALL hook(round 3)只复制 extension 文件,不改 storage
- Plan C 需要 hook 在 silent 模式下能写 JSON 到 `%APPDATA%\...\user-data\`
- NSIS 原生 JSON 支持弱,可调用 PowerShell 或 .exe 完成
- 留作 future work,优先级低于 "Sigma FM 内 UI 安装" 流程的稳定性验证

### 5.4 mss 截图工具当前不可用(本会话踩坑)

- 错误: `mss/factory.py: SyntaxError: source code string cannot contain null bytes`
- 影响: computer-use 的 screenshot 工具无法用
- 影响: 不能自动截图验证 Sigma FM UI 上的 Focus Sync 出现
- 建议: 未来会话先确认 mss 修复再依赖截图验证;或者让用户手动验证后回报

---

## 六、避坑(下次会话)

1. **不要走 PowerShell hashtable 构造 JSON** —— `__binaries` 等特殊键 + 嵌套结构用 PowerShell 容易踩坑。Edit 工具直接写 JSON 字符串更可控。
2. **不要覆盖原 hetima entry** —— 锚定 `"version": "0.1.1"` 唯一字符串,只在该字符串后插入 kizemo。
3. **不要假设 Sigma FM 会读磁盘 extension 目录** —— 那是 broken-extension 检测的输入,不是 UI 数据源。
4. **修改后请用户重启 Sigma FM** —— UI 缓存可能导致磁盘改了但 in-memory 还是旧值。
5. **测试 verbose 路径后,记得删 user-extensions.json 临时调试字段**(本轮未引入临时字段,跳过)。

---

## 七、本轮改动文件清单(都不在 git repo 内,无需 commit)

```
 C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-extensions.json
   (+3597 bytes for kizemo.focus-sync entry)
   backup: C:\Temp\user-extensions.json.bak-kizemo-20260929-132845 (9644 bytes)
```

无代码改动。无 build。无 installer 重建。本轮是纯运行时的 storage 注册 fix。

---

## 八、新会话第一句话参考

> 承接 `handoff-2026-09-29-focus-sync-json-patch.md`(round 6,继承 round 5 root cause discovery)。已手动 patch `user-extensions.json` 添加 kizemo.focus-sync entry,文件 13241 bytes,验证 JSON 解析 + 两个 installedExtension 都在 + Sigma FM 未覆盖。**下一步:用户重启 Sigma FM → Ctrl+Shift+X → "已安装" tab 应该看到 Focus Sync**。如果重启后仍看不到,需查 Sigma FM 的启动错误日志或 manifest 字段合规性。