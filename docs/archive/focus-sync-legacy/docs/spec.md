# Focus Sync — Architecture Spec

**状态**: 0.2.0 实现中(v0.1.0 spike→focus-sync-sidecar 重命名)(2026-09-27 起)
**目标平台**: Windows 10/11 x64
**宿主**: Sigma File Manager >= v2.2.0
**与 fork 关系**: 独立仓库 `kizemo/focus-sync`,不污染 `kizemo/sigma-file-manager`

---

## 1. 决策(为什么这样设计)

### 1.1 为什么做成 extension 而不是 fork 内置功能

| 维度 | Fork 内置(extension 路线旧版) | Extension(本版) |
|---|---|---|
| 仓库 | 必须 fork Sigma FM | 独立 repo |
| Tree 版本影响 | 污染(用户已投诉) | 零影响 |
| Sigma FM 上游 PR 友好 | 否(实验功能混主线) | 是(纯扩展 PR) |
| Marketplace 分发 | 不可 | 可 |
| 复用 Sigma FM 自动下载 + 完整性校验 | 否 | ✓(`binaries[]` + `integrity`)|
| 用户手动安装 focus-sync-sidecar.exe | 需要 | 不需要(host 自动) |

**结论**:Extension 是唯一符合 Sigma FM 生态惯例的方案。

### 1.2 为什么 IPC 用 HTTP 而不是 TCP

Sigma FM 扩展 API 允许的权限:
- `'shell'` — 启动子进程(必要)
- `'http'` + `hosts` 白名单 — `sigma.http.request()`(允许)
- ❌ **不允许** 原始 TCP 套接字(扩展跑在 Web Worker / Webview,无 `socket()` API)

→ focus-sync-sidecar 改 HTTP server,扩展用 `sigma.http.request` 调。

### 1.3 为什么 focus-sync-sidecar 是单独二进制而不是 in-process Rust

- COM STA 约束:focus-sync-sidecar 的 `uia_event` 用单线程 tokio 维持 COM STA,跟 Sigma 主进程 runtime 不兼容
- 子进程隔离:focus-sync-sidecar crash 不应拖垮 Sigma
- 生命周期管理:`sigma.shell.runWithProgress` 返回的 task 由 host 在扩展 deactivate 时自动 terminate(见 `sigma-file-manager/src-tauri/src/extensions/processes.rs`)
- 升级路径:focus-sync-sidecar 可独立发版,扩展 manifest integrity 一致性由 release workflow 自动更新

### 1.4 为什么 focus-sync-sidecar 自己带 HTTP 框架(axum)而不是 stdout JSON-line

- `sigma.shell.runWithProgress` 的 stdout 是字符串累积,**不**给扩展提供 stdin/管道写
- HTTP 是最干净的 request/response 协议,manifest 里 `http` 权限是声明式白名单
- axum 已经是 tokio 生态标准,无新概念负担

---

## 2. 仓库结构

```
focus-sync/
├── README.md                       # 用户文档
├── LICENSE                         # MIT
├── docs/
│   └── spec.md                     # 本文件
├── extension/
│   ├── package.json                # Sigma FM 扩展 manifest
│   ├── src/index.ts                # activate/deactivate
│   ├── locales/
│   │   ├── en.json
│   │   └── zh-CN.json
│   ├── tsconfig.json
│   ├── rollup.config.mjs           # bundle to dist/index.js
│   └── dist/                       # build 产物(本地,不入 git)
├── sidecar/
│   ├── Cargo.toml                  # windows 0.62 + tokio + axum
│   ├── Cargo.lock
│   ├── src/
│   │   ├── main.rs                 # tokio::select!(http_server + uia_event + ctrl_c)
│   │   ├── lib.rs                  # 模块导出
│   │   ├── http_server.rs          # axum: /set_path /get_status /quit /health
│   │   ├── tcp_server.rs           # 旧 TCP 协议(--ipc tcp 保留兼容)
│   │   ├── uia_inject.rs           # 移植自 PathWrap(MIT)
│   │   ├── uia_event.rs            # 移植自 PathWrap(MIT)
│   │   ├── fg_bypass.rs            # 移植自 QwenLM(Apache-2.0)
│   │   ├── config.rs / events.rs / logger.rs / state.rs
│   │   ├── detector.rs / reader.rs / writer.rs
│   │   └── __tests__/              # 11/11 单测
│   ├── scripts/
│   ├── README.md                   # focus-sync-sidecar 文档
│   └── target/                     # cargo build 产物(不入 git)
├── .github/
│   └── workflows/
│       ├── build.yml               # PR check:build + smoke test
│       └── release.yml             # tag 触发:打包 + GitHub Release
└── scripts/
```

---

## 3. 协议(sidecar HTTP API)

### 3.1 端点

所有端点监听 `127.0.0.1:37421`(loopback,无 firewall 弹窗)。

| 方法 | 路径 | 请求体 | 响应 | 用途 |
|---|---|---|---|---|
| GET | `/health` | — | `{"ok":true,"service":"focus-sync-sidecar"}` | 活体检查 |
| GET | `/get_status` | — | `{"ok":true,"current_path":"...","active_dialogs":N}` | 查询状态 |
| POST | `/set_path` | `{"path":"C:\\Users\\foo"}` | `{"ok":true}` | 推送当前路径(扩展每 100ms 防抖) |
| POST | `/quit` | — | `{"ok":true}` | 优雅退出(sidecar 写完响应后 process::exit(0)) |

### 3.2 错误响应

| HTTP 状态 | 含义 |
|---|---|
| 200 | 成功 |
| 400 | 请求体 JSON 解析失败 |
| 500 | 内部错误(应极少见,focus-sync-sidecar 是 stateless HTTP) |

### 3.3 与 Sigma FM 扩展的 IPC 时序

```
Sigma frontend (Vue)
    │  user clicks tree node
    ▼
Pinia store: files.currentPath = 'D:\downloads'
    │  watch(currentPath, ...) 触发
    ▼
extension: listary-focus-sync.syncPath(path) (100ms debounce)
    │  sigma.http.request({url: 'http://127.0.0.1:37421/set_path', method: 'POST', body: '{"path":"D:\\downloads"}'})
    ▼
Sigma host (Tauri): POST → focus-sync-sidecar.exe :37421
    │  state.set_current_path(path)
    │  dialogs = state.active_dialogs()
    │  for d in dialogs: write_path(d.hwnd, path)
    ▼
sidecar UIA: SetValue("filename_edit", path) + Invoke("OK")  (or SendInput fallback)
    │
    ▼
外部 app 文件对话框跳转到 D:\downloads ✓
```

---

## 4. 生命周期

### 4.1 sidecar 进程

| 时机 | 动作 |
|---|---|
| Sigma 启动,扩展 `activate()` | `sigma.binary.getPath('focus-sync-sidecar')` → `sigma.shell.runWithProgress(sidecarPath, ['--ipc', 'http', '--port', '37421'])` |
| Sigma 运行中 focus-sync-sidecar crash | `pushNow()` 失败 → 静默丢弃;下次 activate 重启(用户 toggle 关闭再开可触发重激活) |
| Sigma 退出 / 扩展 disable | host `terminate_all_extension_processes(extension_id)` 强杀 sidecar 进程树(`taskkill /PID /T /F` per `processes.rs`)|
| 用户点 "Enable/Disable" toggle | 改 `sigma.storage.set('enabled', ...)`;sidecar 进程继续跑但 `pushNow()` 直接 return |

### 4.2 二进制分发

| 阶段 | 由谁负责 |
|---|---|
| 用户在 Sigma 里装扩展 | Sigma host 下载 `https://github.com/kizemo/focus-sync/releases/latest/download/package.json` |
| manifest `binaries[0].assets[0]` 解析 | Sigma host 看到 `downloadUrl` + `integrity` → 下载 `focus-sync-sidecar-windows-x64.zip` → SHA256 校验 |
| 失败处理 | 完整性校验失败 → 拒绝安装,提示用户(见 `binaries.rs::require_integrity`) |
| 升级 | 扩展版本号变化 → 自动重新下载(同 install 流程) |

### 4.3 跟 fork 集成(未来)

当扩展开发完成后,**打包到 fork 的 NSIS installer**:

```nsi
; Sigma FM v2.2.0-tree.1 fork installer
; 1. 安装 Sigma FM(原版流程)
; 2. 拷贝扩展到 %APPDATA%\...\extensions\kizemo.focus-sync\
File /r "extension\*.*"
; 3. 拷贝 focus-sync-sidecar.exe 到 binaries dir
File "sidecar\focus-sync-sidecar.exe"
; 4. 安装后 spawn Sigma --extension-folder=...
```

这让用户装 fork 安装包 = 装了 Sigma FM tree + focus-sync 扩展 + focus-sync-sidecar.exe,无需手动装扩展。

---

## 5. 测试

### 5.1 sidecar 单测(11/11,原 extension 路线保留)

- `state::tests` 2 个
- `config::tests` 3 个
- `events::tests` 3 个
- `logger::tests` 3 个

### 5.2 sidecar HTTP 集成测试(GitHub Actions)

`.github/workflows/build.yml::integration`:
- 启动 `focus-sync-sidecar.exe --ipc http --port 37422`
- GET `/health` → 200 + body.ok
- POST `/set_path {path: C:\Users\Public}` → 200
- GET `/get_status` → body.current_path == "C:\Users\Public"
- POST `/quit` → sidecar 进程退出

### 5.3 扩展 bundle 测试

- `tsc --noEmit` — TypeScript 类型检查
- `rollup -c` — 打包到 `dist/index.js`
- CI 验证 `dist/index.js` 存在 + 大小合理

### 5.4 7-app GUI 验证矩阵(用户手动)

见 `meta-repo/docs/superpowers/test-plans/2026-09-27-listary-focus-sync-manual-checklist.md`(继承)。

---

## 6. 部署清单(release cut)

1. **Bump 版本**:`extension/package.json` 改 `"version": "0.X.0"`
2. **更新 manifest integrity**:`release.yml` 自动算 zip 的 SHA256 并写回 manifest
3. **打 tag**:`git tag v0.X.0 && git push origin v0.X.X`
4. **GitHub Actions 自动**:
   - cargo build focus-sync-sidecar.exe
   - zip + sha256
   - 更新 manifest integrity
   - 创建 GitHub Release,附带 `focus-sync-sidecar-windows-x64.zip` + `package.json` + `.sha256`
5. **用户装**:`Extensions → Add from URL: https://github.com/kizemo/focus-sync/releases/latest/download/package.json`

---

## 7. 未来里程碑

| Phase | 内容 | 状态 |
|---|---|---|
| 0.1.0 | HTTP IPC + 扩展骨架 + CI/release | 进行中 |
| 0.2.0 | 7-app GUI 矩阵验证 | 待跑(需 Word / VS Code / 微信) |
| 0.3.0 | 集成到 fork NSIS installer(tree 版本 + 扩展 pre-bundled) | 设计中 |
| 0.4.0 | UWP / XAML 扩展(`is_xaml_host_hwnd` from QwenLM) | 后续 |
| 1.0.0 | 提交上游 Sigma FM PR | 视上游接受度 |

---

## 8. 不做的事(继续不做)

- ❌ macOS / Linux(UIA 是 Windows 专有)
- ❌ DLL 注入(AV / WDAC 风险)
- ❌ 修改 sidecar 已移植文件的版权头
- ❌ fork Sigma FM 主分支(扩展已解耦)
- ❌ 上游自动 PR(走人工评审)