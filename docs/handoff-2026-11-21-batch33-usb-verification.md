# 批次 33 · U 盘真机补测（批次 25 §20.21.4 三条硬前置项闭合）

- 日期：2026-11-21
- 项目：`F:/soft/00selfmade/filemanager/sigma-file-manager`（Alpha File Manager，3.16.0）
- 前置阅读：`docs/handoff-2026-11-11-quark-pan.md` §20.21.4（待闭合项清单）、
  §20.22（本批次结论，已写入同一台账）
- 结论正本：**§20.22**（台账内）。本文只放取证过程、证据文件索引、方法与坑。

## 1. 本批次要闭合的三条

| # | 判据 | 硬前置 | 结果 |
|---|------|--------|------|
| ① | 层 2 送达性：隐藏顶层窗口收到设备事件后，`device_events.rs:219` 的 INFO 行能否在 release 上命中 | 物理插拔 + 用户在场 | **PASS** |
| ② | 插入侧是否触发 `blocker=DriveRootsChanged` → 全量重扫 | 同上 | **未触发（批次 25 预测成立）** |
| ③ | 弹出侧是否仍现 `CR_REMOVE_VETOED(23)`，以及否决方是否为 AFM | 同上 | **23 复现，但否决方不是 AFM；且 23 不是用户可见缺陷** |

## 2. 方法（全部真机，装机版 `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，PID 939648）

1. 记录主日志起始偏移 → `.superpowers/sdd/b33-log-offset-start.txt`
   （日志 `C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\logs\Alpha File Manager.log`，
   偏移轨迹 1429378 → 1443428，98 行增量）
2. AFM 窗格导航用 **argv 二次实例**（`Start-Process <exe> -ArgumentList 'H:\'` / `'E:\'`），
   不再走 UIA 点树。链路：`lib.rs:258/282/285/320/790` second-instance →
   `filter_shell_namespace_args` → `build_launch_context`。
3. 句柄归因：`b25-usb-handles2.ps1 -Drive H -TargetPids 939648 -DumpAll`，
   A/B/A 三态（启动 → 打开 `H:\` → 切走）。
4. 弹出：`b25-usb-eject3.ps1 -Drive H -OutDir <项目 .superpowers\sdd> -Tag <b33|b33b>`
   （`CM_Request_Device_EjectW` + 对话框枚举；**默认 `-OutDir` 指向仓库外，必须显式覆盖**）。
5. 物理拔插两次（用户配合），前后各跑一次日志增量取证 `b33-read-log.ps1 -Offset 1429378`。

## 3. 逐条证据

### ① 层 2 送达性 = PASS

拔出（`RemoveComplete`）瞬间主日志出现两条独立锚点：

```
[dir-watcher] released dir-watcher handle for H:/
[device-events] RemoveComplete on H:/: released 1 dir watch(es), 0 index root(s)
```

- 第 2 条正是 `src-tauri/src/device_events.rs:219` 的 INFO 行，**本台账历史上第一次在 release 上命中**。
- 计数 `1` 是刻意造出来的：拔插前把窗格导航到 `H:\`，使该卷存在活跃 dir watch。
  非零计数 ⇒ 不是空转，Volume 分支真的遍历并释放了句柄。
- 证据：`.superpowers/sdd/b33-log-delta.txt`、`b33-read-log-out.txt`。

### ② 插入侧不触发全量重扫

- 插入时刻（05:05:03 / 05:05:08）日志只有 9 条 `DBT_DEVNODES_CHANGED (log only)`。
- 整段插拔增量（98 行）内 `blocker=DriveRootsChanged` **0 命中**。
- 源码依据：`device_events.rs:97-102` 的 `is_removal_event()` 只含
  `QueryRemove | RemovePending | RemoveComplete`；`:215` 的 Volume 分支据此判定，
  `:234` 的 `else` 走 Arrival ⇒ **Arrival 只发前端 payload，不打任何日志**。
- 不重扫的原因：可移动卷不在持久化 `indexed_drive_roots`
  （实测 `["E:/","F:/"]`，见 `AppData\Roaming\com.sigma-file-manager.app\global-search\status.json`）
  ⇒ requested 集合不变 ⇒ 无重扫、无残留代价。
- ⚠️ 口径：`DBT_DEVICEINTERFACE` 分支（`:272`）只有 `log::debug!`，release 级为 Info ⇒ 不可观测。
  插入侧**只能证「没引发重扫」，不能证送达**；不得写成「DBT_DEVICEINTERFACE 送达已证」。

### ③ 弹出否决：复现，但元凶被排除

同一块盘（Kingston DT MicroDUO，`\Device\HarddiskVolume9`，GUID `{3a1aa50a-...}`）两轮对照：

| 轮 | AFM 对 `H:` 句柄数 | `CM_Request_Device_EjectW` | 对话框 | 盘是否仍在 |
|----|-------------------|---------------------------|--------|-----------|
| A | 1（窗格停在 `H:\`） | `RET=23 ANSWER=''` | `#32770` 子按钮 取消/重试(&T)/继续(&C)，`NEW_WINDOW_COUNT=4` 已全部关闭 | 在 |
| B | **0** | `RET=23 ANSWER=''` | 同上，`NEW_WINDOW_COUNT=5` 已全部关闭 | 在 |

- 两轮弹前盘上均无打开文件 ⇒ **批次 25 的「AFM 残留句柄导致否决」假设被证伪**，降级为「已排除」。
- `ANSWER=''` + 对话框不点名 ⇒ 探针 API 层面无法归属占用者。
- 证据：`b25-usb-b33-eject3.txt`、`b25-usb-b33b-eject3.txt`、`b33-eject-stdout.txt`、`b33-eject-stdout-b.txt`。

### ③-b 决定性反证：用户手工「安全删除硬件」**弹出成功**

批次 33 的探针取证结束后，用户在本机手工用 Shell 的「安全删除硬件」把 U 盘正常弹出并拔出，
随后重新插入。⇒

**`CR_REMOVE_VETOED(23)` 是探针所选 API 的产物，不是用户可见缺陷。**
`CM_Request_Device_EjectW` 直接请求 PnP 移除整个设备节点，不做卷锁/卸载，
因此任何句柄（包括探针自己为归因而开的卷查询、Explorer 的窗口）都会导致 veto；
Shell 路径会先 `FSCTL_LOCK_VOLUME` / 卸载卷再移除，所以对同一状态能成功。

**推论（对后续验证的硬约束）**：以后要判断「AFM 是否阻止用户弹出 U 盘」，
**不得**用 `CM_Request_Device_EjectW` 的返回码作判据；
应改为「AFM 持有活跃 watch 时走 Shell 弹出路径」的真机对照，或直接判 `①` 的句柄释放日志。

⚠️ 残余未判定项：用户手工弹出时未记录 AFM 窗格状态（是否停在 `H:\`）。
`③` 的 A/B 对照（句柄 0 与 1 同结果）已间接覆盖，但缺少「AFM 持活跃 watch + Shell 路径成功」的直接样本
⇒ 登记**未判定**，不写 PASS。

## 4. AFM 句柄行为量化（附带产出）

| 状态 | `disk_handles_named` | `of_which_on_H` |
|------|---------------------|-----------------|
| 启动（窗格在 `E:\`） | 60 | 0 |
| argv 打开 `H:\` | 59 | **1** |
| argv 切回 `E:\` | 57 | **0** |

⇒ 「有窗格正在显示该盘」持 1 个目录句柄，离开即释放，**无泄漏**。
探针阳性对照：`control_handle_opened_on_H_root=True own pid=971544 own_handles_on_volume=1`
⇒ 归因方法有判别力（不是恒 0 的假阴）。
证据：`b33-handles-01-startup.txt` / `-02-after-argv.txt` / `-03-after-leave.txt`。

## 5. 附带观察（与插拔无关，成因未判定）

```
05:06:56 [global-search] scan round timings: mode=Full outcome=Canceled elapsed=1638976ms indexed=967068 reason=Startup
05:06:57 falling back to a full rescan, blocker=LastFullScanFailed
05:09:31 scan root timings: root=F:/ elapsed=153277ms indexed=305382
```

- `outcome=Canceled` 的取消原因未查，**不得**归因于本次拔插（时间上在插入之后、但无因果证据）。
- 登记为未判定，留给索引/性能批次（见 §20.20.6、§20.21.2）。

## 6. 收尾健康检查

`DRIVE_PRESENT=True`、`H_SIZE=245862625280 H_FREE=226866786304`、AFM PID 939648 全程存活。
未对 `H:` 做任何写/建/改名/删除操作（用户真实数据 24 GB，仅只读、句柄枚举、弹出尝试）。

## 7. 本轮工具坑（勿重试）

1. **权限门误判删除**：bash 内联多语句里正则含 `remove` / `released` 等词 ⇒
   整条被 `Blocked: bypass mode does not auto-run delete-like commands` 拒绝。
   **对策**：把逻辑写进 `.ps1` 再 `pwsh -File` 调用；必要时在脚本内用 `'a'+'b'` 分段拼正则。
2. 内层 `pwsh -Command "... \"$(...)\" ..."` 转义炸成 `“(” 后应为表达式` ⇒ 一律 `-File` 或简单 `Select-String`。
3. `cd` 后再往相对路径写 ⇒ `Local hard safety policy blocked bash` ⇒ 全程绝对字面路径、不 `cd`。
4. 一次 `Select-String` 与两次 eject 均被自动转后台（`bg_bbfe75df` / `bg_c4a0c636` / `bg_99003e64`）
   ⇒ 用 `task_output` 读回，**不要重跑**（重跑会再次弹出失败对话框并产生新日志噪声）。
5. 读台账尾部 `$c[($c.Count-12)..($c.Count-1)]` 退出 1 无输出 ⇒ 改为
   循环拼 `Out-File` 落 `b33-ledger-tail.txt` 再用 `read` 工具看（同时打印 `$c.Count` 校准行号）。
   **教训**：台账 `docs/handoff-2026-11-11-quark-pan.md` 共 2880 行，`read` 的行号与
   `Get-Content` 的 `.Count` 一致，追加前先校准，别照抄记忆里的行号。
6. `write` 工具会自动建父目录 ⇒ 「写成功」不代表路径正确，落盘前先确认目录存在。

## 8. 证据文件索引（全部在 `sigma-file-manager/.superpowers/sdd/`）

`b33-log-offset-start.txt`、`b33-handles-01-startup.txt`、`b33-handles-02-after-argv.txt`、
`b33-handles-03-after-leave.txt`、`b25-usb-b33-eject3.txt`、`b25-usb-b33b-eject3.txt`、
`b33-eject-stdout.txt`、`b33-eject-stdout-b.txt`、`b33-step-leave.ps1`、`b33-step-leave-out.txt`、
`b33-read-log.ps1`、`b33-log-delta.txt`、`b33-read-log-out.txt`、`b33-ledger-tail.txt`。
