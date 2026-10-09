**[English](README.md)** | **[中文](README.zh-CN.md)**

<h1>&nbsp;&nbsp;Alpha File Manager</h1>

**一款面向 Windows 与 Linux 的现代文件管理器 —— 补上资源管理器最缺的两件事:
一棵真正的目录树,以及会跟着你跨应用走的文件对话框。**

Alpha File Manager 是 [Sigma File Manager](https://github.com/aleksey-hoffman/sigma-file-manager)
的独立衍生版本,由 [kizemo](https://github.com/kizemo) 维护。它跟随上游同步,并在其上叠加两项改进:

> 🗂️ **「我在哪?」** —— 常驻目录树,永远显示你所在位置的层级结构。
> ⚡ **「这路径为什么又要手打一遍?」** —— 所有文件对话框自动跟随你正在浏览的目录。

> [!IMPORTANT]
> **Alpha File Manager 是独立的社区项目**,与 Sigma File Manager 项目无隶属关系,
> 未获其认可或官方支持。基础应用的功能与品牌归
> [Aleksey Hoffman](https://github.com/aleksey-hoffman) 所有。

---

## 🗂️ 目录树侧边栏

![Alpha File Manager 打开目录树侧边栏,显示 E:/办公文件 目录](./docs/screenshots/tree-sidebar-v6.4.1.png)

左侧常驻**目录树**,实时映射文件系统并跟随当前激活面板 —— 你能看清所在位置的层级结构,
也能一键跳到任意上级目录。

- **始终一键可达** —— 导航工具栏上有独立的 `FolderTree` 按钮,刻意**不**放进布局下拉菜单。
- **跟随一切** —— 自动跟随地址栏、收藏面板,以及**分屏视图中的激活面板**;每个分屏各自维护展开状态。
- **始终紧凑** —— 导航时只保留当前路径的祖先链展开,其余分支自动折叠。
- **点击语义可预期** —— 点击行内容进入该目录;点击折叠箭头只展开,不改变当前目录。
- **盘符不混淆** —— 根节点同时显示卷标与盘符(如 `Win (C:)`),多硬盘与 WSL 环境下也一目了然。
- **记住你的选择** —— 显示 / 隐藏状态跨重启保存。

对应上游 issue [#499](https://github.com/aleksey-hoffman/sigma-file-manager/issues/499) · 259 个单元测试通过。

---

## ⚡ Focus Sync —— 会跟着你走的文件对话框

在 Alpha FM 里浏览到某个目录,然后在**任意** Windows 应用里打开
**另存为 / 打开**对话框 —— Chrome 下载、Word、VS Code、钉钉、飞书 ——
它已经停在你刚才那个目录里了。

**零配置。** 安装包已内置扩展与一个小型本地伴生进程,Windows 计划任务会在登录时自动启动。
**不需要手动运行任何东西。**

**它给自己定的三条规矩**

| | |
|---|---|
| **绝不抢焦点** | 对话框不在前台时,写入会等待,绝不强行切换。 |
| **绝不弄脏文件名框** | 目标是地址栏。当回退路径必须碰文件名框时,它会还原原始值并**读回校验**后才提交。 |
| **绝不写入陈旧路径** | 若 Alpha FM 未在运行,则拒绝写入,而不是把你上次待过的目录灌进对话框。 |

扩展只申请 `commands`、`toolbar`、`notifications`,以及对 `127.0.0.1` 的 HTTP 访问 ——
**无 shell 权限,无文件系统权限。**

> **状态** —— 仍在加固中。已在原生 Win32 对话框上验证;
> Chromium 与 UWP 宿主的表现因宿主而异。

---

## 📦 获取

**预构建版本:** [`v2.2.0-tree.1`](https://github.com/kizemo/alpha-file-manager/releases/tag/v2.2.0-tree.1)
—— 2026-09-26 的目录树构建版。

> 该 tag 早于 `main` 上的 Focus Sync 打包整合:它带目录树,但没有现在的一步式安装包。
> 新版构建即将推出。

Windows 构建未做代码签名,SmartScreen 会提示「未知发布者」—— 选择
**更多信息 → 仍要运行** 即可。

---

## 致谢

- **上游:** [aleksey-hoffman/sigma-file-manager](https://github.com/aleksey-hoffman/sigma-file-manager),
  作者 [Aleksey Hoffman](https://github.com/aleksey-hoffman)。基础应用本身、其功能与品牌归上游所有。
- **Focus Sync 扩展:** [kizemo/focus-sync](https://github.com/kizemo/focus-sync)
- **本分支维护者:** [kizemo](https://github.com/kizemo)

另按原始条款复用以下代码:
[inaku-Gyan/PathWrap](https://github.com/inaku-Gyan/PathWrap)(MIT) ·
[QwenLM/qwen-code](https://github.com/QwenLM/qwen-code)(Apache-2.0)。

---

## 许可证

**GPL-3.0-or-later** —— 详见 [`LICENSE.md`](./LICENSE.md)。本分支新增内容同样以该许可证发布。

按 GPL-3 要求,所分发二进制对应的完整源码可获取:

| 组件 | 源码 |
|---|---|
| Alpha File Manager(本分支) | <https://github.com/kizemo/alpha-file-manager> |
| Sigma File Manager(基础应用) | <https://github.com/aleksey-hoffman/sigma-file-manager> |
| Focus Sync 扩展 + sidecar | <https://github.com/kizemo/focus-sync> |