# 交接：安装后三个故障的根因定位与修复（2026-10-10）

对应版本：**2.2.1**
涉及文件：`src-tauri/installer/hooks.nsh`、`src-tauri/src/lib.rs`

---

## 一、用户报的现象

1. 安装后**开始菜单快捷方式消失**
2. 升级时默认路径在 `D` 后少了冒号（`D\Program Files\...`），**「下一步」点不动**
3. 勾选「运行 Alpha File Manager」，**软件没启动**

另外三件事一并查：WebView2 被全局误杀、release 版无日志、启动崩溃 `0xc0000005`。

## 二、根因：三个现象同一个

Tauri 的 NSIS 模板把卸载记录里的 `InstallLocation` 存成**带引号**的字符串：

```nsis
; 生成的 installer.nsi:691
WriteRegStr SHCTX "${UNINSTKEY}" "InstallLocation" "$\"$INSTDIR$\""
```

旧的安装钩子直接把它赋给 `$INSTDIR`：

```nsis
StrCpy $INSTDIR "$0"     ; $0 自身已带引号 -> 指令变成双重引号
```

NSIS 的分词把盘符冒号吃掉：`"D:\Program Files\Alpha File Manager"` → `D\Program Files\Alpha File Manager`。

**路径一旦没有冒号，NSIS 就把它当成相对系统目录的路径**，重写成 `C:\D\Program Files\Alpha File Manager`。
凡是写 `"$INSTDIR\..."` 的地方全部落到这个不存在的根下，于是：

| 现象 | 实际发生了什么 |
|---|---|
| 快捷方式「消失」 | `CreateShortcut` 成功，但目标是不存在的 `C:\D\...` |
| 路径框少冒号、Next 失效 | MUI 目录页把无冒号值当相对路径，显示成 `D\...` |
| 勾选运行没反应 | `RunAsUser "$INSTDIR\alpha-file-manager.exe"` 指向不存在路径 |

### 现场证据（修复前，本机实测）

| 项 | 值 |
|---|---|
| 开始菜单快捷方式目标 | `C:\D\Program Files\Alpha File Manager\alpha-file-manager.exe` ❌ |
| 桌面快捷方式目标 | 同上 ❌ |
| `MANUPRODUCTKEY` 默认值 | `D\Program Files\Alpha File Manager` ❌ 冒号已丢 |
| `Uninstall\...\InstallLocation` | `"D\Program Files\Alpha File Manager"` ❌ 36 字符，首末码位 34 |
| 真实 exe | `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` ✅ |

**注意复利效应**：两轮写回（`MANUPRODUCTKEY` 与 `InstallLocation`）都写的是已损坏的 `$INSTDIR`，
所以每装一次，损坏值就被固化一次，跨版本累积。

### 为什么 Tauris 自己的恢复逻辑没暴露它

Tauri 的 `RestorePreviousInstallLocation`（installer.nsi:878）读的是 `MANUPRODUCTKEY` 的**默认值**，
且用的是**不加引号**的 `StrCpy $INSTDIR $4`。我们自己的钩子读的是带引号的 `InstallLocation`，
并且用**加了引号**的 `StrCpy $INSTDIR "$0"` —— 只有这一条路径会吃掉冒号。

## 三、修复

### 3.1 `AlphaNormalizeInstDir` —— 单一闸门

新增函数，任何候选路径必须先过它才能成为 `$INSTDIR`：

1. 去掉首尾双引号
2. 修复上一轮已丢失的冒号（仅修复明确的 `<字母>\<其余>` 形态，不做其他猜测）
3. 校验必须是 `X:\` 形态（冒号在下标 1），否则**拒绝并保留安装器默认路径**

在已损坏的机器上会**自愈**，而不是静默改装到 `C:\Program Files` 把原目录留成孤儿。

> 实现约束：`hooks.nsh` 被 `installer.nsi` 与卸载器脚本共同 include，两者都**没有注册**
> StrFunc 的 `${StrRep}`，用它会让 makensis 直接失败
> （`You forgot ${Using:StrFunc} StrRep`）。因此本函数只依赖 `StrCpy` / `IntOp` / `${StrLoc}` / LogicLib。
> `${StrLoc}` 的方向参数是**实测**的：`">"` 返回从开头数的 0 基下标，`"<"` 返回从末尾数的。
> 找末字符不依赖长度：追加一个路径里不可能出现的哨兵 `|`，其下标即字符串长度。

### 3.2 `SetOutPath` 顺序 —— 修掉「静默不更新主程序」

生成脚本的顺序是：

```nsis
Section Install
  SetOutPath $INSTDIR          ; <-- $OUTDIR 在此冻结
  !insertmacro NSIS_HOOK_PREINSTALL   ; <-- 本钩子在这里改写 $INSTDIR
  File "${MAINBINARYSRCPATH}"  ; <-- 写的是 $OUTDIR，不是 $INSTDIR
```

`$OUTDIR` 停留在改写前的**驱动器相对路径**。`File`（主 exe + 全部资源）写偏，
而注册表与快捷方式用的是显式 `$INSTDIR`，**看起来一切正常**。

这解释了本轮一个诡异现象：第一次装 2.2.1 时主 exe 静默没换（仍是 2.2.0），
第二次装就好了 —— 因为第一轮跑完后注册表已被自愈。

**这个故障只在「尚未自愈的机器上首次升级」时发作**，也就是最需要升级的那批用户。
修复：钩子在确定 `$INSTDIR` 之后重新 `SetOutPath "$INSTDIR"`。

A/B 实测（同一台机器、同一份注册表损坏状态）：

| | `$OUTDIR` | `$INSTDIR` |
|---|---|---|
| 有修复 | `D:\Program Files\Alpha File Manager` ✅ | 同上 |
| 无修复 | `D\Program Files\Alpha File Manager` ❌ 驱动器相对 | `D:\Program Files\...` |

### 3.3 POSTINSTALL 回写 `InstallLocation`

安装结束后用**不带引号**的值重写卸载记录。让「控制面板 / 应用程序」显示真实路径，
并把带引号这个陷阱对后续所有读者（包括 Tauri 下次升级的模板）彻底消除。

### 3.4 移除 WebView2 / crashpad 全局 kill

旧逻辑 `taskkill /F /IM msedgewebview2.exe /T` 会杀掉**其它应用**的 WebView2 进程。
实测：仅 `taskkill /F /IM alpha-file-manager.exe /T`（其自带 `/T`）就能清掉
Alpha 自己的全部 7 个 `msedgewebview2.exe` 子进程，存活 0；
而同一时刻机器上还有 **6 个属于其他应用**的 WebView2 进程正被全局 kill 波及。
既多余又有害，已删除。

### 3.5 release 版启用文件日志

`lib.rs` 里 `tauri_plugin_log` 原先被 `cfg!(debug_assertions)` 包住，出货版什么都不写 ——
这也是 `0xc0000005` 无迹可寻的原因。现已在 release 也启用，并把上限从插件默认的
40 KB 提到 2 MB（40 KB 事后不够查），并写入一条启动日志（含版本与 exe 路径）。

## 四、验证

| 项 | 结果 |
|---|---|
| `cargo check --lib` | exit 0 |
| `cargo test --lib` | 202 passed / 0 failed |
| `AlphaNormalizeInstDir` 探针 | 13 用例 + 3 对照全部符合预期（负例必须拒绝，证明闸门非平凡） |
| `SetOutPath` A/B 对照 | 见 3.2 表 |
| WebView2 kill 对照 | 自身子进程 7/7 清除；他应用 6 个未受影响 |
| 端到端（故意损坏注册表后用正式包安装） | 见下表 |

端到端结果（正式包 `Alpha File Manager_2.2.1_x64-setup.exe`，19s，exit 0）：

| 项 | 修复前 | 修复后 |
|---|---|---|
| 开始菜单快捷方式目标 | `C:\D\...\alpha-file-manager.exe`（不存在） | `D:\...\alpha-file-manager.exe` ✅ 存在 |
| 桌面快捷方式目标 | 同上 | 同上 ✅ |
| `InstallLocation` | `"D\Program Files\..."` 首字符码位 34 | `D:\Program Files\...` 长度 35，首字符码位 **68 = 'D'** |
| `MANUPRODUCTKEY` | `D\Program Files\...` | `D:\Program Files\...` ✅ |
| 主 exe | 静默未更新（sha `C6A536…`） | 已重写（sha `A18722…`）✅ |
| 启动 | — | 窗口标题 `Alpha File Manager`，日志写入 `Alpha File Manager.log` |

## 五、尚未定位

**启动崩溃 `0xc0000005`**：故障模块是 `alpha-file-manager.exe` 自身，偏移 `0x17ea3fe`，
Report ID `7eb38380-e044-40eb-a8d8-5dbcafbce9db`，发生于 07:47:58。
这是原生层访问违例，Rust panic hook 抓不到。2.2.1 起已有 release 日志，
下次复现时可用它判断进程死在初始化之前还是之后。

本机还残留一个 0 线程僵尸进程 PID 39920（07:47:58 创建，`taskkill` 报 Access denied）。
已实测它**没有**锁住 exe 文件（独占打开测试通过），故与本轮三个问题无关；
但它是那次崩溃的残留，重启后可清除。

## 六、复现根因的最小手段

`src-tauri/target/release/nsis/x64/installer.nsi` 是 Tauri 生成的、不入库，
但它就在本地，可直接改后用 makensis 单独重编，不必重跑整个 `tauri build`：

```powershell
$dir = 'sigma-file-manager\src-tauri\target\release\nsis\x64'
& "C:\Program Files (x86)\NSIS\makensis.exe" /V3 /INPUTCHARSET UTF8 "$dir\installer-diag.nsi"
```

把脚本副本放在 `$dir` 内（它有 `!include "utils.nsh"` 等相对 include），
插 `FileOpen`/`FileWrite` 探针即可读出 `$INSTDIR` / `$OUTDIR`。
**探针输出要写不同的文件** —— 反复用 `FileOpen ... a` 写同一个文件只保留最后一行。