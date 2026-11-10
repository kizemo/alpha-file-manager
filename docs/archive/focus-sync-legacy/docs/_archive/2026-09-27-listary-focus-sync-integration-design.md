# Listary-Style Focus Sync — Phase 2 Integration Design Spec

**日期**: 2026-09-27
**承接**: `handoff-2026-09-27-spike-complete-next-phase.md`(spike PASS,decision doc 见 `docs/superpowers/decisions/2026-09-27-listary-focus-sync-decision.md`)
**前置依赖**:
- `sigma-listary-spike/` 子目录 spike.exe 已构建并 smoke test 通过(`target/release/spike.exe`,2.1 MB PE32+ x64)
- spike TCP 协议已实现:`get_status` / `set_path` / `quit`(行分隔 JSON,`127.0.0.1:37421`)
- 7-app GUI 验证矩阵覆盖完成(参考 `docs/superpowers/test-plans/2026-09-27-listary-focus-sync-manual-checklist.md` §5)
**目标**:把 spike.exe 从"独立验证二进制"升级为 Sigma FM fork 的**sidecar daemon**,在 Sigma 进程内通过 Tauri command 控制其生命周期,通过 IPC 推送当前路径变更。
**Spec 范围**:
1. Sigma-side Tauri command 定义(spawn / ping / kill / set_path)
2. IPC 选型:TCP(当前)→ named pipe(Phase 2b 推荐)
3. 前端路径变更 → 后端 → IPC → spike 全链路
4. NSIS sidecar 打包 + manifest
5. 错误恢复 + 重连策略

**不在范围**:UWP / XAML dialog 支持(留给 Phase 2c)、macOS / Linux、spike 代码本身的修改(版权头固定)。

---

## 1. 背景与动机

### 1.1 spike 现状(已 PASS)

```
sigma-listary-spike/
├── Cargo.toml                # windows = 0.62 + tokio + tracing + serde + anyhow
├── src/
│   ├── main.rs               # tokio::select!(uia_event + tcp_server + poll_path_changes)
│   ├── uia_inject.rs         # 从 PathWrap dialog.rs 移植(MIT)
│   ├── uia_event.rs          # 从 PathWrap monitor.rs 移植(MIT),egui::Context → Arc<tokio::sync::Notify>
│   ├── fg_bypass.rs          # 从 QwenLM 移植(Apache-2.0),Chromium EnableWindow(FALSE) RAII
│   ├── config.rs / events.rs / logger.rs / state.rs / tcp_server.rs / detector.rs / reader.rs / writer.rs
└── target/release/spike.exe  # 2.1 MB
```

TCP smoke test 已验证(本会话 2026-09-27):
- `get_status` → `{"ok":true,"current_path":"C:\\","active_dialogs":0}`
- `set_path` → `{"ok":true}`
- `get_status`(after set_path)→ `{"ok":true,"current_path":"C:\\Users\\Public\\Documents","active_dialogs":0}`
- `quit` → spike 正常退出(客户端 ReadLine EOF 是预期)

### 1.2 现状空白

- Sigma FM fork v2.2.0-focus.1 **没有**把 spike.exe 集成进来(NSIS 解包确认无 sidecar,无 UIAutomationCore import)。
- 当前 spike 是手动启动的独立进程,需要用户在打开 Sigma 前自己跑。
- Sigma 当前路径变更**不会**自动通知 spike。

### 1.3 Phase 2 目标

把以上三个空白填上:
1. Sigma 启动时自动 spawn spike.exe
2. Sigma 当前路径变更时通过 IPC 通知 spike
3. 用户不再需要手动启动 spike(spike 在 Sigma 退出时被一起 kill)

---

## 2. 架构总览

### 2.1 三进程拓扑

```
┌─────────────────────────┐    spawn/quit     ┌──────────────────┐
│   Sigma FM (Tauri)      │ ◄──────────────► │   spike.exe     │
│   ├─ Frontend (Vue)     │   Tauri cmd      │   (daemon)      │
│   ├─ Rust backend       │                  │                  │
│   │   ├─ Pinia store ───┼── 当前路径 ──►   │  - uia_event    │
│   │   └─ IPC client ────┼── set_path ───►  │  - uia_inject   │
│   └─ WebView2 (Edge)    │                  │  - fg_bypass    │
└─────────────────────────┘                  └──────────────────┘
                ▲                                       │
                │                                       │ UIA
                │                                       ▼
                │                          ┌──────────────────────┐
                │   file dialog 跳转       │  Chrome/Word/VS Code │
                └──────────────────────────┤  /DingTalk/Feishu/  │
                                           │  WeChat 等 7-app    │
                                           └──────────────────────┘
```

### 2.2 边界

- **Sigma 负责**:spike 生命周期(spawn / ping / kill)、Pinia 当前路径订阅、IPC client
- **spike 负责**:UIA 监听 + dialog 写入 + fg_bypass 屏蔽 + 状态回报
- **两者通过 IPC 解耦**:spike crash 不应导致 Sigma crash(只是功能降级)

---

## 3. Sigma-side Tauri Command 定义

### 3.1 Rust backend(`sigma-file-manager/src-tauri/src/commands/spike.rs` 新增)

```rust
use tauri::command;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use std::sync::Arc;

#[derive(Default)]
pub struct SpikeState {
    child: Arc<Mutex<Option<Child>>>,
    ipc: Arc<Mutex<Option<IpcClient>>>,
}

#[tauri::command]
async fn spawn_spike(state: tauri::State<'_, SpikeState>) -> Result<SpikeStatus, String> {
    // 1. 检查 spike.exe 是否已在 PATH 或 resources 目录
    // 2. tokio::process::Command::new(spike_exe_path()).spawn()
    // 3. 建立 IPC client(TCP / named pipe)
    // 4. 启动 ping task(spike crash 检测)
    // 5. 返回 SpikeStatus { running: true, port: 37421 }
}

#[tauri::command]
async fn kill_spike(state: tauri::State<'_, SpikeState>) -> Result<(), String> {
    // 1. IPC send {"cmd":"quit"}
    // 2. await child.wait()
    // 3. 清理 child + ipc state
}

#[tauri::command]
async fn spike_set_path(
    state: tauri::State<'_, SpikeState>,
    path: String,
) -> Result<(), String> {
    // IPC send {"cmd":"set_path","path":<path>}
}

#[tauri::command]
async fn spike_get_status(state: tauri::State<'_, SpikeState>) -> Result<SpikeStatus, String> {
    // IPC send {"cmd":"get_status"} → return parsed JSON
}

#[tauri::command]
async fn spike_ping(state: tauri::State<'_, SpikeState>) -> Result<bool, String> {
    // 健康检查:发 set_path 一个 no-op 或 ping 命令(见 §4.4)
}

#[derive(serde::Serialize)]
struct SpikeStatus {
    running: bool,
    port: u16,
    pipe_name: Option<String>,
    current_path: Option<String>,
    active_dialogs: u32,
}
```

### 3.2 main.rs 注册

```rust
fn main() {
    tauri::Builder::default()
        .manage(SpikeState::default())
        .setup(|app| {
            // Sigma 启动时自动 spawn spike(后台 task)
            let state = app.state::<SpikeState>();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = spawn_spike_inner(&state).await {
                    tracing::warn!("spike auto-spawn failed: {}", e);
                }
            });
            Ok(())
        })
        .on_window_event(|event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event.event() {
                // Sigma 退出时 kill spike
                let state = event.window().state::<SpikeState>();
                tauri::async_runtime::block_on(async move {
                    let _ = kill_spike_inner(&state).await;
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            spawn_spike, kill_spike, spike_set_path, spike_get_status, spike_ping
        ])
        .run(tauri::generate_context!())
        .expect("error while running sigma-file-manager");
}
```

### 3.3 前端 API(TS 类型 + Pinia 集成)

```typescript
// src/store/listary.ts (新增)
import { defineStore } from 'pinia';
import { invoke } from '@tauri-apps/api/core';

interface SpikeStatus {
  running: boolean;
  port: number;
  pipe_name: string | null;
  current_path: string | null;
  active_dialogs: number;
}

export const useListaryStore = defineStore('listary', {
  state: () => ({
    spike: { running: false, port: 0, pipe_name: null, current_path: null, active_dialogs: 0 } as SpikeStatus,
    enabled: true,  // 用户可关闭(默认开)
  }),
  actions: {
    async syncPath(path: string) {
      if (!this.enabled || !this.spike.running) return;
      try {
        await invoke('spike_set_path', { path });
      } catch (e) {
        console.warn('[listary] set_path failed:', e);
        this.spike.running = false;  // 触发 §6 重连逻辑
      }
    },
    async refreshStatus() {
      try {
        this.spike = await invoke('spike_get_status');
      } catch (e) {
        this.spike.running = false;
      }
    },
  },
});

// src/views/Navigator.vue 集成
import { useListaryStore } from '@/store/listary';
import { useFileStore } from '@/store/files';

export default {
  setup() {
    const listary = useListaryStore();
    const files = useFileStore();
    watch(
      () => files.currentPath,
      (newPath) => {
        if (newPath) listary.syncPath(newPath);
      },
      { immediate: true, deep: false }
    );
    return {};
  },
};
```

---

## 4. IPC 协议设计

### 4.1 现状(TCP)

`spike.exe --port 37421` 监听 TCP `127.0.0.1:37421`,行分隔 JSON:

| 命令 | 请求 | 响应 |
|---|---|---|
| `get_status` | `{"cmd":"get_status"}` | `{"ok":true,"current_path":"...","active_dialogs":N}` |
| `set_path` | `{"cmd":"set_path","path":"..."}` | `{"ok":true}` |
| `quit` | `{"cmd":"quit"}` | (服务端关连接) |

### 4.2 Phase 2a(TCP,先稳)

- 复用 spike 已有 TCP,Sigma-side 用 `tokio::net::TcpStream`
- 优点:spike 不用改,Sigma-side 改完即可
- 缺点:localhost:37421 可能被其他进程占用,Windows Firewall 弹窗

### 4.3 Phase 2b(命名管道,推荐)

**为什么不现在直接上 named pipe**:
- spike 0.x 状态稳定,改 IPC 风险大
- 先用 TCP 验证完整链路(Phase 2a)
- Phase 2b 把 spike 改 named pipe + Sigma-side 同步改

**named pipe 优势**:
- Windows 原生,无需 TCP 端口
- ACL 限制:只允许当前用户 + Sigma PID 访问(更安全)
- 不被 firewall 拦截
- 重启 Sigma 不需要担心 TIME_WAIT / 端口冲突

**协议建议**:
```rust
// spike side
const PIPE_NAME: &str = r"\\.\pipe\sigma-listary-spike-v1";

// Sigma side (Windows)
let client = tokio::net::windows::named_pipe::ClientOptions::new()
    .open(PIPE_NAME)?;
```

**Sigma-side IPC client 抽象**(让 Phase 2a/b 共用):
```rust
#[async_trait]
trait IpcClient: Send + Sync {
    async fn send(&self, cmd: serde_json::Value) -> Result<serde_json::Value, String>;
    async fn close(&self) -> Result<(), String>;
}

struct TcpIpc { stream: Arc<Mutex<TcpStream>> }
struct NamedPipeIpc { pipe: Arc<Mutex<NamedPipeClient>> }

#[async_trait]
impl IpcClient for TcpIpc { /* ... */ }
#[async_trait]
impl IpcClient for NamedPipeIpc { /* ... */ }
```

### 4.4 新增 ping 命令(spike-side 改动最小)

spike 加一个 `ping` 命令(零成本健康检查):

```rust
"ping" -> Ok(json!({"ok":true,"pong":true}))
```

Sigma-side 定时(每 30s)`invoke('spike_ping')`,spike crash 后 ping 失败 → 触发重连。

⚠️ **ping 是 spike 唯一允许的代码改动**,版权头保留。

---

## 5. Trigger 链路

### 5.1 数据流

```
用户操作(Navigator 点击 / 输入路径)
  ↓ Vue reactive
Pinia store: files.currentPath
  ↓ watch(currentPath, ...) 触发
listary.syncPath(newPath)
  ↓ invoke('spike_set_path', { path: newPath })
Rust backend: spike_set_path 命令
  ↓ IPC send
spike.exe: tcp_server / named_pipe handler
  ↓ state.current_path = path
uia_event.rs: 当前 foreground dialog 检测
  ↓ PathWrap monitor.rs 识别 Win32 / Chromium dialog
uia_inject.rs: SetValue("filename_edit", path) + Invoke("OK")
  ↓ dialog 跳转到 newPath
```

### 5.2 路径推送策略

**push 频率**:
- Sigma 当前路径变化触发(每次切目录)
- 防抖 100ms(避免短时间内多次触发,如快速 tree 浏览)
- **不**做去重(spike 端按最后 set_path 覆盖即可)

**路径格式**:
- Sigma 端传 `C:\Users\Public`(Windows 反斜杠)
- spike 端 `BSTR::from(path)` 适配
- **不**做路径归一化(spike 端 uia_inject 会处理)

### 5.3 Sigma 端 toggle 开关

- 默认开启(用户首选项)
- 设置面板加 toggle:"Listary-style focus sync (Experimental)"
- 关闭后:不自动 spawn spike,已有 spike 也 kill
- 状态存 Pinia + 持久化到 `%APPDATA%\sigma-file-manager\settings.json`

---

## 6. 错误恢复 + 重连策略

### 6.1 spike crash 场景

| 场景 | 检测 | 恢复 |
|---|---|---|
| Sigma 启动时 spike spawn 失败 | spawn 返回 Err | 记录 warning,功能降级(setting 里显示 spike 未运行) |
| Sigma 运行中 spike crash | ping 失败 / IPC send Err | 自动 respawn(最多 3 次),第 3 次后告警用户 |
| 用户手动 kill spike | ping 失败 | 检测到 spike 未运行,等用户 toggle 开关再 spawn |
| spike 端口冲突 | spawn 成功但 IPC connect 失败 | 用下一个端口重试(37422, 37423, ...)或 named pipe |

### 6.2 IPC 错误

- `set_path` 失败 → 静默丢弃,Pinia 状态不变(用户下次 syncPath 再试)
- `get_status` 失败 → 标记 `running=false`,前端可以显示"spike 未运行"
- **不**做 panic / 重启 Sigma(spike 是 optional daemon)

### 6.3 重连实现(伪代码)

```rust
async fn ensure_spike_alive(state: &SpikeState) {
    let mut backoff = 1u64;
    loop {
        if state.ping().await.is_ok() { return; }
        match state.spawn_spike().await {
            Ok(_) => {
                tracing::info!("spike respawned");
                return;
            }
            Err(e) if backoff <= 3 => {
                tracing::warn!("spike respawn failed (attempt {}): {}", backoff, e);
                tokio::time::sleep(Duration::from_secs(backoff.pow(2))).await;
                backoff += 1;
            }
            Err(e) => {
                tracing::error!("spike respawn exhausted: {}", e);
                // 通知前端:spike 不可用,功能降级
                return;
            }
        }
    }
}
```

---

## 7. 打包(NSIS sidecar)

### 7.1 Tauri sidecar 配置(`tauri.conf.json` 修改)

```json
{
  "bundle": {
    "externalBin": [
      "binaries/spike-x86_64-pc-windows-msvc.exe"
    ],
    "windows": {
      "nsis": {
        "installMode": "currentUser"
      }
    }
  }
}
```

`externalBin` 会让 Tauri 在打包时把 `binaries/spike-x86_64-pc-windows-msvc.exe` 重命名为 `spike.exe` 并放到 `resources/` 目录。

### 7.2 binaries/ 目录布局

```
sigma-file-manager/
├── src-tauri/
│   ├── binaries/
│   │   ├── spike-x86_64-pc-windows-msvc.exe   ← 从 sigma-listary-spike/target/release/spike.exe 拷贝
│   │   └── spike-x86_64-pc-windows-msvc.exe.sig  ← Tauri's signpath signature(可选)
│   └── tauri.conf.json
```

### 7.3 Sigma-side spike 路径解析

```rust
fn spike_exe_path() -> PathBuf {
    // 开发模式
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..").join("..").join("sigma-listary-spike")
        .join("target/release/spike.exe");
    if dev_path.exists() { return dev_path; }
    
    // 打包后(resources/)
    tauri::api::path::resource_dir()
        .expect("resource dir")
        .join("spike.exe")
}
```

### 7.4 签名(可选 Phase 2d)

- 用 `.signpath` 流程(项目已有,见 `sigma-file-manager/.signpath/`)
- spike.exe 单独签名(独立 release artifact)
- 不在 Phase 2a/b/c 必做项

---

## 8. Rollout 计划

### 8.1 Phase 2a: Tauri command + TCP(2-3 周)

- Sigma-side Rust:commands/spike.rs + main.rs 集成
- Sigma-side Frontend:Pinia listary store + Navigator.vue watch
- **保留** spike 现有 TCP(不改 spike)
- 本地手动 build + 端到端测试
- v2.3.0-focus.2 fork release

### 8.2 Phase 2b: named pipe(1 周)

- spike 加 named pipe listener(spike 唯一允许的代码改动)
- Sigma-side 加 NamedPipeIpc impl
- Tauri 启动时检测:`spike --named-pipe` 命令行参数启用 pipe,fallback TCP
- v2.3.0-focus.3 fork release

### 8.3 Phase 2c: UWP / XAML 扩展(1-2 周,可选)

- spike 加 `is_xaml_host_hwnd` 门控(从 QwenLM 完整移植)
- fg_bypass 覆盖 UWP
- 扩大测试矩阵到:Excel / Photos / OneDrive 等 UWP app
- v2.3.0-focus.4 fork release

### 8.4 Phase 2d: spike auto-update(可选)

- Sigma 检测 spike 版本 + 自动下载替换
- 不在 Phase 2a/b/c 必做项

---

## 9. 测试

### 9.1 单元测试

- `commands/spike.rs::tests`:mock Child / mock IPC client,验证 spawn/kill/set_path 命令流
- `IpcClient` trait:TCP 和 NamedPipe 实现各 3 个测试(connect / send / close)

### 9.2 集成测试(本地)

- 启动 spike + Sigma,触发 Navigator 切路径,验证 spike 收到 set_path
- spike crash 后 Sigma 自动 respawn
- 模拟端口冲突,验证 fallback 逻辑

### 9.3 GUI 测试(手动)

完整清单:见 `docs/superpowers/test-plans/2026-09-27-listary-focus-sync-manual-checklist.md`。
要点:
- 7-app 矩阵 5/7 = 71% 通过即 PASS
- spike crash 恢复:手动 kill spike.exe,Sigma 自动重启并恢复功能
- Toggle off:用户关闭功能,spike 被 kill

### 9.4 Spike 端不变

- spike 单测保持 11/11 pass(state 2 + config 3 + events 3 + logger 3)
- spike TCP smoke test 保持 4/4 pass
- spike 不接受新功能,除非 Phase 2b/c 必要时

---

## 10. 风险与权衡

### 10.1 已知风险

| 风险 | 严重度 | 缓解 |
|---|---|---|
| spike 与 Sigma 生命周期耦合(Sigma 退 spike 必须退) | 中 | on_window_event CloseRequested 钩子 + 兜底 timer(60s 后强 kill) |
| TCP localhost:37421 被其他进程占用 | 低 | 自动 fallback 到 37422 / 37423,或切 named pipe(2b) |
| spike.exe 不在 resources/(用户漏装) | 中 | Sigma-side detect 失败时给用户明确错误("spike.exe not found, 请重装") |
| UWP / XAML app 的 dialog 不被覆盖 | 低(本 spec 范围外) | Phase 2c 单独处理 |
| SmartScreen 警告(未签名) | 中 | README 解释,phase 2d 加签名 |
| spike.exe 反病毒误报(非常规文件位置 + 子进程行为) | 中 | spike 用正规路径(`resources/spike.exe`),不是 `C:\Windows\Temp` |

### 10.2 不做的事

- ❌ DLL 注入(AV / WDAC 风险,Plan §1 禁止)
- ❌ 修改 spike 现有 3 个移植文件(版权头固定)
- ❌ 用 IFileDialog COM 走老路线(spike 路线更通用)
- ❌ 跨进程共享内存(过度设计,TCP/named pipe 足够)
- ❌ Phase 2a 阶段就上 named pipe(spike 改动风险高)

### 10.3 与上游 PR 策略对齐

- Phase 2a fork release:`v2.3.0-focus.2`(kizemo fork 独立)
- 上游 PR:等 v2.3.0-focus.2 稳定后,提 PR 到 `aleksey-hoffman/sigma-file-manager`,附 spike 报告 + 决策文档
- 上游不合并:保留 fork release,与 `v2.2.0-tree.1` 并行
- **不**主动推到 upstream 的 main(见 README § Credits)

---

## 11. 引用

- Spike 决策:`docs/superpowers/decisions/2026-09-27-listary-focus-sync-decision.md`
- Spike 报告:`docs/superpowers/spike-reports/2026-09-27-listary-focus-sync-spike.md`
- Spike 源码:`sigma-listary-spike/`
- 手动测试清单:`docs/superpowers/test-plans/2026-09-27-listary-focus-sync-manual-checklist.md`
- 移植来源:`docs/superpowers/plans/2026-09-27-listary-focus-sync.md` §六
- 上游 PR 策略:`docs/superpowers/specs/2026-09-26-upstream-pr-strategy-design.md`

---

## 12. Out of scope(继续不做)

- macOS / Linux(UIA / AXUIElement 是 Windows 专有)
- DLL 注入(AV / WDAC 风险)
- 改 spike 已 commit 的 3 个移植文件(版权头固定)
- 上游自动合并(PR 流程走人工评审)
- sigma-side 用 JS 而不是 Rust 实现 IPC(性能 + STA 约束)

---

## 13. 下一步(新会话承接时)

1. **实施 Phase 2a**:Sigma-side Rust commands/spike.rs + Frontend Pinia
2. **本地 build + 集成测试**:手动跑 `docs/superpowers/test-plans/2026-09-27-listary-focus-sync-manual-checklist.md`
3. **通过 7-app 矩阵后**:决定是否切 Phase 2b(named pipe)
4. **v2.3.0-focus.2 fork release**:Phase 2a 完成后发版
5. **PR 上游**:v2.3.0-focus.2 稳定后提 PR