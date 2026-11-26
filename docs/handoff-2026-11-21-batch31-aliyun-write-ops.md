# 批次 31 交接文档 · 阿里云盘写操作（删除 / 上传 / 新建目录 / 重命名 / 移动）

- 日期：2026-11-21
- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`（sigma-file-manager / Alpha File Manager，版本 3.16.0）
- 起点提交：`6e59909f`（批次 30：二维码 `img` 补非空 `alt`）
- 当前 HEAD：`8dc568d5`（批次 30 证据文档归档）
- 状态：**API 规格 100% 实测坐实 → Rust 实现完成、1576 测试全绿 → build1 打包成功 → build2（含修正后 CHANGELOG）在跑 → 沙箱 GUI 验收进行中**

> 本轮所有改动**尚未 git commit**。工作区改动清单见 §7。

---

## 1. 问题定性（重要，别再走偏）

「阿里云盘只读」**不是账号权限问题，也不是需要申请授权**——是代码里**有意实现的只读桩**：

- `src-tauri/src/remote/aliyun_provider.rs` 中 `write_file` / `write_file_with_progress` / `make_dir` /
  `remove_path` / `rename_path` / `set_permissions` / `get_permissions` 一律返回 `RemoteErrorKind::Unsupported`
  （文案形如「阿里云盘的删除在只读阶段还不支持。」）。
- 原测试（`aliyun_provider.rs` 约 1615-1658 行）**断言这些操作必须失败**，所以必须一并改掉。

⇒ 修法 = 用真实端点实现写操作，不是换账号 / 走授权流程。

## 2. 已实测坐实的 API 规格（web 分支，免密钥）

- host：`https://api.aliyundrive.com`
- 头：`Authorization: Bearer <access_token>`、`Content-Type: application/json`
- 无需 PDS OpenAPI 的 client_id / client_secret，无需开放平台注册应用。

| 用途 | 方法 | 路径 | body 关键字段 | 实测结果 |
|---|---|---|---|---|
| 取默认盘 | POST | `/v2/user/get` | `{}` | 200，取 `default_drive_id`（测试账号 = 15998870，**不是**保险箱 25998870） |
| 列目录 | POST | `/adrive/v3/file/list` | `drive_id, parent_file_id, limit, order_by, order_direction` | 200 |
| 新建目录 | POST | `/adrive/v2/file/create` | `drive_id, parent_file_id, type:"folder", name` | 201，响应含 `file_id` |
| 新建文件 | POST | `/adrive/v2/file/create` | `drive_id, parent_file_id, name, type:"file"` | 201，响应含 `part_info_list[0].upload_url` / `upload_id` / `revision_id` |
| 上传内容 | PUT | `part_info_list[0].upload_url`（OSS 预签名） | 裸 body | **必须不带 Content-Type** 才 200 |
| 上传收尾 | POST | `/adrive/v2/file/complete` | `drive_id, file_id, upload_id, revision_id` | 200（`status:"available"`、`content_hash` sha1、`crc64_hash`） |
| 重命名 | POST | `/adrive/v2/file/update` | `drive_id, file_id, name` | 200 且生效 |
| 删除 | POST | `/adrive/v2/file/delete` | `drive_id, file_id, type` | 204 |
| 移动 | POST | `/adrive/v2/file/move` | `drive_id, file_id, to_parent_file_id` | 200，且实测落位到目标目录 |

### 三条必须记住的「反直觉」规则

1. **OSS PUT 不能带 `Content-Type`**：带 `application/oct-stream` → HTTP 403 `SignatureDoesNotMatch`
   （签名覆盖了该头）；**不设该头** → HTTP 200。reqwest 用 `.body(Vec<u8>)` 天然不带 content-type，别手加。
2. **删除必须带 `type`**（`"file"` / `"folder"`）：缺 `type` → HTTP 400。
3. **重命名字段是 `name`**：用 `file_name` 会返回 200 但**静默不生效**。
   另：`file/create` 建文件的 body 必须极简，多传 `size` / `content_hash` / `check_name_mode` → HTTP 400。

### 移动的落位验证（已做）

`POST /adrive/v2/file/move` 返回 200 **不等于**已搬走（云盘有最终一致性延迟）。
判定要三项齐：`file/get` 的 `parent_file_id` == 目标目录 id、目标目录 list 能看到、源目录 list 已空。
实测：等待 3s 后三项全 PASS（`MOVE_LANDED=true`），文件移动成立；**文件夹移动未单独实测**。

### 已排除的路径（全部 404，别再试）

`/adrive/v1.0/file/createFolder`、`/adrive/v2/file/createFolder`、`/adrive/v1.0|2|3/file/rename`、
`/adrive/v2/file/upload`、`/adrive/v2/file/createFile`、`/adrive/v2/file/precreate`、
`/adrive/v2/file/get_upload_url`、`/v2/file/createFile`。

## 3. Rust 实现（已完成，未 commit）

### 3.1 `src-tauri/src/remote/aliyun_api.rs`（+601 行，66,952 → 89,771 B）

纯函数层，**无任何 `tauri::` 路径**（被 `cargo test --lib` 链接）。

- 端点常量：`ALIYUN_CREATE_URL` / `ALIYUN_COMPLETE_URL` / `ALIYUN_UPDATE_URL` /
  `ALIYUN_DELETE_URL` / `ALIYUN_MOVE_URL`、`ALIYUN_UPLOAD_CHUNK_BYTES = 256KB`
- 纯函数：`aliyun_create_folder_body` / `aliyun_create_file_body` / `aliyun_complete_upload_body` /
  `aliyun_rename_body` / `aliyun_delete_body` / `aliyun_move_body` / `aliyun_parse_create_payload` /
  `aliyun_parse_write_ack` / `aliyun_entry_type_name` / `aliyun_part_ranges` /
  `aliyun_map_write_failure` / `aliyun_failure_is_already_exist`

### 3.2 `src-tauri/src/remote/aliyun_provider.rs`（+1168 / -114 行，76,625 → 113,718 B）

- `AliyunApi` trait 扩 7 个写方法；`UnavailableAliyunApi` 同步扩展。
- `HttpAliyunApi` 落地 7 个实现（L960-1112）：`create_folder` L960、`create_file` L984、
  `upload_parts` L1004、`complete_upload` L1037、`rename_entry` L1057、`delete_entry` L1076、
  `move_entry` L1095。
- 新增：`build_upload_client` / `upload_one_part`(L767) / `aliyun_upload_chunk_stream` /
  `report_upload_progress`(L751) / `send_authorized_json` / `post_write_json_text`。
- `RemoteProvider` 的 `write_file`(L1693) / `write_file_with_progress`(L1703) / `make_dir`(L1764) /
  `remove_path`(L1818) / `rename_path`(L1872) 全部换成真实实现；
  `set_permissions`(L1789) / `get_permissions`(L1805) 保持 `Unsupported`。
- 删除 `reject_write`；新增公开 `move_path`（provider 层能力，见 §6「移动」的定性）。

### 3.3 上传链路的三个关键实现细节

1. `upload_one_part` 用**独立的无默认头客户端** `upload_client`——OSS 预签名 URL 自带签名，
   多带 `Authorization` 会 403。
2. **手工设置 `Content-Length`**（避免 chunked 传输）。
3. **刻意不设 `Content-Type`**（否则 403 `SignatureDoesNotMatch`）。
4. 流式 body 真实字节级进度；用 `futures_util::future::join` 而非 `tokio::select!`
   （生产 tokio 依赖未开 `macros` feature）。

## 4. 测试（已通过）

- 命令（**必须**固定 1.90.0 工具链 + 输出重定向到文件）：
  ```
  set "PATH=C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;C:\Users\Duanyi\.cargo\bin;%PATH%"
  cd src-tauri && cargo test --lib > ..\.superpowers\sdd\b31\cargo-test.log 2>&1
  ```
- 结果：**1576 passed / 0 failed / 1 ignored**，6.71s。
  日志：`.superpowers\sdd\b31\cargo-test.log`（141,072 B）。
- 17 条告警全部指向既有代码（`bench.rs`、`image_edit.rs`、`mcp/commands.rs`、`ftp_provider.rs`、
  `global_search/scan.rs`、`baidu_api.rs`、`quark_api.rs`），**无一指向 aliyun 文件**。
- 新增用例：`remote::aliyun_api::write_tests` 8 例 + `remote::aliyun_provider::tests` 9 例，
  含 `every_write_operation_now_reaches_the_server`（替换旧的
  `every_write_operation_reports_that_it_is_unsupported`）、`the_permission_operations_stay_unsupported`、
  `the_delete_carries_the_real_entry_type`、`the_rename_uses_the_entry_id`、
  `the_move_call_carries_the_target_parent_id`、`the_upload_reports_the_real_byte_count`、
  `a_failed_upload_never_reports_success`、
  `the_upload_request_has_a_content_length_and_no_content_type`
  （TcpListener 抓真实字节断言：有 content-length、无 content-type、无 chunked、无 authorization）、
  `the_error_text_never_leaks_the_secrets`。

## 5. 前端可达性（静态核查结论：前端零改动）

- `src/modules/remote/utils/remote-write.ts`：`createRemoteDirectory` → `makeRemoteDirectory` →
  `invoke('remote_make_dir')`；`renameRemoteEntry` → `invoke('remote_rename_path')`；
  删除走 `remote-delete.ts` → `invoke('remote_remove_path')`；
  上传走 `remote-transfer.ts` → `invoke('remote_upload_paths')`。
  全部走 provider 抽象 ⇒ **阿里云盘零前端改动即打通**。
- `canCreateFile`（新建空文件）对远程**永久隐藏**；但**「新建文件夹」无任何门禁** → 远程窗格可达。
- `src/modules/remote/utils/remote-context-menu-visibility.ts` 无阿里云盘只读判定；
  Rust 侧与前端都没有阻止阿里云盘写操作的协议/能力门禁。

## 6. 「移动」的定性（重要，避免被当成 bug）

**全应用所有网盘都没有「移动」功能**，这不是阿里云盘专属缺陷：

- `src-tauri/src/remote/commands.rs` 的 Tauri 命令注册表（L1277-1284）只有
  `remote_upload_paths` / `remote_download_paths` / `remote_chmod` / `remote_get_permissions` /
  `remote_remove_path` / `remote_make_dir` / `remote_rename_path`，**没有 `remote_move_path`**。
- 前端 grep `remoteMove|movePath|move_path|moveTo` 无远程移动实现；
  `remote-write.ts` 注释明写「跨目录移动是另一件事，不在这一轮」。
- provider 层已备好 `file/move` 能力备用（`move_path`），但**本轮不新增 Tauri 命令、不做前端移动功能**
  ——那是新功能，不是修 bug。

## 7. CHANGELOG 改写（已完成，未 commit）

### 决定：保持 3.16.0，不 bump 到 3.17.0

依据：`gh release list -R kizemo/alpha-file-manager` 显示最新 release 仍是 **v3.14.0**，
3.15.0 / 3.16.0 **从未公开发布** ⇒ 直接改 3.16.0 条目无历史包袱，且零额外构建成本。

### 改动范围（15 个文件）

根 `CHANGELOG.md` + `public/changelog/<locale>/changelog.md` × 14
（`ch de es fa fr hi it ja pt ru sl tr ur vi`）。

### 内容要点

- Summary 去掉「read-only on purpose / never writes」→ 改为
  「It is no longer read-only: you can create folders, upload files, delete them and rename them on the drive, just like with your other netdisks」
- `**Added**` 条目从「browses and reads, and writes nothing」改为
  「**Aliyun Drive can write to your drive as well** - ... Moving a file to another folder and sharing are still not offered - the app has neither one for any netdisk. The vault drive is recognised but not unlocked.」
- **新增一条 `**Fixed**`**：二维码 `alt` 修复（读屏软件现在会念出来）。
- 中文版（`ch`）已核对，语气与既有条目一致；其余 13 语言包按同规范改完。

### 门禁

- `node scripts/verify-changelog.mjs` → **EXIT=0**（应用版本 3.16.0，CHANGELOG 条目 60，顺序正确）。
- 自扫 15 个文件：`files_checked=15 bad=0`（无 BOM、无 U+FFFD、都有 `## [3.16.0]`）。
- 根文件 3.16.0 节内仅剩**故意保留**的 “It is no longer read-only”。

### 为什么必须重跑前端构建

`public/changelog/**` 是运行时 `fetch('/changelog/${locale}/changelog.md')` 指向的**打包内资源**
⇒ 改 changelog 必须重新打包才生效（这就是 build2 存在的原因）。

## 8. 构建（build1 已成功，build2 在跑）

### 8.1 构建环境的三个坑（已修，写在 `build-b31.cmd` 里）

1. **后台 stdout 捕获上限 ~64 KiB**：vite 资产清单实测 65,698 B 被腰斩
   ⇒ 所有 cargo / npm / 构建输出**必须重定向到文件**，只读 tail。
2. **`C:\Users\Duanyi\.cargo\bin` 没有 cargo.exe / rustc.exe shim**：只留它会 `cargo: program not found`。
3. **默认 `stable`(1.97.1) 的 sysroot 是坏的**：`only metadata stub found for rlib dependency rustc_std_workspace_core`
   + `error[E0463]: can't find crate for std` ⇒ **必须固定 `1.90.0-x86_64-pc-windows-msvc`**，
   并把该 toolchain 的 `bin` 显式前置到 PATH。

```cmd
REM .superpowers\sdd\b31\build-b31.cmd 关键行（已验证）
set "RUSTUP_TOOLCHAIN=1.90.0"
set "PATH=C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;C:\Users\Duanyi\.cargo\bin;%PATH%"
call npm run tauri:build >> "%LOG%" 2>&1
```

运行方式（stdout 只有一行 `BUILD_EXIT=0`）：

```
cmd.exe /c "F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\b31\build-b31.cmd"
```

### 8.2 build1 结果（成功）

- `BUILD_EXIT=0`（2026-11-21 00:44:19）
- `src-tauri\target\release\alpha-file-manager.exe` = 85,118,976 B（00:44:16）
- 安装包 `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.16.0_x64-setup.exe`
  与 `release\local-builds\Alpha File Manager_3.16.0_x64-setup.exe` = 45,874,624 B（00:44:15）

注：`release\` 顶层那份 3.16.0（2026-11-20 20:35）是**已发布资产镜像**，内容不同，
所以 publish 脚本把新包落到 `release\local-builds\`。

### 8.3 build2（最终包，含修正后的 CHANGELOG）—— 已完成

- 后台任务：`bg_2509f9a2-9add-4119-a5a1-d0334c653af4`
- 日志：`.superpowers\sdd\b31\build-b31.log`
- 结果：**`BUILD_EXIT=0`（2026-11-21 00:53:37）**
- `src-tauri\target\release\alpha-file-manager.exe` = 85,118,976 B（00:53:32）
  sha256 = `EAD83BE4AD86A56F1B7D724169A6BC99D393FEE3A4EF426966A7468E801DCACA`
- 安装包 `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.16.0_x64-setup.exe`
  与 `release\local-builds\Alpha File Manager_3.16.0_x64-setup.exe` = **45,888,651 B**（00:53:31）
  sha256 = `7B082D8476A65070500E5B842B9319C1B643097C3DC24FE23F43A9E20A4DFE20`
- 体积对比 build1（45,874,624 B）：**+14,027 B**，正是 CHANGELOG 文本增量。

### 8.4 「CHANGELOG 与写操作确实进了包」的判定（归一化链式取证）

> 判据不能是「exe 里能搜到某句文案」——Tauri v2 把**文本类资产用 brotli 压缩**后内嵌，
> 明文搜不到（实测搜 `它已经不再只读` / `It is no longer read-only` 全部 False）。
> 也不该是「装机副本 sha == 构建产物 sha」——NSIS 打包器会改写 29 字节的 bundle-type 标记，必然不等。

本轮用的四段式判据（全 PASS）：

1. **构建输入正确**：`dist/changelog/ch/changelog.md` 的 sha256 == `public/changelog/ch/changelog.md`（未压缩改写源）
   （ch/de/fr/ja 四语言逐一 `match=True`），且 dist 内正文已含新文案
   （「它已经不再只读：建目录、上传、删除、改名，都能在网盘上直接做。」）。
2. **资源键在 exe 内**：`/changelog/ch/changelog.md` 在 exe 中命中（说明这份 dist 被编进了二进制）。
3. **包内 exe 与构建产物同一份**：7-Zip 从 NSIS 里抽出 `alpha-file-manager.exe`（mtime 00:52:58），
   与 `target\release` 的那份**同尺寸**（85,118,976 B）；把 `__TAURI_BUNDLE_TYPE_VAR_[A-Z0-9]+__`
   归一化后按 64 KiB 分块比 sha ⇒ **`normalized_diff_chunks = 0`**，
   唯一差异是打包标记 `NSS`（包内）↔ `UNK`（裸产物）；两侧 VersionInfo 均为 3.16.0。
4. **Rust 写操作确实被编译**：build2 日志里出现 `aliyun_provider.rs:51`（新增常量导入）与
   `aliyun_api.rs:1005`（`aliyun_parse_write_ack`）的告警锚点 ⇒ 修改后的 aliyun 源文件参与了本次编译。

⚠️ 残留说明：`release\` 顶层那份 3.16.0（2026-11-20 20:35）是**已发布资产镜像**，
与本轮产物内容不同 ⇒ 本轮包只落在 `release\local-builds\`，未覆盖已发布资产。

## 9. 沙箱验收（进行中）

### 9.1 规则（AGENTS.md §2.8，硬约束）

- 必须跑在 `CreateDesktop` 隔离桌面（`DESKTOP_ALL` = 0x1FF）；注入端**不切换桌面**，用 `EnumDesktopWindows`。
- 沙箱内只能做**鼠标等价**断言：PostMessage 投 `Chrome_RenderWidgetHostHWND` + UIA + `PrintWindow`。
- **不得假装验过键盘**：上传依赖系统文件选择框，属键盘路径 ⇒ 沙箱内不能声称上传已验。
- 验收前必须先结束运行中的 `alpha-file-manager`（release 单实例锁）。
- 脚本用 `powershell.exe` 5.1、**纯 ASCII**；`STARTUPINFO.lpDesktop` 需 Unicode marshal
  （否则 `0xC0000142` 秒退）；中文匹配用 `[char]` 码点拼，不写中文面量。

### 9.2 已定的交互路径（纯左键，不依赖右键）

- 全局 `contextmenu` 被 preventDefault（原生菜单禁，但 DOM 事件仍触发）⇒ **不依赖右键**。
- 工具栏「新建」下拉：`src/modules/navigator/components/file-browser/file-browser-toolbar-create-button.vue`。
- 行「更多」菜单：`src/modules/navigator/components/file-browser/file-browser-actions-menu.vue`（含 delete 项）。
- i18n 文案：`src/localization/messages/ch.json`。

### 9.3 可复用资产

`.superpowers\sdd\b30\b30-sandbox-qr.ps1`、`.superpowers\sdd\b30-accept-install.ps1`、
`.superpowers\sdd\b30\b30-uia.ps1`、`.superpowers\sdd\b30\b30-save.ps1`、
`.superpowers\sdd\b30\b30-embedded.ps1`、`.superpowers\sdd\b30\b30-uia-dump.txt`

### 9.4 沙箱验收能验什么 / 不能验什么

| 操作 | 沙箱可验？ | 说明 |
|---|---|---|
| 新建文件夹 | ✅ 鼠标等价 | 工具栏「新建」下拉 + 输入 + 确认 |
| 删除 | ✅ 鼠标等价 | 行「更多 → 删除 → 确认」 |
| 重命名 | ⚠️ 可尝试 | 「更多 → 重命名」；若驱动不稳就记「未判定 + 原因」 |
| 上传 | ❌ **不可验** | 依赖系统文件选择框 = 键盘路径，沙箱内**不得声称已验** |
| 移动 | ❌ 无 UI | 全应用所有网盘都没有移动功能（§6） |

## 10. 已验证 / 推断 / 未验证（严格区分）

### 已验证（实测，有证据）
- 阿里云盘全部写端点的**真实 HTTP 形状**（含三条反直觉规则）。
- 端到端链路：create(201) → OSS PUT(无 Content-Type, 200) → complete(200) → file/get(200, size=41,
  status=available) → get_download_url(200) → 下载内容与上传一致（match=True）。
- move 落位三项齐（`MOVE_LANDED=true`）。
- `cargo test --lib` 1576 passed / 0 failed / 1 ignored。
- build1 `BUILD_EXIT=0`，exe 与安装包已产出。
- CHANGELOG 门禁 EXIT=0；15 文件自扫 bad=0。
- 前端调用链静态可达（§5）。

### 推断（由实测事实推得，未单独跑）
- 沙箱内「新建文件夹」「删除」应当 PASS——路径已核实为鼠标等价，但**本轮实跑结果以 §11 为准**。
- 大文件多 part 上传行为（实测 create 只返 1 个 part）。
- 上传进度语义（字节级进度已实现，未在沙箱取证）。

### 未验证（明确留白）
- **GUI 端到端写操作（沙箱）**——见 §11。
- 文件夹移动落位（只验了文件移动）。
- 删除非空目录。
- 写操作**经由 GUI 对真实网盘**发请求——**已于 run1 坐实「新建文件夹」全链路**（见 §11.1）；
  删除 / 改名的 GUI 路径仍未取证，判为「未判定」（无正反证据），见 §11.4；
  上传属键盘路径，沙箱范围外，判为「未验证」。

## 11. 沙箱验收实测结果

三轮实跑（2026-11-21，被测二进制 = build2 产物 3.16.0 / 85,118,976 B / sha16 `ead83be4ad86a56f`），
判定行见 `.superpowers\sdd\b31\b31-accept-write-ops.txt` / `-run.log` / `.log` / `.report.json`，
完整报告见 `.superpowers\sdd\b31\b31-accept-write-ops.md`。

### 11.1 run1（01:22:14）—— GUI 新建目录全链路 PASS

当时 access_token 仍有效（`expires_at_unix=1795195663`＝01:27:43），远程读取是真的。

| 判据 | 结果 | 证据 |
|---|---|---|
| NAVIGATED_TO_REMOTE / AT_REMOTE_CONNECTION_ROOT | true | 面包屑里连接名之后无路径段 |
| REMOTE_LIST_LOADED | true | 远程根列表 106 项 |
| CREATE_DROPDOWN_OPENED | true | 工具栏「新建」下拉打开 |
| CREATE_NAME_FIELD | true | `aid=new-item-input`，rect 1642,1049,2096,1110 |
| CREATE_NAME_ENTERED | true | `via=uia_valuepattern_setvalue`——**程序化注入，键盘路径未被取证** |
| CREATE_CONFIRM / CREATE_REMOTE_LIST_SHOWS_ENTRY / CREATE_DONE | true | 新目录 `afm-b31-20261121-012243` 出现在远程列表且此前不存在 |
| RENAME | false | **探针缺陷**：改名入口没走到 |
| DELETE_MENU_OPENED | false | **探针缺陷**：点到右侧详情面板标题，不是列表行 |
| API_CROSSCHECK | false | **探针缺陷**：当轮取不到 token |
| CLEANUP_REMAINING_ZERO | false | 计数重复（3 vs 实际 1） |

⇒ **GUI → invoke → provider → HTTP → 真实阿里云盘根目录** 这一条链在「新建文件夹」上被坐实。
该目录确实存在于网盘：独立 API 清理脚本 `b31-api-cleanup.ps1` 找到且仅找到 1 个 `afm-b31-*`，
删除后复查 `remaining=0`（`CLEANUP_OK`）。

### 11.2 run2（01:28）/ run3（01:30）—— 会话过期，非产品缺陷

会话在 01:27:43 过期（`expires_at_unix=1795195663` vs `now=1795195884`，差 -221 秒）：

- 探针自己的 API 交叉校验在 `POST /adrive/v3/file/list` 与 v2 回退上均 **401 Unauthorized**；
- 凭据条目里 `client_id` 是**空串**，应用自己也无法续期；
- `POST https://api.aliyundrive.com/token/refresh` 三种请求体
  （`{client_id,refresh_token}` / 带 `grant_type` / 去掉 `client_id`，均用 web client_id
  `25dzX3vbYqktVxyX`）**一律 400** ⇒ 这份 refresh_token 已不可用；
- run3 点连接卡片落到了**「设备」页**（面包屑 `设备 / 1`），工具栏因此没有「新建」可找。

| 判据 | run2 | run3 |
|---|---|---|
| NAVIGATED_TO_REMOTE | true | true |
| AT_REMOTE_CONNECTION_ROOT | false | false |
| REMOTE_LIST_LOADED | true（21 行） | true（9 行） |
| LIST_IS_REMOTE_ROOT | false（401） | false（401） |
| CREATE_DROPDOWN_OPENED / CREATE_DONE | false | false |
| CLEANUP_REMAINING_ZERO | true | true |

这两轮**不能作为产品结论**：一是对着失效会话跑的，二是 run3 看错了窗格。

### 11.3 API 层真机证据（会话有效期内实测，Rust 层发的就是这套形状）

| 操作 | 端点 | 结果 |
|---|---|---|
| 建目录 | `POST /adrive/v2/file/create` | 201 |
| 建文件 + 取上传地址 | `POST /adrive/v2/file/create` | 201，`part_info_list[0].upload_url` |
| 上传字节 | `PUT <预签名>` | 200，**必须不带 `Content-Type`**（带 `application/oct-stream` → 403 `SignatureDoesNotMatch`） |
| 完成上传 | `POST /adrive/v2/file/complete` | 200，`status: available` |
| 改名 | `POST /adrive/v2/file/update` `{name}` | 200，复查生效 |
| 删除 | `POST /adrive/v2/file/delete` `{type}` | 204（`type` 必填） |
| 移动 | `POST /adrive/v2/file/move` `{to_parent_file_id}` | 200，落位复查 PASS |

### 11.4 结论

| 操作 | API 层 | GUI 层（沙箱） |
|---|---|---|
| 新建目录 | 真机 PASS | **真机 PASS（run1）** |
| 上传文件 | 真机 PASS（PUT 200 + complete 200） | **未验证**——系统文件选择框属键盘路径，沙箱范围外 |
| 重命名 | 真机 PASS（`file/update`） | **未判定**——探针没走到改名入口，无任何正反证据 |
| 删除 | 真机 PASS（`file/delete` 204） | **未判定**——探针没打开行右键菜单，无任何正反证据 |
| 移动 | 真机 PASS（落位复查过） | **范围外**——全应用所有网盘都没有移动功能 |

### 11.5 补齐剩下两个 GUI 判据的步骤

1. 在应用里**重新授权**阿里云盘连接：当前会话无法续期（凭据 `client_id` 为空 + 刷新端点 400）。
2. 重跑 `b31-accept-write-ops.ps1`：探针已带行右键回退与「永久删除」/改名条目子串匹配，
   缺的只是有效会话。

## 12. 证据文件（均在 `.superpowers\sdd\b31\`，该目录已被 git 忽略）

| 文件 | 内容 |
|---|---|
| `b31-aliyun-write-probe.ps1` / `aliyun-write-probe.txt` | 首轮写端点探测 |
| `b31-aliyun-path-sweep*.ps1` / `aliyun-path-sweep*.txt` | 路径扫描（含 404 清单） |
| `aliyun-path-sweep6.txt` | **关键**：OSS PUT 的 Content-Type 变体对照 + delete 的 type 必填 + 清理残留=0 |
| `b31-aliyun-move-verify.ps1` / `b31-aliyun-move-verify.txt` | move 落位三项判定（`MOVE_LANDED=true`） |
| `aliyun-write-spec.md` | API 规格实测报告（49,921 B） |
| `b31-aliyun-write-impl.md` | Rust 实现报告（端点表 / 错误映射 / 测试输出尾部 / 存疑项） |
| `cargo-test.log` | `cargo test --lib` 完整输出（1576 passed） |
| `build-b31.cmd` / `build-b31.log` | 打包脚本与完整构建日志 |
| `b31-accept-write-ops.ps1` / `.md` / `.txt` / `-run.log` | 沙箱验收脚本与报告（进行中） |

探针纪律：只建/删 `afm-b31-*` 自建产物，结束前清零（实测 `remaining probe items in root = 0`）；
只输出 token 长度与 sha16 前缀，不落盘 token 明文。

## 13. 实现约束（踩过的坑，别回退）

- `src-tauri/src/remote/aliyun_api.rs` 是**纯函数层**（被 `cargo test --lib` 链接），**不得出现任何 `tauri::` 路径**；
  HTTP 层在 `aliyun_provider.rs`。
- **不新增 i18n key**：前端 17 个语言包要同步，`aliyun-i18n-keys.test.ts` 会卡键不一致。
- 用户可见文案**绝不能**包含 token / code / refresh_token / 服务端原始 message。
- `set_permissions` / `get_permissions`：阿里云盘无 POSIX 权限位，保持 `Unsupported` 并注释说明。
- 阿里云盘会话只有一处落盘：`%APPDATA%\com.sigma-file-manager.app\remote-credentials.json`
  （DPAPI / CurrentUser / base64）；连接清单在 `...\user-data\user-remote-connections.json`
  （**不在 appdata 根目录**）。PS 5.1 解密需先 `Add-Type -AssemblyName System.Security`，否则 `TypeNotFound`。
- `npm run tauri:build` = sync-version → verify:sandbox → tauri build → 复制到 `release/`（**不上传**）。
- git commit 在 PowerShell 下必须用 `-F <msgfile>`，`-m "..."` 的引号会炸。
- `~/.cargo/.global-cache` 是损坏的 sqlite（`file is not a database`），只产生 GC 警告，**不要删**。

## 14. 待办

- [x] Rust 写操作实现落地 + `cargo test --lib` 全绿
- [x] CHANGELOG 15 文件改写 + 门禁通过
- [x] build1 打包成功（`BUILD_EXIT=0`）
- [x] build2 完成（最终包，含修正后 CHANGELOG）
- [x] 沙箱验收实跑并回填 §11（run1 新建目录真机 PASS；删除/改名 = 未判定）
- [x] 更新本文档 §11（验收结果）
- [ ] git commit（本轮改动；commit 前需用户确认）
