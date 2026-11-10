# GitHub 推广 · 操作清单 · 2026-10-17 更新

> 标注说明：
> - ✅ **已完成**（Agent 直接 gh API 跑）
> - ⏳ **需要你的 Web UI / PR**（手动项）
> - 📝 **可以起草，下指令就开干**
> - 🙋 **你自做**（账号私事）

---

## 第 1 周基建（已完成 + 手工）

### Agent 已执行（✅ · gh API）

- [x] ✅ **AFM Topics** — 10 个：ai-tools / cross-platform / file-manager / linux / mcp-server / rust / tauri / typescript / vue / windows
- [x] ✅ **AFM Homepage URL** — `https://www.aiec.fun/alpha-file-manager/`
- [x] ✅ **AFM 开 Discussions**

待手工（⏳）：

- [ ] ⏳ **修 Funding Links** → 见 `docs/marketing/AFM-WEB-UI-MANUAL-STEPS.md` 第 2 节（API 入口被 GitHub 限制）
- [ ] ⏳ **上传 Social Preview** → 见 `AFM-WEB-UI-MANUAL-STEPS.md` 第 1 节，文件 `docs/marketing/social-preview-afm.png` 已就绪
- [ ] ⏳ **加 SECURITY.md** → 见 `docs/marketing/drafts/SECURITY.md`（GitHub API 不能自动加新文件 + 用户希望走 PR）
- [ ] ⏳ **README hero patch** → 见 `docs/marketing/drafts/README-hero-patch.md`（同上，建议走 PR）
- [ ] ⏳ **修本地 origin 错配** → 一条 git 命令（详见 `AFM-WEB-UI-MANUAL-STEPS.md` 第 5 节）

### WeavePage

> 用户 2026-10-17 11:34 指示「WeavePage 暂时不用执行」，本轮跳过他。下次开工时按 `promotion-strategy-alpha-fm-weavepage-2026-10-17.md` §6 的步骤重启。

---

## 第 2-3 周内容弹药（待你说开干）

> Show HN / 公众号 / 掘金 / Reddit 各类文稿，可以一次开干所有版本。

### AFM（已经具备 MCP 钩子的核心素材）

- [ ] 📝 **AFM Show HN 英文文稿**（250 字以内，开头"Show HN: Alpha File Manager – The file manager your AI agent can actually read"）
- [ ] 📝 **AFM Reddit 5 个 sub 的标题 + opening**（r/tauri / r/selfhosted / r/LocalLLM / r/opensource / r/productivity）
- [ ] 📝 **AFM 公众号文稿**（基于 `docs/blog/2026-10-14-alpha-file-manager.md` 草稿，开篇加 AI/MCP 钩子）
- [ ] 📝 **AFM 掘金文稿**（技术角度，独立版）
- [ ] 📝 **AFM 知乎文稿**（QA 风格 / 实测场景，独立版）
- [ ] 📝 **AFM V2EX 短帖**
- [ ] 📝 **给上游 Sigma FM 开 issue / README PR cross-link**（这是性价比最高的 inbound 流量起点，落地早写好）

---

## 第 3-4 周 Show HN 战役（待你选日期）

- [ ] 🙋 **AFM v3.2.1 stable 发版**（beta → stable，最稳的 Show HN 前置）
- [ ] 🙋 **Show HN 提交时间选择**：推荐 2026-10-21（周二）或 2026-10-22（周三），北京时间 21:00-22:00 = 美东 09:00
- [ ] 🙋 **前 6 小时紧密回 HN 评论**
- [ ] 🙋 **HN 上首页后 4 小时内同步 5 个 Reddit sub**

---

## 第 6-8 周复盘

- [ ] 🙋 拉数据：stars / forks / unique clones / 下载量
- [ ] 🙋 判哪些渠道 ROI 高，第二轮加注

---

## ⚠️ 重要旁路发现（不是推广但是 bug）

| # | 现场 | 现象 | 风险 |
|---|---|---|---|
| 1 | 本地 git origin | `F:/soft/00selfmade/filemanager/` 的 git origin 是 `kizemo/focus-sync`，但代码是 alpha-file-manager 的 | 任何 push 会推错仓库 |
| 2 | AFM 是 fork | kizemo/alpha-file-manager 是 fork 自 aleksey-hoffman/sigma-file-manager | 推断给上游 cross-link 是免费顺风 |

详细修法：`docs/marketing/AFM-WEB-UI-MANUAL-STEPS.md` 第 5 节。
