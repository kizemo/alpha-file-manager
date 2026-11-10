---
name: 2026-09-29 handoff (round 5) - Focus Sync registration root cause
description: Round 5. Discovered Sigma FM "已安装" tab reads from user-extensions.json, NOT disk scan. Manual copy to extensions/ alone is insufficient.
type: project
---

# Handoff — Focus Sync Registration Root Cause (2026-09-29 round 5)

**Inherits**: `handoff-2026-09-29-postinstall-hook-fix.md` (round 4 — NSIS hook fix attempt + manual copy workaround)

**Status**: ❌ **Focus Sync STILL NOT visible** despite files in `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\`. Root cause identified: Sigma FM's "已安装" tab reads from `user-extensions.json`, NOT from disk scan.

---

## 一、真正根因(本轮确认)

### 1.1 Sigma FM 的 extension 注册机制

**关键文件**: `%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json`

```json
{
  "sharedBinaries": {},
  "installedExtensions": {
    "hetima.light-themes": {
      "autoUpdate": true,
      "enabled": true,
      "installPendingDependencies": false,
      "installedAt": 1790645220967,
      "manifest": { ...hetima manifest... },
      ...
    }
    // ❌ kizemo.focus-sync 不在这里
  }
}
```

### 1.2 Sigma FM "已安装" tab 的数据源

1. **Vue composable `use-extensions.ts:filteredExtensions`**: 从 `extensionsStore.availableExtensions`(marketplace 注册表)开始,合并 `installedExtensions` 中匹配的 extension。
2. **`installedExtensionsWithManifest` computed**: 直接从 `storageStore.extensionsData.installedExtensions` 取值(不依赖 marketplace)。
3. **`reconcileInstalledExtensions` (extensions.ts:1849-1867)**: 调 Rust `get_installed_extensions` 拿到 disk 上的 extensions,跟 storage 比对,如果 storage 里有但 disk 上没有 → 加入 `brokenExtensionIds`。

**结论**:`get_installed_extensions` 只用于**检测 broken**(storage 有但 disk 没有),**不会主动 sync disk-only extensions 到 storage**。

### 1.3 manual copy 失败的精确机制

我在 round 4 直接从 `D:\Program Files\Sigma File Manager\extensions\kizemo.focus-sync\` 复制到 `%APPDATA%\com.sigma-file-manager.app\extensions\`:
- ✅ Rust scanner 能读到 disk 上的 kizemo(任何时候调 `get_installed_extensions` 都会返回)
- ❌ 但 **storage 没注册**,UI 的 "已安装" tab 从 storage 读,**不显示**
- ❌ Sigma FM 启动时只 reconcile,不会自动把 disk-only extensions 加到 storage

### 1.4 engine 兼容性检查(已确认不是问题)

- Sigma runtime 的 `EXTENSION_API_VERSION` = **`1.11.0`**(`packages/api/package.json`)
- kizemo 要求 `engines.extensionApi: "^1.0.0"`(满足)
- kizemo 要求 `engines.sigmaFileManager: "^2.2.0"`(Sigma 当前 2.2.0,满足)
- kizemo `package.json` 通过 `assertValidManifestData` 全部 schema 检查

所以 engine + schema 都不是问题,纯粹是 **storage 注册缺失**。

---

## 二、真正的修复方案(用户可选)

### 方案 A:手动写 user-extensions.json(快但脆)

直接编辑 `%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json`,把 kizemo.focus-sync 添加到 `installedExtensions`,结构跟 hetima 一样。

**风险**:需要精确知道字段格式(enabled, installPendingDependencies, manifest 完整内容, etc.)。

### 方案 B:走 Sigma FM 内置 install 流程(干净)

1. 关闭 Sigma FM
2. 用 PowerShell 把 `D:\Program Files\Sigma File Manager\extensions\kizemo.focus-sync\` **打包成一个 .zip** (或者保留文件夹原状)
3. 启动 Sigma FM → Ctrl+Shift+X → "本地安装 extension"(从 folder/zip 安装)
4. Sigma FM 会自动:
   - 读 manifest
   - 验证 schema
   - 检查 engine compatibility
   - **写入 user-extensions.json**
   - 显示在 "已安装" tab

**优点**:走设计正确的路径,storage 注册完整。
**缺点**:依赖 UI 操作,需要用户参与。

### 方案 C:修 installer POSTINSTALL hook,自动注册

如果走 installer 自动部署方案,POSTINSTALL hook 需要**两个动作**:
1. 复制文件到 `extensions\` 目录(已经做了)
2. **直接调用 Rust 命令 `register_extension_from_path`** 或者**直接 patch `user-extensions.json`**(写 kizemo entry)

第 2 步是最干净的"installer 自动安装 extension"实现,但需要扩展 Sigma JS API 或 NSIS 调 .exe。

---

## 三、当前文件状态

### 已确认(Sigma 内)
- `D:\Program Files\Sigma File Manager\extensions\kizemo.focus-sync\` ✅ 完整(5 个文件)
- `D:\Program Files\Sigma File Manager\bin\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe` ✅ (2,927,616 B)
- `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\` ✅ 完整(我手动复制)
- `%APPDATA%\com.sigma-file-manager.app\binaries\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe` ✅ (2,927,616 B)

### 缺失(Sigma UI 看不到 kizemo 的根因)
- `%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json` 的 `installedExtensions` 字段**没有 kizemo.focus-sync entry**

---

## 四、本会话前几轮进展(完整链路)

| Round | 任务 | 结果 |
|---|---|---|
| 1 (build-fail → build-success) | Tauri NSIS 基础 build | ✅ 17.1 MB installer |
| 2 (smart-upgrade-cleanup) | PREINSTALL smart-upgrade 钩子 | ✅ 装到 D 盘旧版路径,但 hook layer 1/2/3 key 写错(Tauri 实际用 MANUPRODUCTKEY)|
| 3 (postinstall-hook-fix) | POSTINSTALL hook 改用 SetOutPath + CopyFiles,重建 installer | ✅ Build 成功,但 hook 在 runtime 仍未复制到 %APPDATA% |
| 4 (helper 手动复制) | PowerShell 手动复制 Focus Sync + sidecar 到 %APPDATA% | ✅ 文件就位 |
| 5 (本轮 — registration root cause) | Sigma FM 不显示 kizemo — 诊断到 user-extensions.json 缺失 | ✅ 根因确认 |

**Installer POSTINSTALL hook 没跑**仍然是个谜 — NSIS 编译产物里 hook 完整存在(`!ifmacrodef` 测试过 OK),但 runtime 没复制。可能用户没真正跑 round 3 installer(跑了 `release/` 顶层 round 2 旧版)。但现在**这个问题优先级降低** — 因为即使 hook 跑了,Sigma 也不识别,根因在 storage 注册缺失。

---

## 五、下一步必做

按优先级:
1. **高**:验证方案 B(走 Sigma FM 内置 install 流程)— 用户在 UI 里操作,确认 kizemo 出现在"已安装" tab 且能启用
2. **中**:研究方案 C 的可行性 — patch installer 让 POSTINSTALL 自动注册到 storage(避免每用户手动 install)
3. **低**:回看 POSTINSTALL hook runtime 没跑的根因(可能用户跑过 round 2 旧 installer)

---

## 六、避坑(下次会话必读)

1. **手动复制 extension 到 `%APPDATA%\com.sigma-file-manager.app\extensions\` 不够** — 必须也注册到 `user-data\user-extensions.json`
2. **不要用 `assertValidManifestData` 失败作为根因猜测** — 我检查过 kizemo 的 manifest 全部通过
3. **不要假设 `get_installed_extensions` 会自动 sync disk-only extensions** — 它只 reconcile,不会主动注册
4. **release/ 顶层 vs release/installer/ 的 installer 版本不一致**:
   - `release/Sigma-File-Manager-2.2.0-focus-1-setup.exe` = round 2 (17,096,151 bytes Sep 29 09:38)
   - `release/installer/Sigma-File-Manager-2.2.0-focus-1-setup.exe` = round 3 (17,097,468 bytes Sep 29 12:43)
   - 用户可能跑过 round 2 旧版

---

## 七、新会话第一句话参考(prompt-next-session.md)

```
承接 `handoff-2026-09-29-focus-sync-registration.md`(round 5,继承 round 4 的 POSTINSTALL hook fix + manual copy)。**真正根因**:Sigma FM "已安装" tab 完全依赖 `%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json` 的 `installedExtensions` 字段,不是 disk 扫描。下一步:用户用 Sigma FM 内置的本地 install 流程(Ctrl+Shift+X → 本地安装)注册 kizemo.focus-sync,然后看 "已安装" tab 出现。或者直接手动 patch user-extensions.json(参考 hetima.light-themes entry 格式)。
```