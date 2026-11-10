# 打包任务结论 —— 「装完功能丧失」的根因已定位并修复

**时间**:2026-10-08 20:15
**基线**:`v0.5.8-packaging-baseline-20261008`(HEAD `43bd1f2`,未回退)
**验收标准**:handoff §1 ——「安装后的文件与手动部署 sha 完全一致」,不是「装成功」

---

## 0. 一句话结论

历史安装包**从来没有包含过 v0.5.x 的代码**。仓库里的 `kizemo.focus-sync-0.2.0-setup.exe`
构建于 2026-09-30,内嵌的 `index.js` 只有 **4,626 字节**(2026-09-29),而当前可用的是
**37,144 字节**。装上去的是一个 v0.3.0 之前的**旧架构插件**,它靠 Sigma FM spawn sidecar
——而这条路 **v0.3.0 起已经废弃**。所以它「装成功、注册成功、就是没有功能」。

**证据**(`7z l` 直接读安装包内部,不是看时间戳推测):

```
2026-09-29 21:30:46    4626  $APPDATA\...\kizemo.focus-sync\dist\index.js   ← 旧代码
(无)                         register-scheduled-task.ps1                      ← 计划任务阶段根本不存在
```

装进去的东西能注册、能显示、不报错 —— 符合 handoff §4.4「静默失效」的全部特征。

---

## 1. 已修复的 5 个缺陷

| # | 缺陷 | 症状 | 修法 | 位置 |
|---|---|---|---|---|
| **1** | 安装包从未重建 | 装着 4.6 KB 旧插件,功能全无 | 重新构建,载荷逐字节校验 | `build.ps1` 输出名 `0.2.0`→`0.5.8` |
| **2** | sidecar 装错位置 | `%LOCALAPPDATA%\Programs\…\bin\` 里躺一份**没人看**的副本;§1 的四处 sha **永远不可能一致** | 单一规范路径 `%APPDATA%\…\bin\focus-sync-sidecar\`,计划任务与注册表同步指向它 | `installer.nsi` 新增 `SIDECAR_DEST_PATH` |
| **3** | `$_` 被 NSIS 当变量吞掉 | Stage 1.5 的 `Where-Object { $_.Path … }` 变成 `{ .Path … }` → PowerShell 语法错误 → **杀旧 sidecar 从未成功过** | 转义为 `$$_.Path`(与本文件既有 `$$p` 一致) | `installer.nsi` Stage 1.5 |
| **4** | 注释末尾 `\` 吞掉下一行 | 卸载器的 `RMDir /r …\binaries\focus-sync-sidecar` **被注释吃掉,从不执行** | 拆行,注释不再以 `\` 结尾 | `installer.nsi` Uninstaller |
| **5** | `manual-install.ps1` 硬编码 round-19c 哈希 | 每次运行都报 `Write-Warning`,而 nobody 看 → **一个永远不可能通过的检查** | 改为「副本 sha == 源 sha」,不匹配直接 `exit 1` | `manual-install.ps1` |

附带修掉:`package.json` 版本号 `0.5.5`→`0.5.8`(与实际代码一致);
删除指向**已删除的 TS 构建链**的 `scripts`/`devDependencies`(README 明确警告
用它构建会覆盖 `dist` 并丢掉 v0.3.0→v0.5.8 全部修复)。

---

## 2. NSIS 静默失效:`Function` 包装的 nsExec 不执行

**这是本次最有价值的发现,也是最难查的一个。**

Stage 1 的检测与杀进程原本写成 NSIS `Function` 再用 `Call` 调用。
**它们从来没有执行过**,而且会打印一句自信的 `Sigma FM is not running (good)`。

判定依据不是猜测,是计时:

```
kill-sigma.ps1 一旦执行,最少耗时 = 10s(等待) + 2s(强制杀后等待) = 12s
实测安装总耗时                                   = 10s        ← 杀进程根本没跑
```

同一份文件里,Stage 5 用**内联** `nsExec` 调用 `register.ps1` 是正常执行的
(`C:\Temp\user-extensions.json.bak-register-*` 每次都有新文件)。
⇒ **内联可用,`Function` + `Call` 不可用。** 检测、杀进程、重开 Sigma FM 三处已全部内联。

### 加了断言,不再「相信」结果

杀进程之后**重新检测进程是否真的没了**;还在就直接中止,并写标记文件:

```
%TEMP%\focus-sync-install-FAILED.txt
```

理由:Sigma FM 把 `user-extensions.json` 缓存在内存里,下次保存会覆盖磁盘文件。
带着它继续装 = 制造出「装好了、注册没了、扩展静默不加载」—— 正是本项目最典型的失败形状。

> ⚠️ 中止路径**不使用 `MessageBox`**。实测 NSIS 在 `/S` 下仍会弹出对话框并永久阻塞
> (无人点击时安装器挂死)。曾因此卡死一次,只能强杀安装器进程。

---

## 3. 验收结果(实测,非推断)

```
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-deploy-sha.ps1
```

| 检查项 | 结果 |
|---|---|
| 安装包载荷 vs 仓库(7z 解包比对) | `index.js` `ECED2EBE…` / sidecar `AA4CB767…` / `package.json` — 全部一致 |
| §1 三处 `index.js` + 四处 `sidecar.exe` | **PASS** |
| 计划任务指向 | `%APPDATA%\…\bin\focus-sync-sidecar\focus-sync-sidecar.exe` ✅ |
| `HKCU\…\SidecarPath` | 同上 ✅(安装前该键**不存在**) |
| `$INSTDIR\bin` | 只剩两个 PS 脚本,**无游离 sidecar** ✅ |
| `N01.activate.start` | **count = 1** → 扩展确实加载了 ✅ |
| sidecar `/health` | `version 0.5.8` / `extension_alive true` / `first_push_received true` / `unsupported_dialog_count 0` / `uia_healthy true` ✅ |

### 一个必须说清楚的方法论要点

**sha 全等 ≠ 安装器真的写了东西。**

第一次装机后 sha 门禁就 PASS 了 —— 但那 6 个文件**装机前就已经相等**。
若只看 sha,会得出「装机成功」的错误结论。

真正证明安装器写入了的,是**我故意改过的那个文件**:
装机前 `package.json` = `31C2A061… v0.5.5`,装机后 = `9F34E0D2… v0.5.8`,与仓库一致。
**只有故意制造的差异才能区分「写了」和「什么都没做」。**

---

## 4. 遗留 / 已知限制

| 项 | 状态 | 说明 |
|---|---|---|
| **自动杀 Sigma FM 在本机不生效** | ⚠️ 未解决 | `nsExec` 调用 `kill-sigma.ps1` 返回 `-196608` 而非退出码,进程没被杀。**同一个 kill 命令在普通 PowerShell 里手动执行是成功的**(已实测),故非权限/提权问题,而是 NSIS 调用层。**现在安全**:杀不掉就中止并留标记,不会再静默装坏。**用户手动关掉 Sigma FM 后安装则完全正常**(exit 0 / 9s / 全绿)。 |
| 注册表里 manifest 版本仍是 `0.5.5` | 无害 | `register.ps1` 的 round-2 逻辑「条目已存在就不动」。磁盘上已是 `0.5.8`。实测**不影响加载**(`N01` 正常出现)。 |
| 扩展激活耗时 | 观察中 | 本次 **约 5 分钟**(20:05:14 启动 → 20:09:5x 激活),长于文档记的 1–3 分钟。 |
| `focus-sync-sidecar-canonical.exe` | ✅ 已排除 | 2,570,752 字节的 2026-10-01 旧二进制。已确认 `installer.nsi` 与 `tauri.conf.json` **都没有引用它**,且安装包解包后是 18 个文件、不含它 —— **不会混进安装包**,当前无害。 |
| 整合包 B | ✅ 已验证 | 见 §6。按授权**只构建不安装**。 |
| **大包 hooks.nsh 的两条静默失效** | ✅ 已修 | 见 §7。 |

### §7 大包里同样的病,而且构建工具把它藏起来了(2026-10-08 追加)

回答「Sigma FM 安装时能不能自己杀掉运行中的程序」时顺带查出来的。
**大包不但能杀,而且比小包更彻底** —— `NSIS_HOOK_PREINSTALL` 里是:

```
taskkill /F /IM sigma-file-manager.exe /T   + 各种大小写变体
taskkill /F /IM msedgewebview2.exe /T       + crashpad / werfault / msiexec
taskkill /F /IM spike.exe /T                (插件的 sidecar)
schtasks /Change /TN "\KizemoFocusSync" /DISABLE
Sleep 1500
```

而且它跑在 Tauri 自带的 `CheckIfAppIsRunning` **之前**(hooks.nsh 注释写明:
Tauri 只在卸载时杀进程,安装不杀,这个钩子是为安装补的)。**静默杀,不提示。**

**但「保证安装成功」当时做不到**,因为里面有两处同样的静默失效:

| # | 位置 | 被吞掉的东西 | 后果 |
|---|---|---|---|
| **1** | `hooks.nsh:243`(POSTINSTALL Stage 0.5) | `$_` | `Where-Object { .Path … }` 语法错误 → 旧 sidecar 杀不掉 → 占着 37421 端口 → 新的计划任务启动即失败 |
| **2** | `hooks.nsh:194` / `396`(安装 + 卸载的兜底) | `$env:USERPROFILE\AppData\Roaming` | NSIS 把**整串**当成一个变量名(`:` 和 `\` 在变量名里合法)→ 兜底解析出 `USER_APPDATA=\AppData\Roaming` → **插件文件写到错误位置,注册不到 —— 装上了,没功能** |

**为什么之前没人发现**:`npm run tauri build` 的日志里**一条 NSIS 警告都没有**。
Tauri 的 bundler 把 makensis 的输出过滤掉了。
手动跑 `makensis /V3 installer.nsi` 才露出 5 条 `warning 6000`。

### 新增门禁

`sigma-file-manager\scripts\verify-nsis-warnings.ps1` —— 绕开 Tauri 直接调 makensis,
**任何警告即失败**。还带一条防呆:如果生成的 `installer.nsi` 比 `hooks.nsh` 旧,
直接判失败(否则会拿旧 include 去检查,**平凡地绿**)。

实测:修复前 **5 条警告** → 修复后 **0 条**。

### §6 整合包 B(Tauri)——只构建不安装,已通过

`tauri.conf.json` 的 `bundle.resources` 会把 `release/extension/` 下的
`package.json` / `dist` / `locales` 与 sidecar 一起打包,配置本身是对的。

**构建工具链(踩坑记录)**:README 里给的 `1.85.0` 是给 **sidecar** 用的,
拿它编 Sigma FM 会失败:

| 工具链 | 结果 |
|---|---|
| `1.85.0` | ❌ `icu_properties_data@2.3.0 requires rustc 1.88` |
| `stable`(1.97.1) | ❌ 244 个错误全在第三方 crate(`toml_parser` E0425、`parking_lot_core`、`phf_shared`) |
| **`1.90.0`** | ✅ **成功** |

构建命令:
```powershell
cd sigma-file-manager
$env:PATH = "C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;$env:PATH"
npm run tauri build
```

**验收(解包比对 sha,不安装)**:

```
[OK  ] index.js      ECED2EBE4D1598BA
[OK  ] sidecar.exe   AA4CB76716268FEF
[OK  ] package.json  9F34E0D2FAAC757C
B_VERDICT = PASS
```

产物:`release\Sigma-File-Manager-2.2.2-focus-21-v0.5.8-setup.exe`(17,100,112 字节)
包内 `extensions\kizemo.focus-sync\` 结构完整,`tools\focus-sync-sidecar.exe` 就位。

> ⚠️ **B 未经实机安装验证。** 本轮只证明了「装进去的字节是对的」,
> 没有证明「装到 `D:\Program Files\` 之后注册与计划任务仍然成立」——
> 那需要写受保护路径,已按你的选择跳过。

---

## 8. 用户报「装完地址栏不同步」—— 已定位,**不是打包问题**

用户反馈:装完 → 立刻启动 Sigma FM → 下载/另存为弹窗地址栏不同步;等一会儿就正常。
**用户于 22:36 现场确认了这个现象。**

### 先排除:代码完全一致

| 位置 | sha | 结论 |
|---|---|---|
| 仓库 `release/extension/dist/index.js` | `ECED2EBE…` | — |
| 已安装 `%APPDATA%\…\dist\index.js` | `ECED2EBE…` | ✅ 一致 |
| `D:\Program Files\Sigma FM\extensions\…` | `A78E96D3…`(26,851 字节,v0.5.5) | ⚠️ 旧副本,当前未生效 |

`localSourcePath` 指向 `%APPDATA%`,所以跑的就是新代码。
**⚠️ 那份 D 盘旧副本是雷**:哪天 Sigma FM 改从自带资源读,就会加载 v0.5.5。

### 一个差点上当的陷阱

trace 里 `N01.activate.start` 报 `ver: v0.5.5`,极易据此断定「跑的是旧代码」。
**它是 `index.js:656` 硬编码的、从未更新的字符串。**
同一文件里 `N16` / `N18` / `N19` 这些 v0.5.8 才有的节点都在 —— 跑的就是新代码。
**「仪器上的读数过期」不等于「被测对象是旧的」。**

### 真正的原因:两个因素叠加

```
21:50:46  Sigma FM 启动
21:52:25  弹窗打开 → 21:52:26  WriteFailed reason=extension_not_alive
21:53:03  扩展激活完成(距启动 2 分 17 秒)
21:53:48  Sigma FM 发出首个目录变化 → 恢复正常
```

1. **sidecar 存活闸门**:60 秒内没收到扩展心跳就拒绝写入(防止 Sigma FM 关着时灌旧路径)。
   扩展还没激活 → 心跳没来 → 拒绝。
2. **扩展激活时不知道当前目录**:`getCurrentPath()` 返回 `null`、`lastKnownPath` 也是空
   (trace `N10.ctx.empty`),必须等 Sigma FM 发出第一次目录变化通知。

⇒ **两者都只在「刚装完 / 刚重启」这个窗口内出现**,平时 Sigma FM 常开不会遇到。

## 10. 2026-10-09:「切回弹窗后不再同步」—— 真根因(不是打包问题)

用户升级大包 B 后实测:**打开弹窗会同步(旧)地址;在 Sigma FM 换路径、再切回弹窗,地址不再变化。**

### 先排除:安装没问题

`verify-deploy-sha.ps1` **PASS** —— `index.js` / `sidecar.exe` 全部与仓库逐字节一致,
计划任务指向规范路径。**不是安装缺陷。**

### 日志里的决定性证据

```
23:26:36.8  [monitor] dialog regained foreground (pending='E:\FFOutput') → resync
23:26:37.3  write_path target='E:\FFOutput'
23:26:37.9  H1: dialog 不是前台 (monitor-published fg=0) → NOT stealing focus, aborting
23:26:38.0  H2: target_path 写入文件名框 + 还原并读回验证
23:26:38.0  Injection succeeded via UI Automation.      ← ★ 谎报
```

**H1 被正确拦截(这是 BUG-1 安全闸门在正常工作),然后回落到 H2 —— 而 H2 写的是
「文件名框」再还原,那从来不是导航(Claude Rule 58)。** 代码却记成「成功」,
所以 `/health` 一切正常、JSONL 记 `Write`,而用户看到的是地址栏纹丝不动。

### 为什么 G2 的「在闸门上等」没生效

全历史统计:

```
await_dialog_foreground 调用次数 = 0      ← 那个函数一次都没执行过
```

**因为它挡在前面。** `uia_inject.rs` 里 5 处「等」的闸门都在 SendInput 批次前,
但函数**开头还有一个裸检查**,只采样一次前台就 `return false`:

```rust
let current_fg = state.foreground_dialog();
if current_fg != dialog_hwnd { warn!(...); return false; }   // ← 先跑了,后面的永远到不了
```

Windows 激活窗口会经过一个 `ForegroundStaging` 临时窗口(~150ms)。
由 monitor 的「 regained foreground → resync」触发的写入,正好落在这个过渡窗口里,
`fg` 读到 0 → 开头裸检查立刻放弃 → 5 处等待全部落空 → 落到 H2 → 谎报成功。

### 修法(安全性不变,只是改成等)

```rust
if !await_dialog_foreground(state, dialog_hwnd, ACTIVATION_WAIT_MS) {
    warn!(...); return false;   // 仍然绝不抢焦点
}
```

**不是放宽闸门,是让闸门等用户真的把窗口切回来**,和另外 5 处保持一致。
`await_dialog_foreground` 调用点 5 → **6**,裸检查归零。

### 验证

- `cargo build --release` OK;`cargo test --release` **73 passed / 0 failed**(与基线一致)
- 走 `build-with-sidecar.ps1`(含沙箱门禁)部署,规范路径 sha 与构建产物一致
- 重启后 `/health`:`extension_alive true`、`first_push_received true`、
  `current_path` 自动重新推送 ⇒ 插件自动接管,无需重启 Sigma FM

> ⚠️ **H2 的语义问题依然存在**:它仍然会「写文件名框再还原」并被记为成功。
> 本次只修了「等不到闸门」这个直接原因。若 H1 因为用户真的没把弹窗切到前台
> 而被拒绝,H2 仍会产生一次假成功。彻底解决需要让 H2 不再计入成功路径 ——
> 属于独立改动,本次未做。

## 9. 22:36 追加:大包 B 实装后,§1 门禁失效(门禁本身的缺陷)

用户实装了**大包 B**(22:07),随后 `verify-deploy-sha.ps1` 报 **FAIL**:
`%APPDATA%\…\bin\focus-sync-sidecar\focus-sync-sidecar.exe` **MISSING**。

**不是 B 装错了**,实测全部正确:

| 项目 | 结果 |
|---|---|
| `D:\Program Files\ΣFM\tools\focus-sync-sidecar.exe` | `AA4CB767…` ✅ 与仓库一致 |
| `%APPDATA%\…\dist\index.js` | `ECED2EBE…` ✅ 一致 |
| `%APPDATA%\…\package.json` | `9F34E0D2…` ✅ 一致 |
| `/health` | `version 0.5.8`、`extension_alive true`、`uia_healthy true` ✅ |

### 真正的问题:**两个安装器把 sidecar 放在不同地方**

| 安装器 | sidecar 落点 |
|---|---|
| **A(插件独立 NSIS)** | `%APPDATA%\…\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\` |
| **B(整合进 Sigma FM)** | `$INSTDIR\tools\` = `D:\Program Files\Sigma FM\tools\` |

handoff §1 写死的四处 sha 比对**只适用于 A**。B 装完之后那一条路径根本不存在 ⇒
**门禁必然 FAIL**。两者结构上不兼容。

### 门禁的修法

`scripts\verify-deploy-sha.ps1` 不再猜路径,而是**去问计划任务**
`KizemoFocusSync` 实际要启动哪个 exe —— 那才是权威答案,且与机器无关:

```
  sidecar actually launched by task KizemoFocusSync:
    D:\Program Files\Sigma FM\tools\focus-sync-sidecar.exe
```

同时补了两处防呆(都来自「PASS 可能是平凡的」那条规则):
1. **仓库副本不存在 ⇒ 直接 FAIL**。否则基线会退化成「已安装文件和自己比」,
   在一台什么都没装的机器上也会绿。
2. **计划任务不存在 ⇒ 直接 FAIL**。

判别性测试(证明门禁不是摆设):
仓库路径写错 → `exit 1`(4 处 FAIL);路径正确 → `exit 0`。

### 仍未解决:两个安装器的 sidecar 位置该不该统一?

**已解决(22:41)。统一到唯一规范路径:**

```
%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\
    bin\focus-sync-sidecar\focus-sync-sidecar.exe
```

选它的三条理由(缺一不可):

| 理由 | 说明 |
|---|---|
| 已是既有规范 | `manual-install.ps1` 的 `$binDir`、`register.ps1` 的 canonical 分支、handoff §1 写的就是它 |
| **不需要管理员权限** | Program Files 需要提权,而这个路径是 per-user;per-user 安装器因此也能用同一套逻辑 |
| 卸载可自洽 | 它在插件目录内,卸载插件时一并删除,不会在 Program Files 留下孤儿 |

改动(`sigma-file-manager\src-tauri\installer\hooks.nsh` POSTINSTALL):

- Stage 7 从「只验证 `$INSTDIR\tools\focus-sync-sidecar.exe` 存在」改为
  **复制到规范路径并验证复制结果**(Tauri 仍会把资源解压到 `$INSTDIR`,只是那不再是运行用的副本)
- Stage 8 `SidecarPath` 注册表 → 规范路径
- Stage 9 计划任务 `-SidecarPath` → 规范路径
- 注册成功后 `Delete "$INSTDIR\tools\focus-sync-sidecar.exe"` 删掉重复副本,
  避免以后再有人从旧位置拉起
- makensis 重新编译:**0 warning**

### 线上已同步(不等装机)

```
1. 停掉从 D 盘运行的 sidecar (PID 55988)
2. 从仓库产物复制到规范路径 → AA4CB767… (与仓库一致)
3. register-scheduled-task.ps1 检测到路径变化,自动注销旧任务并重建
   INFO: existing task points to 'D:\Program Files\…', but new path is '%APPDATA%\…'
4. Start-ScheduledTask → State=Running,新进程从规范路径启动
5. /health: version 0.5.8, extension_alive true, uia_healthy true
6. verify-deploy-sha.ps1 → PASS
```

> `D:\Program Files\Sigma FM\tools\focus-sync-sidecar.exe` **本次未删除**
> —— 硬约束 10 规定该路径不可手动删改。下次装新版大包时,
> Stage 7 的 `Delete` 会自动清理它。

**没有改代码** —— 证据不支持「有 bug」:同一份安装产物在 `21:50:40` 成功注入过
(`H1 succeeded: address bar set + Enter sent`),失败那次的 `reason` 明确且合理。
把闸门去掉会重新引入 G3「Sigma 关着时灌旧地址」。

改为**把预期写进文档**:README「安装」章节顶部已加入预热说明、实测时间线、
两个原因,以及一行判断命令(`extension_alive` / `last_health_poll_age_ms`),
避免下次装机时把「冷启动」误判成「装坏了」。

> 仍可改进(未做):拒绝写入时**界面上毫无提示**,只有日志。
> 这是本项目典型的静默失效形状。候选方案是扩展在激活后若 `N10.ctx.empty`,
> 弹一次通知告知「正在等待当前目录」。代价是要改 `index.js`(会被沙箱扫描)
> 并重新构建部署。