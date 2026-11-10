# 上游 Sigma File Manager · Cross-link Issue / PR 文案

**投递目标**：https://github.com/aleksey-hoffman/sigma-file-manager/issues
（或者直接发 README PR，更稳。issue + PR 都可以同时做）

---

## 投递方式选择

| 方式 | 好处 | 风险 |
|---|---|---|
| **Issue（推荐先做）** | Aleksey 看了觉得合适后自己合并，礼貌 | 作者可能比较忙不回 |
| **README PR** | 一次到位，节省作者精力 | 维护者可能不喜欢"陌生人改 README"，需要点说服力 |
| **两个一起** | 既留 issue 痕迹又给 PR 提交路径 | 略显强势 |

**推荐**：先开一个友好 issue 让 Aleksey 知会你存在；附带 PR 文本让他采纳的话直接 merge。

---

## Issue 正文（中文版，发中文区）

标题：

```
Community build · Alpha File Manager · MCP server + folder tree + long-term patch set
```

正文：

```markdown
Hi Aleksey,

我是 kizemo，过去几周一直在用你的 Sigma File Manager 做底子，做了一个活跃维护的下游分支，叫 **Alpha File Manager**：

github.com/kizemo/alpha-file-manager

这个 issue 不是 bug 报告，是想给你同步一下我这边的情况，并问你要不要在 README 里加一行"Community builds"section 提一下我这边。

我在你代码基础上加了这些 Sigma 没有的能力：

- **MCP server（3.2.1-beta）** —— 让 Claude/Codex/Cursor 这类 AI 助手通过标准接口列出我的盘、搜索文件名、读收藏夹。只读，localhost-only，五道安全闸门。
- **Folder tree sidebar** —— 真正的文件夹树，跟着当前面板走。
- **Focus Sync** —— 文件打开/保存对话框会跟着当前浏览位置走（这个我们之前另一个仓库独立做过，https://github.com/kizemo/focus-sync）。
- **Directory search 真的递归** + typo 容忍，跑 1TB 硬盘 ~2s。
- **批量改名冲突提前拦** + 缩略图四档可调 + 列对齐 + 右键菜单等 17 项增量。

我现在每两天一个 release，截至 2026-10-17 已经发了 12 个版本。

我在仓库首页已经写明："Alpha File Manager is an independent community project. It is not affiliated with, endorsed by, or supported by the Sigma File Manager project." —— 不希望我的下游让你觉得在抢你的功劳或牌子。

想问两件事：

1. 你 OK 不 OK 我在 README 把 upstream 链回 `aleksey-hoffman/sigma-file-manager` 的 block 继续保留？（已经保留了，目前这样。）

2. 如果 OK，要不要加一行在 README 末尾，比如"Community builds"区提一下我这个 fork？文本我起草好（见下一个 section），你看了直接 merge 也行。

不管你怎么决定都很感激。希望能持续在你的基础上做贡献。

—— kizemo
```

---

## PR 文本（README 末尾加一小段）

如果 Aleksey 同意，最小化追加：

```diff
 ## License
 
 ...（原文不变）
+
+## Community builds
+
+Independent projects that track Sigma File Manager and ship their own additions:
+
+- **[Alpha File Manager](https://github.com/kizemo/alpha-file-manager)** — Active fork with a built-in MCP server (lets AI assistants read your drives), a true folder tree sidebar, Focus Sync for cross-app dialogs, typo-tolerant directory search, and 17+ incremental fixes. ~12 releases in 24 days as of 2026-10-17.
```

PR title：

```
docs(readme): add a Community builds section with a link to Alpha File Manager
```

PR description：

```markdown
Hi Aleksey, opening this as a follow-up to issue #XXX.

I run https://github.com/kizemo/alpha-file-manager, an active downstream fork
of Sigma File Manager. It tracks upstream and adds an MCP server for AI
assistants, a folder tree, Focus Sync, and a batch of fixes. 12 releases in
the last 24 days.

This PR proposes a single Community builds section near the bottom of the
README so that anyone who lands here looking for "more active fork" or
"AI integration" can find it. The wording is short, the link is one, and it
is clearly disclaimed as an independent project.

If you'd rather keep the README lean — totally fine, close this. I asked in
the linked issue first so you can speak to the idea before you see the diff.

Thanks.
```

---

## 我什么时候投递？

- **不在你授权之前发**。这是别的人的仓库，文案上署名"我 (kizemo)"的语气——你应该看看我写得对不对，必要时改改再发。
- **建议先 PR 再 issue**，或者只 issue 不 PR。PR 主动但 author 可能会不喜欢。
- 投递时机：随便什么时候都行。这种"社区分叉的存在公告"属于礼节，跟 Show HN 不一样，没有时间敏感度。

---

## 递交之后的预期效果

| 上游 6679 stars 中 | 大约 ratio | AFM 增量 stars |
|---|---|---|
| Alpha File Manager 在 README 里被加一条链接 | 0.5-3% | **35-200 stars** |

外加：
- 在上游 issue 列表里多了一个"Alpha File Manager"关键词，未来 GitHub 用户搜 upstream 时能看到
- 偶尔被 Aleksey 自己推特之类渠道引用（如果他有）

---

## 备选方案（如 Aleksey 不回）

万一 author 长时间不回：

1. **自己写一份"基于 Sigma 的派生项目"清单**——你可以维护一个仓库叫 `sigma-file-manager-forks.md` 之类的，也可以放 `awesome-sigma-fm` 风格列表
2. **去 NPM/Reddit/HN 发 "Best fork of Sigma File Manager in 2026" 类推荐文**——把 AFM 名字置顶
3. **直接发 Show HN** 提一句"I fork Sigma File Manager"，让看见的人自然意识到有 AFM 存在

这些是"绕过 upstream" 的备选打法，不在我们这轮主线里。
