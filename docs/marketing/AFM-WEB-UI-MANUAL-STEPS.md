# AFM · 仍需手工完成的两个 Web UI 步骤

日期：2026-10-17

下面两个动作**GitHub 没有公开 API**，必须登录 Web UI 完成。两项都加起来的工时 ≤ 10 分钟。

---

## 步骤 1 · 上传 Social Preview 图（⭐ 必做）

Social Preview 是别人在 Twitter / 公众号 / HN / Reddit 分享你的仓库链接时显示的卡片。GitHub 默认是空的，等于「裸 URL 分享」。

### 文件位置（已为你合成好）

```
F:\soft\00selfmade\filemanager\docs\marketing\social-preview-afm.png
```

- 尺寸：1280×640
- 文件大小：164 KB
- 内容：Alpha File Manager 标志 + 中英 slogan + 4 个标签 (Tauri 2 / 内置 MCP / Windows + Linux / 开源) + GitHub URL

### 上传路径（Web UI）

1. 打开 https://github.com/kizemo/alpha-file-manager/settings
2. 左侧菜单 → **Social preview**
3. 点 **Upload an image** 选 `social-preview-afm.png`
4. 调整（如有需要）
5. 点击 **Save changes**

### 上传完如何验证

1. 打开 https://github.com/kizemo/alpha-file-manager（settings 之外）
2. 在 README 某段点 **Share** → **Copy link**
3. 把这个 URL 粘到 https://www.opengraph.xyz/ 验证预览卡是否变成 1280×640 PNG 而不是空
4. 也可以直接发邮件或 IM 给朋友看，会显示大卡片

---

## 步骤 2 · 修改 Funding Links（⭐ 推荐做）

仓库当前的 `Funding Links` 还是 fork 自 Sigma FM 的 patreon：

```
平台: PATREON
URL:  https://patreon.com/sigma_file_manager
```

赞助按钮会把读者带到 **上游作者** 的 patreon，不是你的。改成本来就应该。

### 操作路径

1. 打开 https://github.com/kizemo/alpha-file-manager/settings
2. 左侧菜单 → **Sponsorships**
3. 如果你还没开 GitHub Sponsors：**先勾 Enable GitHub Sponsors**，然后到 https://github.com/sponsors/kizemo/ 点击 onboard；临时没有 Sponsors 资格也行，先把流程走一遍，等同于预留位置
4. 在现有 Patreon 那行右侧点 **Remove**
5. 添加 GitHub Sponsors（如果已开通）：选 `github` → `https://github.com/sponsors/kizemo`
6. Save

### 没开 Sponsors 怎么办？

暂时把 Patreon 那行也删掉（不展示任何 funding 按钮）也比展示错误的链接要好。等 GitHub Sponsors 开通再回来加。

---

## 步骤 3 · 把 README hero 改用 draft

draft 落在：

```
docs/marketing/drafts/README-hero-patch.md
```

里面给了：
- 英文 README 的 patch （3 行 callout 加在 hero 上方，原文不动）
- 中文 README 的 patch （3 行中文 callout，原文不动）
- 备选 A/B 改法
- 推荐 PR 提交方式（Git Flow 不能直推 main）

推送方式：
1. 在 GitHub 网页端编辑 → 对应 branch 提交 → Open PR
2. 或本地克隆 alpha-file-manager → 改正文 → push branch → Open PR

---

## 步骤 4 · 把 SECURITY.md 加进仓库

draft 落在：

```
docs/marketing/drafts/SECURITY.md
```

标准 GitHub 安全披露格式 + AFM 实际有的 5 条 MCP server 保护闸门（基于你 README 实际写到的内容）。

push 方式同上。

---

## 步骤 5（可选） · 修本地 git remote

本地工作目录 `F:\soft\00selfmade\filemanager\` 的 git `origin` 错配：

```
origin    https://github.com/kizemo/focus-sync.git
```

但目录里的代码是 alpha-file-manager 的。这是历史遗留问题。今后再在本目录用 git push，会推到 focus-sync 而不是 alpha-file-manager — **危险**。

修法（只需执行一次）：

```bash
cd F:/soft/00selfmade/filemanager
git remote set-url origin https://github.com/kizemo/alpha-file-manager.git
git remote -v   # 确认
```

执行后下次再做 README / 改动，可以走正常 PR 流程，不必再绕 gh API。

---

## 状态一览（已完成 / 待办）

| # | 项目 | 状态 | 完成人 |
|---|---|---|---|
| 1 | 设 Topics (10) | ✅ 已完成 | Claude (gh repo edit) |
| 2 | 设 Homepage URL | ✅ 已完成 | Claude |
| 3 | 开 Discussions | ✅ 已完成 | Claude |
| 4 | 修 Funding Links | ⏳ 待你 Web UI | 用户 |
| 5 | 上传 Social Preview | ⏳ 待你 Web UI | 用户 |
| 6 | SECURITY.md | ⏳ 待 PR | 用户 |
| 7 | README hero patch | ⏳ 待 PR | 用户 |
| 8 | 修本地 origin | ⏳ 推荐你 CLI 一条命令 | 用户 |

---

## 后续动作（如前面策略报告 §7 节奏）

这些不在这两天解决，按 §7 节奏推进：

- 第 2 周：起草 Show HN、公众号、掘金、Reddit 等稿件（Claude 已就绪，可在 docs/marketing/drafts/ 起新文件）
- 第 3 周：Show HN 战役（推荐 2026-10-21 或 -22 周二/三 美东 09:00）
- 第 4 周：给上游 Sigma FM 发 issue / PR README cross-link

**继续推进哪一步？** 直接说 "起 Show HN 稿"、"发公众号稿"、"给上游开 issue" 等。
