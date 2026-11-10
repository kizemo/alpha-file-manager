# cc-haha Sidebar 历史会话恢复 — E: 路径修正版

**症状**: 重启 cc-haha 后 sidebar 仍无历史会话
**根因**: `E:\Users\Duanyi\.claude\cc-haha\db\trace-index-v1.sqlite` 537/667 degraded（v2 fingerprint vs v6 schema 不匹配），最后索引时间 2026-09-28 10:46
**重要**: ⚠️ `$env:USERPROFILE\.claude\` 在 Windows 上解析到 `C:\Users\Duanyi\.claude\`，**不是 cc-haha 真正使用的 E:**。所有路径必须**硬编码**到 `E:\Users\Duanyi\.claude\`。

**E: cc-haha 完整备份**: `E:\Users\Duanyi\.claude\cc-haha.e-backup-2026-09-28\` (cp 进行中, 6.8GB)

---

## ⚠️ 操作前必读

1. **路径硬编码**：不要用 `$env:USERPROFILE\.claude\`，那会指到 C: 老目录
2. **必须在 cc-haha 完全关闭状态下操作**（spike 进程持有文件句柄）
3. **不要动 `E:\Users\Duanyi\.claude\cc-haha\traces\`**（3GB+ API call logs，删除 = 永久丢失）
4. **不要删 `E:\Users\Duanyi\.claude\cc-haha\db\index-v1.sqlite`**（这是 cc-haha 的 session 索引，与 trace-index 不同）
5. **trace-index 是派生数据**，可删，cc-haha 会从 traces 重建

---

## 操作步骤（PowerShell）

### 第 1 步：关闭 cc-haha

```powershell
Get-Process -Name "Claude Code Haha" -ErrorAction SilentlyContinue | Stop-Process -Force
```

**验证**：
```powershell
Get-Process -Name "Claude Code Haha" -ErrorAction SilentlyContinue
# 应返回空
```

### 第 2 步：确认备份存在

```powershell
$eBackup = "E:\Users\Duanyi\.claude\cc-haha.e-backup-2026-09-28"
Test-Path "$eBackup\db\trace-index-v1.sqlite"
# 必须返回 True
```

如果备份未完成，先等 cp 完成（看 `Get-ChildItem $eBackup\traces | Measure-Object`）。

### 第 3 步：删除 E: 的 trace-index 数据库

```powershell
$dbDir = "E:\Users\Duanyi\.claude\cc-haha\db"
Remove-Item "$dbDir\trace-index-v1.sqlite" -Force
Remove-Item "$dbDir\trace-index-v1.sqlite" -Force -ErrorAction SilentlyContinue  # 'trace-index-v1 (1).sqlite' 也删
Remove-Item "$dbDir\trace-index-v1.sqlite-shm" -Force -ErrorAction SilentlyContinue
Remove-Item "$dbDir\trace-index-v1.sqlite-wal" -Force -ErrorAction SilentlyContinue
# 注意: E: 有 'trace-index-v1 (1).sqlite' 老副本（13:26 mtime），建议同时删
Remove-Item "$dbDir\trace-index-v1 (1).sqlite" -Force -ErrorAction SilentlyContinue
Remove-Item "$dbDir\trace-index-v1 (1).sqlite-shm" -Force -ErrorAction SilentlyContinue
Remove-Item "$dbDir\trace-index-v1 (1).sqlite-wal" -Force -ErrorAction SilentlyContinue
```

**注意**: `(` 和 `)` 在 PowerShell 是合法字符但需要 escape 或 quote。带空格文件名要用 `"` 引号。

**验证**：
```powershell
Get-ChildItem "$dbDir\trace-index-v1*.sqlite*"
# 应返回空
```

### 第 4 步：重启 cc-haha

```powershell
Start-Process "C:\Program Files\Claude Code Haha\Claude Code Haha.exe"
# 或开始菜单
```

### 第 5 步：等待 cc-haha 重建索引

- cc-haha 启动后会扫描 `E:\Users\Duanyi\.claude\cc-haha\traces\`（462 个 jsonl，约 6.2 GB）
- **预计耗时**: 10-30 分钟（取决于磁盘 I/O）
- **观察方法**: 在 cc-haha sidebar 看是否有 "Reindexing..." 状态条，或用 PowerShell 监控文件大小：
```powershell
while ($true) {
    $f = "E:\Users\Duanyi\.claude\cc-haha\db\trace-index-v1.sqlite"
    if (Test-Path $f) {
        $sz = (Get-Item $f).Length / 1MB
        Write-Host "[$(Get-Date -Format 'HH:mm:ss')] trace-index: $sz MB"
    } else {
        Write-Host "[$(Get-Date -Format 'HH:mm:ss')] trace-index: not yet created"
    }
    Start-Sleep 10
}
```

### 第 6 步：验证 sidebar

- 打开 cc-haha sidebar
- 应该看到 ~462 个历史 session
- 当前活跃会话 `8edd422a-867b-4a72-bb16-a1f765a80f00` 应该在最上面

### 第 7 步：可选清理

重建完成后，可手动清理孤儿 sessions（DB 里 447 条对应文件已删）：
```powershell
# rebuild 会自动 reconcile，通常不需要手动操作
```

---

## 失败回滚（5 步）

如果 rebuild 后 sidebar 仍空 / 启动报错 / 数据错乱：

### 步骤 A：关闭 cc-haha（同第 1 步）

### 步骤 B：从 E: 备份恢复

```powershell
$dbDir = "E:\Users\Duanyi\.claude\cc-haha\db"
$bakDir = "E:\Users\Duanyi\.claude\cc-haha.e-backup-2026-09-28\db"

# 删除当前（坏）文件
Remove-Item "$dbDir\trace-index-v1*" -Force -ErrorAction SilentlyContinue

# 从备份恢复
Copy-Item "$bakDir\*" $dbDir -Recurse -Force
```

### 步骤 C：重启 cc-haha（同第 4 步）

### 步骤 D：确认 cc-haha 用的是旧 DB（数据回到 18:46 状态）

### 步骤 E：上报问题

完整回滚（极端）：
```powershell
Remove-Item -Path "E:\Users\Duanyi\.claude\cc-haha" -Recurse -Force
Copy-Item -Path "E:\Users\Duanyi\.claude\cc-haha.e-backup-2026-09-28" -Destination "E:\Users\Duanyi\.claude\cc-haha" -Recurse -Force
```

---

## 进阶：手动验证 trace-index 重建完成

```powershell
python -c "import sqlite3; c=sqlite3.connect(r'E:\Users\Duanyi\.claude\cc-haha\db\trace-index-v1.sqlite'); print('sessions:', c.execute('SELECT COUNT(*) FROM trace_sessions').fetchone()[0], 'ready:', c.execute(\"SELECT COUNT(*) FROM trace_sources WHERE state='ready'\").fetchone()[0])"
```

**期望**:
- `sessions`: 462 ± 几个
- `ready`: 接近 462
- `degraded`: 0（重建成功后）

---

## 为什么路径必须硬编码到 E:

| 验证项 | C: `~/.claude/` | E: `~/.claude/` |
|---|---|---|
| `.claude.json` | MISSING | 1512 B（**主目录**） |
| `Cache/` | MISSING | 存在（**主目录**） |
| 最新 mtime | 15:31 | 18:47（**最新**） |
| 当前活跃 session | 无 | `8edd422a-...jsonl` 1.4MB |
| Trace count | 227 | 462 |
| `cc-haha/db/trace-index-v1.sqlite` 大小 | 134.7 MB | 135.2 MB |
| `scheduled_tasks.json` | 空白 | 空白（两个都空白） |

**结论**: cc-haha 客户端实际工作目录是 `E:\Users\Duanyi\.claude\`，不是 C:。详见 `docs/cc-haha-path-mismatch.md`。