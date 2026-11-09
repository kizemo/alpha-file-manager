# 2026-11-09 追加批次 C · 三份只读审计落盘快照

> 目的：把本轮三条只读审计子任务的**实测结论**固化到仓库，避免接力时重复劳动。
> 子任务 ID（供追溯）：
> - `bg_2f62e302-6dbf-4dd3-8597-1982c96db032` 百度网盘写操作与本地磁盘对齐度
> - `bg_2c8a9951-ef86-44d1-b3a1-46fb39d1e57c` 顶部状态中心与传输进度链路
> - `bg_e1890a92-37b4-4e0d-af21-54506a5020b2` 远端传输并发与分片现状
>
> 全部行号均为**当前工作副本实测**（HEAD `118884dd` + 未提交改动）。简报里给错的落点已在文末单列。

---

## 0. 一句话结论

反馈 1/2 的真凶是登录 harvest 短路（已定方案，见 §4）；反馈 3 的差距比「有 bug」大得多——**百度网盘上传根本没实现**、复制/移动**没有 trait 方法**；反馈 4 的基础设施已经够用（状态中心 store 可编程、远端进度 sink 已聚合批量），缺的是「写进状态中心」和「删除有进度」；反馈 5 的前提是先有分片上传器，当前远端传输**全程单连接串行、整文件进内存**。

---

## 1. 反馈 3：写操作面与本地语义的差距

### 1.1 后端 provider（`src-tauri/src/remote/baidu_provider.rs`）

| 能力 | 落点 | 状态（实测） |
|---|---|---|
| 列目录 | `impl :632` → `read_dir :633`（list_all `:516` 分页 / stat `:593`） | 已实现 |
| 读文件 | `read_file :645`（dlink；`fs_id=0` → Unsupported `:667`；>500MiB 拒 `:960`） | 已实现但有内存上限 |
| **写文件（上传）** | `write_file :695` / `write_file_with_progress :704` → `reject_upload :622-628` | **未实现，统一 Unsupported** |
| 新建目录 | `make_dir :714` | 已实现 |
| 删除 | `remove_path :753`（删根目录拒 `:763`） | 已实现，单路径 |
| 重命名 | `rename_path :770`（`:776` 禁新名含 `/`） | 已实现，禁跨目录改名 |
| 权限 | `set_permissions :727` Unsupported（`:735`）/ `get_permissions :740 → Ok(None)`（`:750`） | 百度网盘无此概念 |
| **复制 / 移动** | **trait 上不存在** | 见 §1.4 |

`src-tauri/src/remote/provider.rs:39-183` 共 **9 个方法，无 copy/move**。

### 1.2 百度 API 层（`src-tauri/src/remote/baidu_api.rs`）

- 已有写端点封装：`create_dir_url :1022`（`/api/create?a=commit&isdir=1`）、`filemanager_url :1035`（`opera=rename|delete&async=2&onnest=fail`）、`rename_filelist :1231`、`delete_filelist :1238`、`bdstoken_url :1123`、`parse_bdstoken :1059`、`fetch_bdstoken :1425`、`post_form :1378`、`parse_write_response :1329`；`-6 => BaiduFailure::Auth` 在 `:619`，会话失效判据说明在 `:722`。
- `:967-982` 注释自陈：写操作端点形态**未实证**（依据是 bypy/cpan 等社区实现）。
- 缺的封装：上传三段式（`precreate` → 分片 `superfile2`/`blockupload` → `create` 带 `block_list`）、copy、move、回收站与彻底删除分离、批量 filelist（现在只塞一条）。
- 全 `remote/` 目录 grep `precreate|superfile2|uploadid|chunk upload` **零命中**；唯一 `block_list=[]` 是 mkdir 参数（`:1024`）。

### 1.3 前端能力判定与入口（**不存在 capabilities 表**）

- 协议联合类型与判定：`src/types/remote.ts:15`（含 `baidu_pan`）、`:173 isBaiduPanProtocol()`。
- 右键菜单可见性：`src/modules/remote/utils/remote-context-menu-visibility.ts:42 protocolSupportsPermissions()`（只认 `'sftp'`）、`:125`「上传到远程」**不看协议**、`:160 isRemoteDeleteActionVisible()` **不看协议**。
- 后端按协议分发：`src-tauri/src/remote/commands.rs:546-556 provider_with_protocol`；`:746-752 remote_chmod` 显式拒非 SFTP（`:750` 专门给百度网盘一句话）；`:807-830` remove/make/rename **无协议门禁**。
- 结论：入口可见性与后端能力**不对齐**——上传入口对百度网盘开放，但后端必返 Unsupported。

### 1.4 复制 / 移动 / 回收站的落点（跨层）

- 本地语义参照：`src-tauri/src/file_operations.rs` `delete_items :1893`（`:1919-1921` 回收站/永久分流）、`copy_items :1442`、`move_items :1808`、`copy_dir_recursive :253`（进度计数 `:160`）、`rename_item :1828`（重命名与移动本就是两条命令）。
- 远端编排：`src/modules/remote/utils/remote-transfer-service.ts:364-398` 本地↔远程可批量，**远程→远程直接弹「不支持」toast（`:391-398`）**；`handleRemotePaste` 在 `src/modules/navigator/components/file-browser/composables/use-dir-entry-actions.ts:132-193`；`src/modules/remote/utils/remote-write.ts:42-51` 把「远程新建空文件」明确写成**永久非目标**。
- 回收站双轨**做不到等同本地**：远端只有一条 `op=delete`，落点（进回收站还是彻底删）由服务端决定，未实证。前端 `use-file-browser-selection.ts:1449-1456` 注释明确「刻意不复用 delete / delete-permanently」，菜单只有 `file-browser-actions-menu.vue:425` / `:676` 一项 `remote-delete`。

### 1.5 施工顺序（子任务给的）

1. trait 加 `copy_path` / `move_path`（provider.rs 9 个实现都要补默认或 Unsupported）；
2. `baidu_api.rs` 补 copy/move（复用 `filemanager_url`）与上传三段式；
3. `baidu_provider.rs` 用真实现替换 `reject_upload`（`:622`）；
4. 前端补编排：`remote-write.ts`、`use-dir-entry-actions.ts` 放行远程→远程；
5. `remote-context-menu-visibility.ts:125` 等入口加协议维度。

**硬序约束**：2 和 3 必须对着**有效会话**实测，否则端点形态无从证实（findings §4 已证「有会话时请求形态错」，无会话时只回 `-6`）。

---

## 2. 反馈 4：状态中心与进度链路

### 2.1 状态中心（目录是 `src/modules/status-center/`，不是 `src/components/`）

- UI：`status-center-toolbar-button.vue`（挂在 `src/modules/window-toolbar/window-toolbar.vue:14`）+ `status-center-operation-row.vue` + `use-cancel-operation.ts`。
- store：`src/stores/runtime/status-center.ts:94`，`isOpen = ref(false)`（`:96`），**可编程入口 `setOpen/open/close/toggle`（`:201-215`）**；Popover 用 `v-model:open`（`status-center-toolbar-button.vue:29-34、:180`）⇒ 任何地方调 `statusCenterStore.open()` 即可展开。
- **按钮的 pill 视觉已由 `hasActiveOperations` 自动驱动（`:37`、`:187`），但弹层不会自动开**。全仓无「自动展开」先例，唯一程序化开面板是 `src/composables/use-init.ts:100-102 globalSearchStore.open()`。
- `Operation` 接口（`status-center.ts:23-59`）已含 `progress / processedCount / totalCount / phase / totalBytes / freedBytes / itemPercent / bytePercent / currentPath`，展示层判定在 `src/utils/progress-presentation.ts`（被 `status-center-operation-row.vue:25-31` 使用）⇒ **复用模型现成**。

### 2.2 本地作业是怎么写状态中心的（可照抄的四条）

| 作业 | 前端 store | 后端事件 | payload |
|---|---|---|---|
| 复制/移动 | `src/stores/runtime/copy-move-jobs.ts:117-135` | `copy-move-job-progress`（`src-tauri/src/copy_move_job.rs:84`） | `{jobId,percent,detail,processedCount,totalCount}`（`:27-35`） |
| 删除 | `src/stores/runtime/delete-jobs.ts:185-200` | `delete-job-progress`（`src-tauri/src/delete_job.rs:397`） | 含 `phase/scanCompleted/deletedItems/freedBytes/totalBytes/itemPercent/bytePercent/files/directories/currentPath`（`:35-72`） |
| 归档 | `src/stores/runtime/archive-jobs.ts:79-132` | `archive/jobs.rs:201、:255` | — |
| 目录大小 | `src/stores/runtime/dir-sizes.ts:179、:386` | `dir-size-updated` | — |

**无事件总线**，各 store 各自 `listen`。

### 2.3 远端侧现状（缺口）

- 上传/下载：只有一个进度事件 `remote-transfer-progress`（`commands.rs:717`，payload camelCase `{jobId,filesDone,filesTotal,bytesDone,bytesTotal,currentPath}`，`:689-698`），由 `TransferProgressSink`（`src-tauri/src/remote/transfer.rs:114-227`，`set_totals()` `:168`）聚合 ⇒ **已经是「批量整体进度」**，符合用户要的「按整体进度」。
- 但它被前端写进**独立对话框**：`remote-transfer-service.ts:181-191` → `src/modules/remote/utils/remote-transfer-state.ts:147、:167` → `remote/components/remote-transfer-progress-dialog.vue`，**没写状态中心**。远端传输无 finished 事件，结果靠 invoke 的 Promise 返 `RemoteTransferReport`。
- **远端删除零进度上报**：`commands.rs:807 remote_remove_path` 一次只吃一个路径、无 jobId、无 emitter → `baidu_provider.rs:753-768` → `baidu_api.rs:1238 delete_filelist`。前端 `use-file-browser-selection.ts:1699-1747 deleteRemoteItems` → `src/modules/remote/utils/remote-delete.ts:52-73` **顺序 for-await 逐个 invoke**，失败不中断，结束只 toast。
  - ⚠️ 关键发现：百度 `filelist` 本身就是 JSON 数组（`baidu_api.rs:1231-1241` rename/delete 同形），**天生支持一次删多条，现在只塞 1 条**。SFTP/FTP/FTPS 删除分别在 `sftp_provider.rs:646`、`ftp_provider.rs:893`、`ftps_provider.rs:524`。

### 2.4 反馈 4 必改清单（子任务给的）

| # | 文件 : 落点 | 改动 |
|---|---|---|
| 1 | `src/stores/runtime/status-center.ts` | `OperationType` 扩 `remoteUpload/remoteDownload/remoteDelete`；在 store 侧集中「活动作业开始 → `open()`」（含「用户手动关过就不打扰」）⇒ **必须加在 store 而非组件，组件可能未挂载** |
| 2 | `src/modules/remote/utils/remote-transfer-service.ts:181-191` | `watchProgress` 同时写状态中心，并在 Promise 收尾时 `completeOperation` |
| 3 | `src/modules/remote/utils/remote-delete.ts:52-73` | 循环内按条目上报「已删 N / 总数 M」 |
| 4 | `src/modules/navigator/.../use-file-browser-selection.ts:1699-1747` | 改为「建作业 → 走状态中心」 |
| 5 | `status-center-operation-row.vue` + `use-cancel-operation.ts:23-69` | 补新 type 的文案与取消分支。⚠️ **不加取消分支有真风险**：`:58-62 hasCancellableActiveOperations` 对所有 `in-progress` 都返回 true，按钮会出现但点了空转 |
| 6 | `src/localization/messages/**`（17 个包） | 补 `statusCenter.operationTypeShort.*` 等 |
| 7 | `src-tauri/src/remote/commands.rs:807` | 加 `remote_remove_paths`（带 `job_id`）+ `remote-delete-progress` / `-finished`（`:704-719` 的 progress_sink 现成可仿） |
| 8 | `src-tauri/src/remote/baidu_api.rs:1238` | `delete_filelist` 改吃 `&[&str]` 一次批删（N 次往返压成 1 次） |
| 9 | `src-tauri/src/remote/provider.rs:147` | 决定是否加递归/批量 trait 方法（目录非空当前直接被拒） |

上传/下载事件 payload **无需改动**。

---

## 3. 反馈 5：并发与分片现状

### 3.1 实测：远端传输全程单连接串行

- 上传链：前端 → invoke `remote_upload_paths`（`commands.rs:575`）→ `transfer::upload_many`（`transfer.rs:583`）→ **顺序 for 循环（`:664`）** → `upload_file`（`:805`）→ `std::fs::read` **整文件进内存（`:830`）** → `provider.write_file_with_progress`（`:859-863`）。
  - SFTP/FTP 把整块 buffer 按 256KB 顺序写单连接：`provider.rs:29 REMOTE_WRITE_CHUNK_BYTES`、`sftp_provider.rs:528-533`、`ftp_provider.rs:449-454`。
- 下载链：`remote_download_paths`（`commands.rs:618`）→ `download_many`（`transfer.rs:1036`，循环 `:1063/:1124`）→ `download_file`（`:1261`）→ `provider.read_file` **整个文件收成 `Vec<u8>`（`:1279`）** → `std::fs::write`（`:1294`）。
  - 百度：`baidu_provider.rs:645` → `file_dlink`（`:292-294`，走 `filemetas?dlink=1`，`baidu_api.rs:233-241/:901`）→ `download_file`（`baidu_api.rs:920-965`）单 GET + `bytes_stream()`，上限 500MiB（`baidu_provider.rs:73`，超限 `baidu_api.rs:954-958`）。
- **无 Range 分段、无分片临时文件拼接**（grep 证实）。
- ⚠️ 简报给的命令名 `remote_upload_file` / `remote_download_file` / 文件 `download_stream.rs` **都不存在**。

### 3.2 线程模型与红线

- 两个 client 在 `baidu_api.rs`：`build_client :807`（30s 超时、不跟随重定向）与 `build_download_client :888`（900s、redirect limited(5)、connect 30s），在 `HttpBaiduApi::new`（`baidu_provider.rs:210-211`）构造；provider 每次命令调用新建（`commands.rs:540-553`），同一次传输内共享（reqwest 自带连接池）。
- **红线已守住**：`transfer.rs:9-12` 明文禁止 tauri 路径；本地 I/O 用自建 `blocking()` = `tokio::task::spawn_blocking`（`transfer.rs:416-422`），模块注释 `:21-27` 专门解释为何不踩 `GlobalRuntime<Wry>` 坑 ⇒ **新增并发只要用 tokio/futures 原语（`buffer_unordered` / `JoinSet` / `Semaphore`）就不影响测试链接**。`futures` 已在用（`baidu_api.rs:946-948`）。
- 本地并发先例：`dir_size.rs` 用 rayon（`:8/:193/:367/:449`）；`compare/compare.rs:268` 与 `delete_tree.rs:759` 用 `std::thread::scope`；`copy_move_job.rs:96-110` 是 tokio::spawn + 单个 spawn_blocking（任务内部仍串行）。
- 前端无传输队列/并发上限（无 maxParallel）；进度按 `jobId` 过滤，天然支持多任务并行事件（`remote-transfer-service.ts:181-191`）。

### 3.3 最小可行改造路径（子任务给的，按优先级）

1. **先实现百度分片上传本身**（并发的前提）：`baidu_api.rs` 加三段式；`baidu_provider.rs:695-712` 替换 `reject_upload`。分片形态天然可用 `Semaphore(N)+buffer_unordered` 并发传 chunk，进度经 `write_file_with_progress` 回调逐片累加。
2. **多文件并发**：`transfer.rs:664` / `:1124` 的循环改 `futures::stream::iter(...).buffer_unordered(N)`；`TransferProgressSink` 已是可 clone 的共享汇聚器（`transfer.rs:33` Arc+Mutex、`:854` clone 用法），并发下 `files_done/bytes_done` 原子累加即可；`current_path` 语义需改为「最近变更」或去掉。并发度 N 建议进 `user-settings` 由命令参数传入。
3. **百度下载 Range 分段**：`baidu_api.rs:920` 先探测 `Accept-Ranges`，支持则 N 个 `Range: bytes=` 并发 GET 写各自临时文件再拼接；同时把整文件进内存改成流式落盘（500MiB 内存上限是硬伤）。CDN 是否支持 Range **需真机实证**。
4. **SFTP/FTP 多文件并发**：russh 可在单 session 并行开多 channel；FTP 需多控制/数据连接，代价更高，放最后。
5. **前端配套**：保持单批一次 invoke（并发收在后端更好汇总）。

**风险**：① 并发后内存翻倍，整文件 `Vec<u8>` 设计需先改流式；② 百度未文档化的限速/封号风险无代码证据，需真机小并发试起；③ 新增并发务必留在 tokio/futures 原语内。

---

## 4. 本轮已定稿的登录修复方案（尚未落码）

落点全部在 **`src-tauri/src/baidu_login.rs`**（不在 `remote/` 下）：

| 现状落点 | 改动 |
|---|---|
| `poll_loop :404` 建窗后直接开始 `read_cookies` | 轮询开始前 `purge_stale_auth_cookies(&window)`：清 jar 里的 `BDUSS` / `STOKEN` |
| `read_cookies :382`（另起线程 + `recv_timeout`，`COOKIE_READ_BUDGET=10s :74`） | 仿它写一个带超时的 delete 包装 |
| `PendingSession :86` / `store_pending :514` / `emit_event(...,"ready") :516` / `close_login_window :517` | Ready 之前插入**服务端验活**三连；只有 Live 才 store + emit + close |
| `records :485` / `scan_cookies(&records) :488` | 直接从 records 取 BDUSS/STOKEN 原文拼 Cookie 头 |
| `MAX_READ_FAILURES :76` / `STOKEN_WAIT=20s :66` / `GIVE_UP=900s :65` | 暂不改，验活失败继续轮询、不关窗不落盘 |

方案要点：

1. **清 jar**：`wry` 0.57.0 `src/lib.rs:2124/:2134/:2143/:2152` 与 `tauri` 2.12.0 `webview_window.rs:2715/:2740/:2749/:2758`（`webview/mod.rs:2423/:2452/:2461/:2474` 同名）均提供 `cookies_for_url / cookies / set_cookie / delete_cookie` ⇒ 已验证存在。
2. **基线比对**：清 jar 后立刻采 `AuthBaseline { bduss, stoken }`；Ready 必须满足「当前 BDUSS 与基线不同」——清 jar 失败时的第二道保险。判据只用 BDUSS（会话身份）。
3. **服务端验活**：新增纯函数 `judge_session_probe(status, body) -> SessionVerdict{Live, Rejected, Inconclusive}`：200 且 `errno==0` ⇒ Live；`errno==-6` 或 3xx/401/403 ⇒ Rejected；**其余一律 Inconclusive ⇒ 放行**（安全极性，不把有效会话判死）。探针用与读接口同口径的 `GET /rest/2.0/xpan/file?method=list&limit=1...`（findings §4-B 实测该 URL 在失效会话下回 `-6`）。
4. **验活的 HTTP 能力**：`src-tauri/Cargo.toml:54` tokio 特性已含 `rt / rt-multi-thread / sync / io-util / net / time` ⇒ 用**一次性 current-thread runtime + `block_on`**；reqwest `:59` 仍是 `0.13 / default-features=false / ["rustls","stream"]`（无 `blocking`）⇒ **不需要新增特性、不碰 `tauri::async_runtime`、红线不触发**。
5. **不新增事件 status**（避免 17 个语言包的连带改动）；Rejected 只 `log_line` 并继续轮询，用户可重扫；用 `last_rejected_bduss` 记住已拒值，避免每 2s 重复打服务端。
6. `src-tauri/src/baidu_login.rs` **没有** `#[cfg(test)] mod tests` ⇒ 纯函数需自带测试模块或放进 `remote/baidu_auth.rs`（`remote/baidu_auth.rs:198` 有 `#[cfg(test)]`）。

---

## 5. 本轮校正掉的错误落点（勿再照抄简报）

| 简报说法 | 实测 |
|---|---|
| `src/lib/remote/types.ts`、`RemoteConnectionCapabilities`、`use-drive-context.ts`、`use-file-operations.ts`、`src-tauri/src/dir_writer.rs`、`src/command/` | **全仓 0 命中**。实际是 `src/types/remote.ts`；远程写编排是 `src/modules/remote/utils/remote-write.ts`；本地删除是 `src-tauri/src/delete_job.rs` + `file_operations.rs`；无 capabilities 表 |
| `src/components/` 下的状态中心 | 实际 `src/modules/status-center/` |
| `remote_upload_file` / `remote_download_file` / `download_stream.rs` | 不存在。实际 `remote_upload_paths`（`commands.rs:575`）/ `remote_download_paths`（`:618`）/ `src-tauri/src/remote/transfer.rs` |
| 「内嵌登录把失效会话判成已登录」是推断 | **本轮已证实为真缺陷**（findings §5 时序 + §7-② bdstoken 空串） |
| 「BDUSS 存在即已登录」 | 错。`remote/baidu_auth.rs:54-57` 注释自己也说「存在不等于服务端仍有效」⇒ 必须验活 |
| `SftpError::Auth`（`sftp.rs:23` 注释）作为判据先例 | 百度网盘侧真正的先例是 `remote/baidu_api.rs:599-623`：`-6 => BaiduFailure::Auth`、`-6` 语义「用户未登录/会话失效」，`:611` 注释已引用 T1 探针 |

---

## 6. 工具坑（本轮新增，务必读）

1. **`delete_cookie` 的可用性**只能从 crate 源码坐实：`C:\Users\Duanyi\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\wry-0.57.0\src\lib.rs` 与 `tauri-2.12.0\src\webview\webview_window.rs`。项目里 `baidu_probe.rs:173` 用的是 `cookies_for_url`（URL 版，**清不掉**），必须用 `delete_cookie`。
2. **PowerShell 格式化里写 `$_.Line.Trim`（不带括号）会把 MethodInfo 打进输出**，整屏是 `string Trim(), string Trim(char trimChar)...` 之类的垃圾。必须 `$_.Line.Trim()`。
3. `read` 对 crate 源码目录用 `offset/limit` 时，`limit` 超过剩余行数会报 `file part is beyond end of file`（`baidu_login.rs` 读 `:404` offset + limit 330 触发）⇒ 用 smaller limit 或分段。
4. bash 工具的 CWD 是 `F:\soft\00selfmade\filemanager`（**不是**仓库子目录），`.superpowers\sdd` 两处都有，探针只在 `sigma-file-manager\.superpowers\sdd`。
5. `baidu-login.log` 取证脚本必须用 **`C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe`（5.1）**；pwsh 7.6.9 里 `[System.Security.Cryptography.ProtectedData]` 装配解析崩（`Could not load file or assembly ... Version=7.6.0.500` / `Version=10.0.0.0`，且脚本内 try/catch 兜不住）。日志是**无 BOM UTF-8**，`Get-Content -Encoding UTF8` 可用。
6. **交接卡串台**：`E:\Users\Duanyi\.minimax\.minimax\maintenance\task-state\LATEST.md` 本轮内容是**另一个项目**（practilog / design demos）的残留，按 AGENTS.md 与既往教训——核对项目归属后再采用，不属于本会话就忽略。

---

## 7. 本轮登录修复已落地的形态（`src-tauri/src/baidu_login.rs`）

「三道保险」全部落在 `baidu_login.rs`，接线点在 `poll_loop`：

| 保险 | 实现 | 接线位置 | 语义 |
|---|---|---|---|
| ① 建窗后清 jar | `purge_stale_auth_cookies(&app, &window, &url)` | `poll_loop` 循环之前（紧跟 `let started = Instant::now();`） | 用 `window.delete_cookie()` 删掉 jar 里遗留的 `BDUSS` / `STOKEN`，并返回删除前的基线快照 `AuthBaseline` |
| ② 基线比对一票否决 | `has_new_bduss(&baseline, Some(current_bduss))` | `CookieRead::Ok` 分支的 Ready 判定之后 | 当前 BDUSS 与基线同值 ⇒ 判定为旧 jar 遗留，`continue` 继续等真登录，不 Ready；按值去重打日志（`stale_logged`） |
| ③ 服务端验活 | `probe_session_live(&cookies)` → `SessionVerdict` | `let state = BaiduSessionState::new(...)` 之前 | 只有确凿证据（3xx / 401 / `errno=-6`）才拒绝；403/429/5xx、传输错误、非预期 errno、读不出 errno 一律 `Inconclusive` 放行 |

配套的纯函数（可单测，`poll_loop` 只接线）：`probe_errno`（**字节滑窗**扫 `errno`，不用 serde）、`judge_session_probe`（状态码 + errno → 极性判定）、`current_bduss`（同名取第一条）。

**防轰炸**：同一个被判死的 BDUSS 记进 `rejected_bduss`，同值不再重复探针、不再重复日志 —— 否则每 2 秒一次打服务端 + 刷日志。

### 本轮实测的 crate / 依赖事实（照抄即可，别再猜）

- `Cargo.toml`：`tauri = "2.11"`；`reqwest = { version = "0.13", default-features = false, features = ["rustls", "stream"] }` ⇒ **没有 `blocking`、没有 cookie jar** ⇒ 验活必须手工拼 `header::COOKIE`（`baidu_login.rs:571-585` 的 `build_cookie_jar` 就是干这个的）。**不要给 reqwest 加 `blocking` 特性。**
- `tauri` crate 源码确证（本地 registry `…\tauri-2.12.0\src\webview\`）：`pub use cookie;`、`pub use tauri_runtime::Cookie;`、`webview/mod.rs:2423 cookies_for_url`、`:2461 set_cookie`、`:2474 delete_cookie`（`webview_window.rs:2715/2749/2758` 同名）。
- `current_bduss` 的签名是 `fn current_bduss<'a>(mut cookies: impl Iterator<Item = &'a tauri::webview::Cookie<'static>>) -> Option<&'a str>` —— **泛型 `impl Iterator` 形参必须写 `mut`**，否则方法体内 `.find()` 触发 `E0596: cannot borrow cookies as mutable`（本轮第一条门禁就是这么红的，`baidu_login.rs:524`）。
- 验活走**一次性 tokio current-thread runtime + reqwest 异步**，不碰 `tauri::async_runtime::*`（AGENTS.md 的 cargo test 红线）。
- 探针 URL 与 `remote/baidu_api.rs:345-350` 的 `PROBE_URL_TEMPLATE` 同口径：`GET /rest/2.0/xpan/file?method=list&limit=1`，**不带 `callback`**；findings §4-B 实测失效会话回 `errno=-6`。
- **不把 BDUSS 明文存进 `PendingSession` 静态槽**：只在轮询线程局部作用域参与比对与验活。
- **不新增 `BaiduLoginEvent.status` 取值**：避免 17 个语言包连带改动；`Rejected` 只写 `baidu-login.log` 并继续轮询。
- 放弃了「泛型 `call_cookie_op`」方案：`purge_stale_auth_cookies` 直接复用现成 `read_cookies` + 内联一次性删除线程（`tx.send(failed)` / `rx.recv_timeout(COOKIE_READ_BUDGET)`），改动面更小。

### 新增单测（`baidu_login.rs` 末尾 `#[cfg(test)] mod tests`，8 条）

`probe_errno_reads_positive_and_negative` / `probe_errno_is_a_byte_scan_not_a_json_parse`（含「字符串形态 `errno` 也解析」这一预期行为）/ `judge_accepts_only_errno_zero_as_live` / `judge_treats_ambiguous_evidence_as_inconclusive` / `judge_rejects_on_conclusive_evidence` / `judge_keeps_throttling_and_server_errors_inconclusive` / `current_bduss_takes_first_name_match` / `has_new_bduss_vetoes_same_value_only`。

⚠️ 测试断言必须**照真实实现写**：`judge_session_probe` 对「其它非 200」是**单一分支** ⇒ 403/429/500 的 reason 都是同一句「探针返回非 200，判不出会话状态」，不是 4xx/5xx 两条不同文案（本轮先按两段式写断言，读了实现才改齐）。
