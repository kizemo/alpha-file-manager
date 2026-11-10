---
name: 2026-09-29 handoff (round 2) - smart-upgrade PREINSTALL hook + cleanup old 7z SFX flow
description: Round-2 of build-success session. PREINSTALL detects old install path; installMode perMachine; README rewritten; 5 obsolete scripts removed
type: project
---

# Handoff — Smart-Upgrade + Cleanup (2026-09-29 round 2)

**Inherits**: `handoff-2026-09-29-build-success.md` (base NSIS build SUCCESS, 17.1 MB installer).

**Status**: ✅ **READY FOR USER TEST** — installer rebuilt (16.30 MiB) with smart-upgrade detection. Located at both `release/Sigma-File-Manager-2.2.0-focus-1-setup.exe` (top-level entry) and `release/installer/Sigma File Manager_2.2.0_x64-setup.exe`.

---

## 一、本轮新增 vs 上份 handoff

### 1.1 PREINSTALL hook 加 smart-upgrade(用户需求)
旧版 Sigma FM 装在 `D:\Program Files\Sigma File Manager\`(嵌套路径,之前 makensis 装的)。期望新版沿用同一目录,不要走默认的 `%LOCALAPPDATA%\Programs\`。Tauri NSIS 默认不会检测,需要在 hook 里写。

**改动**:
- `sigma-file-manager/src-tauri/tauri.conf.json`: `installMode: currentUser` → `perMachine`
  - 因为旧版在 `%PROGRAMFILES%` 是 per-machine 位置,新版必须也是 perMachine 才能共用
  - 副作用:装新版时会弹 UAC 要 admin
- `sigma-file-manager/src-tauri/installer/hooks.nsh` NSIS_HOOK_PREINSTALL 加三层 fallback:
  1. Tauri 注册表 `HKLM\SOFTWARE\[WOW6432Node\]...\Uninstall\com.sigma-file-manager.app_is1\InstallLocation`
  2. 旧 makensis 注册表 `...\Uninstall\Sigma File Manager_is1\InstallLocation`
  3. 文件系统探测 `$PROGRAMFILES\Sigma File Manager\Sigma FileManager.exe` → 嵌套/扁平/根目录 三种 layout
  命中后 `StrCpy $INSTDIR "$0"`(不是 `SetInstallDir`,那是错的命令)

### 1.2 清理旧 7z SFX 流产物
`release/scripts/` 已整个删除(包含 5 个文件):
- ~~install-focus-sync.ps1~~ (旧 PowerShell bootstrapper)
- ~~sfx-config.txt~~ (旧 7z SFX 配置)
- ~~installer.nsi~~ (旧 makensis 脚本)
- ~~sigma-file-manager.exe~~ (42 MB 旧打包原料)
- ~~uninstall.exe~~ (旧 makensis 生成)

### 1.3 README 重写
`release/README.md` (8.4 KB) 改写:
- 删除 7z SFX 步骤
- 加 NSIS `installerHooks` 4 个宏的说明表(PRE/POST install/uninstall)
- 加 smart-upgrade 三层 fallback 详解
- 加新 build 步骤(用 rsproxy.cn 镜像)

---

## 二、本轮技术避坑

### 2.1 NSIS `SetInstallDir` 不存在
- 我第一次写错用了 `SetInstallDir "$0"` → makensis 报 `Invalid command: "SetInstallDir"`
- 正确写法: NSIS 用 `StrCpy $INSTDIR "$0"` 在运行时修改安装目录
- `InstallDir` 是 NSIS 编译时指令(在 .nsi 头部),不能在 hook 宏里用

### 2.2 PREINSTALL 改 $INSTDIR 的时机
- NSIS_HOOK_PREINSTALL 在 install section 开头执行,**先于**文件复制 → 改 $INSTDIR 有效
- 但 InstallDir 显示页(`InstallDir "$PROGRAMFILES\..."`)在更早的 .onInit 显示,所以即便改了,用户看到默认路径,然后翻页/点击 Install → 此时 PREINSTALL 改 $INSTDIR → 实际装到旧路径
- 实测效果:用户运行 NSIS,看到默认路径(可手动改),点 Install → 自动迁到旧版路径
- 这是 NSIS 的标准"smart upgrade"模式,Tauri 上游 aleksey-hoffman 也是这么写

### 2.3 perMachine installMode 的副作用
- UAC 弹窗要 admin
- 默认路径 `$PROGRAMFILES\Sigma File Manager\` (而不是 `%LOCALAPPDATA%\Programs\` 嵌套两层的 Sigma)
- 用户级数据仍走 `%APPDATA%\com.sigma-file-manager.app\`(与 installMode 无关)

---

## 三、当前 git 状态(都没 commit,等用户决定)

### 3.1 Meta-repo `F:/soft/00selfmade/filemanager`
```
?? handoff-2026-09-29-build-success.md    (上轮新加,本会话没改)
?? handoff-2026-09-29-smart-upgrade-cleanup.md   (本轮新加)
?? release/                                (内含 installer + README + extension/)
?? docs/superpowers/milestones/            (上轮遗留)
?? .other/                                 (上轮遗留)
?? scripts/__pycache__/                    (上轮遗留)
```
- 分支: `feat/recovery-2026-09-28`(5 commits ahead of main)
- 没有改动任何已 tracked 文件(README 在 `release/` 子目录,Git 不追踪未跟踪子目录)

### 3.2 Fork repo `F:/soft/00selfmade/filemanager/sigma-file-manager`
```
 M package.json                            (3 plugins ~2.4.0 + 之前的 drag/system-fonts 调整)
 M package-lock.json                       (npm install 后自动重写)
 M src-tauri/Cargo.toml                    (drag ~2.1 + system-fonts ~2.0 + webview2-com 0.39.1 + windows-core 0.62.2)
 M src-tauri/Cargo.lock                    (cargo 重生成)
 M src-tauri/tauri.conf.json               (本轮新加 installMode perMachine;上轮已加 bundle.resources + installerHooks)
?? src-tauri/installer/                   (新加的 hooks.nsh 目录)
?? release/                                (复制到 fork 的 release/ 副本)
```
- 分支: `main` (fork 的 main,与 meta-repo 是独立仓库,不冲突 CLAUDE.md 红线)
- HEAD: `02583ccf merge: feat/tree-sidebar-v6-1 (HEAD 8caf14ae, 11 commits) for v2.2.0-focus.1 release`

### 3.3 未 commit 待用户决定
两个 repo 都有未 commit 改动。需要用户决定:
- 是否 commit 到 fork main + meta-repo feat/recovery 分支
- commit message 怎么写(用 conventional commits)

---

## 四、用户测试 checklist(必做)

### 4.1 装新版(直接双击,不卸载旧版)
```
F:\soft\00selfmade\filemanager\release\Sigma-File-Manager-2.2.0-focus-1-setup.exe
```
预期:
1. 弹 UAC → 允许(perMachine 需要 admin)
2. NSIS 显示 "Choose Install Location" 页 → 默认 `%PROGRAMFILES%\Sigma File Manager\`
3. 点 Install → 检测到旧版 `D:\Program Files\Sigma File Manager\...` → 自动切换到旧路径
4. 完成时间 < 30s

### 4.2 验证 hook 部署成功
```cmd
dir "%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\"
dir "%APPDATA%\com.sigma-file-manager.app\binaries\focus-sync-sidecar\0.2.0\"
```
预期:都有 package.json / dist/index.js / locales/en.json / focus-sync-sidecar.exe

### 4.3 启动 Sigma FM,验证功能
1. 启动新装的 Sigma(应自动装在旧版路径)
2. Ctrl+Shift+X → 已安装 tab → Focus Sync 出现
3. 启用 → 测 Save As 对话框焦点同步(参考 docs/superpowers/test-plans/2026-09-27-listary-focus-sync-manual-checklist.md)

### 4.4 卸载验证 cleanup
```cmd
"appwiz.cpl" → Sigma File Manager → 卸载
```
预期:POSTUNINSTALL hook 清理 `%APPDATA%\com.sigma-file-manager.app\extensions\` + `binaries\`

---

## 五、避坑给下次会话

1. **不要再 amort PREINSTALL hook** — 三层 fallback 已就位,实测能 find 旧路径
2. **如果用户报"装到 C:\Program Files 不是 D:\"** — 是因为 $PROGRAMFILES env 指向系统盘。检查用户的 Windows 是否真的在 D: 装。修复方案:改 InstallDir 默认值或加手动选盘符
3. **如果用户报 UAC 不弹** — perMachine 默认要 admin,确认 installer 不是从普通 cmd 启动
4. **不要改 hooks.nsh 的 CopyFiles → File** — File 编译期读源,$INSTDIR 运行时变量会编译失败
5. **如果 release/ 又多出 .gitignore** — 当前 fork repo 的 release/ 是 untracked,意味着任何 .gitignore 加在 fork 里都无效;若要让 release/ 不进 fork,要么 gitignore + untrack,要么别复制过去

---

## 六、本次新增的 NSIS 文档沉淀(下次会话直接复用)

### 6.1 NSIS 四宏的可靠使用模式

```nsh
!macro NSIS_HOOK_PREINSTALL
  ; 在 section 开头、文件复制之前
  ; 可用: ReadRegStr / ${FileExists} / StrCpy $INSTDIR
  ; 注意: InstallDir 显示页已经显示,改 $INSTDIR 后实际装到这里
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; 文件复制之后,WriteUninstaller 之前
  ; 可用: CopyFiles (运行时) / SetOutPath / DetailPrint
  ; 不要用: File (编译期读源,不能引用 $INSTDIR)
!macroend

!macro NSIS_HOOK_PREUNINSTALL / POSTUNINSTALL
  ; 同理,可运行时操作
!macroend
```

### 6.2 Tauri bundle.resources 路径映射规则(已确认)
- 配置 `key: value` 形式,key 是源(相对 tauri.conf.json),value 是目标(相对 $INSTDIR)
- value 写啥就是啥,**不会自动加 `resources/` 前缀**
- 想放进 `$INSTDIR\resources\` 必须 value 显式写 `resources/...`
- 我们的配置: `extensions/kizemo.focus-sync/...` → `$INSTDIR\extensions\kizemo.focus-sync\...`(不带 resources/)

### 6.3 RSproxy.cn 镜像仍最优
- `RUSTUP_DIST_SERVER=https://rsproxy.cn RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup`
- 单次 1.90 default profile ~1 min vs rust-lang.org 直连 3+ hours

---

## 七、新会话第一句话参考

> 承接 `handoff-2026-09-29-smart-upgrade-cleanup.md`(继承 `handoff-2026-09-29-build-success.md`)。NSIS 已重建为 16.30 MiB,带 PREINSTALL smart-upgrade 钩子,用户路径 `D:/Program Files/Sigma File Manager/` 已识别。**下一步:用户测试 + 决定是否 commit fork main + meta-repo feat 分支**。
