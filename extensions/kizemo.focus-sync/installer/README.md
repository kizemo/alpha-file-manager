# Focus Sync 扩展独立安装器

本目录是 Focus Sync（`kizemo.focus-sync`）扩展的独立 NSIS 安装器。它把扩展注册进
**Alpha File Manager**，让扩展出现在「已安装」标签页里。

## 侧车生命周期（v0.5.9 起）

侧车（`focus-sync-sidecar.exe`）**由 Alpha File Manager 主进程持有**：主进程启动时拉起它，
退出时结束它。命令行只有 `--port 37421`，不带 `--service`。

因此本安装器：

- **不再注册** legacy 的 `KizemoFocusSync` 计划任务（AtLogOn + `--service`）；
- 只做**幂等清理**——把旧版本可能残留的该任务注销掉（`remove-legacy-focus-task.ps1`）；
- 部署前会结束仍在运行的侧车进程（`stop-sidecar-processes.ps1`），避免 exe 被占用导致覆盖失败。

## 目录内容

| 文件 | 作用 |
|---|---|
| `installer.nsi` | NSIS 源码，编译产物为 `kizemo.focus-sync-0.5.9-setup.exe`。 |
| `build.ps1` | 一键构建脚本（调用 `makensis.exe`）。 |
| `register.ps1` | 往 `user-extensions.json` 里登记扩展条目。 |
| `unregister.ps1` | 从 `user-extensions.json` 里移除扩展条目。 |
| `remove-legacy-focus-task.ps1` | 注销 legacy `KizemoFocusSync` 计划任务（幂等）。 |
| `stop-sidecar-processes.ps1` | 结束仍在运行的侧车进程（幂等）。 |
| `register-scheduled-task.ps1` | **已停用**，仅作为 legacy 任务参数的出处留档。 |
| `kill-sigma.ps1` / `reopen-sigma.ps1` | 安装前关闭 / 安装后重开 Alpha File Manager。 |
| `README.md` | 本文件。 |

## 构建

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File build.ps1
```

产物：`kizemo.focus-sync-0.5.9-setup.exe`（约 3 MB），同时复制一份到上一级目录。

## 使用

### 安装

> **重要 —— 运行安装器前必须彻底退出 Alpha File Manager。**
>
> 安装器会**硬阻断**：检测到 `alpha-file-manager.exe`（或旧名 `sigma-file-manager.exe`）
> 仍在运行时会弹窗询问，关闭尝试之后进程仍存活则直接中止安装。
> 原因是应用内存里留着 `user-extensions.json` 的旧副本，下次保存时会用陈旧状态覆盖
> 磁盘上的注册信息 —— 结果就是「装上了却没生效」，而且全程没有任何报错。

1. 右键任务栏图标 → 退出 Alpha File Manager，确认任务管理器里没有 `alpha-file-manager.exe`。
2. 双击 `kizemo.focus-sync-0.5.9-setup.exe`。
3. 按向导走完：安装器会把扩展文件复制到 `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\`，
   把侧车二进制部署到 `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`，
   并改写 `user-extensions.json`。
4. 打开 Alpha File Manager。
5. 按 **Ctrl+Shift+X** 打开扩展面板。
6. 「已安装」标签页应当列出 **Focus Sync**（作者 kizemo）。
7. 打开开关，工具栏应当出现 **Sync Now / Enable / Disable**。
8. 实测：打开任意应用的「另存为」对话框（记事本、Chrome、Word 等），
   在 Alpha File Manager 里切换目录，对话框应当自动跳到当前目录。

### 卸载

1. 关闭 Alpha File Manager。
2. 打开 `appwiz.cpl`（应用和功能）。
3. 找到「Focus Sync Extension」→ 卸载。

也可以重新运行安装包，在欢迎页选择卸载。卸载会同时删除扩展文件、注销条目、
注销 legacy 计划任务并结束侧车进程。

## 工作原理

Alpha File Manager 的「已安装」标签页读的是
`%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json` 里的
`installedExtensions` 字段，**不是**扫描 `extensions/` 目录。所以只复制文件是不够的，
必须在 `user-extensions.json` 里登记条目。

`register.ps1` 会读取该文件、解析 JSON、写入 `kizemo.focus-sync` 条目（`isLocal: true`，
否则应用启动时的 `removeUnapprovedInstalledExtensions()` 会把它当成「未上架」删掉），
再写回磁盘。版本号从**已部署的** `package.json` 读取，不再硬编码，避免与发布版本漂移。

## 为什么必须先关闭应用？

应用用的是 Tauri LazyStore + 一次性启动引导。进程一旦在跑，它的内存状态就和磁盘文件脱钩了。
运行期间改 `user-extensions.json`，改动虽然写进了磁盘，但应用内存里仍是旧状态，
下一次保存（任何 UI 操作、定期落盘）就会用陈旧状态覆盖回去，注册信息等于白写。

## 装完 Focus Sync 还是没出现怎么办？

1. 确认文件确实被改了：

   ```powershell
   Get-Content "$env:APPDATA\com.sigma-file-manager.app\user-data\user-extensions.json" |
       ConvertFrom-Json |
       Select-Object -ExpandProperty installedExtensions |
       Select-Object *, @{N='kizemo';E={$_.PSObject.Properties.Name -contains 'kizemo.focus-sync'}}
   ```

   `kizemo` 这一行应当显示 `True`。

2. 确认扩展文件在位：

   ```powershell
   Test-Path "$env:APPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\package.json"
   Test-Path "$env:APPDATA\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe"
   ```

   两条都应当返回 `True`。

3. 关掉应用后手工重跑一次 `register.ps1`。

4. 仍然不生效的话，看应用 DevTools 控制台里 manifest 的 schema 校验报错。

## 已知约束

- 安装器里调用 PowerShell 一律用 `powershell -File "<绝对路径>"` 形态。
  内联的 `powershell -Command "..."` 在安装器上下文里不可靠（3.13.5 真机实测）。
- 本目录下的 `.ps1` 全部是**纯 ASCII** 文件：PowerShell 5.1 按系统 ANSI 代码页读取无 BOM 的
  脚本，脚本里出现中文会被解析成乱码。需要中文时走命令行参数。
- `register-scheduled-task.ps1` 已停用，保留它只是为了记录 legacy 计划任务的
  `RestartCount 3 / RestartInterval 1min` 这两个参数的出处。
