# HANDOFF —— 插件源码并入品牌仓(2026-10-09 傍晚)

**交接时间**:2026-10-09 18:40
**上一份**:[`handoff-2026-10-09-alpha-fm-rebrand.md`](handoff-2026-10-09-alpha-fm-rebrand.md)(下午场)
**本轮性质**:验证基线 → 瘦身主仓 → **架构调整(插件源码并入品牌仓)**,两仓均已提交并推送
**状态**:✅ 跨仓依赖已消除 · ⬜ 改名/图标/端到端重验**均未开始**

---

## 0. 三十秒速览

| 项 | 值 |
|---|---|
| **主仓**(发布存档) | `F:\soft\00selfmade\filemanager`,HEAD **`758f466`**,**已推送,与 origin 一致** |
| **品牌仓** | `F:\soft\00selfmade\filemanager\sigma-file-manager`,**分支 `main` @ `a3577184`**,**已推送** |
| 品牌仓 `main` | **`a3577184`**,**✅ 已含插件源码** —— §5 的 A 已于 2026-10-09 完成(快进合并,见 §5.1) |
| 两仓工作区 | **均干净** |
| 品牌仓远程 | `https://github.com/kizemo/alpha-file-manager.git` |
| 装机现状 | **本轮未安装任何东西**,跑的还是昨天那个已验证的安装包 |
| 隔离区 | `C:\Temp\afm-p1-quarantine-2026-10-09\` —— 4423 文件 / 1.9 GB,**全部可回滚** |

---

## 1. 本轮最重要的一条:架构已经变了,旧文档的路径全部作废

```
┌─ 改造前 ────────────────────────────────────────┐
│  主仓 filemanager(focus-sync,MIT)               │
│    sigma-listary-spike/   侧车源码              │
│    release/extension/     插件本体              │
│    scripts/               5 个插件门禁          │
│        ↕ 互相知道对方位置(互相读文件)            │
│  品牌仓 alpha-file-manager(GPL-3,fork)          │
│    tauri.conf.json 指向 ../../release/...  ← 跨仓 │
└──────────────────────────────────────────────────┘

┌─ 改造后(现状)──────────────────────────────────┐
│  品牌仓 alpha-file-manager —— 唯一开发仓         │
│    extensions/kizemo.focus-sync/                │
│      sidecar/      侧车 Rust 源码(13 文件)      │
│      dist/         插件主文件(手工维护)         │
│      locales/      语言包                       │
│      installer/    独立 NSIS + 9 个 PS          │
│      package.json / LICENSE                    │
│    scripts/  build-with-sidecar / verify-nsis   │
│              scan-sandbox-dynamic / deploy-sha  │
│    src/modules/extensions/runtime/sandbox.ts    │
│        ↑ 全部在本仓内,零跨仓依赖                 │
│                                                 │
│  主仓 focus-sync —— 只剩发布存档                  │
│    release/kizemo.focus-sync-0.5.8-setup.exe    │
│    docs/ + 全部历史交接记录                      │
└──────────────────────────────────────────────────┘
```

**证据不是推算,是构建产物**:新生成的 `installer.nsi` 里 **10 条载荷来源 100% 指向品牌仓内部**。

> 旧文档 [`PLAN-…-v3.md`](PLAN-2026-10-09-alpha-file-manager-v3.md) 的 **P3 已作废** ——
> 它设计的是「把产物 stage 进品牌仓」,那仍然要求主仓就摆在旁边,依赖没消除。
> 源码搬进来才是真的消除。

---

## 2. 硬约束(违反即任务失败)

上午的 12 条 + 下午的 13–18 条**全部继续有效**。本轮**新增**:

| # | 约束 |
|---|---|
| **19** | **编侧车也用 Rust 1.90。** 已实测:1.90 冷构建通过 + **73 测试全绿**。合并后单一工具链,**不再需要切换版本**(旧文档说侧车要 1.85,那是「试过能成」不是硬约束) |
| **20** | ⚠️ **Rust 构建不是字节可复现的。** 同源码同编译器,**只换输出目录**,侧车指纹就变。⇒ **侧车指纹永远不能当回归锚点**,只验「73 个测试是否通过」。拿 sha 比侧车会随机失败,而失败时东西没坏 —— **这种门禁比没有更糟,它训练你忽略门禁** |
| **21** | **改 `.nsi` 必须保留 UTF-8 BOM。** 该文件有中文提示框且声明 `Unicode true`,丢 BOM 会让 makensis 读不懂编码。本轮我自己踩过,靠字节数差 63 抓到 |
| **22** | **`.ps1` / `.nsi` 一律纯 ASCII**(延续旧约束 10),改完必须跑语法检查 + ASCII 计数 |
| **23** | **品牌仓的 `dist` 忽略规则会吞掉插件载荷。** 根级 `dist` 匹配任意深度,已加目录级否定 `!extensions/kizemo.focus-sync/dist/`。**否定文件本身无效** —— git 不进入被排除的目录 |

### ⚠️ 旧文档有一处路径是错的

[`PLAN-…-v3.md`](PLAN-2026-10-09-alpha-file-manager-v3.md) §3.2 标的互操作常量路径**少了一层目录**:

| 常量 | 计划文档写的 | **实际位置** |
|---|---|---|
| `MDNS_INSTANCE_NAME` | `src/lan_share/types.rs:15` | **`src-tauri/src/lan_share/types.rs:15`** |
| `LEGACY_DEFAULT_FILE_MANAGER_DATA_DIR_NAME` | `src/windows_installation.rs:30` | **`src-tauri/src/windows_installation.rs:30`** |

行号是对的。**P4 改名时按文档去找会扑空**,很可能就顺手做了全局替换 ——
那正是「静默失效」事故的入口。已写进 [`baseline-2026-10-09-build-anchors.md`](baseline-2026-10-09-build-anchors.md) §5。

---

## 3. 本轮完成的三件事

### 3.1 P0.5 构建基线验证 ✅

证明整合包可由品牌仓完整重建 —— 这是删除任何 `.exe` 的硬前置。

| 验收 | 结果 |
|---|---|
| 载荷 sha 逐字节一致 | **3/3** |
| `verify-nsis-warnings.ps1` | **PASS -- 0 warnings** |
| 沙箱门禁 | `VALID (0 violations / 19 patterns)` |
| 侧车测试 | **73 passed / 0 failed** |

**测量方法先自证过**:同一套「解包比对」跑两个对照 —— 已验证的好包 3/3 MATCH,
2026-09-29 的旧包 2 DIFF + 1 MISS。**方法既能判通过也能判失败,结论才有信息量。**

### 3.2 P1 主仓瘦身 ✅(`0d24f2a`)

隔离式移除 **10 个文件**(先移入隔离区,可回滚,非销毁):
5 个整合包 + 2 个上游原版包 + 2 个 0.2.0 废弃包 + `focus-sync-sidecar-canonical.exe`。

删除前做了**三重验证**:按文件名全仓搜索(只命中文档,无活代码引用)、
扫全部打包脚本(引用全指向已安装目录)、路径红线硬校验(越界即中止)。

`release/` 从 125 MB → 8 MB。`0.5.8` 那个实测通过的包**纳入版本控制**(此前在库的
反而是已废弃的 0.2.0)。顺手修掉两个会出事的地方:`.gitignore` 漏了实际产物命名
(`Sigma File Manager_` 带空格)**和 P4 改名后的新命名**(安全网会在最需要它的
时刻静默过期);`build.ps1` 头部注释写着 0.2.0 而实际输出 0.5.8。

### 3.3 架构调整:插件源码并入品牌仓 ✅(`a3577184` + `48ea331`)

`feat/plugin-source-in` 分支,48 文件 +9571 行;主仓 49 文件 **−9895 行**。

改动的 7 个现存文件:`tauri.conf.json`(bundle.resources 改仓内)、
`.gitignore`、`build-with-sidecar.ps1`(全路径重写 + **新增「产物早于新构建即中止」
的过期检测**)、`verify-nsis-warnings.ps1`、`verify-deploy-sha.ps1`、
`scan-sandbox-dynamic.cjs`、`installer.nsi`。

> 三个门禁脚本原本都硬编码了主仓绝对路径或跨仓拼路径。`verify-nsis-warnings.ps1`
> 单独跑会报一句莫名其妙的 `hooks.nsh not found` —— 本轮实测才发现。

### 3.4 独立安装器改路径后实测可编译 ✅

`extensions\kizemo.focus-sync\installer\installer.nsi` 的 6 条路径改完后
**实际跑了一次 makensis**:退出码 0、**0 warning**、产出 970 KB 安装包。

把它和主仓存档那个已发布的 0.5.8 包逐个载荷拆开比对:

| 载荷 | 结果 |
|---|---|
| `dist/index.js` | **SAME** `ECED2EBE4D1598BA…` |
| `package.json` | **SAME** `9F34E0D2FAAC757C…` |
| `kill-sigma.ps1` / `register.ps1` / `README.md` | **SAME** |
| `focus-sync-sidecar.exe` | DIFF —— **预期内**(1.90 vs 1.85,且 Rust 非字节可复现) |

⇒ **「需要独立发版时从品牌仓打包」这条路已验证走通**,主仓的发布存档模式成立。

---

## 4. ⚠️ 本轮顺手修掉的一处静默失效

主仓提交时输出:`pre-commit: WARNING - scanner or sandbox.ts not found; sandbox gate SKIPPED.`

**沙箱门禁在静默跳过**(脚本搬走了)。这就是本项目最恨的形状:门禁还在,已经不检查
任何东西。

- 主仓 hook → 换成一句说明(已无物可扫,原文件备份在隔离区)
- **品牌仓装上同一道门禁**,并做**双向判别测试**:
  放 `foreground window.`(历史上真实踩雷的那句)→ **拦,exit 1** ✅
  换合规内容 → **放,exit 0** ✅

---

## 5. 下一步

| # | 阶段 | 内容 | 阻塞? |
|---|---|---|---|
| **A** | ~~合并 `feat/plugin-source-in` → 品牌仓 `main`~~ | ✅ **已完成** —— 快进合并,`main` = `a3577184`,已推送 | — |
| B | P4 改名 | 按计划 §3.3 改 12 处显示位。**按 §2 的真实路径核对那两个常量** | ⬅ **从这里开始** |
| C | P5 图标 | 候选 A `afm-v3-2-solid-teal.jpg` → 裁 1024 → `npx tauri icon`(含前后两次 `git status` 快照) | 依赖 B |
| D | P2.5 | 树形视图代码归位 + `integration-points.yaml` 登记表 | 独立 |
| E | P6 | 门禁脚本 + GPL 声明(README 已补 §6 源码义务) | — |
| F | P7 | 端到端重验:构建→安装→sha 门禁→**卸载**(卸载路径至今从未验证过) | 依赖 B/C |

### 5.1 ✅ A 已完成:`feat/plugin-source-in` → `main`(2026-10-09 晚)

**决策:直接快进合并,不开 PR。** 理由:单仓自 fork 无评审人;待入的只有 1 个**已经过 P0.5 验证**的提交;
快进让 `main` 与被验证过的那棵树**逐字节相同**,「main 就是验过的那个状态」可以直接审计。

| 项 | 合并前 | 合并后 |
|---|---|---|
| 品牌仓 `main` | `9e764a16` | **`a3577184`** |
| 与 `origin/main` | 一致 | **一致(服务器侧 `ls-remote` 已确认)** |

**快进判定**:`git merge-base --is-ancestor main feat/plugin-source-in` → true,
待入提交 **1 个**,两侧**零分叉**(feature 侧 1,main 侧 0)。

**合并后实测**:

| 检查 | 结果 |
|---|---|
| 沙箱门禁(合并后的 `main` 上跑) | **VALID(0 violations / 19 patterns,exit 0)** —— 动态从源码提取,**确认在执行而非静默跳过** |
| 约束 23:`dist` 是否被忽略规则吞掉 | 3 个文件 `git check-ignore` 均返回「未忽略」✅ |
| 约束 21:`installer.nsi` BOM | **`EF BB BF` 存在** ✅(17234 字节) |
| 跨仓残留:`scripts/` + `installer.nsi` | 搜 `filemanager` / `00selfmade` / `listary-spike` / `release\extension` → **0 命中** |
| `tauri.conf.json` `bundle.resources` | 8 条全部指向 `../extensions/kizemo.focus-sync/…`,**仓内** |
| 载荷实际落盘 | `dist/index.js` 37144 B、`installer.nsi` 17234 B 等 6 个关键文件均在 |
| 工作区 / 与上游差异 | CLEAN,`0  0` |

**回退**:`9e764a16` 作为祖先永久可达,无需额外打 tag。
若需回退 —— `git branch rollback-a 9e764a16`,再决定是否把 `main` 拨回去。

> ⚠️ **本次未覆盖的**(别当成已验):未构建、未安装、未部署侧车。
> `main` 与被验证的 `a3577184` 逐字节相同是**快进的构造保证**,属**平凡检查**,
> 它证明的是「没有引入偏移」,不是「在新状态下构建通过」。真正的构建验证属 P7。
>
> ⚠️ **观察(非本次引入,未改)**:约束 22 要求 `.ps1` 纯 ASCII,实测 6 个 `.ps1` 含
> `—`(U+2014)、`→`(U+2192)、`§`(U+00A7)**注释**字符,CJK 计数为 0。
> 纯观感问题,且这些文件在 P0.5 已实测编译 0 warning。**留待 P6 门禁阶段一并清理**,勿在推送前动安装器脚本。

### 未做 / 未验证

- ❌ **本轮未安装任何东西。** 「装进 `D:\Program Files\` 后一切正常」仍未验证
- ✅ 品牌仓 `main` 已含插件源码(见 §5.1)
- ❌ 改名、图标未开始
- ❌ 卸载路径从未端到端验证
- ❌ 扩展激活耗时 1–3 分钟根因未查
- ❌ H2 假成功、manifest 版本谎报等旧问题仍在(优先级低)
- ⚠️ 新编的侧车**尚未部署到实机**。1.90 产物通过 73 测试且载荷逐字节一致,
  但**跑着的还是 1.85 那个**。要部署需走 P7 端到端重验

---

## 6. 回退节点

| 位置 | 节点 | 说明 |
|---|---|---|
| **主仓** | **`758f466`** | ★ 当前状态,**已推送** |
| 主仓 | `48ea331` | 插件源码迁出、交接文档入库 |
| 主仓 | `0d24f2a` | P1 瘦身后、架构调整前 |
| 主仓 | `2109d59` | 本轮开工前 |
| **品牌仓** | **`a3577184`** | ★ `main`(已快进合并),**已推送** —— 同一 SHA 也是 `feat/plugin-source-in` |
| 品牌仓 | `9e764a16` | **合并前的 `main`**,作为祖先永久可达,回退锚点 |
| 品牌仓 | `feat/dialog-focus-sync` | ⚠️ **另有 11 个提交尚未并入 `main`**,本轮未处理 |
| 主仓 tag | `v0.5.9-packaging-20261009` | 上午的打包基线,仍有效 |
| 磁盘 | `C:\Temp\afm-p1-baseline-2026-10-09\` | P0.5 基线产物 + 记录 |
| 磁盘 | `C:\Temp\afm-p1-quarantine-2026-10-09\` | **本轮移走的全部东西,可回滚** |

⚠️ **不要**回退到 `v0.5.8-packaging-baseline-20261008`(那个节点的安装包不可用)。

---

## 7. 运维命令

```powershell
# 编侧车(新位置,Rust 1.90)
$env:PATH = "C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;$env:PATH"
cd sigma-file-manager\extensions\kizemo.focus-sync\sidecar
cargo build --release; cargo test --release

# 沙箱门禁 + 部署到两个打包点
cd ..\..\..\..
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build-with-sidecar.ps1

# 构建整合包
npm run tauri build

# NSIS 警告门禁(Tauri 会吞掉 makensis 的警告,必须绕开)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-nsis-warnings.ps1

# 沙箱门禁(单独跑)
node scripts\scan-sandbox-dynamic.cjs `
  extensions\kizemo.focus-sync\dist\index.js `
  src\modules\extensions\runtime\sandbox.ts

# 装机 sha 验收(退出码 0 才算通过)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-deploy-sha.ps1

# 独立安装包(给 Sigma FM 用户)
powershell -NoProfile -ExecutionPolicy Bypass -File extensions\kizemo.focus-sync\installer\build.ps1
```

**冷启动提示**:文件管理器启动后扩展要 1–3 分钟才激活,这期间地址栏不同步是**预期行为**。
```powershell
(Invoke-RestMethod http://127.0.0.1:37421/health) | Select-Object extension_alive
```

---

## 8. 给新会话的一句话 prompt

```
读 F:\soft\00selfmade\filemanager\docs\handoff-2026-10-09-plugin-consolidation.md(必读)。
§5 的 A 已完成:feat/plugin-source-in 已快进合入品牌仓 main,两仓均已推送
(主仓 758f466,品牌仓 main = a3577184,与 origin 一致)。
下一步做 §5 的 B:P4 改名。
动代码前务必读 §2 硬约束 —— 尤其第 20 条(Rust 构建非字节可复现,侧车指纹
不能当锚点)和第 21 条(改 .nsi 必须留 UTF-8 BOM)。
⚠️ 旧文档 PLAN-v3 的 P3 已作废,§3.2 标的两个互操作常量路径是错的
(实际在 src-tauri/src/ 下),P4 改名时务必按 handoff §2 的真实路径核对。
不要重新讨论名字/图标/方案,决策已全部锁定。
```

---

## 9. 本轮经验教训

1. **「PASS」也可能是被跳过的。** 主仓提交时那句 `sandbox gate SKIPPED` 是警告不是
   错误,退出码 0,提交照常成功 —— **没人会注意到**。一个从不执行的门禁等于不存在,
   而且比没有更危险,因为它让人以为有保护。

2. **门禁搬走时要跟着搬,不能只搬文件。** 插件源码进品牌仓那天,沙箱门禁脚本一起
   搬了,但两边的 hook 都失配:主仓的静默跳过,品牌仓的**根本没有**。
   **搬代码时把「围着它的门禁」列成清单。**

3. **Rust 构建不是字节可复现的。** 我一直拿 sha 当回归锚点,这次实测才发现
   同源码同编译器只换输出目录指纹就变。**任何靠指纹判断「有没有变」的检查,
   在 Rust 项目上都是随机失败** —— 而失败时东西没坏。长期这么用,人会开始习惯性
   忽略门禁报警。

4. **字节数对不上是免费的线索。** 我改完 `installer.nsi` 没做任何别的检查,只因为
   大小差 63 字节(路径缩短 60 + BOM 3)才发现编码被自己弄坏了。**改文件前后各看一次
   字节数**,几乎零成本。

5. **自己的测试工具坏了,结论就是无效的。** 我测 pre-commit hook 时用了 `sh`,
   环境里没有 —— 两次都返回 0,看起来「合规内容放行」通过。换成正确路径后又因为
   工作目录不对,门禁报 not found、跳过、exit 0,**又一次看起来通过**。
   **已知为真的东西报假 ⇒ 尺子坏了,不是东西坏了。** 本项目的老毛病,一天犯两次。

6. **文档里的路径也会是错的,不是说错了就是错了。** 计划文档标互操作常量的路径
   少了一层 `src-tauri/`,行号却是对的 —— 这种「一半对」最危险,因为它看起来可信。

7. **搬走的东西要留着退路。** 两次删除(P1 的 10 个包、本轮的目录)都是**移入隔离区**
   而不是销毁。现在隔离区有 4423 个文件 / 1.9 GB,任何一步判断错了都能拿回来。

8. **「记为未验证」不如当场验掉。** 本可以交一份写着「独立安装器尚未编译验证」
   的交接,但那只是把风险推给下一个人。跑一次 makensis 只要十几秒 —— 结果它不仅
   能编,产出的载荷还和已发布包**逐字节相同**。**能用一次测量消掉的未知,不要写进交接。**