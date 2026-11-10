---
name: 2026-09-28 handoff - Tauri NSIS rebuild blocked by rustc 1.90 install failure
description: Session timed out at ~110min on Tauri 2.12 build; rustc 1.90 install incomplete; next session must finish install + run build
type: project
---

# Handoff — Tauri NSIS Rebuild Session (2026-09-28)

**Session goal**: Build Tauri NSIS installer (per upstream aleksey-hoffman) with focus-sync extension pre-bundled, replacing the custom 7z SFX approach.

**Status at handoff**: ❌ **BLOCKED** — 1.90 toolchain install incomplete. All source-side changes committed to fork; only the build step remains.

---

## 一、已完成（git 工作树，**未 commit**）

### 1.1 修改了 `sigma-file-manager/src-tauri/tauri.conf.json`
加了 `bundle.resources` 把 focus-sync extension + sidecar 嵌入 NSIS，加 `bundle.windows.nsis.installerHooks` 指向自定义 NSIS 钩子脚本：

```json
"bundle": {
  "active": true,
  "targets": ["nsis", "appimage", "deb", "app", "dmg"],
  "resources": {
    "../../release/extension/package.json": "extensions/kizemo.focus-sync/package.json",
    "../../release/extension/dist": "extensions/kizemo.focus-sync/dist",
    "../../release/extension/locales": "extensions/kizemo.focus-sync/locales",
    "../../release/extension/bin/focus-sync-sidecar.exe": "bin/focus-sync-sidecar/0.2.0/focus-sync-sidecar.exe"
  },
  "windows": {
    "nsis": {
      "installMode": "currentUser",
      "installerIcon": "icons/icon.ico",
      "installerHooks": "installer/hooks.nsh"
    }
  }
}
```

### 1.2 新建 `sigma-file-manager/src-tauri/installer/hooks.nsh`
4 个 NSIS hook 宏：PREINSTALL (空) / POSTINSTALL (部署 extension 到 %APPDATA% + 写注册表) / PREUNINSTALL (空) / POSTUNINSTALL (清理 extension 目录)。POSTINSTALL 创建 `com.sigma-file-manager.app\extensions\kizemo.focus-sync\` + `binaries\focus-sync-sidecar\0.2.0\` 并用 `File /oname=` 复制 resources。

### 1.3 修改 `sigma-file-manager/src-tauri/Cargo.toml`
所有 `tauri-plugin-* = "2"` 改为 `"~2.4"`（避免解析到 2.5+ 要求 rustc 1.90+），`tauri-plugin-clipboard-manager = "=2.3.2"` 保持精确。

### 1.4 修改 `sigma-file-manager/package.json`
所有 `@tauri-apps/plugin-*` 改为 `^2.4.0` 或 `=2.3.2` / `=2.5.4`（与 Rust 锁一致）。

---

## 二、当前状态

### 2.1 Build 状态 — **BLOCKED**
- ❌ rust-lang.org 直连被 VPN 拦截，**需要用代理 `http://127.0.0.1:7897`**（用户明确指示）
- ❌ rustc 1.85.0 已装但不够（Tauri 2.12 + plugin 2.5+ require rustc 1.90+）
- ❌ rustc 1.90.0 install **严重不完整**（`minimal` profile 下载了 cargo 但缺 rustc + rust-std）
- ❌ 1.90 toolchain 目录只有 41 MB（完整应该 426 MB+）
- ⚠️ partial 下载文件在 `~/.rustup/downloads/`（可能是网络慢）
- ❌ `rustup component add rustc --toolchain 1.90.0` 报 "Missing manifest"（因为 minimal 装的 manifest 不全）
- ⚠️ 试过 `--component rustc --component rust-std` 显式指定，但 partial install 还是只有 cargo

### 2.2 当前 fork 分支状态
- 仍在 `feat/recovery-2026-09-28` meta-repo 分支
- 4 commits ahead of main
- 切换了 `rustup override set 1.90.0-x86_64-pc-windows-msvc`（incomplete）
- `src-tauri/Cargo.lock` 已删（让 cargo 重新生成）

### 2.3 未完成的关键路径
1. 1.90 toolchain 完整安装（含 rustc + rust-std）
2. `npm install` 重新解析 package-lock.json（之前已用新版本号）
3. `npm run tauri:build` 跑 Tauri build
4. 验证 `src-tauri/target/release/bundle/nsis/Sigma File Manager_2.2.0_x64-setup.exe` 生成
5. 复制 NSIS 到 `release/installer/`
6. 跑 `release/scripts/sfx-config.txt` + `release/installer/` 制作完整 bundle
7. 复制 `Sigma File Manager_2.2.0_x64-setup.exe` → `release/Sigma-File-Manager-2.2.0-focus-1-setup.exe`
8. 用户测试

---

## 三、关键文件清单（要读 / 要 build / 要复制）

| 文件 | 路径 | 状态 |
|---|---|---|
| hooks.nsh | `sigma-file-manager/src-tauri/installer/hooks.nsh` | ✅ 新建 |
| tauri.conf.json | `sigma-file-manager/src-tauri/tauri.conf.json` | ✅ 加 resources + installerHooks |
| Cargo.toml | `sigma-file-manager/src-tauri/Cargo.toml` | ✅ plugin ~2.4 |
| package.json | `sigma-file-manager/package.json` | ✅ plugin ^2.4 |
| release/extension/ | `release/extension/{package.json,dist/,locales/,bin/focus-sync-sidecar.exe}` | ✅ 已有 v0.2.0 |
| release/installer/ | `release/installer/Sigma File Manager_2.2.0_x64-setup.exe` | ⏳ 待 build |
| release/scripts/ | `release/scripts/{install-focus-sync.ps1, sfx-config.txt, installer.nsi}` | ⚠️ 之前用的是 7z SFX + install-focus-sync.ps1 方式，新方案不再用 |
| handoff docs | `docs/handoffs/handoff-2026-09-28-plugin-route.md` | ✅ Phase 2a 插件路线规划 |
| 7-app verify template | `docs/superpowers/test-plans/2026-09-28-focus-sync-7-app-verify.md` | ✅ |

---

## 四、技术约束（已确认）

### 4.1 Tauri 2.x build
- 必需 rustc 1.90+（Tauri 2.12 + plugin 2.5+）
- `bundle.resources` 接受 source → target 映射
- `bundle.windows.nsis.installerHooks` 接受 .nsh 文件路径
- Tauri NSIS 默认装到 `%LOCALAPPDATA%\Programs\`（per-user `installMode: currentUser`）
- Tauri NSIS 模板内置 `closeapplications.nsh` 自动 kill 运行中应用

### 4.2 hooks.nsh 资源路径
- Tauri 把 `bundle.resources` 里的文件复制到 `$INSTDIR\resources\` 目录
- 路径映射：
  - `extensions/kizemo.focus-sync/package.json` → `$INSTDIR\resources\extensions\kizemo.focus-sync\package.json`
  - `bin/focus-sync-sidecar/0.2.0/focus-sync-sidecar.exe` → `$INSTDIR\resources\bin\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe`
- hooks.nsh 用 `File "$INSTDIR\resources\extensions\kizemo.focus-sync\package.json"` 引用

### 4.3 网络约束
- rust-lang.org 直连被 VPN 拦截
- **必须用代理 `http://127.0.0.1:7897`**
- crates.io 和 GitHub 可直连（验证过）
- cargo 全局配置（`~/.cargo/config.toml`）已设 `git-fetch-with-cli = true`

### 4.4 用户的偏好（CLAUDE.md 之外）
- 路径硬编码 E:（`E:/Users/Duanyi/.claude/`）— 不依赖 C: 默认 home
- 删项目根 handoff/prompt 文件需要二次确认
- git 操作遵循 fork repo 与 meta-repo 分离

---

## 五、下一步必做（按顺序）

### Step 1 — 装 1.90 toolchain 完整
```bash
# 用代理重装 1.90 (含 rustc + rust-std)
export http_proxy="http://127.0.0.1:7897"
export https_proxy="http://127.0.0.1:7897"
# 先 clean partial
rm -rf /c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc
rm -f /c/Users/Duanyi/.rustup/downloads/*.partial
# 装 default profile (含 rustc + rust-std + cargo + clippy + rustfmt)
# 预计下载 800MB-1GB, 网络慢可能要 30-60min
/c/Users/Duanyi/.cargo/bin/rustup.exe toolchain install 1.90.0-x86_64-pc-windows-msvc --profile default
# 验证
ls /c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin/  # 必须有 rustc.exe
/c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin/rustc.exe --version
```

### Step 2 — 跑 Tauri build
```bash
cd "/f/soft/00selfmade/filemanager/sigma-file-manager"
/c/Users/Duanyi/.cargo/bin/rustup.exe override set 1.90.0-x86_64-pc-windows-msvc
export PATH="/c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin:/c/Users/Duanyi/.cargo/bin:$PATH"
export http_proxy="http://127.0.0.1:7897" https_proxy="http://127.0.0.1:7897"
npm install  # 重新解析 package-lock 到 2.4
npm run tauri:build  # 30-60min Rust 编译
```

### Step 3 — 验证 NSIS 输出
```bash
# 期望路径
ls -la "F:/soft/00selfmade/filemanager/sigma-file-manager/src-tauri/target/release/bundle/nsis/Sigma File Manager_2.2.0_x64-setup.exe"
# 期望 size ~16MB
```

### Step 4 — 复制到 release/installer
```bash
cp "F:/soft/00selfmade/filemanager/sigma-file-manager/src-tauri/target/release/bundle/nsis/Sigma File Manager_2.2.0_x64-setup.exe" \
   "F:/soft/00selfmade/filemanager/release/installer/Sigma File Manager_2.2.0_x64-setup.exe"
```

### Step 5 — 制作最终 installer bundle
- **不需要 7z SFX 了**（Tauri NSIS 自己有 closeapplications 自动 kill）
- 之前 7z SFX 的 `Sigma-File-Manager-2.2.0-focus-1-setup.exe` (31MB) 是过渡产物
- 新版直接用 Tauri NSIS：
  - `release/installer/Sigma File Manager_2.2.0_x64-setup.exe`（含 focus-sync 通过 installerHooks 部署）
- **或者继续用 7z SFX 包装**：把 Tauri NSIS + scripts/ 包成 SFX，run `install-focus-sync.ps1` 部署 extension（**但 Tauri NSIS 已自动部署 extension，** 不需要再跑 ps1）

### Step 6 — 用户测试
- 卸载旧版（nested 路径的 D:\Program Files\Sigma File Manager\Sigma File Manager\）
- 装新 NSIS（含 focus-sync 预装）
- 检查 `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\package.json` 存在
- 检查 `%APPDATA%\com.sigma-file-manager.app\binaries\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe` 存在
- 启动 Sigma → Ctrl+Shift+X → 已安装 tab 看到 Focus Sync

---

## 六、避坑提示

1. **不要用 1.85**（Tauri 2.12 + plugin 2.5+ 需要 1.90+）
2. **不要用 minimal profile 装 1.90**（缺 rustc，要用 default）
3. **必须用代理 127.0.0.1:7897**（用户指示）
4. **不要清 cargo cache 再重装**（之前 6 次 build 都因 cache 损坏失败，但清 cache 是因为之前 cargo 损坏——现在 cache 干净，可不重下）
5. **不要修改 fork main 分支名**（CLAUDE.md 红线，但 fork repo 跟 meta-repo 是独立仓库不算 meta-repo main）
6. **不要 amend commit**（CLAUDE.md 红线：保护 git history）

---

## 七、背景信息

### 7.1 项目主线
- filemanager (kizemo/sigma-file-manager fork)：Listary 风格全局文件对话框焦点同步
- 之前 Spike spike PASS（HEAD `905db31`）
- Phase 2a 集成 spec 已写（`docs/superpowers/specs/2026-09-27-listary-focus-sync-integration-design.md`，已存档到 `_archive/`）
- Plugin 路线：kizemo/focus-sync v0.2.0（已发布 release）

### 7.2 之前的几个错误尝试
1. `installer.nsi`（makensis）— 用户反馈嵌套路径 + 没显示 focus-sync
2. 7z SFX 包装 — 用户反馈"和原开发者打包安装文件的方法也不一致"
3. 当前方案：Tauri NSIS + installerHooks — **正确方向**

### 7.3 用户期望
- 安装方法与原开发者（aleksey-hoffman）打包一致：Tauri NSIS 直接 install
- 安装时自动 kill 运行中 Sigma（Tauri NSIS 内置 closeapplications）
- 装完 focus-sync 已在"已安装" tab
- 默认装到 %LOCALAPPDATA%\Programs\Sigma File Manager\（per-user）

---

## 八、CLAUDE.md 路径

新会话需要遵守：
- `F:/soft/00selfmade/filemanager` (meta-repo)
- `F:/soft/00selfmade/filemanager/sigma-file-manager` (fork repo)
- 路径解析：E: 是用户 Claude 数据主目录（不是 C:）
- fork main ≠ meta-repo main（两个独立 repo）
- 删项目根文件要 ask
- 不 amend commit
- 不修改 meta-repo main 分支

---

## 九、下次会话第一句话参考

> 承接 `handoff-2026-09-28-build-fail.md`。当前状态：fork 已修改 4 个文件（hooks.nsh 新建 + tauri.conf.json + Cargo.toml + package.json），rustup override 已切到 1.90 但 toolchain install 不完整（41MB vs 需要的 426MB）。需要用代理 127.0.0.1:7897 装 1.90 default profile 然后跑 `npm run tauri:build`。
