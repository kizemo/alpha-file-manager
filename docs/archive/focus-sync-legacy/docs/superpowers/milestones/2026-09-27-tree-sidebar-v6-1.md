# Milestone — `feat/tree-sidebar-v6-1` (Stable Baseline)

**日期**: 2026-09-27
**里程碑状态**: ✓ Stable,所有 fork-only 功能基于此分支开发
**Branch**: `feat/tree-sidebar-v6-1`
**HEAD commit**: `8caf14ae`(6 commits,259 unit tests pass)
**Upstream base**: `aleksey-hoffman/sigma-file-manager` HEAD `fed49cee`
**对应 release**: `v2.2.0-tree.1` (kizemo fork GitHub Releases)

---

## 这是什么里程碑

`feat/tree-sidebar-v6-1` 是 kizemo fork 的**稳定基线**。任何后续 fork-only 功能(包括 `focus-sync`、未来其他实验功能)都应基于此分支 fork 出新的 feature branch,而不是从 upstream main 直接开始。

## 为什么这样定位

1. **259 单测全过**:tree sidebar 功能完整、稳定、覆盖充分
2. **不依赖未发布功能**:完全基于 upstream `fed49cee` 已知 API,无 unknown unknowns
3. **release 工程已走通**:`v2.2.0-tree.1` GitHub Release artifact 完整(SHA256 verified,NSIS installer 通过 SmartScreen)
4. **符合 fork 治理原则**:fork-only feature 不污染 upstream PR review;kizemo 维护成本可控

## 后续 fork-only feature 的标准模式

```
feat/tree-sidebar-v6-1 (milestone, this)
    ↓ fork 出
feat/focus-sync-listary (experimental, planned)
    ↓ release as
v2.3.0-focus.1 (or v2.2.0-tree.1+focus.1 bundled)
```

### 规则

1. **新 feature branch 从 `feat/tree-sidebar-v6-1` fork**,不是从 upstream main
2. **新 feature 应优先做成 Sigma FM 扩展**(参 `docs/superpowers/decisions/2026-09-27-focus-sync-plugin-vs-bundled.md` 的派生决策),避免污染主 fork
3. **集成到 fork NSIS installer** 是允许的(用户偏好),但扩展代码 + 二进制在独立 repo 开发
4. **上游 PR**(如有):只针对 upstream 接受的功能,实验功能留在 fork

## 与本里程碑对齐的现有工作

| Feature | 分支 / 仓库 | 状态 |
|---|---|---|
| Folder tree sidebar | `feat/tree-sidebar-v6-1` (本分支) | ✓ Released `v2.2.0-tree.1` |
| Listary-style focus sync(老 picker 路线) | ~~`feat/dialog-focus-sync`~~ | ✗ 撤回(dead code / counter-example) |
| Listary-style focus sync(spike + extension 路线) | `kizemo/focus-sync` 新仓 | 🚧 0.1.0 开发中 |

## 引用

- Spike 决策:`docs/superpowers/decisions/2026-09-27-listary-focus-sync-decision.md`
- Spike 报告:`docs/superpowers/spike-reports/2026-09-27-listary-focus-sync-spike.md`
- Tree 分支历史:`git log feat/tree-sidebar-v6-1 --oneline | head -10`
- v2.2.0-tree.1 release:`https://github.com/kizemo/sigma-file-manager/releases/tag/v2.2.0-tree.1`

---

## 不要做的事(本里程碑边界)

- ❌ 改 tree sidebar 已合并的功能(除非有 critical bug fix)
- ❌ 把 tree 分支 rebase 到 upstream newer commit(会引入未测过的 breaking change)
- ❌ 把 experimental feature merge 进 tree 分支(违背"stable baseline"定位)
- ❌ 删除或 force-push tree 分支 HEAD `8caf14ae`(已发布的 release 不能丢失历史)