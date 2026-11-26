# 回滚说明 —— focus-sync stale-path gate 修复（2026-10-13）

本轮改动了 sidecar 源码并重新部署了二进制。下面是两条回滚路径，都经过实际验证。

---

## 路径 A：换回二进制（最快，秒级，无需重编译）

**适用**：只想让线上立刻回到修复前的状态。

```powershell
$ErrorActionPreference = 'Continue'

# 1. 停掉占用文件的实例（否则覆盖会因文件锁失败，报 os error 5）
Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue |
    ForEach-Object { Stop-Process -Id $_.Id -Force }
Start-Sleep -Seconds 2

# 2. 换回修复前的二进制
Copy-Item `
  'F:\soft\00selfmade\filemanager\_acceptance\rollback-2026-10-13\focus-sync-sidecar.v0.5.10.ORIGINAL.exe' `
  'C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe' `
  -Force

# 3. 校验哈希必须是修复前那一个
(Get-FileHash 'C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe' -Algorithm SHA256).Hash
# 期望：36AC01042BE843402ED9851643F603F00CB1D5B2D8205A9697E94F95BA357E24

# 4. 重启
Start-ScheduledTask -TaskName 'KizemoFocusSync'
```

> ⚠️ 回滚到 v0.5.10 就等于把 2026-10-13 这次故障原样装回去：
> 扩展心跳一旦死掉，所有弹窗的路径同步会全部停摆。这是**已知会坏的版本**。

---

## 路径 B：回滚源码（完整撤销，重编译）

**适用**：要彻底撤掉这次改动而不只是换二进制。

```powershell
$ErrorActionPreference = 'Continue'
$env:PATH="C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;$env:PATH"
[Environment]::SetEnvironmentVariable('PATH',$env:PATH,'Process')

$s = 'F:\soft\00selfmade\filemanager\sigma-file-manager'

# 撤销本次提交（回到它的父提交 087aecd9）
& git -C $s revert --no-edit 311855ed

# 重新构建并部署
Set-Location "$s\extensions\kizemo.focus-sync\sidecar"
cargo build --release
Get-Process focus-sync-sidecar -ErrorAction SilentlyContinue |
    ForEach-Object { Stop-Process -Id $_.Id -Force }
Start-Sleep -Seconds 2
Copy-Item "$PWD\target\release\spike.exe" `
  'C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe' -Force
Start-ScheduledTask -TaskName 'KizemoFocusSync'
```

> 注意：父提交 `087aecd9` **不含** v0.5.10 那批未提交的改动吗？—— 含。
> v0.5.10（`is_classic_shell_signature`、`pick_filename_edit` 等）在本次提交
> 之前就是未提交状态，本次提交把它们一并记录了进去。所以 revert 之后得到的是
> **v0.5.10 的源码状态**，也就是修复前正在运行的那份。

---

## 本次改了什么（便于判断是否需要回滚）

| 文件 | 改动 | 风险 |
|---|---|---|
| `hostproc.rs` | **新增**。用 `tasklist` 读宿主进程 PID | 低。无新依赖 |
| `writer.rs` | stale-path gate 增加第二条放行通道 | 低。**只增不减**：新分支只能把「拒绝」变成「放行」，不可能反向 |
| `state.rs` | 新增 `host_app_stable()` / `gate_open()` / PID 缓存（3 秒） | 低。`extension_alive()` 逐字未改 |
| `http_server.rs` | `/set_path` 增加一次 `note_host_contact()`；`/health` 新增 4 个只读字段 | 低。字段是新增，扩展的 JSON 解析不受影响 |
| `lib.rs` | 注册 `hostproc` 模块 | 无 |
| `uia_inject.rs` / `reader.rs` | **本轮未改动**，是上一轮 v0.5.10 的未提交改动，本次一并提交以便留档 | — |

---

## 验证过的与没验证的

**已验证**
- 89/89 单测通过（原 81 + 新增 8）
- 真机四段确定性门状态验证（A/B/C/D 见提交信息），D 段就是 2026-10-13 的故障态
- 部署二进制哈希与构建产物一致

**未验证**
- **下载弹窗的真机端到端同步** —— 本轮没有完成。Edge 的弹窗无法可靠脚本化触发；
  经典 notepad 对话框不是有效载体（读回路径为空，`should_opened_write_back`
  直接跳过，到不了门）。**需要人工验证。**
- **「打开」对话框（classic shell）的 Ctrl+L 写入路径** —— **尚未实现。**

---

## 回滚演练记录

- 二进制备份哈希：`36AC01042BE843402ED9851643F603F00CB1D5B2D8205A9697E94F95BA357E24`
- 备份文件：`_acceptance/rollback-2026-10-13/focus-sync-sidecar.v0.5.10.ORIGINAL.exe`
- 备份产生时间：2026-10-13 21:39（部署新版本之前）
- 提交：`311855ed`（在 sigma-file-manager 仓库，分支 `docs/navigator-behavior-parity-spec`）

> `sigma-file-manager/` 在**外层** `filemanager` 仓库里是 gitignore 的（注释写明
> "fork 仓库目录，在 sigma-file-manager/ 子目录里独立管理"）。它是独立仓库，
> 回滚命令必须在**它自己的目录**里执行。外层仓库创建的
> `focus-sync-ctrl-l-2026-10-13` 分支对 sidecar 无效（那里看不到这些文件）。