# HANDOFF —— 目录树修复 + exe 改名 + 冷启动根治(2026-10-09 夜)

**交接时间**:2026-10-09 22:45
**上一份**:[`handoff-2026-10-09-alpha-fm-brand-release.md`](handoff-2026-10-09-alpha-fm-brand-release.md)
**本轮性质**:§3 目录树 bug → 打包安装验证 → exe 改名 → 冷启动根因定位与修复
**状态**:✅ 三项完成 · ✅ **目录树已在真机验收** · ✅ **全部已提交并推送**(补记于 2026-10-09 22:50)

---

## 0. 三十秒速览

| 项 | 值 |
|---|---|
| **品牌仓** | `sigma-file-manager`,`main` **`3cdc9e7c`**,✅ **12 个文件已提交**(拆为 3 个提交)+ `d2be2373` **已推送**,本地不再领先远端 |
| **主仓** | `filemanager`,`main` **`f950f72`**,无未推送;大量历史 docs 未跟踪,新增 `AGENTS.md` |
| **装机现状** | `D:\Program Files\Alpha File Manager`,**正在运行**,插件已实测正常 |
| **产物** | `src-tauri\target\release\bundle\nsis\Alpha File Manager_2.2.0_x64-setup.exe`(17.12 MB,21:29) |
| **本轮成果** | 目录树不刷新(修·**已真机验收**)+ exe 仍叫 sigma(修)+ 冷启动 180.5s→18.8s(修) |

---

## 1. 本轮完成的三件事

### 1.1 目录树不刷新(§3 的任务)✅ 代码完成,✅ 真机已验收

**根因**:`use-file-tree.ts` 两层缓存(`loadedSet` + `node.isLoaded`)都没有失效路径,按交接文档 §3.3 的要求**没有删 early-return**,而是新增独立失效通道。

- 新增 `invalidate(path)`:同时清 `loadedSet` 与 `node.isLoaded`,**按整棵子树清**(否则重载父节点会换掉子节点对象,而 `loadedSet` 仍声称它们已加载 → 已展开的孙节点永久空白)
- 重读**按父先于子、串行 await**(父的 `loadChildren` 会整体替换 children 数组,子若并发查找会写到已废弃的旧节点上)
- 新增 store 通道 `markTreeStale` / `consumeStaleTreePaths`,由 `createNewItem` / `confirmRename` / `deleteItems` 发布,tree-view 消费

> **§3.4 的回归项成立**:删除与重命名是**同一条缓存**,同样坏,已一并修好。

### 1.2 exe 改名:`sigma-file-manager.exe` → `alpha-file-manager.exe`✅ 已装机实测

改的是 **`Cargo.toml` 的 `[package] name`**(交接文档 §4 #28 指的那个)。

| 改动 | 位置 | 为什么 |
|---|---|---|
| 包名 | `Cargo.toml` | exe 名由 Cargo 包名决定,`productName` 只管安装包名和窗口标题 |
| kill 列表 | `installer/hooks.nsh` | **不加这条,覆盖重装会静默失败**(Tauri 吞掉原因) |
| 清理遗留 | `installer/hooks.nsh` POSTINSTALL | 否则 44MB 旧 exe 变孤儿,与新 exe 并存 |

**受保护常量一个没动**:`identifier` 仍是 `com.sigma-file-manager.app`(数据目录 `%APPDATA%\com.sigma-file-manager.app` 因此未搬家,已验证);`MDNS_INSTANCE_NAME`、`LEGACY_DEFAULT_FILE_MANAGER_DATA_DIR_NAME` 未动。

**已验证不会破坏"接管默认文件管理器"**:`windows_installation.rs` 的 `APP_EXECUTION_ALIAS_FILE_NAME` **只在微软商店版生效**,而 NSIS 版走 `launcher.rs:30` 的 `current_exe()` 动态取路径。

### 1.3 冷启动 180.5s → 18.8s✅ 同法实测

**根因**:`extensions.ts` 的 `init()` 把**本地已装插件的激活排在三次联网操作之后**——拉注册表 → 探测 6 个仓库可达性 → 查 6 个版本。而 `raw.githubusercontent.com` 在境内**时好时坏**(实测:同一域名,marketplace 仓库 30s 超时,clipboard 仓库 0.96s 通),每个探测都要烧满连接超时+重试才回退到 jsDelivr。

| | 窗口出现 | 插件激活 | 激活耗时 |
|---|---|---|---|
| 改前 | 0.60 s | 180.50 s | **180.5 s** |
| 改后 | 0.45 s | 19.21 s | **18.8 s** |

**改动 A**:本地插件先加载,市场同步改后台不阻塞(`runStartupMarketplaceSync`,`void` 不 await)。
**改动 B**:连接超时 15s → 5s(`extensions/http.rs`),更快切到已内置的 jsDelivr 回退。

**后台同步功能未受损**(专门验过):`registryCache.fetchedAt` 正常刷新,注册表 6 个插件照常拉到。

---

## 2. ✅ 所有改动已提交并推送(2026-10-09 22:50 补记)

原先 12 个文件 modified、0 提交的状态已结束。按语义拆成 3 个提交,方便日后单独回滚:

| 提交 | 范围 | 文件 |
|---|---|---|
| `b28e15b0` | 目录树缓存失效通道 | 7 个(前端 `src/**`) |
| `1e03dcba` | exe 改名 | 3 个(`Cargo.toml`/`Cargo.lock`/`hooks.nsh`) |
| `3cdc9e7c` | 冷启动 180.5s→18.8s | 2 个(`extensions.ts`/`http.rs`) |

对账:`d2be2373..3cdc9e7c` 合计 **12 files / 837 insertions / 93 deletions**,与提交前的 `git diff --stat` 逐字节一致——无丢失、无夹带。

推送验证:推送前 `origin/main` = `a3577184`,推送后 `git ls-remote origin refs/heads/main` 直查 = `3cdc9e7c`,与本地 HEAD 一致,`rev-list --count origin/main..HEAD` = 0。

> ⚠️ **主仓仍有未跟踪文件**:本轮新建的 `AGENTS.md`,以及大量历史 `docs/**`(交接/计划/评审文档)。本轮按约定**未动主仓**,需要时单独提交。

---

## 3. 验证证据与门禁

| 门禁 | 结果 |
|---|---|
| `vue-tsc --build` | ✅ 无输出 |
| `cargo check --release` | ✅ Finished |
| eslint 全仓 | ✅ **225 problems / 104 errors —— 与改动前基线完全一致,零新增** |
| 扩展 store 测试 | ✅ 14/14 |
| 本轮新增测试 | ✅ **18 项**(use-file-tree 5 + folder-tree 10 + tree-view 3),连续跑 3 轮 55/55 |
| NSIS 警告门禁 | ✅ PASS,0 warnings |
| 沙箱门禁 | ✅ VALID(0 violations / 19 patterns) |
| 装机 exit code | ✅ 0 |
| 插件健康 | ✅ `extension_alive=True`、`uia_healthy=True`、`first_push_received=True` |

---

## 4. ❌ 未做 / 未验证

- ~~**目录树刷新修复未经真机确认。**~~ ✅ **已验收(2026-10-09 22:45)**:用户在 21:49 启动的装机版本上实测——展开目录 → 新建文件夹 → **不折叠不重展**,新节点**直接出现**。测法对应下述路径。
- **剩余 18.8 秒**未继续挖。关键路径上已无网络,剩下是纯本地开销(Worker 创建、沙箱校验、清单对账)。
- **复制/移动文件后目录树不刷新** —— §3 范围只含新建+删除+重命名,粘贴/移动没接上。
- **`startup-recovery.js` 残留旧名**:文件里是 `Failed to load Sigma File Manager...`,导致 `module-load-recovery.test.ts` 长期失败。这是 P4 改名的遗留,**已验证在改动前就失败**,不是本轮引入。因"命名已锁定"未动。
- **全量单测 flaky**:每次跑失败数在 1~4 间跳动,且失败文件每次不同;已用 stash 对照确认**改动前同样如此**。别拿"通过率"当结论。
- **卸载路径仍未端到端验证**(本轮只做了覆盖重装)。

---

## 5. 本轮的方法论教训(三次栽在同一类坑上)

### 5.1 测速对象必须是"被测对象真实走的那条路径"

我三次误判,全部因为测的东西不是应用实际用的:

| 我测的 | 应用实际用的 | 后果 |
|---|---|---|
| `api.github.com`(0.96s) | `raw.githubusercontent.com`(30s 超时) | 误判"网络不是瓶颈" |
| PowerShell(会读系统代理) | Rust `reqwest`(**不读** Windows 系统代理) | 误判"代理可用" |
| `last_ping_received_ts` | `last_health_poll_ts` | 误判"心跳停止、闸门将再关" |

**规律**:验证任何性能/连通性断言前,先确认**被测对象走的是哪条链路**——域名、协议栈、进程、字段。名字相似的两个域名、两个字段、两套 HTTP 客户端,行为可以天差地别。

### 5.2 判别性探针要打在"会变"的东西上

`-NoProxy` 那次探针给出 0.01s 的"失败",我一度当证据用——其实是 PowerShell 5.1 参数不兼容,请求根本没发出去。**探针本身也要自证**:先确认它在"什么都没做"时给出预期的失败,再拿它的输出下结论。

### 5.3 测量方法:窗口出现 vs 插件激活

分离"应用启动慢"与"扩展加载慢"的通用手法:

```powershell
$t0 = Get-Date; $p = Start-Process $exe -PassThru
# 轮询 MainWindowHandle != 0 → 窗口出现耗时
# 同时盯侧车日志出现 'EXTENSION ALIVE' → 插件激活耗时
```

本次靠它一刀切开:窗口 0.6s、激活 180.5s ⇒ **应用启动无辜,100% 在扩展管线**。

---

## 6. 运维命令

```powershell
$repo = "F:\soft\00selfmade\filemanager\sigma-file-manager"
$env:PATH = "C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;$env:PATH"

cd $repo
npm run tauri build                                    # 约 8 分钟(前端 4.5 + Rust 全量)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-nsis-warnings.ps1
node scripts\scan-sandbox-dynamic.cjs extensions\kizemo.focus-sync\dist\index.js src\modules\extensions\runtime\sandbox.ts
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-deploy-sha.ps1

# 侧车健康(**判断插件是否活着;改后冷启动约 19 秒,不用再等 6 分钟**)
(Invoke-RestMethod http://127.0.0.1:37421/health) | Select-Object extension_alive,first_push_received,last_health_poll_age_ms
```

> ⚠️ `last_ping_received_ts` 是**快照字段,早已陈旧**;判断闸门开合只看 `last_health_poll_age_ms` 与 `extension_alive`(见 §5.1)。

---

## 7. 下一步(按建议顺序)

| # | 任务 | 说明 |
|---|---|---|
| ~~**1**~~ | ~~真机确认目录树刷新~~ | ✅ **已完成**(2026-10-09 夜),见 §1.1 / §4 |
| ~~**2**~~ | ~~提交 + 推送~~ | ✅ **已完成**,拆为 3 个提交,见 §2 |
| **3** | 复制/移动后目录树刷新 | **现在最要紧**。同一条缓存的最后一类入口,`paste`/`move` 还没调 `markTreeStale` |
| 4 | 挖剩余 18.8 秒 | 已无网络,是 Worker/沙箱/对账的本地开销 |
| 5 | 修 `startup-recovery.js` 旧名 | 一条长期失败的测试,纯遗留 |
| 6 | 端到端验证卸载 | 从没验过 |

---

## 8. 给新会话的一句话 prompt

```
读 F:\soft\00selfmade\filemanager\docs\handoff-2026-10-09-tree-fix-exe-rename-coldstart.md(必读),
并读 AGENTS.md(输出语言规定:一律简体中文)。
§4 第 1 条(目录树真机验收)与 §2(提交推送)均已闭合,不必重做。
当前最要紧的是 §7 第 3 项:复制/移动后目录树仍不刷新——paste/move 还没调 markTreeStale,
与本轮修好的新建/重命名/删除是同一条缓存的最后一类入口。
主仓仍有未跟踪文件(AGENTS.md 与大量 docs/**),需要时单独提交。
验证性能/连通性时注意 §5:必须测被测对象真实走的那条链路(api.github.com ≠ raw.githubusercontent.com,
PowerShell ≠ Rust reqwest,last_ping_received_ts ≠ last_health_poll_ts)。
品牌名已锁定,不要重新讨论命名或图标。
```