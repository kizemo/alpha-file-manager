# focus-sync 扩展缺陷修复记录（下载/另存为弹窗路径不跟随 AFM）

- 日期：2026-11-06
- 涉及组件：`extensions/kizemo.focus-sync/sidecar`（Rust 侧车，crate 名 `sigma-listary-spike`，lib 名 `spike`）
- 侧车版本：0.5.15 → **0.5.16 → 0.5.17 → 0.5.18**
- 本轮**不提交任何 commit**（沿用「4 个侧车文件刻意不提交」约束）。

## 一、现象

从 Alpha File Manager（AFM）切到「下载」/「另存为」弹窗后，弹窗里的当前目录**不跟随** AFM 的路径。

## 二、根因链（三层，逐层坐实）

### 第 1 层（v0.5.16 修复）：reader 解析不出本地化显示名 ⇒ 写回被静默跳过

Windows 的「下载」是**本地化特殊文件夹**，地址栏 UIA Name 形如 `地址: 下载`（显示名，不是路径）。
`reader::parse_address_label` 只接受「路径形状」的字符串 ⇒ 读路径失败 ⇒
`state::should_write_back_when_path_unknown` 把这次写回**静默跳过** ⇒ 界面纹丝不动、**日志只有一条 skip、没有任何错误**。

落地：
- `state.rs`：新增 `normalize_path` / `paths_equal` / `should_write_back_when_path_unknown`。
- `main.rs`：按 `read_ok` 分流；新增 `opened_write_back_skip_path_unknown` 日志；`Event::Read` 增加 `strategy` 字段。
- `uia_event.rs`：路径比较归一化后再判。
- `reader.rs`：新增 `read_path_with_strategy(hwnd) -> Result<(String, &'static str)>`，UIA 失败回落 COM 通道。
- `uia_inject.rs`：新增 `read_current_folder_via_com`（AccessibleObjectFromWindow → IServiceProvider → QueryService(SID_STopLevelBrowser) → IFileDialog::GetFolder → IShellItem::GetDisplayName(SIGDN_FILESYSPATH)）。
- `http_server.rs`：版本 → 0.5.16。

### 第 2 层（v0.5.17 修复）：COM `SetFolder` 对**已打开**的对话框「返回成功但不改目录」

`IFileDialog::SetFolder` 只设置 `Show()` **之前**的初始目录。对已经显示的原生 `IFileDialog`，
它**照样返回 S_OK**，但当前目录完全不变 ⇒ 旧代码在这里直接 `return DialogOutcome::ComSetFolder`，
日志记 `Write`、界面不动。

真机证据：516g 第一次运行 —— BEFORE 地址栏 `地址: C:\Users\Duanyi\Downloads`，
AFTER 仍是 Downloads，但日志有 `Write target=… strategy=uia`。

落地：
- `uia_inject.rs`：新增 `verify_dialog_folder(hwnd, target, attempts)`（3 次 × 200ms 重试，
  用 `reader::read_path_with_strategy` 回读 + `state::paths_equal` 比较）；
  `try_set_folder_via_com` 改为「`SetFolder` 成功**且**回读等于目标」才算成功，否则 warn 并**回落 UIA 地址栏路径**。
- `writer.rs`：加**统一写后回读校验**，不通过 ⇒ 降级为 `WriteOutcome::Failed("write_not_verified")`
  （保持对话框可重试，沿用 v0.5.12 语义；**不**误 bump `unsupported_dialog_count`、**不**误报「不支持的对话框类型」）。
- `http_server.rs`：版本 → 0.5.17。

### 第 3 层（v0.5.18 修复）：COM 调用线程从未 `CoInitializeEx` ⇒ 整个 COM 通道长期静默失效

用生产函数 `read_current_folder_via_com` 对真实原生对话框做跨进程读取 ⇒ 返回 **NONE**；
开 tracing 后仍**没有任何 warn** ⇒ 失败发生在**第一步 `AccessibleObjectFromWindow`**。

根因：侧车的 COM 调用发生在 tokio worker 线程与 dialog monitor 线程上，
这两个线程都**没跑过 `CoInitializeEx`** ⇒ COM 调用以 `CO_E_NOTINITIALIZED` 失败；
而第一步**连日志都没有** ⇒ 「静默失效」看起来跟「这个对话框不是原生 IFileDialog」一模一样。

落地：
- `uia_inject.rs`：新增 `struct ComGuard`（`CoInitializeEx(COINIT_APARTMENTTHREADED)`；
  `hr.is_ok()`（S_OK/S_FALSE）时在 `Drop` 里 `CoUninitialize`；`RPC_E_CHANGED_MODE` **不**反初始化，
  否则会破坏线程原有属主的 COM 状态）。
  接入 `read_current_folder_via_com` 与 `try_set_folder_via_com` 两个函数体首行。
- `uia_inject.rs`：`AccessibleObjectFromWindow` 失败分支补 `warn!`（含 hwnd）——
  「静默失败比崩溃更难查」，这是本轮定位耗时的主因。
- `http_server.rs`：版本 → 0.5.18 + 中文变更说明注释块。

#### 第 3 层的最终结论：COM 通道对「独立进程的侧车」在架构上不可用

v0.5.18 上线后复验（`verify-516h.ps1`，真机 classic shell「另存为」对话框）：

```
[com-read] AccessibleObjectFromWindow failed for hwnd=1904408
           err=Error { code: HRESULT(0x80004005), message: "未指定的错误" }
```

错误码从 `CO_E_NOTINITIALIZED (0x800401F0)` 变成 `E_FAIL (0x80004005)` ⇒
**ComGuard 确实生效了**（COM 已初始化），但下一步仍然失败。

`E_FAIL` 的含义是：`AccessibleObjectFromWindow(hwnd, OBJID_NATIVEOM, …)` 取的是
窗口的**原生对象**，这个对象指针**不可跨进程封送** —— 侧车与对话框分属两个进程，
拿不到对方的 `IFileDialog`。

⇒ **结论：COM `IFileDialog` 通道要求「与对话框同进程」，对独立进程的侧车不可用。**
这不是缺一个 `CoInitializeEx` 就能解决的问题（那是 v0.5.18 修掉的真 bug，
但它只是让失败从「静默」变成「可诊断」，并没有让通道变得可用）。

⇒ 因此本项目的**唯一可行写入通道是 UIA 地址栏**（真机已验证：经典 shell 与
Edge/Chromium 两类弹窗都能真正切换目录）。
⇒ 「读不到路径」的情况由 v0.5.16 的降级层兜住：**读不到 ≠ 不该写**
（`state::should_write_back_when_path_unknown`，按 `last_written_path` 去重后照写）。

**保留 ComGuard 与新增 warn 日志的理由**：它们修掉了一个真实的 COM 初始化缺陷，
并把「COM 通道不可用」从「静默、无任何日志」变成「每次都有明确 warn」。
这是**可诊断性**的胜利，不是通道可用性的胜利 —— 后者在本架构下无法实现。

## 三、写入判定必须可证伪（贯穿三轮的共同原则）

任何通道报「成功」都必须通过**回读对话框当前目录**与目标路径比较（归一化后）。
不通过一律降级为 `write_not_verified`：**保持对话框可重试**，不 bump `unsupported`，不误报用户通知。

路径比较一律归一化（分隔符 / 尾缀 / 大小写），复用 `state::paths_equal`。

**降级层保留**：读不到路径时按 `last_written_path` 去重后照写，**绝不猜路径**。

## 四、安全契约（未变）

- H1 只做**导航**：Ctrl+L → 写地址栏 → 回读守卫 → 受控 Enter。
- **绝不** `commit_save_button`，**绝不**自动点「保存」。
- 不回退 v0.5.1「下载直接启动」事故。
- **禁止**跨进程发送 `CDM_GETCURRENTPATH`（`WM_USER+` 不做跨进程内存封送）。

## 五、真机验收结果

| 项 | 结果 |
|---|---|
| 经典 shell 另存为弹窗（repro 工具） | BEFORE `地址: C:\Users\Duanyi\Downloads` → AFTER **`地址: F:\soft\00selfmade\dedao_dl\extension`** ✅ 目录真的切换 |
| 侧车日志 | `opened_write_back` → `Write target=… strategy=uia`；`unsupported=0`；无 `write_not_verified` |
| Edge/Chromium 弹窗回归 | AFTER 地址栏 = `地址: F:\soft\00selfmade\dedao_dl\extension` ✅ |
| 文件名污染 | `afm-repro.html` 未被污染（只导航不提交保存）✅ |
| 闩语义 | 未访问 AFM 时连续 4 次 `opened_write_back_skip_no_afm_visit`（0 次写）；聚焦 AFM→聚焦弹窗后恰好 1 次写 ✅ |

侧车 `current_path` = `F:\soft\00selfmade\dedao_dl\extension`（**扩展真实推送值**，非人工模拟值）。

## 六、遗留 / 未验证

- COM 读通道：v0.5.18 上线后仍读不出路径，**原因已钉死为跨进程限制**（见「第 3 层的最终结论」）。
  该通道保留为「同进程才有意义」的备用实现 + 诊断日志，不再作为可用路径依赖。
  ⇒ 后续**不要**再试图靠 `CoInitializeEx` / 换 `IFileDialog` 获取方式修它。
- `strategy=com` 回落分支在真机上**永远不会被触发**（COM 通道架构上不可用），
  `Event::Read.strategy` 实际只会是 `uia`。该字段与 `read_path_with_strategy` 的
  COM 分支属于「防御性保留」，不是活路径。
- `opened_write_back_skip_path_unknown` 降级层在真机上**未被触发**
  （Windows 11 经典 `IFileDialog` 与 Edge Chromium 弹窗的地址栏 UIA Name 都是**完整路径**
  `地址: C:\Users\Duanyi\Downloads`，不是本地化名「下载」）——目前仅由单测覆盖。
  这条降级层正是本地化显示名场景（`地址: 下载`）的兜底，**逻辑上已就位**，
  只是本机无法自然构造出该状态。
- 临时工具 `extensions/kizemo.focus-sync/sidecar/src/bin/repro_save_dialog.rs`（未跟踪）是否提交，交由用户决定。

## 七、3.12.0 交接文档「下一步」清尾状态（2026-11-06 本轮一并处理）

| # | 事项 | 状态 |
|---|---|---|
| 1 | 手工删除验收夹具 `C:\Users\Duanyi\afm-accept-3120` | **内容已删**（5 个文件 + `dest\demo.txt - Shortcut.lnk` 全部进回收站）。**两个空目录 `afm-accept-3120\` 与 `afm-accept-3120\dest\` 仍在** —— 本机 trash 路由拒收工作区外的**目录级**删除目标（文件可以、目录不行），按「恢复性删除失败就如实报告、不得绕过」处理，未改用永久删除。⇒ 需用户手工删这两个空目录 |
| 2 | `.acc3120-backup` / `.acc3120-icons-backup` 备份文件 | **已不存在**（核对为 GONE） |
| 3 | 公网安装包逐字节 SHA256 回验 | **已完成**：下载 `Alpha_File_Manager_3.12.0_x64-setup.exe`（45,112,805 B，127 s）→ SHA256 = `c3eb2c5d870a565aed87da012e9bcc0256d0fd9eb4583562c024692c9f5c302e`，与 `version.json` 公布值**逐字节一致**。本地 `release\` 下两份原件（带空格 / 下划线命名）hash 亦相同 |
| 4 | 重核 3.12.0 新包内侧车 sha256 | **已完成**（并查出一个重要事实，见下） |
| 5 | winget PR | 本轮**刻意未提交**（沿用既定决策）；yaml 已在仓库内 |

#### 第 4 项查出的重要事实：公网 3.12.0 包里的侧车**仍是有 bug 的 0.5.15**

- 7-Zip 从安装包解出 `tools\focus-sync-sidecar.exe`：2,841,600 B，
  SHA256 = `b061d8a25445f29ed1da7597e969700b9cd9ffdc0fc1191a2a9a3171373a1a7d`。
- 二进制内嵌版本串扫描命中 `0.5.15`（`SPIKE_VERSION` 常量）⇒ **公网包 ships 的是 0.5.15**，
  即本轮修复前的版本。

⇒ **结论：修复只落在本地部署（0.5.18）与工作区源码里，尚未进任何公开发布物。**
⇒ 触达用户只有一条路：`src-tauri/tauri.conf.json:89` 把
`../extensions/kizemo.focus-sync/bin/focus-sync-sidecar.exe` 打进安装包 `tools/`，
NSIS POSTINSTALL 再拷到 `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\`。
**下一次 `tauri build` 打包即会把 0.5.18 带进去**（当前 `bin/focus-sync-sidecar.exe`
sha256 = `a7424e1273a945123bbffbd29ed04acf30722c5e9e6b5d2ea891d3519281780d`）。

#### 顺带确认：侧车二进制**不入版本库**

`.gitignore:37` 忽略 `extensions/kizemo.focus-sync/bin/`，
`.gitignore:40` 忽略 `extensions/kizemo.focus-sync/installer/*.exe`
⇒ 仓库里那两份 exe 是**构建产物**，不是版本库内容。
「重核新包内侧车 sha256」因此只能以「从安装包解出来独立取证」的方式做（已做）。

## 八、本轮改动清单（工作区，未提交）

```
 M extensions/kizemo.focus-sync/sidecar/src/http_server.rs   版本 0.5.18 + 中文变更说明
 M extensions/kizemo.focus-sync/sidecar/src/main.rs           read_ok 分流 / skip_path_unknown 日志
 M extensions/kizemo.focus-sync/sidecar/src/reader.rs         read_path_with_strategy
 M extensions/kizemo.focus-sync/sidecar/src/state.rs           normalize_path / paths_equal / should_write_back_when_path_unknown
 M extensions/kizemo.focus-sync/sidecar/src/uia_event.rs      归一化比较
 M extensions/kizemo.focus-sync/sidecar/src/uia_inject.rs      ComGuard / read_current_folder_via_com / verify_dialog_folder / folder_readback_matches
 M extensions/kizemo.focus-sync/sidecar/src/writer.rs          统一写后回读校验
?? docs/handoff-2026-11-06-focus-sync-save-dialog-fix.md      本文件
?? extensions/kizemo.focus-sync/sidecar/src/bin/              repro_save_dialog.rs（验收工具，可不提交）
```

**本轮不提交任何 commit**（沿用「4 个侧车文件刻意不提交」约束）。

## 九、证据路径

- 脚本：`F:\soft\00selfmade\filemanager\.superpowers\sdd\{build-sidecar-517.cmd,build-repro-516.cmd,verify-516f.ps1,verify-516g.ps1,verify-516h.ps1}`
- 日志：`sidecar-517-test.log` / `sidecar-517-build.log` / `repro-516-build.log`
- 截图：`516f-before.png` / `516f-after.png` / `516g-before.png` / `516g-after.png`
- 侧车运行日志：`%LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log`（JSON-lines，UTC；应用独占锁定 ⇒ 读须 `FileShare.ReadWrite`）
- 部署位置（3 处 sha256 全等）：
  - `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`
  - `extensions/kizemo.focus-sync/bin/focus-sync-sidecar.exe`
  - `extensions/kizemo.focus-sync/installer/focus-sync-sidecar.exe`
