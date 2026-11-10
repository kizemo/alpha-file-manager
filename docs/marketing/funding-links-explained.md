# Funding Links 是啥？为什么要改？操作怎么走？

日期：2026-10-17
给大白话阅读，没用过 GitHub Sponsors 也能看懂。

---

## 一句话先说清楚

**Funding Links = 仓库页面上的"赞助"按钮。** 现在你这个仓库的赞助按钮，点开之后是带人去 Sigma 作者的 Patreon 打钱。**这不对**。这是 GitHub 在你从 Sigma fork 出新仓库时，自动把别人的赞助链接顺手抄了过来。

下面用三个具体场景让你感同身受。

---

## 三个会让事情变糟的场景

### 场景 1：钱去了别人口袋

某天有个人看到了你的 Alpha File Manager，觉得特别好，点仓库右上角的 **"Sponsor"** 按钮。

他的本意是给你一个月 5 美元，让你有动力继续做下去。

GitHub 把页面跳转到 Patreon 收款页面，账户名叫 **"Sigma File Manager"**。这个人想："嗯，作者的账号是这个，没问题。"

于是一个月过去，你没收到一分钱，Sigma 作者 Aleksey Hoffman 多了一位新赞助人。

> 你的代码，读者帮你拉广告位，钱归别人。

### 场景 2：你的项目看着像 Sigma 的"官方分店"

一位新读者打开你的仓库页面，看见：

- 标题：**Alpha File Manager**
- 描述：**基于 Sigma File Manager 二次开发**
- 顶上还有个"赞助 → Patreon"的按钮，链接到 `patreon.com/sigma_file_manager`

他的第一反应是：「哦，这其实是 Sigma 的官方分店吧？分店又来分叉了？」

读者会困惑：**这是 Sigma 在支持，还是你在支持自己？**

如果是别的开发者想 fork 你的代码二次开发，他也会犹豫：「这个仓库看起来是 Sigma 的子项目，我自己做的项目是不是放错了地方？」

### 场景 3：链接失效那一天

Sigma 作者 Aleksey Hoffman 如果有一天：
- 不想做 Sigma 了，把 Patreon 关了
- 或搬家到 Ko-fi / Buy Me a Coffee
- 或 Sigma 整个仓库 archived 了

**你仓库上的赞助按钮就变成一个坏链。**

你的"赞助"按钮就成了摆设——明明有读者想支持你，点了之后页面 404，错过去。

---

## 解决思路：三种状况，三个选择

| 你的 GitHub Sponsors 状态 | 建议做法 |
|---|---|
| **A · 已经开通**（你在 https://github.com/sponsors/kizemo 有页面） | 删除 Sigma 的 Patreon，改成你的 GitHub Sponsors |
| **B · 还没开通**（你愿意开通） | 先走开通流程（GitHub 要审核，通常要几天），通过之后再回来改 |
| **C · 暂时不开**（不想搞这种事） | 直接把 Sigma 的 Patreon 那行删了，**不显示任何赞助按钮**。这样至少不掉链 |

**任何一种都比"留着错误链接"好。** 我推荐 A 或 C，B 比较慢而且申请不一定通过。

### 关于 A 选项（GitHub Sponsors）

GitHub Sponsors 是 GitHub 自己搞的官方赞助渠道，类似"GitHub 版本的 Patreon"。

- **读者怎么用**：在你仓库点 Sponsor → 选每月 5/10/25 美元 → 绑定信用卡 → 自动每月从卡里扣
- **你收多少**：GitHub 抽 0%（其他平台像 Patreon 抽 5-12%）
- **门槛**：GitHub 官方面向的是被认证的开发者/开源贡献者，需要 OSS 活跃度或受邀，不是所有人都能开

### 关于 C 选项（直接啥都不显示）

这是"懒但对"的做法。仓库上看不到赞助按钮，不影响正常使用。等到你想用的时候再回来加。

> 短期：C 选项就行
> 长期：建议至少开通 Ko-fi（审批快，海外用户多），或者微信 / 爱发电（中国用户多）

---

## 一步步操作（先讲 C，当前最实际）

### 路径 C：直接删了 Sigma 的链接（5 分钟）

**步骤 1 · 登录 GitHub**

浏览器开 https://github.com/login，用你的账号（kizemo）登录。

**步骤 2 · 打开仓库设置**

地址栏直接输入：
```
https://github.com/kizemo/alpha-file-manager/settings
```

页面顶部会出现一长串菜单。

**步骤 3 · 找到「Sponsorships」选项卡**

左侧菜单从上往下数，在 "General" "Options" 之类的标签下方，找到名字叫 **"Sponsorships"** 的那一项。点它。

> 如果左侧没有这一项，说明你的 GitHub 账号还没开过 Sponsors 任何功能。要么先去 https://github.com/sponsors/kizemo 走一遍开通流程，要么这条就当没看到，直接走路径 D（路径 D 在最下面）。

**步骤 4 · 你会看到一个截图样式的区域**

进入「Sponsorships」后，页面会显示**一张卡片**：

- 卡片标题写 "Funding links"
- 卡片里现在有一行："**PATREON**" + 一个网址 `https://patreon.com/sigma_file_manager`
- 那行最右边有个垃圾桶 / 删除 / 移除按钮（不同版本长得不一样，可能是 ✕ 图标或 "Remove" 字样）

**点那个删除按钮。**

弹窗会问："确认删除？" → 选确认。

**步骤 5 · 保存页面**

这一步可选：

- 如果页面底部有 **"Update sponsorship links"** 或类似的"保存"按钮 → 点它。
- 如果是实时生效（删完自动保存）→ 这步不用做。

**步骤 6 · 验证**

打开你的仓库主页 `https://github.com/kizemo/alpha-file-manager`，滚动到仓库标题那一行右侧。**之前有一个 Sponsor / ❤ 按钮，现在应该没了。**

如果还有，说明删除没生效。回到路径 C 重新做一次。

> 验证用隐身模式（Ctrl + Shift + N）打开，避 GitHub 本地缓存。

---

### 路径 A：你已经开了 GitHub Sponsors（10 分钟）

先按路径 C 的步骤 1-3 走到「Sponsorships」页面。然后：

**步骤 4-A · 删旧链接**

把那个 `patreon.com/sigma_file_manager` 链接按路径 C 的方法删掉。

**步骤 5-A · 加上新的**

在卡片里找到类似 **"Add link"** 或 **"Link another platform"** 的按钮。点它。

弹窗里让你选平台 + 填 URL：

- 平台：选 **GitHub Sponsors**（如果菜单里有）
- URL：填你自己的 https://github.com/sponsors/kizemo

（如果没有 GitHub 选项，先去 https://github.com/sponsors/kizemo 看一眼你的页面存在不；如果存在但菜单没这个平台，可能是 GitHub 那边没同步。等几分钟再回来看看。）

**步骤 6-A · 保存 + 验证**

按 GitHub 提示保存。然后用上面路径 C 步骤 6 的方法验证：你仓库页右上角应该出现一个 **"Sponsor"** 或 **"❤  Sponsor"** 按钮，点开能跳到你的 GitHub Sponsors 页。

---

### 路径 B：想去开通 GitHub Sponsors（15 分钟 + 几天审核）

**步骤 1 · 进入申请页**

打开 https://github.com/sponsors/

**步骤 2 · 看你的资格**

页面上会写："你需要满足以下条件之一："

- 是某个开源项目的 maintainer
- 给某个被 GitHub 收录的项目贡献过
- 被已有 Sponsor 用户邀请

看页面具体说什么。多数情况下，活跃维护开源仓库的 maintainer 是有资格的。

**步骤 3 · 点 "Join the waitlist" 或 "Set up your sponsorship profile"**

跟着点，跟着填，会让你：
- 选国家/地区（决定收款方式）
- 填收款资料（PayPal 或银行账户）
- 写自己的简介（公开给别人看的）

**步骤 4 · 等 GitHub 审核**

通常要等几天到几周。审核结果会发到你的邮箱。

**步骤 5 · 通过后，回来按路径 A 加链接**

---

### 路径 D · 你的情况是"没有 Sponsorships 这一项"（备选）

如果路径 C 步骤 3 找不到"Sponsorships"那个菜单项：

- 你的账号从来没启用过 Sponsorships 功能
- GitHub 不让你直接编辑 funding links

**变通做法**：

去 https://github.com/sponsors/kizemo 走一遍 onboarding 流程（即使最后你不打算真用，至少把功能开通了），开通后回到路径 A 操作。

或干脆不管这条，优先做这个：

---

## 整理一下：你想怎么搞？

| 选项 | 你需要做什么 | 时间 |
|---|---|---|
| **路径 C**：直接删了 | Web UI 5 分钟 | 现在 |
| **路径 A**：改成 GitHub Sponsors | Web UI 10 分钟 | 假设你已经开了 |
| **路径 B**：先开通 Sponsors | Web UI 15 分钟 + 几天等审核 | 中期 |
| **路径 D**：啥也不做 | 等 | 永远 |

我推荐 **路径 C**，5 分钟搞定，今天就能消除"赞助到别人那里"的隐患。如果你之后想接 GitHub Sponsors 或微信收款，再加不迟。

---

## 我可以帮你写的"回复 / 配置"开场白

如果你打开 GitHub 设置时不知道某些英文术语怎么理解，下面几个常见按钮的中文对照：

| 英文 | 意思 |
|---|---|
| `Sponsorships` | 赞助设置（在左侧菜单） |
| `Funding links` | 具体的赞助链接卡片标题 |
| `Add link` / `Add a custom link` | 加一个新链接 |
| `Remove` / `Delete` | 删一行 |
| `Update preferences` / `Save` | 保存 |
| `Reusable workflows` | 不相关，跳过 |
| `Code security & analysis` | 不相关，跳过 |
| `Interactions` | 评论权限，不相关 |

如果你看到别的英文看不懂的，把英文告诉我，我解释。

---

**总结一句**：先走"路径 C"删了错链接，5 分钟解决。今天就能好。
