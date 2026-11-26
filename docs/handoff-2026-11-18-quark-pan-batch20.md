# 批次 20 交接卡 · ⑩ toast 真机渲染 PASS ＋ ⑪ 写入闸门 v0.6.4 ＋ ⑫ 13 locale 收口 ＋ ⑤ check 串行化

- 生成时间：2026-11-18
- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`（分支 `main`）
- 台账真源：`docs/handoff-2026-11-11-quark-pan.md` **§20.15**（本卡是入口卡，详细论证看台账）
- 上一卡：`docs/handoff-2026-11-18-quark-pan-batch19.md`（批次 19 = 台账 §20.14）

---

## 一、背景与目标

批次 19（台账 §20.14）修掉了 focus sync 的双写去重（v0.6.3）、完成 3.14.1 装机链判别性实验，**留下 4 项遗留**：

| # | 批次 19 遗留 | 本批次处理 |
| --- | --- | --- |
| ⑩ | 扩展 toast 在 AFM 真机上的**实际渲染** —— 只到源码契约级 | **真机 PASS**（20.15.5）：CDP 命中 `[data-sonner-toast]` ＋ 截图目视确认 |
| ⑪ | 去重后的**真机「一次对话框打开只写一遍」计数观测** —— 只有单测级 | **修复落地 ＋ 单测过；真机三轮受阻，判定 UNDETERMINED**（20.15.6） |
| ⑫ | 其余 **13 个 locale** changelog 补 3.14.1 第 5 条 | **已完成**（20.15.1） |
| ⑤ | `npm run check` 并发跑法 flake —— 批次 19 只做了「缓解」 | **根治：`run-p` → `run-s`**（20.15.2） |

**范围声明**：只做**仓库版本与发版材料**，**不做对外发布**（GitHub Release / winget 清单 PR / upstream 移植需用户授权）。

**本批次未重建安装包** ⇒ `release\` 顶层 3.14.1 安装包**仍是批次 19 的旧件**（载荷里的侧车 sha 仍为 `A9478986…3468`，**不含** v0.6.4 写入闸门修复）。理由见 20.15.7。
用户真实数据只读未改（唯一触碰是 `%APPDATA%\...\user-extensions.json` 的**单个旗标**做字节级替换，已 `STORE_RESTORED=True` 还原）。

---

## 二、这批次改了什么

### 2.1 ⑤ `npm run check` 并发 flake 根治（`run-p` → `run-s`）

批次 19 只做了「缓解」（全局 `hookTimeout: 30000` ＋ 单文件 `testTimeout`）。本批次把 `package.json` 的 `check` / `check:win` 从 `run-p` 改成 `run-s`：

- **CI 分步不受影响**（CI 走各步骤自己的脚本，不经 `run-p` 聚合）；
- `vitest.config.ts` 的 `hookTimeout: 30000` 保留；
- **两轮定向 A/B 未复现** ⇒ **具体争用源 UNDETERMINED**；根治依据是「移除并发这一失败类别」，不夸大。

**代价**：串行比并行慢，换取确定性。

### 2.2 ⑪ 双写根因钉死：TOCTOU（先写后记）

批次 19 的 v0.6.3 用**「写成功后才落写记录」**作为唯一去重键 ⇒ 两条写路径（`/set_path` 即时写 与 `DialogEvent::Opened` 写回）**各自**通过去重检查时，同一目标被写两遍：

| 位置 | 事实 |
| --- | --- |
| `sidecar/src/writer.rs:198-200` | 写成功**之后**才 `record_write` |
| `sidecar/src/http_server.rs:529 → 601` | `/set_path`：去重检查 → 写入 |
| `sidecar/src/main.rs:401 → 433` | 写回：去重检查 → 写入 |

**修复前真机实测**：`w1=2`、`w2=0`。

### 2.3 侧车 v0.6.4：per-hwnd 写入闸门

在**去重检查之前**取闸门，并在**闸内复查**去重键 ⇒ 「检查 ＋ 写入」原子化。

| 文件 | 改动 |
| --- | --- |
| `sidecar/src/state.rs` | 新增 `pub write_gates: Mutex<HashMap<u32, Arc<Mutex<()>>>>`；`AppState::new` 初始化；`use std::sync::{Arc, Mutex};`；新增 `pub fn write_gate(&self, hwnd) -> Arc<Mutex<()>>`（**同一 hwnd 恒返回同一闸门**） |
| `sidecar/src/http_server.rs` | `dialog_holds_path` 去重检查**之前**取闸门，闸内**复查** |
| `sidecar/src/main.rs` | `should_write_back` 决策**之前**取闸门（决策与写入原子化） |

**加锁顺序**：两条路径都是「**闸门 → 状态锁**」，顺序一致 ⇒ 无死锁风险。
**判别性单测**：新增 3 条 ⇒ `cargo test --release` **150 passed / 0 failed**（批次 19 为 147）。

**二进制重建 ＋ 双处部署**（实测三处 sha/size 逐字一致）：

| 位置 | 大小 | sha256 |
| --- | --- | --- |
| `sidecar\target\release\spike.exe`（Cargo package `sigma-listary-spike` ⇒ 产物真名**不是** `focus-sync-sidecar.exe`） | 2,907,648 B | **`480FDE62C11B1A25063B2F252C09A87C5463217A797C7C1893989F07480106CF`** |
| `extensions\kizemo.focus-sync\bin\focus-sync-sidecar.exe`（仓库，不入 git） | 2,907,648 B | 同上 |
| `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`（应用实际解析处） | 2,907,648 B | 同上 |

⚠️ 仓库 `bin/` 不入 git，但 `src-tauri\tauri.conf.json` 的 `bundle.resources` 映射为 `tools/focus-sync-sidecar.exe` ⇒ 重建产物即随包传播。
⚠️ 应用会把扩展目录**复制**到 `%APPDATA%\...\extensions\` 并**从那里**解析侧车 ⇒ 重建后**必须**同步覆盖该处。

### 2.4 ⑫ 13 个 locale changelog 补第 5 条

`de / es / fa / fr / hi / it / ja / pt / ru / sl / tr / ur / vi` 各 +2 行（标题 ＋ 正文），内容与根 `CHANGELOG.md` 的第 5 条对应：**「同一个文件夹不再被写进对话框两次」**。

**复验**：15 个文件（根 ＋ 14 locale）3.14.1 段落 `^\*\*` 行数**全 = 6**；**无 BOM / 无 U+FFFD**；`npm run verify-changelog` **exit 0**（3.14.1、58 条目、顺序正确）。

### 2.5 ⑩ toast 真机渲染 —— **PASS**

**宿主链路（源码级）**：`create-ui-api.ts:70-92` 与 `app-toast.ts:83` 均 `toast.custom(markRaw(ToastStatic), {...})`；`toaster.vue` 挂载 `[data-sonner-toaster]`。

**两条常规路走不通（产品缺陷）**：
① `toolbarDropdowns` **无任何 UI 消费方**、命令面板**未挂载**、focus-sync 未注册 context-menu/sidebar ⇒ **扩展 toast 无用户可达 UI 入口**；
② CDP `Target.getTargets` 只有 **1 个 page target（无 worker）** ⇒ 无法 attach 扩展 worker 调 `sigma.ui.showNotification`。

**取证路线（绕开上面两条）**：扩展 `dist/index.js` 的 `activate()` 里 L900 读 `edgeLegacyFilePickerTipShown`，**非 true 即弹 Edge tip toast**（L921-945，`duration: 12000`）
⇒ **临时**把 `%APPDATA%\com.sigma-file-manager.app\user-extensions.json` 的
`extensions["kizemo.focus-sync"].customSettings.edgeLegacyFilePickerTipShown` **字节级置 false**，
再用 `b20-toast5.ps1`（备份 → 精确替换 → **隔离桌面** CDP → `finally` 字节级还原并断言）取证。

**实测**：沙箱 `AFM-CDP-41495`、CDP 9333 ⇒ `toasts=1 tip=true toaster=true` ⇒ **`VERDICT=PASS`**。
截图 `.superpowers\sdd\focus\b20-cdp3e\toast-tip-1.png`（169,292 B）＋ `toast-viewport.png`（169,302 B）—— 目视确认真实 AFM 中文界面右下角渲染出带「扩展 · Focus Sync」徽标的 toast。

**判据口径**：硬编码英文描述片段（语言无关）；结论只允许 **PASS / HOST_ONLY / UNDETERMINED**。

---

## 三、门禁与产物（本轮实测）

| 门禁 | 结果 |
| --- | --- |
| 侧车 `cargo test --release` | **150 passed / 0 failed**（日志 `.superpowers\sdd\focus\b20-sidecar-test.log`） |
| 侧车 `cargo build --release` | **成功**（日志 `.superpowers\sdd\focus\b20-sidecar-build.log`） |
| 最终 exe 重建（`npx tauri build --no-bundle`） | **成功**：`src-tauri\target\release\alpha-file-manager.exe` 82,778,112 B / sha `A66F93B6…C67AD8`；**`HAS_CDP_FLAG=False`**（无 `remote-debugging-port`）⇒ 无 CDP 插桩；详见台账 20.15.7 |
| exe 内 `additionalBrowserArgs` 扫描 | **`True` —— 但不算插桩证据**：那是 Tauri 配置结构体的**字段名**，必然编进二进制；判插桩只看实际生效值字符串 |
| 侧车随构建传播 | `src-tauri\target\release\tools\focus-sync-sidecar.exe` 2,907,648 B / sha `480FDE62…106CF`，与部署三处逐字一致 ⇒ **v0.6.4 进构建资源目录** |
| `npm run check`（串行五步） | **通过** `GATE_EXIT=0`（串行五步依次执行，无交叠）：ts ✓ / lint:check ✓ / vitest **311 files, 3243 tests passed**（102.80s）/ verify-changelog ✓（3.14.1 顺序正确）/ verify:sandbox ✓（0 violations / 20 patterns）。日志 `.superpowers\sdd\b20-gate.log`。用例数较批次 19 基线 3242 **+1**，文件数不变、无用例删除；+1 来源本轮**未归因**（已排除与 ⑫ locale 补条目相关：无 vitest 用例读 `public/changelog`），详见台账 20.15.7。 |

**产物**：侧车 sha `480FDE62…106CF` / 2,907,648 B（**不入 git**，随 `bundle.resources` 传播）；安装包**未重建**（`release\` 顶层仍是批次 19 的 `1DD82EEE…D2A83` / 45,478,740 B）。

---

## 四、已验证 / 推断 / 未验证

| # | 结论 | 状态 | 证据 |
| --- | --- | --- | --- |
| ⑩ | 扩展 toast 在 AFM 真机上的实际渲染 | **已验证** | 2.5，`VERDICT=PASS`（`toasts=1 tip=true toaster=true`）＋ 截图 |
| ⑪ | 同一目标不会被写两遍（**单测级**） | **已验证（单测级）** | 2.3，v0.6.4 写入闸门；`150 / 0` |
| ⑪ | 同一目标不会被写两遍（**真机计数级**） | **未验证（UNDETERMINED）** | v4/v5/v6 三轮均 `w1=0 w2=0`；**闸门放行已证实**，写入落地受阻于夹具/前台 |
| ⑫ | 13 个 locale changelog 同步 3.14.1 第 5 条 | **已验证** | 2.4，15 文件 `^\*\*` 全 = 6；`verify-changelog` exit 0 |
| ⑤ | `npm run check` 并发 flake 根治 | **已验证（工程口径）** | `run-p`→`run-s`；**具体争用源 UNDETERMINED** |
| — | v0.6.4 侧车已重建并部署到仓库 bin ＋ APPDATA | **已验证** | 2.3，三处 sha/size 逐字一致 |
| — | v0.6.4 修复随安装包传播 | **未做** | 本批次未重建安装包；`release\` 顶层 3.14.1 仍是批次 19 旧件 |
| — | 对外发布（GitHub Release / winget PR / upstream 移植） | **未做（需用户授权）** | 本批次范围外 |

### 4.1 ⑪ 三轮真机尝试（明细）

**计数口径**：日志段内 `event=="Write"` 且 target 归一化后 == `c:\windows\system32`；第一次推送期望 **1**、第二次（正斜杠变体）期望 **0**；`WriteFailed` **不计入**。

| 轮次 | 做法 | 实测 | 结论 |
| --- | --- | --- | --- |
| v4 | 前台往返（接收方进程**自己**抬 AFM 类窗口，再把前台切回对话框） | `registered_dialogs_after_visit=1`；**闸门首次真正放行**（`set_path_iter hwnd=64684132 app=#32770 auto_confirm=false (shared_fg matched)`，`opened_write_back` 也发起）；但 4 次写入全部落不了地 —— 3× `write_not_verified`、1× `unsupported_dialog_type` | UNDETERMINED（`w1=0 w2=0`） |
| v5 | 不做前台往返（对话框从 `Show()` 起保持前台，与修复前 `w1=2` 那轮同条件） | `registered_dialogs=0`、`set_path_no_dialog`、**无任何写入**；诊断坐实侧车采样到 **Ghost 窗口**（`fg_class=Ghost`，owner pid=**1188**） | UNDETERMINED（`w1=0 w2=0`） |
| v6 | 对话框由**自己的线程**重新激活 | `dialog.raised ok=True is_fg=True`、`granted_child=True`；接收方抬前台失败（`rcv_raised ok=False fg_is_me=False`）、写回路径 `opened_write_back_skip_no_afm_visit`、1× `unsupported_dialog_type` | UNDETERMINED（`w1=0 w2=0`） |

**三轮受阻的根因全在夹具/前台语义侧**：
① **Ghost 窗口**：COM 对话框创建期存在 class=`Ghost` 的瞬态前景窗（owner pid 1188），侧车 500 ms tick 采到它 ⇒ 交接 pid 错（`handoff recorded: pid=1188 fg_class=Ghost`）⇒ 对话框从未注册（`active_dialogs=0`）—— **这同时解释了 v3 那轮的 `handoff_pid=1188`**；
② **前台锁**：脚本进程抢不到前台（ALT trick 无效，ALT keyup 被投递到对话框所在线程）；
③ **对话框内部焦点重建**：前台往返后 `inject_folder_path` 判型失准 / 回读校验不过。

**为什么不是 FAIL**：v4 恰恰证明**闸门会放行**（修复要保证的正是这个），失败发生在**放行之后**的注入/回读环节；v5/v6 根本没走到写入。
⇒ **修复既未被真机证实（没拿到 `w1=1`），也未被真机证伪（没出现 `w1=2`）**；单测层已覆盖该不变量。

**下一批次配方**：
1. 子进程（对话框属主）用 `AllowSetForegroundWindow` 授权接收方抬前台 —— **授权调用必须发生在接收方已成为前台进程之后**，否则 `granted=False`；
2. 交接采样避开 Ghost 窗口（侧车在 ghost 消失后再采，或夹具在对话框 `Show()` 之后再抬前台）；
3. 对话框由**自己的线程**重新激活（已实现，v6 验证 `ok=True is_fg=True`）；
4. 最终仍需拿到 **`w1=1 w2=0`** 才算 PASS。

⚠️ **偏离 AGENTS §2.8（如实记录）**：⑪ 依赖**前台语义**，只能在**交互桌面**上做（本机实测私有桌面 `GetForegroundWindow` 恒 0 ⇒ 闸门永远开不了）。
夹具窗口小尺寸、判定后立即关闭、退出前恢复原前台窗口。**⑩ 则在隔离桌面完成**（不依赖前台语义）。

---

## 五、本轮工具坑（省得下一轮再踩）

1. **COM 对话框创建期有 class=`Ghost` 的瞬态前景窗**（owner pid 1188）⇒ 侧车交接 pid 错、对话框不被注册。依赖前台采样的真机实验**必须**先排除 Ghost。
2. **`AllowSetForegroundWindow` 授权时机**：必须在**接收方已成为前台进程之后**调用，否则 `granted=False`。
3. **ALT trick 抢前台无效**：ALT keyup 被投递到**前台窗口所在线程**，不是脚本线程。
4. **私有（非交互）桌面没有前台语义**：`GetForegroundWindow` 恒 0 ⇒ 依赖前台的判据只能在交互桌面做。
5. **UIA 树里查不到 Radix/vue-sonner toast** ⇒ 「UIA 命中数=0」**不等于**「没弹」；必须 CDP 截图 / DOM 取证。
6. **CDP `Target.getTargets` 不暴露扩展 worker** ⇒ 无法 attach worker 调 `sigma.ui.showNotification`；改用扩展自身 `activate()` 的 Edge tip toast 作取证载体。
7. **`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 环境变量与注册表 `AdditionalBrowserArguments` 都无效** ⇒ 只能靠 `tauri.conf.json` 的 `additionalBrowserArgs` 插桩（本批次**已回滚干净**）。
8. **应用把扩展目录复制到 `%APPDATA%\com.sigma-file-manager.app\extensions\` 并从那里解析侧车 exe** ⇒ 重建侧车后**必须**同步覆盖该处。
9. **bash 里 `Set-Location` 之后若有写/删目标会被安全策略拦** ⇒ `cd` 放进 `.ps1` 内部，再用 `pwsh -File` 跑。
10. **`Write` 覆盖「会话上次读取后被其它命令改动」的文件会被拒** ⇒ 改用新文件名。
11. **残留夹具进程互抢前台** ⇒ 跑前/跑后 `Get-CimInstance Win32_Process -Filter "Name='<helper>.exe'"` ＋ `Stop-Process -Id` 强杀（**不要 `taskkill /IM`**），确认 `remaining=0`。
12. **子会话派发被 Token Plan 用量上限堵死（2056）** ⇒ ⑩/⑪ 由根会话直接实现（工时与成本显著上升）。
13. **端口 9222 被 ecclaw 的 Electron 占用（勿杀）**，沙箱 CDP 一律用 **9333**；同一 `--out` 目录重复跑会被日志文件锁挡住 ⇒ 每次用新目录。

---

## 六、下一批次建议入口

1. **⑪ 收口**（唯一硬阻塞）：按第四节「下一批次配方」做完，拿到 **`w1=1 w2=0`** 才能把「真机计数」从 UNDETERMINED 升到 PASS；
2. **⑪ 关联的产品缺陷**：扩展 toast **没有用户可达的 UI 入口**（`toolbarDropdowns` 无消费方、命令面板未挂载、focus-sync 未注册 context-menu/sidebar）⇒ 若要让用户能主动触发 toast，需要先在宿主里补消费方；
3. **安装包重建决策**：v0.6.4 修复**只在侧车二进制里** ⇒ 要传播必须重打安装包 ＋ 更新 winget 清单 sha ＋ 重做装机链复验（本批次默认不做）；
4. **对外发布**：GitHub Release 3.14.1 / winget 清单 PR / upstream 移植 —— **需用户授权**；
5. **⑤ 的残留**：串行化解决了「并发这一失败类别」，但**具体争用源仍 UNDETERMINED**；若要彻底定位，需在并发表格下复现（代价高，优先级低）。

---

## 七、一句话接力 prompt

> 请先完整读 `F:/soft/00selfmade/filemanager/sigma-file-manager/docs/handoff-2026-11-18-quark-pan-batch20.md`（批次 20 入口卡）与 `F:/soft/00selfmade/filemanager/sigma-file-manager/docs/handoff-2026-11-11-quark-pan.md` 的 §20.15，再读 `F:/soft/00selfmade/filemanager/sigma-file-manager/AGENTS.md` 的 §2.8、§2.10，自行掌握背景、进度、已验证·推断·未验证矩阵与遗留项后继续推进。
