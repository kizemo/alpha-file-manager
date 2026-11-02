# winget PR #467381：CLA 签署操作手册

> 生成时间：2026-11-02
> 适用对象：`kizemo`（GitHub 账号本人）
> 目标 PR：https://github.com/microsoft/winget-pkgs/pull/467381 （`New package: kizemo.AlphaFileManager version 3.9.1`）

## 0. 一句话结论

你只需要在那个 PR 页面底部**发一条评论**，内容是一行固定文字。
发完之后剩下的是等社区审核员批准，这一步谁都无法代劳，只能等。

## 1. 现在卡在哪（2026-11-02 实测）

| 项目 | 实测值 |
|---|---|
| PR 状态 | `OPEN`，未合并、未关闭 |
| 合并性 | `mergeable_state=blocked`，`reviewDecision=REVIEW_REQUIRED` |
| 标签 | `Azure-Pipeline-Passed`、`Validation-Completed`、**`Needs-CLA`**、`New-Package` |
| 评审数 | 0 个 review |
| 评论 | 4 条，**全部**是 `microsoft-github-policy-service[bot]` |
| 你的 agree 评论 | **一条都没有** |

阻断原因有两条，**只有第一条是你能自己解决的**：

1. **CLA 未签**（`Needs-CLA` 标签在）← 需要账号本人操作，本手册的全部内容
2. **需社区审核员批准**（无 `Moderator-Approved`）← 外部等待，无法加速

Azure 清单验证已经通过（上游在 2026-11-01 09:16Z 重新加回了 `Azure-Pipeline-Passed` + `Validation-Completed`），所以**不需要再动清单文件**。

## 2. CLA 是什么、为什么要本人签

CLA = Contributor License Agreement（贡献者许可协议）。它是一份法律文件，核心是：

- 你保证提交的代码是你自己写的；
- 你授权微软（和 winget 社区）永久、全球、免版税地使用、修改、分发你的贡献。

**为什么必须你本人做**：这是一份以你个人名义作出的法律承诺，签名要绑定你的 GitHub 身份。任何人替你签，等于替你承担法律责任 —— 这不是技术问题。

> 技术说明：我这边的运行环境里确实带着能发评论的 GitHub 凭据，**但我不会用它去发这条评论**，原因同上。这条评论必须由 `kizemo` 本人发出。

## 3. 操作步骤（浏览器路径 · 推荐 · 零门槛）

### 第 1 步：打开 PR 页面

地址栏粘贴：

```
https://github.com/microsoft/winget-pkgs/pull/467381
```

### 第 2 步：确认登录的是你的账号

看页面**右上角头像**，确认是 `kizemo`。不是的话先退出再登录 —— 用错账号发评论等于没签。

### 第 3 步：滚到页面最底部

- 页面上方有一排标签页（Conversation / Commits / Checks 等），保持在 **Conversation**。
- 一路往下滚到页面**最底部**，会看到一个灰色的大输入框，左上角写着 `Add a comment`（添加评论）。

### 第 4 步：输入那一行固定文字

在输入框里点一下，然后输入（或者粘贴）这一行：

```
@microsoft-github-policy-service agree
```

**必须一字不差**，注意这几点：
- `@` 后面紧跟 `microsoft-github-policy-service`，中间没有空格；
- 全部小写；
- `agree` 前面只有一个空格；
- 结尾不要加标点；
- 不要加任何别的解释文字（多打几个字有可能让机器人不识别）。

> 如果你是**代表公司**贡献（公司对该代码有知识产权），要改成
> `@microsoft-github-policy-service agree company="你的公司名"`。
> 如果 Alpha File Manager 是你个人项目、个人名义发布，用上面那条 `agree` 就行。

### 第 5 步：提交

点输入框右下角那个绿色的 **Comment**（评论）按钮。

### 第 6 步：等 1–3 分钟，刷新页面

机器人（`microsoft-github-policy-service[bot]`）是异步处理的，不是瞬间生效。

## 4. 怎么确认签成功了

**唯一的权威判据：`Needs-CLA` 这个标签消失了。**

具体检查三处：

1. **PR 页面左侧（或顶部）那排彩色标签里，原来写着 `Needs-CLA` 的灰色标签不见了。**
2. 你那条评论下面，机器人给了一条回应（社区同类 PR 里的原话是 "All CLA requirements met." 之类）。
3. 标签栏里仍然保留着 `Azure-Pipeline-Passed` 和 `Validation-Completed`（这两个不该动，动了反而说明有问题）。

### ⚠️ 一个容易误判的坑

GitHub 上有个叫 `license/cla` 的检查项，**它现在已经是绿的（success）**，但 `Needs-CLA` 标签仍然在。

这两者不是同一套记录：

- `license/cla` 是 GitHub 原生机制判定「这个账号在 GitHub 侧已经有有效的 Microsoft CLA 签名」；
- `Needs-CLA` 是 winget 仓库用的 `microsoft-github-policy-service` 这套 policy 机器人在管，它要**在 PR 里看到你那条 agree 评论**才动。

所以：**别拿 `Checks` 里的 `license/cla` 变绿当成功信号**，要看标签消失。
（顺带说明：你的账号在 GitHub 侧其实已经签过 CLA 了 —— 这也解释了为什么你之前那个 PR `#460744` 能正常走完流程。这一次多半是仓库侧记录没同步，或者是 policy 机器人的判定要求。）

## 5. 签完之后会发生什么 / 还要等多久

机器人摘掉 `Needs-CLA` 之后，会继续往下走：

1. 机器人给 PR 开启 **auto-merge（squash）**；
2. 机器人再留一条留言，大意是「社区贡献的 PR 需要审核员批准，审核员是社区志愿者，请耐心等待」；
3. **然后就停了。** 需要一位社区审核员来看你的清单、打上 `Moderator-Approved` 标签，PR 才会自动合并。

这一步没有捷径：
- 不能自己合并（你没有权限）；
- 不能催（他们本来就是志愿者，而且明确说会优先处理更老的 PR）；
- `New-Package` 类（新包）审核员还要额外核实包身份、来源、元数据，因为仓库里没有你的历史清单可参照，**通常比版本更新慢**。

判断当前处于哪一步，看标签：
- 没有 `Needs-CLA`、没有 `Moderator-Approved` → 在排队等审核员，**你现在什么都不用做**；
- 出现 `Moderator-Approved` → 已批准，机器人会自动合并，稍等几分钟 PR 就变成 merged。

## 6. 命令行路径（可选，跟浏览器二选一）

如果你更习惯敲命令。注意**本机 `gh` 现在没有登录**（`%APPDATA%\GitHub CLI\hosts.yml` 不存在），直接跑会报 `You are not logged into any GitHub hosts`。

先登录（会弹出浏览器让你授权）：

```powershell
gh auth login
```

然后发评论：

```powershell
gh pr comment 467381 --repo microsoft/winget-pkgs --body "@microsoft-github-policy-service agree"
```

发完可以直接查标签确认：

```powershell
gh pr view 467381 --repo microsoft/winget-pkgs --json labels --jq '.labels[].name'
```

输出里没有 `Needs-CLA` 就是成功了。

## 7. 出问题了怎么办

### 情况 A：评论发了，标签没消失

先等 30 分钟再看（机器人有时会排队）。超过半小时还没变，说明 policy 机器人的记录没更新，这时候需要审核员帮忙 —— 审核员侧有一个专门的触发词（上游 `doc/Moderation.md` 原文）：

```
[Policy] CLA Signed
```

让审核员在 PR 里发这条就行。**怎么找到人**：打开这个链接，看最近被合并的 PR 是谁审的：

```
https://github.com/microsoft/winget-pkgs/pulls?q=is%3Apr+is%3Aclosed+label%3AModerator-Approved
```

在 PR 里 @ 最多**两位**近期活跃的审核员，并写清楚你要什么（上游明确要求一次最多提两位，超过了反而造成误会）。

### 情况 B：页面上找不到评论框

- 没登录 GitHub → 先登录；
- 页面没滚到底 → 评论框永远在最底部；
- 被"新功能"弹窗之类的浮层挡住 → 刷新一下。

### 情况 C：评论发出去了，但机器人没反应

逐条检查：
- 是不是在 **Code Review** 的行内评论框里发的？要发在**页面最底部的 Conversation 评论框**；
- 是不是有全角字符、多余空格、或者被输入法加了标点 → **最稳的做法是清空输入框，用剪贴板粘贴**（复制下面这一行再 `Ctrl+V`）：
  ```
  @microsoft-github-policy-service agree
  ```
- 是不是少写了 `@` → 少了 `@` 就只是一句普通评论，机器人不会理。

### 情况 D：一次性发了好几条

不要重复刷。发过一条之后等就行，刷屏可能触发反垃圾机制，反而添乱。

## 8. 顺带记住的三件事

1. **CLA 只需签一次。** 官方说明：签过之后再给同一个组织的项目提 PR，`license/cla` 直接就绿，不用重复签。以后提新版本（如 3.9.2）不会再撞这个卡。
2. **不要试图改清单文件来绕过 CLA。** 那是法律检查，跟文件内容无关，改了只会白白多出一次提交。
3. **不要自己点 merge。** 合并权在审核员和机器人手上。

## 9. 相关链接

| 用途 | 地址 |
|---|---|
| 本 PR | https://github.com/microsoft/winget-pkgs/pull/467381 |
| Microsoft CLA 官方说明页 | https://opensource.microsoft.com/cla/ |
| 审核员与审核流程（含 `[Policy] CLA Signed`） | https://github.com/microsoft/winget-pkgs/blob/master/doc/Moderation.md |
| 首次贡献者清单 | https://github.com/microsoft/winget-pkgs/blob/master/doc/FirstContribution.md |

## 10. 证据来源

**已验证（本机实测）**
- PR 的 state / labels / reviewDecision / mergeable_state / 全部评论原文（2026-11-02，`gh api`）
- 本机 `gh` 2.96.0 未登录（清掉 `GITHUB_TOKEN` 后 `gh auth status` 报未登录，`hosts.yml` 不存在）
- `license/cla` check 在 head sha 上 `conclusion=success`

**已验证（上游官方文档原文）**
- `opensource.microsoft.com/cla/`：「You will reply with a comment to accept the agreement on behalf of yourself」→ 贡献者侧动作就是在 PR 里回评论
- `doc/Moderation.md`：审核员侧触发词 `[Policy] CLA Signed`；社区审核员名单；「一次最多 @ 两位审核员」
- `doc/FirstContribution.md`：无贡献者侧签署步骤（只有 pre-submit checklist 与决策树）

**已验证（社区同类 PR 的真实时间线）**
- `#450307`、`#450764`、`#450770` 三个 PR：作者评论 `@microsoft-github-policy-service agree` → 机器人摘掉 `Needs-CLA` → 开启 auto-merge → 留言提示需要审核员批准。**顺序一致**。
