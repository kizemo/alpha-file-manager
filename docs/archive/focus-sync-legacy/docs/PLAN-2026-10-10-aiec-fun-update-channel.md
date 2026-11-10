# 实现计划 —— 更新渠道迁移到 aiec.fun(脱离 GitHub)

**撰写**:2026-10-10 10:10 · **修订**:2026-10-10 10:20(决策已拍板 + GitHub 兜底源勘察完成)
**需求**:Alpha File Manager 已脱离 sigma 主开发独立升级;因国内访问 GitHub 困难,把更新源迁到自建站 `https://www.aiec.fun`,在站点下建目录放安装包与版本信息,应用检查该站版本判断是否需要更新。
**参考项目**:`F:\soft\00selfmade\tiptap_app`(WeavePage),其 `docs/operations/{version-info,release-workflow,dual-channel-release}.md`
**当前状态**:**决策已定,代码尚未改动**

---

## 0. 已拍板的决策(2026-10-10)

| # | 决策 | 结论 |
|---|---|---|
| 1 | 站点目录名 | **`alpha-file-manager`** |
| 2 | 离线安装包 | **暂不做**,保持 17MB 引导下载(不切 offlineInstaller) |
| 3 | GitHub 兜底 | **保留兜底**,且兜底源改为**我们自己的仓 `kizemo/alpha-file-manager`**(现指向 sigma 原仓,必须换) |
| 4 | 完整性校验 | **`sha256`**,不引入 minisign |

---

## 1. 现状勘察结论(全部实测,非推测)

### 1.1 Alpha 现有更新机制

| 位置 | 内容 |
|---|---|
| `src-tauri/src/app_updater.rs:16` | `GITHUB_RELEASES_API_URL` = `https://api.github.com/repos/aleksey-hoffman/sigma-file-manager/releases?per_page=100` ← **指向 sigma 原仓** |
| `app_updater.rs:46 is_semver_app_release_tag` | tag 必须形如 `vX.Y.Z` 或 `vX.Y.Z-pre`,否则整条 release 被丢弃 |
| `app_updater.rs:97 pick_best_installable_upgrade` | 取版本号最高且有安装包资产的那条 |
| `app_updater.rs:191 validate_github_release_download_url` | **域名硬白名单**:仅 `github.com` / `*.github.com` / `*.githubusercontent.com`,HTTPS 强制,拒绝 localhost 与私有 IP(防 SSRF) |
| `app_updater.rs:251 pick_release_installer_asset` | Windows 版挑 `.exe`,优先含 `setup`/`nsis` 的 |
| `app_updater.rs:304 check_for_updates` | 10s 超时;错误文案硬编码「GitHub returned status」 |
| `download_release_installer` | 下载安装包到用户 Downloads 目录,**由用户手动运行** |
| `tauri.conf.json` | **无 updater 配置**;`version` 取自 `../package.json`(当前 `2.2.0`) |

**关键**:UI(`app-updates.vue`:当前版本 / 自动检查 / 立即检查 / 查看所有版本)与交互框架都是现成的,不必重做。

### 1.2 参考项目(WeavePage)方案 —— 已实测其线上产物可用

| 项 | 实测值 |
|---|---|
| 站点目录 | `https://www.aiec.fun/weavepage/`(**已实测可访问**,version.json 正常返回) |
| 服务器路径 | `/var/www/wordpress/weavepage/`(`47.120.26.175:22`,用户 `kizemo`) |
| 本地源 | `E:\办公文件\L1网站\weavepage\`(30 个安装包 + 6 个脚本文件) |
| SFTP 配置 | `E:\办公文件\L1网站\.vscode\sftp.json`(密码已配置,**不得显示**) |
| 上传脚本 | `_upload_weavepage_files.py`(paramiko,上传后 md5+size 双校验) |
| 站点文件构成 | `index.html`(跳转) + `download.php`(下载页,读 `downloads.json`) + `common.php` + `.htaccess` + `version.json` + 各版本 exe |
| 下载链接形态 | `download.php?file=xxx.exe`,**不是** exe 直链 |

**两个必须继承的实战细节**:

1. **cache-busting 必做** —— 站点在 Cloudflare 后面。参考项目的 `.htaccess` **没有给 version.json 设任何 cache 头**,因此吃 Cloudflare 默认 `max-age=7200`,不绕就**两小时拿不到新版本**,用户会以为更新坏了。
   > **实测修正(2026-10-10)**:该站 web 服务是 **nginx**,`.htaccess` 是 Apache 专有机制,**参考项目的 `.htaccess` 一直是摆设**,压根没生效。实测 Cloudflare 对 `version.json` 返回 `CF-Cache-Status: DYNAMIC`(边缘不缓存)。结论不变反而更强:**客户端 cache-busting 是唯一且充分的保障**,服务端配置不要再指望。
2. **`Accept-Encoding: identity` 必带** —— 防止 Cloudflare 对 JSON 做压缩触发客户端解码错误。

### 1.3 ⚠️ 参考项目自身并未闭环

`version-info.md` 白纸黑字写着:① `signature` 为空字符串 → Tauri 官方 updater 报 `UpdaterError(Crypto)`;② `tauri.conf.json` 未配 `plugins.updater` → 应用内根本不会触发检查。

即参考项目的「应用内自动升级」**实际是失效的**,只有浏览器下载链接可用。**只借鉴数据结构与运维脚本,不照抄完成状态。**

### 1.4 我们的 GitHub 仓现状(兜底源,实测)

- 仓地址:`https://github.com/kizemo/alpha-file-manager`(品牌仓 origin)
- `upstream` = `aleksey-hoffman/sigma-file-manager`(仅用于同步上游)
- **`private: false` → 公开仓,releases API 匿名可读,兜底可行**(私有仓则匿名返回 404,兜底会是死的)
- 现存 **1 个 release**:`v2.2.0-tree.1`(2026-09-26),资产仍是 **Sigma 品牌**的 `Sigma.File.Manager_2.2.0_x64-setup.exe`
- **该资产不会被误判为可升级**:`pre_release_label_weight("tree")` = 0(非 alpha/beta/rc),故 `parse_version_to_number("v2.2.0-tree.1")` = `200200000001`,而当前 `2.2.0` = `200200099999`,前者更低 → 被 `<= current` 过滤掉。已逐项核算。
- **但兜底目前给不出任何东西**:必须先发一个 Alpha 品牌的正式 release(见 A0),否则兜底源等于空仓

---

## 2. 方案(已定)

### 2.1 保留 Alpha 现有架构,只换数据源

不照搬参考项目的「前端 `tauri-plugin-http`」。**保留 Rust `reqwest`**:Rust 侧不受 CORS 限制,且已有 5s/10s 超时、错误处理与 10 个单测。

### 2.2 双源设计:站点为主,GitHub 兜底

| | 主源 | 兜底源 |
|---|---|---|
| 地址 | `https://www.aiec.fun/alpha-file-manager/version.json` | `https://api.github.com/repos/kizemo/alpha-file-manager/releases?per_page=100` |
| 形态 | 自定义 JSON | GitHub releases API(现有解析代码**原样复用**) |

**回落触发条件(关键设计,决定回滚开关是否有效)**:

- ✅ **主源「请求失败」** → 网络错误 / 非 2xx / JSON 畸形 → 回落兜底源
- ❌ **主源「解析成功但无更新」** → **不回落**

> 理由:若「无更新」也回落,就等于取两源版本更高者,那「把站点 version.json 的 version 改回去即可停止推送」这个**最廉价的回滚手段会立刻失效** —— GitHub 仍会推新版。用户要的「兜底」是可用性兜底(站点挂了还能更新),不是第二条推送通道。

**诚实说明**:在国内环境里,GitHub 不可达**正是它作为兜底存在的前提**,所以兜底实际只在「站点/服务器/CDN 故障」与「海外用户」两种场景下触发。它成本很低(复用现有解析代码),所以值得做,但**不要指望它在国内日常生效**。

### 2.3 不引入 minisign

Alpha 是「下载安装包 → 用户手动运行」,不是 in-app 自动安装,官方 updater 的强制签名要求不适用。改用 **sha256 校验和**写进 `version.json`,下载后本地比对,不匹配即报错并删除文件。

> 附带好处:**GitHub releases API 现在直接返回资产摘要**(`assets[].digest` = `sha256:...`,已在实测响应中看到),因此兜底源也能做同样的 sha256 校验,**无需给 GitHub 侧额外做发布流程**。

### 2.4 不做双渠道安装包

| | 参考项目 | **Alpha 实测** |
|---|---|---|
| setup.exe | 219–266 MB(内嵌 WebView2 离线包) | **17.94 MB**(引导下载,Tauri 默认) |
| update.exe | 5.7 MB | 不需要 |
| 差距 | 40+ 倍 | 差距很小,不值得多打一个包 |

Alpha 未配 `webviewInstallMode`,走 Tauri 默认引导下载。双渠道的收益前提不成立。

---

## 3. `version.json` 设计(Alpha 定制)

放在 `https://www.aiec.fun/alpha-file-manager/version.json`:

```json
{
  "version": "2.2.0",
  "notes": "v2.2.0 — 目录树刷新修复 / 首次启动按系统语言设语言 / 默认亮色主题 / 卸载残留清理",
  "pub_date": "2026-10-10T01:30:00Z",
  "sha256": "1b6a64f7a8d29fb0ab102e402294b674cb4cc30656ab21243aa759c0ef9c8635",
  "release_url": "https://www.aiec.fun/alpha-file-manager/",
  "platforms": {
    "windows-x86_64": {
      "url": "https://www.aiec.fun/alpha-file-manager/Alpha_File_Manager_2.2.0_x64-setup.exe",
      "signature": ""
    }
  },
  "history": [
    { "version": "2.1.0", "notes": "…", "pub_date": "2026-09-01T00:00:00Z",
      "url": "https://www.aiec.fun/alpha-file-manager/Alpha_File_Manager_2.1.0_x64-setup.exe",
      "sha256": "…" }
  ]
}
```

| 字段 | 作用 | 相对参考项目 |
|---|---|---|
| `version` | 最新版,与 `package.json` 一致 | 同 |
| `notes` | 更新说明,应用内显示 | 同 |
| `pub_date` | RFC3339 UTC | 同 |
| `platforms.*.url` | 安装包完整下载地址 | 同 |
| `sha256` | 包校验和,下载后本地比对 | **新增**(替代 minisign) |
| **`release_url`** | 人类可读的版本说明页 | **新增**——补上原方案的缺口:`UpdateCheckResult.release_url` 现有实现取 GitHub 的 `html_url`,站点源没有这个字段,不补则「查看发布页」按钮会拿到空串 |
| **`history[]`** | 历史版本列表 | **新增**(支撑「查看所有版本」) |

**已算好的首批数据**(取自 `sigma-file-manager/release/`):

- 文件:`Alpha File Manager_2.2.0_x64-setup.exe`,17937518 字节
- sha256:`1b6a64f7a8d29fb0ab102e402294b674cb4cc30656ab21243aa759c0ef9c8635`

**上传到站点时改用下划线命名** `Alpha_File_Manager_2.2.0_x64-setup.exe`,避免 URL 里的空格需要百分号编码,减少一层出错面。

---

## 4. 分阶段实施

### 阶段 A:站点与发布源准备(不涉及应用代码)

| 步骤 | 动作 | 判据 |
|---|---|---|
| **A0** | **在 `kizemo/alpha-file-manager` 发第一个 Alpha release**(tag `v2.2.0`,资产 `Alpha_File_Manager_2.2.0_x64-setup.exe`) | 匿名 `curl https://api.github.com/repos/kizemo/alpha-file-manager/releases` 能读到该资产 |
| A1 | 服务器建目录 `/var/www/wordpress/alpha-file-manager/` | SFTP 能写入 |
| A2 | 本地建源目录 `E:\办公文件\L1网站\alpha-file-manager\` | 目录存在 |
| A3 | 写 `index.html`(下载页)+ `.htaccess` | 浏览器能打开下载页 |
| A4 | 写第一版 `version.json`(用 §3 数据) | JSON 合法、字段齐全 |
| A5 | 写上传脚本 `_upload_alpha_files.py` | 复用 `sftp.json`;上传后 md5+size 双校验 |
| A6 | 首次上传 + curl 回读 | `curl "…/version.json?_=$(date +%s)"` 返回预期内容 |

> A6 的 curl **必须带 cache-busting**,否则可能读到旧缓存而误判。
>
> **A3 的实测结论(2026-10-10)**:服务器是 **nginx**,`.htaccess` **不生效**。原计划里"给 version.json 设 `no-cache` 兜底"这一步做不到 —— 要改得加 nginx location 规则(需 root 权限、影响全站,未擅自改动)。
> 已在 `.htaccess` 顶部写明它无效及正确做法,避免后人误以为它在起作用。**缓存保障完全依赖客户端 cache-busting**,这一条现在是单点。
>
> **A5 的环境障碍(2026-10-10)**:本机 `~/.ssh` 无私钥(服务端只收密码),必须用 paramiko;而 paramiko 依赖的 `cryptography` 在全局 Python 里已损坏(详见 §9)。改用独立 venv `E:\办公文件\L1网站\.venv-sftp`(内含 paramiko 5.0.0)。全局环境的问题**未修**,因为它超出本任务范围且修复方式应由你决定。

### 阶段 B:Rust 侧换数据源 + 兜底

| 步骤 | 动作 | 文件 |
|---|---|---|
| B1 | 新建 `version_json.rs`:`VersionManifest` / `PlatformEntry` / `HistoryEntry`,serde 反序列化 + 单测 | 新建 |
| B2 | 主源 URL 常量改 `https://www.aiec.fun/alpha-file-manager/version.json` | `app_updater.rs:16` |
| B3 | **兜底源 URL 常量改为 `kizemo/alpha-file-manager`**(现值指向 sigma 原仓) | `app_updater.rs:16` |
| B4 | `check_for_updates` 拆成 `check_primary()` / `check_fallback()`,按 §2.2 的回落条件编排;**失败原因必须写进错误文案**(哪个源、什么错) | `app_updater.rs:304` |
| B5 | `validate_github_release_download_url` → `validate_download_url`,白名单改为**并集**:`www.aiec.fun` + `github.com`/`*.github.com`/`*.githubusercontent.com`;**HTTPS 强制与私有 IP 拒绝原样保留** | `app_updater.rs:191` |
| B6 | 主源请求加 cache-busting query + `Accept-Encoding: identity`;`User-Agent` 由 `sigma-file-manager` 改为 `alpha-file-manager` | `app_updater.rs:318` |
| B7 | 下载后 sha256 校验(主源读 `version.json` 的 `sha256`,兜底源读 `assets[].digest`),不匹配则报错并删除已落盘文件 | `download_release_installer` |
| B8 | 补单测:JSON 正常解析 / 版本相同 / 版本更高 / 非白名单 URL 拒绝 / 私有 IP 拒绝 / JSON 畸形 / **主源失败回落兜底** / **主源无更新不回落** | `app_updater.rs` tests |

> B5 的**私有 IP / localhost / metadata 拒绝逻辑必须原样保留**——那是防 SSRF 的安全网,不能因换域名丢掉。
>
> B4 的错误文案当前硬编码「GitHub returned status」。双源后若仍沿用,**用户看到「GitHub 报错」却其实挂的是站点**,排查会被误导 —— 这是静默失效类故障,必须按源分别报。

### 阶段 C:前端适配

| 步骤 | 动作 | 文件 |
|---|---|---|
| C1 | `UpdateCheckResult` 增加 `notes` / `pubDate` / `sha256` 字段 | 类型定义 |
| C2 | 「查看所有版本」改读 `history[]`;主源不可用时回落展示 GitHub releases 列表 | `use-app-updater.ts` |
| C3 | 更新说明显示 `notes`;发布页链接用 `release_url` | `app-updates.vue` |
| C4 | 更新/补齐前端单测 | `use-app-updater.test.ts` |

### 阶段 D:运维流程文档化

| 步骤 | 动作 |
|---|---|
| D1 | `docs/operations/release-workflow.md`(升版本 → 打包 → 上传 → 改 version.json → curl 验证 → GitHub release) |
| D2 | `docs/operations/version-info.md`(schema 与字段说明) |
| D3 | `docs/operations/server-upload.md`(SFTP 上传与校验) |

---

## 5. 验收标准

### 5.1 已在阶段 A 验证(2026-10-10)

- [x] `curl "https://www.aiec.fun/alpha-file-manager/version.json?_=<ts>"` 返回正确版本且**不是缓存**(`CF-Cache-Status: DYNAMIC`)
- [x] 安装包公网可达,`Content-Length` = 17937518,与本地字节数一致
- [x] 上传后服务器端 md5 + size 双校验,4 个文件全部一致
- [x] 下载页公网可打开,内容含 Alpha File Manager 标题
- [x] 页面在浏览器实测渲染正常,控制台 0 报错;`history[]` 分支用独立样例测过
- [x] GitHub 兜底源已就位:`v2.2.0` release 上线,资产摘要 `sha256:1b6a64f7…c8635` **与本地哈希交叉一致**

### 5.2 阶段 B/C 后才能验

- [ ] 应用「立即检查更新」能读到版本号与更新说明
- [ ] 当前已是最新版时,正确显示「已是最新」而非报错
- [ ] 人为把线上 `version.json` 改成更高版本,应用能识别出「有更新」
- [ ] 点下载能拉取安装包到 Downloads,**sha256 与清单一致**
- [ ] 安装包真机安装成功
- [ ] **把主源 URL 改错(模拟站点故障),应用能自动回落到 GitHub 兜底并仍提示有更新**
- [ ] 主源正常且无更新时,**不会**被兜底源的旧版本干扰
- [ ] 私有 IP / 非白名单 URL 被拒绝(单测覆盖)
- [ ] `vue-tsc` 无错误,eslint 无新增,`cargo test` 全绿

---

## 6. 风险与回滚

| 风险 | 应对 |
|---|---|
| **Cloudflare 缓存导致更新推不出去** | 客户端强制 cache-busting(**目前是唯一手段**,服务端 `.htaccess` 在 nginx 上无效,见 §4-A3);上线后用带时间戳的 curl 复验 |
| **兜底源形同虚设**(GitHub 仓无 Alpha release) | A0 前置,先发版再改代码 |
| 换域名后忘了改 URL 白名单 → 下载被拒 | B5 单测锁定白名单 |
| 兜底源与主源版本打架 | 明确「无更新不回落」;A0 起两边同版本发布 |
| 服务器目录权限 / WordPress 覆盖 | A1 先验证写入;必要时由服务器管理员建目录 |
| **回滚** | 最廉价手段:把 `version.json` 的 `version` 改回旧值,用户立刻不再收到更新提示,**无需回滚应用** |

---

## 7. 工作量预估

| 阶段 | 预估 |
|---|---|
| A 站点与源准备(含 A0 发 GitHub release) | 1–2 小时(多为运维操作) |
| B Rust 侧双源换源 | 3–4 小时(比单源多约 1 小时,含回落逻辑与测试) |
| C 前端适配 | 1–2 小时 |
| D 文档 | 0.5 小时 |
| **合计** | **约 1.5 个工作日** |

---

## 8. 与现有工作的衔接

- 安装包取自 `sigma-file-manager/release/`(提交 `535c6abf` 引入的发布目录)
- 阶段 B/C 改动 `app_updater.rs` 与 `use-app-updater.ts`,与默认设置改动(`e3e3cee8`)**无文件重叠**
- 注意 `tauri.conf.json` 的 `identifier` 仍为 `com.sigma-file-manager.app`,属**受保护常量**,本任务不动

---

## 9. 执行记录(2026-10-10)

### 9.1 阶段 A 已完成

| 步骤 | 结果 |
|---|---|
| A0 | `v2.2.0` release 已发布到 `kizemo/alpha-file-manager`,资产 `Alpha_File_Manager_2.2.0_x64-setup.exe`。**GitHub 返回的 `digest` 与本地 sha256 完全一致**,属独立交叉验证 |
| A1 | 服务器已创建 `/var/www/wordpress/alpha-file-manager/`(由上传脚本逐级 mkdir) |
| A2 | 本地源目录 `E:\办公文件\L1网站\alpha-file-manager\` |
| A3 | `index.html`(浏览器实测渲染正常)+ `.htaccess`(已注明在 nginx 上无效) |
| A4 | `version.json` v2.2.0 已上线 |
| A5 | `_upload_alpha_files.py` 已就位,md5+size 双校验生效 |
| A6 | 公网回读通过 |

**本地源目录文件**:`index.html` / `.htaccess` / `version.json` / `Alpha_File_Manager_2.2.0_x64-setup.exe` / `_upload_alpha_files.py` / `_release-notes-v2.2.0.md`
(后两个是本地工具,**不在上传白名单内**,不会传到服务器 —— 因为 `.htaccess` 的屏蔽规则在 nginx 上无效,安全性依赖这个白名单)

### 9.2 上传依赖改用独立 venv

`E:\办公文件\L1网站\.venv-sftp\`(paramiko 5.0.0 / cryptography 50.0.2),走清华镜像安装,数秒完成。
**本机全局 Python 的损坏未修** —— 见 §9.3,修复方式应由用户决定。

### 9.3 ⚠️ 顺带发现:本机 Python 环境大面积文件损坏(与本任务无关,但影响你的文档类技能)

**现象**:`import` 大量第三方包直接报语法错误,且错误位置都在**文件第 1 行**。

| 包 | 报错 |
|---|---|
| paramiko / pptx / olefile / lark / easygui / extract_msg / msoffcrypto / RTFDE / tzlocal / xlrd | `SyntaxError: source code string cannot contain null bytes` |
| bs4 | `SyntaxError: unmatched '}'` |
| chardet | `ValueError: bad marshal data` |
| cryptography | 部分损坏(`import cryptography` 能过,`hazmat` 子模块过不了) |
| pdfminer | **正常** |

**取证**:
- `cryptography/utils.py` 4281 字节里含 30 个空字节,文件开头 16 字节是 `8E B6 71 F4 4B 5A 7C 8A…` —— 根本不是源码
- `cryptography-49.0.0.dist-info/METADATA` 开头是 `00 29 26 97 5C 29 4A CE…` —— 同样不是文本
- 受损文件 mtime 集中在 `2026-06-30 20:26`

**判别方法说明(重要)**:我第一次用"扫描 .py 首字节是否 ≥128"来统计,得出"289 个文件损坏",**这个数字是错的** —— UTF-8 BOM(`EF BB BF`)三个字节都 ≥128,合法文件被误判。正对照样本 `paramiko/__init__.py`、`paramiko/transport.py` 判为正常,但 `bs4/__init__.py` 被误判成损坏,证明方法有假阳性。改用 `import` 功能测试后,得到上面这张**可信**的表:抽样 15 个包,13 个真坏、2 个正常。

**未定性**:这些文件看起来像被**加密或随机覆盖**而非普通截断,不是 pip 正常行为。可能是一次损坏的安装/解压,也可能是别的原因。**没有发现任何证据表明这与本项目或 aiec.fun 有关**,但影响面覆盖 `python-pptx` / `xlrd` / `python-docx` 相关的文档处理技能,建议尽快处置(最省事的做法是重装这些包)。

### 9.4 阶段 B/C 已完成(2026-10-10)

**Rust 侧**(`src-tauri/src/app_updater.rs` + 新建 `src-tauri/src/version_json.rs`)

| 项 | 实现 |
|---|---|
| 主源 | `UPDATE_MANIFEST_URL` = `https://www.aiec.fun/alpha-file-manager/version.json` |
| 兜底源 | `GITHUB_RELEASES_API_URL` 改为 **`kizemo/alpha-file-manager`**(原值指向 sigma 上游) |
| cache-busting | `with_cache_busting()` 追加 `?_=<毫秒时间戳>` |
| 请求头 | `Accept-Encoding: identity` + `User-Agent: alpha-file-manager`(原为 `sigma-file-manager`) |
| 白名单 | `validate_github_release_download_url` → **`validate_download_url`**,放行 `aiec.fun` / `www.aiec.fun` / `github.com` / `*.github.com` / `*.githubusercontent.com`;**HTTPS 强制与 localhost/元数据服务/私有 IP 拒绝原样保留** |
| sha256 | 下载完成后本地比对,**不匹配则删除文件**;主源读 `sha256` 字段,兜底源读 `assets[].digest`(兼容 `sha256:` 前缀);清单未提供可信摘要时跳过校验而非硬算 |
| 双源编排 | `compose_update_result()`,**主源请求失败才回落兜底源** |

**回落语义是本次最关键的设计约束**,已用注释 + 3 个测试锁死:

- `does_not_query_fallback_when_manifest_says_up_to_date` —— 主源说「无更新」时,兜底源的闭包**会 panic**,证明它根本不会被调用
- `prefers_manifest_even_when_fallback_has_newer_version` —— 即使兜底源有 9.9.9,也以主源为准
- `falls_back_to_github_when_manifest_request_fails` —— 主源 503 时确实回落到兜底源

**前端侧**

| 项 | 实现 |
|---|---|
| 新字段 | `UpdateCheckResult` / `UpdateInfo` 增加 `notes` / `pubDate` / `sha256` / `source` |
| 下载校验 | `download_release_installer` 新增 `expectedSha256` 入参(Tauri 自动 camelCase↔snake_case) |
| 更新说明 | 设置页新增 `app-updates__notes` 块,复用既有 `changelog.title` key(**未新增任何 i18n key**,因为项目没有 key 对齐测试也没配 fallbackLocale) |
| 「查看所有版本」 | 改指自建站页面(会渲染 `version.json` 的 `history[]`);`actions.vue` 里同一个入口一并改掉,原先指向 sigma 的 releases |

**与原计划的偏差(一处,主动决定)**

计划 C2 写的是「「查看所有版本」改为读 `history[]` 渲染」。实现时改为**指向自建站页面** —— 该页面本身就读取 `version.json` 并渲染全部历史版本(阶段 A 已上线并实测)。理由:避免在设置项里塞一整块历史列表 UI,也避免把 `history` 作为无用字段塞进每次后台检查结果里。

**验证结果**

| 门禁 | 结果 |
|---|---|
| `cargo check --lib` | **exit 0**,无 error、无代码级 warning |
| `cargo test --lib` | 见下 |
| `vue-tsc --build` | exit 0 |
| `eslint`(仅改动文件) | 0 error 0 warning |
| `stylelint`(仅改动文件) | 0 error 0 warning(初次 6 处已 `--fix`) |
| `vitest` 全量 | **1117 passed / 2 failed** |

两个失败**均与本次改动无关**,已逐一取证:

- `module-load-recovery.test.ts > keeps startup-recovery.js aligned with shared constants` —— **历史遗留**,`startup-recovery.js` 里仍是旧品牌文案(P4 改名漏改),该文件本次未改动
- `use-navigator-item-icon.test.ts > uses separate builtin theme settings` —— **超时型 flaky**,单文件复跑通过;当时正并行跑 cargo 全量编译,CPU 被占导致 5 秒超时

> 该测试套件本身 flaky(同一份代码连续两次全量跑分别是 18 失败和 2 失败),判断回归必须**逐个文件复跑**确认,不能只看总数。

### 9.5 ⚠️ Rust 工具链损坏(与本任务无关,但会让你下次构建失败)

`rustc`/`cargo` 的 rustup 代理 shim 从 `.cargo\bin` 消失(PATH 里仍挂着该目录),且 **`stable` toolchain(1.97.1)的标准库只剩元数据存根**。

**判别过程**(避免误判):

1. 直接编译最小 hello-world → 报 `only metadata stub found for rlib dependency rustc_std_workspace_core` / `cannot resolve a prelude import`
2. 检查 registry 里报错 crate 的源码 → **完好**,不是源码损坏
3. 检查 cargo 配置 → **干净**,没有 `no_std` / `build-std` 之类 rustflags
4. 逐个 toolchain 实测 → **`1.90.0` 完好,只有 `stable` 坏了**

**当前可用的绕过方式**(本轮验证就是用它跑通的):

```
$env:PATH = "$HOME\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;$env:PATH"
cargo check --lib
```

`Cargo.toml` 的 `rust-version = "1.88.0"`,1.90.0 满足。**但这是绕过不是修复** —— 用 1.90.0 构建会与用 stable 构建产生不同的二进制指纹,且如果 stable 是你日常使用的工具链,它现在已经不能用了。建议尽快 `rustup toolchain install stable` 重装。

### 9.6 下一步

- 阶段 D:`docs/operations/{release-workflow,version-info,server-upload}.md`
- 真机验收:打包后验证「立即检查更新」读到 2.2.0、人为改高版本能被识别、模拟站点故障能回落兜底
- 处置两处环境损坏(Rust stable toolchain、Python 包大面积损坏)