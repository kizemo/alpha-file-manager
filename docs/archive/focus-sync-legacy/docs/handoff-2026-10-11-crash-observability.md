# 交接：启动崩溃可观测化 —— 从「猜」改成「有产物」（2026-10-11）

对应版本：**2.2.2**（尚未打包，代码已就位）
涉及文件：`src-tauri/src/startup_probe.rs`（新建）、`src-tauri/src/main.rs`、
`src-tauri/src/lib.rs`、`public/startup-recovery.js`

---

## 一、这一轮做了什么，以及为什么是这个

上一轮结论是「启动崩溃 `0xc0000005` 没有复现路径、没有 dump，下一步该做的是
把崩溃现场变成可观测产物，而不是继续猜」。这一轮就只做这一件事：**建立两条
不依赖「下次一定复现」的观测通道**。

在此之前先做了一件事——**把已有证据查到底**，因为它改变了方案。详见第二节。

---

## 二、新查到的事实（改变了判断）

### 2.1 WER 归档里有一份完整报告，此前没人读过

```
C:\ProgramData\Microsoft\Windows\WER\ReportArchive\
  AppCrash_alpha-file-manag_51bef7e73..._9cdb3e95-051a-4e66-87ea-8eb0901fcfde\Report.wer
```

它和事件日志同源（`IntegratorReportIdentifier` = `7eb38380-...`），但**多了两样
事件日志没有的东西**：

| 新增信息 | 值 | 意义 |
|---|---|---|
| `LoadedModule[39]` | `...\EdgeWebView\Application\154.0.4258.62\EBWebView\x64\EmbeddedBrowserWebView.dll` | **WebView2 当时已加载** |
| `TargetAppVer` | `2026//10//09:13:29:08` | 崩溃二进制构建于 **10-09 13:29** |
| `TargetAppId` | 含模块 GUID 与 PDB 签名哈希 | 可用于校验二进制身份 |

**已加载模块总数 126 个**，其中非 Windows 目录的有：

| 模块 | 类别 |
|---|---|
| `EmbeddedBrowserWebView.dll` | WebView2 |
| `tsf-oime-core.dll` / `doubaoime-rpc-new.dll` / `dbghelp.dll`（豆包输入法） | **输入法 COM 注入** |
| `YunShellExtV164.dll`（百度网盘） | 资源管理器外壳扩展 |
| `CoreSync_x64.dll`（Adobe） | 外壳扩展 |

⇒ 这是一条**新方向**，但本轮**没有据此改任何代码**：输入法 TSF 与外壳扩展都是
正常的进程内注入，能解释「为什么会有第三方 DLL 出现在列表里」，但在**没有调用栈
之前**，它和「Rust 侧访问违例」之间的因果关系是不可知的。列在这里是为了下次复现
时不必重新发现一遍。

### 2.2 崩溃偏移量**无法**用于反查符号 —— 已验证，不是推测

上一轮遗留的诱惑是：偏移 `0x17ea3fe` 在 exe 自身内，若能拿到那个二进制就能反查
符号。**这条路已经实测走不通**，判据如下：

| 项 | 值 |
|---|---|
| 崩溃模块 PE 时间戳（事件日志 + WER 双重确认） | `0x6ac8ec24` |
| `release/Alpha File Manager_2.2.0_x64-setup.exe` 内 exe 的 PE 时间戳 | `0x6ac9f3fa`（2026-10-10 16:14:50） |
| 两者大小 | 44760064 字节（同版本号，但**不是同一次构建**） |

10-10 那次重打 2.2.0 用的是 `--bump none`，**版本号没变但二进制是另一次构建**
（对应已知结论：Rust 构建非字节可复现）。

⇒ **本地 2.2.0 包 ≠ 崩溃时的 2.2.0**。偏移量不能套用，本地也没有那个
`0x6ac8ec24` 的二进制。**符号反查这条路判为不可行**，除非将来能从别处拿到那一次
构建产物。

### 2.3 崩溃发生在 2.2.0，2.2.1 已带 release 日志

事件日志与 WER 均显示 `应用程序版本 2.2.0.0`。2.2.1 起 `tauri_plugin_log` 在
release 也启用。但**日志插件本身不足以定位这次崩溃**，原因见 3.1。

### 2.4 更早还有两次 sigma 崩溃（栈溢出，非本次问题）

| 时间 | 模块 | 异常 | 偏移 |
|---|---|---|---|
| 2026-09-26 07:25 | `sigma-file-manager.exe` | `0xc00000fd`（**栈溢出**） | `0x1593152` |
| 2026-09-25 22:08 | `ntdll.dll` | `0xc00000fd`（栈溢出） | `0xbb313` |

`0xc00000fd` 是 stack overflow，与本次 `0xc0000005` **不同类**。同一天 07:43:43 /
07:43:45 还有两次 AppHangB1（无响应，非崩溃）。这些是**独立的历史问题**，
本轮未处理，在此登记以免下次误并为同一根因。

---

## 三、本轮的改动

### 3.1 为什么 release 日志还不够

崩溃点是**原生层访问违例**，且发生在 `setup_handler` 之前 —— 而 `tauri_plugin_log`
正是在 `setup_handler` 里才注册（`lib.rs:450`）。也就是说：

> **崩溃发生时日志插件还不存在，日志文件里必然什么都没有。**

2.2.1 加的日志能覆盖「初始化之后」的故障，但**结构上就覆盖不到启动期**。这是本轮
新增心跳的直接动因。

### 3.2 新增 `src-tauri/src/startup_probe.rs`

一条**不依赖日志插件**的落盘通道，四个埋点：

| 顺序 | 位置 | 标记文本 | 含义 |
|---|---|---|---|
| 1 | `main.rs:17` | `pre_tauri: main entered` | Rust 代码开始执行（最早点） |
| 2 | `lib.rs:232` | `run: entering tauri builder` | 进入 `run()`，开始搭 Builder |
| 3 | `lib.rs:451` | `run: builder built, entering .run()` | Builder 搭好，即将进 `.run()` |
| 4 | `lib.rs:460` | `setup_handler: entered` | **WebView2 与窗口已创建成功** |

落盘位置与应用日志同目录：

```
%LOCALAPPDATA%\com.sigma-file-manager.app\logs\startup-probe.log
```

每行格式：`<unix毫秒>\t<pid>\t<stage>`。

**设计约束（都是踩过的坑）**：

- **`mark()` 永不 panic、永不返回 Err** —— 诊断通道自己绝不能变成新的崩溃源
- **只 append，不覆盖** —— NSIS 那边栽过「反复 append 同一文件只剩最后一行」
- **不引入任何新依赖** —— 启动期手写格式化，不假设 `log` 宏可用
- **stage 截断到 512 字符** —— 防止调用方塞进巨大的命令行
- 目录名 `com.sigma-file-manager.app` 是**受保护常量**，未改

### 3.3 配置 WER LocalDumps（系统侧，零代码）

```
HKCU\SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps
  DumpFolder = C:\Users\Duanyi\AppData\Local\CrashDumps
  DumpType   = 2      (完整 dump)
  DumpCount  = 10     (最多保留 10 个)
```

已写入并回读确认。选 **HKCU 而非 HKLM** 的理由：不需要管理员权限，且只影响当前
用户，不改动系统全局策略。C 盘余量 155 GB，10 个完整 dump 不会撑爆磁盘。

---

## 四、下次复现时怎么判读（这是本文档的主要用途）

### 4.1 先看心跳文件

```
type "%LOCALAPPDATA%\com.sigma-file-manager.app\logs\startup-probe.log"
```

| 看到什么 | 结论 |
|---|---|
| **文件不存在** | 进程死在 Rust 运行时起来**之前** —— CRT / DLL 注入 / apphelp 阶段。**与本项目 Rust 代码无关** |
| 只有标记 1 | 死在 `run()` 之前，**Tauri Builder 构造期间** |
| 标记 1+2 | 死在 Builder 搭建链上（某个 `.plugin(...)` / `generate_handler!`） |
| 标记 1+2+3，缺 4 | **死在 `.run()` 内部** —— 窗口创建 / WebView2 初始化 / 前端加载。结合 WER 里 WebView2 已加载，优先怀疑这一段 |
| 标记齐全（1+2+3+4） | 死在初始化**之后**，与启动无关，按普通 bug 查即可 |

### 4.2 再看 dump

```
dir "%LOCALAPPDATA%\CrashDumps"
```

有 `.dmp` 就带 Report ID 一起反馈。**dump 才是能真正定位到函数的证据** ——
心跳只能告诉你「死在哪一段」，dump 能告诉你「死在哪个函数」。

### 4.3 两条要一起看

心跳与 dump 互补：**心跳告诉你死在哪一段，dump 告诉你为什么**。
只有 dump 没心跳，不知道是不是启动期；只有心跳没 dump，不知道原因。

---

## 五、验证结果

| 门禁 | 结果 |
|---|---|
| `cargo check --lib` | **exit 0**，无 error |
| `cargo test --lib` | **204 passed / 0 failed**（基线 202 + 新增 2 条心跳测试） |
| `cargo build --release` | **exit 0**，2m32s |
| **真实 release exe 端到端实跑** | **4 条心跳全部落盘**，见 5.2 |
| `vue-tsc --noEmit` | **exit 0** |
| `vitest` 全量 | 1115 passed / 4 failed → 逐个复跑后**全部定性**，见下 |

### 新增测试是判别性的，不是平凡通过

`startup_probe::tests` 两条都断言**磁盘上真实产生了可分辨差异**：

1. `mark_appends_a_new_line_with_pid_and_stage` —— 连写两次，断言**第一行没被覆盖**
   且两行都能读出、pid 与当前进程一致、行数恰好为 2。若实现退化成「只写最后一行」
   或「什么都没写」，这两条断言都会失败。
2. `mark_truncates_absurdly_long_stage` —— 10 000 字符的 stage 被截到恰好 512。

> 对照：NSIS 那次踩的坑正是「反复 `FileOpen ... a` 写同一个文件互相覆盖，只剩最后一行」。
> 所以这里显式断言**历史行仍然存在**。

### 端到端：真实 release 二进制实跑（**这一项最初失败过，值得记**）

`cargo test` 全绿**不等于**它在真实 exe 里有效，所以单独构建了 release 并实跑。

**第一次跑失败**：心跳文件没生成。若只看 `cargo test` 全绿就收工，这个 bug 会带到线上。

排查结论：**是验证方法错，不是代码错**。机器上已有一个 Alpha 实例（PID 9676）在跑，
新进程触发 `tauri_plugin_single_instance`，把焦点交给已有实例后**正常 exit=0 退出**，
因此只写到第 3 条就结束了。顺带纠正一个认知：`Start-Process` **不会**继承我在
当前会话里设置的环境变量，所以「用 `$env:LOCALAPPDATA` 隔离目录」这个隔离手段
实际上没生效，心跳写到了真实目录。

**关掉已有实例后重跑，通过**：

```
运行前心跳存在: False          ← 判别性前提：先证明"什么都没做"时不产生文件
22:01:30.695  pid=49968  pre_tauri: main entered
22:01:30.696  pid=49968  run: entering tauri builder
22:01:30.697  pid=49968  run: builder built, entering .run()
22:01:31.334  pid=49968  setup_handler: entered     ← 与上一条相隔 637ms
```

**四条齐全，且给出了启动各段的真实耗时**：

| 阶段 | 耗时 | 说明 |
|---|---|---|
| `main` → Builder 搭好 | **2 ms** | 纯 Rust 侧，四个埋点在此密集写入 |
| `.run()` → `setup_handler` | **637 ms** | **WebView2 创建窗口 + 加载前端全在这一段** |

⇒ 这 637ms 就是崩溃 `0xc0000005` 最可能发生的窗口。**若下次复现时心跳停在第 3 条，
即可确定崩溃发生在 WebView2 初始化期间**，而不必再猜。

> ⚠️ **读日志前先确认没有另一个实例在跑**，否则单实例插件会让新进程秒退，
> 只留下 3 条标记，看起来像「崩在 `.run()` 里」——那是假象。

### vitest 4 个失败的定性（逐文件复跑，非只看总数）

| 文件 | 复跑结果 | 定性 |
|---|---|---|
| `use-navigator-item-icon.test.ts` | **单独跑 4 passed** | 超时型 flaky，与本改动无关 |
| `use-init.test.ts` | **单独跑 14 passed** | 同上（交接文档已登记的 flaky 类型） |
| `module-load-recovery.test.ts` | 见 5.1 | **真实历史遗留，已修** |
| （第 4 个为上述文件的重复计数） | — | — |

### 5.1 顺手修掉的真实遗留

`public/startup-recovery.js:5` 仍是旧品牌文案，且这是**用户实际会看到的**报错：

```diff
-const recoveryMessage = 'Failed to load Sigma File Manager. Please restart the app.';
+const recoveryMessage = 'Failed to load Alpha File Manager. Please restart the app.';
```

修完该文件 **8 passed**（此前稳定失败 1 条）。

**刻意没改**同一文件第 2 行的 `recoveryStorageKey = 'sigma-file-manager:module-load-recovery-at'`：
它是 `sessionStorage` 的**持久化键**，改名会让存量用户的恢复状态失效。属于隐性契约，
不在品牌统一的范围内。

---

## 六、尚未解决 / 下一步

1. **崩溃本身仍未定位。** 本轮只建立了观测能力，没有也不可能定位根因 ——
   没有 dump、没有调用栈，偏移量已判为不可反查（2.2）。
2. **输入法 / 外壳扩展这条线尚未跟进。** 2.1 记下了豆包输入法 TSF 与两个外壳扩展
   在崩溃时已注入进程。这**只是线索，不是根因**。等下次有 dump 时，用它检验
   崩溃栈里是否出现这些模块。
3. **两处环境损坏未处理**：Rust stable toolchain（用 1.90.0 绕过）、
   本机 Python 大面积损坏（发布走独立 venv）。
4. **P2 真机验收更新流程三条仍未做**（`PLAN-...-update-channel.md` §9.6）。
5. **P1 僵尸进程 PID 39920** 重启即可清除，本轮未验证其是否仍在。

---

## 七、给下一位接手者的三条纪律提醒

1. **不要拿 2.2.1 的 release 包当崩溃现场。** 崩溃是 2.2.0，且本地 2.2.0 包是
   **另一次构建**（PE 时间戳 `0x6ac9f3fa` ≠ `0x6ac8ec24`）。偏移量套不上去。
2. **判 Rust 产物有没有变，一律不要比 sha**（非字节可复现）。用「测试是否全绿」
   +「包内载荷与源文件是否逐字节一致」。
3. **若日志文件是空的，不要当成「没崩」或「日志坏了」。** 启动期崩溃时
   `tauri_plugin_log` 尚未注册，空日志是**预期结果**。改看 `startup-probe.log`。