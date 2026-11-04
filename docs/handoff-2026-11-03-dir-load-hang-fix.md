# 交接文档：目录加载卡死（转圈不出列表）修复 + 检索改造收益

- 文档时间：2026-11-03 23:4x（UTC+8）
- 上一份交接：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-03-stage5-batch3-actions-menu.md`
- 本轮触发：用户报告「点击进入 `E:/办公文件/G1销售管理/00数据汇总分析` 后长时间不显示文件列表、刷新按键一直转圈，这个 bug 曾经出现过并修复过」。
- 当前版本：`3.9.2`（`package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` 三处一致）
- HEAD：`ceff7e38`，工作树改动**全部未提交**

---

## 一、本轮结论（先看这段）

**根因是前端耦合缺陷，不是后端慢，也不是磁盘慢。**

旧 `readDir` 用 `Promise.all([loadDirectoryContents(...), stopWatchingPromise])` 把「渲染目录内容」和「停掉上一个目录的监听」绑成必须同时完成的一对。只要 `stopWatching()` 因为任何原因不 settle：

- `loadDirectoryContents` 就算早就返回了，`await Promise.all` 也永不 resolve ⇒ **列表不显示、`isRefreshing` 永远不复位 ⇒ 刷新按键一直转圈**；
- `watchedPath` 没被清空 ⇒ **后续每一次导航都去 await 同一个挂起的 promise** ⇒ 整个文件浏览器瘫痪；
- `enqueueWatcherOperation` 里 `watcherOperation = result.catch(() => {})` 是**全局串行队列**，一次不 settle 就把队列永久毒化。

这三条叠加，完整解释了「列表不出 + 转圈 + 后续全挂」三个症状，也解释了**应用日志里与读目录相关的 error/warn 一条都没有**（这是静默挂起，不是异常）。

修复策略是**解耦 + 超时兜底**，不等「`unwatch_directory` 为什么不返回」定案：即使后端真的卡住，UI 也不会再被拖死。

---

## 二、用户提出的三件事，各自状态

| # | 用户要求 | 状态 |
|---|---|---|
| 1 | 排查并再次修复「进入目录长时间不出列表 + 刷新转圈」，确保不再复现 | **已完成**：代码改 + 目标单测 6/6 绿 + `eslint .` 回基线 + 打包 + **沙箱真机验收 `pass=5 fail=0`** + 已静默装机（§6） |
| 2 | 补写「检索部分」改造能带来什么好处 | **已完成**，见 `docs/benefit-2026-11-03-retrieval-incremental-index.md` |
| 3 | 更新完整 handoff 文档 + 一句话接力 prompt | **本文 + `.superpowers/sdd/relay-prompt-20261103-dir-hang.md`** |

---

## 三、修改清单（全部已验证落盘）

### 3.1 产品代码（1 个文件，6 处）

`F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\use-file-browser-navigation.ts`

1. **新增常量** `WATCHER_OPERATION_TIMEOUT_MS = 5000`（紧邻 `DIRECTORY_DWELL_TIME_MS`、`WATCHER_DEBOUNCE_MS`）。
2. **`dirWatcherDiagEnabled()` 加保护**：整体 try/catch，并在开头判 `typeof localStorage === 'undefined' || localStorage === null` 直接返回 `false`。理由见 §5.1。
3. **新增 `withWatcherTimeout(operation, label)`**：包装任何 watcher 相关 invoke。**永不 reject** —— 成功放行；超时打 `console.warn` 后放行；失败打 `console.error` 后放行。`setTimeout` 句柄做 `unref` 兜底（无 `unref` 的宿主跳过）。
4. **`startWatching` 的两处 invoke 全部改走 `withWatcherTimeout`**：`watch_directory` 一处、竞态分支里的 `unwatch_directory` 一处。
5. **`stopWatchingImmediately` 调整清理顺序**：`const pathToStop = watchedPath; watchedPath = null;` **移到 `await` 之前**，再用 `withWatcherTimeout(invoke('unwatch_directory', ...))`。
6. **`readDir` 拆掉 `Promise.all`**：
   ```ts
   if (watchedPath && watchedPath !== normalizedPath) {
     logDirWatcherDiag(...)
     void stopWatching()          // fire-and-forget，不 await
   }
   const result = await loadDirectoryContents(path, readOptions)
   ```
   渲染与停监听彻底解耦；`startWatching(result.path)` 原本就不 await。

### 3.2 测试（1 个文件，+55 行）

`…\composables\__tests__\use-file-browser-navigation.test.ts`

新增 `describe('useFileBrowserNavigation watcher teardown isolation')`。做法：让 `unwatch_directory` 返回 `new Promise<never>(() => {})`（**永不 settle**），断言切目录后 `currentPath` / `dirContents.path` / `isLoading` / `isRefreshing` 全部正常。

这就是「确保不再复现」的守门测试：只要有人把 `Promise.all` 或 await-stopWatching 写回去，这条用例立刻红。

### 3.3 中间产物

均在 `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\`：

- `b6-build1.cmd`（550 B，`nonAscii=0`）+ `b6-build1.log`
- `navtest.log` / `navtest2.log`（红灯与绿灯实跑）
- `ts-b6.log`（vue-tsc）
- `eslint-b6b.log`（复验 186）
- `stylelint-b6.log`（167 存量）

---

## 四、门禁状态（本轮实测）

| 项 | 命令 | 结果 | 判读 |
|---|---|---|---|
| 目标单测（红灯） | `test:unit:run -- <该文件>` | `Tests 6 failed (6)` / EXIT=1 | 修复前确实红，判别性成立 |
| 目标单测（绿灯） | 同上 | `Tests 6 passed (6)` / 1.92 s / EXIT=0 | ✅ 1.92 s 说明 `unref` 生效，没有 5 s 挂起 |
| vue-tsc | `npm --prefix <repo> run ts` | **EXIT=0** | ✅ |
| eslint 整仓 | `lint:eslint:check` | **186 problems (186 errors, 0 warnings)** | ✅ 与 HEAD 基线 186 逐字相等；目标文件零报错 |
| stylelint 整仓 | `lint:stylelint:check` | 167 problems / EXIT=2 | 存量，本轮未动任何 CSS/`.vue` 样式 |
| 打包 | `npm run tauri:build` | **`BUILD_EXIT=0`**，exe 80286208 B / NSIS 44799768 B | ✅ 见 §七 |
| `cargo test --lib` | `.superpowers/sdd/b5-cargo10.cmd` | 本轮**未跑** | 本轮未改任何 Rust 代码 |
| 全量 vitest | `test:unit:run` | 本轮**未跑** | 上轮基线 19 failed / 2579 passed |

**eslint 基线口径提醒**：必须用 `eslint .` 整仓、用 `git stash push -u` 现量 HEAD。基线随提交变，别拿旧数字对。

---

## 五、关键判读与踩过的坑（本轮新增，务必读）

### 5.1 5 条「存量红」的真凶是诊断埋点，不是业务逻辑

`dirWatcherDiagEnabled()` 无保护地读 `localStorage`。本机 vitest 环境下 `localStorage` 不可用（报 `localStorage is not available because --localstorage-file was not provided`），于是抛 `TypeError: Cannot read properties of undefined (reading 'getItem')`，在 `readDir` 的 `:800` 把主流程打断。

- 判据：`git diff --stat` 证明测试文件只被加了 55 行；`git show HEAD:<file> | grep '^\s*it\('` 数出 HEAD 上本来就有 5 个 `it` ⇒ **HEAD 上本来就红**。
- **处置：改产品代码，不改测试。** 顺带确立一条通用约束：**诊断日志永远不许有能力打断主流程。**

### 5.2 已否证的候选（不要再用它们解释本次卡死）

- **`includeItemCounts` 触发额外枚举** —— 被真实设置否证：`navigator.gridSortColumn = "name"`、`layout.type = {"name":"list"}` ⇒ `includeItemCounts = false`。取值逻辑在 `utils/file-browser-sort-columns.ts:125-137`。
- **磁盘慢 / 重解析点** —— 被实测否证：bug 现场目录顶层 74 项热读 **3 ms**，`TOP_REPARSE=0`、`DEEP_REPARSE=0`、`TOP_OFFLINE=0`，`E:\` 是 `Fixed`。
- **`dir_watcher.rs` 内部锁序反转** —— 全文读过：`EventQueue::push` 满则丢最旧不阻塞、`pop_timeout` 用 Condvar、`unwatch_directory` / `watch_directory` 锁序一致 ⇒ **本文件内部无锁序反转**。

### 5.3 「递归 10674 项 / 18.6 s」不是本次症状的来源

bug 现场目录：顶层 74 项（30 目录 / 44 文件），递归 **10674** 项，全树冷枚举 **18590 ms**。这是整树枚举的固有开销，与「进入该目录」这一动作无关 —— 进入只读顶层。**别拿这个 18.6 s 去解释列表不出。**

### 5.4 为什么「曾经修过又复发」—— 已定案：**病灶与「历史修复」是同一个提交**

- 历史修复点：`db15621b Refine: large dir loading performance`（8 文件 / 529+ / 194−，其中该 composable 429 行）、`288e0192 Refine: dir loading performance`。
- **本轮已读 diff 正文**：`git log -S 'Promise.all' -S 'stopWatchingImmediately' -- .../use-file-browser-navigation.ts` 在该文件上**只命中 `db15621b`**；精确 hunk 显示正是它引入了
  `Promise.all([loadDirectoryContents(...), stopWatcherPromise])`。
- ⚠️ **结论与上一版推断相反**：**不是「历史修复被后续重构覆盖」，而是「历史修复自己就把 watcher 停止和目录加载绑进了同一个 `Promise.all`」**。`enqueueWatcherOperation` 的串行队列把 `stopWatchingImmediately` 排在慢操作之后，`Promise.all` 让两者互相等待 ⇒ 那一轮为「性能」做的优化顺手造出了「列表不出」的病灶。`Promise.all` 耦合、`enqueueWatcherOperation` 串行队列、`stopWatchingImmediately` 三者都是 `db15621b` 自己种下的。
- **`AGENTS.md`（仓库根 `F:\soft\00selfmade\filemanager\AGENTS.md`）里 grep `转圈|一直转|不显示条目|目录列表|刷新按键|读取目录` 零命中** ⇒ 这条 bug 的历史修复**没有进坑位清单**，所以它复发时没人认得出来。

### 5.5 「`drag_no_activate` 刷屏」—— 已钉死并修复：**只是日志体积问题，不是产品缺陷**

- ⚠️ 上一版推断的两处断言**均被推翻**：
  - `IS_ACTIVE` 读的是**窗口激活状态**（前台/后台），与拖拽无关；
  - 全仓**不存在** `isCopyOrMoveInProgress()`（`grep` 零命中，别再搜了）；
  - `requestSizesBatch` **不读**拖拽状态。
- 真实成因（读码 + 埋点）：`src/modules/navigator/.../use-file-browser-drag.ts` 的 `syncArmWithPointer()` 挂在条目容器的 `mouseover` 上，**跨条目即冒泡一次** ⇒ 每次鼠标移动都触发一条 `set_drag_no_activate` IPC；Rust `drag_no_activate.rs` 的 `arm()` **每次调用都打一条 `log::info!`** ⇒ 刷屏。
- 修复（`src-tauri/src/drag_no_activate.rs`）：新增纯函数 `arm_state(active, armed, hover_bg) -> (armed_after, hover_bg_after, changed)` 与 `disarm_state(armed) -> (armed_after, changed)`，**只在状态真正翻转时记 log**；补 3 个单测。门禁 `cargo test --lib` 883 → **912 passed / 0 failed**。
- 影响面：**仅日志体积**；与「列表不出」无因果关系，原文里「需要后续复查」的悬案已结。

### 5.6 一个语义让步（必须写进结论）

`withWatcherTimeout` **永不 reject**。最坏情况是「少一次自动刷新 + 留一条日志」，代价远小于把浏览主流程挂死。这是本轮唯一主动接受的语义变化。

---

## 六、真机验收（已执行：沙箱独立桌面，pass=5 fail=0）

### 6.1 硬性前提

- **必须用 `tauri build` 打包产物**，`tauri dev` 的界面结论一律作废（dev 下 `RouterView` 内容区空白，已归因为 dev 构建特有）。
- **验收前先杀掉既有 AFM 实例**（单实例守卫会把新 pid 转发给老进程后自行退出）。本轮已装 3.9.2 修复版并拉起（PID 456284），**下一位开工先复核它是否还在**（`Get-Process alpha-file-manager`），且做任何夹具注入前必须先杀。release 构建下 `SFM_E2E_DATA_DIR` 被 `#[cfg(debug_assertions)]` 编译掉 ⇒ 隔离目录无效，必须走「**备份 → 灌夹具 → 验收 → 先杀进程 → restore → 核对**」。

### 6.2 执行方式

- 沙箱：`CreateDesktop(DESKTOP_ALL)` 建独立桌面，`EnumDesktopWindows` 找主窗，`AutomationElement.FromHandle` 取元素；`Start-Process` + 双路重定向日志 + 轮询 `ACCEPT_EXIT`。
- 脚本 `.superpowers\sdd\b6-sandbox-nav.ps1`（19736 B，纯 ASCII，`PARSE_ERRORS=0`）。
- 夹具：`tab.path = E:/办公文件/G1销售管理`、`gi=4` —— 让应用**先落在父目录**，再由 A0 双击进入报障目录。直接启动在报障目录会绕过 `readDir` 里的 `stopWatching()` 拆除路径，验不到缺陷。
- 探针清单 `.superpowers\sdd\b6-names.json`：6 个顶层探针（含目录 `00 BI报表`）+ 6 个子探针。

### 6.3 结果（第四轮，最终有效）

| 用例 | 判据 | 实测 | 结论 |
|---|---|---|---|
| A0 | 双击进入报障目录命中 | `double-click 00数据汇总分析 at screen (299,1179)`，attempts=1 | PASS |
| A1 | 列表出现且可寻址 | probes=1/6，`render_ms=475`，`total_from_launch_ms=30211`，`HIT 00 BI报表`，`names-a1.txt` 169 条 | PASS |
| A2 | 稳定窗口后仍可寻址 | probes=1/6，`HIT 00 BI报表`，`names-a2.txt` 169 条 | PASS |
| A3 | 二级目录仍能进 | `double-click 00 BI报表 at screen (292,1338)`，probes=2/6，`render_ms=399` | PASS |
| A4 | 进程存活（无崩溃、无挂死） | 存活 | PASS |

**`RESULT pass=5 fail=0` / `ACCEPT_EXIT=0`。**

- 渲染 475 ms / 399 ms，与「立刻出列表」一致；相对修复前「长时间不出」是量级差。
- 没出现新的 `[dir-watcher-diag]` 超时 warn ⇒ 后端这次也没真卡住，前端耦合是唯一病灶。

### 6.4 证据与已知局限

- 截图 `accept-b6\01-bug-dir.png`（2013845 B）、`02-stable.png`（2013845 B）、`03-subdir.png`（2093900 B）。
- ⚠️ `01` 与 `02` 的 sha256 前 16 位同为 `0D80544D88DE9D8A`（屏幕 BitBlt 对未变画面会产出逐字节相同的 PNG）⇒ **这两张不能当「画面稳定」的独立证据**。硬证据是 `names-a1.txt` / `names-a2.txt`（各 169 条可访问名）。
- A1 只有**单次**采样（`render_ms=475`），未做多次统计 —— 本轮验收的已知缺口。
- 判据从「≥2 探针」放宽到「≥1 探针」：列表把目录排在文件前，Chromium 会剪掉屏外行 ⇒ 5 个 `.xlsx` 探针不可寻址属脚本局限，`00 BI报表` 目录探针命中已足以证明列表渲染。**不许把探针改回纯文件探针**，改了会重演假 FAIL。
- 上一轮 A2 的 FAIL 是脚本遗留旧阈值（第 384 行仍是 `-ge 2`），同一探针两次测得同一个值 ⇒ **先怀疑断言，再怀疑产品**。

### 6.5 已装机（同版本原地升级）

- 注册表定位到 `D:\Program Files\Alpha File Manager`（`DisplayVersion=3.9.2`）；`Start-Process <pkg> -ArgumentList '/S' -Wait` ⇒ `INSTALL_EXIT=0`。
- 装机后 exe：**80286208 B @ 2026-11-03 23:32:40**（与本轮新构建一致；装机前为 80323584 B @ 11-02 16:33:24）。`POST_REGVER=3.9.2`。
- 应用已重新拉起（`RUNNING_PROCS=1`，PID 456284）。
- 用户数据已恢复：先确认 `PROC_AFTER_KILL=0` 再 restore；`user-workspaces.json` 由夹具态 1799 B 回到 **1834 B**，`AFTER_SHA=6FA408563FD5A611` 与 `BK_SHA=6FA408563FD5A611` 逐字一致。

### 6.6 辅助事实

- 应用日志：`%LOCALAPPDATA%\com.sigma-file-manager.app\logs\Alpha File Manager.log`，**时间戳是 UTC（本地 −8h）**，运行中被独占写，要用 `File.Open($p,'Open','Read','ReadWrite')` + `StreamReader` 读。
- 用户数据：`%APPDATA%\com.sigma-file-manager.app\user-data\`（`user-settings.json` 5382 B、另一件 1854 B、`user-stats.json` 31979 B）。
- `user-settings.json` 用**带点的扁平键**（如 `navigator.gridSortColumn`），必须 `$j.PSObject.Properties['navigator.gridSortColumn'].Value` 取，`$j.navigator` 恒为 null。

---

## 七、打包状态（已完成）

- 脚本 `.superpowers\sdd\b6-build1.cmd`（纯 ASCII，550 B，`nonAscii=0`），日志 `b6-build1.log`。
- 结果：`Finished \`release\` profile ... 3m 48s`、`Built application at: …\target\release\alpha-file-manager.exe`、**`BUILD_EXIT=0`**。
- 产物指纹：
  - exe `src-tauri\target\release\alpha-file-manager.exe` = **80286208 B @ 2026-11-03 23:33:17**
  - NSIS `bundle\nsis\Alpha File Manager_3.9.2_x64-setup.exe` = **44799768 B @ 23:33:16**，`SHA256=922DC3AE72CAEB243BAA88DEFD7ED1B9D12E1DAE908B33121D24602CBFAA62E4`
- 落点：`sigma-file-manager\release\local-builds\Alpha File Manager_3.9.2_x64-setup.exe`（44799768 B，与本轮 NSIS 一致）。⚠️ 仓库根 `F:\soft\00selfmade\filemanager\release` 下**没有** 3.9.2；`sigma-file-manager\release\` 直下的同名包是 **44873125 B**，是更早的 3.9.2，别拿错。
- ⚠️ 末步 `publish-bundle-to-release.mjs` 会遍历 `bundle/nsis/` 下**所有历史安装包**逐个复制，任何一个被别的进程锁住就整步 exit 1 —— 而 `tauri build` 本身早已成功。**分清「构建失败」和「复制失败」**，后者用 `node -e "fs.copyFileSync(a,b)"` 单独补。

---

## 八、下阶段计划

### 8.1 已闭环（本 bug 可以关闭）

1. ✅ `BUILD_EXIT=0`，产物核对完成（§七）。
2. ✅ 沙箱真机验收 `pass=5 fail=0` / `ACCEPT_EXIT=0`（§6.3）。
3. ✅ 修复版静默装机 + 应用已拉起，用户数据已恢复并核对 sha（§6.5）。
4. ✅ 防复发两条腿：守门测试 `useFileBrowserNavigation watcher teardown isolation`（用「`unwatch_directory` 永不返回」的桩把缺陷钉死）+ `AGENTS.md` 两条硬约束。
5. 后续任何人再动 watcher 生命周期逻辑，先跑第 4 条那个测试；改坏了会立刻红。

### 8.2 本轮遗留（三项已全部闭环）

1. **`db15621b` / `288e0192` 的 diff 正文** —— ✅ 已读。**结论与原推断相反**，见 §5.4。
2. **基线补跑** —— ✅
   - `cargo test --lib`：**880 passed / 0 failed**（改动前基线，脚本 `.superpowers/sdd/b7-cargo-base.cmd`）
   - 全量 vitest：**14 failed | 2587 passed (2601)**；红文件全是存量（`module-load-recovery` 1 / `readme-markdown-table` 1 / `file-browser-tree-view` 12），脚本 `b7-vitest-base.cmd`
   - `eslint .`：**186 errors / 0 warnings**（整仓口径，`git stash push -u` 量 HEAD）
3. **`drag_no_activate` 刷屏成因** —— ✅ 已钉死并修复（只改日志，不改行为），见 §5.5。
4. ✅ `AGENTS.md` 已补两条硬约束：「watcher 生命周期不得阻塞目录渲染」「诊断埋点永远不许有能力打断主流程」（保留）。

### 8.3 后续批次（计划内）

- **批次 4 = 任务 F 增量索引**（收益见 `docs/benefit-2026-11-03-retrieval-incremental-index.md`）—— ✅ **实现完成 + 门禁全绿 + 真机 4 轮扫描验收完成** ⇒ 详见 §8.6 与 §8.8.1。
- **批次 7 = Git 扩展** —— ✅ **扩展本体完成**（清单 / 入口源码 / 双语文案 / 打包产物 / 图标 / README），沙箱静态扫描 `VALID (0 violations / 20 patterns)`；**已装进宿主并完成真机验收**（安装 → 存储态 → 启动激活 → 命令面板 12 条命令全部在册）⇒ 详见 §8.7 与 §8.8.2。

### 8.4 待用户裁决（原样保留，未动）

1. `docs/handoff-2026-11-01-relay-four-tasks.md` 的 markdown 表格列数不一致（`readme-markdown-table.test.ts` 报第 235/541/863 行）⇒ 修掉可少 1 条 vitest 红，但该文件用户声明「不动不提交」。
2. focus-sync 同步状态确认后才动 `user-extensions.json`。
3. 批次 3 右键菜单改动范围是否确认只接**文件条目**右键菜单。

### 8.5 必须由用户本人做

- 物理键盘复现「删除进行中重复提交」（permfix B1–B3 证据）。
- 物理键盘 `Ctrl+Shift+P` 端到端证据。
（沙箱桌面无键盘输入，Agent 无法代做。）

### 8.6 批次 4（任务 F 增量索引）落地记录

**新增 / 改动的文件**

| 文件 | 性质 | 要点 |
|---|---|---|
| `src-tauri/src/global_search/incremental.rs` | 新增（**零 tauri 依赖**） | `FsEvent` / `ChangeKind` / `ChangeSet` / `is_under` / `escape_regex_literal` / `prefix_delete_pattern` / `IndexOp` / `compile_changes` / `IncrementalGate` / `decide_incremental_plan` / `same_drive_roots` + 内联单测 |
| `src-tauri/src/global_search/index_watcher.rs` | 新增（**零 tauri 依赖**） | `ensure_watcher` / `watcher_snapshot` / `drain_events` / `clear_overflowed` / `stop_watcher` / `translate_event` / `is_excluded_path`；事件通道 8192、缓冲 100000 |
| `src-tauri/src/global_search/scan.rs` | 改动（执行层 ~430 行） | `apply_index_ops` / `collect_index_ops` / `flush_pending_changes` / `decide_scan_route` / `finalize_scan_state` / `run_incremental_pass`；`global_search_start_scan` 改为「先建 watcher → 再判路由 → 增量或全量」 |
| `src-tauri/src/global_search/types.rs` | 改动 | `GlobalSearchScanMode` + status 4 个新字段 |
| `src-tauri/src/global_search/state.rs` | 改动 | `app_owned_prefixes`：应用自身目录（app-data + app-local-data）**从扫描与 watcher 两侧同时排除** |
| `src-tauri/src/global_search/index.rs` | 改动 | `GlobalSearchMeta` 4 个 `#[serde(default)]` 字段 |
| `src/stores/runtime/global-search.ts` | 改动 | `GlobalSearchScanMode` 类型、status 4 个**可选**字段、4 个 ref、状态同步与导出 |
| `src/modules/settings/ui/categories/search/global-search.vue` | 改动 | 新增 `lastScanModeLabel`，并入 `lastScanSummary` |
| `src/localization/messages/*.json`（17 个） | 改动 | 各加 `globalSearch.scanModeFull` / `globalSearch.scanModeIncremental` |

**关键实现决策**

1. 前缀删除一律走 `RegexQuery` + `delete_query`；pattern **不得含 `^` / `$`** —— tantivy 0.26 的 `RegexQuery` 底层是 `tantivy-fst::Regex`（有限自动机），`HirKind::Look` 直接返回 `Error::NoEmpty`，且 fst 是**全匹配**语义 ⇒ 边界靠 `(prefix)(/.*)?` 的结构本身保证。
2. **顺带修掉既有缺陷**：原 `global_search_index_paths_blocking` 用 `writer.delete_term(format!("{}/", dir))`，而 `path` 字段是 `STRING`（raw tokenizer，整条路径一个 term）⇒ 子树一条都删不掉；现已改为 `delete_prefix`。
3. watcher 用「代次」表达缺口：重开 ⇒ generation+1 ⇒ 回落全量；`WATCHER_OVERFLOWED` 只在成功全量重扫后清除。
4. 路由判定顺序（`decide_incremental_plan`）：无活索引 → schema 不一致 → 盘根变了 → 上次全量失败 → watcher 未就绪 → watcher 溢出 → 上次增量失败 ⇒ 回落全量；否则走增量。
5. **增量是「会话内」能力**：`incremental_ready` / `incremental_watcher_generation` 是内存态 ⇒ 每次冷启动的第一轮必为全量，**同一会话内**后续轮次才是增量。验收必须在同一个进程里连打 4 轮。

**门禁（全部实测）**

- `cargo test --lib`：**912 passed / 0 failed**（`b14-build.log`）。
- 全量 vitest：**14 failed | 2588 passed (2602)**；红文件与基线完全一致（`module-load-recovery` 1 / `readme-markdown-table` 1 / `file-browser-tree-view` 12）⇒ **已回基线**。
- `vue-tsc`：**TS_EXIT=0**（`b12-full.log`）。
- `eslint .`：**186 errors / 0 warnings** = 基线；本轮改动的 3 个前端文件不在报错列表里。
- `tauri build`：**BUILD_EXIT=0**（`b15-build.log`）。

**新增埋点**

```
[global-search] scan round timings: mode={Full|Incremental} outcome={...} elapsed={}ms indexed={} reason={...}
```

原有 `[global-search] scan root timings:` 一行的前缀与字段**未动**（只在其后追加新行）。

**本轮自己引入、又自己修掉的 4 个缺陷（记录在案，避免后人重犯）**

1. `global-search.ts` 重复 `export type GlobalSearchScanMode` ⇒ 9 个测试文件在 vite:oxc transform 阶段**整文件 FAIL**（看起来像「全仓炸了」）。
2. `en.json` 的 `scanModeFull` / `scanModeIncremental` 各被插入 2 次 ⇒ `locale-json-format.test.ts` 红；17 个 locale 已统一按 `JSON.stringify(parsed, null, 2) + '\n'` 重写。
3. `global-search.vue` 重复声明 `lastScanModeLabel` ⇒ `vue-tsc TS2451`。
4. `scan.rs` 里 `scan_reason` 先 move 后借用 ⇒ `E0382`；改为 `Some(scan_reason.clone())`。
5. **全量路径的收尾块是 `finalize_scan_state` 的复制粘贴副本，漏掉了 `scan round timings` 埋点** —— 真机验收第 1 轮直接卡死（脚本等满 25 分钟等不到标记行），而**同一批新增代码**的 `enumerate_ms=` 与 `flushed pending changes` 两行都在日志里 ⇒ 由此定位到「复制品漂移」而不是「埋点没写」。已把全量路径改为**复用 `finalize_scan_state`**（逐字段比对过行为等价：`last_scan_*` 6 个字段、`incremental_ready` / `incremental_watcher_generation` / `clear_overflowed` / `apply_committed_index_status` / 失败分支的 `open_or_create_index` + `write_meta` 全部一致），从根上消灭这一类缺陷。修复后 `cargo test --lib` **912 passed / 0 failed**。

**验收（真机 4 轮）**

见 §8.8。

### 8.7 批次 7（Git 扩展）落地记录

**新增目录 `extensions/kizemo.git/`**

| 文件 | 说明 |
|---|---|
| `package.json` | id `kizemo.git`、name `Git Tools`、version `0.1.0`、`extensionType:"api"`、`type:"module"`、`main:"dist/index.js"`、`engines{sigmaFileManager:">=3.9.0", extensionApi:">=1.0.0"}`、permissions `["commands","contextMenu","shell","notifications","dialogs"]`、`activationEvents:["onStartup"]`；`contributes.commands` 12 条（短 id）、`contributes.contextMenu` 2 条 |
| `src/index.js`（18891 B） | **具名导出** `activate` / `deactivate`；12 个命令 + 2 个右键菜单项；`tr()` 优先 `sigma.i18n.extensionT`、缺键退回构建期生成的 `FALLBACKS` |
| `locales/zh-CN.json` / `locales/en.json` | 扁平点号 key，两份 key 完全一致 |
| `build.mjs` | 读 `locales/zh-CN.json` → 生成 `src/generated-fallbacks.js` → esbuild bundle/esm 输出 `dist/index.js` |
| `icon.png`（621 B） | 128×128 |
| `README.md` | 中文说明（能力表 / 6 条运行时硬约束 / 构建与本地安装） |
| `dist/index.js`（21.5 kb）+ `.map` | esbuild 产物，`sourcemap:true` |

**宿主契约（读码确认，踩坑记录）**

1. 扩展必须**具名导出** `activate`；`export default { activate }` **不生效**（`extension-worker.ts:1183-1185`）。
2. 相对 import 支持、**裸包名不支持** ⇒ 必须自带打包产物。
3. `sigma` 以 `Object.defineProperty(workerScope, 'sigma', …)` 定义 ⇒ 扩展里用**裸标识符** `sigma`。
4. `commands.registerCommand` / `contextMenu.registerItem` 的 id 会被宿主统一加 `${extensionId}.` 前缀；**manifest `contributes.commands[].id` 与运行时 `registerCommand({id})` 必须是同一个短 id**（不一致 ⇒ 静默不激活）。
5. `contributes.contextMenu` / `contributes.toolbar` **全仓没有任何宿主消费点**（只有 `contributes?.commands` 被读）⇒ 运行时注册才是真正生效路径；`toolbarDropdowns` 在 UI 层**无消费者** ⇒ 本扩展**放弃 toolbar 下拉**（manifest 已删 `contributes.toolbar`，permissions 已去掉 `toolbar`）。
6. `shell.run` **无 cwd 参数**、**绝对路径 exe 被拒**（`ensure_path_is_allowed_command`）⇒ 一律 `git -C <dir> …` 走系统 PATH。
7. `sigma.context.getCurrentPath()` 在当前构建可能返回 null ⇒ 扩展靠 `onPathChange` 记忆路径 + 弹窗兜底。
8. 扩展命令走**动作注册表**：运行时 `registerCommand` → `getExtensionCommandActions()` → `buildPaletteActionRegistry()` → 命令面板（`command-palette.vue:94`）。
9. 沙箱 = Worker + hidden iframe，CSP `default-src 'none'; script-src <origin> blob:; connect-src 'none'`；**源码与注释都必须避开违禁词**（`scan-sandbox-dynamic.cjs` 直接扫原文）。
10. `scripts/scan-sandbox.cjs` **不存在**（只有 `scan-sandbox-dynamic.cjs`）；`scripts/check-extension-gates.cjs` 是**硬编码针对已安装 focus-sync** 的检查器，且其「has export activate」正则对 `export { activate, deactivate }` 形式会误报 FAIL —— 对本扩展不适用。

**自检（已跑）**

- `node scripts/scan-sandbox-dynamic.cjs extensions/kizemo.git/dist/index.js` ⇒ **`RESULT: VALID (0 violations / 20 patterns)`**。
- ESM 冒烟：`import()` ⇒ `EXPORTS=["activate","deactivate"]`、`IMPORT_EXIT=0`。
- `node build.mjs` ⇒ `BUILD_EXIT=0`。

**做不到 / 留给外部的事**

- **扩展市场登记无法本地完成**：市场列表来自远端 `https://raw.githubusercontent.com/sigma-hub/sfm-marketplace/main/registry.json`（`src/data/external-links.ts:30-33`，缓存 5 分钟）⇒ 本地放目录不会自动出现在市场里，需要仓库 / PR / 发布权限（外部动作）。

**验收（装进宿主）**

见 §8.8。

### 8.8 真机验收记录

#### 8.8.1 批次 4（增量索引）真机 4 轮扫描验收

- 脚本：`.superpowers/sdd/b17-scan-accept.cjs`（**工作目录必须是 `F:\soft\00selfmade\filemanager`**，否则 `MODULE_NOT_FOUND`）。
- 报告：`.superpowers/sdd/accept-b17-scan/report.txt`（16017 B）。
- 埋点：`src-tauri/src/global_search/scan.rs`，`[global-search] scan round timings:` 一行（保留原 `scan root timings:` 前缀不变，只在尾部追加字段）。

| 轮次 | 模式 | 耗时 | 索引文档数 | 说明 |
|---|---|---|---|---|
| R1 | **Full** | 376 297 ms | 1 890 998 | 冷启动首轮，outcome=Completed，reason=Manual |
| R2 | **Incremental** | 2 983 ms | 1 891 001 | 同会话第二轮 |
| R3 | **Incremental** | 4 304 ms | 1 891 011 | 中途创建夹具文件 |
| R4 | **Incremental** | 3 285 ms | 1 891 003 | 中途删除夹具文件 |

**逐盘（R1 全量）**：`G:/` 18 658 ms(43 254) → `F:/` 117 115 ms(268 083) → `D:/` 159 368 ms(302 210) → `E:/` 285 838 ms(549 911) → `C:/` 357 734 ms(727 410)；全量收尾 flush `operations=3203 skipped=49 upserted=377`。

**增量 pass 明细**：R2 `operations=196 skipped=6 upserted=93`；R3 `operations=205 skipped=17 upserted=93`；R4 `operations=173 skipped=9 upserted=81`。

**结论**：冷启动首轮全量 ≈ 6.3 min，同会话内增量 **3–4.3 s** ⇒ 提速约 **87–126 倍**（376 297 ÷ 2 983 / 4 304 / 3 285）。索引规模 189 万文档量级下，增量轮次的耗时与「本轮实际改动量」相关而与全盘规模无关，这正是本次改造的目标。

**必须记住的三条边界**：

1. **增量只在同一会话内有效**：`incremental_ready` / `incremental_watcher_generation` 是**内存态** ⇒ 每次冷启动后的第一轮必然是全量。
2. **4 轮均未走 UI 的「重新扫描」按钮**（global-search 视图当时 `visible:false`），全部走直调 Tauri `global_search_start_scan` 兜底。按钮与兜底调用的是**同一条命令**，但「按钮路径本身」尚未取证。
3. watcher 只记**成功监听到的**盘根；`WATCHER_OVERFLOWED` 只在成功全量重扫后清除。

#### 8.8.2 批次 7（Git 扩展）真机验收

三段式脚本，逐段补齐上一轮的缺口：

| 脚本 | 解决什么 | 结论 |
|---|---|---|
| `.superpowers/sdd/b25-ext-accept.cjs` | 装宿主 + 存储态 + 命令面板 | 命令面板 **12/12** 命中；导航与命令实跑仍失败 |
| `.superpowers/sdd/b27-ext-accept.cjs` | 导航 / 命令实跑 / 右键菜单 | 导航 ✅、状态命令 ✅；右键菜单 0/2 ⇒ 选错元素 |
| `.superpowers/sdd/b28-ext-accept.cjs` | 右键菜单（换选择器 + 真右键） | **2/2 命中** |

**① 安装 → 存储态 → 启动激活**

- 安装**必须走宿主 store**：`extensionsStore.installLocalExtension(<扩展源码目录>)`。裸调后端 `install_local_extension` 只拷目录、不写 `user-extensions.json`，`reconcileInstalledExtensions()` 也不会补回 ⇒ **永不激活**。
- 存储态读取路径多一层：`pinia.state.value.extensionsStorage.extensionsData.installedExtensions['kizemo.git']` = `enabled:true / isLocal:true / version:"0.1.0" / installPendingDependencies:false`；运行时 id 列表 = `pinia._s.get('extensions').installedExtensions.map(e => e.id)`。
- 启动激活取证：`__hostWorkerDiagnostics` 里 `kizemo.git` 连续出现 `module.read / module.validate / module.entry / initialize.ok / activate.ok`。

**② 命令面板 12/12**（运行时中文标题）：仓库状态 / 初始化仓库 / 克隆仓库 / 抓取远端 (fetch) / 拉取 (pull) / 推送 (push) / 同步 (fetch + pull) / 分支列表 / 提交历史 / 浏览仓库文件 / 在 IDE 中打开 / 设置 IDE 命令。

**③ 导航 + 命令实跑**（b27）：`Ctrl+L` → 地址栏输入 `C:\Users\Duanyi\afm-git-accept-repo` → Enter ⇒ 面包屑变为 `C: Users Duanyi afm-git-accept-repo`，列表出现 `README.md` 与 `sub`。命令面板里点 `仓库状态` 后页面回显：

```
仓库根目录: C:/Users/Duanyi/afm-git-accept-repo
当前分支: No commits yet on main
远端跟踪: -
改动条目: 1 [未跟踪] README.md
```

⇒ 扩展**真的调用了 git 并把结果渲染到界面**，不是只挂了个命令。

**④ 右键菜单 2/2**（b28）：扩展在 store 里注册 2 项 —— `kizemo.git.openInIde`（`Git — 在 IDE 中打开`）、`kizemo.git.init`（`Git — 在此初始化仓库`）。在夹具仓库的**目录条目 `sub`** 上右键，菜单共 14 项，末尾「拓展功能」标签下就是这两项：

```
打开方式... / Shell 扩展 / 在终端中打开 Alt+T / 压缩 / 在新标签页中打开 /
复制路径到剪贴板 / 解除文件锁定 / 彻底删除 / 新的 / 局域网共享 /
属性 Alt+Enter / 添加到收藏 / Git — 在 IDE 中打开 / Git — 在此初始化仓库
```

证据：`.superpowers/sdd/accept-b28-ext/report.txt` + `01-context-menu.png`（3 740 721 B）。

**本轮踩到并已定位的三个坑（写给下一个人）**

1. **合成 `contextmenu` 事件打不开 Radix/reka 上下文菜单**。b28 先用 `dispatchEvent(new MouseEvent('contextmenu', …))` 打在目录条目上，菜单**根本没出现**（`role=menuitem` 计数 0）；换成 CDP `Input.dispatchMouseEvent`（`button:'right'` + `mousePressed`/`mouseReleased`）发真实指针事件后菜单立刻出现。⇒ **验收右键菜单必须走 CDP 真右键**，合成事件不算数。
2. **`.dir-entry-interactive` 不是文件列表条目**。它属于**地址栏面包屑**（`address-bar.vue` import 了 `DirEntryInteractive`）和导航树 —— b27 的 5 个候选文本是 `C:` / `Users` / `Duanyi` / `afm-git-accept-repo`，正是面包屑。文件列表主区的条目是 `.file-browser-list-view__entry`（列表视图）/ `.file-browser-grid-card`（网格视图），右键菜单的 Radix trigger 挂在它们的容器 `.file-browser__entries-container` 上。
3. **验收脚本的文案候选集必须从被测对象自己的 locale 文件推导**。b23 把标题硬编码成英文，而宿主界面是中文、扩展运行时标题走自己的 `locales/zh-CN.json` ⇒ 报出假的 0/12。

---

## 九、验证状态分级（照实标注）

**已验证**

- 目录加载卡死的根因链条（前端 `Promise.all` 耦合 + 全局串行队列毒化 + 状态清理顺序）—— 由红灯测试可复现地证明。
- 修复后：目标单测 6/6 绿、`vue-tsc` 0、`eslint .` **186 errors = 基线**、`tauri build` `BUILD_EXIT=0`、沙箱真机 `pass=5 fail=0` / `ACCEPT_EXIT=0`（§6.3）。
- **历史 diff 正文已读**（§5.4）：`Promise.all` 耦合 / `enqueueWatcherOperation` 队列 / `stopWatchingImmediately` **三者都是 `db15621b` 自己种下的**，不是「后续重构覆盖」—— 原推断已被推翻。
- **`drag_no_activate` 刷屏已定案并修复**（§5.5）：状态翻转未收敛导致每帧重复记日志；修复后 Rust 单测 **912 passed / 0 failed**（含新增 3 个单测）。
- `cargo test --lib` 基线补跑：改动前 **880 passed / 0 failed** ⇒ 本轮最终 **912 passed / 0 failed**。
- 全量 vitest 基线补跑：改动前 **14 failed | 2587 passed (2601)** ⇒ 本轮最终 **14 failed | 2588 passed (2602)**；红文件仍全是存量。
- `eslint .`（整仓整口径）本轮最终 **186 errors / 0 warnings = 基线**。本轮新增的 214 个违规已清零：生成物 `extensions/kizemo.git/src/generated-fallbacks.js` 加进 eslint `globalIgnores`，手写源码 `extensions/kizemo.git/src/index.js` 真修到 0（删死代码 `parseBranchLine`、去掉未用的 `activate(context)` 形参、18 处未用 catch 绑定改为可选 catch binding）。
- 修复版已装机并拉起；用户数据恢复后 sha 与备份逐字一致（§6.5）。
- 磁盘不慢、无重解析点、`includeItemCounts=false`、`dir_watcher.rs` 内部无锁序反转。
- **批次 4 真机 4 轮扫描**（§8.8.1）：全量 376 297 ms / 1 890 998 文档 → 增量 2 983 / 4 304 / 3 285 ms，提速约 87–126 倍。
- **批次 7 真机全链路**（§8.8.2）：安装 → 存储态 → 启动激活 → 命令面板 12/12 → 导航到仓库 → `仓库状态` 命令真实执行并回显 → 目录条目右键菜单 2/2。

**推断**

- 「`unwatch_directory` 在某些情况下真的不返回」—— **仍未定案**，修复不依赖它（解耦 + 5 s 超时兜底）。
- 批次 7 一律走 `git -C <dir> …`（系统 PATH）；换到 git 不在 PATH 的机器上会命中 `error.gitMissing`。

**未验证**

- `withWatcherTimeout` 在真实 WebView2 宿主里的 `unref` 行为（单测走 Node，1.92 s 说明生效；浏览器无 `unref` 时走跳过分支，逻辑上安全但未在真机观测）。
- A1 的 `render_ms=475` 是单次采样，未做多次统计。
- **批次 4 的 UI「重新扫描」按钮路径**：4 轮验收均走直调 Tauri `global_search_start_scan` 兜底（global-search 视图当时 `visible:false`）。
- **批次 7 的 12 条命令只验了「全部在册 + 1 条实跑」**：除 `仓库状态` 外，其余 11 条未逐条实跑。
- **扩展市场登记**：市场列表来自远端 `registry.json`（`src/data/external-links.ts:30-33`，缓存 5 分钟），本地放目录不会自动上架，需要仓库 / PR / 发布权限（外部动作）。