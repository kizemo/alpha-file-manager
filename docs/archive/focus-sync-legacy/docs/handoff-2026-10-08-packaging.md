# HANDOFF — 打包任务(focus-sync v0.5.8 → 安装包)

**交接时间**:2026-10-08 17:55
**任务**:把插件打包成 exe 安装包,并整合进 Sigma FM 做成单一安装包
**给新会话的第一句话**:见文末 §7

---

## 0. 三十秒速览

| 项 | 值 |
|---|---|
| 仓库 | `F:\soft\00selfmade\filemanager`(本地)+ `github.com/kizemo/focus-sync`(公开) |
| HEAD | `43bd1f2`,工作区**干净** |
| **打包基线标签** | **`v0.5.8-packaging-baseline-20261008`** ← **出问题就回滚到这里** |
| 上一回退点 | `v0.5.8-user-verified-20261008` (`64962ad`)/ `v0.5.7-verified-20261008` (`1d79983`) |
| 远程 | 本地领先 `origin/main` **2 个提交尚未推送** |
| 运行中 | sidecar PID 见 `/health`,`version=0.5.8`,`extension_alive=True` |
| 已验证 sha | `index.js = ECED2EBE4D1598BA…` / `sidecar.exe = AA4CB76716268FEF…` |
| 测试 | 73/73 通过 |
| 沙箱门禁 | `VALID (0 violations / 19 patterns)` |

---

## 1. ⚠️ 本任务最重要的一条约束

> **历史上:手动部署功能正常 → 改用安装包安装 → 功能丧失。**
>
> 因此本任务的**验收标准不是「安装成功」,而是「安装后的文件与手动部署完全一致」。**

这不是一个可以靠「装完试试看」满足的要求。安装包路径曾经失败过一次,而失败原因**至今没有定论** —— 不要假设它已经被解决了。

**具体做法**:每次安装后,用 sha256 逐个比对下面三个位置,不一致就不算通过。

```
仓库            release/extension/dist/index.js
  ↓ 必须逐字节相同
staging         release/extension/dist/index.js   (Tauri bundle.resources 来源)
  ↓ 必须逐字节相同
已安装          %APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\dist\index.js
```

sidecar 同理,四处必须一致:

```
sigma-listary-spike/target/release/spike.exe
release/extension/bin/focus-sync-sidecar.exe
release/extension-installer/focus-sync-sidecar.exe
%APPDATA%\...\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe
```

比对命令(直接可用):

```powershell
Get-FileHash <path> -Algorithm SHA256
```

---

## 2. 必读文档(按顺序)

| # | 文档 | 为什么读 |
|---|---|---|
| 1 | **本文** | 当前状态、约束、已知的坑 |
| 2 | [`handoff-2026-10-08.md`](handoff-2026-10-08.md) | v0.5.7 → v0.5.8 的完整变更史与**今日四个新根因** |
| 3 | [`extension-changelog.md`](extension-changelog.md) | v0.3.0 至今全部变更 + **已知未解决表** + **沙箱约束** |
| 4 | [`known-issues-2026-10-08.md`](known-issues-2026-10-08.md) | BUG-1/2/3 的根因与更正过程 |
| 5 | [`../README.md`](../README.md) | 架构、部署契约、诊断工具 |
| 6 | [`../release/extension-installer/installer.nsi`](../release/extension-installer/installer.nsi) | **打包实际做了什么**(本文 §4 摘要) |

> **不要**读 `docs/superpowers/plans/` 里 100+ 篇历史文档。它们是失败记录,不是参考。

---

## 3. 硬约束(违反即任务失败)

| # | 约束 |
|---|---|
| 1 | **回滚节点已建立:`v0.5.8-packaging-baseline-20261008`。任何时候不确定就回滚,不要在坏状态上继续叠加** |
| 2 | **安装后必须做 §1 的 sha 比对**,不能只看「装完了」 |
| 3 | **改插件后必须跑沙箱门禁** → `node scripts\scan-sandbox-dynamic.cjs release\extension\dist\index.js` 必须 `VALID (0 violations / 19 patterns)` |
| 4 | **注释同样会被沙箱扫描。** 一句 `foreground window.` 曾导致 extension 静默不加载 108 分钟 |
| 5 | **不要 commit git**,除非用户明确要求 |
| 6 | **一次只改一处**,每处独立验证、独立可回滚 |
| 7 | **每次改动后回读源码确认落地** —— `String.Replace` 锚点不匹配是**静默 no-op**,今天之前已有两次「编译过、测试全绿、功能完全没修好」的发布 |
| 8 | **不要在用户重启 Sigma FM 的窗口期部署** —— extension 只在启动时加载 |
| 9 | **不要用 `Get-Content` 默认编码读日志**(ANSI),必须 `[IO.File]::ReadAllText($p,[Text.Encoding]::UTF8)`。**本项目自己已经因此被坑过 4 次**,包括写诊断脚本时用了无 BOM 的 UTF-8 + 中文,PowerShell 5.1 按 ANSI 读,乱码冲掉引号导致语法错误 |
| 10 | `D:\Program Files\` 是受保护路径,**不能写入或删除** |

---

## 4. 打包现状(已核查,非猜测)

### 4.1 仓库里有**两套**安装包

| 产物 | 位置 | 内容 |
|---|---|---|
| **A. 插件独立安装包** | `release/extension-installer/installer.nsi` → `kizemo.focus-sync-0.x.x-setup.exe` | 只装插件 + sidecar + 计划任务 |
| **B. 整合 Sigma FM** | `release/installer/` 与 `release/Sigma-File-Manager-2.2.2-focus-21-v0.5.x-setup.exe` | Sigma FM 本体 + 插件 |

⚠️ **本任务要同时做这两者,但必须先搞清楚历史上「功能丧失」的是哪一个。** 这是接手后要问用户的第一个问题。

### 4.2 `installer.nsi` 的 8 个阶段

```
Stage 1  检测 Sigma FM 是否在运行 → 若是则杀掉
         ★ 注释原文警告: "Sigma FM keeps user-extensions.json in memory
           and will overwrite the file on next save. We must kill it before
           install so ..." (L122-123)
Stage 3  复制插件文件到 %APPDATA%\...\extensions\kizemo.focus-sync\
         File "..\extension\package.json"
         File "..\extension\dist\index.js"        ← 来源是仓库 dist
         File "..\extension\dist\index.js.map"
         locales\*.json
Stage 4  (v0.3.0 已移除:Sigma FM 不再 spawn sidecar)
Stage 5  register.ps1 写 user-extensions.json(含 isLocal: true)
Stage 6  写 HKCU\SOFTWARE\kizemo\focus-sync\SidecarPath
Stage 6.5 register-scheduled-task.ps1 建计划任务 KizemoFocusSync
Stage 8  若 Stage 1 杀过 Sigma FM,则重新打开
```

### 4.3 ⚠️ 已发现的可疑陈旧文件

```
release\extension\bin\focus-sync-sidecar-canonical.exe   2570752  2026-10-01
release\extension\dist\index.js.map                       4263  2026-09-27
release\extension\package.json                           2460  2026-10-06
```

`installer.nsi` 当前只引用 `focus-sync-sidecar.exe`,没引用 `-canonical.exe`,所以**大概率无害**。但它是一个躺在打包输入目录里的旧二进制 —— **建议在打包前先确认它该删还是该留**,别让历史文件混进安装包。

### 4.4 三个已知的历史失败模式(假设,非结论)

按可疑度排序。**这些是待验证的假设,不要当已确认的根因**:

1. **Sigma FM 内存态覆盖安装器写入。** `installer.nsi` L11-13 与 L122-123 自己都写了这个警告。如果 Stage 1 杀 Sigma FM 失败,Stage 5 写入的 `user-extensions.json` 会被 Sigma FM 下次保存时覆盖 → 扩展未注册 → **静默不加载**(与 Phase 4 的沙箱拒绝是同一种表现:无 UI 报错、无通知、sidecar 收不到请求)。
2. **打包时捕获的是旧的 `dist/index.js`。** 如果构建顺序是「先构建安装包,后改 dist」,装出来的就是旧代码。**这正是 §1 的 sha 比对能抓住的。**
3. **沙箱拒绝。** 装出来的 `index.js` 若含 `window.` / `fetch(` / `self` 等,Sigma FM 会**静默拒绝整个扩展**。症状与「扩展没装上」完全一样。诊断:插件 trace 是否有 `N01.activate.start`。

---

## 5. 诊断手段(已就位,直接用)

```powershell
# sidecar 日志 + 存活对话框枚举(强制 UTF-8,含存活对话框对比)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\read-sidecar-log.ps1

# 插件持久化 trace(N01-N20,跨重启保留)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\read-focus-trace.ps1

# 沙箱门禁
node scripts\scan-sandbox-dynamic.cjs release\extension\dist\index.js

# 构建并 stage sidecar(走门禁,不要手工拷贝)
.\sigma-file-manager\scripts\build-with-sidecar.ps1 -RepoRoot $PWD
```

**插件 trace 的关键节点**(判断插件是否加载):

| 节点 | 含义 |
|---|---|
| `N01.activate.start` | **`activate()` 被调用了。没有它 = 扩展根本没加载** |
| `N02.storage.probe` | `sigma.storage` 是否可用 |
| `N18.schedule` / `N19.push.enter` | v0.5.8 新增。推送链路入口 |
| `N03.health.resp` | 健康轮询 |

> ⚠️ 若安装后功能丧失,**第一件事是看 `N01.activate.start` 在不在**。
> 不在 = 扩展没加载(沙箱拒绝 / 未注册 / 文件不对),此时查日志没有意义。

---

## 6. 建议的推进顺序(尚未执行,待新会话评估)

```
0. 问用户:历史上「安装后功能丧失」用的是哪一套安装包?A 还是 B?
   → 这决定后面所有工作的范围
1. 回滚到 v0.5.8-packaging-baseline-20261008,确认功能正常(基线复现)
2. 记录基线 sha(三处 index.js + 四处 exe)
3. 用现有 A 安装包装一次,立即做 §1 的 sha 比对
   → 不一致 = 直接抓到根因,不必再猜
4. 一致但功能仍丧失 → 走 §4.4 的三个假设,先查 trace 的 N01
5. 修 A,验证
6. 再做 B(整合进 Sigma FM)
```

**第 3 步是本任务的核心动作。** 它把一个「不知道为什么坏了」的历史问题变成一次可观测的对比。

---

## 7. 给新会话的一句话 prompt

```
读 F:\soft\00selfmade\filemanager\docs\handoff-2026-10-08-packaging.md(必读),
然后按它的 §2 必读文档顺序读完 §2 的 1-6 号。当前代码状态已固化为回滚节点
v0.5.8-packaging-baseline-20261008(HEAD 43bd1f2,工作区干净,73/73 测试通过,
沙箱门禁 VALID),不要回退它。本任务是把插件打包成 exe 安装包并整合进 Sigma FM;
历史上「手动部署正常、改用安装包后功能丧失」,所以验收标准不是「装成功」而是
「安装后的文件与手动部署 sha 完全一致」,见 handoff §1。动代码前务必先读 §3 硬约束,
并遵守 §8 的经验教训。
```

---

## 8. 经验教训(今天用 18 小时换来的,浓缩版)

1. **先证明你的尺子没坏,再拿它量东西。** 今天我犯过四次同类错误:
   - 用 3 秒 CPU 采样断定「monitor 线程已死」(30ms 轮询用 `recv_timeout` 阻塞,3 秒测不出差异)
   - 用「现在桌面上没弹窗」反推「用户测试时也没弹窗」
   - 在盖章之后才测量,导致 `extension_alive` 恒为 true
   - 统计时把「不是 #32770 的行」也算进了「#32770 的计数」

2. **编译通过 ≠ 补丁生效。** `String.Replace` 锚点不匹配是静默 no-op。改完必须回读源码。

3. **拿被测对象自己的日志去诊断它,是循环论证。** 今天「窗口激活经过 `ForegroundStaging` 临时窗口」这个根因,**只有独立观察进程才看得见** —— sidecar 自己只记「dialog not foreground」这个结果。

4. **共享端点不能当信号源。** 我曾用 `GET /health`(所有诊断脚本都会调)当「插件是否存活」的信号,结果一次诊断读取就能把闸门顶开。**信号必须来自专属通道。**

5. **静默失效是这个项目最大的敌人。** extension 被沙箱拒绝 → 不加载、无报错。`JSON.parse` 失败 → 整块逻辑跳过、无报错。去重键不更新 → 无限循环、只有日志能看出。**给「本应发生的事」建立可见性,别只检查「正在发生的事」。**

6. **看到异常值就死盯异常值是一种系统性偏误。** 全量统计:`H1 succeeded 48 / failed 8`;`F3 基线全历史仅 5 次`。定性之前先看分布。

7. **文档写的和日志说的不一致时,信日志。** 今天 BUG-2 的根因在文档里被记成 H2,全量统计证明是 F3。保留原推断过程有意义(它记录了这个失败模式),但必须标注更正。
