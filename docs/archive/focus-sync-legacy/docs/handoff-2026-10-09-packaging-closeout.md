# HANDOFF —— 打包任务收尾(2026-10-09)

**交接时间**:2026-10-09 10:20
**上一份**:[`handoff-2026-10-08-packaging.md`](handoff-2026-10-08-packaging.md)(打包任务的起点)
**本轮产出**:两个安装包均可正常安装并通过验收;插件地址同步实测通过
**状态**:✅ 打包任务完成 · ⚠️ 仍有 7 项未收尾(见 §5)

---

## 0. 三十秒速览

| 项 | 值 |
|---|---|
| 主仓 | `F:\soft\00selfmade\filemanager` + `github.com/kizemo/focus-sync` |
| 主仓 HEAD | **`35cfe8b`**(main),标签 **`v0.5.9-packaging-20261009`** ← **新回滚点** |
| 主仓远程 | **领先 `origin/main` 4 个提交,未推送** |
| Sigma FM fork | `…\sigma-file-manager`,HEAD **`4365281b`**(main),**未推送** |
| 测试 | `cargo test --release` **73 passed / 0 failed** |
| 沙箱门禁 | `VALID (0 violations / 19 patterns)` |
| NSIS 门禁 | `PASS -- 0 warnings` |
| 部署 sha 门禁 | `PASS` |
| 已装 sidecar sha | `30166457D37FF34A…` |
| 已装 `index.js` sha | `ECED2EBE4D1598BA…` |
| 运行中 | sidecar 走计划任务,`version 0.5.8`,`extension_alive true` |
| 用户实测 | **地址同步成功**;两个安装包都装过并验收通过 |

---

## 1. ⚠️ 本任务最重要的一条约束(仍然有效)

> **验收标准不是「装成功」,而是「安装后的文件与手动部署逐字节一致」。**

这条从未变过,而且今天被证明是**唯一能抓住真实故障的标准**:安装包「装成功」了整整好几天,
内嵌的却是一年前 4,626 字节的旧插件 —— sha 比对一眼就能看出来,而「装成功了」看不出来。

**执行方式**:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-deploy-sha.ps1
```

退出码 0 才算通过。这条门禁已改成**跟随计划任务实际启动的 sidecar**,
因此对「插件独立安装包」和「整合包」两种装法都成立。

> ⚠️ 该门禁内置两道防呆,读源码前先知道:
> ① 仓库副本缺失 ⇒ 硬失败(否则基线退化成「已安装文件和自己比」,在空机器上也会绿)
> ② 计划任务缺失 ⇒ 硬失败

---

## 2. 必读文档(按顺序)

| # | 文档 | 为什么读 |
|---|---|---|
| 1 | **本文** | 当前状态、约束、未完成的工作 |
| 2 | [`packaging-findings-2026-10-08.md`](packaging-findings-2026-10-08.md) | **本轮全部根因与证据链**。今天定位的每一个 bug 都在里面,含反直觉的那些 |
| 3 | [`handoff-2026-10-08-packaging.md`](handoff-2026-10-08-packaging.md) | 打包任务的起点、验收标准 §1 的由来 |
| 4 | [`../README.md`](../README.md) | 架构、部署契约、**装机步骤与冷启动预热说明** |
| 5 | [`extension-changelog.md`](extension-changelog.md) | v0.3.0 至今变更史 + **沙箱约束** |
| 6 | [`known-issues-2026-10-08.md`](known-issues-2026-10-08.md) | BUG-1/2/3 的根因与「文档与日志不一致」的更正过程 |
| 7 | [`../release/extension-installer/installer.nsi`](../release/extension-installer/installer.nsi) | 独立安装包实际做了什么 |
| 8 | [`../sigma-file-manager/src-tauri/installer/hooks.nsh`](../sigma-file-manager/src-tauri/installer/hooks.nsh) | 整合包实际做了什么(**本轮刚入库**) |

> **不要**读 `docs/superpowers/plans/` 里 100+ 篇历史文档。它们是失败记录,不是参考。

---

## 3. 硬约束(违反即任务失败)

| # | 约束 |
|---|---|
| 1 | **回滚点是 `v0.5.9-packaging-20261009`(`35cfe8b`)。任何时候不确定就回滚到它,不要在坏状态上继续叠加** |
| 2 | **安装后必须跑 §1 的 sha 门禁**,不能只看「装完了」 |
| 3 | **改插件后必须跑沙箱门禁** → `node scripts\scan-sandbox-dynamic.cjs release\extension\dist\index.js` 必须 `VALID (0 violations / 19 patterns)` |
| 4 | **注释同样会被沙箱扫描。** 一句 `foreground window.` 曾导致 extension 静默不加载 108 分钟 |
| 5 | **不要 commit / push git,除非用户明确要求** |
| 6 | **一次只改一处**,每处独立验证、独立可回滚 |
| 7 | **每次改动后回读源码确认落地** —— `String.Replace` / 脚本化替换锚点不匹配是**静默 no-op**。本项目已有三次「编译过、测试全绿、功能完全没修好」的发布 |
| 8 | **不要在用户重启 Sigma FM 的窗口期部署** —— extension 只在启动时加载(激活需 1–3 分钟) |
| 9 | **不要用 `Get-Content` 默认编码读日志**(ANSI)。必须 `[IO.File]::ReadAllText($p,[Text.Encoding]::UTF8)`,且 sidecar 日志文件被占用时要用 `FileShare::ReadWrite` 打开 |
| 10 | **写 .ps1 / .nsi 诊断脚本一律纯 ASCII。** PS 5.1 按 ANSI 读无 BOM 文件,中文注释会冲掉引号导致语法错误(本项目已被坑 4 次) |
| 11 | `D:\Program Files\` 是受保护路径,**不能手动写入或删除**(安装器自身写它不算) |
| 12 | **编 Sigma FM 必须用 Rust 1.90**;README 里那个 1.85 是给 sidecar 用的,编 Tauri 会失败 |

---

## 4. 本轮修了什么(摘要,细节见 §2-2)

**5 个历史根因**,每一个都先用可证伪的证据定位,不是推理:

1. **安装包自 2026-09-30 起从未重建** —— 内嵌 4,626 字节的旧插件(v0.3.0 之前的 spawn 架构,早已废弃)。装成功、注册成功、零功能。
2. **sidecar 路径分裂** —— 独立包装 A 处、整合包装 B 处,互相覆盖计划任务、留孤儿副本,并使验收门禁**永远不可能通过**。现统一到 `%APPDATA%\…\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\`。
3. **NSIS 三处静默失效** —— `$_` 被当变量吞掉;注释末尾 `\` 吞掉下一行的 `RMDir`;**`$PLUGINSDIR` 展开为空字符串**(导致自动关闭 Sigma FM 从未生效)。
4. **sidecar 入口有个「读一次前台就放弃」的裸检查**,挡在 5 处 `await_dialog_foreground` 之前 ⇒ 那 5 处等待**全历史 0 次执行** ⇒ H1 退出后落到 H2(写文件名框,不是导航)并记成成功。
5. **manifest 版本谎报**(0.5.5 vs 实际 0.5.8)+ `manual-install.ps1` 里一个**永远不可能通过**的硬编码哈希校验。

**新增两道门禁**:
`scripts\verify-deploy-sha.ps1`(部署 sha)、`sigma-file-manager\scripts\verify-nsis-warnings.ps1`(NSIS 警告 —— 因为 **Tauri 会吞掉 makensis 的警告,构建日志全绿而产物是坏的**)。

---

## 5. ⚠️ 未完成的工作(接手从这里开始)

| # | 事项 | 严重度 | 说明 |
|---|---|---|---|
| **1** | **fork 的打包配置完全没进版本控制** | **高** | `sigma-file-manager` 的 `src-tauri\tauri.conf.json`(**`bundle.resources` + `installerHooks` + `installMode: perMachine` 全在这里**)、`extension_http.rs`、`Cargo.toml`、`package.json` 均**未提交**。今天已把 `hooks.nsh` 入库,但**引用它的配置还没入库** —— 状态不一致,重新 clone 会得到一个「有钩子但没接线」的仓库 |
| 2 | **主仓 4 个提交未推送** | 中 | 含 `v0.5.9` 标签。用户一直不主动 push,需确认是否要推 |
| 3 | **H2 语义:假成功** | 中 | H2 写文件名框再还原并被记为 `Injection succeeded`,而那从来不是导航(Claude Rule 58)。今天修了直接原因(等不到闸门),修复后 0 次触发。**样本仅 2 次,不敢断言它不会再发生**。彻底修需让它不再计入成功路径 —— 独立立项 |
| 4 | **卸载路径从未端到端验证** | 中 | 只验证过安装。卸载器的 `RMDir` 刚修过(原先被注释吞掉),未实测 |
| 5 | **扩展激活耗时 1–3 分钟,根因未查** | 中 | 实测 2 分 17 秒。它造成「装完立刻测必然失败」的体验。缩短需先查明为何慢 |
| 6 | `tools\user_appdata.txt` 残留 | 低 | 46 字节临时文件留在 `D:\Program Files\ΣFM\tools\`。在受保护路径里未手动删(硬约束 11)。下次改 hooks 时加一行 `Delete` |
| 7 | `focus-sync-sidecar-canonical.exe` | 低 | `release\extension\bin\` 里 2.45 MB / 2026-10-01 的旧二进制。已确认 `installer.nsi` 与 `tauri.conf.json` **都不引用它**,当前无害。上一份 handoff §4.3 要求「确认该删还是该留」—— **至今没定** |

---

## 6. 回退节点

| 标签 | 含义 |
|---|---|
| **`v0.5.9-packaging-20261009`** | ★ **当前状态。安装包可用,同步实测通过(`35cfe8b`)** |
| `v0.5.8-user-verified-20261008` | 功能基线用户确认过的状态(`64962ad`) |
| `v0.5.8-packaging-baseline-20261008` | ⚠️ **该节点的安装包不可用**(内嵌 9-29 的旧插件,装着没功能)。**不要**回退到这里再用它的安装包 |

```powershell
git reset --hard v0.5.9-packaging-20261009
```

⚠️ **只改代码后需要重新构建部署**,回退 git 不会自动更新 `%APPDATA%` 里的二进制。

---

## 7. 运维命令

```powershell
# 部署 sha 验收(退出码 0 才算通过)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-deploy-sha.ps1

# 沙箱门禁
node scripts\scan-sandbox-dynamic.cjs release\extension\dist\index.js

# NSIS 警告门禁(绕开 Tauri,直接调 makensis)
powershell -NoProfile -ExecutionPolicy Bypass -File sigma-file-manager\scripts\verify-nsis-warnings.ps1

# 构建 sidecar(必须 Rust 1.85)
$env:PATH = "C:\Users\Duanyi\.rustup\toolchains\1.85.0-x86_64-pc-windows-msvc\bin;$env:PATH"
cd sigma-listary-spike; cargo build --release; cargo test --release

# 构建 + 部署 sidecar 到两个打包输入目录(含沙箱门禁)
.\sigma-file-manager\scripts\build-with-sidecar.ps1 -RepoRoot $PWD

# 构建整合包(必须 Rust 1.90,不是 1.85)
cd sigma-file-manager
$env:PATH = "C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;$env:PATH"
npm run tauri build

# 读日志(sidecar 日志被进程占用,必须共享读)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\read-sidecar-log.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\read-focus-trace.ps1

# 唯一 sidecar 真身:计划任务指向的那个
(Get-ScheduledTask -TaskName 'KizemoFocusSync').Actions[0].Execute
```

**日志位置**:`%LOCALAPPDATA%\kizemo\focus-sync\logs\`
(`spike.log` JSONL 事件 / `spike.log.YYYY-MM-DD` tracing 原因,**按 UTC 划分,本地下午可能落在前一天的文件里**)

**冷启动**:Sigma FM 启动后扩展要 1–3 分钟才激活。这期间 sidecar 会**故意拒绝**写入
(防止 Sigma 关着时把旧路径灌进弹窗)。**这不是故障,是设计**。自查:

```powershell
(Invoke-RestMethod http://127.0.0.1:37421/health) | Select-Object extension_alive,last_health_poll_age_ms
```

`extension_alive=false` = 还在预热,等;`true` 还不同步 = 真问题。

---

## 8. 给新会话的一句话 prompt

```
读 F:\soft\00selfmade\filemanager\docs\handoff-2026-10-09-packaging-closeout.md(必读),
按它的 §2 顺序读完 1-8 号。当前状态已固化为回滚节点 v0.5.9-packaging-20261009
(main 35cfe8b),不要回退到 v0.5.8-packaging-baseline(那个节点的安装包不可用)。
先看 §5「未完成的工作」,从第 1 项开始 —— fork 的打包配置还没有进版本控制,
这是当前最高风险。动代码前务必先读 §3 硬约束,并遵守 §8 经验教训。
```

---

## 9. 经验教训(本轮新增,浓缩版)

1. **「环境问题」不是根因,「NSIS 调用层有问题」也不是。** 自动关闭 Sigma FM 长期失效,
   我三次汇报都停在「调用层的问题」——那只是现象。用最小隔离程序 + 对照探针
   才挖到真因:`$PLUGINSDIR` 展开成空字符串。
   **任何「原因不明」,先问:我能不能造一个最小复现?**

2. **对照探针是必须的。** 隔离测试里我塞了 `cmd /c exit 7` 和 `powershell -Command "exit 3"`
   两个对照,它们都返回正确值,才让我敢断言「nsExec 没问题、问题在路径」。
   没有对照时,「失败」无法区分「机制坏了」和「被测对象坏了」。

3. **同一个可疑机制出现第二次,去全盘扫描。** `$PLUGINSDIR` 在独立安装器里坏了之后,
   整合包的 `hooks.nsh` 里还有 8 处同类用法。修一处不够。

4. **我自己的测量工具也骗过我两次。** `FileOpen ... a` 反复写同一个文件会互相覆盖,
   对照探针的行被吞掉,差点得出完全相反的结论;
   NSIS `CopyFiles` 保留源文件时间戳,让我差点误判「安装器没干活」。
   **一次观察写一个文件;下结论前先问「这个读数能区分什么」**

5. **Tauri 会吞掉 makensis 的警告。** 构建日志全绿,产物是坏的 —— 这在本轮出现两次。
   任何「用别人的构建工具打包」的场景,都要有一条绕开它的独立门禁。

6. **仪器上的读数过期 ≠ 被测对象是旧的。** 插件 trace 里的 `ver: v0.5.5` 是
   `index.js:656` 一个从未更新的硬编码字符串;同一文件里 `N18`/`N19` 这些
   v0.5.8 才有的节点全都在。差点据此错判「跑的是旧代码」。

7. **PASS 也可能是平凡的。** 装机后 sha 门禁第一次就绿,但那 6 个文件**装机前就已经相等**。
   真正证明安装器写了东西的,是我故意改过的那个 `package.json`。
   想要「证明执行者真的做了事」,只能靠**故意制造的差异**。