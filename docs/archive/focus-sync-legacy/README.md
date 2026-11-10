# Focus Sync —— 发布存档

> **本仓库已停止接收插件源码。**
> 插件的源码与活跃开发已迁至 **Alpha File Manager** 仓库:
> **<https://github.com/kizemo/alpha-file-manager>**

---

## 1. 这里还剩什么

| 内容 | 位置 | 说明 |
|---|---|---|
| **已发布的独立安装包** | `release/kizemo.focus-sync-0.5.8-setup.exe` | v0.5.8,用户实测通过的那一版 |
| **文档与交接记录** | `docs/` | 架构说明、变更史、历次 handoff |
| **历史交接文件** | `handoff-*.md` / `prompt-*.md` | 失败记录也是记录,不要删 |

**源码不在这里了。** 插件的完整源码在品牌仓的
[`extensions/kizemo.focus-sync/`](https://github.com/kizemo/alpha-file-manager):

```
extensions/kizemo.focus-sync/
├── sidecar/      后台程序 Rust 源码
├── dist/         插件主文件(手工维护,无构建链)
├── locales/      语言包
├── installer/    独立 NSIS 安装器 + PS 脚本
└── package.json
```

---

## 2. 为什么要迁走

原先插件在独立仓库、文件管理器在 fork 仓库,两者**互相知道对方的位置**:
打包时要从隔壁目录取插件文件。结果是**单独克隆文件管理器仓构建不出带插件的安装包**,
沙箱门禁也要跨仓读源码。README 里当时写的「两者零源码耦合」并不属实。

2026-10-09 把插件源码并入品牌仓后,这个依赖**真正消失**了。

**源码只有一份,是这次迁移唯一的硬要求。** 两处副本各自演化正是本项目历史上
最大的一次故障(`sidecar 路径分裂`:两个安装器把同一个文件放在不同位置,
互相覆盖计划任务、留下孤儿副本,使验收门禁永远不可能通过)。

---

## 3. 需要独立发版时怎么办

```powershell
# 1. 在品牌仓构建侧车并打包
cd <品牌仓>
cd extensions/kizemo.focus-sync/sidecar; cargo build --release
cd ../../..; powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build-with-sidecar.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File extensions\kizemo.focus-sync\installer\build.ps1

# 2. 把产出的安装包复制到本仓 release/ 存档
```

**不要**在本仓恢复任何插件源码。两处副本必然漂移。

---

## 4. 许可证与源码获取义务(GPL-3 §6)

本仓历史上以 MIT 分发插件。插件源码迁入 **GPL-3.0-or-later** 的品牌仓后,
随二进制分发的组件整体按 GPL-3 条款分发。

分发二进制时必须同时提供对应源码,获取方式:

| 组件 | 源码位置 |
|---|---|
| **Focus Sync 插件 + sidecar** | <https://github.com/kizemo/alpha-file-manager> → `extensions/kizemo.focus-sync/` |
| Alpha File Manager 本体 | <https://github.com/kizemo/alpha-file-manager> |
| Sigma File Manager(上游基座) | <https://github.com/aleksey-hoffman/sigma-file-manager> |

---

## 5. 这个插件是干什么的

你在文件管理器里切换目录 → 所有已打开的文件对话框自动跳到同一个目录。
Save As / 下载弹窗的地址栏会跟随。

架构:UIA 只认前台对话框,绝不抢焦点;sidecar 由 Windows 计划任务
`KizemoFocusSync` 启动,与文件管理器生命周期解耦。

详见品牌仓 README 与本仓 `docs/`。