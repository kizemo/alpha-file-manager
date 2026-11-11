# 交接卡 · 夸克网盘接入（Alpha File Manager）

- 日期：2026-11-11
- 方案：`docs/plan-2026-11-11-quark-pan.md`（1440 行）
- 评审：`docs/review-2026-11-11-quark-pan.md`（451 行，结论「需修改后放行」）
- 状态：**T0–T7 已实现并过门禁；T8 真机实拍 14 条全部完成 —— 12 条 PASS、2 条 FAIL（第 9 / 第 11 条）；
  真实用户数据已还原、沙箱与 cookie 明文已清理；3 条 commit 已提交（未 push）**
  —— T8 逐条结论见 §12，缺陷细节见 §11 与 §12.3

---

## 1. 一句话背景

在「添加网盘」处新增**夸克网盘**，行为对齐现有百度网盘：点添加 → 弹网页 → 登录即完成配置。
本轮做到 **T7（前端 + 17 语言包）**；**T8 真机验收**是唯一的剩余关口。

---

## 2. T0 实测定案（最关键的一节，勿重跑）

T0 的三态结论 = **主路线成立（MAIN_ROUTE_HOLDS）**：不需要 WebView2 内 fetch 退路。
所有「未实证」的猜测都被实测钉死，下面的数字**都来自实测**，不是抄方案。

### 2.1 端点与参数

| 用途 | 端点 | 方法 | 关键参数 |
|---|---|---|---|
| 列目录 | `https://drive-pc.quark.cn/1/clouddrive/file/sort` | GET | `pdir_fid` / `_page` / `_size` / `_fetch_total=1` / `_fetch_sub_dirs=0` / `_sort=file_name:asc,updated_at:desc` |
| 取直链 | `https://drive-pc.quark.cn/1/clouddrive/file/download` | **POST** | 基础参数在 query，`fid` 在 body `{"fids":[fid]}` |
| 账号信息 | `https://pan.quark.cn/account/info?fr=pc&platform=pc` | GET | `code` 是**字符串** `"OK"` |

基础参数集（缺了就 401）：
`pr=ucpro` / `fr=pc` / `sys=win32` / `ve=1.0.0.0` / `uc_param_str=dsdnfrpfbivesscpgimibtbmnijblauputogpintnwktprchmt` / `client_id=533`

### 2.2 八条踩坑级事实

1. **`fr` 是成功与失败的分水岭**：`fr=pc` → 200/code=0；`fr=win` → 401/code=31001 `require login [guest]`。
2. **下载端点只收 POST**。方案 §3.2.5 / §4.6 写的 `GET .../download?fid={fid}` 是**错的** —— 实测三个 GET 变体**一律 HTTP 405**。
3. **下载的 base 参数必须在 query**。实测 6 个 POST 变体里，只有「query 带基础参数 + body 带 `{"fids":[fid]}`」返回 200/code=0，其余 5 个（裸路径 / 只带 `?fid=` / 无 body / `{"fid":...}` / 带 `pdir_fid`）**一律 401**。
4. **`metadata` 在顶层**，与 `code` / `status` 同级，不在 `data` 里。
5. **`name_space` 是数字**、`format_type` 是 **MIME 字符串**（如 `"image/jpeg"`），都不是方案里写的形态。
6. **`updated_at` / `created_at` 是毫秒**（13 位），不是秒。
7. **`ut` / `device_model` 都非必需**：取常量、取主机名、甚至整个省略，列表与下载端点都是 200/code=0。
   ⇒ 产品代码已改成**不发主机名**（`device_model` 固定 `windows-pc`），满足 §6.4「主机名不落盘、不进日志」。
8. **`download_url` 里已内嵌 `auth_key`**（顶层**没有**独立的 `auth_key` 字段，也没有顶层 `expires`）。

### 2.3 真实响应形状（根目录 list=5，其中 2 个目录）

```
root keys : status, code, message, timestamp, data, metadata
data keys : last_view_list, recent_file_list, list
list[0]   : 48 个字段（fid / file_name / pdir_fid / size / file_type / dir /
             format_type / name_space / download_url / md5 / thumbnail /
             created_at / updated_at / …）
file_type  : 1 = 文件，0 = 目录；dir 另有布尔字段
```

### 2.4 账号现状（验收用）

- 根目录 list=5（2 个目录）；子目录 `727be478bf9a4280a64f8fff3bbd9961` list=20（全为文件）。
- 根目录首个文件 `size=47654277`，`fid` 长度 32。

---

## 3. 已实现的模块

| 任务 | 文件 | 规模 / 测试 |
|---|---|---|
| T1 API 纯函数层 | `src-tauri/src/remote/quark_api.rs` | 2289 行 / 112 test（后随实测扩到 117） |
| T2 会话层 | `src-tauri/src/remote/quark_auth.rs` | 1219 行 / 53 test |
| T3 只读 provider | `src-tauri/src/remote/quark_provider.rs` | 2263 行 / 57 tokio::test |
| T4 真实登录窗 | `src-tauri/src/quark_login.rs` | 35857 B / 9 test |
| T5 命令接线 + 协议注册 | `remote/commands.rs` `remote/registry.rs` `remote/connection.rs` `remote/mod.rs` | — |
| T6 前端 provider 选择器 | `src/modules/settings/ui/categories/remote/*` 等 10 处 | 5 个新测试文件 |
| T7 i18n | `src/localization/messages/*.json` × 17 | 21 键/语言 |

### 3.1 接线要点

- 协议 id：`quark_pan`；协议下拉**不动**，网盘区用 **provider 选择器**（百度网盘 / 夸克网盘二选一）。
- `netdiskProtocol` prop 默认 `'baidu_pan'`；表单协议判定优先级
  `connection?.protocol ?? (variant==='netdisk' ? (netdiskProtocol ?? 'baidu_pan') : (initialProtocol ?? 'ftp'))`。
- 事件频道**分开注册、分开清理**：`afm://baidu-login` 与 `afm://quark-login` 各有独立
  `onMounted` 监听器与 `onUnmounted` 清理。
- 夸克未授权保存：store 把 `remote_quark_login_claim` 失败归一化为 `throw new Error('quarkLoginRequired')`。
- provider 的 `stat` 语义：trait 无 `stat` ⇒ 夸克走「列父目录再按名字精确匹配」。
- 夸克条目**没有路径**，服务端只给 `fid` ⇒ 路径下钻是「逐级列目录 + 按名字匹配」。
- `remote/mod.rs` 对与百度侧同名的裸名统一加 `quark_` 前缀别名导出（规避 E0252）。

---

## 4. 门禁现状（两个都绿）

| 门禁 | 结果 | 日志 |
|---|---|---|
| `cargo test --lib`（必须在 `src-tauri/` 跑） | 1417 passed / 0 failed / 1 ignored | `.superpowers/sdd/quark/cargo-t8c.log` |
| `npm run check`（= ts + lint:check + test:unit:run + verify-changelog + verify:sandbox） | exit 0 / 305 test files / 3198 tests | `.superpowers/sdd/quark/check-full-5.log` |

---

## 5. T8 真机验收（**已完成，逐条结论见 §12**）

### 5.1 实际执行方式（与原计划不同，务必知道）

1. `npm run tauri:build`（**慢，早启动、丢后台**；产物落 `release/`）→ 静默装机 3.14.0。
2. 备份用户数据 —— **已做**：`COPIED=28 / BAD=0 / BACKUP_VERDICT=OK`，
   备份在 `.superpowers/sdd/quark/t8-user-data/`。
3. **建立沙箱**（本轮新增，替代「直接动真实数据」）：真实 appdata 目录 park 到
   `.superpowers/sdd/quark/t8-sandbox/real-appdata-parked`，原路径换成 **junction** 指向副本
   `.superpowers/sdd/quark/t8-sandbox/appdata`；DPAPI 密文绑定 Windows 用户而非路径 ⇒ 副本仍可解密。
   所有验收动作（含第 12 条「会话失效」）都在副本里做，真实账号与真实凭据零污染。
4. 逐条实拍 §6.3 的 14 条 ⇒ 结论见 §12。
5. 验收后核对：`user-remote-connections.json` **无** cookie 明文（`PLAINTEXT_LEAK=False`）；
   `remote-credentials.json` **无**夸克 cookie 明文（只有 DPAPI 密文，2893 B = 原值未被覆盖）。
6. 还原：`pwsh -File .superpowers/sdd/quark/t8-sandbox.ps1 -Action Restore`
   —— **必须先杀进程再 restore**（脚本会先 `Stop-Process`）；已核对 `STATE=real`、
   user-data 28 文件 / 4041879 B、同尺寸文件 SHA256 **零差异**。

### 5.2 §6.3 的 14 条清单

1. 设置 → 远程 → 网盘 → 点「添加」→ provider 选择器出现「百度网盘 / 夸克网盘」二选一
2. 选「夸克网盘」→ host/port/密码框消失，出现「添加夸克网盘」授权按钮
3. 点添加 → 内嵌窗口正常显示（不是白板）→ 扫码登录 → 关窗后卡片显示账号昵称
4. 二次点「添加」⇒ 弹窗正常，**不**误判为已登录
5. 保存 → 侧边栏「网盘」分区出现该连接
6. 双击 → 列出根目录，条目名 / 大小 / 时间正确
7. 进两层子目录 → 地址栏与面包屑正确
8. 右键一个文件 → 下载到本地 → 文件落地且 sha256 与原文件一致
9. 复制一个远端文件 → 粘贴到本地目录 → 落地一致
10. 右键菜单里**没有**「权限」类项
11. 断网 → 提示是**分类文案**（如「网络不可达」），不是「未知错误」
12. 让会话失效 → 提示「请重新登录」，**不是**静默失败
13. 夸克连接与百度连接同时存在，两条链路互不影响
14. 重启应用 → 夸克连接仍在且仍可浏览（凭据持久化）

### 5.3 T8 的已知风险点

- 第 3 条需要**用户扫码** —— 自动化到不了那一步，得请用户配合。
- 第 8 条「sha256 与原文件一致」需要一个本地原件做基准。夸克侧响应里有 **`md5`** 字段，
  可用它当交叉校验；或用 `.superpowers/sdd/quark/t0-download*.cjs` 取直链后自行下载一份做基准。
  ⚠️ `t0-download-post.cjs` 里的直链回源探测 **20 秒超时被 abort**（47 MB 文件），
  基准下载要么延长超时，要么挑子目录里的小文件。
- 直链 host 是 `dl-pc-sz.drive.quark.cn`；CDN 下载可能很慢，注意 `QUARK_FILE_TIMEOUT`。

---

## 6. 本轮修掉的真 bug

1. **夸克登录块不可达**：夸克内嵌登录块原先挂在百度块的 `v-else-if` 上，而百度块条件是
   `v-if="isNetdisk"` ⇒ 夸克连接**永远命中百度那一支**。已改为 `v-if="isNetdisk && !isQuarkNetdisk"`。
2. **下载 URL 少一个 `?`**：新写的 `quark_download_url()` 拼出 `.../download&pr=...`，
   被新单测 `the_download_url_carries_the_base_parameters` 当场抓住。已修。
3. **前端测试 mock 抓错监听器**：`baidu-connection-form.test.ts` 的 listen mock 改为**按事件名分别记回调**
   （两个频道并存后「抓最后注册的监听器」会抓错对象）。

---

## 7. 工具坑（本轮踩到的，别再踩）

| 坑 | 结论 |
|---|---|
| **`cargo fmt` 绝不能跑** | 本仓库**本来就不是 rustfmt-clean**（`cargo fmt --check` 报 300+ 文件差异，含 `app_updater.rs` / `batch_rename/*` / `remote/transfer.rs` 等无关文件）。跑它会污染全仓。`npm run check` 也不含 rustfmt 门禁。 |
| `cargo test --lib` 必须在 `src-tauri/` 跑 | 仓库根目录没有 `Cargo.toml`，在根目录跑会 exit 101。 |
| 同一 target 目录只能有一个 cargo 进程 | 并行会 `os error 32` 互抢 `.rlib`。`cargo` 与 `npm run check` 也不要并发（会假红）。 |
| cargo 不在 PATH | 走 `.superpowers/sdd/quark/cargo-test-lib.cmd`（先 `vcvars64.bat >nul 2>&1`，再挂 1.90.0 工具链）。 |
| PowerShell `Copy-Item -LiteralPath 'dir\*'` 不展开通配符 | 必须用 `-Path`（`-LiteralPath` 把 `*` 当字面量 ⇒ 静默复制 0 个文件、`BAD=28`）。 |
| 批量改 Rust 不要用 PowerShell 文本管道 | 用 node 精确匹配脚本（`.superpowers/sdd/quark/patch-download.cjs` + `patch-download.blocks`），逐条断言「命中且唯一」才写盘。 |
| `.cjs` 注释必须 `//` | Node v26 拒绝 `#` 注释。 |
| 一次 `edit` 调用只能带一组 `old_string`/`new_string` | JSON 里重复键会被静默丢弃；改完必须 `grep` 逐处核对落盘。 |

---

## 8. 证据文件（全部在 `.superpowers/sdd/quark/`）

| 文件 | 内容 |
|---|---|
| `t0-record.md` | T0 完整记录（含三态结论与「cookie 待销毁」标注） |
| `t0-subdir.md` | 子目录二次列举 ⇒ `VERDICT=MAIN_ROUTE_HOLDS` |
| `t0-provider-shape.md` | provider 精确请求形状 ⇒ `PROVIDER_SHAPE_OK` |
| `t0-sample-response.json` | 真实响应结构样本 |
| `t0-download.md` | GET 三个变体全 405 |
| `t0-download-post.md` | POST 六个变体 ⇒ `DOWNLOAD_ENDPOINT_OK`（P1 命中） |
| `t0-device-model.md` | `device_model` 常量/省略都通 ⇒ `CONSTANT_DEVICE_MODEL_OK` |
| `cargo-t8c.log` / `check-full-5.log` | 两套门禁日志 |
| `t8-user-data/` | 验收前用户数据备份（28 文件） |
| `commit-msg-rust.txt` / `commit-msg-frontend.txt` | 两条提交信息草稿 |

> ⚠️ `.superpowers/` 是 gitignore 目录 ⇒ 这些证据**不进仓库**，所以本交接卡才把关键数字**抄进正文**。

---

## 9. 未决项与风险

| # | 事项 | 状态 |
|---|---|---|
| R1 | T8 真机 14 条 | **已完成**：12 PASS / 2 FAIL（第 9、第 11 条）；逐条结论见 §12 |
| R2 | T0 凭据销毁 | **已销毁**：`t0-cookie.txt`（含 `__pus` 明文）已删除；清理脚本 `t8-cleanup.ps1`；全库明文扫描仅命中一处源码变量名 |
| R8 | **真实应用里的下载仍会失败** | 登录时抓到的 cookie 罐**缺 `__puus`** ⇒ 直链回源 403/412。沙箱内补齐后第 8 条 PASS，但**真实 vault（2893 B）未补** ⇒ 用户在正式应用里下载仍失败，需重登或修 `quark_auth.rs` 的 cookie 抓取。见 §12.4 |
| R3 | 提交 | **已提交 3 条（未 push）**：`99c73632` Rust / `02897006` 前端+i18n / `ee0a23ea` 交接卡；版本 bump + changelog 作第 4 条。push 与 GitHub Release 仍未授权 |
| R4 | T7 译文质量 | 17 份为自撰人工翻译，**未做母语复核** |
| R5 | `Ctrl+Shift+P` 本机不可达 | `Win32Error=1409`（OS 级 `RegisterHotKey` 占用）—— 冲突徽标真机复验须换机器，**不得伪造 PASS** |
| R6 | `p5b-install.ps1` 哈希判据仍是错的 | **已修（2026-11-11）**：判据换成「大小一致 + 打包标记归一化后逐 64KB 块比对 + VersionInfo 交叉校验」，裸 sha256 降级为信息打印；`$Version` 改从 `package.json` 推导；安装参数加 `/FORCE`；新增 `-VerifyOnly`。阳/阴性对照均通过，见 §17.9 |
| R7 | 子代理 Token Plan 上限 | 已撞过一次 ⇒ 继续自己做主线，别盲目开大 worker |

---

## 10. 铁律提醒（来自 `AGENTS.md`）

- 工作目录铁律：文件只落在 `sigma-file-manager` 内，外层目录不写新文件。
- 面向用户输出一律简体中文；代码标识符 / 路径 / 命令保持英文。
- 真机验收只用 `npm run tauri:build` 打包产物；`tauri dev` 的界面结论一律作废。
- 需要装机实测就直接打包 exe，别等用户开口。
- 浏览器调试用 **Edge**，不用 Chrome。
- 交接任务：handoff 文档落盘进项目 + **一句话接力 prompt 直接写在回答正文里**（不另建文件）。
- 遇到明显 bug 直接按推荐方案修，不要发问卷；只有「删/覆盖用户数据、付费或对外发布、
  大幅改架构方向」才必须先问。

---

## 11. T8 真机实拍第一轮结果与两个缺陷（2026-11-11 追加）

装机版：3.14.0 · 屏幕 3840x2160（150% 缩放，逻辑 2560x1440）。
证据目录 `.superpowers/sdd/quark/t8-shots/`（01–27），逐条记录 `.superpowers/sdd/quark/t8-acceptance-log.md`。

### 11.1 已判定（4 条 PASS）

- **1 PASS**：provider 恰为「百度网盘 / 夸克网盘」二选一。
- **2 PASS**：选夸克后 host/port/密码框消失，出现网盘授权卡（仅名称 + 登录 + 初始/本地目录 + 描述）。
- **3 PASS**：登录窗非白板 —— 有手机号/扫码 tab、可用二维码、手机号+验证码+立即登录、底部协议勾选；
  后端日志同轮出现「拿到会话并验活通过」（`session-1794360038386`）。
- **4 PASS**：每轮登录新建独立 cookie 罐 + `purge` 检查；超时**没有**被误判成成功
  （保存被拒 + `user-remote-connections.json` 无 quark_pan + 无会话文件）。
- **5–14 未做**：会话只活在后端内存 pending slot，**点「保存」时才落盘**；后续每次点登录
  都会新建空罐并 sweep 掉成功那次 ⇒ 必须先由真人扫码完成一次干净登录。

按钮实际文案是「用内嵌窗口登录」（不是「添加夸克网盘」），同一入口，验收记录已注明措辞差异。

### 11.2 缺陷 A（高，已修）：登录窗被主窗口压在下面

- **现象**：主窗最大化时点登录，屏幕上找不到登录窗；把主窗最小化才拍到内容。
- **后果链**：成功那次（gen1）恰好是主窗最小化时做的；gen2/gen3 全部停在
  `+2s 登录窗尚未 Ready，继续等`，gen3 约 902 s 后 `give up: timeout`。
  ⇒ WebView2 在完全遮挡下不渲染，登录必然超时 —— **不是网络或夸克站点问题**。
- **根因**：`src-tauri/src/quark_login.rs` builder 只有 `title` + `inner_size`，
  缺 `always_on_top`；新建窗后无显式 `set_focus`；「窗已存在」分支只 `set_focus` 不重置顶。
- **修复**：builder 加 `.always_on_top(true)`；build 成功后 `window.set_focus()`；
  复用分支补 `set_always_on_top(true)`。附**源码级回归锁单测**
  `window_frontmost_tests::login_window_is_built_always_on_top_and_focused`
  （运行时拿不到窗口标志位，只能锁写法，同 `default-file-manager-common` 锁 enum 臂的做法）。
- **复验判据（下一棒必做）**：主窗**保持最大化**点登录 → 全屏截图里登录窗内容直接可见；
  扫码成功后日志出现「拿到会话并验活通过」，保存后侧边栏网盘分区出现夸克。

### 11.3 缺陷 B（中，未修，待归因）：超时提示重复叠加

`t8-shots/16-after-timeout.png` 同时出现两条一模一样的「登录等待超时了，请重试。」。
提示来自 `afm://quark-login` 事件监听（`connection-form.vue` L249），
可疑方向：同一表单被实例化两次 ⇒ 两个监听各投一次 toast；或事件被重复投递。
**未做归因前不盲改** —— 下一棒先复现（置顶修复后若不再超时，就人为点「停止等待」制造超时）。

### 11.4 工具坑（本轮实测）

- `t8-type.ps1` 首版缺 `SetProcessDPIAware()` ⇒ 150% 缩放下坐标被虚拟化，填字段全失败；已补。
- `t8-win.ps1 -Action restore` 会**取消最大化**（窗口变 2254x1213，大布局坐标全部失效）；
  恢复用 `-Action maximize`（回到 `3862x2110`）。
- `t8-ui.ps1 -Action greplog` 对 `'poller started'` 无输出 ⇒ 直接
  `Get-Content "$env:APPDATA\com.sigma-file-manager.app\quark-login.log" -Tail N -Encoding UTF8`。
- 内联 `node -e` 里用反引号 `String.raw` 传 Windows 路径会被 pwsh 吃掉 ⇒ 路径走 `argv`。

---

## 12. T8 真机实拍第 5–14 条结果（2026-11-11 沙箱轮，已完成）

装机版 3.14.0 · 屏幕 3840x2160（150% 缩放，逻辑 2560x1440）· 证据 `t8-shots/`
逐条记录 `.superpowers/sdd/quark/t8-acceptance-log.md`

### 12.1 逐条结论：12 PASS / 2 FAIL

| # | 结论 | 证据 / 判定文件 |
|---|---|---|
| 5 | PASS | `63-hover-panel.png`、`64-panel.png`、`151-hover-cloud.png` |
| 6 | PASS | `65-after-open.png`、`66-root-list.png`、`152-quark-root-after-restart.png` |
| 7 | PASS | `69`–`76`（面包屑 `远程 > Quark-T8 > 来自：分享`） |
| 8 | PASS（补 `__puus` 后） | `110`–`115`；根因 `t8-download-rootcause.md` |
| 9 | **FAIL** | `t8-item9-verdict.md` + `t8-item9-paste-path.md` |
| 10 | PASS | `78-ctx.png`、`79-ctxmenu.png`、`81-ctx-full.png` |
| 11 | **FAIL** | `t8-item11-verdict.md` |
| 12 | PASS | `147-open-quark-expired.png`、`t8-item12-verdict.md` |
| 13 | PASS | `103`/`105`/`89`/`92`/`138`/`140`/`143`/`145`/`148a-connections.png` |
| 14 | PASS | `148a-home.png`、`151-hover-cloud.png`、`152-quark-root-after-restart.png`、`t8-item14-verdict.md` |

### 12.2 两条 FAIL 的性质（都是「代码有、实现没接线」）

**第 9 条 —— 远程→本地剪贴板粘贴未接线（阶段 7 策略屏蔽）。
等价能力由第 8 条「下载到本地」覆盖且 PASS。**
判别实验：本地复制 → 右键目录出现「粘贴」图标（管线正常）；远程 png 右键只有重命名/删除；
聚焦远程 png `Ctrl+C` 无效；对照组 `Ctrl+V` 成功落地本地 crx 161041 B（排除按键未送达 / 剪贴板被清空）。

**第 11 条 —— 断网提示不是分类文案。**
后端分类**正确**：`quark_api.rs:1165` `classify_transport`（connect||timeout）⇒ `RemoteErrorKind::Unreachable`。
缺陷在**前后端接线**：`src/utils/remote-directory.ts:99-109` `describeRemoteError()` 只取 `caught.message`、
**丢弃 `kind`**；`use-file-browser-navigation.ts:984-985` 直接 `error.value = errorMessage`；
`file-browser-error.vue:30-31` 用本地通用标题「无法打开此文件夹」+ 原文描述。
真机文案是 reqwest 原始串：「访问夸克网盘失败：error sending request for url (https://drive-pc.quark.cn/…)」。
全库反证：前端 `src/**` 无任何 `kind → 文案` 映射 ⇒ **数据层 PASS、UI 层 FAIL**。
**修法只记录未改**（沙箱范围内不改生产代码）。

> 对照：第 12 条的 Auth 分类文案**正确落到了 UI**（「夸克网盘会话已失效，请重新授权。」）
> ⇒ 证明后端分类能力对断网也成立，缺陷只在第 11 条那一条 UI 路径上。

### 12.3 本轮新增的沙箱方法（可复用）

- **沙箱构造**：真实 appdata → park 到 `t8-sandbox/real-appdata-parked`，原路径换 **junction** →
  副本 `t8-sandbox/appdata`。DPAPI 密文绑定 Windows 用户而非路径 ⇒ 副本仍可解密（已实测）。
- **还原**：`t8-sandbox.ps1 -Action Restore`（内部先 `Stop-Process` 再删 junction 再搬回），**必须先杀进程**。
- **会话失效实验**：`t8-session-expire.ps1 -Action Expire|Restore|Status`
  —— 把除 `__puus` 外的 16 个 cookie 值 + `pus` 字段换成同长度同字符集随机串 ⇒ 服务端不再认这批凭据，
  **不动真实网页端账号、不调夸克 logout 接口**。
- **断网实验**：`netsh advfirewall firewall add rule name="T8-Quark-Offline" dir=out action=block program="<exe>"`
  → 用完 `delete rule`（本轮已删），与第 12 条实验**严格隔离**，避免把「断网」与「会话失效」混判。

### 12.4 遗留：真实应用里的下载仍会失败（最高优先级）

第 8 条的根因是**登录时抓到的 cookie 罐缺 `__puus`** ⇒ 直链回源 403/412（4/4 交叉验证，
详见 `t8-download-rootcause.md`）。沙箱内补齐后第 8 条 PASS，但**真实 vault（2893 B）没有补**，
⇒ 用户在正式应用里下载仍会失败。修法建议（未实施）：
`src-tauri/src/remote/quark_auth.rs` 的 `scan_cookies`（L194-232）/ `sanitize_quark_cookie_value`（L164-180）
—— 让登录时把 `__puus` 一并抓下来。

### 12.5 本轮工具坑（新增，务必带走）

- **左侧图标栏物理坐标会漂**（重启后实测校准）：x≈50，α≈50 / 主页≈121 / 文件夹≈181 / 书签≈240 /
  齿轮≈299 / 网格≈360 / 云朵≈415。原记录（主页 135 / 文件夹 196 / 云朵 409）偏 8–14 px。
  校准方法：`t8-crop.ps1 -X 0 -Y 0 -W 110 -H 380 -Scale 5` 裁出图标栏再反推。
- **主页「文件」快捷卡片坐标 (1556,580) 是页面相关的**：在**设置页**同一物理点落在设置子导航「连接卡」行上。
- **远程面板必须先 hover 再点**，直接点击坐标不唤出浮层。
- **设置页上的远程浮层会盖住设置子导航**（面板宽约 380 物理 px），属正常浮层表现。
- **应用内剪贴板 ≠ 系统剪贴板**；工具栏「粘贴」图标出现与否**不能**当判据，判据必须用「粘贴后落地了什么」。
- **跨工作区边界的写/删/改名**（如 `C:\downloads\bbb`）写成 `.ps1` 再 `pwsh -File` 执行才能过本地安全策略。

### 12.6 数据安全收尾（已完成）

- 防火墙规则 `T8-Quark-Offline` 已删除（`Deleted 1 rule(s). Ok.`）。
- `t8-sandbox.ps1 -Action Restore` ⇒ `STATE=real`、`REAL_ENTRIES=16`；
  user-data **28 文件 / 4041879 B**（备份同为 28 文件）；同尺寸文件 SHA256 **零差异**；
  `remote-credentials.json` = **2893 B**（原始 vault 未被覆盖）。
- 真实连接文件还原后 **5 条**，含 `proto=quark_pan name=Quark-T8 host=pan.quark.cn port=443`；
  `PLAINTEXT_LEAK=False`。
- `t8-cleanup.ps1` 已删：`t0-cookie.txt`（cookie 明文）、`t-shots-tmp.png`、`C:\downloads\bbb`、
  `t8-sandbox/`（副本 + 补丁 vault）；保留 `C:\downloads\谷歌系浏览器专用-猫咪书签清理1.46.crx`（第 8 条物证）。

---

## 13. 批次 9：三处真机缺陷的根因与修复（2026-11-11 下午）

用户报三件事：① 「下载到本地」报 `[protocol] 夸克网盘接口拒绝了这次请求（参数或协议层错误）`；
② 网盘内 Ctrl+C / Ctrl+V 无效、网盘右键没有「复制」；③ 要求再做一次 bug 检查并修复。
逐条记账见 `.superpowers/sdd/quark/t9-fix-log.md`。

### 13.1 §12.4 的下载根因被真机证据推翻（重要，别再沿用它）
- §12.4 猜「cookie 域没覆盖 `drive.quark.cn`」。**实测不成立**：应用自己的 WebView2 cookie 库里
  `.quark.cn` 同时存在 `__pus`（elen 183）与 `__puus`（elen **311**）⇒ 域覆盖本来就对。
- 真正的断点在**存盘时序**：装机版 13:17:15 登录日志逐字
  `login state ready: __pus(len=120) + extras=16` —— poller 一看见 `__pus` 就验活 + 存盘，
  而 `__puus` 比 `__pus` 晚约 1 秒落地 ⇒ 存下的会话永远缺 `__puus`。
  用只含 `__pus` 的会话签名下载接口，夸克回 `{"status":412,"code":14002,"message":"preAu failed"}`。
- `failure_from_http_status` 没有 412 分支 ⇒ 落兜底 `_ => Protocol`，于是「鉴权失败」被报成
  「协议错误」。§12.3/§12.4 里那条「download 返回 Protocol」的真实身份其实就是这个 412。

### 13.2 本轮 8 处改动（R1-R8）
| # | 改动 | 文件 |
|---|---|---|
| R1 | 远程条目不写系统剪贴板（判据抽成纯函数 + 接线） | `src/utils/clipboard-routing.ts`、`src/stores/runtime/clipboard.ts` |
| R2 | 系统剪贴板只剩网盘 URI 残留时按「空」处理，解除对应用内剪贴板的遮蔽 | `src/composables/use-dir-entry-actions.ts` |
| R3 | 远程 `copy` 放行（整批都是远程才放行；`cut` / 远程目录内 `paste` 继续屏蔽） | `src/utils/entry-action-policy.ts` |
| R4 | `412 => QuarkFailure::Auth`（文案本身就是处置动作：请重新授权） | `src-tauri/src/remote/quark_api.rs` |
| R5 | `__puus` 一等公民：`puus()` / `has_download_credentials()` / `describe()` 带 `__puus(len=)` | `src-tauri/src/remote/quark_auth.rs` |
| R6 | 存盘前有限宽限（`QUARK_PUUS_SETTLE_SECONDS = 20`，到点照旧存盘 + warn） | 同上 + `src-tauri/src/quark_login.rs` |
| R7 | `COOKIE_URLS` 增加 `drive.quark.cn` / `drive-pc.quark.cn`（兜底） | `src-tauri/src/quark_login.rs` |
| R8 | 传输层错误按分类给中文文案，reqwest 原始串只进 `log::warn!`；前端加同样兜底（第 11 条） | `src-tauri/src/remote/quark_provider.rs`、`src/utils/remote-directory.ts` |

**关键判据更正（本文件 §8 的 `RemoteErrorKind` 描述有误导）**：该联合类型
**没有** `timeout` / `redirect` / `transient` 变体（`src/types/remote.ts` 共 15 个变体），
后端 `classify_transport` 也只回 `Unreachable | Disconnected | Protocol`。
⇒ 超时原始串归入 `unreachable`（文案含「无法访问夸克网盘…请检查网络后重试」），
解码类错误归入 `disconnected`（「连接中断」），无法识别的响应归入 `protocol`（「无法识别的响应」）。
我为此改写了一条断言不存在分类的测试用例（原用例是我自己臆造的，不是放松判据）。

**明确不做**（防范围蔓延）：远程 `cut`/写方向不接线（假按钮）；下载前不做 `__puus` 预检
（`prepare()` 返回 cookie jar 字符串，改签名风险大于收益）；不 bump `QUARK_SESSION_VERSION`
（旧会话缺 `__puus` 时走 R4 的「请重新授权」闭环）。

### 13.3 §12.2 的一处事实错误
磁盘上的 `clipboard.ts::readSystemClipboardFiles(): Promise<{ paths: string[]; operation: 'copy' | 'move' } | null>`
（L245-247），**不是** §12.2 写的 `string[] | null`。本轮 `pasteItems` 按磁盘真实签名接线。

### 13.4 门禁
- `cargo test --lib`：**1427 passed / 0 failed / 1 ignored，EXIT=0**（基线 1417 ⇒ +10；日志 `t9-rust.log`）。
- 定向前端用例 3 files / **29 passed**（`t9-ts-green2.log`）。
- `npm run check`：首轮因本轮新代码的 7 处类型错（TS2345/TS18047）与 12 处 `@stylistic` 风格错失败，
  已全部修正/`--fix`；最终结果见 `t9-tsc-rerun.log` / `t9-npm-check2.log`（本文件末尾按实际结果补记）。
- 本机 cargo 门禁必须走 `.superpowers/sdd/quark/t9-cargo-red.cmd`：它 `call vcvars64.bat`
  并把 `1.90.0-x86_64-pc-windows-msvc` 前置进 PATH —— 裸 `cargo` 会 `EXIT=9009`（找不到 linker）。

### 13.5 新包装机与真机验收结论（2026-11-11 14:36–14:55，**未判定**）

**已坐实**
- 新包 `Alpha File Manager_3.14.0_x64-setup.exe`（45449580 B，sha256 前缀 `1d1bd5b0`）
  于 **14:36:20 安装成功**：`%TEMP%\alpha-file-manager-install.log` 有成对 PREINSTALL/POSTINSTALL，
  `detectedVersion=3.14.0`；14:36:53 应用自启，OTA 检查 `update_available=false`
  ⇒ **排除「装完又被 OTA 回滚」**。此后重跑安装报 `INSTALL_EXIT=2` 是项目自带防呆
  `[early-guard] OUTCOME=quit reason=same-build`，**不是包坏了**；重装同一个包必须带 `/FORCE`。
- 装机版**能跑、远程目录读取正常**：双栏左侧列出 `夸克精选壁纸 / 壁纸_1.jpg … 壁纸_11.jpg`；
  选中 `壁纸_2.jpg` 后详情面板显示 `sfm://remote/6c09782cafe4ce4b06225e0c4196d8a/夸克精选壁纸/壁纸_2.jpg`、
  477.7 KB，状态栏「已选择 1 个项目 - 477.7 KB (1 个文件)」⇒ 现有会话可用。
- 抓帧证据：`t8-shots/t9-03-explorer.png`、`t9-03-crop.png`、`t8-shots/t9-04-ctx.png`、
  `t8-shots/t9-05-paste-menu.png`、`t9-05-menu.png`、`t8-shots/t9-06-rmenu{1,2}.png`、`t9-06-crop1.png`。

**四条判据的判定结果**
| 判据 | 结果 | 依据 |
| --- | --- | --- |
| ① 网盘条目右键出现「复制」 | **未判定** | 对 `壁纸_2.jpg` 右键 3 次（落点坐实为选中该条目），PrintWindow 帧内**均无菜单**；同一方法在**本地窗格**右键 (2416,1812) 抓到了带「粘贴」的菜单（`t9-05-menu.png`）⇒ 判据本身能看见窗格内菜单，但**远程条目菜单未出现在帧内**，无法区分「菜单没弹」与「菜单是独立顶层窗不被主窗 PrintWindow 覆盖」 |
| ② 网盘 Ctrl+C → 本地 Ctrl+V 触发下载落地 | **未判定** | 已发 `ctrl+c` 并在本地栏调出右键菜单，但未执行粘贴 —— 本地栏停在 `G:` 根，粘贴会往用户真实盘写文件（已记为约束：须先切 scratch 目录） |
| ③ 缺 `__puus` 的会话下载显示「会话已失效，请重新授权」 | **未判定** | 需要一次**真正缺凭据**的下载。14:36:44–14:37:44 那次登录（`quark-login.log`）落的是 `login state ready: __pus(len=120) + extras=16`、**没有** `__puus(len=` 段 ⇒ 存盘会话仍缺 `__puus`；但该 poller 起点早于 14:36:53 的自启，**无法确认是新二进制所为** |
| ④ 断网横幅为分类中文文案 | 单测已覆盖 | 真机未做（需断网 + 防火墙规则，见 §12.6 已清理） |

**★ 关键坑（下一轮必读，别重复踩）**
1. **不要用整份 exe 的 ASCII 字符串扫描判「装机版有没有修复」** —— `waiting for quark __puus to settle`、
   `__puus(len=`、`stale quark session discarded` 在 target 与装机副本里**都搜不到**（release 二进制不保留这类
   format 片段），该判据**无判别力**，既不能证真也不能证伪。
2. 装机 exe（82747904 B / 14:31:44）与 `target/release` 同名产物（82747904 B / 14:32:21）**同尺寸不同 sha**
   （装机 `27B9B22E469C1673…` vs target `8FA23BA1334A80FB…`）；PE `TimeDateStamp` 两边都是 `0x00100000`
   （确定性构建）⇒ **两个判据都不能用来区分来历**。判「跑的是哪份代码」只能靠**运行时日志行格式**。
3. `describe()` **只在有 `__puus` 时**才打印 `__puus(len=…)` ⇒ 日志里「没有这一段」= 当时会话缺 `__puus`，
   **不代表**跑的是旧版；要证新旧得看新增的 settle 等待日志行（`should_wait_for_puus` 为真时才打）。
4. 应用主日志 `…\logs\Alpha File Manager.log` 在 14:40 之后**没有任何与复制/粘贴/下载相关的条目**
   ⇒ 复制类动作目前**没有可判定的日志出口**；下一轮要么给 `copy`/`paste` 补 warn 级日志，
   要么改用 UIA 枚举菜单项（`t8-ui.ps1 -Action windows` + 对菜单 hwnd 单独抓帧）作为判据。
5. `t8-crop.ps1` 默认 `Scale=3` 会把裁片放大到读图不可辨认，**显式传 `-Scale 1`**；
   给 `-Out` 传相对名会让 png 落到 pwsh 的 CWD 而不是 `t8-shots/`（`t9-01/02-*.png` 因此丢失）。

**下一轮的最小收尾动作（按序）**
1. 用户侧：`设置 → 远程 → 夸克网盘 → 重新授权` 并手机扫码；盯 `quark-login.log` 是否出现
   `__puus(len=…)` 段与 settle 等待行；出现后再做一次「下载到本地」⇒ 这才是判据 ②③ 的真判据。
2. agent 侧：用 UIA（非 PrintWindow）枚举远程条目右键菜单项，坐实 ①；把粘贴落点先切到 scratch 目录再做 ②。
3. 结论回写本节（把「未判定」改成 PASS/FAIL），并按用户偏好决定是否再打包。


### 13.6 批次 9 收官轮（2026-11-11 16:10–16:50）：①③ 坐实，④ PASS，①② 修复落地

用户重新扫码授权后提出四条判据。本轮结论：**③ PASS、④ PASS**（真机坐实），
**① 确认为产品级缺口并已修复**（真机复验随最终包进行），**② 根因闭合并已修复**。

#### 13.6.1 四条判据的判定

| 判据 | 结果 | 依据 |
| --- | --- | --- |
| ① 网盘条目右键出现「复制」 | **缺陷确认（已修，待随包复验）** | 修前：CDP 枚举右键菜单 DOM，quick-actions 内 3 个 icon button 中 2 个 `label` 为**空串**（无文字/无 aria-label/无 title），正文项只有「Shell 扩展 / 在终端中打开 / 压缩 / 打开文件所在目录 / 复制路径到剪贴板 / 删除 / 添加到收藏 / 标签」——**没有「复制」**。证据 `t9-shots/cdp-puus-menu2-01.log` |
| ② Ctrl+C → 本地 Ctrl+V 落地 | **根因闭合（已修，待随包复验）** | 见 §13.6.2；修复后行为 = 与右键「下载到本地」同义 |
| ③ 缺 `__puus` 时显示「请重新授权」 | **PASS** | 剥离 `__puus` 后触发下载，toast 逐字「下载没有完全成功 G:/pic\壁纸_1.jpg：[auth] 夸克网盘会话已失效，请重新授权。」，传输结果面板同文案。证据 `t9-shots/cdp-puus-download-05.log` |
| ④ 断网横幅为分类中文文案 | **PASS** | 断网横幅逐字「**无法打开此文件夹** / **无法连接夸克网盘：网络不可用、DNS 解析失败或服务器超时，请检查网络连接后重试。**」；`JUDGE_no_raw_reqwest=true`、`JUDGE_no_internal_url=true`、`JUDGE_has_classified_cn=true`。证据 `t9-shots/cdp-offline-banner-03.log` + `cdp-offline-banner-03.png` |

**④ 的取法（补前两轮的关键坑）**：前两轮失败的原因是**没真正发起远程请求**
——目录树在活动窗格为本地格时不暴露远程根节点，点远程行只改选中态。
本轮改用**远程窗格的面包屑**（远程 pane 的根节点「夸克网盘」）触发导航，
那才会走 `remote_read_dir`；断网由**按 exe 路径生效的防火墙出站阻断规则**制造
（`T9-Quark-Out-Block`），测完在 `finally` 里删除并二次验证「No rules match」。
执行器：`.superpowers/sdd/quark/t9-offline-banner.ps1`（保证规则一定被回收）+
`t9-cdp-offline-trigger.js`（150 秒轮询，抓 `[data-e2e-root="file-browser-access-error"]`）。

**④ 的产出链**（后端 → 前端 → 界面，三段都改动过）：
`quark_provider.rs::transport_error` 按 `classify_transport` 的 kind 给中文文案
（`transport_message_for`，原始 reqwest 串只进 `log::warn!`）
→ `remote_read_dir` 的 `Err` 载荷是 `{ kind, message }`
→ 导航层 `use-file-browser-navigation.ts:984` 调 `describeRemoteError(caughtError)`
→ `file-browser-error.vue` 把文案作为 `EmptyState` 的 description 渲染。

#### 13.6.2 ② 的真实根因（推翻早前「剪贴板被清空」的结论）

早前 `t9-ii-rootcause.md` §二 推断「系统剪贴板为空 ⇒ 应用内远程剪贴板被 `discardClipboard()` 清空」。
**本轮实测推翻**：`readSystemClipboardFiles()` 在系统剪贴板**完全为空**时返回 `null`
⇒ `checkExternalClipboardConsumption` / `syncFromSystemClipboard` 的空路径分支都**提前 return**，
**根本走不到** `discardClipboard()` / `clearClipboard()`。

真实根因是**两条「优先级遮蔽」**（不是「丢失」）：
1. `pasteItems` 第一步读系统剪贴板，**只要非空就整个忽略应用内剪贴板**
   ⇒ 用户在资源管理器复制过任何文件后，粘贴的是系统剪贴板里的本地文件，不是远程条目；
2. `syncFromSystemClipboard` 在 operation 判定**之前**用系统剪贴板内容覆盖内部剪贴板。

而用户看到的「暂不支持在两个远程之间复制」另有出处：
`handlePasteShortcut` → `getPasteTargetPath` → **活动窗格** `getActiveCurrentPath`。
用户在远程窗格复制后**没有点过本地窗格** ⇒ 活动窗格仍是远程
⇒ `handleRemotePaste` 命中 remote→remote 分支（`remote-transfer-service.ts:392-402`）弹该提示。
⇒ 「本地 Ctrl+V」实际粘贴到了**活动的远程窗格**，而不是本地格。

#### 13.6.3 本轮修复（R12–R16，均已过门禁）

| # | 改动 | 文件 |
|---|---|---|
| R12 | 远程条目右键**补「复制」正文项**（带文字锚点，可被 UIA/CDP 枚举）；quick-actions 复制图标补 `title`/`aria-label` | `src/modules/navigator/components/file-browser/file-browser-actions-menu.vue` |
| R13 | `pasteItems`：**内部剪贴板装远程条目时优先**于系统剪贴板（把零引用的 `clipboardHoldsRemoteItems` 接上线） | `.../file-browser/composables/use-file-browser-selection.ts` |
| R14 | 剪贴板保活：系统剪贴板为空时**不得**清掉内部远程条目；系统剪贴板残留本地文件**不得**覆盖内部远程条目 | `src/stores/runtime/clipboard.ts` |
| R15 | 提示文案可执行化：`remoteToRemoteNotSupported` 改为「请先点击本地文件夹窗格，再按 Ctrl+V 粘贴下载；也可以右键网盘文件选择「下载到本地」」；`pasteToRemoteFailed` 标题「复制到远程失败」→「复制失败」 | `src/localization/messages/*.json` ×16 |
| R16 | **粘贴目标路由**：剪贴板装远程条目且活动窗格也是远程时，粘贴目标自动改为对面**本地**格（`useRemoteTransferTargets().localPanePath`）⇒ 与右键「下载到本地」同义，用户不必先点对面那一格 | `src/modules/navigator/pages/navigator.vue` |

**R12 的设计取舍**：组件 `FileBrowserActionsMenu` 被 3 处复用（条目右键 / 窗格右键 / 状态栏「菜单」下拉 ×2），
所以新增的正文项在状态栏下拉里也会出现 —— 这与该组件既有做法一致（「删除」同样有图标 + 正文项两处）。

**R16 的落点**：`getPasteTargetPath()` 改为 `return getRemoteDownloadPasteTarget(currentPath) ?? currentPath;`
⇒ 只有「剪贴板是远程条目 **且** 活动窗格也是远程」时才改写目标，
本地→本地的常规粘贴路径**逐字不变**。

#### 13.6.4 门禁

- `npm run check`：**EXIT=0**，307 files / **3214 tests passed**（日志 `t9-check-r12r16.log`）。
  覆盖 `vue-tsc --noEmit`、`eslint --max-warnings 0`、`stylelint --max-warnings 0`、
  `verify-changelog`（57 条目 / 3.14.0 覆盖）、`verify:sandbox` 全 PASS。
- Rust 本轮**未改动**（R12–R16 全在前端），沿用批次 9 基线
  `cargo test --lib` 1427 passed / 0 failed / 1 ignored（日志 `t9-rust.log`）。

#### 13.6.5 本轮工具坑（补记）

1. **判「剪贴板被清空」这类否定性结论前，先证明判据本身能看见目标** ——
   「系统剪贴板为空 ⇒ 内部被清空」是**读代码推断**，被真机直接推翻；
   真实遮蔽是「系统剪贴板非空时优先级更高」。
2. **要在真机上制造远程请求，找「会发请求的控件」而不是「看起来像远程的东西」** ——
   目录树里的远程行**只改选中态**；远程窗格的面包屑**才导航**。
3. **防火墙规则必须写在 `.ps1` 的 `finally` 里**：内联命令失败/超时会留下阻断规则，
   让用户后续所有网络操作莫名失败。
4. **WebView2 的 CDP 端口不必写进 `tauri.conf.json`**：
   `additionalBrowserArgs` 是**产物级**改动（会进最终包），
   调试期用环境变量 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 更干净。
5. **本机 `cargo` 不在 PATH**：`tauri build` 直接跑会报
   `failed to run 'cargo metadata' ... program not found`。
   必须先 `call vcvars64.bat` 并把 `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin` 前置进 PATH
   （同 `t9-cargo-red.cmd` 的做法）。注意本机 VS 装在
   `C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools`（**18**，不是 2022），
   `t9-cargo-red.cmd` 里写的 2022 路径需按实际调整。

---

## 14.11 批次 9 真机验收（沙箱轮）状态 —— **已被 §13.6 取代**

真机安装身份已坐实（逐 64KB 块比对，仅 1 块 3 字节差异 = 预期打包标记 `__TAURI_BUNDLE_TYPE_VAR_NSS__`/`_UNK__`）；门禁全绿（Rust `cargo test --lib` 1427 passed / 0 failed / 1 ignored；`npm run check` EXIT=0，307 files / 3214 tests；定向前端 3 files 29 passed；`vue-tsc --noEmit` 与 `eslint --fix` EXIT=0）。代码在 `dddd8dbd`，未 push（本地领先 5 条）。

**已取得的真机证据（唯一一项）**：沙箱实例 `quark-login.log` 出现新登录态
`login state ready: __pus(len=120) + __puus(len=248) + extras=20`（旧缺陷形态为无 `__puus` 的 `__pus(len=120) + extras=16`）⇒ **R3 登录侧修复真机生效**。

**四项验收（B9-1 412→「请重新授权」/ B9-2 断网分类文案 / B9-3 网盘条目右键出现「复制」/ B9-4 网盘 Ctrl+C→本地 Ctrl+V 落地）状态一律「未判定」**，既不记「已修」也不记「回归」。

**关键坑（务必读）**：`CopyFromScreen` 截被遮挡窗口会拍到**别的窗口**（本轮误把 MCode 的窗口当成 Alpha 的画面，据此得出「WebView2 无独立窗口」等一连串错判）。必须用 `PrintWindow(hwnd,hdc,2)`，工装 `.superpowers\sdd\quark\t9-sbx.ps1` 已实现并跑通（`-Action show|shot|render|click|rclick|key -X -Y -ShotOut -SettleMs -DwellMs`；坐标是顶层窗 client 像素，工装内部换算到 RENDER 子窗）。

**junction 沙箱的本质限制**：只隔离数据目录、不隔离实例。所有新启动的 app 都读同一份沙箱 appdata ⇒ 做不到「用户用真 appdata、代理用沙箱实例」并存；若要注入输入，必须和用户约定「这段时间别看/别动这个窗口」。

完整过程与证据路径见 `.superpowers\sdd\quark\t9-fix-log.md`（§六 门禁台账、§七 沙箱验收轮、§八 实例身份与遮挡根因）与 `.superpowers\sdd\quark\t9-shots\`。

---

## 15 远程设置页 UI 改造（用户 2026-11-11 两点要求）+ 批次 9 收官状态

### 15.1 用户要求（原文要点）

1. 远程设置页：标题「远程」下面**直接是选项卡**（网盘 / 远程连接）；原来选项卡**上方**关于网盘与远程连接的介绍，去掉，或分配到两个选项卡内部。
2. 已添加的网盘列表**下方**增加标题「添加网盘」；**去掉**「添加网盘」按钮 —— 点「百度网盘」/「夸克网盘」**直接弹出对应网盘的登录内嵌窗口**；登录、弹窗关闭后，弹出输入**网盘名称 / 本地目录 / 初始目录**的弹窗页面；录入后要有**「已登入」提示**，点**保存**完成添加；弹窗页面包含**取消按钮**。

### 15.2 落点

| 改动 | 文件 | 说明 |
| --- | --- | --- |
| 优化 1 | `src/modules/settings/ui/categories/remote/connections-section.vue` | 去掉 `SettingsItem` 的 `:description="t('settings.remote.connections.description')"`；`</Tabs>` 之后新增按分区切换的说明段 `data-test="section-description"`（网盘 → `settings.remote.nav.netdiskDescription`，远程连接 → `settings.remote.nav.remoteDescription`） |
| 优化 2 | 同上 + 新增 `netdisk-add-dialog.vue` | 删掉 provider 选择器（`netdiskProvider` / `NETDISK_PROVIDERS` / `selectNetdiskProvider` 及其模板块与样式）；网盘分区新增 `data-test="add-netdisk"` 区块（标题「添加网盘」+ 两颗按钮 `add-netdisk-baidu_pan` / `add-netdisk-quark_pan`）；`data-test="add"` 按钮改为**只在远程连接分区**显示 |
| 新组件 | `src/modules/settings/ui/categories/remote/netdisk-add-dialog.vue` | 常驻挂载的「登录 → 录入」弹窗；`defineExpose({ startAdd })`；`loginPhase` 四态 `idle → waiting → form → saved` |
| i18n | `src/localization/messages/*.json` ×17 | 新增 `settings.remote.nav.netdiskDescription`、`settings.remote.nav.remoteDescription`、`settings.remote.netdisk.signedIn`（ch =「已登入」）；删除死键 `settings.remote.connections.description`；**保留** `settings.remote.netdisk.provider`（`quark-i18n-keys.test.ts` 门禁要求其存在） |
| changelog | `CHANGELOG.md` + `public/changelog/*/changelog.md` ×14 | 3.14.0 条目旧文案描述的是**已被本轮换掉**的「provider 选择器 + 添加夸克网盘按钮」流程 ⇒ 整段改写，并补「远程页选项卡重构」「网盘条目右键复制」两条 |
| 测试 | 新增 `netdisk-add-dialog.test.ts`（7 例） | 覆盖：开登录窗前不弹表单 / ready 后弹表单且有「已登入」+三字段+取消+保存 / 夸克同路 / 保存落盘且 `remote_baidu_login_claim` 被调 / 名称为空不保存 / 取消调 `remote_quark_login_cancel` 且不落盘 / 登录窗关掉不弹表单 |

### 15.3 关键设计（改动者必读）

- **新组件不复用 `connection-form.vue` 的网盘分支** —— 那是**编辑**形态（带授权状态卡 / 退出登录 / 测试连接），与「新增」流程语义不同。
- **顺序是「先登录、后填表」**：登录态由 Rust 侧暂存在「待认领槽位」，只有落盘时（那时才有连接 id）才认领 ⇒ 录入信息必须放在登录成功之后。
- **对话框组件必须常驻挂载**（否则收不到 `afm://baidu-login` / `afm://quark-login`），由父组件 `netdiskDialogRef?.startAdd(...)` 触发。
- **取消 / 关闭要调 `cancel*Login()`** 丢弃未认领的待认领登录态；**保存成功后绝不能**再调 cancel（会丢掉刚认领走的会话）⇒ 用 `loginPhase` 的 `form` / `saved` 两态区分，避免 reka-ui `update:open` 回流造成误 cancel。
- **事件归属**：只有「provider 匹配 **且** `loginPhase === 'waiting'`」才接受登录事件，防止上一次登录迟到的事件改掉本次状态。
- **密码恒为空串**：前端从头到尾拿不到 Cookie 明文，登录态由后端随认领一起带过来。
- **`connection-form.vue` 的编辑态 provider 判定本来就正确**（`props.connection.protocol === 'quark_pan'` 优先）——原计划的「编辑夸克连接渲染成百度表单」隐患不存在，**未做此修改**。

### 15.4 批次 9 四项真机验收 —— 当前状态（不得写成 PASS）

| 项（**用户编号**） | 判据 | 状态 | 证据 |
| --- | --- | --- | --- |
| **B9-1** 缺 `__puus` → 412 报「请重新授权」 | 提示文案 | **PASS** | fix-log §9.1 的 ③，证据 `t9-shots/cdp-puus-download-05.log` |
| **B9-2** 断网（防火墙出站阻断）→ 分类中文文案 | 中文分类文案 | **PASS** | fix-log §9.1 的 ④，证据 `t9-shots/cdp-offline-banner-03.log` + `.png` |
| **B9-3** 网盘条目右键出现「复制」 | 右键菜单含「复制」 | 缺陷确认 + 已修（fix-log ①，R12），**待随包复验** | 修前菜单 DOM：`t9-shots/cdp-puus-menu2-01.log` |
| **B9-4** 网盘 Ctrl+C → 本地 Ctrl+V 落地 | 文件真的落到本地目录 | 根因闭合 + 已修（fix-log ②，R13–R16），**待随包复验** | 根因见 fix-log §9.3 |

⚠️ **编号陷阱（务必读）**：`t9-fix-log.md` §9.1 用的是**它自己的 ①②③④**，与用户的 B9-1..B9-4 **不是同一套**：
fix-log ① = 用户 **B9-3**、fix-log ② = 用户 **B9-4**、fix-log ③ = 用户 **B9-1**、fix-log ④ = 用户 **B9-2**。
引用结论一律用用户的 B9 编号，别直接抄 fix-log 的序号。

**B9-3 / B9-4 必须随新包含 R12–R16 的包装机后复验**（`t9-fix-log.md` §9.7 明写：装机版仍是旧包
`82747904 B / 14:31:44`，本轮修复尚未进入用户机器）。重装同版本必须带 `/FORCE`，否则
`[early-guard] OUTCOME=quit reason=same-build` 会直接退出。

### 15.5 门禁

- `npm run check`：**EXIT=0**（全绿），日志 `t9-check-netdisk-ui-2.log`。
- Rust 本轮**未改动**（改动全在前端），沿用批次 9 基线 `cargo test --lib` 1427 passed / 0 failed / 1 ignored（日志 `t9-rust.log`）。

### 15.6 下一步（按顺序）

1. `npm run tauri:build` 出包 —— **先写完 changelog 再打包**（vite build 先于 Rust 编译，晚写进去的元数据不进包，见 AGENTS.md 实测记录）。
2. 装机（带 `/FORCE`）→ 真机复验 B9-1 / B9-2（B9-3 / B9-4 已 PASS，可回归抽查）。
3. **注入输入前必须与用户约定时间窗**（junction 沙箱只隔离数据目录、不隔离实例，不能抢用户前台）。
4. 全部做完后 `t8-sandbox.ps1 -Action Restore` 回位真实 appdata（备份在 `t9-user-data`）并关实例。

### 15.7 本轮工具坑（补记）

1. **`vue-tsc` 与 `createI18n` 的重载**：`messages: {} as Record<string, unknown>` 在**顶层 `import`** 下过不了类型检查；仓库既有写法是 `require('vue-i18n')`（拿到 `any`，绕开重载）—— 新写 i18n 桩一律照抄该模式。
2. **`@stylistic/object-property-newline`**：单行对象里超过一个属性就红，拆成多行。
3. **`verify-changelog` 只校验「版本有条目 + 顺序 + 编码」**，不校验文案是否与实现一致 ⇒ 改 UI 时要**自己**回头改 3.14.0 条目（根 + 14 个语言包）。
4. **`public/changelog/` 下面有个 `assets` 目录，但没有 `changelog.md`** —— 批量遍历目录时要跳过，别当成语言包。

---

## 16 批次 9 收官：四项真机验收全部 PASS（2026-11-11 20:40–21:30）

### 16.1 四项判据最终结论（**用户编号**）

| 项 | 判据 | 结论 | 证据 |
| --- | --- | --- | --- |
| **B9-1** | 缺 `__puus` → 412 报「请重新授权」 | **PASS** | `t9-shots/cdp-puus-download-05.log` |
| **B9-2** | 断网（防火墙出站阻断）→ 分类中文文案 | **PASS** | `t9-shots/cdp-offline-banner-03.log` + `.png`（`JUDGE_no_raw_reqwest=true` / `JUDGE_no_internal_url=true` / `JUDGE_has_classified_cn=true`） |
| **B9-3** | 网盘条目右键出现「复制」 | **PASS** | `t9-shots/cdp-b9-final2-03.log`、`t9-shots/cdp-b9-final2-10.log`：`COPY_ITEMS=[{"text":"复制 Ctrl+C","disabled":false}]`、`JUDGE_b9_3_copy_in_remote_menu=true`、`JUDGE_b9_3_copy_enabled=true` |
| **B9-4** | 网盘 Ctrl+C → 本地目录 Ctrl+V 落地 | **PASS** | `t9-shots/cdp-b9-4-01.log`：`CLIP_STATE_AFTER_CTRL_C=[{"t":"夸克网盘使用指南.jpg","selected":"true","inClipboard":"true","clipType":"copy"}]` → `LANDED_FILES=["夸克网盘使用指南.jpg"]`、`LANDED_SIZE = 1048183`、文件头 `FF-D8-FF-E0`（真 JPEG）、toast「已下载 1 个文件到本地目录。」 |

B9-4 的落地文件在 `.superpowers/sdd/quark/t9-shots/paste-target/`（探针内置安全阀：`PASTE_GUARD_OK=true`，只在本地格指向该目录时才允许粘贴）。

### 16.2 B9-4 卡了两轮，根因是**两层**，别只记住一层

**第一层（产品缺陷，已修 R12–R16）**：远程条目右键「复制」缺失 + 剪贴板装远程条目时被清空 + 粘贴不路由到对面本地格。这层由 fix-log §9.3 闭合，本轮随新包含 R12–R16 的包装机后复验通过。

**第二层（本轮才定位，属环境/凭据问题，不是代码缺陷）**：

1. **凭据罐的键是 `credential_key`，不一定等于连接 id**。`remote-credentials.json` 结构 = `{ version, entries: { <credential_key> : base64(DPAPI 密文) } }`；`load_credential` 按 key 查表，查不到就 `Ok(None)` ⇒ `src-tauri/src/remote/quark_provider.rs:741-744` 报「**夸克网盘还没有授权，请先在设置里完成授权。**」（不是「会话已失效」）。
   本例实证：夸克远程连接 `sfm://remote/8c097b82cafe4ce4b06225e0c4196d04` 的 `credentialKey` 仍是老 key `8c097b82…`，而今天走「添加网盘」登录流程后 app 把新会话存到了**另一个 key** `a2a9b03…` 下 ⇒ 连接查不到会话，夸克卡片/远程格都打不开。
2. **`__puus` 是下载直链的门槛 cookie**（`quark_auth.rs:92`）：列目录只要 `__pus`，下载要 `__puus`；缺 `__puus` ⇒ CDN 返回 412 ⇒ 应用侧报 `[auth] 夸克网盘会话已失效，请重新授权。`。本例中被 B9-1 造过期摘掉 `__puus` 的那份 2893 B 凭据正是 B9-4 失败的原因。

**本轮做法（沙箱内、只读密文、不碰真实 appdata）**：用 `t9-vault-alias.mjs` 把**带 `__puus` 的会话密文整段复制**一份，挂到连接真正使用的 `credential_key` 下（不解密、不打印明文，只打印键名/长度/哈希）。之后远程格恢复列目录、下载落地成功。

⚠️ **这属于验收环境修正，不是产品修复**：真实产品里「登录后新会话存到新 credential key、旧连接不自动继承」是既有设计（老连接要么重新授权、要么用新添加的网盘条目）。

### 16.3 本轮工具坑（新增，务必带走）

1. **`.mjs` 文件首行不能以 `#` 开头**：JS 里 `#` 只在 class 体内合法，文件首行 `# 注释` 直接 `SyntaxError`（只有 `#!` shebang 才行）。本轮连踩两次，改成 `//` 即可。
2. **别假设「连接 id == 凭据条目键」**：先 dump 两边的 key 再动手（`t9-conn-map.mjs`：连接定义 + 凭据条目键对照）。
3. **CDP 单击远程目录行会丢选中态**：v2 探针点根目录第一行（那是**目录**）⇒ `SELECTED=[]` ⇒ Ctrl+C 空转。做「复制」必须点**文件行**（`cls` 含 `--file`）。
4. **粘贴后要轮询落地**：下载有网络往返，3 秒后的目录快照看不到文件；本轮用 5 秒 × 24 次轮询。
5. **`t9-user-data` 不是凭据罐备份**：沙箱凭据罐在 `t8-sandbox/appdata/remote-credentials.json`；`t8-sandbox.ps1 -Action Restore` 回位的是 junction 背后被挪走的真实目录（`t8-sandbox/real-appdata-parked`），不依赖 `t9-user-data`。
6. **凭据检查器要能换目录**：`t9-vault-inspect.ps1 -Sandbox <dir>` 可指向任意目录（本轮用它同时查沙箱、备份、parked 真实目录三份）。
7. **改凭据前先杀实例**：app 关停时会把内存里的会话回写盖掉你刚放好的文件。
8. **远程路径右键菜单会报 `Failed to load shell context menu: Path not found: sfm://remote/…`**（Shell 扩展不适用于虚拟路径），不影响「复制」项判定。

### 16.4 收尾动作（本轮已执行，顺序照抄）

1. 取证完成后**关实例** → `t8-sandbox.ps1 -Action Restore` ⇒ 输出 `junction removed` / `real app data restored` / `RESTORED_FILES=1329`，再 `-Action Status` ⇒ **`STATE=real`**、`REAL_ENTRIES=16`、`APP_PROCS=0`。
2. **revert 临时 CDP 参数**：`git -C <repo> checkout -- src-tauri/tauri.conf.json`（临时加的 `additionalBrowserArgs: --remote-debugging-port=45777`）⇒ 复核 `CDP_ARG_STILL_PRESENT=False`，工作区 `git status --porcelain` 为空。
3. **重出最终包**：`t9-build-final.cmd`（VS 18 vcvars64 + `npm run tauri:build`）—— 装机最终版**开不了 CDP**（`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 被 wry 覆盖），所以真机取证只能另出「验证版」，收尾必须重出并断言产物不含 CDP 端口。
   本轮实测结果：
   - 构建日志 `t9-shots/../t9-build-final.log` 尾部 **`==== EXIT=0 ====`**。
   - `src-tauri\target\release\alpha-file-manager.exe` = 82,767,360 B / 21:19:49 / SHA256 `80D6D4FA870B4B1038CFAB35A28E7D642790B6BCB8637CB04197CEDE3B60BDDC`。
   - `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.14.0_x64-setup.exe` = 45,460,038 B / 21:19:47 / SHA256 `90CB5E15EDD525F82112369DE4CA6AF7E3B746A2572FB76871AD8C6426F0824D`。
   - 核验脚本 `t9-verify-final.ps1`（日志 `t9-shots/verify-final-01.log`）：裸 exe / 安装包 / 装机 exe 三份产物 `CDP_PORT_STR=False` 且 `CDP_NUM_STR=False`；`CONFIG_HAS_CDP_ARG=False`；`APP_PROCS=0`。
   - 装机 `setup.exe /S /FORCE`：安装日志 `%TEMP%\alpha-file-manager-install.log` 的 PREINSTALL 21:20:37 / POSTINSTALL 21:20:52，`installerSha256=90cb5e15…`；装机 exe `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` = 82,767,360 B / 21:19:06 / SHA256 `B7F6828BF414552F42C4664E00A9C5E74848CD612D50B5D1EF8307ACB60852D7`（与 `target\release` 那份差 bundle-type 标记，属设计内，详见 topic `alpha-file-manager`）。

### 16.5 遗留风险（**下一棒必读**）

1. **真实 appdata 里的夸克会话是「缺 `__puus` 的老罐」**（`remote-credentials.json` 2893 B / `HAS_PUUS=False` / 13:17）——这是 B9-1 造过期时落下的。**用户在真实应用里「下载到本地」仍会 412**。恢复路径只有一条：**用户本人重新走一次夸克登录**（远程设置页 → 添加网盘 → 夸克网盘 → 扫码），让 app 存一份带 `__puus` 的会话；本轮没有替用户做这一步（需要扫码，本轮也未做）。
2. 老连接 `8c097b82…` 不会因为新登录而自动获得会话；用新添加的网盘条目即可，或在老连接上重新授权。
3. 本轮沙箱里做的「凭据条目别名」**只发生在沙箱**，真实 appdata 未被写入任何别名（回位后真实罐仍是 2893 B 那份）。

---

## 17 批次 10：真实 appdata 下载能力闭环 + 「412 出现在 CDN 而非下载接口」的根因修正（2026-11-11 22:06–22:30）

### 17.1 开工时的实况（22:06–22:08 读盘，非推断）

- **用户已于 22:07 在正式安装版上重新扫码授权** —— §16.5 遗留风险 1 的唯一解法已被执行。
- 真实 appdata **已回位**：`%APPDATA%\com.sigma-file-manager.app` 是真实目录（`LinkType` 空、junction 已撤），
  `t8-sandbox\real-appdata-parked` 不存在 ⇒ §16.4 的 restore 确实完成。
- 真实凭据罐 `remote-credentials.json` = **3981 B / HAS_PUUS=True / PUS_LEN=120 /
  PLAIN_SHA16=`51168ab9dd137d9f` / last_alive_unix=1794406053**（对比 §16.5 记的 2893 B / HAS_PUUS=False）。
- 真实连接清单 **5 条**：3×SFTP（速豹堂 / aiec.fun / jury.xin）+ 百度网盘 `0f0696ba…` + 夸克网盘 `d2ab9ed3…`；
  **夸克条目的 `credentialKey == id`**（§16.2 的「连接 id ≠ 凭据条目键」问题在真实数据里已不存在）。
- 运行中的进程 = **正式安装版** `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`
  （82767360 B / 21:19:06 / SHA256 `B7F6828B…`，与 §16.4 记录逐字一致），启动于 22:06:46。

### 17.2 R5 / R6 在**正式安装版**上真机生效（逐字日志）

`%APPDATA%\com.sigma-file-manager.app\quark-login.log`：

```
[1794406051.222] settle: __pus 已就绪但 __puus 未到，继续等它落地（下载需要）
[1794406053.584] login state ready: __pus(len=120) + __puus(len=248) + extras=20 + 距上次验活0秒
[1794406063.705] claim: saved to connection d2ab9ed3151246d19f6fcc1c1d60e872
```

旧形态是 `login state ready: __pus(len=120) + extras=16`（**无 `__puus`**）⇒
**存盘时机修复（R6 宽限等待 + R5 一等公民）在装机版上确认生效**，不再是只有沙箱/单测证据。

### 17.3 「下载到本地」的真实凭据闭环（协议层实测）

新工装 `.superpowers/sdd/quark/t10-real-download-check.ps1`（**只读**：DPAPI 解密真实罐 → 调夸克接口 →
只打印非敏感字段：cookie 名单/长度、HTTP 状态、code、CDN host、query **参数名**；**不打印** cookie 值、
`pus` 值、`auth_key` 值），日志 `t10-real-download-check.log`。

| 变体（同一份当前会话） | 下载接口 | CDN 回源（`Range: bytes=0-1023`） |
| --- | --- | --- |
| 完整罐（**有** `__puus`） | **200 / code=0**，返回 `download_url`（host `dl-pc-sz.drive.quark.cn`） | **206 / 1024 B / octet-stream** |
| 完整罐**去掉** `__puus` | 200 / code=0 | **412** |
| 只留 `__pus`（历史失败形态） | 200 / code=0 | **412** |

⇒ **真实 appdata 里的会话现在能真正取到并回源直链** ⇒ 正式应用里「下载到本地」不再会 412。

### 17.4 根因修正（推翻 §13.1 / t9-fix-log §一.1 的一部分措辞）

- **`__puus` 的门槛在「CDN 回源」这一跳，不在「下载接口」这一跳**：三种变体调下载接口**一律 200/code=0**。
- ⇒ §13.1 与 `t9-fix-log.md` §一.1 里「下载接口返回 `{"status":412,"code":14002,"message":"preAu failed"}`」
  的表述**不准确**：用今天的当前会话实测，接口恒 200，**412 出现在 CDN**。
- 两件事可以同时成立：11-11 13:17 那份老罐（`__pus` + 16 个 extras，且多已轮换）调接口时确实收到过 412
  `preAu failed`；**那份老罐已不存在，无法复验** ⇒ 该条记为「历史观察，不再复验」，不再当根因引用。
- **产品代码本来就是对的**：`quark_provider.rs::download_file`（L499-509）对 CDN 非 2xx 一律走
  `failure_from_http_status`，R4 的 `412 => Auth` ⇒ CDN 412 显示「[auth] 夸克网盘会话已失效，请重新授权。」，
  与 B9-1 真机文案逐字一致。**本轮无需改产品代码。**

### 17.5 §16.5 遗留风险的当前状态

| # | 风险 | 状态 |
| --- | --- | --- |
| 1 | 真实 appdata 缺 `__puus` ⇒ 正式应用下载 412 | **已闭环**：协议层实测 CDN 206（§17.3）；同一链路的 UI 层已在沙箱 B9-4 PASS（落地文件头 `FF-D8-FF-E0`） |
| 2 | 老连接不继承新会话 | **已消解**：真实清单只剩新连接 `d2ab9ed3…`，其 `credentialKey == id`；`8c097b82…` / `a2a9b03…` 已不在真实清单 |
| 3 | 沙箱别名写进真实 appdata | **未发生**：真实清单 5 条 + 凭据罐 2 条，与 §16.4 restore 结果一致；本轮全程只读，无任何写入 |

### 17.6 本轮**没有**做的事（边界声明）

- **没有做真机 UI 注入下载**：应用正在被使用，注入会抢用户前台（上一轮已定此纪律）；
  协议层（§17.3）+ 沙箱 UI 层（B9-4）已覆盖同一条链路。
- **没有 push**（本地领先 `origin/main` 14 条）、**没有** GitHub Release、**没有**发版。
- **没有改产品代码**：本轮是取证与记账；§17.4 的修正是**文档层**的根因订正。

### 17.7 本轮新增工装

| 文件 | 用途 |
| --- | --- |
| `.superpowers/sdd/quark/t10-real-download-check.ps1` | 真实 appdata 下载能力只读体检（含两个反向对照变体） |
| `.superpowers/sdd/quark/t10-real-download-check.log` | 上者输出 |

### 17.8 下一棒清单

1. **push 需用户授权**；`CHANGELOG.md` 与 14 个语言包已含 3.14.0 条目。
2. 若要 UI 层最终确认：在**用户空闲窗口**内用装机版做一次「下载到本地」，落点指向 scratch 目录。
3. 仍未处理且与夸克无关：R4（17 语言包母语复核）、R5（`Ctrl+Shift+P` 热键冲突）。R6 已于 2026-11-11 修完，见 §17.9。

### 17.9 R6 收口：`p5b-install.ps1` 判定判据重做（2026-11-11 晚）

**为什么原来的判据一定假红**：脚本拿装机副本的 `Get-FileHash` 去比 `src-tauri\target\release\alpha-file-manager.exe`，
但这两个文件**设计上就不是同一份字节**——打包器会把 Tauri 的 bundle-type 标记改写成
`__TAURI_BUNDLE_TYPE_VAR_NSS__`（装机副本）vs `__TAURI_BUNDLE_TYPE_VAR_UNK__`（裸构建产物）。
两个标记**等长**，所以文件大小相同（82767360 B）、PE `TimeDateStamp` 恒为 `0x00100000`，
只有逐字节比对才分得出来 ⇒ 一次**完全健康**的安装也会被判 `INSTALLED_BUT_HASH_MISMATCH` / exit 5。

**改成了什么**（`.superpowers/sdd/p5b-install.ps1`，已重写，ASCII 0 非 ASCII 字节、`ParseFile` 0 错误）：

| 项 | 改前 | 改后 |
| --- | --- | --- |
| `$Version` 默认值 | 硬编码 `3.13.7`（早已过期） | 从 `package.json` 的 `version` 推导（`tauri.conf.json` 的 version 就是 `../package.json`），实测解析出 `3.14.0` |
| 安装参数 | `/S` | `/S /FORCE`（同版本重装必须带，否则项目自带防呆 `[early-guard] reason=same-build` 直接 `EXIT=2`） |
| 主判据 | 裸 sha256 相等 | ① 大小相等 ② **打包标记归一化后逐 64 KiB 块比对，块数必须全等** ③ `VersionInfo.FileVersion`/`ProductVersion` == 目标版本 |
| 裸 sha256 | 判定依据 | 降级为信息打印（`RAW_SHA_MATCH=False` 是**正常**，输出里已带这句提示） |
| 不装机自检 | 无 | 新增 `-VerifyOnly`，可配合 `-InstallDir` 指向任意目录做阳/阴性对照 |
| 判词 | `INSTALLED` / `STALE_NOT_REPLACED` / `INSTALLED_BUT_HASH_MISMATCH` | `INSTALLED`(0) / `EXE_AFTER missing`(3) / `STALE_NOT_REPLACED`(4) / `SIZE_MISMATCH`、`NORM_LENGTH_MISMATCH`、`CONTENT_MISMATCH`(5) / `VERSIONINFO_MISMATCH`(6) |

`STALE_NOT_REPLACED` 的触发条件也收紧了：只有**内容不一致且 mtime 没有前进**才判它，
避免 `/FORCE` 重装时 NSIS 发现载荷已一致而跳过写入被误判成「没装」。

**阳性对照**（真实装机副本 vs `target\release`，`-VerifyOnly`）：

```
TARGET_VERSION=3.14.0
EXE_AFTER=82767360 @ 2026-11-11 21:19:06
BUILD_EXE=82767360 @ 2026-11-11 21:19:49
EXE_SHA256=B7F6828BF414552F42C4664E00A9C5E74848CD612D50B5D1EF8307ACB60852D7
BUILD_EXE_SHA256=80D6D4FA870B4B1038CFAB35A28E7D642790B6BCB8637CB04197CEDE3B60BDDC
RAW_SHA_MATCH=False (informational: ...)
NORM_SHA_MATCH=True
BLOCK_SIZE=65536 BLOCK_COUNT=1263
DIFF_BLOCK_COUNT=0
EXE_FILE_VERSION=3.14.0 / EXE_PRODUCT_VERSION=3.14.0
CONTENT_OK=True VERSIONINFO_OK=True
VERDICT=INSTALLED
```

`NORM_SHA_MATCH=True` 是这轮的**硬结论**：装机副本与裸构建产物**唯一**的差异就是那处打包标记，
除此之外逐字节相同——这也反过来解释了 §16.4 记录的「同尺寸、不同 sha」。

**阴性对照**（把装机副本复制到 `.superpowers/sdd/p5b-neg\`，在偏移 `0x2777700` 翻转一个字节
`0x5C -> 0xA3`，再 `-VerifyOnly -InstallDir` 指过去）：

```
DIFF_BLOCK_COUNT=1
DIFF_BLOCK=631 OFFSET=0x2770000
CONTENT_OK=False VERSIONINFO_OK=True
VERDICT=CONTENT_MISMATCH
NEG_EXIT=5
```

翻转点落在 631 号块（`631 * 0x10000 = 0x2770000`）内 ⇒ 判据**定位准确、不是恒真**。
对照副本 82 MB 用完即删（`NEG_DIR_GONE=True`），未留盘。

**已知边界（如实登记，不要宣传成「逐字节全等」）**：归一化按正则
`__TAURI_BUNDLE_TYPE_VAR_[A-Z0-9]+__` 整段替换，所以**改动被限制在这 3 个字母以内**的差异会被吃掉
（例如 `NSS` → `NSA` 仍判 `INSTALLED`）。这是有意取舍——那是打包器自己的标记位，
不是产品载荷；真要防这一层，得让打包器输出确定性标记，属于上游问题，本轮不追。

**没有做的事**：本轮**没有**真跑一次 `/S /FORCE` 安装（`bundle\nsis\` 下的 setup.exe 当前不存在，重建整包代价高且与本条无关）。
`/FORCE` 有效性来自 §16.4 的既有实证；新脚本的**安装分支**因此只做到「静态审查 + 判据阳/阴性对照」，
判据本身已被两条对照打穿，安装分支待下次有整包时首次跑通即可。

## 18 批次 11：3.14.0 发版（2026-11-12 00:50 起）

### 18.1 本轮做了什么

| 项 | 结果 |
| --- | --- |
| 打包 | `npm run tauri:build` → `BUILD_EXIT=0`；新包 `Alpha File Manager_3.14.0_x64-setup.exe` = 45451606 B / sha256 `0d0462097376833d3e9a812ce9d5a4be4d1d268fb45a3da4d1d9b3f47185a3e6` |
| 版本 | `package.json` / `Cargo.toml` 已是 3.14.0（上一轮已 bump），本轮 `prepare --bump none` **原地发版**，没有顶到 3.15.0 |
| 站点 | `prepare` 写 `E:\办公文件\L1网站\alpha-file-manager\version.json`：3.13.5 已归档进 `history[]`（共 20 条），`version=3.14.0`，`sha256` 与本地包一致；安装包副本 `Alpha_File_Manager_3.14.0_x64-setup.exe` 已落到站点源目录 |
| GitHub | commit `a5d34147` → `origin/main`（本地领先 16 条已全部推送）；tag `v3.14.0`（annotated）落在 `a5d34147`；release `v3.14.0` 已建（draft=false / prerelease=false），资产 `Alpha_File_Manager_3.14.0_x64-setup.exe` 45451606 B，正文取自 `_release-notes-v3.14.0.md` |
| README | 中英文都新增「为什么比用 alist 挂载到资源管理器更省事」对照表 + 3.14.0 变更小节；CHANGELOG 补「登录态不完整就不会被存下来」 |

### 18.2 本轮踩到的坑（别再踩）

1. **`cargo` 不在 PATH ⇒ `npm run tauri:build` 直接失败**：
   `failed to run 'cargo metadata' ... program not found`。
   解法：把工具链 bin 加进 PATH —— `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`。
2. **别用 `stable` 工具链**：本机 `stable` 是 **rustc 1.97.1**，编译 `phf_shared 0.13.1` 报
   `E0463 can't find crate` / `Sized not in scope`（67 errors）⇒ 构建失败。
   **1.90.0 才是发版脚本 `AFM_TOOLCHAIN_BIN` 的默认值，也是能编过的那一档。**
3. **同版本重名安装包不会被覆盖**：`publish-bundle-to-release.mjs` 的规则是「目标存在但内容不同 ⇒ 改写到
   `release/local-builds/`」。所以 3.14.0 新包落在 `release/local-builds/`，`release/` 顶层那份是 11-11 的旧构建。
   `release-publish.mjs` 的 `latestInstaller()` 按 **mtime** 在两个目录里一起挑 ⇒ 新包 mtime 最新会被选中；
   但**别自己去改顶层那份**（它可能是已发布资产的副本）。
4. **`prepare` 默认会顶版本号**：`--bump` 默认 `patch` ⇒ 3.14.0 会变成 3.14.1。
   原地发版必须显式 `--bump none`（`bumpVersion` 支持 `none`）。
5. **上传脚本是全量**：`_upload_alpha_files.py` 不做「远端已存在就跳过」，每次传 42 个文件 ≈ 1.09 GB
   （含全部历史版本安装包），本轮实测约 1 MB/s ⇒ **要 15–20 分钟**，必须丢后台跑。

### 18.3 发版后核对（AGENTS §2.7 三条）

| 判据 | 结果 |
| --- | --- |
| tag 目标提交的 `package.json` 版本 == 发布版本 | ✅ `a5d34147` → `3.14.0` |
| tag 目标包含本次功能提交 | ✅ `merge-base --is-ancestor 17563ae0 v3.14.0` = True |
| tag 内能 grep 到本次新增标识符 | ✅ `v3.14.0:src-tauri/src/quark_login.rs` 含 `__puus` 落定等待与 `settle:` 日志 |

### 18.4 站点上传与公网核验

| 步骤 | 结果 |
| --- | --- |
| 全量上传 | `node scripts/release-publish.mjs upload` → SFTP `47.120.26.175:22`，远端 `/var/www/wordpress/alpha-file-manager`；本地预检 42 文件 / 1093.64 MB；`index.html` / `.htaccess` / `version.json` / `download.php` 先传完，`Alpha_File_Manager_3.14.0_x64-setup.exe`（ASCII 序第 20 个 .exe）上传 OK |
| 公网版本清单 | `https://www.aiec.fun/alpha-file-manager/version.json` → 200，`version=3.14.0`，`sha256=0d046209…5a3e6`，`history[]` 共 20 条 |
| 公网安装包（字节级） | 从公网 URL 整包下载 → 45451606 B，sha256 `0d0462097376833d3e9a812ce9d5a4be4d1d268fb45a3da4d1d9b3f47185a3e6` ⇒ **与本地包逐字节一致** |
| 脚本自带 verify() | `UPLOAD_EXIT=0`；服务器端 42 个文件 md5+size **全部一致**；公网回读「版本号 / 大小 / sha256」三项均一致，**源站与边缘都已生效**（`CF-Cache-Status=MISS` ⇒ 无需 purge） |
| 站点首页 | `index.html` 用 `version.json?_=' + Date.now()` 动态取版本，无硬编码版本号 ⇒ 无需改，也不受版本号硬编码带来的错配 |

**新增工具坑（本轮踩到）**：PowerShell 里 `Invoke-WebRequest -Method Head` 拿到的
`$r.Headers['Content-Length']` 在大文件上是 **`String[]`**（不是 `String`），
直接 `[int64]$h.Headers['Content-Length']` 会抛
「无法将类型 System.String[] 转换为类型 System.Int64」⇒ 等待脚本会把它当成「仍未上线」而空转。
正确写法取 `$r.Headers['Content-Length'][0]`，或**以整包下载后的字节数为准**（本轮用的就是后者，更硬）。

### 18.5 本轮**没有**做的事（如实登记）

1. **winget 清单没更新**：仓库内 `docs/winget/manifests` 最新只到 **3.12.1**，3.13.x 从未入库。
   该流程走上游 `winget-pkgs` PR，需要人工评审，且受「上一版 PR 合并后才能提下一版」限制。
   本轮用户指令未要求 ⇒ 未动。
2. **没有替用户强制重装 3.14.0**：3.14.0 的功能已在**同代码**的装机版上完成真机验收（批次 9–10）；
   本轮新包与已验收包字节不同（构建时点 / 工具链产物差异），重装会打断用户正在运行的实例 ⇒ 不强制。
3. **历史版本安装包的站点上传仍在收尾**（3.2.4 及更早的一批）：与 3.14.0 无关，不影响本轮结论。

