# 阶段 1 实现计划：百度网盘接入 AFM「远程位置」

- 分支：`feat/baidu-remote-provider`（基线 `main` = `6eabbc67`）
- 文档时间：2026-11-08
- 前置文档：`docs/2026-11-07-baidu-netdisk-drive-feasibility.md`（可行性与工作量分析）
- 预估：8–10 人天
- 用户已批准：路径 A（复用 `sfm://remote/`），语义为「侧边栏『远程位置』里出现『百度网盘』」

---

## 0. 一句话

把百度网盘实现成 `RemoteProtocol` 的**第 5 个变体** `BaiduPan`，复用既有 `sfm://remote/<连接 id>/<路径>` 整套 VFS 与前端通道，得到的效果是：新建一个「百度网盘」连接后，它自动出现在侧边栏「远程位置」，双击即可像本地目录一样浏览、复制、拖拽。

**不需要新造「虚拟驱动器」概念。** 既有远程连接已经会出现在侧边栏「远程位置」列表里（`remote-nav-panel.vue`），只要多认一种协议，它就自然长在那里。这一条是本计划相对于上一版设想的**简化**，也是把它压在 8–10 人天的关键。

---

## 1. 范围

### 做（阶段 1）

1. **应用内授权**：内嵌 WebView2 打开 `pan.baidu.com`，用户扫码/账号密码登录，应用自己取凭据并加密落盘。
2. **列目录**：`read_dir`，含分页（百度一页上限 1000 条，目录超过要翻页）。
3. **下载**：单文件与多文件，复用 `transfer::download` 的进度事件。
4. **文件管理**：新建文件夹、重命名、删除（进回收站语义与永久删除按百度 API 分别映射）。
5. **连接设置表单**：百度协议专属的授权卡，而不是 host/port/密码那一套。
6. **错误分类**：所有失败必须落到既有 11 个 `RemoteErrorKind` 之一，不新增 kind。

### 不做（阶段 2 / 3）

| 项 | 归属 | 理由 |
|---|---|---|
| 上传（拖进来、粘贴进来） | 阶段 2 | 需要上传分片 + 秒传判定，独立一轮 |
| `set_permissions` / `get_permissions` | 永不 | 百度没有 POSIX 权限位，必须返回 `Unsupported` |
| 分享链接、离线下载、秒传 | 永不 | 与文件管理无关 |
| 流式下载（`read_file_stream`） | 阶段 3 | 要改 `RemoteProvider` trait，波及 4 个既有 provider |
| 多账号同时在线 | 不做 | 多账号 = 建多条连接，token 各存各的 |

---

## 2. 架构决策

### D1 用 `RemoteProtocol` 新变体，不新建子系统

**理由**：`sfm://remote/` 这条链已经全部接线并经过真机验收 —— 目录树根（`remote-tree-roots.ts`）、地址栏、面包屑、双屏、右键菜单、传输引擎（`transfer.rs` 1955 行）、比较引擎（复用 `provider_with_protocol`）。新建子系统等于把这些各抄一遍。

**代价**：`provider_with_protocol`（`commands.rs:386`）的 `match` 是穷尽的，加变体后**编译器会在所有需要改的地方报错**。这正是我们要的：改动清单由编译器给，不靠人工找。

### D2 认证走「内嵌 WebView2 登录 + 一次性换 token」，不手抄 cookie

**理由**：`BDUSS` 是百度账号的**最高权限凭据**（等价于登录态本身），让用户从浏览器 F12 复制等于每次换设备都要交一次钥匙。改用 `refresh_token`：可撤销、可换发、不暴露会话。

**风险**：Tauri 能不能读内嵌 webview 的 cookie 是本计划的**第一号未知**（见 §4 的 U1）。读不到就走降级路径，不影响其余部分开工。

### D3 阶段 1 不实现 `write_file`，但必须明确报错

`write_file` / `write_file_with_progress` 返回 `RemoteError::unsupported("百度网盘的上传将在下一阶段支持")`。

**绝不静默成功** —— 这是 `provider.rs` 顶部已经写死的规矩（`set_permissions` / `remove_path` / `rename_path` 对不支持的协议必须返回明确 `Protocol`/`Unsupported` 错）。静默成功会让用户的文件凭空消失且没有任何提示。

### D4 不往 `RemoteConnection` 加新字段

百度的所有专属状态（`refresh_token`、缓存的 `access_token`、到期时间、账号昵称）**都不进 `user-remote-connections.json`**：

- `host` 复用为固定值 `pan.baidu.com`（不可编辑）
- `username` 复用为授权后回填的账号昵称（只读展示）
- `port` 复用为 `443`
- `credential_key` = 连接 id（指向 DPAPI 密文）
- 其余全在密文档里

**理由**：加字段要同时改读白名单（`USER_STATS_TOP_LEVEL_KEYS` 那类）与写列表，AGENTS.md 已经记着「新建收藏分类 → 重启即丢」那次事故。能不加字段就不加。

---

## 3. 数据模型改动

### 后端 `src-tauri/src/remote/connection.rs`

```rust
pub enum RemoteProtocol {
    Ftp,
    FtpsExplicit,
    FtpsImplicit,
    Sftp,
    // 新增
    BaiduPan,
}
```

- `as_str()` → `"baidu_pan"`
- `default_port()` → `443`
- `validate()`：百度变体**跳过** `host` 白空格校验之外的服务器校验（`host` 是常量，`port` 是常量），改为校验 `credential_key.is_some()`，缺失时报 `RemoteError::auth("尚未授权百度网盘")`

### 前端 `src/types/remote.ts`

```ts
export type RemoteProtocol = 'ftp' | 'ftps_explicit' | 'ftps_implicit' | 'sftp' | 'baidu_pan';
```

- `getRemoteProtocolDefaultPort` 加 `case 'baidu_pan': return 443`
- 新增 `export function isOAuthRemoteProtocol(p: RemoteProtocol): boolean`
- 新增 `export function protocolNeedsAuthorization(p: RemoteProtocol): boolean`（目前只有百度为真）

### 不需要改的

- `user-remote-connections.json` 顶层键（仍是 `connections`）
- `src-shared/remote-connection.sample.json` 跨语言护栏 fixture（字段没变）
- 读/写白名单

---

## 4. 认证链路与三个未知

### 目标链路

```
[设置页] 点「授权百度网盘」
   ↓
[内嵌 WebView2] 打开 https://pan.baidu.com/ → 用户扫码/账号密码登录
   ↓
[应用] 从 webview cookie 存储读 BDUSS                       ← U1
   ↓
[应用] BDUSS → oauth code（redirect_uri=oob）               ← U2
   ↓
[应用] code → refresh_token + access_token
   ↓
[应用] refresh_token 写 DPAPI 密文档（键 = 连接 id）
   ↓
[应用] method=uinfo 取账号昵称 → 回填 RemoteConnection.username
   ↓
[应用] 关窗，表单显示「已授权：<昵称>」
```

### 三个必须先实测的未知（T1 的交付物）

| 编号 | 未知 | 不成立的后果 |
|---|---|---|
| **U1** | Tauri v2 的内嵌 `WebviewWindow` 能否读到 cookie（`BDUSS`） | 降级：设置页给一个「粘贴 BDUSS」输入框（+0.5 人天） |
| **U2** | `BDUSS` → `code` 这一步用哪个 `client_id`、走 `getquan` 还是 `authorize` | 降级：直接要求用户粘贴 `refresh_token`（+0.5 人天） |
| **U3** | `access_token` / `refresh_token` 实际有效期与刷新语义 | 只影响缓存策略，不阻塞 |

**U1 的实测方式**：写一个 `#[ignore]` 的手动测试或一个临时命令，在真机打包产物里跑（`tauri dev` 下所有路由页空白，见 AGENTS.md）。**不要**在 `cargo test --lib` 里试 —— 那里一碰 `tauri::` 就 `STATUS_ENTRYPOINT_NOT_FOUND`。

**U2 的合规提示**：官方开放平台需要企业实名且限制 `/apps/{应用名}/`（已在可行性文档中坐实），所以本计划走的是**网页端非官方接口**。这条路不稳定、可能被封，`client_id` 的来源必须写进代码注释，不接受「社区抄来的」这种无来源结论。

---

## 5. 逐文件改动清单

### 后端

| 文件 | 动作 | 规模 | 要点 |
|---|---|---|---|
| `src-tauri/src/remote/baidu_api.rs` | 新增 | ≈400 行 | 纯 HTTP + 纯函数解析层，**零 `tauri::` 路径** |
| `src-tauri/src/remote/baidu_auth.rs` | 新增 | ≈350 行 | token 交换、刷新、内存缓存，**零 `tauri::` 路径** |
| `src-tauri/src/remote/baidu_provider.rs` | 新增 | ≈700 行 | `impl RemoteProvider for BaiduRemoteProvider` |
| `src-tauri/src/remote/mod.rs` | 改 | +6 行 | 三个 `mod` + `pub use` |
| `src-tauri/src/remote/connection.rs` | 改 | +30 行 | 变体、`as_str`、`default_port`、`validate` 分支 |
| `src-tauri/src/remote/commands.rs` | 改 | +25 行 | `provider_with_protocol` 加分支；新增 `remote_baidu_authorize` / `remote_baidu_auth_status` 两个命令 |
| `src-tauri/src/lib.rs` | 改 | +2 行 | `generate_handler!` 注册新命令 |

`baidu_api.rs` / `baidu_auth.rs` / `baidu_provider.rs` 三个文件顶部必须照抄 `mod.rs:8-18` 那段红线说明（`cargo test --lib` 链接，不得出现 `tauri::`）。

**前置确认**：`src-tauri/Cargo.toml` 里是否已有 `reqwest`（`src-tauri/src/extensions/http.rs` 用了 HTTP 客户端）。若没有，加 `reqwest` 是**新增第三方依赖**，需要过一遍合规审查；备选是复用 `extensions` 那层的客户端能力。

### 前端

| 文件 | 动作 | 要点 |
|---|---|---|
| `src/types/remote.ts` | 改 | 协议联合类型 + 两个判定函数 |
| `src/modules/settings/ui/categories/remote/connection-form.vue` | 改 | `PROTOCOLS` 加项；百度分支**隐藏** host/port/username/密码框，改显示授权卡 |
| `src/modules/settings/ui/categories/remote/remote-authorize-card.vue` | 新增 | 授权按钮、账号昵称、失效重授权 |
| `src/modules/remote/utils/remote-context-menu-visibility.ts` | 改 | `protocolSupportsPermissions('baidu_pan') === false` |
| `src/utils/remote-directory.ts` | 改 | `trustKindForProtocol('baidu_pan') === null`（无主机密钥概念） |
| `src/stores/storage/remote-connections.ts` | 改 | 保存时百度连接跳过 host/port 表单校验 |
| `src/localization/messages/*.json`（**17 个**） | 改 | `protocols.baidu_pan` + 授权文案；中文 locale code 是 `ch` |
| `src/utils/__tests__/remote-directory.test.ts` | 改 | 加 `baidu_pan` 分支断言 |
| `src/modules/remote/utils/__tests__/remote-context-menu-visibility.test.ts` | 改 | 同上 |

### 不需要改（明确写出来，防下一轮白跑）

- `src/modules/nav-sidebar/components/remote-nav-panel.vue` —— 它按连接列表渲染，不认协议
- `src/utils/remote-paths.ts` —— 路径规则与协议无关
- `src-tauri/src/remote/transfer.rs` —— 递归/冲突/进度规则协议无关（见该文件顶部）
- `src-tauri/src/remote/path.rs` / `dir_adapter.rs` / `registry.rs` / `credentials.rs`
- `src-tauri/src/remote/connection_test.rs` —— 百度不握手，不复用 `ProbeOutcome`

---

## 6. 任务拆解

| # | 任务 | 人天 | 验收判据 |
|---|---|---:|---|
| **T1** | 认证可行性实测 U1/U2/U3 | 1.5 | 一份实测记录（三问逐条给结论与证据）；一个 `#[ignore]` 手动测试或临时命令 |
| **T2** | `baidu_api.rs`：URL 拼装、分页、错误码映射，全部纯函数 | 1.5 | 单测全绿，**零网络**；错误码映射覆盖 11 个 kind 的穷尽表 |
| **T3** | `baidu_auth.rs`：token 缓存与刷新（内存 `Mutex<HashMap>`，过期前 5 分钟刷新） | 1.0 | 单测覆盖「未过期不刷新」「过期刷一次」「刷新失败分类为 Auth」 |
| **T4** | `baidu_provider.rs` 只读部分（`read_dir` / `read_file` / `get_permissions`） | 2.0 | 对真实账号手测：根目录、深目录、空目录、>1000 项目录 |
| **T5** | 写操作（`make_dir` / `remove_path` / `rename_path`）+ `write_file`/`set_permissions` 明确 Unsupported | 1.0 | 单测断言 Unsupported 分支不是静默成功 |
| **T6** | 命令接线 + `provider_with_protocol` 分支 | 0.5 | 编译通过；`cargo test --lib` **不崩**（红线） |
| **T7** | 前端表单 + 授权卡 + 两个判定函数 | 1.5 | 真机截图：协议下拉有「百度网盘」、表单切成授权卡 |
| **T8** | 语言包 17 个 + 前端单测 | 0.5 | 门禁零回归 |
| **T9** | 真机验收 + 打包 | 1.0 | 安装包 + 屏幕实拍（见 §7） |

T2 与 T3 可并行（不同文件、无依赖）。合计 10.5 人天，实际按 8–10 排（T8 并入 T7，T4/T5 可压）。

---

## 7. 验收判据

### 单测

- `cargo test --lib` 新增 ≥ 40 条，且**不出现** `STATUS_ENTRYPOINT_NOT_FOUND`
- `npm run test:unit` 新增 ≥ 10 条
- 关键反例（必须有，缺一条就算没做完）：
  - `.` / `..` 段必须被 `normalize_remote_path` 拒 → `RemoteErrorKind::Protocol`
  - `write_file` 必须返回 `Unsupported`，**不能返回 `Ok(())`**
  - `set_permissions` 必须返回 `Unsupported`，**不能返回 `Ok(())`**
  - `get_permissions` 返回 `Ok(None)`（协议无权限位），不是 `Err`

### 门禁

- `eslint .` 用 `git stash push -u` 量 HEAD 真实基线后逐条对照（基线数字随提交变，别拿旧数字对）
- `npm run tauri:build` 成功

### 真机（**必须打包产物，`tauri dev` 的界面验收一律作废**）

1. 设置 → 远程 → 新建连接 → 协议下拉出现「百度网盘」
2. 选它 → host/port/密码框消失，出现「授权百度网盘」按钮
3. 点授权 → 内嵌窗口打开百度 → 扫码登录 → 关窗后卡片显示账号昵称
4. 保存 → **侧边栏「远程位置」出现该连接**（用户需求的原话落点）
5. 双击 → 列出根目录，条目名/大小/时间正确
6. 进两层子目录 → 地址栏与面包屑正确
7. 右键一个文件 → 下载到本地 → **文件落地且 sha256 与原文件一致**
8. 复制一个远端文件 → 粘贴到本地目录 → 落地一致
9. 新建文件夹 → 出现在百度网盘网页端
10. 重命名 → 网页端同步改名
11. 删除 → 网页端进入回收站
12. 右键菜单里**没有**「权限」类项
13. 断网 → 提示是分类文案（如「网络不可达」），不是「未知错误」
14. 手动让 `refresh_token` 失效（网页端撤销授权）→ 提示重新授权，不是静默失败

### 数据安全（验收前必做）

- `remote-credentials.json` 是**新增风险点**：授权会真的写进用户的密文档。验收前备份整个 `user-data/` 目录
- 恢复顺序**必须先杀进程再 restore**（AGENTS.md：应用退出时会把内存状态写回，先 restore 会被覆盖）
- `dash-accept-data.cjs backup` 的语义是「备份不存在才建」，**跑完必须看它打印的字节数**

---

## 8. 风险清单

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| U1 不成立（读不到 cookie） | 中 | T1 卡住 | 降级粘贴 `BDUSS`，+0.5 人天 |
| U2 不成立（换不到 token） | 中 | 同上 | 降级粘贴 `refresh_token` |
| 非官方接口被封或改字段 | 中 | 功能失效 | 错误映射里单列「协议已变更」文案；分支可整体丢弃 |
| 下载限速 | 高 | 体验差 | **不做任何绕过**；UI 显示真实速率与已下载量 |
| 大文件内存峰值 | 中 | 几百 MB 会吃满内存 | `read_file` 整份进内存是既有 trait 限制；阶段 1 加 500 MB 硬上限，超限返回 `Unsupported` 并提示用官方客户端 |
| `reqwest` 不在依赖树 | 低 | +0.5 人天 | T2 开工前先看 `Cargo.toml` |
| 百度根目录层级差异（网盘是 `/`，而「我的资源」/「分享」是并列入口） | 中 | 用户找不到文件 | 根目录只列 `method=list` 能给的；「分享」这类非文件入口**不伪装成目录** |

---

## 9. 与既有铁律的对照

| 铁律 | 本计划的落实 |
|---|---|
| `cargo test --lib` 不得链接 `tauri::` | 三个新文件全部零 `tauri::`；所有 `AppHandle` 只在 `commands.rs` |
| 真机验收用打包产物 | T9 走 `npm run tauri:build` |
| 需要装机实测就直接打包 | T7/T9 一做完就起构建，不等用户开口 |
| 改 `.vue`/`.ts`/`.json` 用编辑工具 | 语言包 17 个用 `node` 脚本按 UTF-8 读写，改完验 `\uFFFD` 为空 |
| 面向用户文案简体中文 | 语言包中文值走 `ch.json`；代码标识符保持原样 |
| 受保护常量不动 | 不碰 mDNS 服务名、legacy 数据目录名、Tauri `identifier` |