# 百度网盘接入 AFM 的可行性分析 + 工作量评估

日期：2026-11-07
状态：**分析完成，待方向确认**（本文档不是实现计划）

---

## 0. 一句话结论

**「只做一个扩展」在现有 AFM 架构下做不到**——扩展权限清单里没有任何「注册驱动器 / 挂载文件系统」能力，扩展 worker 也无法实现 Rust 侧的 `RemoteProvider` trait。

但 **AFM 核心已经有一套完整的远程文件系统骨架**（`src-tauri/src/remote/`，约 13,000 行，FTP/FTPS/SFTP 三个 provider 跑在上面）。新增一个 `BaiduRemoteProvider` 复用这套骨架，是技术上最省、体验最好的路径：**核心改动约 2,000–2,800 行 Rust + 600–900 行前端**，扩展只负责「授权引导 + 连接管理」这类可选壳。

同时有一个**必须先跟用户对齐的语义分歧**：所谓「显示为本地一个驱动器」是指

- **(A)** AFM 侧边栏「远程位置」里出现一个「百度网盘」条目，双击进去像本地目录一样浏览、复制、拖拽 —— 复用 `sfm://remote/`，**不需要任何驱动、不需要管理员权限**；
- **(B)** Windows 资源管理器里真的多出一个盘符（如 `Z:`）—— 需要另起一个本地 WebDAV/SMB 服务再 `net use` 映射，或引入 WinFsp 驱动。

这两条的工作量差 3–5 倍，且 (B) 的体验会明显劣于 (A)。下面的分析两条都覆盖。

---

## 1. 决定性约束：官方开放平台做不到

这是整个分析的**前提性事实**，先摆出来，因为它一票否决了「走正规渠道」的路径。

百度网盘开放平台的第三方应用，**只能读写 `/apps/{应用名}/` 这一个目录**，无法访问用户网盘里已有的文件。

> 「每个第三方应用在网盘只能拥有一个文件夹用于存储上传文件，该文件夹必须位于 /apps 目录下……这步骤非常重要，我们只能对这个文件夹里面的东西进行操作。」
> —— 稀土掘金《NestJS 实战 09：在 NestJS 中如何调用百度网盘 API》

> 「文件访问限制: 应用只能访问 /apps/{appname}/ 目录下的文件」
> —— GitHub `yzbtdiy/BaiduPanClient` README「API 限制说明」

> 「后来发现百度网盘不提供对网盘上所有目录的访问权限，只能访问 `/apps/$app_id/` 这个目录的访问权限……所以就放弃了。」
> —— GitHub `cztchoice/BaiduNetdiskPython` README

三条独立来源一致。用户看到的是 `/我的应用数据/{应用名}/`。

**结论**：用官方 OAuth + 开放平台 API，做出来的东西是「网盘里多了一个 AFM 专用文件夹」，不是「把百度网盘挂成驱动器」。这跟需求不符，直接排除。

另外开放平台还要求**企业实名认证**（个人开发者无法申请网盘 API 权限），并且需要应用审核 —— 即使愿意接受 `/apps/` 限制，资质也是一个外部卡口。

### 那 AList / BaiduPCS-Go 是怎么做到的

它们走的是**网页端接口**（`pan.baidu.com/rest/2.0/xpan/*`），凭据是网页版的 `refresh_token`（由浏览器 Cookie 里的 `BDUSS` 换取），这个 token 对应的 scope 是百度自家的「网盘管家」应用，因此**能访问全部文件**。

技术上完全可行，社区已有成熟实现可对照（AList 的 baidu driver、`qjfoidnh/BaiduPCS-Go`）。

代价要说清楚：**这条路不是官方授权渠道**，属于对网页接口的非官方调用。它对个人工具没影响，但如果 AFM 是商业分发产品，需要在协议层面评估风险（下面 §5）。

---

## 2. 现有架构：AFM 已经有的东西

### 2.1 后端远程文件系统骨架（这是本次最大的资产）

`src-tauri/src/remote/`，实测行数：

| 文件 | 行数 | 作用 |
|---|---:|---|
| `transfer.rs` | 1,955 | 递归传输编排（上传/下载/进度/冲突） |
| `ftp_provider.rs` | 1,617 | FTP provider 实现（最完整的参照样板） |
| `ftps_provider.rs` | 1,387 | FTPS provider |
| `connection_test.rs` | 1,167 | 连接测试 |
| `transport.rs` | 918 | 传输底座 |
| `sftp_provider.rs` | 891 | SFTP provider（**最简参照样板**） |
| `fake.rs` | 812 | 测试用 Fake provider |
| `commands.rs` | 708 | Tauri command 层 |
| `known_hosts.rs` | 607 | SSH 主机指纹 |
| `ftp_parsing.rs` | 545 | FTP 协议解析 |
| `credentials.rs` | 520 | 凭据存储 |
| `path.rs` | 441 | `sfm://remote/<连接id>/<路径>` scheme |
| `connection.rs` | 407 | 连接模型 |
| `error.rs` | 326 | 错误类型 |
| `provider.rs` | 310 | `RemoteProvider` trait + `RemoteEntry` |
| `registry.rs` | 308 | 连接注册表 |
| `dir_adapter.rs` | 163 | `RemoteEntry` → 本地 `DirEntry` 适配 |
| `mod.rs` | 92 | 模块声明 |

`RemoteProvider` trait（`provider.rs:40`）方法集：

```
read_dir / read_file / write_file / write_file_with_progress
make_dir / set_permissions / get_permissions / remove_path / rename_path
```

`#[async_trait]` + `dyn` 兼容，已有单测证明可以当 trait object 用（`providers_are_usable_through_a_trait_object`）。

**这意味着**：只要写一个 `BaiduRemoteProvider` 实现这 9 个方法并注册进 `registry.rs`，就能自动获得：

- `sfm://remote/` 路径体系与面包屑导航
- 文件浏览器、目录树、缩略图、属性面板
- 双向传输（含进度、冲突处理、断点逻辑）
- 凭据加密存储
- 现有全部远程相关单测契约

**不需要**改文件浏览器、不需要改导航器、不需要新写传输逻辑。

### 2.2 前端已有的远程 UI

`src/` 下与远端相关的非测试文件约 28 个，关键几个：

- `src/modules/nav-sidebar/components/remote-nav-panel.vue` —— **侧边栏「远程位置」面板**，这就是「显示为驱动器」在 AFM 内的落点
- `src/modules/settings/ui/categories/remote/connection-form.vue` —— 连接表单
- `src/stores/storage/remote-connections.ts` —— 连接状态
- `src/modules/navigator/utils/remote-tree-roots.ts` —— 目录树根节点
- `src/types/remote.ts`、`src/utils/remote-paths.ts`

### 2.3 扩展系统能做到什么（实测）

`src-tauri/src/extensions/manifest_permissions.rs` 的权限清单实测只有：

```
commands / contextMenu / toolbar / shell / notifications / dialogs
http(带 hosts 白名单) / fs.read / binaries
```

**没有**「注册驱动器」「挂载」「文件系统提供者」这三类能力中的任何一个。扩展运行在 worker 里（`extension-worker.ts`），只能通过 `create-fs-api.ts` 拿到**受限的本地 fs 读写**，无法向后端注册一个虚拟文件系统。

现存的两个扩展也印证了这个边界：

- `extensions/kizemo.focus-sync`：自带 `bin/focus-sync-sidecar.exe` + NSIS 安装脚本 + 计划任务，用 `http` 权限访问 `http://127.0.0.1:37421`。**它是「外挂一个本地服务」，不是「接入文件系统」**。
- `extensions/kizemo.git`：纯 shell 调 git。

⇒ **纯扩展方案否决**。这跟「愿不愿意多做」无关，是权限模型上不存在这条缝。

---

## 3. 三条候选路径

### 路径 A：核心新增 `BaiduRemoteProvider`（复用 `sfm://remote/`）

**做法**：在 `src-tauri/src/remote/` 下新增 `baidu_provider.rs`，实现 `RemoteProvider`；在 `registry.rs` 注册 `RemoteKind::Baidu`；在 `path.rs` 支持解析；前端 `connection-form.vue` 加「百度网盘」一种连接类型 + 授权流程。

**「显示为本地驱动器」的落地形态**：AFM 侧边栏「远程位置」下出现「百度网盘」，双击进入后与本地目录同样的浏览/复制/拖拽/排序体验。

**优点**
- 复用 13,000 行既有基础设施，无需新写传输、导航、树、进度
- 零驱动、零管理员权限、零外部进程
- 可打包进安装包，用户装完即用
- 与 FTP/SFTP 行为一致，维护成本可控

**缺点**
- 不是 Windows 真盘符，资源管理器里看不到
- `RemoteProvider::read_file` 是整份读进内存（已知限制），超大文件要额外处理流式
- 百度侧限速会直接暴露在 UI 上

**工作量估算（人天，按熟悉本仓库的节奏）**

| 工作项 | 内容 | 估算 |
|---|---|---:|
| 凭据与授权链路 | BDUSS → web `refresh_token` 换取、token 刷新、失效重试、凭据加密落盘（复用 `credentials.rs`） | 3–4 |
| 目录与元数据 | `read_dir`（`xpan/file?method=list`）、路径规范化、`RemoteEntry` 映射、分页、错误码映射 | 2–3 |
| 下载 | `filemetas` 取 `dlink`、加 `User-Agent: pan.baidu.com`、Range 分片、限速与重试 | 3–4 |
| 上传 | `precreate` 秒传判定 → 分片上传 → `create` 合并，接 `write_file_with_progress` | 4–6 |
| 文件管理 | 删除 / 移动 / 重命名 / 复制 / 配额查询 | 2–3 |
| 注册与接线 | `registry.rs`、`path.rs`、`commands.rs`、`dir_adapter.rs` 对接 | 1–2 |
| 连接表单与授权 UI | `connection-form.vue` 新增类型、内嵌 WebView2 扫码/登录、状态提示 | 3–4 |
| 测试 | 单测（Fake 风格）+ 门禁 + 真机验收 | 3–4 |
| **合计** | | **21–30 人天** |

如果要压缩，可以砍掉「内嵌登录」，改让用户手动粘贴 `BDUSS`/`refresh_token` —— 省 2–3 天，但体验明显变差。

### 路径 B：sidecar WebDAV + `net use` 映射真盘符

**做法**：扩展（复用 focus-sync 的 sidecar 模式）拉起一个本地 WebDAV 服务，把百度网盘暴露为 `http://127.0.0.1:PORT/dav`；再用 `net use Z: \\127.0.0.1@PORT\DavWWWRoot` 映射盘符。

- 现成实现：**AList / OpenList**（AGPL-3.0，40+ 网盘聚合，baidu driver 成熟）
- Windows 的 WebDAV 客户端是 `WebClient` 服务（默认**未启动**，需要 `sc config WebClient start= auto` 或手动启动，通常要管理员）

**优点**
- 真的是盘符，资源管理器、Office、任何第三方程序都能用
- AFM 侧几乎零改动（现有「网络驱动器」就是 `net use`）

**缺点**
- **性能灾难**：WebDAV 是同步小块随机读语义，而百度网盘每次取数据的往返延迟 200ms+。打开一个含 500 张图的目录，资源管理器会连续拉缩略图 —— 实际体验会卡到不可用。AList 挂 WebDAV 给人用的场景基本都是「偶尔拷文件」，不是「日常当盘用」。
- 需要 `WebClient` 服务开机自启（管理员操作）
- 引入 AGPL-3.0 外部二进制（见 §5）
- 打包体积增加（AList 二进制 ~40MB，且需随扩展分发）
- 出问题时的故障面横跨「扩展 → sidecar → WebDAV → WebClient → 资源管理器」五层，极难排障

**工作量估算**：**8–12 人天**（主要是 sidecar 打包分发 + 服务拉起 + 安装器改造 + 盘符映射错误处理）。看起来比 A 便宜，但**产品质量不可接受**，我不推荐。

### 路径 C：WinFsp 用户态文件系统（真盘符 + 高性能）

用 `winfsp-rs` 写一个用户态 FS，直接调百度 API 实现 `readdir`/`open`/`read`/`write`。

**优点**：真盘符 + 按需流式读 + 缓存可控，是 RaiDrive / CloudDrive2 的技术路线，体验最好。

**缺点**
- 必须安装 WinFsp 驱动（需管理员 + 重启 + 可能触发杀软误报）
- 缓存/预读/并发控制是另一个量级的工程
- 与 AFM 现有架构完全脱节，等于新开一个产品

**工作量**：**45–70 人天**。不建议本轮做。

---

## 4. 推荐

**推荐路径 A**，理由三条：

1. 用户真实需求是「在 AFM 里像本地驱动器一样用百度网盘」，路径 A 直接满足；路径 B 给的是「资源管理器里的真盘符」，反而绕远且体验差。
2. 复用既有 13,000 行远程骨架，是最符合本仓库架构的解法；扩展系统也确实没有挂载能力，硬做要动权限模型，风险远大于收益。
3. 21–30 人天的量级与「只做一个扩展」的期望匹配，可以分阶段交付。

**分阶段建议**

- **阶段 1（8–10 人天）**：只读可用 —— 授权 + 目录浏览 + 下载 + 文件管理。能在 AFM 里浏览和拷出百度网盘文件。
- **阶段 2（7–10 人天）**：写入 —— 分片上传、秒传、覆盖/冲突。
- **阶段 3（4–6 人天）**：体验打磨 —— 内嵌扫码登录、限速提示、失败重试、进度细化。

如果想先验证价值，**阶段 1 结束就该给用户装包试用**。

**关于「扩展」这个形式**：如果一定要保留扩展，可以让扩展承担「授权引导 + 连接管理 + 状态展示」的壳（读 `credentials.rs` 暴露的 API），核心提供 provider。但这是可选加分项，不是必需——先有 provider 才有意义。

---

## 5. 风险清单

| 风险 | 影响 | 应对 |
|---|---|---|
| **非官方接口** | 百度随时可能改接口或封禁 UA；商业分发有协议风险 | 抽象成可替换 provider；准备「接口失效」降级提示；发版说明注明 |
| **限速** | 非会员下载 50–200 KB/s 甚至更低，大文件几乎不可用 | UI 明确提示「速度受百度账号权益限制」；不做加速（那是灰色手段） |
| **`dlink` 有效期短** | 下载 URL 有时效，长传输中断后需重新获取 | 传输层实现 dlink 刷新重试 |
| **`read_file` 整份进内存** | 大文件 OOM 风险 | provider 内做分块下载 + 落临时文件；必要时给 trait 加流式方法（波及 FTP/SFTP） |
| **`BDUSS` 属高敏凭据** | 泄露等于账号被接管 | 只用于换 `refresh_token` 后立即丢弃；token 走 `credentials.rs` 加密存储；日志严禁打印 |
| **`cargo test --lib` 红线** | 测试二进制不能碰 `tauri::async_runtime::*` | provider 全程用 `reqwest` + 普通 async，不引入 Tauri 运行时（与 `remote/mod.rs` 现有约定一致） |
| **AGPL 传染（仅路径 B）** | 分发 AList 二进制需提供对应源码 | 路径 B 才涉及；路径 A 无此问题 |
| **百度客户端版本漂移** | 本机 8.9.7.101，用户机上版本各异 | 路径 A 不依赖客户端，**这是它相对「逆向客户端」的额外优势** |

### 关于「用 REA 逆向 `D:/Program Files/BaiduNetdisk`」

调研后判断**没有必要**，理由：

- 本机装的是 **8.9.7.101**（Chromium/CEF 架构，`libcef.dll` 184MB、`kernel.dll` 56MB）。客户端自身的网络协议是私有加密的，逆向它得到的结论无法产品化——总不能要求用户装百度客户端再让我们去 hook。
- 而公开可用的接入点是**网页 API**，社区（AList、BaiduPCS-Go、PCS 系列）已有完整实现可对照，逆向带来的边际信息接近零。
- 唯一值得看的是 `YunShellExt64.dll` 等 Shell 扩展，但那是「资源管理器右键菜单集成」，与「挂载驱动器」无关。

⇒ 除非要做「直接读取百度客户端本地缓存/离线文件」，否则 REA 这一步可以跳过。**如果后续发现网页 API 有某个能力缺口，再定点逆向不迟。**

---

## 6. 待确认的决策

1. **「驱动器」的语义**：选 (A) AFM 内虚拟驱动器（推荐，21–30 人天），还是 (B) Windows 真盘符（8–12 人天但体验差），还是 (C) WinFsp（45–70 人天）？
2. **授权方式**：内嵌 WebView2 扫码/账号登录（体验好，+2–3 天），还是让用户手动粘贴凭据（快但难用）？
3. **交付节奏**：一次性做到可写，还是先交阶段 1（只读）装机验证价值？
4. **扩展是否保留**：是否需要我把「授权引导/连接管理」做成名义上的扩展壳（如果你希望产品话术上叫「扩展」）？

---

## 7. 证据来源

- AFM 代码（本机实测行数与结构）：`sigma-file-manager/src-tauri/src/remote/`、`src-tauri/src/extensions/manifest_permissions.rs`、`src/modules/nav-sidebar/components/remote-nav-panel.vue`
- 既有扩展形态：`sigma-file-manager/extensions/kizemo.focus-sync`、`extensions/kizemo.git`
- 网络驱动器历史结论：`sigma-file-manager/docs/handoff-2026-10-21-compare-sync-and-network-drive.md`（现有「网络驱动器」= `net use` 盘符映射）
- 开放平台 `/apps/` 限制：`yzbtdiy/BaiduPanClient` README、`cztchoice/BaiduNetdiskPython` README、掘金《NestJS 实战 09》
- 本机百度网盘：`D:/Program Files/BaiduNetdisk`，版本 `8.9.7.101`，无 AList/OpenList 安装痕迹