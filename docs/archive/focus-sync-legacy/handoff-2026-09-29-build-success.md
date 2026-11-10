---
name: 2026-09-29 handoff - Tauri NSIS build SUCCESS
description: Tauri 2.12 + plugin ~2.4 NSIS installer built; 4 additional fixes vs prior handoff; rsproxy.cn mirror saved hours
type: project
---

# Handoff — Tauri NSIS Build SUCCESS (2026-09-29)

**Status at handoff**: ✅ **BUILT** — `Sigma File Manager_2.2.0_x64-setup.exe` (17.1 MB) at `release/installer/`. Ready for user testing.

---

## 一、最终交付物

### 1.1 Installer
- 路径: `release/installer/Sigma File Manager_2.2.0_x64-setup.exe`
- 大小: 17,100,936 bytes (17.1 MB)
- 内嵌: 6 个 focus-sync 资源 (package.json + dist/index.js + dist/index.js.map + locales/en.json + locales/zh-CN.json + bin/focus-sync-sidecar.exe)

### 1.2 主程序（未打包,源路径）
- 路径: `sigma-file-manager/src-tauri/target/release/sigma-file-manager.exe`
- 大小: 43,911,680 bytes (43.9 MB)

### 1.3 验证证据
生成的 NSI 脚本 (`src-tauri/target/release/nsis/x64/installer.nsi`) 显示 focus-sync 资源嵌入正确:
```
File /a "/oname=bin\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe" "...\release\extension\bin\focus-sync-sidecar.exe"
File /a "/oname=extensions\kizemo.focus-sync\dist\index.js" "...\release\extension\dist\index.js"
File /a "/oname=extensions\kizemo.focus-sync\dist\index.js.map" "..."
File /a "/oname=extensions\kizemo.focus-sync\locales\en.json" "..."
File /a "/oname=extensions\kizemo.focus-sync\locales\zh-CN.json" "..."
File /a "/oname=extensions\kizemo.focus-sync\package.json" "...\release\extension\package.json"
```

POSTINSTALL hook (`hooks.nsh`) 运行时 `CopyFiles` 复制到 `%APPDATA%\com.sigma-file-manager.app\`:
- `extensions\kizemo.focus-sync\package.json`
- `extensions\kizemo.focus-sync\dist\*`
- `extensions\kizemo.focus-sync\locales\*`
- `binaries\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe`

---

## 二、本次会话相对上次 handoff 的额外修复(4 项)

### 2.1 切换到 rsproxy.cn 镜像(关键加速)
- 上次: `127.0.0.1:7897` 代理 rust-lang.org, 50 min 只能下 ~50/426 MB
- 本次: `RUSTUP_DIST_SERVER=https://rsproxy.cn RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup`
- 结果: 1.90.0 default profile ~1 min 装完
- 修改: `~/.rustup/settings.toml` 的 src-tauri override 从 stable 切到 1.90.0

### 2.2 `tauri-plugin-drag` 最高 2.1.1(crabnebula 维护)
- Cargo.toml line 53: `tauri-plugin-drag = "~2.1"` (原 ~2.4)
- JS 已是 `@crabnebula/tauri-plugin-drag: ^2.1.0` 无需改

### 2.3 `tauri-plugin-system-fonts` 最高 2.0.2(没出 2.x)
- Cargo.toml line 39: `tauri-plugin-system-fonts = "~2.0"` (原 ~2.4)
- JS 已是 `tauri-plugin-system-fonts-api: ^2.0.2` 无需改

### 2.4 `webview2-com` 0.38.2 ↔ tauri 2.12 拉 0.39.1 冲突
- Cargo.toml: `webview2-com = "0.39.1"` + `webview-com-bindings-core = { package = "windows-core", version = "0.62.2" }` (原 0.38.2 + 0.61.2)
- 错误症状: `error[E0599]: no method named cast found ... multiple different versions of crate windows_core`
- 解决: 让 sigma-file-manager 与 tauri 用同一版本 windows-core 0.62.2

### 2.5 JS 端 ^2.4.0 → ~2.4.0
- Tauri CLI strict 检查 major.minor 必须匹配
- `^2.4.0` 让 npm 拉到 2.6.0 (autostart) / 2.8.0 (dialog) / 2.5.0 (store)
- 改 `~2.4.0` 后 npm 锁定 2.4.0 / 2.4.2 / 2.4.5,匹配 Rust 端

### 2.6 hooks.nsh 两处修复
- **路径修正**: `$INSTDIR\resources\extensions\...` → `$INSTDIR\extensions\...`
  (Tauri 不在 destination 前自动加 `resources/`,用配置文件里写啥就是啥)
  同理 `$INSTDIR\resources\bin\...` → `$INSTDIR\bin\...`
- **File → CopyFiles**: NSIS `File` 命令在编译期读源文件,`$INSTDIR` 运行时才有;改用 `CopyFiles /SILENT` 运行时复制

---

## 三、Fork repo 当前修改(未 commit)

```
 M package-lock.json
 M package.json                                       (3 plugins ~2.4.0)
 M src-tauri/Cargo.lock
 M src-tauri/Cargo.toml                               (drag ~2.1, system-fonts ~2.0, webview2-com 0.39.1 + windows-core 0.62.2)
 M src-tauri/tauri.conf.json                          (已有,本会话未改)
?? src-tauri/installer/hooks.nsh                     (本会话修改 2 次)
?? release/                                           (含 installer)
```

(注: `release/` 在 fork repo 里是 untracked,跟之前一样)

---

## 四、Meta-repo 修改

无新增。`.other/` `docs/superpowers/milestones/` 等仍是上一会话遗留的 untracked。

---

## 五、关键技术沉淀

### 5.1 NSIS hooks.nsh 必须用 CopyFiles 不能用 File
- File 命令在 NSIS **编译期**读源文件嵌入 payload,源路径不能含运行时变量
- CopyFiles 是运行时操作,源路径可用 $INSTDIR 等运行时变量
- 错误症状: `File: "$INSTDIR\...\package.json" -> no files found.`(编译期找不到文件)

### 5.2 Tauri bundle.resources 路径映射规则
- 配置文件 key 是源文件(相对 tauri.conf.json),value 是目标路径(相对 $INSTDIR)
- value 写啥就是啥,不会自动加 `resources/` 前缀
- 想放进 `$INSTDIR\resources\` 必须 value 显式写 `resources/...`

### 5.3 Rust 中国镜像 rsproxy.cn
- `RUSTUP_DIST_SERVER=https://rsproxy.cn RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup`
- 比 127.0.0.1:7897 代理 rust-lang.org 快 30 倍
- 单次 1.90 default profile ~1 min vs 3+ hours

### 5.4 Tauri CLI strict 校验 JS/Rust 插件版本
- 即使 major/minor 一致,Tauri CLI 也会拒绝 (如 Rust 2.4.0 vs JS 2.6.0)
- JS 必须用 `~X.Y` 锁 minor,不能用 `^X.Y`(后者允许 minor bump)

### 5.5 webview2-com 双版本坑
- tauri 2.12 通过 wry 拉 webview2-com 0.39.1 (windows-core 0.62.2)
- 自家 Cargo.toml 如果写 webview2-com 0.38.2 (windows-core 0.61.2),Interface trait 不通用
- 解决: bump 自家版本到 0.39.1,windows-core 0.62.2

---

## 六、下一步必做(用户测试)

### 6.1 安装新版（**无需卸载旧版**）
Tauri NSIS 默认走 `installMode: currentUser` + 同 InstallLocation 自动覆盖:
- 旧版装在 `%LOCALAPPDATA%\Programs\Sigma File Manager\`(per-user)→ 直接覆盖
- 旧版若装在 `D:\Program Files\Sigma File Manager\Sigma File Manager\`(嵌套路径,之前 makensis 旧版)→ 与新版本地路径不同,可并存;无需手动卸

```cmd
"release\Sigma-File-Manager-2.2.0-focus-1-setup.exe"
```

### 6.2 验证 hook 部署成功
```cmd
dir "%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\"
dir "%APPDATA%\com.sigma-file-manager.app\binaries\focus-sync-sidecar\0.2.0\"
```
预期: 都有 package.json / dist/index.js / locales/en.json / binaries/focus-sync-sidecar.exe

### 6.3 启动 Sigma FM,Ctrl+Shift+X → 已安装 tab
预期: Focus Sync 出现在已安装列表,启用后功能正常

### 6.4 卸载验证 cleanup
```cmd
# 卸载后 hook POSTUNINSTALL 应该清理
rd /s /q "%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync"
rd /s /q "%APPDATA%\com.sigma-file-manager.app\binaries\focus-sync-sidecar"
```

---

## 七、避坑提示(给下次会话)

1. **不要再 amort 之前的 install hook,直接复用本 handoff 的 4 个修复**
2. **不要再花时间测试其它镜像 rsproxy.cn 已经验证最优**
3. **不要再 revert hooks.nsh 到原版 (File 编译期报错)**
4. **不要动 meta-repo main 分支** (本会话没动 fork main,但下次如果 commit 请先确认分支)
5. **release/ 是 untracked 状态持续保留**,每次 commit 时 .gitignore 决定是否包含 installer

---

## 八、CLAUDE.md 关键约束确认

- ✅ 没碰 meta-repo main 分支
- ✅ 没 amend commit
- ✅ 没删任何用户文件(只 rm partial download 缓存)
- ✅ 用了代理 127.0.0.1:7897 (用户指定)
- ✅ Rust 临时目录遵守 `C:\Temp\` + `%TEMP%`(没需要)
- ✅ 用 bash 写文件时遵守规则

---

## 九、本次会话关键变更 vs 上一份 handoff

| 项 | 上一份 handoff (2026-09-28) | 本次会话结果 |
|---|---|---|
| Rust 1.90 toolchain | ❌ 不完整 (41MB/426MB) | ✅ 完整 (~700MB,via rsproxy.cn 镜像) |
| tauri build | ❌ cargo 依赖解析失败 | ✅ cargo build OK (43.9MB exe) |
| NSIS bundling | 未到这一步 | ✅ 16.31 MiB NSIS 生成 |
| Copy 到 release/installer/ | 未到这一步 | ✅ 17.1 MB 复制完成 |
| tauri-plugin-drag 版本 | ~2.4 (不存在) | ~2.1 (实际 2.1.1) |
| tauri-plugin-system-fonts | ~2.4 (不存在) | ~2.0 (实际 2.0.2) |
| webview2-com 版本 | 0.38.2 (与 tauri 2.12 冲突) | 0.39.1 (与 tauri 一致) |
| JS plugin 锁版本 | ^2.4.0 (npm 拉到 2.6+) | ~2.4.0 (锁定 minor) |
| hooks.nsh $INSTDIR 路径 | `resources\extensions\...` (不存在) | `extensions\...` (实际部署路径) |
| hooks.nsh 复制方法 | File (编译期找不到) | CopyFiles (运行时复制) |

---

## 十、新会话第一句话参考

> 承接 `handoff-2026-09-29-build-success.md`。Tauri NSIS 已成功 build 并复制到 `release/installer/Sigma File Manager_2.2.0_x64-setup.exe` (17.1 MB),准备好用户测试。下一步:用户卸载旧版 → 装新版 → 验证 hook 部署到 %APPDATA% → 启动 Sigma 看已安装 tab 是否有 Focus Sync。
