# cc-haha Path Mismatch — C: vs E: 双 Home 现状

**日期**: 2026-09-28
**触发**: 用户自述 Claude 数据存储路径是 `E:\Users\Duanyi\.claude\`，不是默认的 `C:\Users\Duanyi\.claude\`
**验证**: 通过对比 `.claude.json` 存在性、`Cache/` 存在性、最新 mtime、当前活跃 session 等多个指标

---

## 一、现状诊断

### 1.1 两个独立 home 目录

| 路径 | 大小 | 最新 mtime | 角色 |
|---|---|---|---|
| `C:\Users\Duanyi\.claude\` | 12 GB | 15:31 | **老 home**（projects-bootstrap 后几乎空） |
| `E:\Users\Duanyi\.claude\` | 14 GB | 18:47 | **当前主 home**（活跃写入） |

两个目录**独立存在**（不是 symlink，不是 junction），都是真实的 NTFS 目录。

### 1.2 关键差异

| 指标 | C: | E: | 判定 |
|---|---|---|---|
| `.claude.json` | MISSING | 1512 B | E: 是主 home |
| `Cache/` | MISSING | 存在 | E: 是主 home |
| 当前活跃 session | 0 字节占位 | `8edd422a-...jsonl` 1.4MB | E: 是主 home |
| `cc-haha/db/trace-index-v1.sqlite` mtime | 15:31 | 18:47 | E: 是主 home |
| `backups/index-v1.sqlite.pre-projects-bootstrap-20260928_173500.bak` | 124 MB | 124 MB | 两个都有（**可能是同一个文件的不同位置**） |
| 整体大小 | 12 GB | 14 GB | E: 更大 |
| `scheduled_tasks.json` | 空白 | 空白 | 两个都空白（无论哪边） |

### 1.3 项目命名差异

C: 和 E: 的 projects/ 目录名**不同**：

```
C: projects/                              E: projects/
├── C--Users-Duanyi                       ├── C--Users-Duanyi
├── E-办公文件-L1网站                      ├── E-------G1-----00data-analysis       ← Sales BI 项目
├── F-resourse_study                       ├── E-------G1-----00data-analysis--claude-worktrees-desktop-main-58196b95  ← worktree
├── F-soft-00selfmade-filemanager          ├── E-------L1--                         ← L1 站点
└── F-soft-00selfmade-rime_claude          ├── F--resourse-study
                                           ├── F--soft-00selfmade-filemanager       ← 当前活跃
                                           ├── F-soft-00selfselfmade-loop-engineering
                                           └── F-resourse_study
```

**关键观察**:
- E: 有 Sales BI（`E-------G1-----00data-analysis`）和 loop-engineering 项目，C: 没有
- E: 项目名用**双横线** `--`，C: 用**单横线** `-`
- 同一个项目 `F-soft-00selfmade-filemanager` 在两边都存在，但**双横线** vs **单横线**导致两个独立目录

### 1.4 当前活跃会话路径

```
E:\Users\Duanyi\.claude\projects\F--soft-00selfmade-filemanager\
  └── 8edd422a-867b-4a72-bb16-a1f765a80f00.jsonl  (1.4 MB, 18:47 最后写入)
```

这就是用户**当前正在跑的 Claude 会话**的 jsonl。它在 E:，不是 C:。

### 1.5 cc-haha 三层 traces

E: 和 C: 都有 cc-haha portable 残留：

| 路径 | traces 数 |
|---|---|
| `E:\Users\Duanyi\.claude\cc-haha\traces\` | 462（**主目录，最新**） |
| `E:\Users\Duanyi\.claude\.claude\cc-haha\traces\` | 362（portable 残留 1） |
| `C:\Users\Duanyi\.claude\.claude\cc-haha\traces\` | 224（portable 残留 2，老） |

**总计 1048 个 jsonl**，内容有重复。

---

## 二、为什么会同时存在两个 home

最可能的原因：
1. **E: 是数据盘**，用户主动把 Claude 数据迁移到 E: 节省 C: 系统盘空间
2. **cc-haha 客户端配置** `providers.json` 写死了 E: 路径
3. 早期 portable 模式用过 C:，后来切到 E: 永久 home，但 C: 没清理
4. 多个 Claude 客户端实例（cc-haha / 老 desktop）各自用不同 home

---

## 三、为什么之前流程基于 C: 是错的

### 3.1 错误的诊断（commit c8192aa 前）

我之前的诊断脚本里所有路径都基于 `Path.home() / '.claude'`：

```python
# 错误（git status 显示 fh = ~/.claude/file-history，但 ~ 是 C:）
fh = Path.home() / '.claude' / 'file-history'

# 错误（traces 用 ~/.claude/.claude/cc-haha/traces, C: portable 残留）
traces = Path.home() / '.claude' / '.claude' / 'cc-haha' / 'traces'
```

实际 cc-haha 用的是 E:，所以：
- `_memory-audit.csv` (1084 hashes, 167 sids) → 应该是 E: (1123 hashes, 174 sids)
- `docs/legacy-sessions/` (27 sessions) → 应该是 E: (68 sessions)
- `~/.claude/cc-haha.backup-2026-09-28/` (3.4 GB, C: 老数据) → 应该是 E: `~/.claude/cc-haha.e-backup-2026-09-28/` (6.8 GB)

### 3.2 错误的 sidebar recovery 指南

`docs/cc-haha-sidebar-recovery-guide.md`（第一版）用 PowerShell:
```powershell
$dbDir = "$env:USERPROFILE\.claude\cc-haha\db"
```

`$env:USERPROFILE` 是 `C:\Users\Duanyi`，所以 `$dbDir` 解析为 `C:\Users\Duanyi\.claude\cc-haha\db`，**不是 cc-haha 真正使用的 E:！**

按第一版指南删 `C:\Users\Duanyi\.claude\cc-haha\db\trace-index-v1.sqlite` 后重启 cc-haha，**不会有任何效果**，因为 cc-haha 读的是 E: 的。

**修正指南**: `docs/cc-haha-sidebar-recovery-guide-e.md`（E: 路径硬编码）

### 3.3 错误的 cc-haha 备份

`~/.claude/cc-haha.backup-2026-09-28/`（C: 老 cc-haha，13:32 mtime）是**历史的、被遗弃的**数据副本，不是当前活跃数据。

**正确备份**: `E:\Users\Duanyi\.claude\cc-haha.e-backup-2026-09-28\`（正在 cp 中，6.8 GB 最新）

---

## 四、流程修改清单

### 4.1 已完成

| # | 修改 | 产出 |
|---|---|---|
| 1 | 重新生成 audit CSV from E: | `_memory-audit-e.csv`（1123 hashes, 174 sids） |
| 2 | 重新生成 legacy-sessions from E: | `docs/legacy-sessions-e/`（68 sessions, 含 9/28 关键工作） |
| 3 | 重写 sidebar recovery guide 用 E: 路径 | `docs/cc-haha-sidebar-recovery-guide-e.md` |
| 4 | 写路径差异说明 | `docs/cc-haha-path-mismatch.md`（本文件） |
| 5 | 启动 E: cc-haha 备份 | `E:\Users\Duanyi\.claude\cc-haha.e-backup-2026-09-28\`（6.8GB，cp 进行中） |

### 4.2 待用户决策

| # | 决策 | 备注 |
|---|---|---|
| 1 | 是否删除老 `~/.claude/cc-haha.backup-2026-09-28/`（C: 备份）| 节省 3.4GB；与 E: 备份功能重复 |
| 2 | 是否删除 `docs/legacy-sessions/`（C: 27 个）和 `_memory-audit.csv`（C: 1084 hashes）| 与 E: 版本重复，保留会混淆 |
| 3 | 旧 sidebar guide (`-recovery-guide.md` 不带 -e) 是否 git rm | 同上 |
| 4 | 新的 E: 产物（`legacy-sessions-e/`, `_memory-audit-e.csv`, `-recovery-guide-e.md`, `path-mismatch.md`, 新脚本）是否单独 commit 或合并到 c8192aa |

### 4.3 路径使用规则（强制）

**所有 cc-haha 相关操作必须用 E: 路径**：

| 用途 | 正确路径 | 错误路径 |
|---|---|---|
| cc-haha db | `E:\Users\Duanyi\.claude\cc-haha\db\` | `C:\Users\Duanyi\.claude\cc-haha\db\` |
| cc-haha traces | `E:\Users\Duanyi\.claude\cc-haha\traces\` | `C:\Users\Duanyi\.claude\.claude\cc-haha\traces\` |
| 当前活跃 session | `E:\Users\Duanyi\.claude\projects\F--soft-00selfmade-filemanager\8edd422a-...jsonl` | `C:\Users\Duanyi\.claude\projects\F-soft-00selfmade-filemanager\ed1534ed-...jsonl`（0 字节占位） |
| file-history | `E:\Users\Duanyi\.claude\file-history\` | `C:\Users\Duanyi\.claude\file-history\` |
| .claude.json（应用配置）| `E:\Users\Duanyi\.claude\.claude.json` | （不存在于 C:）|
| backups | `E:\Users\Duanyi\.claude\backups\` | `C:\Users\Duanyi\.claude\backups\`（次要备份）|

**检测 home 是 E: 还是 C: 的快速方法**：
```powershell
Test-Path "E:\Users\Duanyi\.claude\.claude.json"  # 应为 True
Test-Path "C:\Users\Duanyi\.claude\.claude.json"  # 应为 False
```

---

## 五、给未来会话的避坑提示

1. **永远先验证 home 路径**：用 `Test-Path E:\Users\Duanyi\.claude\.claude.json` 判断主目录
2. **不要用 `$env:USERPROFILE`**：会指到 C:（不是 cc-haha 真正用的）
3. **不要用 `Path.home()` in Python**：同样指到 C:
4. **PowerShell 路径硬编码 E:**：避免环境变量陷阱
5. **写新脚本时支持 --home 参数**：让用户显式指定 home 路径
6. **commit message 不要硬编码路径**：用 "primary home" 等抽象描述，避免路径漂移误导

---

## 六、参考

- 修复指南: `docs/cc-haha-sidebar-recovery-guide-e.md`
- 重做产物: `_memory-audit-e.csv`, `docs/legacy-sessions-e/`
- 老 commit (基于 C: 的): c8192aa `docs(recovery): catalog handoffs + extract cc-haha legacy sessions`
- git handoff: `handoff-2026-09-28-session-recovery.md`（也需要加路径修正注）