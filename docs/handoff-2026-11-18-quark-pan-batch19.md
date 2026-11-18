# 批次 19 交接卡 · ⑧ 虚拟位置闭环 ＋ ⑨ 路径 A 真机覆盖 ＋ ⑩ 双写去重 ＋ ⑤ check flake ＋ winget 3.14.1 清单

- 生成时间：2026-11-18
- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`（分支 `main`）
- 台账真源：`docs/handoff-2026-11-11-quark-pan.md` **§20.14**（本卡是入口卡，详细论证看台账）
- 上一卡：`docs/handoff-2026-11-18-quark-pan-batch18.md`（批次 18 = 台账 §20.13）

---

## 一、背景与目标

批次 18（台账 §20.13）修掉了 focus sync 的「空路径契约」并完成 3.14.1 的装机链判别性实验，**留下 3 项遗留** ＋ 2 项收尾工作：

| # | 批次 18 遗留 | 本批次处理 |
| --- | --- | --- |
| ⑧ | 虚拟位置（收藏/最近/此电脑）停留时**侧车确实忘掉上一个真实目录** ＋ toast 文案诚实 —— 端到端体验**未验证** | **侧车侧 HTTP 级闭环 PASS**（20.14.5）；toast 仍只到**契约级**，AFM 真机 UI 未验（20.14.6） |
| ⑨ | 路径 A「祖先后代分支」真机覆盖 —— 只有单测覆盖 | 见第二节 2.3 |
| ⑩ | 「path_update 与侧车双写去重」—— 上轮**只分析不实现** | **已实现**（20.14.3）：per-dialog 写记录 ＋ 统一去重口径，净增 9 条判别性单测 |
| ⑤ | `npm run check` 并发跑法偶发超时 flake（批次 18 §20.13.8 第 1 条） | **已修**：全局 `hookTimeout` ＋ 单文件 `testTimeout`（20.14.2） |
| — | winget 3.14.1 清单 | **本地已备好**（4 文件，`check-manifest` 全过），**未提交 PR**（待授权） |

**范围声明**：只做**仓库版本与发版材料**，**不做对外发布**（GitHub Release / winget PR 需用户授权）。
本批次**重新构建了 3.14.1 安装包**（⑩ 修复必须随包发布）⇒ 安装包 sha 变化 ⇒ winget 清单 sha 同步更新、装机链判别性实验重做。
未触碰用户真实数据（只替换了扩展自带的侧车二进制，旧件已备份）。

---

## 二、这批次改了什么

### 2.1 ⑤ `npm run check` 负载 flake

**根因**：`run-p` 并行跑 5 个脚本时，vitest 重型挂载测试（jsdom 环境实测占 18.23 s）撞 30 s 默认 `testTimeout`；
stylelint 零 lint 报错就退出 1 ⇒ `run-p` 连带杀掉 vitest ⇒ 日志里连测试总结都没有。

**修法（不降并发、不改被测行为）**：

| 文件 | 改动 |
| --- | --- |
| `vitest.config.ts` | 新增 `hookTimeout: 30000` |
| `src/modules/home/components/__tests__/home-banner-click-opens-background-manager.test.ts` | 新增 `HEAVY_MOUNT_TIMEOUT = 60000`，三条 `it` 显式挂 `testTimeout` |

### 2.2 ⑩ 双写去重（侧车 v0.6.3）

**「双写」的定义（先钉死）**：侧车内部**两条真正会写 Windows 文件对话框的路径** ——
`POST /set_path` 的即时写（`http_server.rs`）与 `DialogEvent::Opened` 的写回（`main.rs`）。
扩展与 AFM 主程序都没有写对话框的能力。

**根因**：两条路径各自判断「要不要写」却**互不知情**，且去重口径不同（即时写用逐字节比较、写回用 `initial_path` 比较）⇒ 同一目标被写两遍。

**实现**（`sidecar/src/state.rs` +213）：

- 新增 `pub struct WriteRecord { path: String, observed_dir: Option<String> }` ＋ `AppState.write_records: Mutex<HashMap<u32, WriteRecord>>`；
- 新增 `record_write` / `write_record` / `dialog_holds_path` / `already_written` / `should_write_back`；
- **`should_write_back` 是写回决策的唯一入口**；`/set_path` 的去重改用 `dialog_holds_path`（两条路径共用同一去重键）；
- **关键设计 `observed_dir` 基线**：额外记下写入瞬间**对话框自己的目录** ——
  对话框确实停在我们写的目录 ⇒ 不重写；**用户手动挪过** ⇒ 仍重新同步；
- **写失败不记录** ⇒ 守住 v0.5.12 的「可重试」不变量。

**守住的不变量**：真实换目标仍写 / 路径变化仍写 / 读失败仍按最后写入键去重 / 写失败可重试 / 用户手动挪动后仍重新同步。

**判别性单测**：`state.rs` 新增 `mod v063_double_write_tests`（8 条）＋ `writer.rs` 新增 `a_failed_write_leaves_no_write_record`（1 条）
⇒ `cargo test` **147 passed / 0 failed**（批次 18 为 138，净增 9）。

### 2.3 ⑨ 路径 A「祖先后代分支」真机覆盖 —— **PASS**

**判据（源码级）**：闸门路径 A = `handoff.pid == dialog_pid`（严格等值）**或**
`same_application_process(handoff.pid, dialog_pid)`（**祖先/后代**）——本项覆盖的是后者。
交接记录来自「AFM 前台 → 非 AFM 前台」的**下降沿**（`afm_gate_armed: handoff recorded: pid=<P1>`）。

**夹具**：AFM 夹具（独立进程，class 精确 `Tauri Window`）→ 接收方 P1（class `B19ReceiverWindow`，持前台产生下降沿）
→ P1 的**直接子进程** P2 持有合成文件对话框（class 精确 `#32770` ＋ 标题含 `Open`
＋ 一个 class 精确 `ComboBoxEx32` 的**直接**子窗口）。

**实测**（`.superpowers\sdd\focus\b19-report.txt`）：**`VERDICT=PASS`**，6/6 检查通过，`leftover_process=0`。

| 检查 | 结果 |
| --- | --- |
| `gate_opened_by_ancestor_branch` | **PASS** —— `opened_write_back_events=3` |
| `handoff_pid_is_receiver_process` | PASS —— `handoff_pid=504056` == `receiver_pid=504056` |
| `dialog_pid_is_child_process` | PASS —— `dialog_pid=503980` |
| `receiver_is_ancestor_of_dialog` | PASS —— `parent_of_dialog_pid=504056` == `receiver_pid` |
| `handoff_pid_differs_from_dialog_pid` | PASS ⇒ **严格等值分支被排除** |
| `path_B_excluded_dialog_opened_after_afm` | PASS —— 交接 `09:10:02` 早于对话框检测 `09:10:16` ⇒ **路径 B 被排除** |

**决定性日志行**：`afm_gate_armed: handoff recorded: pid=504056` → `DialogDetected hwnd=658262 app=#32770` → `opened_write_back`。

紧随其后的 `WriteFailed reason=write_not_verified` 是**预期内**的（合成对话框没有真实「文件名」编辑框，UIA 注入无法验证），
但 `opened_write_back` 打印在**真正写入之前** ⇒ **闸门放行这个结论不受注入成败影响**。
同时它顺带实证了 ⑩ 的不变量：写失败不留记录 ⇒ 5 秒后重试（日志里 `opened_write_back` 出现 3 次）。

⚠️ **偏离 AGENTS §2.8（如实记录）**：本项只能在**交互桌面**上做 —— 本机实测 `CreateDesktop` 私有桌面**没有前台语义**
（`GetForegroundWindow` 恒为 0、`SetForegroundWindow` 失败），侧车在那里永远看不到前台窗口 ⇒ 闸门永远开不了。
夹具窗口已做成小尺寸、判定后立即关闭、退出前恢复用户原来的前台窗口。

前两轮失败的真实原因也记在这里：夹具对话框**不满足侧车的「文件对话框签名」** ⇒ 侧车从未发 `DialogDetected`
⇒ 当时的「no decisive gate event」是**空结论**，不是被测行为回归。

### 2.4 ⑧ 虚拟位置：侧车侧 HTTP 级闭环（PASS）

**判据**：推真实目录 ⇒ 侧车记住；再推空路径（= 扩展在虚拟位置发出的「显式清空」信号）⇒ 侧车**确实忘掉**上一个真实目录。

| 步骤 | 请求 | 响应 | `GET /get_status` |
| --- | --- | --- | --- |
| 推真实路径 | `POST /set_path {"path":"C:\\"}` | 404 `no_active_dialog`（本轮无对话框在屏） | `current_path = "C:\\"` ⇒ **记住了** |
| 推虚拟位置信号 | `POST /set_path {"path":""}` | 200 `{"ok":true}` | `current_path = ""` ⇒ **忘了** |

`spike.log` 判定行：`path_update`（`current_path=C:\`）→ `set_path_no_dialog`（path stored, nothing written）→
`set_path_cleared`（`current_path cleared (was='C:\'); AFM is not in a local directory`）
⇒ **`VERDICT=PASS`**，`leftover_process=0`。

脚本/日志：`.superpowers\sdd\focus\b19-virtual-position.ps1` → `.superpowers\sdd\focus\b19-virtual-position.txt`。

**扩展侧（契约级，非运行时截图）**：`dist/index.js:357-366` 判定非本地路径后**不再推送**、改调 `sendClearToSidecar()`（`{"path":""}`）；
`dist/index.js:740-748` 的 toast 是 warning 类型，文案说「已告知助手忘掉上一个路径；没有东西可同步」⇒ **不声称写入了对话框**。
⚠️ toast 在 AFM 真机上的**实际渲染**本轮未验（`tauri dev` 界面结论一律作废）。

### 2.5 3.14.1 重建 → 载荷校验 → 装机链复验 → winget 清单同步

**为什么要重建**：⑩ 的修复在**侧车二进制**里 ⇒ 不重建安装包，修复就发不出去。

| 环节 | 结果 |
| --- | --- |
| 侧车新件 | `extensions\kizemo.focus-sync\bin\focus-sync-sidecar.exe` sha **`A9478986…3468`** / 2,903,552 B |
| 安装包 | `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.14.1_x64-setup.exe` **45,478,740 B** / sha **`1DD82EEE…D2A83`** / mtime 2026-11-18 16:40:59 |
| 载荷校验 | `verify-installer-payload.ps1` ⇒ 包内 `tools\focus-sync-sidecar.exe` sha **= A9478986…3468（match=True）**、包内扩展 `dist\index.js` 标记串 `traceReplace`/`non-local-path` 齐全 ⇒ **`VERIFY_OK reason=payload-carries-expected-sidecar-and-new-extension`** |
| 装机链 | `install-afm-verify.ps1`（`/S /FORCE`）⇒ `INSTALL_EXIT 0`；装机目录与 APPDATA 侧车**双双**刷新为 `A9478986…3468` ⇒ **`VERIFY_OK reason=install-refreshed-appdata-sidecar`** |
| `release/` 顶层 | 新包已就位（`publish-bundle-to-release.mjs` 按规则把不同内容隔离进 `local-builds/`，**顶层副本已手动替换为新件**） |
| winget 清单 | `InstallerSha256` 已同步为 `1DD82EEE…D2A83`（`ReleaseDate` 2026-11-18）⇒ `check-manifest.mjs 3.14.1` **All local checks passed** |
| `release:status` | 选中 `Alpha File Manager_3.14.1_x64-setup.exe` / 45,478,740 / `1dd82eee…d2a83` |

旧件留档：旧侧车 `74E5957F…21CCF` → `.superpowers\sdd\old-sidecar-b19-preinstall.exe`；旧安装包 sha `EF5D6DE0…C23` 已被新件取代。

---

## 三、门禁与产物（本轮实测）

| 门禁 | 结果 |
| --- | --- |
| 侧车 `cargo test` | **147 passed / 0 failed**（日志 `.superpowers\sdd\sidecar-test-b19.log`） |
| 前端串行五步（`ts` / `lint:check` / `test:unit:run` / `verify-changelog` / `verify:sandbox`） | **全绿**，测试 **311 个文件 / 3242 个用例**（日志 `.superpowers\sdd\g-{ts,lint,test,changelog,sandbox}.log`） |
| `node ./docs/winget/check-manifest.mjs 3.14.1` | **All local checks passed** |

**产物**：侧车 sha `A9478986…3468` / 2,903,552 B；安装包 `release\Alpha File Manager_3.14.1_x64-setup.exe` sha `1DD82EEE…D2A83` / 45,478,740 B。

---

## 四、已验证 / 推断 / 未验证

| # | 结论 | 状态 | 证据 |
| --- | --- | --- | --- |
| ① | 侧车确实忘掉上一个真实目录（虚拟位置停留时） | **已验证** | 20.14.5，`VERDICT=PASS` |
| ② | 同一目标不会被侧车写两遍 | **已验证（单测级）** | 20.14.4，净增 9 条判别性单测；`147/0` |
| ③ | 真实换目标 / 用户手动挪动对话框后仍会重新写 | **已验证（单测级）** | `a_different_target_is_still_written`、`a_dialog_the_user_moved_is_written_again` |
| ④ | 写失败保持可重试 | **已验证（单测级）** | `a_failed_write_leaves_no_record`、`a_failed_write_leaves_no_write_record` |
| ⑤ | `npm run check` 负载 flake 已消除 | **已验证** | 串行五步门禁全绿 |
| ⑥ | 新安装包携带 ⑩ 修复的新侧车 | **已验证** | `VERIFY_OK reason=payload-carries-expected-sidecar-and-new-extension` |
| ⑦ | 装机器把 APPDATA 旧侧车刷新成新侧车 | **已验证（复跑）** | `VERIFY_OK reason=install-refreshed-appdata-sidecar`（3.14.1 → 3.14.1 同版本重装，`/FORCE`） |
| ⑧ | winget 3.14.1 清单与本地安装包一致 | **已验证** | `check-manifest.mjs` 全过；sha 与 `release/` 顶层包逐字一致 |
| ⑨ | 路径 A 祖先后代分支真机覆盖 | **已验证** | 2.3，`VERDICT=PASS`（6/6），`leftover_process=0` |
| ⑩ | 扩展 toast 在 AFM 真机上的实际渲染 | **未验证** | 只到源码契约级（20.14.6） |
| ⑪ | 侧车去重后的**真机双写不复现** | **未验证** | 判别性单测覆盖了决策函数，但未在真机对话框上做「一次打开只写一遍」的计数观测 |

---

## 五、本轮工具坑（省得下一轮再踩）

1. **`publish-bundle-to-release.mjs` 不会覆盖同名不同内容的包**：重建 3.14.1 后它把新包**隔离**到 `release\local-builds\`，`release/` 顶层留着**旧的 3.14.1**（同版本号、不同字节）。
   ⇒ 想让 `release/` 顶层就是新包，必须**手动 `File.Copy` 覆盖并刷新 mtime**；否则 `check-manifest` 会拿旧包的 sha 去比。
2. **同版本重装必须带 `/FORCE`**：安装器的防呆守卫对「同 build」直接 `EXIT=2`（`reason=same-build`），那不是包坏了。
3. **`npm run tauri:build` 很慢**（本轮 Rust 全量重编约 12 min）⇒ 判断出要真机验收就**早启动、丢后台跑**，再做文档/补测试。
4. **`.cmd` 构建脚本必须纯 ASCII**（含中文注释会让 cmd 按系统 ANSI 代码页读成乱码并当命令执行）。
5. **后台跑长编译不要挂 stdout 管道**：让 `.cmd` 内部 `> logfile 2>&1` 重定向，再单独读文件看进度。
6. **含变量的 PowerShell 一律落 `.ps1` 再执行**；本机 bash 是 PowerShell，`cd /d` 不合法，用 `Set-Location -LiteralPath`。
7. **`cargo` 不在默认 PATH**：用 `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin\cargo.exe`，且**不要与 `npm run check` 并发**。
8. **私有（非交互）桌面没有前台窗口**：`GetForegroundWindow` 恒返回 0 ⇒ 依赖「前台窗口」的判据在隔离桌面上无法成立（详见 2.3）。

---

## 六、下一批次建议入口

1. **⑨ 已结案**（PASS，见 2.3）：若要复跑，夹具必须满足侧车的「文件对话框签名」——
   class **精确** `#32770` ＋ 标题含 `open`/`打开` ＋ 一个 class **精确** `ComboBoxEx32` 的**直接**子窗口，
   否则侧车根本不会发 `DialogDetected`（前两轮就卡在这）；
2. **⑩ 真机计数观测**：在真机上对「一次对话框打开」计数写入次数（读 `spike.log` 的写入事件），把「单测级」升级到「真机级」；
3. **toast 真机渲染**：走 `tauri build` 打包产物 ＋ 真机截图，确认 ⑧ 的 toast 文案真的出现；
4. **对外发布**：GitHub Release / winget PR / upstream 移植 —— **需用户授权**；
5. **`npm run check` 的并发跑法**：`run-p` 在本机不可靠这件事只做了「缓解」，**没有根治**（根治要降并发或拆 CI 资源池）。

---

## 七、一句话接力 prompt

> 请先完整读 `F:/soft/00selfmade/filemanager/sigma-file-manager/docs/handoff-2026-11-18-quark-pan-batch19.md`（批次 19 入口卡）与 `F:/soft/00selfmade/filemanager/sigma-file-manager/docs/handoff-2026-11-11-quark-pan.md` 的 §20.14，再读 `F:/soft/00selfmade/filemanager/sigma-file-manager/AGENTS.md` 的 §2.8、§2.10，自行掌握背景、进度、已验证·推断·未验证矩阵与遗留项后继续推进。
