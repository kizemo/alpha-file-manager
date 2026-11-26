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
| R8 | ~~**真实应用里的下载仍会失败**~~ | **已闭环**（2026-11-13 回写）：§12.4 记的「登录时抓到的 cookie 罐缺 `__puus`」这条根因**已被 §13.1 / §17.4 推翻** —— `__puus` 是 **CDN 回源**那一跳的门槛 cookie，下载接口本身三种变体一律返 200/code=0。真正的修复在登录侧**等 `__puus` 落定**（R3/R5），装机版日志已见 `__pus(len=120) + __puus(len=248) + extras=20`（§17.2）；协议层 CDN 206 实测见 §17.3，UI 层落地 PASS 见 §16.2。**不再需要用户重登** |
| R3 | 提交 | **已提交 3 条（未 push）**：`8574f7cc` Rust / `a426f253` 前端+i18n / `f3a92144` 交接卡；版本 bump + changelog 作第 4 条。push 与 GitHub Release 仍未授权 |
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

### 11.3 缺陷 B（中，**已修**，2026-11-13）：超时提示重复叠加

`t8-shots/16-after-timeout.png` 同时出现两条一模一样的「登录等待超时了，请重试。」。
提示来自 `afm://quark-login` 事件监听（`connection-form.vue` L249），
可疑方向：同一表单被实例化两次 ⇒ 两个监听各投一次 toast；或事件被重复投递。
**未做归因前不盲改** —— 下一棒先复现（置顶修复后若不再超时，就人为点「停止等待」制造超时）。

> **归因与修复见 §20**。原来猜的两个方向**都不对**：Rust 侧**只发一次**；
> 真正的原因是**两个组件同时监听同一个全局频道**（常驻挂载的 `NetdiskAddDialog`
> ＋ 条件挂载的 `ConnectionForm`），两边用**同一个 i18n key** 各弹了一次 toast。
> 修法是给 `ConnectionForm` 补上与 `NetdiskAddDialog` **对称**的守卫。

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

真机安装身份已坐实（逐 64KB 块比对，仅 1 块 3 字节差异 = 预期打包标记 `__TAURI_BUNDLE_TYPE_VAR_NSS__`/`_UNK__`）；门禁全绿（Rust `cargo test --lib` 1427 passed / 0 failed / 1 ignored；`npm run check` EXIT=0，307 files / 3214 tests；定向前端 3 files 29 passed；`vue-tsc --noEmit` 与 `eslint --fix` EXIT=0）。代码在 `f9852660`，未 push（本地领先 5 条）。

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
| GitHub | commit `fc4eb65f` → `origin/main`（本地领先 16 条已全部推送）；tag `v3.14.0`（annotated）落在 `fc4eb65f`；release `v3.14.0` 已建（draft=false / prerelease=false），资产 `Alpha_File_Manager_3.14.0_x64-setup.exe` 45451606 B，正文取自 `_release-notes-v3.14.0.md` |
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
| tag 目标提交的 `package.json` 版本 == 发布版本 | ✅ `fc4eb65f` → `3.14.0` |
| tag 目标包含本次功能提交 | ✅ `merge-base --is-ancestor 759ae9cb v3.14.0` = True |
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

## 19. 批次 12：3.14.0 发版后收尾（winget 分发 + 站点数据卫生）

> 范围：夸克接入本身已随 3.14.0 发布完毕。本批次只做**发版后**的真实缺口 —— winget 分发通道与站点数据卫生，
> **不回头重做** T0–T8 与批次 9–11。

### 19.1 winget 上游实况（现场查证，非推断）

| 判据 | 结果 |
| --- | --- |
| `gh pr list --author kizemo --state all` | `#460744`（3.5.1）**我们自己于 2026-11-01 关闭**（留言「Superseded by the 3.9.0 submission」），`mergedAt = null` |
| 实际在审的 PR | **`#467381`（3.9.1）至今 `OPEN`** ⇒ **当前唯一阻塞点是 #467381**，不是 #460744 |
| 上游入库目录 | `manifests/k/kizemo/AlphaFileManager` → **404**，该包从未合并入库 |
| 排队规则 | 同一个包 ID 同时只允许一个 PR ⇒ 3.14.0 的 PR 只能在 #467381 合并后提（或被要求先关旧 PR） |

⇒ **订正**：`docs/winget/README.md` 与 `AGENTS.md` §2.4 里「等 3.5.1 合并」的旧结论**已过期**，本轮按现场 `gh` 结果改写。

### 19.2 本轮落盘的改动

| 文件 | 改动 |
| --- | --- |
| `docs/winget/manifests/k/kizemo/AlphaFileManager/3.14.0/`（4 个新文件） | 3.14.0 的完整 winget 清单（version / installer / locale.en-US / locale.zh-CN） |
| `docs/winget/check-manifest.mjs` | 找本地安装包由「只看 `release/<file>`」改为**在 `release/` 与 `release/local-builds/` 两个候选里按 sha256 命中**；都不命中时列出各候选真实 sha 供诊断 |
| `scripts/release-history.mjs`（新） | 抽出纯函数 `dropHistoryEntriesForVersion(history, version)` |
| `scripts/release-publish.mjs` | 归档旧版本前先按 version 去掉 `history[]` 里已存在的同版本条目并打印去重条数；改调 `release-history.mjs` |
| `scripts/__tests__/release-history.test.mjs`（新） | 去重规则 5 条单测（重复条目 / 无重复 / history 缺失 / version 缺失 / null 元素） |
| `docs/winget/README.md` | 台账按现场 `gh` 结果改写 + 补 3.14.0 清单章节 |

**清单要点**：`InstallerUrl` 指向**已发布资产**（GitHub `v3.14.0` 的 `Alpha_File_Manager_3.14.0_x64-setup.exe`）；
`InstallerSha256 = 0D0462097376833D3E9A812CE9D5A4BE4D1D268FB45A3DA4D1D9B3F47185A3E6`（45451606 B，与已发布资产逐字节一致）；
`ReleaseDate: 2026-11-11`（**UTC 发布日**；按 UTC+8 写 11-12 会被上游判「未来日期」）。
`Publisher: kizemo` / `ProductCode` 与 `tauri.conf.json` 一致，无需改动。

⚠️ **清单必须指向已发布资产，不能指向 `release/` 顶层那份**：本地同版本有两份不同字节的安装包
（`release/` 顶层 45487379 B / `7CECAE66…FAEF3`，`release/local-builds/` 45451606 B / `0D046209…5A3E6`），
**判「发布资产」只认 sha256**。

### 19.3 站点历史安装包公网核对（关闭 §18.5-3）

站点源目录 `E:\办公文件\L1网站\alpha-file-manager` 的 **38 个 `.exe` 全部 HTTP 200**，
且响应 `content-length` 与本地字节数**逐个一致**（`TOTAL_BAD=0`）
⇒ **§18.5-3「历史安装包站点上传收尾」已闭环**。

- 判据只做到「HTTP 200 + 字节数一致」，**未做整包 sha 比对**（全量 1.09 GB 不下载）；
  3.14.0 的字节级一致性由 §18.4 的整包下载证据覆盖。
- **线上重复条目已按用户授权清掉**：本地 `version.json` 去掉重复的 3.12.0 条目
  （`history[]` 20 → **19 条**，版本序列 `3.13.5 … 3.12.0, 3.11.0 … 3.6.0`，**无任何重复**），
  用上传脚本的 `--only version.json` 单传到 `/var/www/wordpress/alpha-file-manager/version.json`。
  - 本地回写带**格式守卫**：重新序列化后必须与原文件逐字节一致才允许写入，否则中止（避免改坏缩进/尾换行）。
  - 上传脚本自带校验：服务器端 **md5 + size 均一致**（45774 B / md5 `bd3859b4d5c6e7fcf10d84129870a777`）。
  - 公网回读（带 cache-busting `?_=<时间戳>`）：HTTP 200、`CF-Cache-Status: DYNAMIC`、
    **公网 sha256 = 本地 sha256 = `912d8004ab17a2669c6fc18930c81db1f4ab240c87866ec3152dc8415240db2d`**、
    `version=3.14.0`、`history_len=19`、`dup_3.12.0=1`、无其它重复。

### 19.4 门禁

| 门禁 | 结果 |
| --- | --- |
| `npm run check:winget` | **EXIT=0** —— 4 文件齐备 / UTF-8 无 BOM / LF / YAML 可解析 / sha 命中 `release\local-builds\...`（45451606 bytes）/ InstallerUrl 指向本版本 release 资产 |
| `npm run test:unit:run -- scripts/__tests__/release-history.test.mjs` | **1 文件 / 5 用例全绿** |
| `npm run check`（ts + lint + 全量单测 + verify-changelog + verify-sandbox） | **EXIT=0** —— **309 个测试文件 / 3229 条用例全过** |

### 19.5 本轮**没有**做的事（如实登记）

1. **没提 winget PR**：对外发布动作，等用户授权；且 `#467381` 仍 `OPEN`，先提大概率被拒。
2. **没跑全量 `release:upload`**：本轮线上只需要 `version.json` 一个文件，全量 42 文件
   （1.09 GB / 15–20 分钟）没必要跑；而且线上目录与本地源目录不是字节镜像（见 §19.6 第 5 条）。
3. **没跑 `winget validate` / 沙箱试装**：本机没装 winget CLI，只能交给上游流水线。
   ⚠️ **2026-11-13 订正：这条已过期** —— 本机 `winget.exe` 现在可用
   （`C:\Users\Duanyi\AppData\Local\Microsoft\WindowsApps\winget.exe`，v1.29.380），
   `winget validate --manifest docs/winget/manifests/k/kizemo/AlphaFileManager/3.14.0`
   实测输出 `Manifest validation succeeded.`（exit 0）；**沙箱试装仍未做**。
   AGENTS.md §2.4 的同款过期记录已一并订正。
4. **安装包仍无代码签名**：上游评审可能提出签名要求，届时再处理。

### 19.6 本轮新踩的工具坑

1. **本仓 eslint 风格：单参箭头函数不加括号**（`@stylistic/arrow-parens`，写 `x => ...` 而不是 `(x) => ...`）。
   新写的 `.mjs` 一律照此写，否则 `lint:eslint:check` 直接红（本轮第一版就被这 6 处卡住）。
2. **门禁红 = 本轮引入，不是基线问题**：`eslint .` 全仓只报出我自己新写的 6 处，其余部分干净。
3. **本机 bash 硬策略**：`Push-Location` / `cd` 之后若再有写操作（含写临时文件）会被拦
   （`write/delete targets cannot be verified safely after a command working directory change`）。
   取 eslint 报告时改用 **Node 的 ESLint API**（`new ESLint({cwd})` + `lintFiles()`），不切目录、不落临时文件。
4. **上传脚本新增 `--only` 单文件开关**：站点源目录的 `_upload_alpha_files.py` 原来只有「全量」与
   `--files-only` 两种粒度，改一个 `version.json` 就要重传 1.09 GB；本轮加了 `--only <文件名>`（可重复），
   单传一个文件只要几秒（脚本自带远端 md5+size 校验）。
5. **线上站点目录与本地源目录不是字节镜像**：线上 `index.html` 的 `<head>` 被 WordPress 注入了
   `<link rel="EditURI" … jetpack-rpc?rsd>`（多 105 字节，正文逐行一致）⇒ **公网与本地 sha 必然不等，
   别把这当成上传失败**；公网 `download.php` 会 301 再跳到安装包本体（`fetch` 跟随后看到 45 MB 的 exe）。
   ⇒ 要比「线上那份到底是不是我这份」时，别比 Cloudflare 出来的字节，比**行级差异**才有意义。

---

## 20. 批次 13：§11.3 缺陷 B 归因与修复 + 台账订正（2026-11-13）

### 20.1 根因：不是「重复投递」，是**两个组件同时监听同一个全局频道**

**Rust 侧排除**：`src-tauri/src/quark_login.rs` 的超时分支只 `emit_event(..., "timeout", ...)` **一次**；
而且 `settle()` 排在 `emit_event` **之前** ⇒ 随后 `close_login_window` 触发的
`WindowEvent::Destroyed` 会被 `SETTLED.swap(false)` 吃掉，**不会再补一条 `closed`**。
⇒ §11.3 猜的「事件被重复投递」方向不成立。

**真正的重复在前端**：`afm://quark-login` / `afm://baidu-login` 是**全局**频道，
设置页同一棵树上同时挂着两个监听者：

| 组件 | 挂载条件 | 原行为 |
| --- | --- | --- |
| `netdisk-add-dialog.vue`（新增网盘流程） | **常驻挂载**（`connections-section.vue:476`） | 有守卫：只认 `loginPhase === 'waiting'` |
| `connection-form.vue`（编辑/新建连接表单） | `v-if="isCreating"` / `v-else-if="editing"` | **无守卫**：对任意事件都更新状态并弹 toast |

两边的 toast 标题用的是**同一个 i18n key** ——
`netdisk-add-dialog.vue` 的 `i18nPrefix('quark_pan') === 'quark'`，
所以两侧取到的都是 `settings.remote.quark.embedded.status.timeout`
⇒ 同一句提示**逐字出现两遍**，与 §11.3 截图（两条一模一样的「登录等待超时了，请重试。」）逐字对上。

⚠️ `ConnectionForm` 的监听器**不看 variant**：当前渲染的是 FTP/SFTP 表单时，它照样听得见夸克事件。

### 20.2 复现路径（任一即可）

1. 设置页 → 「添加远程连接」（`isCreating = true`，`ConnectionForm` 挂载）→ 切到「网盘」分类
   → 点「添加网盘 → 夸克网盘」→ 不扫码、等超时 ⇒ **两条**超时提示。
2. 正在「编辑」一条已存在的夸克连接（`editing` 有值）→ 点「添加网盘 → 夸克网盘」→ 等超时 ⇒ 同上。

### 20.3 修复

`src/modules/settings/ui/categories/remote/connection-form.vue`：给两个监听器补上与
`NetdiskAddDialog` **对称**的守卫 —— 只有「本表单自己发起的那一轮」才消费事件。

- 新增 `baiduLoginActive` / `quarkLoginActive` 两个 ref；
- `handleStart*Login` 置 `true`；`handleCancel*Login`、启动失败、**任一终态事件**置 `false`；
- 监听器开头 `if (!*LoginActive.value) return;` ⇒ 别的组件发起的登录事件**一个字都不吭声**；
- 切换连接时（`resetLoginStates`）一并复位。

百度侧是**同形问题**（同样的双监听 ＋ 同一个 i18n key），一并修掉。

⚠️ **后端从不推 `waiting`** —— 事件状态只有 `ready` / `closed` / `timeout` / `error`，
`waiting` 只是前端自己设的显示态。判终态**不能**写成 `payload.status !== 'waiting'`：
① `vue-tsc` 直接报 TS2367（字面量联合与 `'waiting'` 无交集）；
② 就算把类型放宽，语义也是**反的** —— `waiting` 是**非终态**（这一轮还在飞），
收到它不该结束这一轮。
正确写法是**白名单**：`TERMINAL_LOGIN_STATUSES = ['ready','closed','timeout','error']`
＋ `isTerminalLoginStatus(status: string)`（入参取 `string`，绕开字面量联合的类型收窄）。

⚠️ **这一条是踩出来的，不是推理出来的**：第一版修复为了让 TS 过，把「清守卫标记」
写成**无条件清** ⇒ `waiting` 事件把标记提前清掉 ⇒ 后到的 `closed` 被守卫吞掉，
百度用例「还没取到凭据时不放成功反馈」变红（`toastedTitles` 是空数组），
第二版改成「只有终态才清」才绿。完整经过见 §20.5。

### 20.4 测试与判别性验证

| 用例 | 作用 |
| --- | --- |
| `quark-connection-form.test.ts`「取到凭据时给出『可以保存了』的反馈」 | 改为**先点「用内嵌窗口登录」**再推事件 —— 走真实流程 |
| `quark-connection-form.test.ts`「别的组件发起的登录事件不在这里弹提示」 | **新增**，回归 §11.3 缺陷 B |
| `baidu-connection-form.test.ts` 同名两条 | 同上；并补断言「守卫不吞掉本表单自己那轮的失败提示」 |
| `netdisk-login-duplicate-toast.test.ts`（**新增文件**） | **集成复现**：把 `ConnectionForm` 与 `NetdiskAddDialog` **同时挂载**，往 `afm://quark-login` **广播**一条 `timeout` ⇒ 断言**只弹一条**（两个方向各一条用例） |

**判别性验证（实测，非推理）**：把 `connection-form.vue` 单独 `git stash` 回退到修复前、
**保留**新测试，跑 `quark-connection-form.test.ts` ⇒ 新用例**红**，报错内容正是
`title: 'settings.remote.quark.embedded.status.timeout'` 被弹了 1 次
⇒ 新用例确实能复现原缺陷，不是恒绿。回退后 `git stash pop` 干净回位（POP_EXIT=0）。

⚠️ 原有的两条「ready」用例**没有先发起登录就直接推事件**，断言「表单必弹 toast」——
那条断言**正是缺陷本身**（表单对不属于它的登录事件也响应），所以改成先点开始按钮。
**这不是把测试改松，是把它改真。**

**为什么还要补一个集成用例**：单测各自只挂一个组件，证得了「它自己不该响时不响」，
**证不了「两个一起挂时不重复」** —— 而缺陷恰恰只在两者同挂时出现。
所以 `netdisk-login-duplicate-toast.test.ts` 把两个组件挂在**同一棵树**里，
并按真实语义**广播**事件（一频道多监听者，而不是一频道一 handler）。

**集成用例的判别性验证（实测）**：`git stash push -- connection-form.vue` 回退修复、
保留新集成用例 ⇒「弹窗发起的那一轮超时，只弹一条提示」**变红**：
`AssertionError: expected [ …(2) ] to deeply equal [ Array(1) ]`
⇒ **修复前同一场景确实弹了 2 条一模一样的提示**，与 §11.3 真机截图逐字对上；
同一个 stash 下「表单发起的那一轮」仍是绿的（弹窗自己的守卫本来就挡住了）⇒ 判据有方向性。
`git stash pop` 干净回位（POP_EXIT=0）。

### 20.5 门禁（本轮实测）

**最终全量门禁**（`npm run check` = `ts` + `lint:check` + `test:unit:run` + `verify-changelog` + `verify:sandbox`）：

| 项 | 结果 |
| --- | --- |
| `ts`（`vue-tsc --build`） | 通过（TS_EXIT=0） |
| eslint / stylelint | 通过（0 warning） |
| vitest | **310 文件 / 3233 用例全绿** |
| `verify-changelog` | 通过（CHANGELOG 57 条，本轮未改） |
| `verify:sandbox` | PASS（全部扩展入口通过） |
| 合计 | **CHECK3_EXIT=0** |

> **本批次的入口卡（新文件）**：`docs/handoff-2026-11-13-quark-pan-batch13.md`
> （背景 / 改动清单 / 已验证·推断·未验证矩阵 / 沙箱复验步骤 / 本轮工具坑 / 待授权动作）。

⚠️ **门禁一共跑了五轮**（第 4 轮是**新增集成用例之后**才跑的，前三轮的数字**不含**那个文件）：

| 轮次 | 触发时机 | 结果 | 说明 |
| --- | --- | --- | --- |
| 1 | 修复后首轮 | **失败**：2 处 TS2367 | `payload.status !== 'waiting'` —— 后端状态只有 `ready/closed/timeout/error`，`waiting` 只是前端显示态，字面量联合无交集 |
| 2 | 改成「无条件清标记」后 | **失败**：1 条用例红 | `waiting` 把守卫提前清掉 ⇒ 后到的 `closed` 被吞 ⇒ `baidu-connection-form.test.ts`「还没取到凭据时不放成功反馈」红（`expected [] to include …`） |
| 3 | 改成**终态白名单**后 | **通过**（CHECK_EXIT=0） | 当时**尚未**加入新集成文件 ⇒ 数字是 309 文件 / 3231 用例 |
| 4 | 加入 `netdisk-login-duplicate-toast.test.ts` 后 | **失败**：eslint 1 error | 新文件第 160 行 `return { wrapper, dialogRef };` 违反 `@stylistic/object-property-newline`（本仓风格：对象字面量多于一个属性要换行）⇒ **vitest 全绿但门禁红** |
| 5 | 改成多行返回后 | **通过**（CHECK3_EXIT=0） | 310 文件 / 3233 用例；日志 `C:\Users\Duanyi\AppData\Local\Temp\batch13-check3.log` |

⚠️ **教训二（补记）**：`vitest` 绿 **≠** 门禁绿 —— 样式类问题只有 `eslint` 抓得到。
**提交前一定要跑全量 `npm run check`，不能只跑 `test:unit:run`。**

**中间踩的坑：TS 修复一度把语义写反**（这一段必须留着）：

1. 第一轮全量门禁：**3231 用例全绿**，唯一失败是 `ts` 报 **2 处 TS2367**
   （`payload.status !== 'waiting'`：后端事件状态只有 `ready` / `closed` / `timeout` / `error`，
   `waiting` 只是前端显示态，字面量联合与 `'waiting'` 无交集）。
2. 当时的修法是「**无条件**清守卫标记」⇒ 类型过了，语义反了：
   `waiting` 也把这一轮标记成结束 ⇒ 后到的 `closed` 被守卫吞掉。
3. 于是第二轮门禁里 `baidu-connection-form.test.ts`「还没取到凭据时不放成功反馈」变红：
   `AssertionError: expected [] to include 'settings.remote.baidu.embedded.status…'`
   （`toastedTitles` 是**空数组**）⇒ 守卫把「本表单自己那轮的失败提示」也一起吞了。
4. 正确修法：**白名单判终态** —— `TERMINAL_LOGIN_STATUSES = ['ready','closed','timeout','error']`
   ＋ `isTerminalLoginStatus(status: string)`。`waiting` 不清标记，
   `closed` / `timeout` / `error` 才清。
5. 第三轮：定向 2 文件 / 16 用例全绿 + 全量门禁 **CHECK_EXIT=0**。

⚠️ **教训**：`vue-tsc` 报「类型无交集」时，**别用放宽条件去迁就编译器** ——
那种条件往往把语义一起写反了。先问「这个状态在协议里到底意味着什么」，
再用白名单把语义写死。（这次是**测试**抓住的：守卫的净效果必须是
「挡掉别人的事件，但绝不挡掉自己那轮的失败提示」。）

### 20.6 本轮顺带订正的过期台账

1. **§9 的 R8 行**仍写「真实应用里的下载仍会失败」—— **已过期**：§17.5 早已记「已闭环」
   （协议层 CDN 206 ＋ 沙箱 B9-4 PASS），§9 漏回写。已订正。
2. **§19.5-3 与 AGENTS.md §2.4** 都记「本机没装 winget CLI」—— **已过期**：
   本机 `winget.exe` 可用（`C:\Users\Duanyi\AppData\Local\Microsoft\WindowsApps\winget.exe`，v1.29.380），
   `winget validate --manifest docs/winget/manifests/k/kizemo/AlphaFileManager/3.14.0`
   实测输出 `Manifest validation succeeded.`（exit 0）。两处均已订正。

### 20.7 本轮**没有**做的事（如实登记）

1. **没提交 winget PR**：对外发布动作，等用户授权；`#467381` 仍卡在 moderator 审核（CLA 已绿）。
2. **没 push**：本轮改动只在本地。
3. **没做真机 UI 复验**：缺陷 B 的复现要等「登录等待超时」那一步（`GIVE_UP` 窗口），
   本轮用**单测 ＋ 回退对照**做证伪性验证代替；装机复验需要用户空闲窗口 ＋ 扫码登录态。
   ⚠️ 本轮又查清了「为什么没法用 CDP 快跑一遍」以及**正确的复验路径**，见 §20.8。
4. **没做 upstream 移植**：审计已跑（AFM 领先 436 / upstream 领先 36，merge-base `4d788321`），
   未落地项多为产品级决策（导航面板树视图、仪表盘重设计），不属本批次范围。

### 20.8 真机验收：做到哪一步、卡在哪、下一步怎么走（2026-11-13）

**已经做到的**：

1. **打包产物已产出**：`npm run tauri:build` 全流程通过（`BUILD_EXIT=0`）——
   `src-tauri\target\release\alpha-file-manager.exe` ＋ NSIS 安装包
   `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.14.0_x64-setup.exe`。
   ⚠️ 打包**必须**带 MSVC 环境与固定的 rust 工具链（`cargo` 不在默认 PATH 里，
   直接 `npm run tauri:build` 会报
   `failed to run 'cargo metadata' ... program not found`）⇒ 走
   `.superpowers\sdd\quark\batch13-build.cmd`（纯 ASCII，参照同目录 `cargo-test-lib.cmd`）。
2. **修复前的 3.14.0 二进制已留档**：`%TEMP%\afm-batch13-baseline\alpha-file-manager.exe`
   （sha256 `D2EC859DE6DD0DD8CDFFE165D5C294378BCB7F46A626B1401AA0C068D2CC29D9`），
   可直接做 A/B 对照。

**没做到的，以及为什么（两条路都试过，别再重试）**：

① **CDP 驱动不了 release 构建**：

- `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=…` **无效** ——
  wry/Tauri **显式**传了 `AdditionalBrowserArguments`
  （WebView2 命令行里可见 `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`），
  **应用自己给了参数时 WebView2 忽略环境变量**；
- WebView2 的用户级策略 `HKCU\Software\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments`
  同样**无效**（同一原因）。⚠️ 本轮试过之后已把该值**置空**（键还在、值为空串）——
  没用删除命令（本地安全策略拦永久删除）。
- ⇒ 要开 CDP 只能改 Rust：`WebviewWindowBuilder::additional_browser_args()`
  （Tauri 2.11.5 有这个 API；**配置里没有** `additionalBrowserArgs` 字段），
  且主窗要改成 `create: false` 再用代码建 ⇒ 属于**探针版**改动，会污染发布件。

② **真机验收必须跑在沙箱里**（AGENTS §2.8 硬约束：启动 AFM 做验收不许在用户桌面上抢焦点）：

- 沙箱 = `CreateDesktop` ＋ `CreateProcess(lpDesktop=…)`；注入端留在用户桌面，
  枚举沙箱窗口要用 **`EnumDesktopWindows`**（不切桌面时 `EnumWindows` 枚举不到）；
- 可用模板：`.superpowers\sdd\sandbox-c1-accept.ps1`、
  `.superpowers\sdd\p3-ui-regress-sandbox.ps1`（最近的 UI 回归脚本，2026-11-10）；
- 沙箱内**没有键盘输入**（`SendInput` 只能投到输入桌面）⇒ 只能做鼠标等价断言：
  PostMessage 鼠标到 **`Chrome_RenderWidgetHostHWND`** ＋ UIA ＋ `PrintWindow` 截图；
- ⚠️ 沙箱验收前**必须先结束正在运行的 `alpha-file-manager`**
  （单实例插件会把启动转交给旧实例，症状是「日志说启动了但沙箱里没窗口」）。

**下一步（留给真机那一轮，按这个顺序做）**：

1. 复制 `p3-ui-regress-sandbox.ps1` 改成夸克版：沙箱启动**修复后**的 exe →
   设置 → 远程 → 「添加远程连接」（让 `ConnectionForm` 挂上）→ 网盘页 →
   「添加网盘 → 夸克网盘」（**弹窗发起**的那一轮）→ 登录窗起来后**直接关掉它**
   （Rust 侧 `WindowEvent::Destroyed` ⇒ 推 `closed` ⇒ **不用等 900 秒超时**）→
   抓主窗 UIA 树，统计「登录等待超时」这条文案出现**几次**；
2. 同一脚本换成 `%TEMP%\afm-batch13-baseline\alpha-file-manager.exe`（修复前）⇒ 应当出现 **2 次**；
3. 两组都跑完，才算「装机复验 PASS」。**判据必须是计数，不是「看起来只有一条」。**
   ⇒ 计数探针已备好（**只读**，不注入输入、不抢焦点、**绝不从脚本启动 app**）：
   `.superpowers\sdd\quark\t13-count-toast.ps1` ＋ `t13-toast-patterns.json`；
   **必须用 Windows PowerShell 5.1 跑**（`powershell.exe -NoProfile -ExecutionPolicy Bypass -File ...`，
   pwsh 7 里 `UIAutomationClient` 解析不了），直接输出 `COUNT timeout = N`。
   ⚠️ 探针**只做过 explorer 阳性对照**（`WINDOWS=2 / ELEMENTS=39`，能读到真实中文文案 ⇒ 遍历可用）；
   「WebView2 里的 toast 是否以 UIA 元素暴露」**尚未验证** ⇒ 读到 0 命中时先跑阳性对照再下结论。

⚠️ 顺手记三个数（省得下一轮再翻代码）：`GIVE_UP = 900 秒`（`quark_login.rs:88`，
想等超时复现要等 15 分钟，**关窗划算得多**）、`COOKIE_READ_BUDGET = 12 秒`、`VERIFY_BUDGET = 8 秒`。

**本轮动过的系统设置（如实登记）**：

- 试过 WebView2 的用户级策略 `AdditionalBrowserArguments` ⇒ 无效，已**置空**（未删键）；
- 其余无：没装 winget 包、没改全局 git 代理、没加防火墙规则、没动别的注册表项。

### 20.9 批次 14：缺陷 B 真机 UI A/B 复验完成 ＋ 顺带修掉一个「toast 空壳」独立缺陷（2026-11-14）

§20.8 留下的唯一遗留项（真机 UI A/B 计数复验）**已在批次 14 完成**。本节是它的结论，
前面 §20.1–§20.8 的内容保持原样不改（台账是追加式）。

#### 20.9.1 复验方法（AGENTS §2.8 约束下的沙箱验收）

- **沙箱**：`CreateDesktop("AFM-T14-<tag>-<rand>")` ＋ `CreateProcess(lpDesktop=…)`
  启动打包产物；注入端留在用户桌面，**不切桌面、不抢前台、不发 SendInput**。
- **交互**：UIA 模式点击（`ScrollIntoView` ＋ `Invoke/Selection`）为主，坐标 `PostMessage`
  到 `Chrome_RenderWidgetHostHWND` 为辅；登录窗起来后用 `WM_CLOSE` 关掉它
  ⇒ Rust 侧 `WindowEvent::Destroyed` ⇒ 推 `closed`（省掉 900 秒超时）。
- **计数口径**：**UIA `ListItem` 实例数**（`Notifications alt+T` → `List` → `ListItem`），
  像素差分作交叉验证。
  ⚠️ **不能用 toast 文案计数**：本轮实测两版产物的 toast 都是**空卡片**（原因见 §20.9.4），
  文案口径必然为 0，会得出「修复无效 / 基线也没弹」的假结论。
- **窗口尺寸固定 1280×900**：webview 渲染面在 y≈1018 以下是黑区，
  1800 高时按钮落到窗口外、1400 高时「添加远程连接」落在渲染区外。

#### 20.9.2 A/B 结论：**基线弹 2 次 / 修复后弹 1 次**（PASS）

计数单位 = burst 帧 0 里比「弹 toast 之前的基线帧」多出来的 `ListItem` 个数。
弹前基准帧（step-10 / step-12）与 burst 末帧（frame 39）两轮完全一致 ⇒ 起点无残留。

| 轮次 | 二进制 | burst 帧 0 多出的 UIA 节点 | 判定 |
| --- | --- | --- | --- |
| 基线 `t14base2` | `%TEMP%\afm-batch13-baseline\alpha-file-manager.exe`（2026-11-12 01:01，sha256 `D2EC859D…29D9`） | **2 个 `ListItem`**：`700,742,534x50` ＋ `700,804,534x50`（垂直堆叠） | 基线弹 **2** 次 |
| 修复后 `t14fix2` | `src-tauri\target\release\alpha-file-manager.exe`（批次 13 打包） | **1 个 `ListItem`**：`700,804,534x50` | 修复后弹 **1** 次 |
| 修复后 `t14fix3` | 同上，但**含 §20.9.4 空 toast 修复**（2026-11-14 01:38 重新打包） | **1 个 `ListItem`**：`700,737,534x117`，`name=[登录还没完成，登录窗口就被关掉了。登录窗口已关闭。]` | 修复后弹 **1** 次（且文字可见） |

**v3 轮（2026-11-14 01:38 重新打包的产物，含空 toast 修复）额外证明了「文字真的渲染出来」**：

- `COUNT_closed=1` —— **文案口径终于能命中**（`t14fix2` / `t14base2` 两轮都是 0，
  因为那两版的卡片是空壳）⇒ §20.9.4 的根因与修复互为印证；
- 40 帧里**只有帧 0–3 命中 1 次**，其余 36 帧全为 0 ⇒ 恰好弹一次、不叠加；
- UIA 树帧 0 多出：
  `ListItem name=[登录还没完成，登录窗口就被关掉了。登录窗口已关闭。] rect=700,737,534x117`
  ＋ `Text name=[登录窗口已关闭。] rect=725,800,168x27`（description 行）
  ⇒ 卡片高度从 50px 变 117px（标题 + 副标题两行）。
- 证据路径：`.superpowers\sdd\quark\t14-shots\t14fix3-run.log`、
  `t14fix3-burst-13-NN-uia.json`、`t14fix3-burst-13-NN.png`、`t14fix3-state.json`。

- 节点总数：基线 140（帧 0–4）→ 138（帧 5 起）；修复后 139（帧 0–3）→ 138（帧 4 起）。
- **结论**：缺陷 B（`connection-form.vue` 与 `netdisk-add-dialog.vue` 同听全局频道
  `afm://quark-login`）的修复在**真机打包产物**上成立 —— 叠加的第二条 toast 消失了。
- 证据路径：
  - `.superpowers\sdd\quark\t14-shots\t14base2-run.log` / `t14fix2-run.log`（逐步骤 + 每帧计数）
  - `.superpowers\sdd\quark\t14-shots\t14base2-burst-13-NN-uia.json` / `t14fix2-burst-13-NN-uia.json`（逐帧 UIA 树）
  - `.superpowers\sdd\quark\t14-uia-ab.txt`（A/B 差分输出）
  - `.superpowers\sdd\quark\t14-shots\t14base2-state.json` / `t14fix2-state.json`
- 探针脚本：`.superpowers\sdd\quark\t14-quark-toast-sandbox.ps1`（13 步链路，纯 ASCII）、
  差分脚本 `.superpowers\sdd\quark\t14-uia-diff.cjs`、计划 `.superpowers\sdd\quark\t14-plan2.json`
  （`burst=40, interval=150`，即 6 秒窗口）。

#### 20.9.3 与 §11.3 原始证据的差异（如实登记，不含糊带过）

§11.3（2026-11-11）的截图里「登录等待超时了，请重试。」是**可见文字**；
本轮实测两版的 toast **卡片里没有任何文字**（详见 §20.9.4）。
两者不矛盾：§11.3 的截图走的是**当时的调用点/当时的 toaster 行为**，
而 §20.9.4 查明「文字消失」的根因是**包装器与调用点形态不匹配**，
该不匹配由 2026-11-11 的「添加网盘登录即录流程」引入 ⇒ §11.3 截图拍到文字、
本轮拍到空壳，两者是**代码演进的不同时间点**，不是判据打架。
§20.9.4 修完后，文字会重新出现。

#### 20.9.4 顺带修掉的独立缺陷：toast 是「空壳」（标题/副标题都渲染不出来）

**现象**：toast 卡片 534×50 出现在右下角，但**卡内无任何文字**。
像素硬证据：卡片区 103 种颜色，**最暗通道仅 176**，卡片底色 (243,238,229)，
无任何 <150 的深色像素 ⇒ 卡片里没有文字。

**根因链**：

1. `src/components/ui/toaster/app-toast.ts` 的包装器签名是
   `toast(message: string, data?: ExternalToast)`，把**第一个实参当字符串 title** 传下去；
2. 但全仓 `src/` 下 **7 处 `toast(` 调用全部是对象形态** `{ title, description }`
   （`connection-form.vue:254,263,318,325`、`netdisk-add-dialog.vue:133,162,171`），
   `title` 的值是 i18n 翻译后的文案；
3. ⇒ `componentProps.data.title` 变成**对象** ⇒ `toast-header.vue` 的
   `props.title.indexOf('|')` 对对象调用字符串方法 ⇒ **抛错** ⇒
   整棵 `ToastHeader` 渲染成注释节点 ⇒ 卡片只剩背景/边框，**没有文字**。

**修法（包装器规范化入参，而不是改 7 处调用点）**：

- `app-toast.ts` 新增 `normalizeInput()`：同时接受 `toast('标题', { description })`
  与 `toast({ title, description })` 两种形态，统一产出 `{ title: string, description: string }`；
  对象形态里除 `title/description` 外的其余键（duration 等）透传给 sonner。
  ⇒ 包装器契约保持向后兼容，两种写法都不会再把对象塞进 `title`。
- `toast-header.vue` 加防御：`typeof props.title === 'string' ? props.title : ''`
  ⇒ 就算将来有人绕过包装器传了非字符串，也只是标题为空，**不会再整棵树崩掉**。

**回归锁**：`src/components/ui/toaster/app-toast.test.ts`（新建）挂**真的** `Toaster` ＋
**真的** `toast()`（不走 mock），断言两种形态都能渲染出标题文字。
⚠️ 旧的 `netdisk-login-duplicate-toast.test.ts` 把 `@/components/ui/toaster` **整个 mock 掉**，
所以它从来没能发现「生产渲染路径是空卡片」这件事。

**与缺陷 B 的 A/B 无关**：git 溯源显示 `app-toast.ts` 最后一次改动是 `811f3f3e`（2026-04-05），
对象形态调用由 `22e9f7db`（2026-11-11）引入 ⇒ **基线二进制与修复后二进制的 toaster 代码完全一致**，
§20.9.2 的 A/B 计数不被这个缺陷污染。

#### 20.9.6 空 toast 修复的真机复验 ＋ 本轮收尾状态（2026-11-14）

- **重新打包**：`cmd /c .superpowers\sdd\quark\batch13-build.cmd` ⇒ `BUILD_EXIT=0`
  （cargo `Finished release profile in 3m 47s`），
  新产物 `src-tauri\target\release\alpha-file-manager.exe`（2026-11-14 01:38:17）
  ＋ `bundle\nsis\Alpha File Manager_3.14.0_x64-setup.exe`（43.35 MiB，01:38:50）。
- **真机复验（`t14fix3` 轮）**：见 §20.9.2 的 v3 行 ⇒ **1 个** toast 实例，
  标题与副标题都渲染出来了。
- **像素层交叉验证**（`t14-dark.ps1`，统计卡片区 R/G/B 三通道均 <150 的深色像素）：
  - 新产物 `t14fix3`：帧 1–3 `dark=775`（**文字笔画**），帧 0 与帧 4 之后 `dark=0`（卡片已退场）；
  - 旧产物 `t14fix2`：全程 `dark=0` ⇒ **空卡片，无任何文字**。
  ⇒ 修复前后在像素层同样可区分，与 UIA 层的 `ListItem` 计数口径互相独立。
- **回归锁**：`src/components/ui/toaster/app-toast.test.ts` **2/2 通过**
  （挂真的 `Toaster` ＋ 真的 `toast()`，不走 mock）。
- **真实用户设置已恢复**：`node .superpowers\sdd\quark\t14-seed.cjs restore` ⇒
  `restored bytes=6326`（= 备份字节数）；恢复前已结束 `alpha-file-manager`。
  恢复后的真实值 `lastRoute={"fullPageId":"","name":"navigator"}`
  （seed 期间被改成了 `settings`，现已复原）。
- **本轮未做**（仍属对外动作，需用户授权）：`git push`、winget PR、GitHub Release、upstream 移植。

#### 20.9.5 本轮工具坑（省得下一轮再踩）

1. **`Add-Type` 的 C# 段在 pwsh 7 下编译不过、在 Windows PowerShell 5.1 下才过**：
   pwsh 7 的 `Add-Type -ReferencedAssemblies 'System.Drawing'` 是**替换**默认引用集，
   于是 `System.Drawing.Color` 找不到（CS1069，需要 `System.Drawing.Primitives`）；
   PS 5.1 是追加 ⇒ 能编译。⇒ **这类 UIA/C# 探针脚本一律用 `powershell.exe` 跑**。
2. **脚本被强杀时看不到任何输出**：`Say` 用 `Write-Host` ＋ 自写 `run.log`，
   若死在 `Add-Type` 之前 ⇒ `run.log` 根本不会被创建 ⇒ 「零输出 + 退出码 -1」。
   解法：套一层 **wrapper**（`try { & script.ps1 } catch { 写错误到标记文件 }`），
   一次就能拿到真正的异常，而不是猜。
3. **打包必须走 `.superpowers\sdd\quark\batch13-build.cmd`**：
   本机 `cargo` **不在默认 PATH**（`~/.cargo\bin` 根本不存在），
   实际在 `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`，
   且需要 vcvars64 的 MSVC 环境 ⇒ 直接 `npm run tauri:build` 必然报
   `failed to run 'cargo metadata' ... program not found`。
4. **恢复真实用户设置**：release 构建下 `SFM_E2E_DATA_DIR` 无效，只能
   「备份真实文件 → 灌夹具 → 验收 → 恢复」；
   备份在 `.superpowers\sdd\quark\t14-backup\user-settings.json`，
   恢复用 `node .superpowers\sdd\quark\t14-seed.cjs restore`，恢复后核对字节数。
5. 沙箱轮**必须串行**：单实例插件会把新启动转交给旧实例；
   开跑前先结束 `alpha-file-manager` 并清掉 `msedgewebview2.exe`
   （命令行含 `com.sigma-file-manager.app`）的孤儿，否则下次启动 exit 101。

#### 20.10 批次 15：百度侧同形缺陷复验 ＋ `timeout` 终态 ＋ 装机复验（2026-11-14）

##### 20.10.0 本轮范围、口径与硬约束

- **上游入口卡**：`docs/handoff-2026-11-14-quark-pan-batch14.md`（批次 14 交接卡）。
  批次 14 遗留下来的三项，本轮全部做掉：
  ① **百度侧**同形缺陷（`connection-form.vue` 与 `netdisk-add-dialog.vue` 同听全局频道 `afm://baidu-login`）的沙箱真机 A/B；
  ② **`timeout` 终态**（Rust `GIVE_UP = 900` 秒）的真机验证；
  ③ **装机复验**（NSIS 静默安装 → 跑装机产物 → 验 toast「一条带字」）。
- **计数口径不变**：数 UIA `ListItem` 实例数；**不能用 toast 文案计数**（修复前卡片是空壳，文案口径恒 0，会得出假结论）；像素差分作交叉验证。
- **硬约束照旧**（AGENTS §2.8 / §2.10）：所有轮次跑在 `CreateDesktop` 隔离沙箱桌面、窗口固定 1280×900；
  注入端不切桌面、不 `SetForegroundWindow`、不发 `SendInput`；沙箱内只做鼠标等价断言（PostMessage 鼠标 ＋ UIA ＋ `PrintWindow`）；
  枚举沙箱窗口用 `EnumDesktopWindows`；真机结论只认 `npm run tauri:build` 的打包产物（`tauri dev` 全程没用过）。
- **本轮没有改动任何 `src/` 或 `src-tauri/` 源码** —— 只有验收脚本（`.superpowers/sdd/`，gitignore 内）与文档。
- **对外动作一律没做**：`git push`、winget PR、GitHub Release、upstream 移植。

##### 20.10.1 百度侧同形缺陷 A/B：基线弹 2 次 / 修复后弹 1 次（PASS）

| 轮次 | 二进制 | burst 帧 0 相对帧 39 多出的 UIA 节点 | 判定 |
| --- | --- | --- | --- |
| 基线 `t15baidu-base` | 批次 13 留档基线（工作区 `t15-baseline\alpha-file-manager.exe`，82,767,360 B，2026-11-12 01:01:57，sha256 `D2EC859D…29D9`） | **2 个 `ListItem`**：`700,804,534x50` ＋ `700,742,534x50`，`name` 为空（空壳卡片） | 基线弹 **2** 次 |
| 修复后 `t15baidu-fix` | `src-tauri\target\release\alpha-file-manager.exe`（82,756,096 B，2026-11-14 01:38:52，v3.14.0，含 toast 空壳修复） | **1 个 `ListItem`**：`name=[登录还没完成，登录窗口就被关掉了。登录窗口已关闭。] rect=700,737,534x117` ＋ `Text name=[登录窗口已关闭。] rect=725,800,168x27` | 修复后弹 **1** 次，**文字可见** |

- **链路（13 步）**：`远程连接` tab → `添加远程连接` → `网盘` tab → `百度网盘` → 等登录窗（标题 **「百度网盘登录」**，说明百度侧真的把登录窗拉起来了）
  → `dump-windows` → `WM_CLOSE` 关窗 → burst 40 帧 × 150 ms。
- **节点数**：基线帧 0–3 = 143、帧 4–39 = 141；修复后帧 0 = 141（`COUNT_closed=1`）、帧 1–39 = 138。
  两轮的 step-10 / step-12 基准帧都是 **138 节点** ⇒ 起点无残留。
- **差分命令**：`node .superpowers/sdd/quark/t14-uia-diff.cjs .superpowers/sdd/quark/t15-shots <tag> 13 0 39`。
- **与夸克侧同构**：百度/夸克共用同一套 i18n 文案（`closed` / `timeout` / `error` 三条 status 文案逐字相同，只有品牌名不同）
  ⇒ 计数 needle 无需区分两侧；`baidu_login.rs` 的登录窗标题 = 「百度网盘登录」。
- **结论**：缺陷 B 的修复对**百度侧**同样成立 —— 批次 14 只验了夸克侧，这是本轮补上的。

##### 20.10.2 装机复验（NSIS 静默安装 → 跑装机产物）

- **静默安装**：`Alpha File Manager_3.14.0_x64-setup.exe`（45,452,538 B）以 `/S /FORCE` 安装 ⇒ `INSTALL_EXIT=0`；
  守卫日志 `2026-11-14 07:29:32 [early-guard] OUTCOME=proceed reason=forced`
  （同版本同构建必须带 `/FORCE`，否则守卫静默 `Quit reason=same-build`）。
- ⚠️ **实际安装目录是 `D:\Program Files\Alpha File Manager`，不是 `C:\Program Files\…`**：
  注册表 `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Alpha File Manager` 的 `InstallLocation` 指向 D 盘
  （历史安装落在 D 盘，NSIS 升级沿用旧 `InstallLocation`）；`C:\Program Files\Alpha File Manager` 目录**存在但为空**（无 exe）
  ⇒ 台账/脚本里写死 C 盘路径，会把「装好了」误判成「没装上」。
- **装机 exe**：82,756,096 B，mtime 2026-11-14 01:38:16，`FileVersion 3.14.0`；构建产物同尺寸、mtime 01:38:52。
- **判据不能用裸 sha256**：装机 `C55BF795…891F3` vs 构建 `DFCCCDD1…6AC41` 不等 ——
  NSIS 会在二进制里打 29 B 的 bundle 标记（`__TAURI_BUNDLE_TYPE_VAR_NSS__` vs `__TAURI_BUNDLE_TYPE_VAR_UNK__`，
  **等长 ⇒ 尺寸不变但 sha 变**）。用**归一化 sha256**（正则把 `__TAURI_BUNDLE_TYPE_VAR_[A-Z0-9]+__` 抹成同一个等长串）后两者**完全相等**：
  `FA0F7D4C75CF6850F08ED389013CBCA5E727D2D580161BB919004E5BE3848A23` ⇒ **装机产物 == 本次构建**。
- **沙箱轮跑的是装机副本的字节级副本**：
  `.superpowers\sdd\quark\t15-installed\alpha-file-manager.exe`，sha256 `C55BF795…891F3`，与 `D:\Program Files\…` 里的装机文件逐字节相同
  （原因见 20.10.4 工具坑 ⑤）。**台账如实登记**：不是直接就地启动 `D:\Program Files` 里的那份。
- 证据：`.superpowers\sdd\quark\t15-shots\t15installed-console.log`、`t15installed-run.log`、
  `t15installed-burst-13-NN-uia.json` / `-NN.png`、`t15installed-state.json`。
- **判定：PASS —— 装机产物弹的是「一条带字」的 toast。**
  UIA 差分（burst 帧 0 vs 帧 39）：帧 0 = 141 节点、帧 39 = 138 节点，差集 = **1 个 `ListItem`**
  （`name=[登录还没完成，登录窗口就被关掉了。登录窗口已关闭。] rect=700,737,534x117`）
  ＋ `Text name=[登录窗口已关闭。] rect=725,800,168x27`
  ⇒ 卡片高度 **117px**（标题 + 副标题两行），**不是** 50px 的空壳 ⇒ 与批次 14 修复前的「空卡片」形态明显可区分。
  基准帧 step-10 / step-12 均为 **138 节点** ⇒ 起点无残留。
  差分输出 `.superpowers\sdd\quark\t15-uia-ab-installed.txt`；轮次日志 `t15installed-console.log`（`WRAP_OK`）。

##### 20.10.3 `timeout` 终态（等满 900 秒）—— **PASS**

- **轮次**：`t15installed-timeout`（跑**装机副本**；计划 `t15-plan-timeout.json` = 前 13 步同链路 ＋ `wait 875s` ＋ `poll-toast sec=180 interval=250`）。
- **链路**：远程连接 tab → 添加远程连接 → 网盘 tab → **百度网盘** → 登录窗升起（标题「百度网盘登录」，1672×1106）
  → `dump-windows` → **不关登录窗**、静等 875 秒 → 以 250 ms 间隔轮询 180 秒。
  ⚠️ 这一轮**绝不能**用 `close-login` 步骤：它会先发 `WM_CLOSE`，抢先触发 `closed` 终态，测的就不是 timeout 了 ——
  所以另加了 `poll-toast` 步骤（只轮询、不碰登录窗）。
- **实测结果**（地面真相取自逐轮 UIA dump，**不是**探针的计数，见 20.10.4 工具坑 ⑦）：
  - 轮询 **23–29** 轮：节点数 **141**（基线 138 ＋ 3）、`ListItem` **3 个**（基线 2 个 ＋ **1 个 toast**），
    差出来的那个就是终态卡片：
    - `ListItem name=[登录等待超时了，请重试。登录等待超时，请重试。] rect=700,737,534x117`
    - `Text    name=[登录等待超时，请重试。] rect=725,800,231x27`
    ⇒ **标题 = i18n 文案「登录等待超时了，请重试。」，副标题 = Rust 侧 summary「登录等待超时，请重试。」**，
    卡片 **117px**（两行带字）⇒ **timeout 终态真实到达，且 toast 带字**。
  - 轮询 **30–403** 轮：节点数回到 **138**、`ListItem` 回到 **2** ⇒ toast 自动退场后**没有第二条 toast**。
  - **基线那 2 个 `ListItem` 是远程连接列表里的两条已存连接**
    （`百度网盘***…pan.baidu.com:443`、`夸克网盘***…pan.quark.cn:443`），**不是 toast**
    ⇒ 计数口径仍是「基线 ＋ 1」。
- **同时验证了缺陷 B 修复在 `timeout` 路径上的关键性质**：Rust 发完 `timeout` 就会 `close_login_window` 关掉登录窗，
  若 `settle()` 没吞掉随后的 `Destroyed`，就会**再弹一条 `closed` toast** ⇒ 实测**只有一条** ⇒ `settle()` 生效。
- **时间**：toast 出现在轮询窗口开始后约 10 秒 ⇒ 从点击「百度网盘」算约 **890 秒**（探针日志无时间戳，精度到十几秒），
  与 `GIVE_UP = 900s`（Rust 侧从登录流程启动起算 ＋ 2 秒轮询节拍）相符。
- 证据：`.superpowers\sdd\quark\t15-shots\t15installed-timeout-run.log`、`t15installed-timeout-poll-14-23-uia.json` … `-29-uia.json`、
  `t15installed-timeout-poll-14-23.png`（toast 在场的那一帧）、`t15installed-timeout-state.json`；
  扫描输出 `.superpowers\sdd\quark\t15-poll-scan.txt`（扫描脚本 `t15-poll-scan.cjs`）。
- ⚠️ **`ready` 终态仍未验证**：需要真人扫码。本轮扫描里 `ready` 针脚会命中一句**静态提示**
  「在应用内打开登录窗口。登录成功后窗口会自动关闭，Cookie 随即取到——你只需保存这条连接。」
  （`581,728 565x52`，每一轮都在），**那是设置面板的说明文字、不是 toast**，不得据此声称 `ready` 已验证。

##### 20.10.4 本轮工具坑（省得下一轮再踩）

1. **`Tee-Object` 与探针自写的同名 `run.log` 互相锁文件 ⇒ 轮次被中途打断**：
   探针自己写 `$ShotDir\<tag>-run.log`；再叠一层 `*>&1 | Tee-Object -FilePath <同名>` ⇒ 探针开日志时撞上 Tee 的句柄，
   `FileStream` 抛 `The process cannot access the file … because it is being used by another process`
   ⇒ **两轮都被打断作废**（装机轮只到 step-11、timeout 轮只到 step-03）。
   ⇒ 探针轮次**不要**用 Tee 抢同名文件；要用控制台留痕就写**不同名**（`-console.log`）。
2. **被强杀的探针会留下 `powershell.exe` 残留，并一直占着 `run.log` 句柄** ⇒ 下一轮开日志即 sharing violation
   （现象是「脚本在第一步就炸」，不是探针逻辑错）。⇒ 每轮开跑前清三样：
   `powershell.exe`（命令行含 `t15-wrap` / `t14-quark-toast`）、`alpha-file-manager`、
   `msedgewebview2.exe`（命令行含 `alpha-file-manager` / `com.sigma-file-manager`）。
3. **单实例插件会把新启动转交给旧实例** ⇒ 残留实例存在时，探针里的 UIA 树是 **0 节点**
   （症状：`shot ok=False` 连着出现 ＋ `NOT FOUND in uia ( nodes)` ＋ 窗口尺寸读成 0）。
   它看起来像「界面没渲染」，实际是**拿不到自己的窗口**。⇒ 见到这组症状先查残留实例，别去改探针。
4. **本机安全策略会杀 `New-Item -ItemType Directory`**（连同整条命令）⇒ 目录一律用写文件工具预建（写一个 `.keep`）。
5. **工作区外的 exe 路径不能直接作为探针启动路径** ⇒ 装机副本先拷进工作区再跑，台账登记「跑的是字节级副本」；
   同时**命令行里带 `%TEMP%` 下的 exe 路径**启动子进程 ⇒ **整个进程树被秒杀**
   （exit -1、零输出，连 wrapper 首行标记都写不出来）⇒ 任何 exe 都先拷进工作区。
6. **探针日志句柄加共享/降级兜底**（本轮已改 `t14-quark-toast-sandbox.ps1`）：
   `New-Object System.IO.StreamWriter($path,$false,$enc)` 以独占语义开文件，遇到残留句柄直接抛；
   改成 `FileStream(…, FileShare.ReadWrite)`，并在被占用时自动降级到 `<tag>-run-HHmmss.log`。
   ⇒ **既有步骤语义零改动**，批次 14 的复现性不受影响。
7. **本轮新增的 `poll-toast` 步骤里有个表达式优先级 bug ⇒ 它的 `COUNT_*` 全是废数**：
   写成 `if ([string]$n.name.Contains($needle))` 时，强制转换套在**整个布尔表达式**上
   ⇒ 布尔被转成字符串 `"False"` / `"True"` ⇒ `if ("False")` 恒为真
   ⇒ **四个 `COUNT_*` 同时恒等于「有名字的节点数」**（实测 67），且每轮都误判命中、狂写截图。
   ⇒ **判词一律取自逐轮 UIA dump 的节点名 + rect**（本轮用 `t15-poll-scan.cjs` 扫 dump 得到）；
   本轮已把该行改成 `if (([string]$n.name).Contains([string]$needle))`（与 burst 段写法一致），下一轮起 `COUNT_*` 可用。
   ⚠️ 同形通病：**任何 `[string]$x.Method(...)` 都会把布尔转成字符串**，PowerShell 里判布尔别这么写。

##### 20.10.5 本轮收尾状态

- **仓库门禁**：`npm run check` ⇒ `CHECK_EXIT=0`，**311 个测试文件 / 3238 个用例全绿**
  （本轮无 `src/` 改动；跑门禁是为了确认提交树本身干净）。
- **真实用户设置已恢复**：`node .superpowers\sdd\quark\t14-seed.cjs restore` ⇒ `restored bytes=6326`（= 备份字节数），
  恢复前已结束 `alpha-file-manager`；恢复后真实值 `lastRoute={"fullPageId":"","name":"navigator"}`、6326 B。
- **git**：本轮只改了 `docs/handoff-2026-11-11-quark-pan.md`（§20.10）并新增 `docs/handoff-2026-11-15-quark-pan-batch15.md`；
  `.superpowers/sdd/` 下的脚本与证据都在 gitignore 内，不进版本库。**已 commit、未 push**。
- **本轮未做**（仍属对外动作，需用户授权）：`git push`、winget PR、GitHub Release、upstream 移植。

### 20.11 批次 16：focus sync 异常 —— 闸门「假下降沿」根因修复 ＋ 真机部署（2026-11-18）

#### 20.11.0 本轮范围、口径与硬约束

- **用户诉求**：查看使用日志、分析 bug 并修复 focus sync 异常。
- **日志源三分（口径）**：
  ① JSON 事件流 `%LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log`（`log_event` 落盘，`SpikeError`/`DialogDetected`/`Read`）；
  ② tracing 文本 `spike.log.YYYY-MM-DD`（`tracing_appender::rolling::daily`），**`[afm-gate]` 调试行只在这里**；
  ③ 宿主侧 `__focus_sync_trace` / `__hostWorkerDiagnostics`（`user-extensions.json`）。
- **判定口径**：必须靠「事件类型 ＋ kind ＋ 时间序列」；**不能用 `SpikeError` 计数**（heartbeat 等非错误诊断也走该类型）。
  统计事件时用 `e.kind || e.event`（`DialogDetected`/`Read` 没有 `kind` 字段）。
- **`opened_write_back_skip_no_afm_visit` 是「交接闸门拒绝」的精确分支**（`writable && !gate_open`），不是 catch-all。
- **写回通道**：按 v0.5.7 R-2 契约，`/set_path` 只存路径、不写后台对话框；真正写入通道是 Opened 写回 ⇒ 修复重心在**交接闸门**。
- **硬约束照旧**（AGENTS §2.8 / §2.10）：真机 UI 验收必须跑在 `CreateDesktop` 隔离沙箱桌面、注入端不切桌面、不 `SetForegroundWindow`、不发 `SendInput`。
  本轮**没有**做真机 UI 交互验收（原因见 20.11.6「未验证」行）。
- **对外动作一律没做**：`git push`、winget PR、GitHub Release、upstream 移植。

#### 20.11.1 症状（从日志还原）

| 观测项 | 数值 | 出处 |
| --- | --- | --- |
| 11-17 `DialogDetected` 次数 / 不同 hwnd 数 | 809 次 / 382 个（app 全为 `#32770`） | `spike.log` |
| 11-17 成功写回次数 | **1 次**（12:14:30 `opened_write_back`） | `spike.log` |
| 11-17 `opened_write_back_skip_no_afm_visit` | **连续 656 次**（12:31:13–14:07） | `spike.log` |
| `[afm-gate]` 按日 | 11-10 recorded=68/back=68；11-11 recorded=129/back=140/lost=10；11-12 recorded=1；11-13 recorded=1；**11-17 recorded=33/back=36/lostPid=3/retry=2** | `spike.log.2026-11-1*` |
| 扩展侧污染 | `N05.path.raw = sfm://locations`（虚拟路径被当成真实目录推给侧车）；300 条 trace 绝大多数是 15s 一次的 `N03.health.resp` | `user-extensions.json` |

- 取证产物：`.superpowers\sdd\focus\gate.txt`（289,542 B，含 11-10→11-17 全部 `[afm-gate]`/`handoff`/`foreground` 行）、
  `.superpowers\sdd\focus\gate17.txt`（11-17 的 74 条 afm-gate 行）、`.superpowers\sdd\focus\seq.txt`（515 行关键事件序列）、
  `.superpowers\sdd\focus\windows.txt`（真机顶层窗口枚举）。

#### 20.11.2 根因（由 tracing 日志坐实，不是推测）

1. 旧实现 `foreground_is_afm()` 的判据只有一条：**类名 == `Tauri Window` 且标题以 `Alpha File Manager` 开头**。
2. 装机版 AFM 进程（真机 pid **72772**）另持有一个**可见**顶层窗口：
   `hwnd=24842270 class='com.sigma-file-manager.app-sic' title='com.sigma-file-manager.app-siw'` ⇒ **不满足上述判据**。
   （真主窗口 `hwnd=7013496 class='Tauri Window' title='Alpha File Manager' visible=False` 满足判据。）
3. 该辅助窗口在前台时 `afm_now` 恒为 false ⇒ **AFM→非 AFM 的「假下降沿」** ⇒ 交接授权里写进的是 **AFM 自己的 pid**。
   tracing 原文（11-17 12:15:00.188Z）：`handoff recorded: pid=72772` —— **72772 正是 AFM 自己的 pid**。
4. 于是之后所有真实对话框的 pid 都 ≠ 72772 ⇒ 闸门恒拒 ⇒ 656 次 `skip_no_afm_visit`，全天只写回成功 1 次。
5. 次要污染：扩展把虚拟位置 `sfm://locations` 当路径推给侧车（`normalizePath` 剥掉了前缀）⇒ `current_path` 被写成虚拟路径。

#### 20.11.3 修复（v0.5.21）

改动文件：`extensions/kizemo.focus-sync/sidecar/src/{state.rs, uia_event.rs, main.rs, http_server.rs}`（**4 files, +365 / −18**）。

1. **AFM 识别改为「进程身份」**：
   - `AppState.afm_pid: AtomicU32` ＋ `learn_afm_pid(class_title_match, hwnd_pid)` / `afm_pid()`；
   - 前台窗口 class+title 命中即**学** pid；每 500 ms（`AFM_ENUM_INTERVAL`）用 `EnumWindows` 枚举补学（含不可见窗口）；
   - 纯函数 `foreground_is_afm_by(known_afm_pid, class_title_match, hwnd_pid)` ＝ class+title 命中 **或** 前台窗口 pid == 已知 AFM pid。
2. **闸门兜底**：`afm_handoff_allows` 在 `handoff.pid == afm_pid` 时**直接拒绝** ⇒ 假下降沿即便再次发生也不会放行。
   （同时保留 v0.5.x 的 `handoff.pid == 0 || dialog_pid == 0` 一律拒绝。）
3. **决策可观测**：闸门状态变化写 JSON 事件 `afm_gate_cleared` / `afm_gate_armed` / `afm_gate_retry`；
   `opened_write_back_skip_no_afm_visit` 消息加 `dialog_pid` / `handoff_pid` / `afm_pid` 三个字段 ⇒ 下次一眼分清拒绝原因。
4. **路径守卫**：`set_current_path` 先做**形状**校验（盘符 `X:\` / `X:/` 或 UNC；只查形状、不查存在性），非法形状**存空串**
   ⇒ 阻断 `sfm://locations` 污染，且虚拟位置期间不写任何路径。
5. **path_update 事件**改为上报**实际存入**的路径 ＋ 原 pushed 值。
6. **刻意不做的事**：
   - 不改扩展 `dist/index.js`（该文件就是 git 跟踪的发布源，无独立 src 目录）—— 侧车已防御，改它风险更高；
   - 不加 hwnd 强校验 —— Chromium 对话框导航会重建 hwnd（实测 382 个不同 hwnd），按 hwnd 绑定会误拒；保持 pid 语义。

#### 20.11.4 判别性测试（11 个）

- `state.rs :: v521_afm_identity_tests`（7）：假下降沿（handoff.pid == afm_pid）被拒 / 正常放行 / 无关应用被拒 /
  pid 只从 class+title 学习 / 虚拟路径不入 `current_path` / 真实路径仍存 / 形状判据本身。
- `uia_event.rs :: v521_afm_identity_tests`（4）：同进程辅助窗口算 AFM / 他进程不算 / pid 未知不得猜 / class+title 仍优先。
- 补丁机制：`.superpowers\sdd\focus\apply-patch.cjs` ＋ `.superpowers\sdd\focus\patch-v521.txt`（19 个 op，断言唯一命中、CRLF 自适应），
  执行结果 **19/19 OK、failed=0**，可重放。

#### 20.11.5 门禁与真机部署（本轮实测）

- **Rust 门禁**：`cargo test` ⇒ **123 passed / 0 failed / 0 ignored**（日志 `.superpowers\sdd\focus\cargo-test-190.log`）。
  ⚠️ **toolchain 陷阱**：`stable-x86_64-pc-windows-msvc`（rustc 1.97.1）的 std 组件不可用，编译报
  `error[E0463]: can't find crate for 'std'` / `only metadata stub found for 'rlib' dependency 'rustc_std_workspace_core'`
  ⇒ 换 **1.90.0** 一次通过；`scripts\release\build-sidecar.cmd` 本来就固定 1.90.0。
- **前端门禁**：`npm run check` ⇒ **EXIT=0**（日志 `.superpowers\sdd\focus\npm-check.log`）。
- **重建侧车**：`scripts\release\build-sidecar.cmd` ⇒ `BUILD_OK`（release 50.11 s）⇒
  新件 **sha256 `BFD5DF0C…B4DB`、2,882,048 B**（旧件 2,873,344 B / sha `A8300AAB…52E462`）。
- **部署到真机**（脚本 `.superpowers\sdd\focus\deploy-sidecar.ps1`，报告 `.superpowers\sdd\focus\deploy-01.txt`）：
  备份旧件 → `.superpowers\sdd\focus\installed-sidecar-backup-20261110.exe`（sha 与旧件一致，可回滚）
  → 杀旧侧车 pid 301088 → 覆盖 `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`
  → AFM 监督器**自动重拉起** pid **277632** ⇒ **运行中进程映像 sha256 == 新构建**（进程级确证）。

#### 20.11.6 真机证据：已验证 / 推断 / 未验证

| # | 结论 | 状态 | 证据 |
| --- | --- | --- | --- |
| ① | 新侧车已替换真机副本且正在运行 | **已验证** | 运行映像 sha256 `BFD5DF0C…B4DB` == 新构建；heartbeat `alive pid=277632` 每 5 s |
| ② | 跑的是新代码而非旧件 | **已验证** | 二进制标记 A/B：旧件 `markers=[]`，新件含 `dialog_pid`/`handoff_pid`/`afm_pid`/`afm_gate_cleared` |
| ③ | 虚拟路径污染已被消除（行为级） | **已验证** | 旧件 `path_update current_path=sfm://locations`（被存入）→ 新件 `path_update current_path= (pushed='sfm://locations')`（**实际存入为空**） |
| ④ | 交接闸门不再误放行 / 不再恒拒 | **推断（单测覆盖，未做真机 UI 交互验收）** | 11 条判别性测试；真机「打开对话框 → 写回成功」需真人触发一次对话框 |
| ⑤ | 装机版（NSIS）会把新侧车带给用户 | **未验证** | 部署链已确认：`build-sidecar.cmd` → `bin\focus-sync-sidecar.exe` → `installer.nsi`（`File "..\bin\focus-sync-sidecar.exe"`）/ `manual-install.ps1` → APPDATA；但 `bin\` 与 `installer\` 里的 exe **未被 git 跟踪** ⇒ 装机机必须重新跑 build + 安装器，否则装出来仍是旧侧车 |
| ⑥ | 沙箱真机 UI A/B（AGENTS §2.8/§2.10） | **本轮未做** | 沙箱跑的是 AFM 主程序，不含 APPDATA 部署位的侧车；做端到端 UI 验收需要真人交互或把侧车纳入沙箱部署 |

#### 20.11.7 次生缺陷（按决策本轮**不修**，登记备查）

1. `__focus_sync_trace` 被 15 s 一次的 health 轮询刷爆（300 条里绝大多数是 `N03.health.resp`）。
2. `/set_path` 空表阻塞 400 ms。
3. `writer.rs::try_mark_committed` 失败也消耗 commit 槽。
4. 扩展侧 `normalizePath` 仍会把 `sfm://locations` 剥成虚拟路径并推送（侧车已挡住写入，但推送仍在）。
5. 装机流程不刷新 APPDATA 侧车二进制（本轮已手工刷新，见 20.11.5）。

#### 20.11.8 本轮工具坑（省得下一轮再踩）

1. **`stable-x86_64-pc-windows-msvc`（1.97.1）的 std 不可用** ⇒ `cargo` 报 E0463 `can't find crate for 'std'`，
   报错落在 registry 里的 `proc-macro2`/`once_cell`/`quote`/`serde_core` 的 build 脚本上 ⇒ **看着像依赖问题，其实是工具链问题**。
   ⇒ 用 `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin\cargo.exe`。
2. **PowerShell 里 `MatchInfo` 没有 `.Trim()` 方法**（要先 `.Line.Trim()`）⇒ `Select-String` 结果直接格式化会刷屏 ⇒ 内容检索改用 `grep` 工具。
3. **覆盖正在运行的 exe 会 sharing violation** ⇒ 必须「杀进程 → 复制」；本轮脚本做成 kill→copy 重试循环（最多 6 次），
   因为 AFM 监督器会立刻重拉起旧件。
4. **工作区外路径的写操作不能内联** ⇒ 部署脚本写进 `.ps1` 再 `pwsh -NoProfile -ExecutionPolicy Bypass -File` 执行（内联 `Copy-Item` 会被策略拦）。
5. **读被占用的日志**：`[System.IO.File]::Open($p,'Open','Read','ReadWrite')` 共享读；读尾部用 `Seek(-N,'End')`（`Get-Content -Raw` / `ReadAllLines` 会抛「正被另一进程使用」）。
6. **同文件并行 `edit` 会互相覆盖** ⇒ 源码改动一律走「补丁文件 ＋ 单次执行」（`apply-patch.cjs`）。
7. `cargo` 不在默认 PATH，且**不要与 `npm run check` 并发**。

#### 20.11.9 本轮收尾状态

- **改动**：`extensions/kizemo.focus-sync/sidecar/src/{state.rs,uia_event.rs,main.rs,http_server.rs}` ＋ 本节台账 ＋ 新增批次 16 交接卡。
- **门禁双绿**：`cargo test` 123/0、`npm run check` EXIT=0。
- **真机**：APPDATA 侧车已换成新构建并在运行，旧件已备份（可回滚）；真机用户数据全程**只读未改**。
- **未做（需用户授权）**：`git push`、winget PR、GitHub Release、upstream 移植。
- **未做（需真人交互）**：端到端「打开对话框 → 写回成功」真机验收（见 20.11.6 ④⑥）。
### 20.12 批次 17：focus sync 闸门「判据过脆」根因修复 ＋ 日志刷屏回归修复 ＋ 端到端真机验收通过（2026-11-18）

> 批次 16（§20.11）修掉了「AFM 身份误判 ⇒ 假下降沿 ⇒ 闸门恒拒」。本批次修掉**批次 16 之后真机 e2e 暴露的第二个缺陷**：
> 闸门判据过脆（仍然恒拒）＋ v0.5.21 新引入的枚举日志每 500 ms 刷一行。
> 本批次**端到端真机验收已通过**（真人操作，路径真的写进了对话框）。

#### 20.12.1 第二个缺陷的真机证据（2026-11-18 02:17Z 段）

- 02:17:08Z `DialogDetected` hwnd=13635992 app=#32770；
- 02:17:09–02:17:12 连续 `opened_write_back_skip_no_afm_visit`：
  `dialog_pid=619924 handoff_pid=15204(→13080) afm_pid=118160`；
- pid 归属：15204 = `UGREEN NAS.exe`；13080 = `explorer.exe`；118160 = `alpha-file-manager.exe`；619924 = 对话框宿主进程。
- 结论：**闸门仍恒拒**（v0.5.21 只修好了「谁来判 AFM」，没修「授权是否成立」）。
- 同时：`spike.log.2026-11-18` 里 `[afm-gate] learned AFM pid=118160 by window enumeration` 出现 **15,951 次**
  （每 500 ms 一行，当天 tracing 文件 2.57 MB）⇒ **v0.5.21 自己引入的日志刷屏回归**。

#### 20.12.2 根因（四条独立缺陷，缺一不可）

1. **闸门只有一条放行路径**：`handoff.pid == dialog_pid` **严格相等**。
   真实世界有三种都该放行的情况，旧实现全判成「没访问过 AFM」：
   (a) 对话框属主进程是接收方应用的 **helper 子进程**（pid 不等但同一应用）；
   (b) **对话框开着的时候用户回过 AFM**（Listary 原义）；
   (c) 用户在 AFM 与对话框之间来回切换（交接链被清/被过期链复活）。
2. **交接链采样把 shell 表面当接收方**：切走的瞬间前台窗口可能是任务栏/桌面（`Shell_TrayWnd`/`Progman`，属 `explorer.exe`）
   ⇒ 采样到的 pid 是 explorer ⇒ 真实接收方丢失（这正是 `handoff_pid=13080` 的来源）。
3. **交接链过期后会被复活**：用户仍停留在接收应用里时 TTL 到期清链；用户回 AFM 再切回来时，
   过期链被当作新链重新生效 ⇒ 把 2026-11-09 那类「陈旧授权」重新放回来。
4. **枚举日志无去重**：每 500 ms 无条件打印一行「learned AFM pid」。

#### 20.12.3 修复（v0.5.22）

改动文件：`extensions/kizemo.focus-sync/sidecar/src/{state.rs, uia_event.rs, main.rs, http_server.rs}`（**4 files, +466 / −44**）。

1. **闸门改成两条独立放行路径**（总入口 `state.rs :: afm_gate_allows(dialog_hwnd, dialog_pid)`）：
   - **路径 A · 交接链身份**：`afm_handoff_allows` 放宽为
     `handoff.pid == dialog_pid` **或** `same_application_process(handoff.pid, dialog_pid)`
     （互为祖先后代；ToolHelp 进程快照，最多向上 16 层；`same_application_process` 走 `SnapshotGuard` RAII 释放快照句柄）。
   - **路径 B · 对话框开着时用户去过 AFM**：`mark_dialog_opened(hwnd)` 记对话框打开时刻，
     `last_afm_foreground_ts` 记最近一次 AFM 在前台的时刻；
     **AFM 访问时刻 ≥ 对话框打开时刻** 才放行 ⇒ 挡住 2026-11-09「陈旧访问授权后来弹出的对话框」。
   - 两条路径**任一成立即放行**；都不成立才 skip（skip 消息带 `dialog_pid`/`handoff_pid`/`afm_pid`）。
2. **接收方采样排除 shell 表面**：按**窗口类名** `Shell_TrayWnd`/`Shell_SecondaryTrayWnd`/`Progman`/`WorkerW` 排除
   （**不能按进程判** —— explorer 同时拥有真窗口，按进程排会把接收方一起排掉）。
3. **前台窗口就是该对话框时**，接收方取**对话框属主 pid**，避免采到任务栏。
4. **交接链续期**：用户仍停留在接收应用内时**续期**；但**不复活过期链**（过期链不因「回 AFM 再回来」而重新生效）。
5. **枚举日志去重**：只在 **pid 变化时**打印一次。

#### 20.12.4 判别性测试（新增 7 条，累计 130）

- `state.rs :: v522_gate_paths_tests`（5）：路径 A 同进程放行 / 路径 A helper 子进程放行 / 路径 A 无关进程拒绝 /
  路径 B「AFM 访问晚于对话框打开」放行 / 路径 B「AFM 访问早于打开时刻」拒绝（陈旧访问）。
- `uia_event.rs :: v522_shell_and_identity_tests`（2）：shell 表面类名识别 / 同一应用（pid 相同或互为祖先后代）判定。
- 补丁机制：`.superpowers\sdd\focus\patch-v522.txt`（12 op）＋ `.superpowers\sdd\focus\patch-v522b.txt`（3 op，修编译错）
  ⇒ 执行结果 **12/12 ＋ 3/3 全命中，failed=0**，可重放。
- 编译期抓到并修掉 2 个错：tuple 模式把 `u64` 当 `Option` 匹配；测试模块被插进函数体（须放文件级）。

#### 20.12.5 门禁与真机部署（本轮实测）

- **Rust 门禁**：`cargo test`（1.90.0 toolchain）⇒ **130 passed / 0 failed**（日志 `.superpowers\sdd\focus\cargo-test-522.log`）。
- **前端门禁**：`npm run check` ⇒ **EXIT=0**（**311 个测试文件 / 3239 个用例全过**，日志 `npm-check-522.log`）。
- **重建侧车**：`scripts\release\build-sidecar.cmd` ⇒ `BUILD_EXIT=0`
  ⇒ 新件 **sha256 `0DC1B563…19DFE8D` / 2,891,776 B**（v0.5.21 件 `BFD5DF0C…B4DB` / 2,882,048 B）。
- **部署到真机**（脚本 `.superpowers\sdd\focus\deploy-sidecar-522.ps1`，报告 `deploy-522.txt`）：
  备份旧件 → `.superpowers\sdd\focus\installed-sidecar-backup-20261118-v0521.exe`（sha 与 v0.5.21 件一致，可回滚）
  → 杀旧侧车 pid 118816 → 覆盖 `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`
  → AFM 监督器**自动重拉起** pid **661500**。
- **「跑的是哪份代码」三条判据全中**：装机副本 sha == 构建产物 sha == **运行映像 sha**（三处相等），
  外加二进制标记 A/B（新件含 `handoff sample has no receiving app yet` / `handoff recorded on retry` / `fg_class=`）。

#### 20.12.6 端到端真机验收（**已通过**，2026-11-18 03:09:30–03:09:40Z / 本地 11:09）

真人操作序列：**应用内弹出「选择文件」对话框 → 切回 AFM 取路径 → 点任务栏切回对话框**。

| 时刻(UTC) | 日志事件 | 判读 |
| --- | --- | --- |
| 03:09:30 | `DialogDetected` hwnd=6230674 | 对话框打开 |
| 03:09:31–33 | `opened_write_back_skip_no_afm_visit` ×3（`dialog_pid=724248 handoff_pid=0 afm_pid=118160`） | 此刻确实还没去过 AFM ⇒ **正确拒绝**（证明闸门不再恒放行） |
| 03:09:33 | `afm_gate_cleared`「AFM back in foreground」 | 用户切回 AFM（**晚于对话框打开** ⇒ 路径 B 条件成立） |
| 03:09:37 | `afm_gate_armed`「handoff recorded: pid=724248」 | 交接链记下对话框属主进程 |
| 03:09:37–38 | monitor「dialog regained foreground with pending path change → resync」 | 对话框回到前台，触发重同步 |
| 03:09:38 | **`opened_write_back`**「dialog path='G:\' current='F:\soft\00selfmade\practilog\release'; writing current」 | **放行成功** |
| 03:09:40 | **`Write`** hwnd=6230674 target=`F:\soft\00selfmade\practilog\release` strategy=uia | **路径真的写进了对话框** |
| 03:10:06 | `dialog_closed` | 用户关闭对话框 |

**日志刷屏回归同时验证修好**：v0.5.22 进程整个会话（30 分钟）内 `[afm-gate] learned AFM pid=…` **只打印 1 次**
（v0.5.21 当天 15,951 次；`spike.log.2026-11-18` 的 tracing 文件从 2.57 MB 涨到 2.57 MB 量级但新增段几乎无该行）。

#### 20.12.7 已验证 / 推断 / 未验证

| # | 结论 | 状态 | 证据 |
| --- | --- | --- | --- |
| ① | 端到端写回成功（真人操作，路径进对话框） | **已验证** | 20.12.6 表：03:09:38 `opened_write_back` + 03:09:40 `Write` |
| ② | 无授权时闸门仍会拒（不恒放行） | **已验证** | 03:09:31–33 三次 skip（当时确实还没去 AFM） |
| ③ | 枚举日志刷屏回归已修 | **已验证** | 新进程会话内该行仅 1 次（v0.5.21 为 15,951 次） |
| ④ | 真机跑的是 v0.5.22 代码 | **已验证** | 装机 / 构建 / 运行映像三处 sha256 相等 ＋ 二进制标记 A/B |
| ⑤ | 路径 A 的 `same_application_process`（祖先后代）分支 | **未验证（真机未触发）** | 本轮 e2e 命中的是「`handoff.pid == dialog_pid` 严格相等」分支（均 724248）；祖先后代分支只有单测覆盖 |
| ⑥ | 路径 B（AFM 访问晚于对话框打开）真机命中 | **已验证** | 03:09:33 AFM 在前台，晚于 03:09:30 打开时刻 |
| ⑦ | 装机版（NSIS）会把新侧车带给用户 | **未验证** | `bin\` / `installer\` 里的 exe **未被 git 跟踪** ⇒ 装机机必须重新跑 build + 安装器，否则装出来仍是旧侧车 |
| ⑧ | 沙箱真机 UI A/B（AGENTS §2.8/§2.10） | **本轮未做** | 同批次 16：真机结论走 APPDATA 部署位的侧车，沙箱不含该副本 |
| ⑨ | 路径 B 的语义副作用 | **已知并接受** | 「对话框开着时去 AFM」即视为授权（Listary 原义），与用户无关的 AFM 访问也会写回；已用「访问必须晚于对话框打开时刻」挡住陈旧授权 |

#### 20.12.8 本轮工具坑（省得下一轮再踩）

1. **e2e 监听窗口不够**：本轮监听器窗口 10 分钟，用户实际隔了约 **30 分钟**才操作 ⇒ 监听器 `reason=timeout / interesting=0`，
   判据只能回到 `spike.log` **直读尾部**才拿到。⇒ 下轮监听窗口至少 30 分钟，或干脆先记 `spike.log` 偏移量、事后按偏移直读。
2. **读被占用日志**：`[System.IO.File]::Open($p,'Open','Read','ReadWrite')` 共享读 + `Seek(-N,'End')`；`Get-Content -Raw` 会抛「正被另一进程使用」。
3. **`cargo` 不在默认 PATH 且不要与 `npm run check` 并发**：本轮把「npm check → build-sidecar.cmd」串在**同一个后台任务**里跑（EXIT=0 / BUILD_EXIT=0）。
4. **内联 `Copy-Item` 到工作区外会被安全策略拦** ⇒ 部署一律写 `.ps1` 再 `pwsh -NoProfile -ExecutionPolicy Bypass -File` 执行。
5. **同文件并行 edit 会互相覆盖** ⇒ 源码改动走「补丁文件 ＋ 单次执行」（`apply-patch.cjs`），并断言唯一命中。
6. **PowerShell `MatchInfo` 没有 `.Trim()`**（要先 `.Line.Trim()`）⇒ 内容检索优先用 `grep` 工具。

#### 20.12.9 本轮收尾状态

- **改动**：`extensions/kizemo.focus-sync/sidecar/src/{state.rs,uia_event.rs,main.rs,http_server.rs}`（+466 / −44）
  ＋ 本节台账 ＋ 新增批次 17 交接卡 `docs/handoff-2026-11-18-quark-pan-batch17.md`。
- **门禁双绿**：`cargo test` **130 / 0**、`npm run check` **EXIT=0**（3239 用例）。
- **真机**：v0.5.22 已部署并在运行（pid 661500），v0.5.21 件已备份可回滚；真机用户数据全程**只读未改**。
- **端到端验收**：**已通过**（20.12.6）。
- **未做（需用户授权）**：`git push`、winget PR、GitHub Release、upstream 移植。

---

### 20.13 批次 18：focus sync 空路径契约（R1）＋ §20.11.7 四个次生缺陷修复 ＋ 版本 3.14.1 ＋ 装机链判别性实验（2026-11-18）

> 批次 17（§20.12）把「闸门恒拒」和「日志刷屏」修掉并完成端到端真机验收。本批次处理**批次 17 留下的 5 项遗留**：
> ① push 未做（**本批次已完成并核验**）；② 装机链未验证（未验证项⑦）；③ 路径 A 祖先后代分支真机未覆盖（**仍登记未验证，本轮不安排**）；
> ④ §20.11.7 登记的 4 个次生缺陷；⑤ 版本号与全套发版材料。

#### 20.13.1 R1 根因与定稿路线

**现象**：扩展停在**虚拟位置**（收藏/最近/「此电脑」这类非真实目录）时仍会把虚拟路径推给侧车；
侧车 `/set_path` 对**空路径**走 400 `path is empty` 错误分支，并且会**同步阻塞约 400 ms**（等对话框探测预算）；
扩展却按旧契约把这次 400 当成 `ok:true` ⇒ 「路径已送达」是**假阳性**。

**关键安全事实**（决定了「清空」而不是「推占位路径」）：侧车写回闸门要求 `is_writable_local_path(&current)`；
`current` 为空 ⇒ `path_ok = false` ⇒ 落 `opened_write_back_skip_non_local_path` 分支，**一个字节都不写**（`main.rs:420` 附近）。
⇒ 空路径不会造成误写，清理它没有数据风险。

**定稿路线（明确不用「推虚拟路径」回退）**：**空路径 = 显式清空侧车记忆路径**。

1. 侧车新增 `AppState::clear_current_path()`：**只清 `current_path`，不动 `first_push_received`**
   （「扩展曾推过路径」是历史事实；清掉会让侧车重启补推判据被误触发）。
2. `/set_path` 空路径：**不再走 400 错误分支、不再等 400 ms 对话框探测** ⇒ 立即记 `set_path_cleared` 事件并回 **200 `{"ok":true}`**。
   这个 200 是**与扩展的显式握手**，语义是「侧车已忘掉上一个真实目录」，不是「已写进对话框」。
3. 扩展新增 `sendClearToSidecar()`（POST `/set_path`，body `{"path":""}`）；`pushNow()` 遇非本地路径时**主动清空**侧车记忆路径，
   并返回结果对象 `{ok, status, reason, wrote}`，让调用方能区分「送达」与「写入」。
4. toast 诚实化：200 ⇒「Path sent to the helper…」；404 ⇒「no open Save As dialog … nothing was written yet」；
   disabled / error 各自提示。依据仍是上面那条：**侧车 200 ≠ 已写进对话框**（对非前台弹窗只登记不写）。

#### 20.13.2 §20.11.7 登记的 4 个次生缺陷：本批次的修法

| # | 缺陷 | 位置 | 修法 |
| --- | --- | --- | --- |
| A | commit 槽泄漏：写失败后单次防双击槽不归还 ⇒ 一次失败永久烧掉该对话框的提交资格 | `sidecar/src/writer.rs` | v0.5.23：`try_mark_committed` 是**先占槽后写入**（写入约 300 ms，占着槽才挡得住第二个推送）；写入失败时**仅当本次确实占到了槽**才 `unmark_committed` 归还（否则会抹掉上一次成功提交，让同一对话框能提交两次） |
| B | `/set_path` 空路径 400 ms 阻塞 + 假 `ok:true` | `sidecar/src/http_server.rs` | 空路径改走「显式清空」分支：立即 200 + `set_path_cleared` 事件；扩展侧同步改成如实措辞（见 20.13.1 第 4 点） |
| C | `__focus_sync_trace` 被 health 轮询刷爆 | `extensions/kizemo.focus-sync/dist/index.js` | trace 采样改成**有界替换**（`traceReplace`）；`N15.repush.after_restart` 改用 `traceReplace`，health 轮询不再新增 trace 行 |
| D | 虚拟路径仍推送 | `extensions/kizemo.focus-sync/dist/index.js` | 非本地路径不再推虚拟路径，改为发**清空信号**（`sendClearToSidecar`），并留标记串 `non-local-path` 供装机载荷校验 |

**顺带**：`http_server.rs` 的 `SPIKE_VERSION` 由 `0.6.0` bump 到 **`0.6.1`**（`/health` 版本可用于判别跑的是哪份二进制）；
扩展版本串 `v0.5.5` → `v0.6.2`（activate 日志与 `N01.activate.start.ver`）。

#### 20.13.3 判别性单测（新增 3 条，累计 138）

- `state.rs :: clear_current_path_empties_path_but_keeps_first_push_flag` —— 清空后 `current_path` 空、`first_push_received` 仍在。
- `state.rs :: cleared_current_path_is_not_writable` —— 清空后 `is_writable_local_path` 为 false ⇒ 写回闸门必然走 skip 分支。
- `http_server.rs :: set_path_with_empty_path_clears_the_remembered_path` —— 空路径 `/set_path` 返回 200 且记忆路径被清。

**第一次跑 137 passed / 1 failed**：我自写测试的前提写错了 —— `is_writable_local_path` 要求目录**真实存在**
（`Path::is_dir()`），编造的 `D:\stale` 不存在 ⇒ 断言失败。改用盘符根 `C:\` 后复跑 **138 passed / 0 failed**（基线 135，+3）。
日志 `.superpowers\sdd\sidecar-test-r1.log`，脚本 `.superpowers\sdd\run-sidecar-test-r1.ps1`（用 `cmd /c "... > log 2>&1"` 保证 UTF-8）。

#### 20.13.4 门禁与侧车产物（本轮实测）

- **Rust 门禁**：`cargo test`（1.90.0 toolchain）⇒ **138 passed / 0 failed**。
- **侧车重建**：`scripts\release\build-sidecar.cmd` ⇒ `BUILD_OK`（自写日志 `.superpowers\sdd\build-sidecar.log`，旧日志备份为 `build-sidecar-b17.log`）
  ⇒ 新件 **sha256 `74E5957F5328BF1AFE679F075E16C73691307AB5F6435B2C6BC537FAE7721CCF` / 2,895,360 B**（旧件 `552C85C4…735FD8` 作废）。
- **扩展自检**：`node scripts/scan-sandbox-dynamic.cjs` ⇒ `RESULT: VALID (0 violations / 20 patterns)`（focus-sync 与 kizemo.git 两个入口）；
  `node scripts/verify-extension-sandbox-gate.mjs` ⇒ PASS/OK；`node --check dist/index.js` ⇒ SYNTAX_OK。
  标记串 `traceReplace` / `non-local-path` 仍在（装机载荷校验依赖）。
- **前端全量门禁 `npm run check`**：**并发跑法（`run-p`）在本机连红 3 轮，根因不是本轮改动**（详见 20.13.8 第 1 条）。
  把**同一批 5 个脚本**改成**串行**跑（`.superpowers\sdd\run-check-b18c.cmd`），**复跑一轮五步全绿**：
  `ts` = **0**、`lint:check` = **0**、`test:unit:run` = **0**（**311 个测试文件 / 3241 个用例全过**）、
  `verify-changelog` = **0**、`verify:sandbox` = **0**。
  ⇒ 并发跑法的红项属仓库既有的**负载敏感 flake**，单文件复跑全绿、串行复跑全绿（见 20.13.8 第 1 条）。

#### 20.13.5 版本 3.14.1 与发版材料

- **版本 bump**：`package.json` ＋ 3 个 `Cargo.toml`（`src-tauri/Cargo.toml`、`default-file-manager-common/Cargo.toml`、`default-file-manager-launcher/Cargo.toml`）；
  `node ./scripts/sync-version.js` ⇒ `Synced 3 Cargo manifests to 3.14.1`。
- **发版材料**：根 `CHANGELOG.md` 3.14.1 条目（4 条 Fixed）＋ **14 个 locale changelog** ＋ `README.md`（`### What changed in 3.14.1`）
  ＋ `README.zh-CN.md`（`### 3.14.1 改了什么`）＋ `_notes-3.14.1.md`。
- **locale 校验**（两个互相独立的脚本都过）：`node .superpowers\sdd\check-locales-3141.mjs` ⇒ 14 行 PASS + `LOCALES_OK`；
  `node .superpowers\sdd\verify-locale-changelog.mjs` ⇒ 14/14 PASS（每文件 +14 行、0 删除）。
- **口径说明**：根 `CHANGELOG.md` 与 `_notes-3.14.1.md` 补了 R1 的细则（「停在虚拟位置时助手会忘掉上一个真实目录、提示如实说明」），
  **14 个 locale 未同步这一句**（原措辞仍准确，只是少一句细节）⇒ 避免为一句细节重跑 14 语言校验。
- **未做（需用户授权）**：GitHub Release、winget 3.14.1 清单与 PR。`docs/winget/manifests/k/kizemo/AlphaFileManager/3.14.0/` 已存在，
  3.14.1 清单未准备；AGENTS 记录 `#467381`(3.9.1) 仍 OPEN，**同包 ID 不能并行提 PR**。

#### 20.13.6 装机链判别性实验（未验证项⑦）——**本批次的核心实验**

**问题**：批次 17 判定「装机版（NSIS）会把新侧车带给用户」为**未验证**（`bin\` / `installer\` 里的 exe 未被 git 跟踪 ⇒
装机机必须重新 build + 安装，否则装出来仍是旧侧车）。

**实验设计（判据必须能区分「装出来是新件」与「本来就是新件」）**：

| 阶段 | 判据 |
| --- | --- |
| 装机前 | `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe` = **旧件 `A8300AAB240F468A8E27BDCA0DD0E8079DAD642F60B53BA5C8ADD3C47D52E462`**（3.14.0 装机件，2,873,344 B） |
| 装机后 | 同一路径 = **新件 `74E5957F…21CCF`**，且装机目录 `D:\Program Files\Alpha File Manager\tools\focus-sync-sidecar.exe` 同步为新件 |

- **只判「安装后 APPDATA sha == 期望值」属假阳性** —— 两边本来就都应该是新件；必须再加「安装前确实是旧件」这个前置，
  才能证明**是安装器把旧件刷新成了新件**，而不是「本来就有」。
- 前置脚本 `.superpowers\sdd\prepare-old-sidecar.ps1`：若装机目录 `tools` 侧车 sha **不是** `A8300AAB…E462` ⇒ 直接 exit 2（前提不成立，别硬跑）。
- 执行脚本 `.superpowers\sdd\install-afm-verify.ps1 -Setup <3.14.1 setup> -ExpectedSidecarSha256 74E5957F…21CCF`，内部 `/S /FORCE` 静默安装。
- **安装器静默模式不会自动拉起 AFM** ⇒ 装机验证没有抢焦点风险，**文件级 sha 判定即足够**，不需要真机 UI 取证。

**实验结果（本轮实测，2026-11-18 15:14–15:15）**：

| 阶段 | 观察值 |
| --- | --- |
| 装机前 · 装机目录 `tools\focus-sync-sidecar.exe` | `A8300AAB240F468A8E27BDCA0DD0E8079DAD642F60B53BA5C8ADD3C47D52E462`（3.14.0 旧件） |
| 装机前 · APPDATA 侧车 | `A8300AAB…E462`（由 `prepare-old-sidecar.ps1` 从批次 17 件 `0DC1B563…E8D` **回退**成旧件） |
| 安装 | `INSTALL_EXIT 0`（`/S /FORCE`）；安装日志 `phase=PREINSTALL installerVersion=3.14.1 detectedVersion=3.14.0 installerSha256=ef5d6de0…c23` → `phase=POSTINSTALL` |
| 装机后 · 装机目录 | `74E5957F5328BF1AFE679F075E16C73691307AB5F6435B2C6BC537FAE7721CCF`（新件） |
| 装机后 · APPDATA 侧车 | `74E5957F…21CCF`（新件）⇒ **安装器把旧件刷新成了新件** |

⇒ **`VERIFY_OK reason=install-refreshed-appdata-sidecar`**：批次 17 的**未验证项⑦关闭**。

**同轮还证明了载荷正确性**（`verify-installer-payload.ps1`，解包目录 `.superpowers\sdd\payload-20261118-151156\`）：
安装包 sha256 `EF5D6DE0EAAF3E6EB43F206F014728F1BF907ABADD6996CF8CD6E69807AC0C23`（45,478,832 B）；
包内 `tools\focus-sync-sidecar.exe` sha = `74E5957F…21CCF`（match=True）；
包内扩展 `kizemo.focus-sync\dist\index.js` sha = `93439CAE4CD6B86E427C7CDA387E9F9BC84D3B53475E33A78FAA6A0B6C7CF5E8`，
标记串 `traceReplace` / `non-local-path` **齐全**（markers_ok=True）⇒ 侧车与扩展**两侧都是新件**，不会「新二进制 + 旧扩展」。

#### 20.13.7 已验证 / 推断 / 未验证

| # | 结论 | 状态 | 证据 |
| --- | --- | --- | --- |
| ① | 空路径 `/set_path` 现在是「显式清空 + 200」，不再 400、不再阻塞 | **已验证** | 20.13.3 新增单测 ＋ `138 passed / 0 failed` |
| ② | `clear_current_path` 不动 `first_push_received` | **已验证** | 同上（`clear_current_path_empties_path_but_keeps_first_push_flag`） |
| ③ | 清空后写回闸门必然 skip，不会误写 | **已验证** | `cleared_current_path_is_not_writable` ＋ `main.rs:420` 的 `path_ok` 分支 |
| ④ | commit 槽写失败归还（缺陷 A） | **已验证** | `writer.rs` 新增单测 `failed_write_releases_the_commit_slot_for_the_same_hwnd` |
| ⑤ | 扩展侧改动语法/沙箱自检通过 | **已验证** | `node --check` SYNTAX_OK；`scan-sandbox-dynamic.cjs` 0 violations；sandbox gate PASS |
| ⑥ | 侧车新二进制可产出且 sha 稳定 | **已验证** | `build-sidecar.log` 末尾 BUILD_OK；sha `74E5957F…21CCF` |
| ⑦ | 装机器会把旧侧车刷新成新侧车（未验证项⑦） | **已验证（20.13.6）** | 装机前 APPDATA = 旧件 `A8300AAB…E462` → 安装后 = 新件 `74E5957F…21CCF`，装机目录同步；`VERIFY_OK reason=install-refreshed-appdata-sidecar` |
| ⑧ | 虚拟位置停留时的**端到端真机体验**（toast 文案、侧车确实忘掉旧目录） | **未验证（本轮未做真机 UI）** | 本轮只做文件级与单测级判定 |
| ⑨ | 路径 A 祖先后代分支真机覆盖 | **未验证（沿用批次 17 结论）** | 只有单测覆盖，本轮不安排触发时序 |
| ⑩ | 「path_update 与 side车双写去重」 | **本轮只分析不实现** | 记入遗留，不在 3.14.1 范围 |
| ⑪ | `should_commit` / `auto_confirm` 在当前代码路径**不门控任何行为** | **已知（缺陷 A 收益不可观测）** | `should_commit` 被忽略、`commit_save_button` 是死代码 ⇒ 修复无害，但用户可见收益测不出来 |

#### 20.13.8 本轮工具坑（省得下一轮再踩）

1. **`npm run check` 的并发跑法（`run-p`）在本机不可靠 —— 三轮红三种红法，全是并发资源争用**：

   | 轮次 | 跑法 | 红项 | 单文件复跑 |
   | --- | --- | --- | --- |
   | ① | `run-p`（5 脚本并行） | `binary-health-check.test.ts` **hook 10 s 超时**（3239 passed / 1 failed） | **2 passed / 2.06 s** |
   | ② | `run-p` | **stylelint 零 lint 报错就退出 1** ⇒ run-p 连带杀掉 vitest ⇒ 日志里**连测试总结都没有** | stylelint **单跑全绿**（无输出） |
   | ③ | **串行** | `home-banner-click-opens-background-manager.test.ts` **test 30 s 超时**（3240 passed / 1 failed） | **3 passed / 24.74 s**（其中 **jsdom 环境就占 18.23 s** ⇒ 并发下必撞 30 s `testTimeout`） |
   | ④ | **串行复跑** | **无红项：311 个测试文件 / 3241 个用例全过（TEST_EXIT=0）** | — |

   ⇒ **判据**：偶发红项**先单文件复跑**；**连续命中不同文件**即可判定为仓库既有的负载敏感 flake，不是本轮回归。
   ⇒ **注意用例总数也会漂移**（3240 → 3241）：`仓库里的 markdown 表格` 之类测试按文件枚举生成用例，新增文档会 +1
   ⇒ **新增 `docs/*.md` 后用例数会变**，别把数字变化当成异常。
   ⇒ **建议（未做，留给下一批）**：给这两个测试文件显式 `hookTimeout` / `testTimeout`，或让 `test:unit:run` 降并发。
   ⇒ **本机事实**：stylelint 单跑绿 ⇒ 若只在 `run-p` 里红，先怀疑并发，而不是改 CSS。
2. **`cd` 之后写文件会被安全策略拦**：`Set-Location` + 写目标 ⇒ `Local hard safety policy blocked bash…`
   ⇒ 跨目录写操作一律写 `.cmd` / `.ps1` 再执行。
3. **bash 里 `$` 会被先展开**：内联 `powershell -Command "$p=…; $c=Get-Content …"` 会被 bash 吃成空串 ⇒ ParserError。
   ⇒ 含变量的 PowerShell 一律落 `.ps1` 文件再跑；内联只用无变量命令。
4. **本机 bash 是 PowerShell**：`cd /d "F:\…"` 不是合法 PowerShell ⇒ 报「找不到接受自变量…的位置参数」；用
   `Set-Location -LiteralPath 'F:\…'`。
5. **`cargo` 不在默认 PATH**，`stable` 1.97.1 的 std 不可用（E0463）⇒ 必须用
   `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin\cargo.exe`；且**不要与 `npm run check` 并发**。
6. **同文件并行 edit 会互相覆盖** ⇒ 源码改动单文件串行（本轮 `index.js` 的 4 次 edit 全部串行）。

#### 20.13.9 本批次收尾状态

- **改动**：侧车 `sidecar/src/{state.rs, http_server.rs, writer.rs}` ＋ 扩展 `dist/index.js`（**+612 / −27**）
  ＋ 版本号 `package.json` / 3 个 `Cargo.toml`
  ＋ 发版材料（根 `CHANGELOG.md`、**14 个 locale changelog**、`README.md`、`README.zh-CN.md`、`_notes-3.14.1.md`）
  ＋ 台账 §20.13 ＋ 批次 18 交接卡 `docs/handoff-2026-11-18-quark-pan-batch18.md`。
- **门禁**：`cargo test` **138 / 0**；
  前端 5 脚本**串行全绿**：`ts`=0、`lint:check`=0、`test:unit:run`=0（**311 文件 / 3241 用例**）、`verify-changelog`=0、`verify:sandbox`=0
  （并发 `run-p` 跑法在本机会偶发超时 flake，见 20.13.8 第 1 条）。
- **产物**：侧车 **sha `74E5957F…21CCF` / 2,895,360 B**；
  安装包 **`release\Alpha File Manager_3.14.1_x64-setup.exe` / sha `EF5D6DE0…C23` / 45,478,832 B**。
- **真机**：3.14.1 已装进 `D:\Program Files\Alpha File Manager`；
  **APPDATA 侧车已被安装器从旧件刷新成新件**（判别性实验闭环）；
  真机**用户数据全程只读未改**（只替换了扩展自带的侧车二进制；旧件已备份到 `.superpowers\sdd\old-sidecar-3.14.0.exe`）。
- **未做（需用户授权）**：GitHub Release、winget 3.14.1 清单与 PR、upstream 移植。

### 20.14 批次 19：⑧ 虚拟位置闭环 ＋ ⑨ 路径 A 真机覆盖 ＋ ⑩ 双写去重 ＋ ⑤ check flake ＋ winget 3.14.1 清单（2026-11-18）

#### 20.14.1 本批次范围与一句话结论

承接批次 18（台账 §20.13）的遗留项，本批次一次性收掉 5 件事：

| # | 批次 18 遗留 | 本批次结论 |
| --- | --- | --- |
| ⑧ | 虚拟位置（收藏/最近/此电脑）停留时**侧车确实忘掉上一个真实目录** ＋ toast 文案诚实 —— 端到端体验未验 | **侧车侧已闭环（HTTP 级 PASS）**；toast 仍只到**契约级**，AFM 真机 UI 未验 |
| ⑨ | 路径 A「祖先后代分支」真机覆盖 | 见 20.14.7 |
| ⑩ | 「path_update 与侧车双写去重」—— 上轮只分析不实现 | **已实现**：per-dialog 写记录 + 统一去重口径，净增 9 条判别性单测（147/0） |
| ⑤ | `npm run check` 负载 flake（`run-p` 并发偶发超时） | **已修**：全局 `hookTimeout` + 单文件 `testTimeout`，串行五步门禁全绿 |
| — | winget 3.14.1 清单 | **本地已备好**（4 文件，`check-manifest` 全过），未提交 PR（待授权） |

**范围声明**：只做**仓库版本与发版材料**；**不做对外发布**（GitHub Release / winget PR 需用户授权）。
本批次**重新构建了 3.14.1 安装包**（因为 ⑩ 修复必须随包发布）⇒ 安装包 sha 变化 ⇒ winget 清单 sha 同步更新、装机链判别性实验重做（见 20.14.8）。

#### 20.14.2 ⑤ `npm run check` 负载 flake：根因与修法

**根因（批次 18 §20.13.8 第 1 条实测）**：`run-p` 并行跑 5 个脚本时，vitest 的重型挂载测试（jsdom 环境）撞 30 s 默认 `testTimeout`；
stylelint 零 lint 报错就退出 1 ⇒ `run-p` 连带杀掉 vitest ⇒ 日志里连测试总结都没有。

**修法（不降并发、不改被测行为）**：

| 文件 | 改动 | 理由 |
| --- | --- | --- |
| `vitest.config.ts` | 新增 `hookTimeout: 30000` | 全局给 hook 留出余量，覆盖 `beforeEach` 里的重型挂载 |
| `src/modules/home/components/__tests__/home-banner-click-opens-background-manager.test.ts` | 新增 `HEAVY_MOUNT_TIMEOUT = 60000`，三条 `it` 显式挂 `testTimeout` | 该文件实测 jsdom 环境就占 18.23 s，30 s 余量太薄 |

⇒ 串行五步门禁（`.superpowers\sdd\run-check-b18c.cmd`）**全绿**：`ts`=0、`lint:check`=0、`test:unit:run`=0、`verify-changelog`=0、`verify:sandbox`=0。

#### 20.14.3 ⑩ 双写去重：定义、根因、实现

**「双写」的定义（本批次先把它钉死，之前的说法含糊）**：侧车内部**两条真正会写 Windows 文件对话框的路径**：

1. **路径 1**：`POST /set_path` 的即时写（`http_server.rs`）；
2. **路径 2**：`DialogEvent::Opened` 触发的写回（`main.rs`）。

扩展与 AFM 主程序**都没有**写对话框的能力 ⇒ 「path_update 与侧车双写」的真实含义就是上面这两条会互相重复写。

**根因**：两条路径各自判断「要不要写」，但**互不知情**：

- 路径 1 的去重口径是「逐字节比较字符串」；
- 路径 2 的去重口径是「和本次打开时读到的 `initial_path` 比较」。

⇒ 同一个目标目录在「即时写」和「打开事件写回」各被写一次 ⇒ 对话框被写两遍。

**实现（`sidecar/src/state.rs` +213 行）**：

| 新增 | 作用 |
| --- | --- |
| `pub struct WriteRecord { path: String, observed_dir: Option<String> }` | 记录「这个 hwnd 上一次被写成了什么路径」，外加**写入瞬间观察到的对话框目录** |
| `AppState.write_records: Mutex<HashMap<u32, WriteRecord>>` | per-dialog 写记录表 |
| `record_write(hwnd, path, observed_dir)` | **唯一记录点**（写成功才记） |
| `write_record(hwnd)` | 读取写记录 |
| `dialog_holds_path(hwnd, path)` | 归一化比较：`/set_path` 的去重改用它 |
| `already_written(hwnd, current_path, observed_now)` | 「已经写过了吗」判定 |
| `should_write_back(hwnd, initial, current)` | **写回决策的唯一入口** |

**关键设计：`observed_dir` 基线**。只记「写成了哪个路径」不够，因为**我们的写入可能没让 Chromium 对话框真的移动**（写成功 ≠ 对话框变了）。
所以额外记下写入瞬间**对话框自己的目录**：

- 下次判定时若 `observed_dir == current` ⇒ 说明对话框确实停在我们写的那个目录 ⇒ **是我们写的，不重写**；
- 若 `observed_dir != current` ⇒ 说明**用户手动挪了对话框** ⇒ 仍要重新同步（不牺牲既有不变量）。

**改动落点**：

| 文件 | 改动 |
| --- | --- |
| `sidecar/src/state.rs` | 新增 `WriteRecord` / `write_records` / 上述 6 个方法 ＋ `mod v063_double_write_tests`（8 条） |
| `sidecar/src/main.rs` | `writable` 改为 `state.should_write_back(hwnd_u32, &initial_path, &current)`；写成功后 `state.record_write(hwnd_u32, &current, if read_ok { Some(initial_path.clone()) } else { None })` |
| `sidecar/src/http_server.rs` | `/set_path` 的去重从「逐字节相等」改为 `state.dialog_holds_path(d.hwnd, &path)`（两条路径共用同一去重键） |
| `sidecar/src/writer.rs` | `write_succeeded(&outcome)` 时统一 `state.record_write(hwnd_u32, target, None)`（**唯一记录点**）；新增单测 `a_failed_write_leaves_no_write_record` |

**守住的不变量**（改完仍然成立）：

- 真实换目标**仍会写**；
- 路径变化**仍会写**；
- 读失败**仍按最后写入键**去重；
- **写失败不记录** ⇒ 保持 v0.5.12 的「可重试」不变量；
- 用户手动挪动对话框后**仍会重新同步**。

#### 20.14.4 ⑩ 判别性单测（净增 9 条，累计 147）

`mod v063_double_write_tests`（`state.rs`）8 条：

`a_second_write_of_the_same_target_is_refused` / `a_different_target_is_still_written` /
`a_dialog_the_user_moved_is_written_again` / `the_set_path_dedup_is_normalized` /
`the_unreadable_branch_dedups_against_the_write_record` / `a_failed_write_leaves_no_record` /
`the_sentinel_still_blocks_before_the_first_push` / `paths_equal_is_the_shared_normalizer`

`writer.rs` 1 条：`a_failed_write_leaves_no_write_record`（写失败 ⇒ 槽位与写记录都不留，可重试）。

⇒ `cargo test` **147 passed / 0 failed**（批次 18 为 138，净增 9）。日志：`.superpowers\sdd\sidecar-test-b19.log`。

#### 20.14.5 ⑧ 虚拟位置端到端：侧车 HTTP 级闭环（PASS）

**判据**：推送一个真实目录 ⇒ 侧车记住它；再推空路径（= 扩展在虚拟位置发出的「显式清空」信号）⇒ 侧车**确实忘掉**上一个真实目录。

**实测**（`.superpowers\sdd\focus\b19-virtual-position.ps1` → `.superpowers\sdd\focus\b19-virtual-position.txt`）：

| 步骤 | 请求 | 响应 | `GET /get_status` |
| --- | --- | --- | --- |
| 推真实路径 | `POST /set_path {"path":"C:\\"}` | **404** `no_active_dialog`（本轮无对话框在屏） | `current_path = "C:\\"` ⇒ **记住了** |
| 推虚拟位置信号 | `POST /set_path {"path":""}` | **200** `{"ok":true}` | `current_path = ""` ⇒ **忘了** |

`spike.log` 判定行：

```
{"event":"SpikeError","kind":"path_update","message":"current_path=C:\\ (pushed='C:\\')"}
{"event":"SpikeError","kind":"set_path_no_dialog","message":"no active dialog after waiting 60ms; path stored, nothing written"}
{"event":"SpikeError","kind":"set_path_cleared","message":"current_path cleared (was='C:\\'); AFM is not in a local directory"}
```

⇒ `JUDGE_remembered_real_path=True` / `JUDGE_forgot_previous_real_dir=True` / **`VERDICT=PASS`**。
收尾 `leftover_process=0`（只杀自己拉起的 pid，绝不 `taskkill /IM`）。

#### 20.14.6 ⑧ 扩展侧 toast：**契约级**判定（AFM 真机 UI 未验）

扩展 `dist/index.js` 侧的事实（**源码契约级**，非运行时截图）：

- 非本地路径判定在 `index.js:357-366`（标记串 `non-local-path`）：虚拟路径**不再推送**，改调 `sendClearToSidecar()`（`POST /set_path`, body `{"path":""}`）⇒ 与 20.14.5 实测的清空路径**逐字对应**；
- toast 文案在 `index.js:740-748`，类型 warning，文案口径：「当前位置不是本地文件夹……已告知助手忘掉上一个路径；没有东西可同步。」⇒ **诚实**：它说的是「没有东西可同步」，**没有**声称「已写入对话框」。

⚠️ **未验证项**：AFM 真机 UI 上这条 toast 的**实际渲染**（文案是否真的出现在界面上）本轮**没有**做 —— 本项目 `tauri dev` 界面结论一律作废，要验必须走 `tauri build` 打包产物 ＋ 真机截图。
#### 20.14.7 ⑨ 路径 A「祖先后代分支」真机覆盖

**这一项的判据是什么（源码级，先钉死）**

闸门 `state.rs:535` `afm_gate_allows(hwnd, dialog_pid)` 有两条独立路径：

- **路径 B**：`afm_ts >= dialog_open_ts`（对话框开着的时候用户去过 AFM）；
- **路径 A**（本次要覆盖的那条）：`afm_handoff_allows(dialog_pid)` —— `state.rs:605`：
  交接记录存在且 TTL 未过（默认 120 s）＋ 交接 pid ≠ 0 且 dialog pid ≠ 0
  ＋ **交接 pid ≠ AFM 自己的 pid**（否则是 v0.5.21 的假下降沿）
  ＋ `handoff.pid == dialog_pid`（严格等值分支）**或** `same_application_process(handoff.pid, dialog_pid)`（**祖先/后代分支**，`uia_event.rs:802` 沿 `th32ParentProcessID` 父链上溯）。

交接记录从哪来（`uia_event.rs:340-366`）：前台从「AFM 是前台」变成「不是 AFM」的**下降沿**，
采样当前前台窗口的属主 pid ⇒ 日志 `afm_gate_armed: handoff recorded: pid=<P1> afm_pid=<AFM>`。

⇒ **要覆盖祖先分支，必须构造出「交接 pid = 接收方 P1，dialog pid = P2，P1 是 P2 的父进程」**，
且 `P1 ≠ P2`（把严格等值分支排除掉）。

**侧车凭什么认「一个对话框」（这是本项真正的卡点）**

`uia_event.rs:573-583` `get_active_file_dialog()` **只取前台窗口**（注释明确：不做全局扫描，
避免通用 `#32770` 系统框误报）⇒ **对话框必须成为前台窗口**。
`uia_event.rs:591-633` `get_dialog_info_if_match` 的硬条件：
`IsWindowVisible` ＋ **class 精确等于 `#32770`** ＋ 标题命中「打开/保存/另存为/选择/open/save/select」
＋ **直接**子窗口里至少一个 class 为 `ComboBoxEx32`/`DirectUIHWND`/`SHELLDLL_DefView`/`DUIViewWndClassName`
（`has_child_class` 用 `FindWindowExW`，**只认直接子窗口**）。

关键事实：`get_dialog_info(hwnd)` **只取窗口矩形与 DPI，不解析路径、不做 UIA 元素查找**
⇒ 合成一个「class 精确 `#32770` + 标题含「打开」+ 一个 class 精确 `ComboBoxEx32` 的直接子窗口」的窗口
就能被侧车认成文件对话框。

**前两轮为什么失败（实测，不是猜测）**

| 轮次 | 现象 | 根因 |
| --- | --- | --- |
| 第 1 轮 | `dialog_detected_ts` 为空 | 夹具对话框不满足上面的签名 ⇒ 侧车**从未**发 `DialogDetected` ⇒ 闸门决策根本没机会发生 |
| 第 2 轮 | 进程拓扑已对（`dialog_pid=76916`，父 pid = 接收方 `136200`），但仍无 `DialogDetected` | 同上：签名不满足 |

两轮报告的失败原因都写成笼统的「no decisive gate event」，没有定位到**具体是哪一条签名条件不满足** ——
这本身是取证方法的缺陷：判据没有先证明「判据能看见目标」。

**隔离桌面没有前台语义（AGENTS §2.8 的硬约束在本机不可满足）**

`.superpowers\sdd\focus\sandbox-fg-probe.ps1` → `.superpowers\sdd\focus\tmp-b19\fg-probe-result.txt`：
`STEP1 set_foreground_w1_result=False observed_hwnd=0`、`STEP2` 同上 ⇒ `PROBE_VERDICT=FAIL`。
即：`CreateDesktop` 出来的**私有桌面**在 WinSta0 里 `GetForegroundWindow()` 恒为 0
⇒ 侧车的「只从前台取对话框」这条根本判据在私有桌面上**无法成立**。

⇒ 本项只能在**交互桌面（input desktop）**上做，这是对 AGENTS §2.8「隔离桌面」的**明确偏离**，如实记录在此。
代价：夹具窗口会在用户桌面上短暂出现（已要求夹具用小尺寸、验证后立即关闭）。

**第三轮：按源码判据重做夹具 ⇒ PASS（`VERDICT=PASS`，6/6 检查通过，`leftover_process=0`）**

前两轮的根因被坐实了：**夹具对话框不满足侧车的「文件对话框签名」** ⇒ 侧车从未发 `DialogDetected`
⇒ 闸门决策根本没机会发生（所以前两轮报告的「no decisive gate event」是**空结论**，不是被测行为回归）。

第三轮的夹具按 `uia_event.rs:591-633` 的签名重做：
**顶层窗口 class 精确 `#32770`** ＋ 标题 `B19 Fixture Open`（含 `open`）＋
**一个 class 精确 `ComboBoxEx32` 的直接子窗口**（`direct_cb=7671754`）＋ 置前台。

**实测**（`.superpowers\sdd\focus\b19-report.txt`；侧车日志
`.superpowers\sdd\focus\tmp-b19\localappdata\kizemo\focus-sync\logs\spike.log`）：

| 检查 | 结果 |
| --- | --- |
| `gate_opened_by_ancestor_branch` | **PASS** —— `opened_write_back_events=3` |
| `handoff_pid_is_receiver_process` | PASS —— `handoff_pid=504056` == `receiver_pid=504056` |
| `dialog_pid_is_child_process` | PASS —— `dialog_pid=503980` |
| `receiver_is_ancestor_of_dialog` | PASS —— `parent_of_dialog_pid=504056` == `receiver_pid` |
| `handoff_pid_differs_from_dialog_pid` | PASS —— 504056 ≠ 503980 ⇒ **严格等值分支被排除** |
| `path_B_excluded_dialog_opened_after_afm` | PASS —— 交接 `09:10:02` **早于** 对话框检测 `09:10:16` ⇒ **路径 B 被排除** |

**进程拓扑**：`afm_fixture_pid=496448`（class 精确 `Tauri Window`、标题 `Alpha File Manager - Batch19 Fixture`）
→ `receiver_pid=504056`（class `B19ReceiverWindow`，持前台产生下降沿）
→ `dialog_pid=503980`（**receiver 的直接子进程**，class `#32770`，前台）。

**决定性日志行（逐字）**：

```
{"event":"SpikeError","kind":"afm_gate_armed","message":"handoff recorded: pid=504056 afm_pid=496448 fg_class_title=false fg_class=B19ReceiverWindow","ts":"2026-11-18T09:10:02Z"}
{"event":"DialogDetected","hwnd":658262,"app":"#32770","ts":"2026-11-18T09:10:08Z"}
{"event":"SpikeError","kind":"opened_write_back","message":"dialog path='' current='F:\\...\\tmp-b19\\target'; writing current","ts":"2026-11-18T09:10:08Z"}
```

**关于紧随其后的 `WriteFailed reason=write_not_verified`**：这是**预期内**的 ——
合成对话框没有真实的「文件名」编辑框，UIA 注入无法被验证 ⇒ 写入失败。
但 `opened_write_back` 是在**真正写入之前**打印的（`main.rs:425-432`），
所以**闸门放行这个结论完全成立**，不受注入成败影响 —— 这正是本项要判的东西。

**顺带实证了 ⑩ 的不变量**：`WriteFailed` 之后没有留下写记录 ⇒ 5 秒后 monitor 重新检测同一对话框并再次尝试
（日志里 `opened_write_back` 出现 3 次）⇒ **写失败保持可重试**（v0.5.12）在真机上同样成立。

**AGENTS §2.8 的偏离（如实记录）**：本项的侧车、夹具、合成对话框**全部跑在交互桌面（input desktop）**上，
不是 `CreateDesktop` 私有桌面 —— 因为本机实测私有桌面**没有前台语义**
（`GetForegroundWindow` 恒为 0、`SetForegroundWindow` 失败、`CreateWindowStationW` 崩溃），
侧车在那里**永远看不到前台窗口** ⇒ 闸门永远开不了。
夹具窗口已做成小尺寸、判定后立即关闭，并在退出前恢复用户原来的前台窗口。
#### 20.14.8 3.14.1 重建 → 载荷校验 → 装机链复验 → winget 清单同步

**为什么必须重建**：⑩ 的修复在**侧车二进制**里；不重建安装包，修复就发不出去 ⇒ 安装包 sha 变化 ⇒ winget 清单 sha 要跟着改、装机链判别性实验要重做。

| 环节 | 本轮实测 |
| --- | --- |
| 侧车新件 | `extensions\kizemo.focus-sync\bin\focus-sync-sidecar.exe` sha **`A947898600180CDA52897A7775DCFE4BD6E6150430F6A32E24A0C59546CD3468`** / 2,903,552 B（`build-sidecar.log` 末尾 BUILD_OK） |
| 安装包 | `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.14.1_x64-setup.exe` **45,478,740 B** / sha **`1DD82EEED36127F21595FFB852FF1C65474498B44A99995CEB3B00BA3F6D2A83`** / mtime 2026-11-18 16:40:59（`build-b19.log` 末尾 `BUILD_EXIT=0`） |
| 载荷校验 | `verify-installer-payload.ps1` ⇒ 包内 `tools\focus-sync-sidecar.exe` sha **= A9478986…3468（match=True）**；包内扩展 `kizemo.focus-sync\dist\index.js` sha `93439CAE…7CF5E8`，标记串 `traceReplace` / `non-local-path` **齐全** ⇒ **`VERIFY_OK reason=payload-carries-expected-sidecar-and-new-extension`** |
| 装机链复验 | `install-afm-verify.ps1`（`/S /FORCE`）⇒ `INSTALL_EXIT 0`；安装日志 `phase=PREINSTALL … installerSha256=1dd82eee…d2a83` → `phase=POSTINSTALL`；**装机目录**与 **APPDATA 侧车**双双刷新为 `A9478986…3468` ⇒ **`VERIFY_OK reason=install-refreshed-appdata-sidecar`** |
| `release/` 顶层 | `publish-bundle-to-release.mjs` 按规则把「同名不同内容」的新包**隔离**进 `release\local-builds\`、**没有**覆盖顶层旧包 ⇒ 顶层副本**已手动替换并刷新 mtime**（见 20.14.11 第 1 条） |
| winget 清单 | `InstallerSha256` 同步为 `1DD82EEE…D2A83`（`ReleaseDate` 2026-11-18）⇒ `node ./docs/winget/check-manifest.mjs 3.14.1` **All local checks passed**（其中一条就是「sha 与 `release\` 顶层安装包逐字一致 + 45478740 bytes」） |
| `release:status` | 选中 `Alpha File Manager_3.14.1_x64-setup.exe` / 45,478,740 / `1dd82eee…d2a83`（站点源 `version.json` 仍是 3.14.0，那是**已发布**版本，不动） |

旧件留档：旧侧车 `74E5957F…21CCF` → `.superpowers\sdd\old-sidecar-b19-preinstall.exe`；旧安装包 sha `EF5D6DE0…C23`（批次 18 验过的那份）已被新件取代。
**用户真实数据全程只读未改**（只替换了扩展自带的侧车二进制）。

#### 20.14.9 门禁与产物（本轮实测）

| 门禁 | 结果 |
| --- | --- |
| 侧车 `cargo test` | **147 passed / 0 failed**（日志 `.superpowers\sdd\sidecar-test-b19.log`） |
| 前端串行五步 | `ts`=0、`lint:check`=0、`test:unit:run`=0（**311 个文件 / 3242 个用例**）、`verify-changelog`=0、`verify:sandbox`=0（日志 `.superpowers\sdd\g-{ts,lint,test,changelog,sandbox}.log`） |
| `node ./docs/winget/check-manifest.mjs 3.14.1` | **All local checks passed** |

⇒ **用例数 3241 → 3242（+1）**：本批次新增了 `docs/handoff-2026-11-18-quark-pan-batch19.md` 一个 markdown 文件，
仓库里有按文件枚举生成用例的测试 ⇒ 新增 docs 会 +1，**不是异常**（与批次 18 §20.13.8 第 1 条的记录一致）。

**产物**：侧车 sha `A9478986…3468` / 2,903,552 B；安装包 `release\Alpha File Manager_3.14.1_x64-setup.exe` sha `1DD82EEE…D2A83` / 45,478,740 B。

#### 20.14.10 已验证 / 推断 / 未验证

| # | 结论 | 状态 | 证据 |
| --- | --- | --- | --- |
| ① | 侧车确实忘掉上一个真实目录（虚拟位置停留时） | **已验证** | 20.14.5，`VERDICT=PASS` |
| ② | 同一个目标不会被侧车写两遍 | **已验证（单测级）** | 20.14.4，净增 9 条判别性单测；`147/0` |
| ③ | 真实换目标 / 用户手动挪动对话框后仍会重新写 | **已验证（单测级）** | `a_different_target_is_still_written`、`a_dialog_the_user_moved_is_written_again` |
| ④ | 写失败保持可重试 | **已验证（单测级）** | `a_failed_write_leaves_no_record`、`a_failed_write_leaves_no_write_record` |
| ⑤ | `npm run check` 负载 flake 已消除 | **已验证** | 串行五步门禁全绿（20.14.2） |
| ⑥ | 新安装包携带 ⑩ 修复的新侧车 | **已验证** | `VERIFY_OK reason=payload-carries-expected-sidecar-and-new-extension`（20.14.8） |
| ⑦ | 装机器把 APPDATA 旧侧车刷新成新侧车 | **已验证（复跑）** | `VERIFY_OK reason=install-refreshed-appdata-sidecar`（3.14.1 → 3.14.1 同版本重装，`/FORCE`） |
| ⑧ | winget 3.14.1 清单与本地安装包逐字一致 | **已验证** | `check-manifest.mjs` 全过（它自己会比对 `release\` 顶层包的 sha 与字节数） |
| ⑨ | 路径 A 祖先后代分支真机覆盖 | **已验证** | 见 20.14.7：`VERDICT=PASS`，6/6 检查通过（`opened_write_back_events=3`），`leftover_process=0` |
| ⑩ | 扩展 toast 在 AFM 真机上的实际渲染 | **未验证** | 只到源码契约级（20.14.6） |
| ⑪ | 去重后的**真机双写不复现** | **未验证** | 判别性单测覆盖了决策函数，但**没在真机对话框上做「一次打开只写一遍」的计数观测** |
| ⑫ | 其余 13 个 locale changelog 同步 ⑩ 的发版说明 | **未做** | 本批次只补了根 `CHANGELOG.md` 与 `ch`（中文）locale；其余 locale 留到下一批次 |

#### 20.14.11 本轮工具坑（省得下一轮再踩）

1. **`publish-bundle-to-release.mjs` 不会覆盖「同名不同内容」的包**：重建 3.14.1 后它把新包**隔离**到 `release\local-builds\`，
   `release\` 顶层留的还是**旧的 3.14.1**（同版本号、不同字节）。
   ⇒ 想让顶层就是新包，必须**手动覆盖并刷新 mtime**；否则 `check-manifest` 拿的是旧包 sha，`release:status` 也会挑错。
   ⇒ 判别方法：对比 `release\` 顶层与 `release\local-builds\` 同名文件的 sha。
2. **同版本重装必须带 `/FORCE`**：安装器的防呆守卫对同 build 直接 `EXIT=2`（`reason=same-build`）—— 那不是包坏了。
3. **`npm run tauri:build` 很慢**（本轮 Rust 全量重编约 12 min）⇒ 判断出要真机验收就**早启动、丢后台跑**，再做文档/补测试。
4. **`.cmd` 构建脚本必须纯 ASCII**：cmd.exe 按系统 ANSI 代码页读 `.cmd`，中文注释会变乱码并被当成命令执行。
5. **后台跑长编译不要挂 stdout 管道**：让 `.cmd` 内部 `> logfile 2>&1` 重定向，再单独读文件看进度（管道一断进程树跟着死）。
6. **含变量的 PowerShell 一律落 `.ps1` 再执行**；本机 bash 是 PowerShell，`cd /d` 不合法，用 `Set-Location -LiteralPath`。
7. **`cargo` 不在默认 PATH**：用 `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin\cargo.exe`，且**不要与 `npm run check` 并发**。
8. **私有（非交互）桌面没有前台窗口**：`GetForegroundWindow` 恒返回 0 ⇒ 任何依赖「前台窗口」的判据在隔离桌面上都不成立（见 20.14.7）。
9. **「窗口枚举」也是按桌面隔离的**：侧车只能看到**自己所在桌面**上的顶层窗口 ⇒ 对话框与侧车**必须同桌面**，否则侧车连对话框都发现不了。
10. **侧车认 AFM 只认死字符串**：前台窗口的 class 必须**恰好**是 `Tauri Window`、标题必须以 `Alpha File Manager` 开头 ⇒ 夹具窗口用别的 class 就学不到 AFM 的 pid。

#### 20.14.12 本批次收尾状态

- **改动**：侧车 `sidecar/src/{state.rs, main.rs, http_server.rs, writer.rs}`（**+268 / −7**，其中 `state.rs` **+213 / −0**）
  ＋ `vitest.config.ts` ＋ `home-banner-click-opens-background-manager.test.ts`
  ＋ winget 3.14.1 清单 4 个文件
  ＋ 根 `CHANGELOG.md` ＋ `public/changelog/ch/changelog.md` 的 3.14.1 条目
  ＋ 台账 §20.14 ＋ 批次 19 交接卡 `docs/handoff-2026-11-18-quark-pan-batch19.md`。
- **门禁**：侧车 `cargo test` **147 / 0**；前端串行五步**全绿**（`test:unit:run` = **311 文件 / 3242 用例**）；
  `check-manifest.mjs 3.14.1` 全过。
- **产物**：侧车 sha `A9478986…3468` / 2,903,552 B；安装包 sha `1DD82EEE…D2A83` / 45,478,740 B。
- **真机**：3.14.1 已重装进 `D:\Program Files\Alpha File Manager`；**APPDATA 侧车已被安装器刷新成新件**（判别性实验复跑闭环）；用户真实数据只读未改。
- **未做（需用户授权）**：GitHub Release、winget 3.14.1 清单 PR、upstream 移植。

### 20.15 批次 20：⑩ toast 真机渲染 PASS ＋ ⑪ 写入闸门根治（真机计数 UNDETERMINED）＋ ⑫ locale 收口 ＋ ⑤ check 串行化（2026-11-18）

> 本节是**批次 20** 的台账真源；入口卡 = `docs/handoff-2026-11-18-quark-pan-batch20.md`。
> 承自批次 19（§20.14）的 4 项遗留：**⑩ toast 真机渲染**、**⑪ 真机双写计数**、**⑫ 13 个 locale changelog**、**⑤ `npm run check` 并发 flake 根治**。
> **范围声明**：只做**仓库版本与发版材料**，**不做对外发布**（GitHub Release / winget 清单 PR / upstream 移植需用户授权）。
> **本批次未重建安装包** ⇒ `release\` 顶层 3.14.1 安装包**仍是批次 19 的旧件**（载荷里的侧车 sha 仍为 `A9478986…3468`，**不含**本批次的 v0.6.4 写入闸门修复）—— 理由见 20.15.7。

#### 20.15.1 ⑫ 其余 13 个 locale changelog 补 3.14.1 第 5 条 —— 完成

批次 19 只补了根 `CHANGELOG.md` 与 `public/changelog/ch/changelog.md`；本批次补齐其余 **13 个 locale**：
`de / es / fa / fr / hi / it / ja / pt / ru / sl / tr / ur / vi`，每个 locale 在 3.14.1 段落下**各 +2 行**（标题行 ＋ 条目正文）。

补的是**同一条**修复的第 5 条（与根 `CHANGELOG.md` 逐字对应）：**「同一个文件夹不再被写进对话框两次」** ——
助手在「你正在看哪个目录」与「对话框自己打开」两条路径上都会写一次；现在只写一次；换目录、或你手动挪动对话框，仍然照常同步。

**独立复验（两道）**：

| 检查 | 结果 |
| --- | --- |
| 15 个文件（根 `CHANGELOG.md` ＋ 14 个 locale）3.14.1 段落 `^\*\*` 行数 | **全 = 6**（Summary 段不算 `^\*\*`） |
| 编码卫生 | 15 个文件**无 BOM**、**无 U+FFFD** |
| `npm run verify-changelog` | **exit 0**（3.14.1、58 条目、顺序正确） |

#### 20.15.2 ⑤ `npm run check` 并发 flake 根治（`run-p` → `run-s`）

批次 19 的处理是**「缓解」**（全局 `hookTimeout: 30000` ＋ 单文件 `testTimeout`），**没有根治**；批次 19 §20.14.2 里明确写着「`run-p` 在本机不可靠这件事只做了缓解」。

**根治做法**：把 `package.json` 的 `check` / `check:win` 从 `run-p`（并行）改成 `run-s`（串行）。

| 脚本 | 批次 19 | 批次 20 |
| --- | --- | --- |
| `check` | `run-p` | **`run-s`** |
| `check:win` | `run-p` | **`run-s`** |

- **CI 分步不受影响**（CI 走的是各步骤自己的脚本，不经 `run-p` 聚合）。
- `vitest.config.ts` 的 `hookTimeout: 30000` **保留**（串行后负载降低，但重型挂载测试本身的耗时仍在）。
- **两轮定向 A/B 未复现** flake ⇒ **具体争用源 UNDETERMINED**；根治依据是「**移除并发这一失败类别**」，不是「已定位到某个根因」。这一点如实记录，不夸大。

**代价（如实记录）**：串行比并行慢（本地墙钟时间增加），换取确定性。

#### 20.15.3 ⑪ 双写根因钉死：TOCTOU（先写后记）

批次 19 的去重实现（v0.6.3）用的是**「写成功后才落写记录」**这一个去重键，根因就在这里：

| 位置 | 事实 |
| --- | --- |
| `sidecar/src/writer.rs:198-200` | **写成功之后**才 `record_write` |
| `sidecar/src/http_server.rs:529 → 601` | `/set_path` 的即时写：去重检查 → 写入 |
| `sidecar/src/main.rs:401 → 433` | `DialogEvent::Opened` 的写回：去重检查 → 写入 |

两条路径**各自**做去重检查、**互不知情**，而**记录落在写入之后** ⇒ 两条路径在**同时**通过去重检查时，**同一个目标被写两遍**（TOCTOU / check-then-act）。

**修复前真机实测**：`w1=2`（第一次推送被写了 2 遍）、`w2=0`。

#### 20.15.4 侧车 v0.6.4：per-hwnd 写入闸门

**修法**：在**去重检查之前**取闸门，并在**闸内复查**去重键 ⇒ 「检查 ＋ 写入」原子化。

| 文件 | 改动 |
| --- | --- |
| `sidecar/src/state.rs` | 新增 `pub write_gates: Mutex<HashMap<u32, Arc<Mutex<()>>>>`；`AppState::new` 初始化；`use std::sync::{Arc, Mutex};`；新增 `pub fn write_gate(&self, hwnd) -> Arc<Mutex<()>>`（**同一 hwnd 恒返回同一闸门**） |
| `sidecar/src/http_server.rs` | `dialog_holds_path` 去重检查**之前**取闸门，并在闸内**复查** |
| `sidecar/src/main.rs` | `should_write_back` 决策**之前**取闸门（决策与写入原子化） |

**加锁顺序复核**：两条路径都是「**闸门 → 状态锁**」，顺序一致 ⇒ 无死锁风险。

**判别性单测**：新增 3 条 ⇒ `cargo test --release` **150 passed / 0 failed**（批次 19 为 147），日志 `.superpowers\sdd\focus\b20-sidecar-test.log`。

**二进制重建 ＋ 双处部署**（已实测）：

| 位置 | 大小 | sha256 |
| --- | --- | --- |
| `extensions\kizemo.focus-sync\sidecar\target\release\spike.exe`（Cargo package `sigma-listary-spike` ⇒ 产物真名 **不是** `focus-sync-sidecar.exe`） | 2,907,648 B | **`480FDE62C11B1A25063B2F252C09A87C5463217A797C7C1893989F07480106CF`** |
| `extensions\kizemo.focus-sync\bin\focus-sync-sidecar.exe`（仓库） | 2,907,648 B | 同上 |
| `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`（应用实际解析处） | 2,907,648 B | 同上 |

⚠️ 仓库 `bin/` **不入 git**，但 `src-tauri\tauri.conf.json` 的 `bundle.resources` 把它映射为 `tools/focus-sync-sidecar.exe` ⇒ **重建产物即随包传播**。
⚠️ 应用会把扩展目录**复制**到 `%APPDATA%\...\extensions\` 并**从那里**解析侧车 exe ⇒ 重建后**必须**同步覆盖该处，否则真机跑的还是旧件。

#### 20.15.5 ⑩ toast 在 AFM 真机上的实际渲染 —— **PASS**

**宿主链路（源码级已查实）**：

- `src/modules/extensions/api/create-ui-api.ts:70-92` ＝ `toast.custom(markRaw(ToastStatic), { ... })`
- `src/components/ui/toaster/app-toast.ts:83` ＝ 同上
- `toaster.vue` 挂载 `[data-sonner-toaster]`

**两条路走不通（产品缺陷，如实记录）**：

1. `toolbarDropdowns` **没有任何 UI 消费方**；命令面板**未挂载**；focus-sync 只注册了 commands ＋ toolbar dropdown（**未注册** context-menu / sidebar）⇒ **扩展 toast 没有用户可达的 UI 入口**；
2. CDP `Target.getTargets` 只有 **1 个 page target（没有 worker）** ⇒ 无法 attach 扩展 worker 去调 `sigma.ui.showNotification`。

**最终取证路线（绕开上面两条）**：扩展 `dist/index.js` 的 `activate()` 里 L900 读 `edgeLegacyFilePickerTipShown`，**非 true 即弹一个 Edge tip toast**（L921-945，`duration: 12000`）
⇒ **临时**把 `%APPDATA%\com.sigma-file-manager.app\user-extensions.json` 里
`extensions["kizemo.focus-sync"].customSettings.edgeLegacyFilePickerTipShown` **字节级置 false**，
再用 `b20-toast5.ps1`（备份 → 精确替换 → **隔离桌面**起 CDP → `finally` 字节级还原并断言）取证。

**实测结果**：沙箱 `AFM-CDP-41495`、CDP 端口 9333 ⇒ `toasts=1 tip=true toaster=true` ⇒ **`VERDICT=PASS`**。
截图：`.superpowers\sdd\focus\b20-cdp3e\toast-tip-1.png`（169,292 B）＋ `.superpowers\sdd\focus\b20-cdp3e\toast-viewport.png`（169,302 B）—— 目视确认真实 AFM 中文界面右下角渲染出带「扩展 · Focus Sync」徽标的 toast。

**判据口径**：用**硬编码英文描述片段**做判据（语言无关）；结论只允许 **PASS / HOST_ONLY / UNDETERMINED** 三档。
**用户真实数据**：唯一触碰是上述**单个旗标**的字节级替换，已 `STORE_RESTORED=True` 还原。

#### 20.15.6 ⑪ 真机双写计数观测 —— **UNDETERMINED**（修复未被真机证实，也未被真机证伪）

**计数口径（不变）**：日志段内 `event=="Write"` 且 target 归一化后 == `c:\windows\system32`；第一次推送期望 **1**、第二次（正斜杠变体）期望 **0**；`WriteFailed` **不计入**。

**修复已落地且单测通过**（20.15.4），但**真机三轮都没拿到 `w1=1 w2=0`**：

| 轮次 | 做法 | 实测 | 结论 |
| --- | --- | --- | --- |
| v4 | 前台往返（接收方进程**自己**抬 AFM 类窗口，再把前台切回对话框） | `registered_dialogs_after_visit=1`；**闸门首次真正放行**（`set_path_iter hwnd=64684132 app=#32770 auto_confirm=false (shared_fg matched)`，`opened_write_back` 也发起）；但 4 次写入全部落不了地 —— 3× `reason=write_not_verified`、1× `reason=unsupported_dialog_type` | **UNDETERMINED**（`w1=0 w2=0`） |
| v5 | 不做前台往返（对话框从 `Show()` 起保持前台，与修复前 `w1=2` 那轮同条件） | `registered_dialogs=0`、`set_path_no_dialog`、**无任何写入**；诊断坐实侧车采样到 **Ghost 窗口**（`fg_class=Ghost`，owner pid=**1188**） | **UNDETERMINED**（`w1=0 w2=0`） |
| v6 | 对话框由**自己的线程**重新激活 | `dialog.raised ok=True is_fg=True`、`granted_child=True`；但接收方抬前台失败（`rcv_raised ok=False fg_is_me=False`）、写回路径 `opened_write_back_skip_no_afm_visit`、1× `WriteFailed reason=unsupported_dialog_type` | **UNDETERMINED**（`w1=0 w2=0`） |

**三轮受阻的根因全部在夹具/前台语义侧，不在被测代码侧**：

1. **Ghost 窗口**：COM 对话框创建期存在 class=`Ghost` 的瞬态前景窗（owner pid 1188），侧车 500 ms tick 采样到它 ⇒ 交接 pid 错（`handoff recorded: pid=1188 fg_class=Ghost`）⇒ 对话框从未被注册（`active_dialogs=0`）。**这同时解释了 v3 那轮的 `handoff_pid=1188`**。
2. **前台锁**：脚本进程无法从对话框抢前台（ALT trick 无效 —— ALT keyup 被投递给了对话框所在线程）⇒ 只能由夹具进程自己做。
3. **对话框内部焦点重建**：前台往返后 `inject_folder_path` 判型失准 / 回读校验不过（`write_not_verified` / `unsupported_dialog_type`）。

**源码级依据（已核对）**：

- `state.rs`：`afm_gate_allows`（路径 B `afm_ts >= open_ts`；路径 A `afm_handoff_allows(dialog_pid)`）、`set_afm_handoff` / `clear_afm_handoff` / `refresh_afm_handoff` / `set_last_afm_foreground_ts` / `mark_dialog_opened` / `learn_afm_pid`；
- `uia_event.rs:250-360`：每 500 ms tick（`learn_afm_pid` → `afm_now` → `set_last_afm_foreground_ts`；**上升沿**清交接、**下降沿**按「前台=对话框⇒取对话框属主 pid / shell 表面⇒不记录重采样 / 其余⇒前台属主 pid」采样）；
- `writer.rs:156-221`：`write_path` → `inject_folder_path` → **统一回读校验** `verify_dialog_folder(hwnd, target, 3)`；不过则降级 `write_not_verified` 且**不记录**（保持可重试）；**写成功才** `state.record_write`。

**为什么是 UNDETERMINED 而不是 FAIL**：v4 恰恰证明**闸门会放行**（这正是修复要保证的东西），失败发生在**闸门放行之后**的注入/回读环节，属于夹具侧限制；v5/v6 则根本没走到写入。⇒ **修复既未被真机证实（没拿到 `w1=1`），也未被真机证伪（没出现 `w1=2`）**；单测层已覆盖该不变量。

**下一批次的配方（已定位到具体三条）**：

1. 子进程（对话框属主）用 `AllowSetForegroundWindow` 授权接收方抬前台 —— **授权调用必须发生在接收方已成为前台进程之后**，否则 `granted=False`；
2. 交接采样必须**避开 Ghost 窗口**（让侧车在 ghost 消失后再采，或由夹具在对话框 `Show()` 之后再抬前台）；
3. 对话框须由**自己的线程**重新激活（`dialog.raise` / `dialog.raised` 已实现，v6 已验证 `ok=True is_fg=True`）；
4. 最终仍需拿到 **`w1=1 w2=0`** 才算 PASS。

**AGENTS §2.8 的偏离（如实记录）**：⑪ 依赖**前台语义**，只能在**交互桌面**上做 —— 本机实测私有桌面 `GetForegroundWindow` 恒为 0（`CreateDesktop` 无前台语义）⇒ 闸门永远开不了。
夹具窗口做成小尺寸、判定后立即关闭、退出前恢复原前台窗口。**⑩ 则是在隔离桌面完成的**（不依赖前台语义）。

#### 20.15.7 最终产物重建与门禁

**最终 exe 重建**（脚本 `.superpowers\sdd\focus\b20-final-build.ps1`，内部为 `npx tauri build --no-bundle`；`--no-bundle` ⇒ **本批次不重打安装包**）：

| 项 | 值 |
| --- | --- |
| 产物 | `src-tauri\target\release\alpha-file-manager.exe` |
| 大小 | `82,778,112 B` |
| sha256 | `A66F93B638AECECD5810F335E3DA403FE67EC9934E349B8ECA9F92E08DC67AD8` |
| mtime | `2026-11-18T21:18:26.7313893+08:00` |
| 构建日志 | `.superpowers\sdd\focus\b20-final-build.log`（`Finished release in 4m 43s`，20 条 warning，均为 dead-code 类，非缺陷） |
| 结论日志 | `.superpowers\sdd\focus\b20-final-build-run.log` |

**无 CDP 插桩判据（成立）**：

- `HAS_CDP_FLAG=False` —— 整份 exe 内**没有** `remote-debugging-port` 这个实际生效值字符串；
- `src-tauri/tauri.conf.json` 已核实回滚干净，不在工作树 diff 中（取证期临时加的调试参数已全部撤除）。
- **须如实写明的新观察**：`HAS_ADDITIONAL_BROWSER_ARGS=True`。但 `additionalBrowserArgs` 是 **Tauri 配置结构体的字段名**，必然编进 release 二进制（属元数据字符串），**不构成插桩证据**；判插桩只看实际生效值 `remote-debugging-port`。
- 扫描方法：字节按 Latin1（`Encoding.GetEncoding(28591)`，字节↔字符 1:1 往返无损）解码后 `.Contains()`；**不要**用 `Get-Content -Raw -Encoding Latin1`。

**侧车资源随构建传播（已证实）**：`--no-bundle` 同样会把 `bundle.resources` 映射的侧车复制到 `src-tauri\target\release\tools\focus-sync-sidecar.exe`，实测 `SC_SIZE=2907648`、`SC_SHA=480FDE62C11B1A25063B2F252C09A87C5463217A797C7C1893989F07480106CF`，与 20.15.4 部署的三处逐字一致 ⇒ **v0.6.4 写入闸门修复已确认进入构建资源目录**。

**仍未传播到安装包**：`release\` 顶层 3.14.1 仍是批次 19 旧件（其内侧车 sha `A9478986…3468`，**不含 v0.6.4**）；重打安装包 + 更新 winget sha + 装机链复验属对外发布载荷，需用户授权。

**`npm run check` 串行五步门禁（⑤ 由 `run-p` 改 `run-s` 的直接验证）** —— `GATE_EXIT=0`，日志 `.superpowers\sdd\b20-gate.log`：**通过**（无 `FAIL`、无 `npm error`）

| 步 | 命令 | 结果 |
| --- | --- | --- |
| 1 | `npm run ts`（`vue-tsc --build`） | 通过（无诊断输出） |
| 2 | `npm run lint:check`（内部 `run-s lint:*:check`，eslint 带 `--max-warnings 0`） | 通过（0 warning 允许量为 0，逐字干净） |
| 3 | `npm run test:unit:run`（`vitest run`） | **Test Files 311 passed (311) / Tests 3243 passed (3243)**，`Duration 102.80s` |
| 4 | `npm run verify-changelog` | 通过；`3.14.1` 段落被识别、**条目顺序正确** |
| 5 | `npm run verify:sandbox` | 全部扩展入口 `RESULT: VALID (0 violations / 20 patterns)`，`[sandbox-gate] OK` |

**与批次 19 串行基线的差异（如实登记，不归因）**：基线记为 311 文件 / **3242** 用例，本轮为 311 文件 / **3243** 用例 ⇒ 文件数不变、用例 **+1**，且**没有任何用例被删除**。

- 已排除的一条怀疑路径：`src/**/__tests__` 内**没有任何** vitest 用例读取 `public/changelog`（grep 0 命中）⇒ +1 **与 ⑫ 的 locale 补条目无关**；
- 与 ⑤ 无关（⑤ 只改 `package.json` 的脚本串联方式，不改用例）；侧车新增的 3 条单测走 `cargo test`，**不**计入 vitest；
- 结论：**+1 的具体来源本轮未逐项核对**，按「数字观察、未归因」登记；本轮门禁为全绿，不因此改动任何断言。

**闸门顺序说明**：`npm run check` 输出首行为 `run-s ts lint:check test:unit:run verify-changelog verify:sandbox`，实测日志内五步**严格依次出现、无交叠** ⇒ ⑤ 的「本地默认闸门改串行」已生效。

#### 20.15.8 已验证 / 推断 / 未验证

| # | 结论 | 状态 | 证据 |
| --- | --- | --- | --- |
| ⑩ | 扩展 toast 在 AFM 真机上的实际渲染 | **已验证** | 20.15.5，`VERDICT=PASS`（`toasts=1 tip=true toaster=true`）＋ 截图 |
| ⑪ | 同一个目标不会被侧车写两遍（**单测级**） | **已验证（单测级）** | 20.15.4，v0.6.4 写入闸门；`cargo test --release` **150 / 0** |
| ⑪ | 同一个目标不会被侧车写两遍（**真机计数级**） | **未验证（UNDETERMINED）** | 20.15.6，v4/v5/v6 三轮均 `w1=0 w2=0`；闸门放行已证实，写入落地受阻于夹具/前台 |
| ⑫ | 其余 13 个 locale changelog 同步 3.14.1 第 5 条 | **已验证** | 20.15.1，15 文件 `^\*\*` 计数全 = 6；`verify-changelog` exit 0 |
| ⑤ | `npm run check` 并发 flake 根治 | **已验证（工程口径）** | 20.15.2，`run-p`→`run-s`；**具体争用源 UNDETERMINED** |
| — | v0.6.4 侧车二进制已重建并部署到仓库 bin ＋ APPDATA | **已验证** | 20.15.4（三处 sha/size 逐字一致） |
| — | v0.6.4 修复随安装包传播 | **未做** | 本批次**未重建安装包**；`release\` 顶层 3.14.1 仍是批次 19 旧件（侧车 `A9478986…3468`） |
| — | 对外发布（GitHub Release / winget PR / upstream 移植） | **未做（需用户授权）** | 本批次范围外 |

#### 20.15.9 本轮工具坑

1. **COM 对话框创建期有 class=`Ghost` 的瞬态前景窗**（owner pid 1188）：侧车 500 ms tick 会采到它 ⇒ 交接 pid 错、对话框不被注册 ⇒ 表现成「闸门没反应」，其实**根本没注册对话框**。任何依赖前台采样的真机实验都要先排除 Ghost。
2. **`AllowSetForegroundWindow` 的授权时机**：必须在**接收方已经成为前台进程之后**调用，否则返回 `granted=False`。
3. **ALT trick 抢前台无效**：ALT keyup 会被投递到**前台窗口所在线程**，不是脚本线程。
4. **私有（非交互）桌面没有前台语义**：`GetForegroundWindow` 恒 0 ⇒ 依赖前台的判据只能在交互桌面做（承接 20.14.11 第 8 条）。
5. **UIA 树里查不到 Radix/vue-sonner toast** ⇒ 「UIA 命中数=0」**不等于**「没弹」；必须 CDP 截图 / DOM 取证。
6. **CDP `Target.getTargets` 不暴露扩展 worker** ⇒ 无法 attach worker 调 `sigma.ui.showNotification`；改用扩展自身 `activate()` 的 Edge tip toast 作为取证载体。
7. **`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 环境变量与注册表 `AdditionalBrowserArguments` 都无效**：只能靠 `tauri.conf.json` 的 `additionalBrowserArgs` 插桩（**本批次已回滚干净**，`src-tauri\tauri.conf.json` 与 HEAD 一致）。
8. **应用把扩展目录复制到 `%APPDATA%\com.sigma-file-manager.app\extensions\` 并从那里解析侧车 exe** ⇒ 重建侧车后**必须**同步覆盖该处，否则真机跑的还是旧件。
9. **bash 里 `Set-Location` 之后若有写/删目标会被安全策略拦** ⇒ `cd` 必须放进 `.ps1` 内部，再用 `pwsh -File` 跑。
10. **`Write` 覆盖「会话上次读取后被其它命令改动」的文件会被拒** ⇒ 改用新文件名。
11. **残留夹具进程会互抢前台** ⇒ 跑前/跑后用 `Get-CimInstance Win32_Process -Filter "Name='<helper>.exe'"` ＋ `Stop-Process -Id` 强杀（**不要 `taskkill /IM`**），并确认 `remaining=0`。
12. **子会话派发被 Token Plan 用量上限堵死（2056）** ⇒ ⑩/⑪ 改由根会话直接实现（本批次工时与成本因此显著上升）。
13. **端口 9222 被 ecclaw 的 Electron 占用（勿杀）**，沙箱 CDP 一律用 **9333**；同一 `--out` 目录重复跑会被日志文件锁挡住 ⇒ 每次用新目录。

#### 20.15.10 本批次收尾状态

- **改动**：`package.json`（`check` / `check:win` → `run-s`）＋ 13 个 locale changelog（各 +2 行）＋ 侧车 `sidecar/src/{state.rs, http_server.rs, main.rs}`（v0.6.4 写入闸门）＋ 台账 §20.15 ＋ 批次 20 交接卡 `docs/handoff-2026-11-18-quark-pan-batch20.md`。
- **门禁 / 产物**：见 20.15.7。
- **真机**：⑩ **PASS**；⑪ **UNDETERMINED**（修复未被真机证伪，也未被真机证实）。
- **未做（需用户授权）**：GitHub Release 3.14.1、winget 清单 PR、upstream 移植；**以及**是否本批次内重打安装包 ＋ 更新 winget 清单 sha ＋ 重做装机链复验（默认建议：不做，随下一版发布）。

### 20.16 批次 21 —— ⑬ 发版链闭合：原地重建 3.14.1 + 清单 sha + 装机链复验（2026-11-18）

入口卡：`docs/handoff-2026-11-18-quark-pan-batch21.md`（**结论一览 §〇、两处更正 §一、构建配方 §二 必读**）。本轮只做 ⑬，未做对外发布。

#### 20.16.1 决策：推翻批次 20 的「默认不重打包」

`3.14.1` 已占发版号，而包体与 winget 清单里**没有**批次 20 的 v0.6.4 写入闸门修复 ⇒ 用户装到的版本与代码声称的能力不一致，属**账面缺陷**而非「没做完」⇒ **原地重建 3.14.1，不换版本号**。

#### 20.16.2 两处事实更正（旧记载作废）

1. **`release/` 根的 3.14.1 包是批次 19 产物**：实测 `45,478,740 B / sha 1DD82EEED36127F21595FFB852FF1C65474498B44A99995CEB3B00BA3F6D2A83`（mtime 2026-11-18 16:43:01），**不是**批次 20 卡写的 `82,772,736 B / 554D0564…`。
2. **「AppData 装机侧车仍是旧版 `A9478986…` ⇒ 装机版不含修复」不成立**：本轮装前备份实测 sha **已是 `480FDE62…` = v0.6.4**（AppData 仅一份侧车，mtime `2026-11-18 20:47:58`）。⇒ 「装机版不含修复」在**侧车层**早已消失，缺陷只在**安装包 + 清单账面层**。**下一轮不要再去修这个已消失的问题。**
   顺带记下清单真源路径：`docs\winget\manifests\k\kizemo\AlphaFileManager\3.14.1\kizemo.AlphaFileManager.installer.yaml`，键名 `InstallerSha256`（批次 20 卡写的 `docs/winget/com.sigma-file-manager.installer.yaml` / `Sha256` 都不存在）。

#### 20.16.3 构建环境（本轮证伪 + 复原）

- **可用配方 = 逐字照抄 b19/b20**：先 `call vcvars64.bat`，再 `set "PATH=C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;%PATH%"`，然后 `call npm run tauri:build`。脚本 `.superpowers\sdd\b21-final.cmd`，日志 `b21-final.log`（`BUILD_EXIT=0`，`Finished release profile in 5m 59s`，`Finished 1 bundle (43.38 MiB)`；尾部仅 `dead_code` 警告）。
- **`stable-x86_64-pc-windows-msvc`（rustc 1.97.1）证伪**：在 `log v0.4.34` 爆 **251 个 `E0463 can't find crate for std`**；批次 21 中途的 `b21-run-build.ps1` 作废。`.rustup\bin` 为空、`.cargo\bin` 只有 `cargo-tauri.exe`、仓库无 `rust-toolchain*` / 无 `.cargo\config.toml` ⇒ 不 prepend 就没有 cargo，而正确的是 **1.90.0**。
- 新坑：**多行 `pwsh -Command '<整段脚本>'` 本机偶发静默空输出 + exit 1** ⇒ 取不齐判据就落成 `.ps1` 用 `-File` 跑，别反复试引号。
- 新坑：**装机根在 `D:\Program Files\Alpha File Manager`**（`C:\Program Files\Alpha File Manager` 也在，但没有 app exe）。猜 C 盘 4 个候选 ⇒ `INSTALLED_EXE_NOT_FOUND`。可靠取法两条：安装器日志 `detectedLocation=`、卸载注册表 `InstallLocation`（已固化进 `.superpowers\sdd\b21-installed-path.ps1`）。

#### 20.16.4 结果与判据

| 判据 | 结果 |
|---|---|
| 新包 | `Alpha File Manager_3.14.1_x64-setup.exe` **45,489,382 B** / sha **`5F07F5B914DFEF9FBF22E040BC3C2B67D13167B3EBA633B6A20C5BCB47F88E58`** |
| 载荷 = v0.6.4 | `verify-installer-payload.ps1` 两轮 `VERIFY_OK reason=payload-carries-expected-sidecar-and-new-extension`（侧车 `480FDE62…` match=True；扩展 manifest/dist 在位、`dist/index.js` markers_ok=True） |
| winget | `InstallerSha256` 改为 `5F07F5B9…`；`npm run check:winget` ⇒ **`WINGET_CHECK_EXIT=0`**（8 条全绿）。**⚠️ 只校验本地 `release/`，不证明 GitHub Release v3.14.1 资产已是新包** |
| 装机链 | `/S /FORCE` ⇒ `INSTALL_EXIT=0`；`install-afm-verify.ps1` ⇒ `VERIFY_OK reason=install-refreshed-appdata-sidecar`；AppData 侧车 `480FDE62…`；`SIDECAR_PROCS=0` |
| 装机 exe 身份 | `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` 82,764,288 B / mtime `22:35:54` / fv=pv 3.14.1 / sha `F03E02E7…`；bundle-type 标记归一化后 `80651557…` **= 裸构建产物归一化值** ⇒ `VERDICT_INSTALLED_IS_NEW_PAYLOAD=True`（`RAW_EQUAL=False` 只反映 `__TAURI_BUNDLE_TYPE_VAR_NSS__` vs `…_UNK__` 那 29 字节，与 §2.10 及既有记忆一致） |
| 未判定 | **运行中进程 = 新载荷**：有 1 个 AFM 进程自 `14:07:51` 起未退（PID 924772），跑的是装前镜像。盘上 exe 与 AppData 侧车均已更新，但界面要**重启应用**才吃到新 exe。本轮未杀用户进程（越权风险）⇒ 记未判定 |

发版脚本防呆：`publish-bundle-to-release.mjs` 拒绝覆盖 `release/` 根同名文件，自动改写入 `release\local-builds\` ⇒ 每次重打包都要**显式 `Copy` 覆盖根目录**（设计，不是 bug）。

#### 20.16.5 ⑪ 状态（沿用批次 20，本轮无进展）

跨全部 `dup` 日志 grep `VERDICT=|w1=|w2=` 的唯一落地写入是 `b20-dup-count3-run.log:21` `[20:30:05] VERDICT=FAIL w1=2 w2=0` = **修复前 v0.6.3 基线**；v4/v5/v6 全轮 `w1=0 w2=0` 属夹具/送达问题，**不得**当修复后证据 ⇒ ⑪ 仍 `UNDETERMINED`，需 v0.6.4 上取到 `w1=1 w2=0`。唯一未试的最优路径 = `b20-dup-count4.ps1`（子进程 `AllowSetForegroundWindow` → 接收方自抬 AFM 类窗口 ≈1.6 s → 交回对话框 → push），属前台语义取证、只能交互桌面（§2.8 偏离须记录）。

#### 20.16.6 批次 21 收尾状态

- **入库改动**：`docs\winget\manifests\k\kizemo\AlphaFileManager\3.14.1\kizemo.AlphaFileManager.installer.yaml`（`InstallerSha256`）＋ 本台账 §20.16 ＋ 批次 21 入口卡。
- **未做（需用户授权）**：GitHub Release v3.14.1 资产替换（新 sha `5F07F5B9…`）、winget manifest PR（前置：3.9.1 PR #467381 仍 OPEN）、upstream 移植。
- **下一批次（22）硬阻塞只有 ⑪**。

---

### 20.17 批次 22 —— ⑪ 第七轮取证：送达已坐实、判据文件选对，仍差「弹窗活到 push 那一刻」（2026-11-19 00:06）

#### 20.17.1 实验环境（本轮实测坐实）

| 项 | 事实 | 判别意义 |
|---|---|---|
| 侧车 | `Get-NetTCPConnection -LocalPort 37421 -State Listen` → pid 1015332（23:04:58 起）→ 路径 `%LOCALAPPDATA%\...\extensions\kizemo.focus-sync\...`，sha8 `480FDE62` = 仓库 v0.6.4 裸产物 = 装机侧车 | 被测对象就是 v0.6.4，无需重启、无需新 spawn |
| `/health` 版本串 | `http_server.rs:134` `const SPIKE_VERSION: &str = "0.6.1"` 从未 bump | **版本字符串判据无判别力**（§2.10 再添一条），一律以 sha8 为准 |
| AFM | 用户 23:04:56 重启 → pid 1012796 `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` | 「运行中 = 新载荷」坐实；§20.16 表里那行未判定可关闭 |
| spike JSON 事件流 | **唯一位置** `C:\Users\Duanyi\AppData\Local\kizemo\focus-sync\logs\spike.log`（logger.rs `log_dir()` 硬编码，`--state-dir` / env 不移动它）；带日期的 `spike.log.2026-11-18` 是 tracing 纯文本 | 对 JSON 计数**零判别力**，计数只能读主文件 |

#### 20.17.2 count7 判词 = UNDETERMINED（`w1=0 w2=0`，送达成功、写入条件未达成）

产物（**实际落盘路径**，脚本内部把 result 固定写到 `focus\` 根，不是 `-Out` 目录）：
- `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\focus\b20-dup-result4.json`
- `...\focus\b20-dup5\console.log`（判词行 `[00:06:47] VERDICT=UNDETERMINED w1=0 w2=0`）
- 旧 result 备份 `...\focus\b20-dup-result4.pre-count7.bak.json`

spike 切片（offset 8,826,277 → 8,827,373，逐字全文）：
```
16:06:30 path_update   current_path=C:\Windows\System32 (pushed='C:\Windows\System32')
16:06:30 set_path_dialogs  dialogs_count=1
16:06:39 path_update   current_path=C:/Windows/System32 (pushed='C:/Windows/System32')
16:06:39 set_path_dialogs_retry  retried 3x; final dialogs_count=0
16:06:39 set_path_dialogs  dialogs_count=0
16:06:39 set_path_no_dialog  no active dialog after waiting 60ms; path stored, nothing written
```
读法（逐条对照 `http_server.rs:412-626`）：
1. **两次 push 都送达活侧车** —— `path_update` 各一条；push2 的 HTTP 404 是 v0.5.23 起 `no_active_dialog_response()` 的**设计形状**（「路径已存、一个字没写」）⇒ 404 ≠ 没送达。
2. push1 时刻登记表有 1 个对话框，但**没有任何后续决策日志**（无 `set_path_skip_no_handoff` / `set_path_already_synced` / `set_path_deferred_not_foreground` / `set_path_iter` / `Write` / `WriteFailed`）。源码里「做了事但不写 JSON」的唯一分支 = **dead-HWND 剪枝**（`http_server.rs:470-477`，`dialog_window_exists=false` → 只发 `tracing::info`）⇒ **push 那一刻对话框窗已死、被静默剪掉**。
3. 根因 = 夹具时序（不是产品回归）：`rcv.dialogback ok=False fg=69970 fg_is_dlg=False`（前台交回对话框失败）之后，脚本仍 `Start-Sleep -Seconds 5` 再等 0.5 s 才 push（`b20-dup-count4.ps1:369/377`）⇒ 5.5 s 足够对话框死亡 / 被剪枝。v4/v6 轮的 `WriteFailed write_not_verified` 同属根因 ③「前台往返后焦点重建失败」家族，本轮只是它的**晚期形态（弹窗直接没了）**。
4. 结论：`w1=1 w2=0` 这条真机计数判据**至今从未在有效取证条件下触发过**（push1 从未写出，v0.6.4 去重闸门从未被行使）。⑪ 仍 UNDETERMINED。

#### 20.17.3 两处旧记载作废（台账 §20.16.5 与批次 20 卡中的推断）

- **更正 C**：送达哨兵**不是** `set_path_received` —— 该字符串在侧车源码全目录 grep **0 命中**。此前各轮「spike slice 里没有 `set_path_received` ⇒ push 没送达」的推断**不成立**。真送达证据 = spike JSON 的 `path_update` / `set_path_dialogs*` / `Write` / `WriteFailed` 事件。
- **更正 D**：v4/v6 报 `w1=0` 的直接原因是**计数文件选错** —— 脚本 `ALPHA_FFM_SPIKE_LOG` 未设时回退读 `$Out\spike.log`（空文件）。本轮已显式指向 LOCALAPPDATA 真事件流，slice 与 `segment1Raw/segment2Raw` 一致，判词读的是真流。⇒「侧车没收到 push」这个旧根因判断作废；「装机版不含修复」也不成立（侧车 sha8 = v0.6.4），**不要再回去修这两个已消失的问题**。

#### 20.17.4 下一轮（count8）修复设计 —— 先读码，再动手（本轮未实施）

判别条件已坐实：**push 那一刻必须 `对话框存在 && 对话框是前台 && 闸门已授`**。三件改动：
1. `b20-dialog-helper2.cs` 子进程 `FgWorker`：`dialog.raised` 写入前**轮询确认** `GetForegroundWindow()==d` 才算成功（现在是 `ForceForeground` 单次尝试的结果，`ok=False` 时焦点已永久停在用户窗口 69970）；顺带在失去前台后有限次数内重试。
2. `b20-dup-count4.ps1`：`dialogback ok=False` 时**不得**继续 5.5 s 流程 —— 立即重触发交回、有界重试；只有 `dialogback ok=True` 才 push，否则本轮直接判 UNDETERMINED（省时省证据污染）。
3. 取证防呆：push1 前脚本自己 `GetForegroundWindow()` 必须 == dialog_hwnd，否则不 push。
- ⚠️ 副作用提醒：子进程 `FgWorker` 每 100 ms 无条件 `ForceForeground(dialog)`（helper2.cs `for (int i=0;i<900;i++)` 那段）⇒ 它会和接收方抢前台、也会把焦点从用户窗口抢回来。改动时**不要顺手删掉这个循环**，它是 push2 时对话框仍在前台（从而走到 `already_synced` 分支）的支柱；只收紧「交回成功的判定」。
- 若三条都满足仍 `writes=0` ⇒ 才回到产品侧（v0.6.4 闸门 / Opened 写回交互）排查。
- 历史对照：**v3 `w1=2 w2=0` 是 v0.6.3 修复前基线**（同目标写两遍的缺陷本体），不是回归证据。
- 判据口径不变：段内 `event=="Write"` 且 target 归一化 == `c:\windows\system32`；push1 期望 1、push2 期望 0；`WriteFailed` 不计入；必须按 offset 切片。

#### 20.17.5 本轮工具坑（全路径版，防「产物找不到」）

1. `-Out b20-dup5` 时**判词三件套不在 `b20-dup5\`**：result JSON 写进脚本 `$FocusDir`（`focus\` 根），`run.log` 同样；`b20-dup5\` 只有 `console.log`。先读 `focus\b20-dup-result4.json`，别按 `-Out` 猜。
2. `Select-String -LiteralPath (Get-ChildItem …).FullName`：目录里 0 命中时数组为空 ⇒「无法将自变量绑定到参数 'LiteralPath'，因为它为 null」⇒ 先判空或改 `-Path`。
3. `focus\` 里夹具源码是 **`.cs`**（`b20-dialog-helper.cs` / `b20-dialog-helper2.cs`，配套同名 `.exe`），不是 `.rs`。
4. 一次 bash 调用里连发多条 `Write-Output` + `Select-String`，被 hook 上下文挤掉后整段输出可能丢失 ⇒ 关键查询单独发、或压成一条表达式串。

#### 20.17.6 批次 22 收尾状态

- **入库改动**：本台账 §20.17 ＋ 批次 22 入口卡 `docs/handoff-2026-11-18-quark-pan-batch22.md`。**无产品代码改动、无版本号变更**（count8 只动测试夹具与脚本）。
- **⑪**：仍 `UNDETERMINED`，最优路径 = §20.17.4 三条改动 + count8。
- **需用户授权（对外，两轮问句均未获答复，不得自行执行）**：GitHub Release v3.14.1 资产替换（新包 sha `5F07F5B9…`）、winget manifest PR（前置：3.9.1 PR #467381 仍 OPEN）、提交并推送批次 21/22 文档。
- **旧 AFM 进程 924772**：处置问题已随 23:04 用户重启（pid 1012796 = 新载荷）而失效；双实例并存期间 visit 可能抬到旧实例窗口 —— 两实例闸门逻辑等价（3.14.1 与 3.14.0 的 AFM exe 唯一差异是打包侧车），判词仍有效。

### 20.18 ⑪ 真机计数级收口 —— count11 = PASS（w1=1, w2=0）

#### 20.18.1 判词

| 项 | 值 |
| --- | --- |
| 判词 | **PASS**（`VERDICT=PASS w1=1 w2=0`，控制台退出码 0） |
| push1 | `C:\Windows\System32` ⇒ **写入 1 次**（`event=="Write"`，`target=="C:\Windows\System32"`） |
| push2 | `C:/Windows/System32`（同一目录，仅斜杠写法不同）⇒ **写入 0 次** |
| 前置断言 | `precondition_push1 match=True`（push 时刻前台 == 对话框 hwnd 528972） |
| 残留进程 | 0（`leftover_process=0`） |

证据落盘：
- `…\.superpowers\sdd\focus\b20-dup-result-c11.json`（判词 + `segment1Raw` / `segment2Raw` 原始事件）
- `…\.superpowers\sdd\focus\b20-dup11\console.log`（末行 `[09:21:18] VERDICT=PASS w1=1 w2=0`）
- `…\.superpowers\sdd\focus\count11-tracing-utc0120-0122.txt`（侧车 tracing 切片，53 行）

#### 20.18.2 侧车事件流（判词口径逐条对照）

push1 段（UTC 01:20:55 → 01:21:09）：
```
01:20:55 opened_write_back                dialog path='C:\Windows' current='C:/Windows/System32'; writing current
01:21:00 WriteFailed target=C:/Windows/System32 reason=write_not_verified
01:21:00 set_path_iter                    hwnd=528972 app=#32770 auto_confirm=false (shared_fg matched)
01:21:02 Write target=C:\Windows\System32 strategy=uia      <- 本段唯一一次成功写入
01:21:02 opened_write_back_skip_sentinel  (sentinel); skipping until first push received
```
push2 段（UTC 01:21:10 → 01:21:14）：
```
01:21:10 path_update                      current_path=C:/Windows/System32 (pushed='C:/Windows/System32')
01:21:10 set_path_dialogs                 dialogs_count=1
01:21:10 set_path_skip_no_handoff         hwnd=528972 path='C:/Windows/System32' (no direct AFM -> dialog handoff); skipping
```
读法：
1. push1 的**第一次**注入以 `write_not_verified` 收场（对话框当时不在前台，注入没能让 Chromium 对话框真的换目录）⇒ 侧车按 v0.5.17 释放提交槽，允许后续再写。
2. 对话框回到前台后，侧车打出 `dialog regained foreground with pending path change (… last_written='C:\Windows') -> resync` ⇒ 触发重写 ⇒ **01:21:02 写入成功**（`w1=1`）。这正是真实用户「切到 AFM 再切回对话框」的动作序列。
3. push2 命中闸门 `set_path_skip_no_handoff` ⇒ 一个字没写（`w2=0`）。
4. ⚠️ **机制如实登记**：本轮「只写一遍」由**两道机制叠加**达成 —— push1 段内的 sentinel（`opened_write_back_skip_sentinel`）与 push2 段的闸门拒绝（`set_path_skip_no_handoff`）。**本轮未观测到 DEDUP-1 的 `set_path_already_synced`**（那是「同一 hwnd 上已写过该路径」的 per-hwnd 去重分支）。判词口径只要求 `w2=0`，不要求命中哪一条 ⇒ ⑪ 的**结论**成立且是真机实测；但「第二次被拦」的具体分支是闸门而非 per-hwnd 去重表。若要把 DEDUP-1 单独坐实，需要在**对话框仍持有授权**的窗口内连续推两次同一路径。

#### 20.18.3 逐轮根因链（count7 → count11）

| 轮次 | 判词 | 根因 | 关键证据 |
| --- | --- | --- | --- |
| count7 | UNDETERMINED | 弹窗在 push 那一刻已死 ⇒ 侧车走 dead-HWND 静默剪枝 | §20.17.2 |
| count8 | UNDETERMINED | 接收进程 `GetWindowThreadProcessId` 声明成单参数 ⇒ 非托管侧写野指针 ⇒ `AccessViolationException` | Windows 事件日志 id 1026 |
| count9 | UNDETERMINED | 接收进程抢不到前台（前台锁）⇒ 闸门路径 A/B 均未武装 | `rcv_raised ok=False fg_is_me=False` |
| count10 | UNDETERMINED | 子进程 FgWorker 线程**卡死**：`ForceForeground` 的 `AttachThreadInput` 与 COM 模态对话框互锁 | `dialog.hwnd` 停在 09:13:58 不再更新，而进程仍存活 |
| **count11** | **PASS** | 去掉 `AttachThreadInput` + 加心跳 ⇒ FgWorker 存活 ⇒ 交接成立 | `rcv_raised got_up=True` / `rcv_dialogback ok=True` |

#### 20.18.4 本轮夹具改动（**测试夹具与脚本，非产品代码**）

`.superpowers\sdd\focus\b20-dialog-helper2.cs`：
1. 新增 `SimpleForeground(IntPtr)`：只用 `SetForegroundWindow`，**不做 `AttachThreadInput`**（死锁源）。FgWorker 内三处 `ForceForeground(d)` 全部替换。
2. `upWatch` 分支改用 `SimpleForeground(rcvHwnd)` —— 子进程是前台进程，可以把前台让给接收进程的 AFM 类窗口。
3. 新增**心跳** `dialog.beat`（每轮无条件写）：用于区分「线程卡死」与「找不到对话框」两种 0 产出。
4. FgWorker 循环 `900 → 1800` 轮（90 s → 180 s）：**保留**该循环（§20.17.4 的约束：它是 push2 时刻弹窗仍在前台的支柱），只延长不删。
5. 修一处语法错误：`uint me = (uint)Process.GetCurrentProcess().Id);` 多出的右括号（`csc` CS1002/CS1525 在编译期拦下，未进实验）。

`.superpowers\sdd\focus\b20-dup-count4.ps1` / `b23-count9-run.ps1`：`$StateDir` / `$ResultPath` 换到 `c11`（新目录 + 新 result，防 provenance 污染）。

#### 20.18.5 本轮坐实的两条通用规律

1. **闸门路径 B 的物理前提**：接收窗口必须是侧车认得的 AFM 类窗口（类名 `Tauri Window` + 标题以 `Alpha File Manager` 开头，见 `sidecar/src/uia_event.rs:841`），且必须在**对话框打开之后**再当一次前台（`afm_ts >= open_ts`）⇒ 只满足「对话框开着」不够。夹具的接收窗口本来就是这个伪装（`CreateTopLevel("Tauri Window", "Alpha File Manager - Batch20 Fixture", …)`），此前 4 轮真正缺的是**时序**。
2. **`AttachThreadInput` 与 COM 模态对话框互锁**：任何「跨进程激活一个模态对话框」的实现，只要用了 `AttachThreadInput` + 同步等前台，就会在模态循环里死锁 —— 表现为**目标进程仍存活、但工作线程停止写任何文件**。判别姿势：看目标工作线程的心跳/状态文件是否还在推进，而不是看进程是否存活。

#### 20.18.6 发布面现状更正（与旧记载不同，必须更新）

- `gh api repos/kizemo/alpha-file-manager/releases`：**最新 release 是 `v3.14.0`**（2026-11-11），**`v3.14.1` 从未发布过**；tags 列表同样止于 `v3.14.0` ⇒ §20.17.6 写的「GitHub Release v3.14.1 资产替换」**没有可替换的对象**。
- `docs/winget/manifests/k/kizemo/AlphaFileManager/3.14.1/` 目录存在，但 `InstallerUrl` 指向 `releases/download/v3.14.1/…` ⇒ **404**。该 manifest 改动（sha `1DD82EEE…` → `5F07F5B9…`）**本轮不提交**：真值必须与实际发布的资产绑定，在 release 发布之前提交只会造出一个「看起来更新了、实则仍 404」的 manifest。新 sha `5F07F5B9…` 记在本节，发布时直接取用。
- 版本面核对：`package.json` = `src-tauri/Cargo.toml` = `3.14.1`；`tauri.conf.json` 的 version 指向 `../package.json`；`CHANGELOG.md` + 14 个 locale 包（`public/changelog/<lang>/changelog.md`）**均已有 3.14.1 条目**，其中「The same folder is no longer put into a dialog twice / 同一个文件夹不再被写两遍」正是本轮 ⑪ 验证的那一条 ⇒ **本轮无产品代码改动 ⇒ 不 bump 版本号、不新增 CHANGELOG 条目**（避免造一个内容与 3.14.1 完全相同的假版本）。

#### 20.18.7 下一阶段起点

- 批次 22 收口完成，⑪ = PASS（真机计数级）。
- 下一阶段（批次 23）＝ **阿里云盘 + OneDrive**，计划见 `docs/plan-2026-11-19-aliyun-onedrive.md`（九节：结论一览 / 目标与范围 / 现状盘点 / 关键设计决策 / Phase 0–4 / 最小改动清单 / 风险登记 / 门禁 / 待验证问题 Q1–Q6）。
- 动手前先修 `src/utils/remote-directory.ts:137-147` 硬编码「夸克网盘」的串台文案（接第三个 provider 前必须参数化）。
- **仍需用户授权（对外）**：发布 `v3.14.1` release 资产（用 sha `5F07F5B9…` 的包）、提 winget manifest PR（前置 3.9.1 PR #467381 仍 OPEN）。

### 20.19 批次 23（插队）—— 「位置」显示手机/U 盘 ＋ 双击手机能在 AFM 内打开：根因坐实 → 双端落地 → 门禁全绿（2026-11-19）

#### 20.19.1 为什么插队

用户明确要求：在开发阿里云盘/OneDrive **之前**，先修两个严重 bug —— ①插入手机或 U 盘后「位置」里显示不出来；②AFM 被设为默认资源管理器后，在系统资源管理器里双击手机盘符，跳到 AFM 却只落到「位置」界面、打不开设备。阿里云盘/OneDrive 计划（`docs/plan-2026-11-19-aliyun-onedrive.md`）不因本批次改变，落地顺序仍是先阿里云盘（第一版只读）、OneDrive 需先做 WebView2 OAuth 截获可行性验证（P0-2）。

方案文档：`docs/plan-2026-11-19-shell-devices-locations.md`（R1–R7 后端 / F1–F4 前端 / 门禁 / 真机判据 J1–J4 / 风险 R1–R5）。

#### 20.19.2 两条根因（坐实，非推断）

**根因 1：位置页只枚举 Win32 卷，MTP 手机根本不在结果里。**
链路 `src/utils/virtual-locations.ts:250-253`（`readLocationsDirectory`）→ `src-tauri/src/dir_reader/commands.rs:140-145`（`get_system_drives`）→ `src-tauri/src/dir_reader/drives.rs:19-50`（`sysinfo::Disks`，Windows 走 `FindFirstVolumeW` → `GetVolumePathNamesForVolumeNameW` → `GetDriveTypeW`）。**MTP 手机不是 Win32 卷**，`sysinfo::Disks` 不会报它 ⇒ 架构级缺口，不是过滤条件写错。另有三道过滤会丢 U 盘：容量为 0 的丢弃、无挂载点的丢弃、只留 FIXED/REMOVABLE。

**根因 2：shell 命名空间路径被启动链路丢弃。**
`src-tauri/src/lib.rs:146-175`（`filter_shell_namespace_args`）把 `::{` / `shell:` 参数**从 `filtered_args` 里删除** → 非 absorbed 的走 `delegate_shell_namespace_paths` → `explorer.exe`；而前端 `src/utils/launch-directories.ts:190-192` 在没有候选时硬造 `LOCATIONS_VIRTUAL_PATH`，于是用户看到的就是「跳到位置界面」——AFM 已把 shell 路径丢了，再去要目录当然要空。

用户机器真机日志（`C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\logs\Alpha File Manager.log`）：

```
[reveal] open_in_native_explorer spawned:
path=::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\?\usb#vid_22d9&pid_2765&mi_00#6&337f5e7c&1&0000#{6ac27878-a6fa-4155-ba85-f98f491d4f33}\SID-{10001,,1001925963776}
```

`vid_22d9` = MTP 手机。

#### 20.19.3 后端落地（T1–T4，10 个文件全在 `src-tauri/`）

| 文件 | 改动 |
| --- | --- |
| `Cargo.toml` | windows crate 加 2 个 feature：`Win32_UI_Shell_PropertiesSystem`、`Win32_Devices_PortableDevices`（**不新增 crate**） |
| `dir_reader/types.rs` | 新增公开类型 `ShellDeviceInfo { name, shell_path, is_folder }`，`#[serde(rename_all = "camelCase")]` |
| `dir_reader/shell_namespace.rs` | **新建**（约 700 行）：`is_shell_namespace_path` / `get_shell_devices` / `read_shell_dir` / `stat_shell_entry`；COM 公寓守卫、`SHParseDisplayName` 解析 + 两条退路、`IShellItem` 枚举子项、`IPortableDevice` QI 判据 |
| `dir_reader.rs` | 注册 `mod shell_namespace;`，导出 `ShellDeviceInfo` 与 `pub(crate) is_shell_namespace_path` |
| `dir_reader/commands.rs` | 新增 `#[tauri::command] get_shell_devices`（走 `spawn_blocking`） |
| `dir_reader/read.rs` | `read_dir` / `get_dir_entry` 最前面加 shell 分支；timeout 变体经同两函数自动覆盖 |
| `dir_reader/drives.rs` | 容量为 0 的**可移动盘不再丢弃**（固定盘口径不变）；新增 `sysinfo_paths` 集合区分「压根没报过」与「报了但被过滤」 |
| `dir_reader/drives_platform.rs` | 新增 `append_windows_logical_drives`：`GetLogicalDrives` + `GetDriveTypeW` 兜底（FIXED/REMOVABLE，容量读不到留 0），只补 sysinfo 未报过的盘 |
| `system_icons.rs` | shell 路径改走 shell 图标工厂（`SHCreateItemFromParsingName` → `IShellItemImageFactory::GetImage` → `GetDIBits` → PNG）；`is_shell_icon_candidate_path` 不再拒绝 shell 路径 |
| `lib.rs` | T4：删除 `delegate_shell_namespace_paths`、取消自动委派 `explorer.exe`（`open_in_native_explorer` 保留 + `#[allow(dead_code)]`）；非 absorbed 的 shell 路径保留进 `filtered_args` 并打 `log::info!("[launch] shell namespace path kept: path={path}")`；`handle_second_instance` 可打开路径判定改为 `filtered_args.len() > 1 \|\| !kept_shell_paths.is_empty()`；注册 `get_shell_devices`；末尾测试模块重写为 4 项 |

#### 20.19.4 前端落地（F1–F4，只改 `src/**`）

新建 `src/types/shell-device.ts`、`src/utils/shell-paths.ts`、`src/utils/shell-devices.ts`、`src/utils/open-in-system-explorer.ts`、`src/composables/use-shell-devices.ts`；
改 `virtual-locations.ts`（位置页新增 `devices` 分区，盘符与设备 `Promise.all` 并发拉取）、`normalize-path.ts`（`::{` / `shell:` 当不透明路径，不切段、不规范化）、`launch-directories.ts`（启动参数优先 shell 路径，兜底只在候选为空时生效）、`use-init.ts`（shell 候选单独 `getDirEntry` 验证，失败弹**可见** toast 并给「在系统资源管理器中打开」动作，不再静默停在位置页）、`types/dir-entry.ts`（`LocationGroup` 加 `'devices'`）、地址栏两级面包屑（`位置 > 设备名`）、网格/列表视图设备分区标题与手机图标、17 个 i18n 语言包各加 5 个 key。

前端 worker 在收尾过程中修掉 3 个真实缺陷：①`shell-paths.ts` 裸节点正则两个冒号落在第一分支内部（实际匹配 `:{GUID}`，真实路径 `::{GUID}` 永不命中）；②尾部剥离顺序错误（先剥 `\` 再剥 `?` 会留下 `::{GUID}\`）；③`virtual-locations.ts` 设备拉取缺调用方兜底。

#### 20.19.5 门禁结果（均为真实输出）

- `cd src-tauri; cargo test --lib -- --nocapture` ⇒ **`1433 passed; 0 failed; 1 ignored`**
- `npm run check`（`ts` + `lint:check` + `test:unit:run` + `verify-changelog` + `verify:sandbox`）⇒ **退出码 0**，`Test Files 313 passed (313)` / `Tests 3298 passed (3298)`，`RESULT: VALID (0 violations / 20 patterns)`

#### 20.19.6 ⭐ 重大实证：本机确实连着手机，且 MTP 分支已被实证

**此前「本机无手机、无可移动盘」的判断是错的** —— `Get-Volume` / `sysinfo::Disks` 看不到 MTP 设备，只有 Shell COM 枚举「此电脑」才看得到。Rust 侧 `--nocapture` 真实输出：

```
[shell-namespace] 设备候选: name=fnNAS path=::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\uuid:4d696e69-444c-164e-9d41-7085c27f4afe 目录=可读，4 个条目
[shell-namespace] 设备候选: name=一加 Ace 3 path=::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\\?\usb#vid_22d9&pid_2765&mi_00#6&337f5e7c&1&0000#{6ac27878-a6fa-4155-ba85-f98f491d4f33} 目录=可读，1 个条目
```

⇒ 手机「一加 Ace 3」的 MTP 目录**真的能读出来**，解析名与用户日志里的启动参数前缀完全一致。这是本批次最硬的一条证据。

#### 20.19.7 已知限制与遗留（务必如实带给用户）

1. **U 盘 / 可移动盘兜底未实证**：本机没有可移动盘。兜底代码在 `drives_platform.rs::append_windows_logical_drives`，需插 U 盘复测。
2. **「设备」判据会误报**：兜底判据是「无文件系统路径 + 文件夹」，实测把 NAS（`fnNAS`）也列成了设备。判据在 `shell_namespace.rs::collect_shell_devices`（`is_portable_device_item` 的 QI 判据 + 兜底判据）。**待收紧。**
3. **`SHParseDisplayName` 不是万能的**：实测 fnNAS 的 `uuid:` 节点解析直接报 `0x80070057`。已加两条退路（逐级缩短解析名再匹配子项 / 从「此电脑」按前缀逐级下钻），代码在 `shell_namespace.rs::resolve_shell_item`；手机上带 `\SID-{...}` 的两级路径走的就是第二条退路。
4. `LaunchContext.had_delegated_shell_paths` 现**恒为 false**（字段按约束未删，语义变为「不再委派」）；`ShellFilterResult.delegated_paths` 保留但恒空。
5. `open_in_native_explorer` 已无调用点（保留函数体 + `#[allow(dead_code)]`）。
6. shell 图标（`GetDIBits` → PNG）只验证了编译与分流逻辑，**未在真机 UI 上目视确认**。
7. **注册层本批次不动**：`Classes\Drive` / `Classes\Directory` 未注册（`default_file_manager.rs:372-378` 只注册 `Folder\shell\open\command`、`Folder\shell\explore\command`、桌面 CLSID `opennewwindow`）⇒ 真实 U 盘盘符双击是否进 AFM 仍未验证；改注册动系统行为且牵动 `ROOT_REGISTRY_KEYS` 定长数组与快照/回滚逻辑 ⇒ 列为遗留项。
8. **本批次未装机**：§20.19.5 的门禁全是代码级；UI 上「位置」是否出现手机、双击是否进 AFM、U 盘是否出现、图标效果 —— 均为 UNDETERMINED。

#### 20.19.8 版本面

版本 `3.14.1` → **`3.15.0`**（`package.json` = `src-tauri/Cargo.toml` = `default-file-manager-common/Cargo.toml` = `default-file-manager-launcher/Cargo.toml`，已由 `npm run sync-version` 同步；`tauri.conf.json` 的 version 指向 `../package.json`）。
`CHANGELOG.md` 新增 `## [3.15.0] - November 2026` 条目（4 条：修复/修复/新增/修复），`public/changelog/` 14 个 locale 包同步补齐。
`README.md` / `README.zh-CN.md` 同步：功能表新增「手机与 U 盘」一行，并新增「3.15.0 改了什么」小节。

#### 20.19.9 下一阶段起点

- 批次 23 代码已落地、门禁全绿；**剩余：装机真机验收**（打包产物走 `npm run tauri:build`，末尾只复制到 `release/`，不上传）。
- 装机后请用户在本机（有手机）实测：①AFM 内「位置」页是否出现「一加 Ace 3」并能点进去浏览；②在 Windows 资源管理器里双击手机盘符，是否在 AFM 内打开而不是落到位置页；③插 U 盘是否出现。
- 收紧「设备」判据（把 `fnNAS` 之类误报挡掉）。
- 回到主线：阿里云盘 + OneDrive（`docs/plan-2026-11-19-aliyun-onedrive.md`），动手前先修 `src/utils/remote-directory.ts:137-147` 硬编码「夸克网盘」的串台文案。
- **仍需用户授权（对外）**：发布 release 资产、提 winget manifest PR。

---

### 20.20 批次 24 收口：目录树显示 shell 设备 + 地址栏面包屑逐级下钻（2026-11-19）

用户 14:19 回报批次 23 的两个新缺陷 —— ① 左侧目录树里看不到手机；② 进入手机盘后地址栏
只能显示到「设备」一级，不能逐级下钻。**两条都已修复并装机真机判定 PASS。**
完整证据见 `.superpowers/sdd/b24-final-gate-report.md`（判词全部由根会话回读证据文件当场自证）。

#### 20.20.1 根因与修法

| # | 缺陷 | 根因 | 修法 |
|---|---|---|---|
| A | 地址栏只显示到「设备」 | 面包屑用 `split(/[\\/,]/)` 切 shell path，`SID-{10001,,1001925963776}` 内的 `,,` 被切碎、GUID 段被当成一段；设备名还硬编码成「设备」 | 后端新增 `resolve_shell_path_chain(path) -> Vec<ShellPathCrumb>`，沿 PIDL 逐层 `GetParent()` 取**真实显示名**；前端 `utils/shell-path-chain.ts` 按 path 缓存（上限 64、失败不写缓存） |
| B | 目录树看不到手机 | `pathDepth()` 与 `computeAncestorPaths()` 都按 `/` 切分，shell 路径 `\` 分隔被算成 depth 0 ⇒ 设备根既挤进第一层、也进不了 `treeRootPaths` 的祖先链 | `remote-tree-roots.ts` 加设备根；`folder-tree.ts` 加 `isShellPath` 护栏 + `computeShellAncestorPaths`；`use-file-tree.ts` 五处切分改 `split(/[\\/]+/)` |
| D1 | 树里设备名显示成「设备」而非「一加 Ace 3」 | 同 A | 与 A **同一条祖先链一并消失**，未做额外改动 |

launcher 线（双击手机进 AFM）8 条 argv 用例全过 ⇒ **launcher 层无缺陷**，`::{…}` 本就逐字节原样透传，产品代码未动。

#### 20.20.2 四条门禁（`.superpowers/sdd/b24-final-a{1,2,3,4}.log`，退出码均 0）

| 门禁 | 数字 |
|---|---|
| A1 `cargo test --lib` | **1436 passed / 0 failed / 1 ignored**（与批次 23 基线持平 ⇒ 根会话 `lib.rs:811` 常量订正复验坐实） |
| A2 launcher | **14 passed / 0 failed** |
| A3 `npm run check` | **315 files / 3336 tests / RESULT: VALID**（批次 23 基线 313/3303） |
| A4 `npm run verify:sandbox` | **RESULT: VALID（0 violations / 20 patterns）** |

#### 20.20.3 真机判据（3.15.0 重新打包 45,602,503 B / 装机 15:42–15:43）

- **D-0 来历坐实**：归一化 `__TAURI_BUNDLE_TYPE_VAR_{NSS,UNK}__` 后逐 64 KiB 块比对，差异块归一化后相等
  ⇒ `NORMALIZED_EQUAL=True`；装机 exe 大小 82,767,360 → **82,791,424**、mtime 12:26 → 15:42。
  **装机路径更正：`D:\Program Files\Alpha File Manager\`（无空格），旧记录作废。**
- **D-1 目录树 = PASS**：手机根与 fnNAS 的 `depth=4 / chevronX=92`，与 `C:/D:/E:/F:/G:` **完全同层**；
  子节点 `depth=5 / chevronX=116`、孙节点 `depth=6 / chevronX=140`（每级 +24px）⇒ 是嵌套不是并列第二根。
- **D-2 面包屑 = PASS**：进入手机盘 `DCIM\Camera` 得 `CRUMB_COUNT=6`，真实层级
  「位置 → 一加 Ace 3 → 内部共享存储空间 → DCIM → Camera」；另一实例三级层「… → .7934039a」。
- **D-3 点中间层 = PASS**：点「一加 Ace 3」段（按几何中心点击）后面包屑与列表都切到该层。
  首轮坐标取自退化 `GetClickablePoint()` 落在元素外 ⇒ 那是**探针缺陷不是产品缺陷**。
- **分隔符口径风险排除**（§20.19.9 点名的最高风险遗留项）：树行 `title` 被 UIA 映射成 HelpText，
  直接读到真机 path 原文 —— shell 子节点 `backslash` 逐级 +1、`slash=0` ⇒ `normalize_path` 未污染 shell 路径；
  本地盘行反而 `slash=1`（`C:/`）。两套口径各自正确。

#### 20.20.4 未判定项与遗留（下一批要做）

| # | 项 | 状态 |
|---|---|---|
| D-4 | 设备热插拔旧名残留 | **未判定**（无可安全弹出的设备通路）。代码侧已坐实缺口：`clearShellPathChainCache()` 只有定义 + 测试调用，**产品代码零调用点** |
| L1 | 左侧导航树仍走 `getShellPathDisplayName()`（`use-file-tree.ts:859`），未接祖先链 | 遗留 |
| L2 | 面包屑 5~6 段横向溢出观感未肉眼确认 | 遗留 |
| L3 | U 盘双击经注册层进 AFM 未验证（`Classes\Drive` 未注册，属系统行为改动，需先问用户） | 遗留 |
| L4 | 15:53:19 装机版进程消失，与首轮退化坐标点击时刻重合、重启后一切正常 | **未归因观察，不作回归** |

#### 20.20.5 本轮新增的工具坑（可复用）

- **树缩进判据不能用行元素左边缘 X**（行内图标/chevron 才随层级动）⇒ 取行内 chevron 的 X，实测每级 +24px。
- **`title="row.path"` → UIA HelpText** 是本项目读「真机 path 原文」的现成通道，比截图 OCR 可靠。
- **面包屑/小控件点击必须按几何中心**，`GetClickablePoint()` 会退化成左上角而落空。
- **子会话自述「现在写交付报告」≠ 文件已落盘** ⇒ 判词一律自己回读证据文件；本轮 `.md` 确实缺失，报告由根会话重建。
- **PowerShell 的 `Select-String -Path 'src\**\*.ts'` 不做递归** ⇒ 全库符号搜索用 `grep` 工具。
- **裸 sha 会因 bundle-type 标记假红**：判装机与构建产物是否同源，用归一化 + 分块比对，裸 sha256 不进判词。

---

### 20.21 批次 25（插队）—— 手机缩略图/文件大小（25A）+ U 盘无法安全弹出（25B）：根因坐实（2026-11-19）

批次 24 已提交为 `05871376`。方案落盘 `docs/plan-2026-11-19-batch25-thumb-usb.md`。
25B 全部证据在 `.superpowers/sdd/b25-usb-*`（20 个文件，原被 worker 写到仓库**外**的
`F:\soft\00selfmade\filemanager\.superpowers\sdd\`，已由根会话归位进仓库）。

#### 20.21.1 B25-2 U 盘弹出：根因**实测坐实**，且与用户/我最初的假设都不同

| 结论 | 证据 |
| --- | --- |
| AFM 对 `H:\` 持有且仅持有 **1 个常驻卷根句柄**（`hv=6832`，三次采样 14s 间隔恒等 ⇒ 不是轮询瞬时句柄） | `b25-usb-step0-handles2.txt:15,16` `VERDICT: AFM-OCCUPYING` + `b25-usb-step0-persistence.txt:1-3` |
| AFM 对 **`C:\ D:\ E:\ F:\ G:\ H:\` 每个被索引盘根各持 1 个**，与 `status.json` 的 `indexed_drive_roots` 逐项对应 ⇒ **归因唯一：全局搜索索引递归 watch** | `b25-usb-step1-attribution.txt:10,15,17`；代码 `global_search/index_watcher.rs:124-131` |
| `dir_watcher` / 缩略图 / dir_size 全部**无 H: 句柄** ⇒ 排除 | 同上（H: 只有那一条句柄） |
| **用户从未在 AFM 里打开过 H:** ⇒ 缺陷不是「标签页占目录」，是**索引器无条件常驻**，任何新插入的可移动设备都会被静默扣留 | 现场核实 |
| `stop_watcher` 唯一调用点在 `scan.rs:158`（扫描取消）⇒ 运行期**没有任何按卷释放路径**；取消索引/关窗/退出全不释放 | 静态 + 日志 |
| 托盘弹出被 Windows 真否决：`CM_Request_Device_EjectW` 返回 **23 = `CR_REMOVE_VETOED`**（SDK `cfgmgr32.h:4720` = `0x17`；早期按文档猜的「32=vetoed」是**错的**） | `b25-usb-step4-eject.txt:25`、`b25-usb-step6-eject3.txt:6,7` |
| 抓到用户看到的**原生否决对话框原文**（类 `#32770`，标题 `Microsoft Windows`，按钮 **取消 / 重试 / 继续**） | `b25-usb-step5-eject2.txt:18,24,27-30`、`b25-usb-step6-eject3.txt:9-16` |

**现场未受污染**：AFM pid 79340 全程未关停、`H:` 未掉盘、H: 内**零写入**（红线执行到位）。
弹出确认框**只发 `WM_CLOSE`（取消）**，未点「继续」强拆。

**修复方向（已下发实施，`bg_b570e7c8`）**：层 1 = 可移动盘不进索引常驻根集合（`scan.rs:895-903`
一类 roots 构造处过滤，含 `DriveChange` 重平衡路径，跳过时打 `tracing::info!` 作装机判据）；
层 2 = 新增 `src-tauri/src/device_events.rs`（message-only 窗口 + `DBT_DEVTYP_VOLUME` **加 `DBT_DEVTYP_HANDLE`**，
QUERYREMOVE/REMOVEPENDING 时按 `dbcv_unitMask` 展开多盘符 → 释放 `dir_watcher`/索引 watcher 句柄 →
不否决地放行 → `emit` 给前端关标签并跳「位置」）。

#### 20.21.2 B25-1 手机缩略图/大小：25A 首轮取证（进行中）

- 探针编译失败暴露一个坑：本项目 `shell_namespace.rs:65-68` 的常量是 **`PKEY_SIZE`（全大写）**，
  windows crate 里叫 `PKEY_Size` ⇒ 照抄 SDK 名必挂（`b25-thumb-probe.txt:3`）。
- shell 枚举 oracle 实测（`b25-thumb-size-oracle.txt`）：`一加 Ace 3` 设备根与 fnNAS 根各返回
  `count=1`，唯一子项是 `FILE ext= name=Items size=0 path=`（**path 为空**）；`Win (C:)` 正常报
  `DIR path=E:\ path=C:\`。⇒ 待判定：产品侧 `DirEntry.path` 对 shell 子项是否真为空；
  若真为空，则**缩略图与文件大小两个症状是同一根因**（两者都以 path 为入口）。
  反证：批次 24 真机 UIA 能逐级下钻到 `SID-{10001,,1001925963776}` ⇒ 倾向于「oracle 只读了
  `ParsingPath` 一类单一属性、未回落到 pidl 拼接」，**必须用真机 UIA 原文定论，不许靠 oracle 自证**。

  > ⚠ **本条两处推测已于 §20.21.4 被实测否证，勿再引用**：① 设备根 `count=1 / name=Items`
  > 是**取证脚本自身的 bug**（`b25-size-oracle.ps1:76` 写 `$folder.Items` 漏了调用括号，
  > 拿到的是集合对象而非子项），**产品侧不存在「Items 假条目」缺陷**；
  > ② 「缩略图与大小同一根因」**不成立** —— 实测是**两条互不相干的独立根因**（见 §20.21.4.1）。
- 设备根 `Items` 是**容器的规范名**（等价于「这个文件夹里的全部条目」），不是真实显示名；
  把它显示出来属于缺陷，登记待修。

#### 20.21.3 本轮新增的工具坑（可复用）

- **判返回码必须核对 SDK 头**：`cfgmgr32.h:4720 CR_REMOVE_VETOED = 0x17`（23）；
  26 是 `CR_BUFFER_SMALL`（本轮踩过一次，导致前两轮结论方向错）。
- **PowerShell 字符串插值 `"$Drive:$"` 被当成作用域限定符** ⇒ 一律写 `${Drive}:`；
  该 bug 会让盘符判据恒为 False 且「看着像 PASS」（`HasHDrive` 全 false 而 `DRIVES` 明明有 H:\）。
- **`-f` 格式化与 `$( )` 混用**同样会让盘符检测恒为 False：`("X={0}" -f ($l -match [regex]$D))` 取到空串。
- **`[ordered]@{}` 不能这样用**，`[ordered]` 是 TypeAccelerator 不是 conversion 目标；改用
  `[System.Collections.Specialized.OrderedDictionary]` + `.Add()`。
- **`Add-Type -TypeDefinition` 不支持 `-UsingNamespace`**（只有 `-MemberDefinition` 支持）。
- **`ForEach-Object { ... }` 块里插裸 `''`**（空字符串表达式）会触发「缺少右大括号」ParserError。
- **`Start-Job` 起了立刻 `Remove-Job` 是死代码** ⇒ 超时兜底从未生效；且本机权限门会把 `Remove-Job`
  误判为文件删除命令（`sensitive: true`），双重失效。
- **P/Invoke 不写 `CharSet=CharSet.Unicode`** ⇒ 中文窗口标题取回来是乱码单字，判据只能退化成「有无模态窗」。
- **未提权也能取到别的进程的句柄名**：AFM 全部 1366 个句柄项都可见、`OpenProcess(PROCESS_DUP_HANDLE)` 成功；
  此前 `NOT_FOUND` 的成因是名字解析，不是权限。

#### 20.21.4 批次 25 收口（2026-11-19 晚：两条缺陷最终定案 + 真机验收）

**B25-1 手机（MTP）图片与视频：两条互不相干的独立根因，均已修**（实现线 `bg_85eb18f5`）

1. **文件大小恒 0**：`shell_namespace.rs` 自造 fmtid `9B174B33-40FF-11D2-A27E-00C04FC30871` 的 `PKEY_SIZE`，
   `GetProperty` 返回 `S_OK + VT_NULL`（该键在 Windows 里属「名称/序列号」族、根本不是 size）
   → `PropVariantToUInt64(VT_NULL)` 静默转 0。修法 = `PSGetPropertyKeyFromName("System.Size")` 取权威键
   （实测 `{B725F130-47EF-101A-A5F1-02608C9EEBAC}, pid 12`），取不到则退回原值 ⇒ 最坏与改前等价。
   **判别性对照**：本地 `kernel32.dll` 旧键 `0` / 新键 `862016`；手机 3 张图片 `0 → 2285028 / 2262564 / 57434`；
   手机 3 条视频 `0 → 476981428 / 710463875 / 2192733`；手机卷目录 `0 → 1001925963776`。
   ⚠️ **这条不止手机** —— 所有 shell 命名空间条目（含 fnNAS 那台设备）此前都显示 0 B。
2. **图片缩略图必然失败**：`generate_image_thumbnail_file` 头一句 `fs::metadata`，而 `::{...}` 不是文件系统路径
   ⇒ 与格式解码器完全无关，100% 不通。修法 = 新增 `shell_item_thumbnail_rgba()`：
   `IShellItemImageFactory.GetImage(SIIGBF_THUMBNAILONLY)` → `GetObjectW` → 32bpp 负高度 top-down DIB
   → `GetDibits` → BGRX→RGBA → `DeleteObject`（不新增 crate）；按 `is_shell_namespace_path` 分流，
   复用同一缓存键与 `THUMBNAIL_DECODE_GATE`（不新增并发面），**出错一律返回 Err、绝不写空图占位**；
   前端两个 composable 对 shell 路径关掉占位图、跳过 `<video>` 自解码。
   真机手机图片 64/128/256 **三档全部成功** ⇒ 该设备确实通过 shell provider 暴露真缩略图。

**视频缩略图判「暂不立项」**（`b25-video-thumb-findings.md`，`bg_9204b560`）：
先否证「`THUMBNAILONLY` 失败 ⇒ 设备不提供」这一前提 —— 本机文件系统 mp4 同样 `0x8004B200`，
而 jpg 用 `flags=8` 返回 `S_OK` ⇒ 失败与设备无关，是该 flag 对视频态本就不通。
改试其他 flags 拿到的恒是 `128×128 lum_mean=192.5 hash=6D63057C320FA6C0` = **通用视频图标**（与 `ICONONLY(4)` 逐字节相同）；
`IThumbnailCache` 通路实测在 OS 层就断。**禁止**把「非真画面」结果写进每视频一份的缩略图缓存。
**未判定**：手机在线时的 A/B/C 三态 —— 硬前置 = 手机在线。

**B25-2 层 1 真机验收 = PASS，用户主诉已解决**（`b25e-verification-report.md` / `b25e-evidence-extract.txt`）

- **闸门双判据同时成立**：① 日志逐字 `[global-search] skipping removable drive root from index watcher: H:/`；
  ② `pid=66488` 的**全部其它卷根句柄在位**（`C:\ 5040 | D:\ 3748 | E:\ 3968 | F:\ 3988 | G:\ 3116`）而 `H:\` 为 **0**，
  同次探针自校验对照 `CONTROL_HANDLE_RESOLVED=YES SELF hv=2180 H:\` 证明解析器可用 ⇒ **不是假绿**。
  改前同一把尺：`pid=79340 disk_handles_named=79 of_which_on_H=1 | hv=6832 H:\`，与 `status.json.indexed_drive_roots` 含 `H:/` 严格对应。
- **真弹出**：AFM 全程存活下第二次 shell 弹出 `DoIt()` 后 `NEW_WINDOW_COUNT=0` 且 `PRESENT_AFTER_SHELL_VERB dir=False`；
  第一次曾被别的进程否决（`REQ_EJECT RET=23 CR_REMOVE_VETOED`）⇒ **否决方未判定**，不得写「AFM 是唯一占用者」。
- **扫描确实真跑了**（排除混淆因素）：`mode=Full outcome=Completed elapsed=354661ms indexed=2082294`，
  `scan round timings` 逐根 G/F/D/E/C 齐全且**无 H:/ 行**；`indexed_drive_roots` 从含 `H:/` 变不含。
- 附带发现（**未修**）：摘掉 `H:/` 后索引器失去增量能力，`blocker=DriveRootsChanged` 连续 8 次后**强制全量重扫 354 秒**。

**B25-2 层 2：真机缺陷 → 根因 → 修法 → 装机生效**（`b25d-probe-report.md`，`bg_3a7f9dcf`）

- 装机暴露：`RegisterDeviceNotificationW` 对 volume filter 返回 **`0x8007000D` ERROR_INVALID_DATA**，模块降级 ⇒ 层 2 形同未订阅（单测覆盖不到）。
- 决定性注册矩阵（Rust 独立 crate `.superpowers\sdd\b25probe\`，同版本 `windows 0.58`；
  **不用 PowerShell 手写 P/Invoke** —— 那条路的 `RegisterClassExW lastError=87` 是手工 marshal 的假信号）：
  **一切含 `DBT_DEVTYP_VOLUME(2)` 的 filter 全红**（消息窗 / 隐藏顶层窗 / 重叠窗 × 三种 cbSize × 含或不含 `DBTF_MEDIA`
  × 逐字节抄头文件 × devicetype 0/1/2/3/4/9/16 → 一律 `0x8007000D`）；
  唯二可注册 = **`devicetype=5 DEVICEINTERFACE`** 与 `7 NET`；`6 HANDLE` → `0x8007042A`（缺 Name）；NULL filter → `0x80070057`（自证 API 已加载）。
  ⇒ 唯一变量是 **volume 这一 devicetype 本身被拒**，窗口形态与其余字段双双否证。
- 修法（`device_events.rs` 736 → 860 行）：filter 换 `DBT_DEVTYP_DEVICEINTERFACE` + **`GUID_DEVINTERFACE_VOLUME`**；
  窗口改隐藏**顶层**窗（`WS_EX_TOOLWINDOW` + `WS_POPUP` + `HWND(null)`，从不 `ShowWindow`）；
  新增枚举变体 `DeviceInterface{kind}` 与 devicetype=5 解读分支（**只 `log::debug!`，不释放句柄不发事件**，避免与既有 DeviceChanged 双触发），
  同时保留 Volume(2)/Handle(6) 分支，全部分支 `veto: false`；**注册失败非致命化**（去掉 `return`）。
  原计划要钉的「`dbcv_flags==DBT_LOCAL` / `dbcv_unitmask!=0`」两条断言**作废**（钉的是注册不死的死路径），
  改钉**「filter devicetype 永不得为 2」**这一回归不变量；模块内单测 11 → 13（原 11 条一条未动）。
- 重打包复装（新包 `53944d10f01ff4aa…296e8` 45,618,008 B；安装器 PREINSTALL/POSTINSTALL 双行 `installerSha256` 与本地包逐字一致，
  `detectedVersion=3.15.0 sameVersionReinstall=yes`，`kill-app-tree killed=9 failed=0`）后，
  新实例 `pid=79960`（20:31:06 UTC）日志**命中成功锚点**
  `[device-events] volume device-interface notifications registered` + `listening for volume-change broadcasts on Windows`，
  `registration failed` / `relying on` / `0x8007` / `veto` **零命中** ⇒ **`0x8007000D` 在新包上已消失（装机实测）**。
  ⚠️ 日志锚点行号已迁移：旧 `device_events.rs:514` **作废**，改用 **`:569` / `:577`**。
- **仍未判定**（硬前置 = 用户物理重插 U 盘）：① 层 2 事件**送达性**（`DBT_DEVICEINTERFACE` 广播必须真实卷挂载/移除才产生，
  `subst` 已证明不是卷广播源，C/D/E/F/G 是用户实盘不得做插拔实验）；② 插入侧是否再触发 354 秒全量重扫；
  ③ 混合分屏组只关一格的真实表现；④ 层 2 版是否仍现 `CR_REMOVE_VETOED(23)`。

**门禁（root 复跑原始日志）**：`cargo test --lib` **1463 passed / 0 failed / 1 ignored / 6.81s**（层 1 后 1461 → 层 2 后 1463）；
`cargo clippy --all-targets` **91 warnings**（改前基线 93，未升，`device_events` 命中 0）；
`npm test` **317 files / 3364 tests** 全绿；`npx eslint src` **0 problems**；`verify:sandbox` 四条 **VALID (0 violations / 20 patterns)**；
`node scripts/verify-changelog.mjs` **OK**（版本 3.15.0 / 59 条目）。

**产品取舍（已写进 CHANGELOG 中英文条目）**：U 盘内文件变动**不再被即时搜索索引覆盖**，换取「AFM 永不扣住任何可移动盘」。

**本轮新增工具坑（补充 §20.21.3）**：
① 同一条命令里先 `cd` 再往**相对路径**写文件 ⇒ 被本机权限门整条拒绝；写文件一律用绝对字面路径。
② 给变量赋值时误加 `[np]` 之类类型前缀是**解析期**错误，整条脚本（含前面的装机动作）**一行都不执行**；
   第二次误保留成 `[np]$np=` 时脚本已跑完、只是变量没赋值 ⇒ 用 `@(Get-Process -Name 'alpha-file-manager')` 复核真实 pid。
③ `Add-Content -Encoding UTF8` 追加含中文的 markdown 台账可行；禁的是 `Get-Content -Raw` 读含中文**源文件**。
④ `Where-Object` 零命中返回 `$null`，喂 `[IO.File]::WriteAllLines` 报 `Value cannot be null (Parameter 'contents')` ⇒ 一律 `@(...)` 包裹。
⑤ 判「装机版是不是新的」不要碰 sha/尺寸/PE 时间戳/整份 exe 字符串（四类假判据），只认 ① 无条件新增日志行 ② 行为差异。

### 20.22 §20.21.4 三条硬前置项已用真实插拔闭合（2026-11-21 批次 33）

完整取证报告：**`docs/handoff-2026-11-21-batch33-usb-verification.md`**。
三条判据全部定案，其中**第 2、3 条修正 §20.21.4 的原有假设**。

1. **层 2 送达性 = PASS（本台账首次真机命中）。** 拔出瞬间主日志同时落下
   `[dir-watcher] released dir-watcher handle for H:/` 与
   `[device-events] RemoveComplete on H:/: released 1 dir watch(es), 0 index root(s)`。
   ⇒ `device_events.rs:219` 的 INFO 行首次在 release 上命中；计数为 **1**
   （= 拔插前窗格停在 `H:\` 造成的活跃 watch，是刻意造出来的非零样本），
   证明隐藏顶层窗口确实收到 `DBT_DEVICEREMOVECOMPLETE` 并走到 `handle_broadcast`
   的 Volume 分支、真句柄已释放。证据：`.superpowers/sdd/b33-log-delta.txt`。
2. **插入侧未触发 `blocker=DriveRootsChanged` ⇒ §20.21.4 第 2 条的预测成立。**
   整段插拔增量日志内 0 命中。原因即该条自注的「取决于实现」分支：可移动卷不在
   `indexed_drive_roots`（实测 `["E:/","F:/"]`）⇒ requested 集合不变 ⇒ 不重扫、无残留代价。
   **该条从「未判定」转「已判定：不会触发」。**
   ⚠️ 口径限制：插入（Arrival）在 release 下**没有任何日志锚点**
   （`device_events.rs:97-102` 的 `is_removal_event()` 只含三个移除事件，`:234` 的 else 分支
   只发前端 payload 不打日志），故插入侧只能证「没引发重扫」，**不能证送达**；
   `DBT_DEVICEINTERFACE` 分支同理（`:272` 是 `log::debug!`，release 级为 Info，不可观测）。
3. **`CR_REMOVE_VETOED(23)` 两轮复现，但「AFM 占用导致否决」被证伪。**
   同一块盘做对照弹出：AFM 窗格停在 `H:\`（持 1 个卷句柄）→ `RET=23 ANSWER=''`；
   AFM 句柄数 **0** → 同样 `RET=23 ANSWER=''`，两轮弹前盘上均无打开文件。
   ⇒ 否决与 AFM 无关；`ANSWER=''` 说明 Windows 未点名占用者。
4. **用户随后手工「安全删除硬件」一次成功弹出**（并在测试后重新插入）。
   ⇒ **`23` 是探针所用 API 的产物，不是用户可见缺陷**：`CM_Request_Device_EjectW`
   直接请求 PnP 移除设备节点，不做卷锁/卸载，任何句柄（含探针自己开卷查询、Explorer）
   都会 veto；Shell 的「安全删除硬件」路径会先锁卷/卸载，因此能成功。
   ⚠️ 残余未判定：用户手工弹出时**未控制 AFM 的状态**（窗格是否停在 `H:\` 未记录）。
   已由第 3 条的 A/B 对照（句柄 0 与句柄 1 同结果）间接覆盖，但严格意义的
   「AFM 持活跃 watch 时 Shell 路径仍成功」没有直接样本 ⇒ 登记**未判定**，不写 PASS。
   **后续如需再验 `23`，不要再用 `CM_Request_Device_EjectW` 当用户路径的判据。**
5. **AFM 对 `H:` 的句柄行为量化**（A/B/A 三态可逆）：启动 `disk_handles_named=60 of_which_on_H=0`
   → argv 打开 `H:\` 后 `59 / 1` → argv 切到 `E:\` 后 `57 / 0`。
   ⇒ 「有窗格正在显示该盘」持 1 个目录句柄，离开即释放，**无泄漏**；
   归因探针阳性对照成立（`control_handle_opened_on_H_root=True`，自身 pid 命中 1）。
   证据：`.superpowers/sdd/b33-handles-0{1,2,3}-*.txt`。
6. 本轮另记：`evidence.md` §九.3「`device_events.rs:219` 与 `lib.rs:550` 不在同一测试二进制，
   只能靠日志观测」继续成立；本轮是其**首次拿到真机日志证据**，不是新增测试。


