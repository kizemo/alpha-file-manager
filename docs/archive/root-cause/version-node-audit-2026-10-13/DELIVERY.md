# 交付报告：sidecar 回退到 0.5.8 + 重新打包 + 运行时替换

**日期**：2026-10-13 23:20（北京）
**执行路径**：路径 A（不动 git，只重新打包）+ 一并替换运行时
**结果**：打包与替换全部成功；**但端到端功能仍未恢复，原因是扩展侧从未激活，与 sidecar 版本无关**

---

## 1. 版本节点考古结论

用户要求：「找到 3.0.3 修改过程中，在明确开始修改侧车以实现『文件打开路径同步』之前的打包发版记录，恢复到该操作之前的最晚版本节点。」

| 项 | 值 |
|---|---|
| **目标节点** | **`9ed9440a` / 应用 3.0.2** |
| 打包时间 | 2026-10-12 16:43:15 |
| 内含 sidecar | **0.5.8** |
| 是否上线 | **是**（3.0.3 之前唯一真正发布的版本） |
| 3.0.3 是否上线 | **否** —— 提交 `50c3ec97` 标题自认「验证产物,未发布」 |

完整证据链见同目录 `VERSION-NODE-AUDIT.md`。

### 关键反转

```
git diff --stat 9ed9440a f404215e -- extensions/kizemo.focus-sync
（空）
```

当前源码的 sidecar 与 3.0.2 **逐字节相同**（都是 0.5.8）。
上一轮的回滚已经把 sidecar 带回 3.0.2 状态，**「恢复」在 sidecar 维度无需任何 git 操作**。

源码与 3.0.2 的剩余 66 文件差异（+3390 / −215）**全部在应用本体**，与 sidecar 无关，故按用户选择保留。

---

## 2. 构建与打包

| 步骤 | 结果 |
|---|---|
| `cargo test --lib`（0.5.8 源码基线） | **73/73 通过**，exit 0 |
| `cargo build --release` | exit 0，产物 `spike.exe` 2,831,360 bytes |
| 新构建 sidecar 指纹 | sha256 `7C7EB7C3EB100E71…`，内嵌版本串 `0.5.7 0.5.8` → **0.5.8** |
| 进包路径替换 | `extensions/kizemo.focus-sync/bin/focus-sync-sidecar.exe`：0.5.10（`36AC0104`）→ **0.5.8**（`7C7EB7C3`） |
| `npm run tauri:build` | **exit 0** |
| 新安装包 | `release\Alpha File Manager_3.0.3_x64-setup.exe`，2026-10-13 23:16:15，18,100,242 bytes，sha `9DAC2305B114E554…` |

### 安装包内容验证（7-Zip 解包实测）

| 载荷 | 实际 | 期望 | 结论 |
|---|---|---|---|
| `tools\focus-sync-sidecar.exe` | 2,831,360 / `7C7EB7C3` / 版本串 `0.5.8` | 同左 | **吻合** |
| `extensions\kizemo.focus-sync\dist\index.js` | 38,550 / `9D2B602E` | 工作区 36 行改动版 | **吻合** |

### 打包配置确认

`src-tauri/tauri.conf.json`：

```json
"../extensions/kizemo.focus-sync/bin/focus-sync-sidecar.exe": "tools/focus-sync-sidecar.exe"
```

替换会被打进安装包，已由解包结果证实。

---

## 3. 运行时替换

| 步骤 | 结果 |
|---|---|
| 备份原 0.5.11 | `deployed-0.5.11.BACKUP.exe`，sha `8C3DFFB12AFF7A92…`，2768 KB |
| 关闭应用 | pid 110328 已终止 |
| 停止计划任务 | `KizemoFocusSync` 已停，sidecar pid 130288 已停 |
| 替换二进制 | 2,833,920 → **2,831,360**，sha `7C7EB7C3EB100E71…` |
| 启动计划任务 | `Running`，sidecar pid **105212**，23:17:22 启动 |
| 启动应用 | pid **123320**，23:17:40 启动 |
| `/health` 自报 | **`version: "0.5.8"`**，`uia_healthy: true`，`session_id: 1` |

**运行时版本替换已确认生效。**

---

## 4. 未恢复的部分（重要）

### 4.1 现象

应用启动 131 秒后：

```json
{
  "version": "0.5.8",
  "uia_healthy": true,
  "last_ping_received_ts": 0,
  "first_push_received": false,
  "last_health_poll_ts": 0,
  "extension_alive": false
}
```

扩展 trace（`user-extensions.json` → `__focus_sync_trace`）**零新增**：
最后一条仍是 `2026-10-13T08:22:15.130Z`（北京 16:22），`seq=154`，属上一次 3.0.3 安装的会话。
本次启动连 `N01.activate.start` 都没有产生。

应用日志（`Alpha File Manager.log`，mtime 23:18:06）中**没有任何扩展加载/激活记录**，只有 GitHub 超时。

### 4.2 结论

**扩展 Worker 从未启动，因此从未与侧车发生任何接触。**
下载弹窗与打开弹窗的路径同步**均未恢复**，且**未经验证**。

侧车版本不是根因 —— 把 0.5.11 换成 0.5.8 之后，零接触现象与之前完全一致。

### 4.3 已知线索

1. **`worker-runtime.ts` 缺 `onerror`**：第 38 行 `new Worker()`，第 76 行只设 `worker.onmessage`，
   没有 `onerror` / `onmessageerror`。`extension-worker.ts:1084` 的 `await import(entryUrl)`
   一旦失败，`postWorkerRequest` 返回的 Promise 永远 pending，**且零日志**。
   两个假设（Worker 加载失败 / Worker 被回收）目前无法区分。

2. **网络**：本次实测 PowerShell 直连 `raw.githubusercontent.com` **也已超时**
   （会话早前同一地址返回 200）。应用日志显示对 `registry.json` 等的请求持续超时。
   「本地扩展激活是否被市场源请求阻塞」是一个待验证假设，本次没有证据支持也没有排除。

3. 部署的扩展产物未被破坏：`dist/index.js` 38,550 bytes / sha `9D2B602E`，与安装包一致。

---

## 5. 可回滚性

| 项 | 位置 |
|---|---|
| 0.5.11 部署备份 | `deployed-0.5.11.BACKUP.exe`（本目录） |
| 0.5.10 部署备份 | `..\rollback-2026-10-13\focus-sync-sidecar.v0.5.10.ORIGINAL.exe` |
| git 状态 | 未做任何 git 操作，HEAD 仍为 `f404215e` |
| git 工作区 | 仍只有 `extensions/kizemo.focus-sync/dist/index.js` 一处改动（用户要求保留） |

恢复 0.5.11 只需把 `deployed-0.5.11.BACKUP.exe` 复制回部署路径并重启计划任务。

---

## 6. 下一步建议

优先解决**扩展不激活**，而不是继续调 sidecar。理由：sidecar 已是 3.0.2 的正确版本，
且 73/73 测试通过；在扩展不与侧车接触的前提下，任何 sidecar 侧改动都无法端到端验证
（会产出「单测全绿但实际无效」的东西）。

最小可观测化改动：给 `worker-runtime.ts` 的 `new Worker(...)` 补 `onerror`，
并给 `extension-worker.ts` 的 `await import(entryUrl)` 加 try/catch 上报。
有了这一步，才能区分上面两个假设。