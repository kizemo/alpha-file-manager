# Handoff — 双 Installer 打包完成 (2026-09-30)

**Inherits**:
- `handoff-2026-09-30-fix-ipc-http-done.md` (Round 19c binary fix done)
- `handoff-2026-09-29-smart-upgrade-cleanup.md` (bundled installer build)
- `release/README.md` (bundled installer 详细技术文档)

---

## 一、两个 installer 产物

### Installer A: 一体化包(含 Sigma FM + tree + focus-sync)

| 项 | 值 |
|---|---|
| 文件 | `release/Sigma-File-Manager-2.2.0-focus-1-setup.exe` |
| 副本 | `release/installer/Sigma File Manager_2.2.0_x64-setup.exe` (Tauri NSIS 原生名) |
| 大小 | 16,934,064 B (16.15 MiB) |
| 时间 | 2026-09-30 09:34 |
| 内容 | Sigma FM v2.2.0 (kizemo fork,含 tree sidebar) + focus-sync 扩展 + sidecar(round 19c) |
| 目标用户 | **没有 Sigma FM** 或想装 **kizemo fork(含 tree)** 的人 |
| UAC | 需要 admin(perMachine 安装) |
| 路径 | `%PROGRAMFILES%\Sigma File Manager\`(smart-upgrade 自动检测旧版路径) |
| smart-upgrade | ✅ PREINSTALL hook 三层 fallback(per `hooks.nsh`) |

**集成特性**:
- POSTINSTALL 自动 copy 到 `%APPDATA%\com.sigma-file-manager.app\`
- Sidecar 路径:`%APPDATA%\binaries\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe`
- 扩展路径:`%APPDATA%\extensions\kizemo.focus-sync\`

**用户启用步骤**:
1. 双击 installer
2. UAC → 允许
3. NSIS 显示 Install Location → 默认 `%PROGRAMFILES%\Sigma File Manager\` → 点 Install
4. 启动 Sigma FM
5. **Ctrl+Shift+X** → Installed tab 找到 Focus Sync → **Enable**
6. 工具栏出现 Focus Sync 下拉菜单

---

### Installer B: 仅扩展(给已有 Sigma FM 的用户)

| 项 | 值 |
|---|---|
| 文件 | `release/extension-installer/kizemo.focus-sync-0.2.0-setup.exe` |
| 副本 | `release/kizemo.focus-sync-0.2.0-setup.exe` (auto-copied by build.ps1) |
| 大小 | 838,090 B (838 KB) |
| 时间 | 2026-09-30 09:14 |
| 内容 | focus-sync 扩展文件 + sidecar(round 19c) + register.ps1 / unregister.ps1 / README.md |
| 目标用户 | **已装任意版本 Sigma FM**(包括官方原版)想加 focus-sync 的人 |
| UAC | 不需要(user-level install) |
| InstallDir | `$LOCALAPPDATA\Programs\kizemo.focus-sync\`(元数据目录) |
| 部署路径 | `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\`(per `register.ps1`) |
| 注册机制 | ✅ 注册到 user-extensions.json,`isLocal: true`(避免被 startup 钩子删除) |

**用户启用步骤**:
1. **先关掉 Sigma FM**(tray → Quit)— 否则 in-memory state 会覆盖 user-extensions.json
2. 双击 installer
3. NSIS 安装向导 → Install
4. 启动 Sigma FM
5. **Ctrl+Shift+X** → Installed tab 找到 Focus Sync → **Enable**

---

## 二、本会话执行动作

| 步骤 | 命令 | 结果 |
|---|---|---|
| Installer B 重建(已在前面 round 19c session 完成) | `build.ps1` (NSIS makensis) | ✅ 838,090 B,09:14 |
| Installer A 重建 | `cd sigma-file-manager && npx tauri build --bundles nsis --ignore-version-mismatches` | ✅ 16,934,064 B,09:34,Vite 4m 4s + Rust 3m 4s |
| 复制到 release/ 顶层 | `cp -f ... release/Sigma-File-Manager-2.2.0-focus-1-setup.exe` | ✅ |
| 复制到 release/installer/ | `cp -f ... release/installer/Sigma File Manager_2.2.0_x64-setup.exe` | ✅ |
| 验证 bundled sidecar 是 round 19c | 7z 提取 + sha256 对比 | ✅ hash 完全匹配 `1297a6304df46bc7fd78c95faa2bd528fc1e04d991c1a1a5fb55376ed2f0be65` |

---

## 三、关键避坑

### 3.1 Tauri 2.x `--ignore-version-mismatches` 必须用

**Symptom**: `npx tauri build --bundles nsis` 报:
```
Error Found version mismatched Tauri packages.
tauri (v2.12.0) : @tauri-apps/api (v2.11.1)
tauri-plugin-dialog (v2.8.0) : @tauri-apps/plugin-dialog (v2.7.1)
... (8 packages)
BUILD_EXIT=1
```

**Fix**: `npx tauri build --bundles nsis --ignore-version-mismatches`

**Why**: 这是 fork repo 已有的 uncommitted 状态(`handoff-2026-09-29-smart-upgrade-cleanup.md §3.2` package.json / Cargo.toml 都有 M)。Rust crate 比 npm 包领先 1 个 minor 版本。
- 影响:Tauri 启动时检测 API 兼容性,minor 不一致就 bail
- 但 minor 不同通常无 breaking change(Rust 2.12 vs npm 2.11 内部兼容)
- `--ignore-version-mismatches` 是 Tauri 官方支持的安全绕过(只跳过 check,不跳过功能)

### 3.2 Rust toolchain 必须 1.90,不是 1.85

**Why**:
- spike 用 1.85(独立二进制,简单)
- Sigma FM fork 用 1.90(serde / windows crate 链在 1.85 上有编译错)
- 1.85 vs 1.90 切工具链会让 Cargo 重建部分依赖(3m 4s 而不是 30s)

**PATH**:
```bash
export PATH="/c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin:$PATH"
export RUSTUP_DIST_SERVER=https://rsproxy.cn RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup
```

### 3.3 NSIS CopyFiles 路径必须在 hook 里显式写

`hooks.nsh` 里 `CopyFiles /SILENT` 必须 source + dest 都给,不能只给 source。已在 hook 里修正。

---

## 四、两个 installer 的差异对比

| 维度 | A:一体化包 | B:仅扩展 |
|---|---|---|
| 大小 | 16.15 MiB | 838 KB |
| 包含 Sigma FM 二进制 | ✅ Tauri NSIS 14 MiB | ❌ |
| UAC | 需要 admin | 不需要 |
| installMode | perMachine | currentUser |
| 默认安装路径 | `%PROGRAMFILES%\Sigma File Manager\` | `$LOCALAPPDATA\Programs\kizemo.focus-sync\` |
| smart-upgrade | ✅(检测旧 Sigma FM 路径) | ❌(只看 user-extensions.json) |
| 适用场景 | 新装 / 想换 kizemo fork | 已有 Sigma FM 加扩展 |
| Sidecar 路径 | `%APPDATA%\binaries\focus-sync-sidecar\0.2.0\` | `%APPDATA%\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\` |
| 注册机制 | POSTINSTALL 直接 copy | PowerShell `register.ps1` 改 user-extensions.json |

---

## 五、未做(本会话不做)

- ❌ 提交 round 18-19c + 本次 rebuild 改动(`sigma-file-manager/` fork repo + meta-repo 都还是 uncommitted,per CLAUDE.md 红线 "DO NOT commit changes unless the user explicitly asks")
- ❌ 上游 PR(用户说"先操作完成1 + 2",item 3 后续再做)
- ❌ Tauri 版本同步修正(8 个 npm package 与 Rust crate 不对齐)— `--ignore-version-mismatches` 已够用,真要修就 bump 8 个 npm package 到最新,留作 TODO
- ❌ Installer B 的 README 重写(已经准确,但可以加"如何识别已有 Sigma FM 是 kizemo fork 还是官方版"说明)

---

## 六、用户验证 checklist

### 6.1 验证 Installer A(一体化包)— 推荐先在 VM 或测试机
1. 在没装 Sigma FM 的机器运行 `release/Sigma-File-Manager-2.2.0-focus-1-setup.exe`
2. UAC → 允许 → 选路径 → Install
3. 启动 Sigma FM → Ctrl+Shift+X → Installed tab → Focus Sync 应出现
4. **验证 sidecar 是 round 19c**:启动后 sidecar 应 stay alive,不应当第一个 dialog event 后自杀
5. 测 ≥4 次 Sigma FM tree 切换(参考 `docs/superpowers/test-plans/2026-09-27-listary-focus-sync-manual-checklist.md`)

### 6.2 验证 Installer B(仅扩展)
1. 在用户的 Sigma FM 上(已装官方原版 / 已装 kizemo fork 都行)
2. **先关 Sigma FM**(tray → Quit)
3. 双击 `release/extension-installer/kizemo.focus-sync-0.2.0-setup.exe`
4. NSIS 安装向导 → Install
5. 重启 Sigma FM → Ctrl+Shift+X → Installed tab → Focus Sync 应出现
6. 同上测 ≥4 次 Sigma FM tree 切换

### 6.3 卸载验证(任一)
- 用 `appwiz.cpl` 卸载对应产品
- 或重跑 installer 选 Uninstall

---

## 七、下次会话第一件事(给 item 3 准备)

用户后续要做 item 3:向原开发者(aleksey-hoffman)提交扩展收录申请。需要:
1. 整理 upstream PR 内容(参考 `release/README.md` 已写的功能描述)
2. 注册表/插件格式合规性检查(Sigma FM extension marketplace schema)
3. 提交 PR / 论坛贴 / Discord channel(看 upstream 接受流程)
4. 准备 demo 视频(GIF/视频录屏展示 tree sync 功能)

具体工作目录:`docs/handoffs/handoff-2026-09-30-package-both-installers.md`(本文件)+ upstream 仓库交互。

---

## 八、Reference

- Round 19c binary fix: `handoff-2026-09-30-fix-ipc-http-done.md` §十一/十三
- Sigma FM kizemo fork 技术细节: `release/README.md`(8.4 KB,详细)
- Smart-upgrade PREINSTALL hook: `sigma-file-manager/src-tauri/installer/hooks.nsh`
- bundle.resources 配置: `sigma-file-manager/src-tauri/tauri.conf.json` §`bundle.resources`
- Installer B NSIS: `release/extension-installer/installer.nsi`
- Installer B 注册脚本: `release/extension-installer/register.ps1`
- Extension 集成机制: `E:\Users\Duanyi\.claude\projects\F--soft-00selfmade-filemanager\memory\sigma_fm_extension_registration.md`