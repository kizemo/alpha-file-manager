# 批次 22 入口卡 —— 夸克网盘同步：⑪ 真机计数级收口

生成时间：2026-11-19（count7 判词落盘后）
会话：`mvs_e134df2d929344cfa83d94c693cafa93`
状态：**⑪ 已收口 —— count11 判词 PASS（`w1=1 w2=0`）**。本卡是批次 22 的起点记录；收口结论、逐轮根因链与发布面更正见台账 `docs/handoff-2026-11-11-quark-pan.md` **§20.18**。

---

## 〇、结论一览

| 项 | 结论 | 证据 |
| --- | --- | --- |
| 批次 22 唯一硬阻塞 | ⑪「同一目标目录连续 push 两次只被侧车写一遍」的**真机计数级**取证 | `docs/handoff-2026-11-11-quark-pan.md` §20.15 / §20.16 |
| count7 判词 | `VERDICT=UNDETERMINED w1=0 w2=0`（控制台退出码 0） | `.superpowers/sdd/focus/b20-dup5/console.log` 末行 `[00:06:47]` |
| v0.6.4 去重闸门 | **至今从未在有效取证条件下被行使过**（v3 之后所有轮次均非判定轮） | §20.17.2 + 源码逐行对照 |
| count7 根因 | 弹窗窗在 push 那一刻**已死** ⇒ 侧车走 dead-HWND 静默剪枝分支 ⇒ 既不 `Write` 也不打决策日志 | `extensions/kizemo.focus-sync/sidecar/src/http_server.rs:470-477` |
| 是否产品回归 | **不是**。属既有根因③「前台往返后焦点重建失败」家族的晚期形态，是**夹具/送达**问题 | §20.17.2 |
| 下一步 | count8 三件改动（见 §四），全部满足后若仍 `writes=0` 才回到产品侧排查 | §20.17.4 |

---

## 一、继承状态（批次 21 已闭合项 + 本轮两处更正）

### 1.1 批次 21 账面（全绿，无需重做）
- 3.14.1 原地重建：`45,489,382 B`，sha `5F07F5B9…`（详见台账 §20.16）。
- `npm run check:winget` 8 项绿；装机 `INSTALL_EXIT=0` + `VERIFY_OK reason=install-refreshed-appdata-sidecar`。
- 装机 exe 归一化后 sha == 裸构建产物（bundle-type 标记差异已按 §2.10 归一化处理）。
- 文档 HEAD = `18f54134`；**批次 21 / 22 文档尚未提交推送**。

### 1.2 本轮更正 C —— 送达哨兵用错了字符串
`set_path_received` 在侧车源码全目录 **grep 0 命中**，历史上把它当「push 已送达」哨兵的推理链**全部作废**。
真送达证据只有 spike JSON 事件流里的：`path_update` / `Write` / `WriteFailed`。

### 1.3 本轮更正 D —— v4 / v6 的 `w1=0` 不是「未送达」
直接原因是**计数文件选错**：脚本回退读了空文件 `…\b20-dup5\spike.log`，而真事件流在
`C:\Users\Duanyi\AppData\Local\kizemo\focus-sync\logs\spike.log`（`sidecar/src/logger.rs:56-62` 硬编码 `LOCALAPPDATA`）。
因此此前两条推断**同时不成立**：「push 未送达」不成立、「装机版不含修复」也不成立。

> 由此得出一条通用姿势：**跑磁盘版取证脚本时，必须显式设 `ALPHA_FFM_SPIKE_LOG` 指向 LOCALAPPDATA 真事件流**，否则计数判据读空文件、`writes=0` 毫无判别力。

---

## 二、实验环境坐实（照抄可用）

- 侧车身份：端口 `37421` 监听进程，exe sha8 = `480FDE62` ⇒ 坐实为 **v0.6.4 仓库产物 = 装机侧车**（pid、启动时刻见台账 §20.17.1 环境表）。
- **`/health` 返回的 `0.6.1` 无判别力**：`http_server.rs:134` 的 `SPIKE_VERSION = "0.6.1"` 是**从未 bump 的硬编码常量**。版本认定一律以 sha 为准（AGENTS.md §2.10）。
- 运行中 AFM 为新载荷：用户于 `2026-11-18 23:04:56` 重启（pid `1012796`，D 盘 3.14.1 安装路径）。旧 pid 的处置问题**已失效**；双实例并存不影响本轮判词有效性。
- 夹具是 **C# 不是 Rust**：`.superpowers/sdd/focus/b20-dialog-helper2.cs`（26,236 B）+ 同名 `.exe`（19,456 B）。
- 实验纪律：**不 spawn 新侧车、不动用户 AFM、不杀用户进程**；每轮必须**新目录 + 新 run.log + 新 result 文件**（防 provenance 污染）。

---

## 三、本轮动作与判词（count7）

### 3.1 有效启动命令（EXIT=0，可直接照抄改 `-Out` 为 `b20-dup6`）
```powershell
$env:B20_AFM_VISIT='1'
$env:ALPHA_FFM_SPIKE_LOG='C:\Users\Duanyi\AppData\Local\kizemo\focus-sync\logs\spike.log'
$env:ALPHA_FFM_TEST_TIMEOUT_MS='150000'
$env:B20_VISIT_TIMEOUT_MS='20000'
& 'C:\Users\Duanyi\AppData\Local\Microsoft\WindowsApps\pwsh.exe' -NoProfile -File `
  'F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\focus\b20-dup-count4.ps1' `
  -Port 37421 `
  -Out 'F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\focus\b20-dup6' `
  2>&1 | Out-File -FilePath 'F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\focus\b20-dup6\console.log' -Encoding utf8
```
> 注意：result JSON 与 run.log 由脚本内部**固定写 `$FocusDir`（`.superpowers\sdd\focus\` 根目录）**，不跟 `-Out`。count7 实际产物是 `focus\b20-dup-result4.json`（改前的旧副本已备份为 `focus\b20-dup-result4.pre-count7.bak.json`）。

### 3.2 判词读法（口径**不可改**）
按 result JSON 里的 `segment1Raw/segment2Raw` 偏移，对真 spike.log **切片**读（大文件用
`[System.IO.File]::Open(...,'Open','Read','ReadWrite')` + `Position=offset` + `StreamReader`）：
- 段内 `event=="Write"` 且 target **归一化后** == `c:\windows\system32` 才计数；
- `WriteFailed` **不计入**；
- 期望：push1 段 `w1=1`、push2 段 `w2=0`。
- count7 实测切片 `8,826,277 → 8,827,373`，与 result JSON 记录一致 ⇒ 修正版 slice 逻辑本身已生效。

### 3.3 切片内容（逐字）
- push1：`path_update current_path=C:\Windows\System32 (pushed='C:\Windows\System32')` + `set_path_dialogs dialogs_count=1`，**之后零决策日志**（无 `set_path_skip_no_handoff` / `already_synced` / `deferred_not_foreground` / `set_path_iter` / `Write` / `WriteFailed`）。
- push2：`path_update C:/Windows/System32` + `set_path_dialogs_retry retried 3x; final dialogs_count=0` + `set_path_no_dialog "no active dialog after waiting 60ms; path stored, nothing written"`。

### 3.4 根因（对照 `http_server.rs:412-626` 决策链逐行）
1. 两次 push **都送达活侧车**（各有 `path_update`）。push2 的 HTTP **404 是 v0.5.23 起 `no_active_dialog_response()` 的设计形状**（`:444-455` 空表分支）⇒ **404 ≠ 没送达**。
2. push1 时刻登记表确有 1 条（`:426` `wait_for_active_dialogs` ⇒ `:470` 遍历），但源码里「做了事却不写 JSON」的**唯一分支**是 **dead-HWND 静默剪枝**（`:470-477`，只发 `tracing::info`，**不进 spike JSON**）⇒ 判定：**push 那一刻对话框窗已死**。
3. 时序根因在脚本侧：`rcv.dialogback ok=False fg_is_dlg=False`（前台交回失败）之后，脚本仍继续 `Start-Sleep -Seconds 5` + 0.5 s 才 push（`.superpowers/sdd/focus/b20-dup-count4.ps1:369` / `:377`）⇒ **5.5 s 足够弹窗死亡/被剪枝**。

---

## 四、count8 配方（三件改动，全部落在夹具/脚本侧）

> 硬约束：**不得删掉** `b20-dialog-helper2.cs:464-527` 里 `FgWorker` 的「900 轮 × 100 ms 无条件 `ForceForeground(dialog)`」循环 —— 它是 push2 时刻弹窗仍在前台、从而能走到 `already_synced`（`:538-550`）的支柱。只收紧「交回成功的判定」。

| # | 改动 | 位置 | 判据 |
| --- | --- | --- | --- |
| ① | `FgWorker` 写 `dialog.raised` 前**轮询确认** `GetForegroundWindow()==d` 才算成功（现在 `ok` 取单次 `ForceForeground` 返回值） | `focus/b20-dialog-helper2.cs`（`dialog.raised` / `dialog.raise` / ASFW 握手：`:382-424`、`:464-527`） | 交回文件里的 `ok` 与真实前台一致 |
| ② | 脚本在 `dialogback ok=False` 时**不得继续 5.5 s 流程**：立即重触发交回、有界重试；只有 `ok=True` 才 push，否则**直接判 UNDETERMINED** | `focus/b20-dup-count4.ps1:369`、`:377` | 不再出现「交回失败仍 push」 |
| ③ | push1 **之前**脚本自证 `GetForegroundWindow()==dialog_hwnd`（把哨兵写进 run.log） | `focus/b20-dup-count4.ps1` push1 前 | 前置条件不成立即本轮作废 |

- 重编译 `.cs` 夹具（`csc` / `dotnet` 均可，产物覆盖 `b20-dialog-helper2.exe`），**新开 `focus/b20-dup6\` 目录**跑 count8。
- AGENTS.md §2.8（第 647 行）：⑪ 属**文档化偏离** —— 私有桌面 `GetForegroundWindow` 恒 0，只能在交互桌面做；夹具窗口做小、判完立即关、退出恢复原前台（脚本 `finally` 已内置，勿删）。
- **分流规则**：三条改动全部满足后若仍 `writes=0`，才回到产品侧（v0.6.4 写闸门 `:502` `afm_gate_allows` / `:538-550` 去重 / `:612-619` `write_path`+`Event::Write` / `state.rs` 路径 A/B）排查；否则一律登记为夹具问题，**不得写成产品回归**。

---

## 五、状态矩阵

**已验证（有判词/字节级证据）**
- 侧车身份 sha8 `480FDE62` = v0.6.4 装机产物；两次 push 均送达。
- count7 判词 `UNDETERMINED w1=0 w2=0`；切片偏移与 result JSON 一致。
- `SPIKE_VERSION` 常量无判别力；404 是设计形状。
- 批次 21 构建/装机/winget 账面全绿。

**推断（源码逐行支持，未被真机判别性测试确认）**
- push1 时对话框窗已死 ⇒ 走 `:470-477` dead-HWND 静默剪枝（唯一「不写 JSON」分支）。
- 交回失败 + 5.5 s 睡眠是弹窗死亡的直接时序成因。

**未验证 / 未判定（不得当结论用）**
- ⑪ 真机计数级收口：**未判定**。v0.6.4 去重闸门从未在有效取证条件下行使过。
- v3 的 `w1=2 w2=0` 是 **v0.6.3 修复前的缺陷基线**，不是回归证据；v4/v5/v6/v7 一律 UNDETERMINED。
- 对外发布链三项（Release 资产替换 / winget PR / 文档提交推送）—— 均需用户授权，两轮问句未答。

---

## 六、遗留项

1. **⑪ 闭合**（本轮主线，见 §四）。
2. 批次 21 + 22 文档**未提交未推送**（`git -C F:\soft\00selfmade\filemanager\sigma-file-manager status --porcelain`；HEAD `18f54134`）。push 若报 TLS EOF，用一次性绕过代理：`git -C <repo> -c http.proxy= -c https.proxy= push origin main`（勿改全局 config）。
3. GitHub Release v3.14.1 资产**未替换**为新 sha `5F07F5B9…`；winget PR 前置 #467381 仍 OPEN。
4. `release/` 被 gitignore；未经用户授权不得对外发布、不得提交/推送、不得杀用户进程（AGENTS.md §2.9）。

---

## 七、下一轮阅读顺序（不在此复述内容，请自行掌握背景/目标/进度/计划）

1. `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-18-quark-pan-batch22.md`（本卡）
2. `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-11-quark-pan.md` §20.15 / §20.16 / **§20.17**（台账真源，本轮全部细节与证据路径在此）
3. `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-18-quark-pan-batch21.md`（批次 21 卡：构建配方 + 前一轮更正）
4. `F:\soft\00selfmade\filemanager\sigma-file-manager\AGENTS.md` §2.8（交互桌面偏离）、§2.9（发布/授权约束）、§2.10（禁用假判据）
5. 判词原文：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\focus\b20-dup5\console.log`、`...\focus\b20-dup-result4.json`
6. 源码判据点：`F:\soft\00selfmade\filemanager\sigma-file-manager\extensions\kizemo.focus-sync\sidecar\src\http_server.rs`（134 / 412-626）、`...\sidecar\src\state.rs`、`...\sidecar\src\logger.rs:56-62`
7. 夹具：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\focus\b20-dialog-helper2.cs`（382-424 / 464-527）、`...\focus\b20-dup-count4.ps1`（369 / 377）

---

## 八、实测失败姿势（勿再试）

- 把 `set_path_received` 当送达哨兵（源码无此串）。
- 读 `spike.log.2026-11-18` 日滚文件计数（纯文本，JSON 判据 0 命中）。
- 不设 `ALPHA_FFM_SPIKE_LOG` 就跑磁盘版脚本 ⇒ 回退读空文件，`writes=0` 无判别力。
- `New-Object System.Windows.Automation.TreeScopeCondition`（该类型不存在）；对话框枚举改用 `PropertyCondition(ControlTypeProperty, Dialog)`。
- 对整份 exe `Select-String` 判版本（§2.10 禁止）；按 `-Out` 目录猜 result JSON 位置（实际写 `$FocusDir`）。
- `Select-String -LiteralPath (Get-ChildItem ...).FullName` 空数组绑定错误。
- 一次 shell 调用连发多条大输出，会被 hook 上下文挤掉整段 —— 一律写文件、对话留指针。
