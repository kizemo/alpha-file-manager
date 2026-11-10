# docs/archive —— 归档区说明

本目录存放**从外层文档仓并入的历史材料**，以及**已完结的一次性验收材料**。归档时保留原始相对路径，便于按旧引用回查；"放在 archive 下"本身就是"不再维护"的标记。

## 两个子目录

| 子目录 | 来源 | 内容 | 是否仍参与构建 |
| --- | --- | --- | --- |
| `focus-sync-legacy/` | 外层仓 `F:\soft\00selfmade\filemanager` 的根目录与 `docs/` | 40 份 `handoff-*.md`、11 份 `prompt-*.md`、`task.md`、外层 `README.md`，以及外层 `docs/` 下除"四类例外"之外的全部 markdown | 否 |
| `root-cause/` | 外层 `_acceptance/**` | 根因分析与版本节点审计报告 | 否 |

## 四类不进 archive 的例外

外层 `docs/` 迁移时，下面四类**跳过** `focus-sync-legacy/`，直接落到本仓的正常位置：

- `docs/brand/` —— 品牌素材（应用图标）
- `docs/marketing/` —— 营销物料与草稿
- `docs/blog/` —— 已发布博文
- `docs/SFM-EXTENSION-DEVELOPMENT-GUIDE.md` —— 扩展开发指南

判定理由：这四类**当前仍被引用**（发布说明、官网、扩展开发流程），属于活文档，不是历史留档。把它们放进 archive 会让人误判为已废弃。其余 `docs/` 内容（设计稿、评审记录、sync 笔记、legacy session 记录等）属于历史过程材料，因此进 `focus-sync-legacy/`。

## 引用约定

- 新文档不要引用 archive 内的路径作为"当前依据"；确需引用时须写明"历史材料"。
- archive 内文件不参与换行符统一、lint、格式化等约定，允许保留原样。
- 外层仓已被降级为"迁移源 + 回收目录"，不再接受新文档；本轮之后所有文档一律写入本仓。