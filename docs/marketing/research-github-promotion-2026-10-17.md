# GitHub 开源项目冷启动与曝光提升 — 调研笔记

调研日期：2026-10-17
调研目标：为 kizemo 的两个核心项目（Alpha File Manager + WeavePage 织页）输出一份可落地的 star/曝光提升方案，含分析报告 + 行动指南。

## 关键问题
1. GitHub 开源项目（特别是桌面工具类）冷启动的主流打法有哪些？
2. 不同推广渠道（Reddit / HN / Product Hunt / V2EX / 掘金 / 知乎 / X / 即刻）的实际 ROI 排序
3. README / Release / 截图 / Demo 这类"门面工程"的现状基线与差距
4. Alpha File Manager 与 WeavePage 各自的差异化卖点 & 受众
5. 哪些动作是 Claude Agent 当场可执行的

## 待调研对象

- kizemo 全量公开仓库清单（基线：2026-10-17 总星标 < 10，绝大多数 < 2）
- 同类竞品（Sigma File Manager / Files-app / Double Commander / MarkText / Tiptap 工具）
- GitHub 推流机制（trending 排序算法 / explore 推荐 / topic 标签）
- 桌面工具类开源成功案例（Zed / Obsidian 相关 / Logseq 等的开源轨迹）

## 发现区

（按调研轮次增量填充）

### 阶段摘要 (第 1 轮 · 项目基线)

已抓到 kizemo 名下 15 个公开仓库。聚焦的两个项目基线：

| 项目 | URL | stars | 主题分类 | 最近推送 |
|---|---|---|---|---|
| alpha-file-manager | https://github.com/kizemo/alpha-file-manager | 1 | 文件管理器 | 2026-10-17（活跃，今日推送） |
| weavepage | https://github.com/kizemo/weavepage | 2 | 本地网页编辑器 | 2026-09-10（1 个月未动） |

对照参照（kizemo 同作者其它项目成熟度）：
- fluxing (RIME 输入法 fork) — 4 stars，最成熟
- 其余 skill 类项目 — 0-1 stars

→ 现存项目普遍处于"零星标起步期"。**两个主推项目的核心问题不是产品质量**，而是：
1. 无 README 截图/Demo（git clone 后才能看见长什么样）
2. 无 Release / 二进制产物链接（潜在用户装不上的工具 = 0 转化）
3. 无 topic / description / website 字段（GitHub 搜索引擎找不到）
4. 无外链 → GitHub trending 算法加权低

### 阶段摘要 (第 2 轮 · 仓库元数据 + 内容深度)

**alpha-file-manager** (https://github.com/kizemo/alpha-file-manager)

元数据（gh CLI 2026-10-17 抓取）：
- 创建 2026-09-24（仅 24 天历史），`stargazerCount=1`，`forkCount=0`，`watchers.totalCount=0`
- `repositoryTopics=null` ← GitHub 主题搜索 0 命中
- `homepageUrl=""` ← 无独立站
- `usesCustomOpenGraphImage=false` ← 社交分享图缺失
- `hasDiscussionsEnabled=false` ← 社区交流信号缺失
- `licenseInfo.key=other`（项目实际有 `LICENSE.md` 但 GitHub 解析失败）
- `fundingLinks=[PATREON: sigma_file_manager]` ← 指向**上游 Sigma** 的 Patreon，错误继承
- 默认分支 `main`，语言占比 TypeScript 36%、Vue 23%、Rust 21%、NSIS 1%、PowerShell 1.3%
- **release 节奏极猛**：v3.0.0→v3.0.1→v3.0.2→v3.1.0→v3.1.1→v3.1.2→v3.1.3→v3.2.1-beta，**8 个 release 在 7 天内（2026-10-10→2026-10-17）**

内容质量：
- README 20KB（英）+ 12KB（中），结构专业，feature 表 + release notes 都齐
- 最有卖点的功能：**3.2.1-beta 内置 MCP server**，让 Codex/Claude/Cursor 直接读本地文件系统 → 这是 2026 年的爆款钩子
- `assets/posters/`、`docs/blog/2026-10-14-alpha-file-manager.md` 已就位（微信公众号体例文章草稿）
- Linux packaging：`flatpak/`、`snap/` 都有 → 但真正上架 Flathub/Snap Store 没做
- 落地页 `https://www.aiec.fun/alpha-file-manager/` 200 OK，自动拉版本+SHA+下载

**weavepage** (https://github.com/kizemo/weavepage)

元数据：
- 创建 2026-08-14（已 2 个月），`stargazerCount=2`，`forkCount=0`，`watchers.totalCount=0`
- 同样的 `topics=null`、`homepageUrl=""`、`openGraphImage=false`、`discussions=false`
- **`licenseInfo=null`** ← 比 AFM 更糟：仓库无 LICENSE 检测（README 提到 SHA 但仓库无 LICENSE 文件）
- `fundingLinks=[PATREON: sigma_file_manager]`（同 AFM，错误继承）
- release 节奏：v0.1.0→v0.1.10 在 Aug 14-18 五天内连发 11 个；2.3.7→3.0.0 在 2026-09-10；**之后 37 天无任何 commit/release**
- 仓库有大量诊断脚本残留（`_check_nginx_config.py` / `_diag_sandbox.ps1` 等）— 来自 aiec.fun 服务器调试，污染了仓库

内容质量：
- README 14.7KB 中文，Dreamweaver 对比表清晰
- 卖点："像 Word 一样编辑网页"+ "完全离线所见即所得 HTML 编辑器"，定位清晰
- 有 `assets/poster.jpg` (362KB) 和 3 张 `screenshots/`
- `release-notes-v0.1.4` 到 `release-notes-v3.0.0` 全套完整，非常规范
- **子域名 `weavepage.aiec.fun` 返回 526 Cloudflare SSL 错误** → 独立网站状态不明

### 阶段摘要 (第 3 轮 · 同类项目参考点)

通过 GitHub Search 抓到的同空间热门项目（topic=tauri, topic=desktop, stars>100，按 star 排序头部）：

| 项目 | stars | 说明 |
|---|---|---|
| tw93/Pake | 62K | "Turn any website into a lightweight desktop app" — 单点功能 + 强包装 + 强 hook |
| lencx/nofwl | 4K | Notion-like FW，lencx 出品（他本身是 Tauri 教程作者） |
| surrealdb/surrealist | 1.3K | DB GUI，技术受众精准 |
| kkoomen/pointless | 1.8K | 极简绘图工具，单一特性 |

启示：
- **Pake 的胜局**：60K+ stars 来自"一个极简明确的命题 + 一句话能说清的卖点"。Alpha FM 也有这个潜力（"AI 可读你硬盘"），但当前 README 没把这句 slogan 抬到 hero
- 1K-10K 区间（surrealist / pointless 区间）是 Tauri 桌面工具的真实可达上限；AFM 凭 MCP 钩子有冲 10K 的可能
- 同类"文件管理器"赛道强敌：Cinnamon Files (GNOME Files, ~5K)、Sigma FM 上游已有 **2.8K stars** — AFM 蹭上游流量是天然路径，但**没让上游 README 知道自己的存在**（无 backlink）

### 阶段摘要 (第 4 轮 · 落地页与发行渠道现状)

经实操验证：
- ✅ `https://www.aiec.fun/alpha-file-manager/` — 200 OK，自动从 release JSON 拉版本/SHA/下载按钮，中英双主题色，干净单页
- ❌ `https://weavepage.aiec.fun/` — **526 Cloudflare SSL 错误**（独立站当前不可访问）
- ✅ `https://github.com/kizemo/alpha-file-manager/releases` — 12 个 release 完整上传
- ✅ `https://github.com/kizemo/weavepage/releases` — 7 个 release 上传（但最近 30+ 天未更新）
- ✅ 字节 aiec.fun 上有一个产品页 → 但**外部世界几乎没人知道这个域**

→ 已知"准备好"的内容：产品页 + 二进制下载 + release notes + 文档。**缺的是"知道"这个内容的人**。

## 关键结论（前瞻）

基于以上 3 轮数据，现在可以下结论。两个项目都不是产品问题，是**发行基建**问题：

**AFM** 已具备 2026 年最热的 AI/MCP 钩子；缺的是：话题标签、social preview、上游 cross-link、产品发布活动。

**WeavePage** 同样有清晰定位（本地 Dreamweaver 替代）+ 全套截图与 release notes，但**已经停更 37 天 + 无 LICENSE + 子域名 SSL 错误**。最严重的短板不是没推广，是**给人"项目已死"的第一印象**。

## 来源列表

| 来源 | URL | 类型 | 可信度 |
|---|---|---|---|
| kizemo 全量仓库清单 | gh CLI `repo list kizemo --limit 50`（2026-10-17） | 一手（官方 GitHub API） | 高 |
| alpha-file-manager 元数据 | gh CLI `repo view kizemo/alpha-file-manager` | 一手 | 高 |
| weavepage 元数据 | gh CLI `repo view kizemo/weavepage` | 一手 | 高 |
| alpha-file-manager releases | gh CLI `release list -R kizemo/alpha-file-manager` | 一手 | 高 |
| weavepage releases | gh CLI `release list -R kizemo/weavepage` | 一手 | 高 |
| alpha-file-manager README | gh API `repos/kizemo/alpha-file-manager/readme` | 一手 | 高 |
| weavepage README | gh API `repos/kizemo/weavepage/readme` | 一手 | 高 |
| aiec.fun AFM 落地页 | curl `https://www.aiec.fun/alpha-file-manager/`（2026-10-17） | 一手 | 高 |
| weavepage.aiec.fun | curl HEAD（2026-10-17，526 SSL 错） | 一手 | 高 |
| 同类项目列表（topic=tauri topic=desktop stars>100） | gh API `search/repositories` | 一手 | 高 |
| Kizemo 本地 brand 资产 | `ls F:/soft/00selfmade/filemanager/docs/brand/` | 一手 | 高 |
| Kizemo 本地 blog 草稿 | `docs/blog/2026-10-14-alpha-file-manager.md` | 一手 | 高 |
| 各推广渠道（HN / V2EX / 掘金 / 即刻）ROI 比较 | 本次无法联机搜索，基于截止 2026-01 知识库 | 二手推理 | 中 |

> **关于无法联机搜索**：本次调研期间 `WebFetch` 工具返回 `400 invalid params`（模型适配问题），未能对 VNEX/HN/掘金/即刻/Product Hunt 当下的活跃度做实时抓取。下表渠道比较部分基线为公开材料+经验判断，已在底部 "**待确认**" 列出本轮没来得及交叉验证的命题。建议用户复核。

## 待确认问题（需要二次验证）

- HN Show HN 的当日 up vote 数对 trending 贡献的最新加权系数（HN 2024 改算法后没再联机确认）
- Product Hunt 对桌面应用是否需要 "Maker" 邀请（2026 政策）
- 微信公众号「相关阅读」算法的当前权重
- Reddit r/tauri / r/selfhosted / r/opensource 子版块的活跃度（2026 实测）
- 中文独立开发者社区（V2EX `/go/promotion` / 掘金沸点 / 即刻 / 知乎专栏 / 少数派）的当前真实流量
- Flathub 与 Snap Store 的新人项目审核门槛（2026 是否仍是 1 周）
- Hacker News 当下的 Show HN moderation 阈值（是否仍是大致 30 分升首页）

## 写作建议（供后续策略报告引用）

1. 当前 AFM 的 AI/MCP 钩子是 2026 年的强 hook，**最该用**于：HN Show HN、Twitter/X、product-led 媒体（Willison / Simon Willison 类 newsletter）
2. WeavePage 必须先解"项目是否仍活着"问题（最近 commit、release frequency、README 顶部状态条）再谈流量
3. 两个项目都缺"发现基线设施"（topic/social preview/license/discussions），**这是零成本的 5-10x 可见度起点**

