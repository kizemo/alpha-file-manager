# Alpha File Manager —— 双仓重构与品牌化方案(v3,已并入 Claude 评审)

**时间**:2026-10-09 11:45
**性质**:**规划文档。本轮未修改任何被跟踪文件,未提交、未推送、未切分支。**
**基线**:主仓 `dc5f152` / fork `4365281b`(均未推送)
**取代**:`PLAN-2026-10-09-alpha-file-manager.md`(v2,已存档)。**本文是唯一权威版本。**
**并入**:`review-2026-10-09-plan-alpha-file-manager.md`(Claude 评审,2026-10-09 11:35)

---

## 0. ★ 本轮最重要的变化:执行顺序必须调整

Claude 评审 §2 指出了一个我 v2 漏掉的**依赖倒置**:

> **fork 的 `tauri.conf.json` 还没进版本控制。**
> 在这种状态下跑构建,改的全是未跟踪文件 —— **无法 diff、无法 commit、无法回滚**,
> 而 handoff §5-1 那个最高风险项依然悬着。

这条是对的,而且比我原先的排序更要紧。往后看:

| 阶段 | 要改什么 | 那个文件在版本控制里吗? |
|---|---|---|
| P2 开产品分支 | 从 main 切分支 | ✅ 在 |
| P3 改 `bundle.resources` | `tauri.conf.json` | ❌ **不在** |
| P4 改名 | `tauri.conf.json` `productName` | ❌ **不在** |
| P5 换图标 | `tauri.conf.json` `icon` 段 | ❌ **不在** |

**P3/P4/P5 三个阶段全都改同一个未入库的文件。** 如果不先把它纳入版本控制,
这三个阶段全部无法回滚 —— 硬约束 1「任何时候不确定就回滚」直接失效。

### 修正后的执行顺序

```
S0   fork 打包配置入库          ← handoff §5-1,新增前置,必须最先做
  ↓   (tauri.conf.json 等 6 个文件进入版本控制,才谈得上回滚)
P0.5 构建基线验证                ← 新增,证明整合包可重建
  ↓   (产物存档 + 载荷 sha 锚点)
P1   主仓瘦身(删除)              ← 依赖 P0.5 通过
  ↓
P2   fork 产品分支
  ↓
P2.5 树形视图代码归位 + 集成点登记
  ↓
P3   sync 载荷 stage 进 fork
  ↓
P4   改名(productName)
  ↓
P5   图标落地
  ↓
P6   门禁与文档
  ↓
P7   端到端重验
```

> ⚠️ **S0 需要你明确授权才能 commit**(硬约束 5:未经明确要求不提交 git)。
> 你此前说过「暂时不提交」,现在决策已锁定,是否放行 S0 的提交请明示。

---

## 1. 对 Claude 反馈的处理:采纳 4 项,纠正 1 项,降级 1 项

我没有全盘接受。以下逐条给出判断与依据。

### 1.1 ✅ 采纳:P1 删除前必须实测可重建

Claude 说得对,而且这正是我自己记忆里的 **Rule A(测量前先自证)** 与
**Rule I(PASS 可能是平凡的)** 的直接应用。已升级为独立阶段 **P0.5**。

### 1.2 ⚠️ 纠正:重建锚点不该用 `verify-deploy-sha.ps1`

Claude 建议「把 `verify-deploy-sha.ps1` 反过来用」作为基线锚点。**这条我不同意。**

理由是**它属于不同的层**:

| | `verify-deploy-sha.ps1` | 基线构建验证 |
|---|---|---|
| 检查对象 | **已安装**的文件 vs 仓库 | **构建产物**内的载荷 vs 仓库 |
| 前提 | 必须先装一遍 | 不需要安装 |
| 失败含义 | 装机后不一致 | 打包时抓错文件 |

拿一个「装机后验收门禁」去验「构建基线」是层级错配 —— 它要求你**先把刚构建的包装一遍**才能得到结论。

**正确做法**(handoff `packaging-findings` §3 已经用过这个方法):构建后用 `7z l` / 解包,
把安装包里的 `index.js`、`sidecar.exe`、`package.json` 与仓库逐字节比对 sha。
**这才是真正的重建锚点** —— 它证明「同样的输入能产出同样的载荷」。

### 1.3 ✅ 采纳:新增 4 个门禁脚本(见 §9)

Claude 提的 3 条门禁 + 我补的第 4 条,全部采纳,集中列在 §9。

### 1.4 ⚠️ 降级:P3 路径解析的风险被高估了(但仍要测)

Claude 说层级从 `../../` 变 `../` 是「重要风险」。**我认为风险等级比这低,理由如下:**

已经用构建产物证实过(见 §6.1):`bundle.resources` 的源路径以 `<fork>/src-tauri/` 为基准,
且 `../../`(向上两级)能正确解析。`../afm-staging/` 是**向上一级** ——
在同一套解析规则下属于**更简单**的情形,不是更难的情形。

**但仍要实测**:「逻辑上更简单」不等于「已验证」。所以 P3 仍保留首次构建时的当场确认,
只是它不该被描述成一个高风险项。Claude 建议的 dry-run(临时加一条 `../afm-staging/test.txt`)
成本极低,采纳。

### 1.5 ✅ 采纳:集成点登记改用 YAML

Claude 说「用 YAML 而非自然语言表格,便于将来脚本批量对比」—— 这是对的,
自然语言表格无法 diff 出「某个集成点消失了」。已写入 P2.5。

### 1.6 ✅ 采纳:GPL 缺少「同源代码传播」义务

**这是真缺口。** GPL-3 §6 要求:分发二进制时必须同时提供 Corresponding Source。
我 v2 列了 5 条义务,漏了这条最实际的。已补入 §4.3。

### 1.7 ✅ 采纳:「不要全局替换」要写进改动清单,而不只是标注两个例外

Claude §3.6 的提醒很到位。**失败模式是**:有人跑一次全局 find-replace →
`lan_share/types.rs` 和 `windows_installation.rs` 一起被改 →
sha 门禁**照样 PASS**(因为它只比对已安装文件,不看源码常量) →
LAN 共享与默认文件管理器**静默失效**。

已写入 §3.2,并配套门禁 `check-identifier-unchanged.ps1` + 人工审核清单。

### 1.8 ❌ 未采纳:§1.5 中「不写等同 Rule 65 类型的实际产品 bug」

这句引用的是 Claude 自己的规则编号体系,对我没有可执行含义。
但**同一条里的 GPL 同源代码观点独立成立**,已按 1.6 采纳。

### 1.9 ✅ 采纳并已沉淀:模型对字形渲染不可靠

Claude 建议沉淀到 memory。已通过记忆三问:

1. 换用户会变吗?—— 不会,这是模型能力问题不是用户偏好
2. **换项目仍成立吗?—— 成立**,任何需要准确字形的资产都适用
3. 只在本项目成立吗?—— 否

⇒ 写入 **Agent Memory**。

---

## 2. 更正:关于侧栏树形视图(维持 v2 的更正)

我 v1 写「侧栏树形视图 ❌ **不能** 独立成目录」,这句话读起来像在说功能没做完 ——
**我没有这个意思**,你装包验证通过了,我也没有任何证据说它有问题。
而且那个「不能」下得太重。

`git diff --name-status upstream/main...8caf14ae` 实测:

| 分类 | 数量 | 说明 |
|---|---|---|
| **新增文件(A)** | **10** | 全部是我们自己的代码 |
| i18n 语言包 | 18 | 各语言加一个 `tree` 标签 |
| 代码集成点 | 约 29 | **每个仅 1–14 行** |
| 合计 | 55 | 2023 增 / 117 删 |

核心逻辑全是新增,天然可搬:
`use-file-tree.ts` 359 行 / `file-browser-tree-view.vue` 233 行 / `folder-tree.ts` 174 行。

**修正后的结论**:树形视图**可以**做到「自有代码集中在专属目录 + 集成点降到最小且被登记」。
做不到的是「基座零改动」—— Sigma 没有布局扩展插槽,`navigator.vue` 那类接线消不掉。

**你的树形视图不需要任何返工。**

---

## 3. 改名:Alpha File Manager(低成本路径)

### 3.1 核心决策

```
productName : Sigma File Manager  →  Alpha File Manager   ← 改(显示层)
identifier  : com.sigma-file-manager.app  →  不动          ← 技术契约层
```

`identifier` 决定插件安装路径、注册表键、NSIS 卸载注册表项,
**并且是 `verify-deploy-sha.ps1` 的比对锚点**。改它 ⇒ 全新安装路径 ⇒ 重装 + 重写门禁锚点。

### 3.2 ⚠️ 改动方式:逐文件审核,**禁止全局替换**

**这是本节最重要的一条。** 失败模式:

```
全局替换 "Sigma File Manager"
   ├─ 改对了 12 处显示位                    ✅
   └─ 顺手改掉了 2 个互操作常量              ❌ 不会报错
        ├─ lan_share/types.rs:15           → LAN 共享在别的设备上搜不到本机
        └─ windows_installation.rs:30      → 破坏接管 Windows 默认文件管理器
```

**而 `verify-deploy-sha.ps1` 仍然 PASS** —— 它只比对已安装文件与仓库,不检查源码常量。
**这是典型的静默失效。**

| 文件:行 | 常量 | 语义 | 处置 |
|---|---|---|---|
| `src/lan_share/types.rs:15` | `MDNS_INSTANCE_NAME` | mDNS 网络发现服务名 | **禁止改动** |
| `src/windows_installation.rs:30` | `LEGACY_DEFAULT_FILE_MANAGER_DATA_DIR_NAME` | 接管默认文件管理器的数据目录名 | **禁止改动** |
| `src-tauri/default_file_manager_launcher/src/launcher.rs:149` | 临时目录名 | 无对外契约 | 建议不动(无收益) |

### 3.3 需要改的显示位(共 12 处,逐个改)

| # | 文件 | 位置 | 改什么 |
|---|---|---|---|
| 1 | `src-tauri/tauri.conf.json` | L3 | `productName` |
| 2 | `src-tauri/tauri.conf.json` | L16 | 窗口 `title` |
| 3 | `index.html` | `<title>` | 窗口标题 |
| 4 | `src-tauri/Cargo.toml` | L3 | `description` |
| 5 | `src/utils/auxiliary-windows.ts` | — | 辅助窗口标题 |
| 6 | `src/stores/runtime/extensions.ts` | — | 扩展显示名 |
| 7 | `src/stores/runtime/quick-view.ts` | — | 窗口标题 |
| 8 | `src/modules/print-view/pages/print-view.vue` | — | 打印视图标题 |
| 9 | `src/modules/quick-view/pages/quick-view.vue` | — | 快速视图标题 |
| 10 | `src/utils/module-load-recovery.constants.ts` | — | 恢复窗口标题 |
| 11 | `src-tauri/installer/hooks.nsh` | 产物命名段 | 安装包文件名 |
| 12 | fork `.gitignore` | — | 加 `release/Alpha-File-Manager-*.exe` |

### 3.4 连锁影响

| 影响 | 判定 |
|---|---|
| 全新安装默认目录 | `…\Alpha File Manager\`。**符合预期** |
| 现有用户升级 | PREINSTALL 钩子读注册表 `InstallLocation` 复用旧位置 ⇒ **推断不受影响,未实测** |
| 插件安装路径 | **完全不变**(由 identifier 决定) |
| 计划任务 `KizemoFocusSync` | 不变 |
| 验收门禁 | `verify-deploy-sha.ps1` 无需改 |

**判别性验证**:`identifier` 若被误改,门禁**立即 FAIL**。
⚠️ 反过来说,**「门禁 PASS」本身不是证据** —— 必须同时确认 `git diff` 里
`lan_share/types.rs` 与 `windows_installation.rs` 确实没被改。

---

## 4. 品牌与 GPL-3 合规

### 4.1 Alpha ↔ Sigma 的对应

| | Sigma File Manager | Alpha File Manager |
|---|---|---|
| 字母 | Σ(Sigma,求和) | α(Alpha,首位) |
| 关系 | 上游基座 | 二次开发产品 |

同为希腊字母、字母表上紧邻。语义自洽:σ 是「聚拢」,α 是「首位」。

### 4.2 图标不得是 Sigma 图标的像素级复制

fork 许可证为 **GPL-3.0-or-later**,允许 rebrand。三个候选均为独立设计,
§5 的候选 A 是「同构但不同字母」,不构成复制。

### 4.3 ⚠️ GPL-3 完整义务(含本轮补齐的第 6 条)

1. 以 GPL-3 分发,附完整 `LICENSE` 文本
2. 保留上游版权声明
3. 声明本产品**与上游无官方关联**
4. 不使用 Sigma 商标/logo,不得暗示官方背书
5. 不使用会让用户误以为是原版或官方版的措辞
6. **【本轮补齐】分发二进制时必须同时提供 Corresponding Source**
   —— 这是 GPL-3 §6 的核心义务,也是最容易在商业分发时踩空的一条

第 6 条要求 README / 首次运行说明 / 安装器关于页面中明确写:

> 本软件按 GPL-3 分发。
> 源码获取方式:
> - Alpha File Manager: `<仓库 URL>`
> - Sigma File Manager(上游基座): `https://github.com/aleksey-hoffman/sigma-file-manager`
> - Focus Sync 插件: `<仓库 URL>`

主仓 focus-sync 为 MIT,MIT→GPL-3 组合法,组合后整体按 GPL-3 分发。
建议把主仓 `LICENSE` 与实际分发条款对齐,避免混淆。

---

## 5. 图标:候选 A 已选定

| # | 文件 | 构造 | 状态 |
|---|---|---|---|
| **A** | `afm-v3-2-solid-teal.jpg` | 实心 α + 钢青灰文件夹,与 Sigma 的实心 Σ 同构 | ✅ **已选** |
| B | `afm-v3-3-solid-indigo.jpg` | 实心 α + 靛紫 | 备选 |
| C | `afm-v2-1-steel.png` | 描边式 α | 备选 |

生成过程(含 3 轮失败)见 v2 §4.1,要点:

> 第 1、2 轮失败,根因是我写了「thick even weight **strokes**」,把模型引向**描边/环形**构造;
> 改写为「**SOLID FILLED** letterform」后第 3 轮全对。
> **模型对准确字形不可靠,对实心色块可靠。**(已沉淀 memory)

被淘汰的 4 张仍在同目录(本机安全策略拦截了移动操作),**请以本表的 ✅ 为准**。

### 5.1 落地步骤(P5 执行时)

```powershell
# 用脚本,不要手工裁 —— 手工裁剪易错
.\sigma-file-manager\scripts\crop-icon.ps1 `
    -Source docs\brand\icon-candidates\afm-v3-2-solid-teal.jpg `
    -Output C:\Temp\afm-icon-1024.png `
    -Size 1024

cd sigma-file-manager
git status --short src-tauri/icons/      # 起点快照 ← 关键
npx tauri icon C:\Temp\afm-icon-1024.png
git status --short src-tauri/icons/      # 终点快照,必须与预期一致
```

> **为什么先拍起点**:`tauri icon` 会覆盖整个 `src-tauri/icons/`。
> 没有起点快照,就无法区分「正常覆盖」和「误删了图标」。
> 这是 Rule I(判别性探针)的直接应用。

---

## 6. 双仓架构与跨仓路径事实

### 6.1 已由构建产物证实的路径事实

上一次构建生成的 `src-tauri/target/release/nsis/x64/installer.nsi`(2026-10-09 07:40):

```
File /a "/oname=tools\focus-sync-sidecar.exe" "…\sigma-file-manager\src-tauri\..\..\release\extension\bin\focus-sync-sidecar.exe"
```

⇒ `bundle.resources` 源路径以 `<fork>/src-tauri/` 为基准,**8/8 全部指向主仓**。
单 clone fork 仓无法构建出带插件的安装包。

### 6.2 目标架构

```
┌─ 主仓 filemanager = Focus Sync 插件项目(独立) ─────────┐
│  sigma-listary-spike/        sidecar 源码               │
│  release/extension/          插件本体        ★受保护     │
│  release/extension-installer/ 独立 NSIS + 9 PS ★受保护  │
│  release/kizemo.focus-sync-*.exe   只放 sync 独立包      │
│  scripts/  docs/             门禁 + 文档     ★受保护     │
│  ★ 永不出现整合包                                          │
└──────────────────┬──────────────────────────────────────┘
                   │ 构建期 stage(只读主仓 / 只写 fork)
                   ▼
┌─ fork sigma-file-manager = Alpha File Manager ──────────┐
│  upstream 基座 + 28 commit 偏离                          │
│  src/modules/navigator/tree/     树形视图自有代码         │
│  afm-staging/                    stage 产物(须 gitignore) │
│  release/Alpha-File-Manager-<ver>-setup.exe             │
└─────────────────────────────────────────────────────────┘
```

---

## 7. sync 插件仓:必保 / 可删清单

### 7.1 绝对保护(任何阶段不得删改 —— 门禁 `check-fork-readonly.ps1` 守护)

| 路径 | 为什么 |
|---|---|
| `release/extension/{package.json, dist/, locales/}` | 插件本体,stage 的输入 |
| `release/extension/bin/focus-sync-sidecar.exe` | 插件侧 sidecar,stage 的输入 |
| `release/extension-installer/` 全部源码 | 独立安装器 |
| `scripts/{scan-sandbox-dynamic.cjs, verify-deploy-sha.ps1, check-*.cjs}` | 门禁 |
| `sigma-listary-spike/` | sidecar 源码 |
| `docs/` | 文档与 handoff |

### 7.2 可删(已确认直接删除,不迁移保留)

| 路径 | 体积 | 理由 |
|---|---|---|
| `release/Sigma-File-Manager-*.setup.exe` × 6 | **16.7 MB × 6** | 整合包应由 fork 产出 |
| `release/installer/` × 2 | **16.5 MB × 2** | 上游原版包 |
| `release/kizemo.focus-sync-0.2.0-setup.exe` | 821 KB | 废弃旧包,反而被误入库 |
| `release/extension/bin/focus-sync-sidecar-canonical.exe` | 2.5 MB | 已核实两处安装配置**均不引用**;结掉 handoff §5-7 |

### 7.3 应当入库(修复版本控制倒挂)

| 路径 | 理由 |
|---|---|
| `release/kizemo.focus-sync-0.5.8-setup.exe` | **你实测通过的那个包,现在没入库**;废弃的 0.2.0 反而入库了 |

---

## 8. 分阶段计划

### S0 · fork 打包配置入库(★ 新增前置,必须最先)

解决 handoff §5-1:把 6 个未提交文件纳入版本控制。

| 文件 | 内容 | 类别 |
|---|---|---|
| `src-tauri/tauri.conf.json` | `bundle.resources` + `installerHooks` + `installMode: perMachine` | 打包接线 |
| `src-tauri/Cargo.toml` + `Cargo.lock` | Rust 1.90 兼容的依赖钉版本 | 构建兼容 |
| `package.json` + `package-lock.json` | 同批 JS 侧钉版本 | 构建兼容 |
| `src-tauri/src/extensions/extension_http.rs` | `.no_proxy()` focus-21 修复 | 功能修复 |

**为什么 6 个必须一起进**:B 类(依赖钉版本)是让 fork 能在 Rust 1.90 下编译的那批改动
(硬约束 12)。只进 A 类会得到「接线对但编不过」的仓库。

> ⚠️ **该因果关系是推断,未实测** —— 我没跑过「去掉 B 类就编不过」的对照实验。
> P0.5 的首次构建会顺带证实或证伪它。

**验证**:`git status` 干净;`git show HEAD:src-tauri/tauri.conf.json` 能查到 `installerHooks`
**回滚**:`git reset --hard 4365281b`
**阻塞**:需要你授权 commit(硬约束 5)

### P0.5 · 构建基线验证(★ 新增)

**目的**:证明整合包可由 fork 完整重建 —— 这是 P1 删除的唯一硬前置。

```powershell
# 1. Rust 1.90(硬约束 12;1.85 编不了 Tauri)
$env:PATH = "C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;$env:PATH"

# 2. 构建,产物归档到仓库外
cd sigma-file-manager
npm run tauri build
# 产物复制到 C:\Temp\afm-p1-baseline-2026-10-09\

# 3. ★ 重建锚点:解包比对载荷 sha(不是 verify-deploy-sha —— 见 §1.2)
7z l C:\Temp\afm-p1-baseline-2026-10-09\<产物>.exe
#   → 比对 index.js / sidecar.exe / package.json 与 release/ 下源文件逐字节一致
```

**验收标准**:
- 3 个载荷文件 sha 全部与仓库一致
- 记录这些 sha 作为后续所有阶段的**回归基线**
- `verify-nsis-warnings.ps1` **PASS / 0 warnings**(构建期 NSIS 门禁)

**为什么不能用 `verify-deploy-sha.ps1`**:它是**装机后**验收门禁,
用它验构建基线需要先把刚构建的包装一遍 —— 层级错配(详见 §1.2)。

**失败处理**:构建失败则**停止 P1**,先修复。所有 `.exe` 均未入库,删了救不回来。

### P1 · 主仓瘦身(删除已确认)

**前置:P0.5 必须通过。**

1. 删除 §7.2 的 4 类文件
2. 把 0.5.8 独立包纳入版本控制(§7.3)
3. 修 `.gitignore`:加 `release/Sigma-File-Manager-*.exe` 与 `release/installer/`

**验证**:`git ls-files release` 只剩插件相关;§7.1 清单逐项仍在
**回滚**:删除的文件未被跟踪 ⇒ **只能靠 P0.5 的构建重新生成**

### P2 · fork 建立产品分支

- 从 `main` 开 `product/alpha-fm`
- 规则:`upstream/main` 的同步只进基座分支,产品功能只进产品分支
- fork `.gitignore` 加 `release/` 与 `afm-staging/`

### P2.5 · 树形视图代码归位

- 10 个自有文件迁入 `src/modules/navigator/tree/`
- **生成 YAML 登记表**(Claude §1.3 建议):`sigma-file-manager/docs/integration-points.yaml`
- **只搬不改逻辑** —— 功能已完成且验证通过,任何逻辑改动都是引入风险

YAML 字段(机器可比,便于将来脚本批量 diff):

```yaml
- file: src/modules/navigator/pages/navigator.vue
  lines: [120-365]
  kind: integration          # integration | new | i18n
  summary: 挂载 tree layout 分支
  upstream_ref: <upstream commit 如有>
```

**维护纪律**:每次同步 upstream **前后**都重新生成并 diff 这份 YAML。
自然语言表格做不到这件事。

### P3 · sync 载荷 stage 进 fork(方案 B)

**① dry-run**(Claude §1.2):先加一条临时资源 `"../afm-staging/test.txt"`,构建看 NSIS 能否解析
—— 成本极低,能提前排除路径层级问题。**构建后立刻移除该条。**

**② 新增 stage 目录**(在 fork 仓根,必须在 `.gitignore` 里)

```
sigma-file-manager/afm-staging/
├── extension/{package.json, dist/, locales/}
├── installer/{register.ps1, resolve_user_appdata.ps1,
│              register-scheduled-task.ps1, unregister-scheduled-task.ps1}
└── bin/focus-sync-sidecar.exe
```

> ⚠️ **必须 gitignore。** 一旦提交,等于把插件二进制复制进 fork 仓,
> README「仓库边界」一节就作废了。stage 是**产物,不是源码**。

**③ 改 `bundle.resources`**(源路径改指 fork 内部,右侧目标路径不变)

```json
"resources": {
  "../afm-staging/extension/package.json": "extensions/kizemo.focus-sync/package.json",
  "../afm-staging/extension/dist":         "extensions/kizemo.focus-sync/dist",
  "../afm-staging/extension/locales":      "extensions/kizemo.focus-sync/locales",
  "../afm-staging/installer/register.ps1":                 "tools/register.ps1",
  "../afm-staging/installer/resolve_user_appdata.ps1":     "tools/resolve_user_appdata.ps1",
  "../afm-staging/installer/register-scheduled-task.ps1":   "tools/register-scheduled-task.ps1",
  "../afm-staging/installer/unregister-scheduled-task.ps1": "tools/unregister-scheduled-task.ps1",
  "../afm-staging/bin/focus-sync-sidecar.exe":              "tools/focus-sync-sidecar.exe"
}
```

右侧不变 ⇒ **hooks.nsh 的 POSTINSTALL 一行都不用改**。

**④ 扩展 `build-with-sidecar.ps1`**:构建 sidecar 后,新增「复制 8 项载荷到 `afm-staging/`」,
**复制前逐项校验源存在且 sha 非空** —— 缺一项就中止,不留半份 stage。

**⑤ 隔离红线**:stage **只允许「读主仓、写 fork」**。
主仓对 fork 的依赖方向永远是**单向只读**。由 `check-fork-readonly.ps1` 守护。

**验证**(三条,缺一不可):
1. 新生成的 `target/release/nsis/x64/installer.nsi` 中 `File` 源路径指向 `…\afm-staging\…`
2. `verify-deploy-sha.ps1` **PASS**(判别性:载荷字节必须与主仓一致)
3. `git status` 中 **不出现** `afm-staging/`(证明 gitignore 生效)

### P4 · 改名

按 §3.3 逐文件改 12 处。**禁止全局替换**(§3.2)。
同步更新 `hooks.nsh` 产物命名与 fork `.gitignore`。

**验证**:
- `git diff` 确认 `lan_share/types.rs` 与 `windows_installation.rs` **零改动**
- 确认 `identifier` 仍是 `com.sigma-file-manager.app`
- 构建 → 安装 → `verify-deploy-sha.ps1` PASS
- **补充**:跑一遍 LAN 共享发现与默认文件管理器接管(这两个是改名唯一的真实风险面)

### P5 · 图标落地

按 §5.1 执行,**含起点/终点两次 `git status` 快照**。

### P6 · 门禁与文档

落地 §9 全部门禁;更新主仓 README「仓库边界」+ fork README + GPL 同源代码声明(§4.3 第 6 条)。

**验证**:**判别性测试** —— 故意制造错误(stage 少一项、改错 identifier、
动一下受保护文件),门禁必须 FAIL。**在空操作下也绿的检查不含任何信息。**

### P7 · 端到端重验

构建 → 安装 → `verify-deploy-sha.ps1` → **卸载**(顺带结掉 handoff §5-4)。

**验证**:sha 门禁 exit 0;卸载后计划任务 `KizemoFocusSync` 消失、扩展目录清空、
主仓 §7.1 清单 sha 未变

---

## 9. 门禁清单(本轮新增 4 个)

| # | 脚本 | 位置 | 作用 | 防的是 |
|---|---|---|---|---|
| 1 | `check-stage-payload-sha.ps1` | 主仓 `scripts/` | 比对 `afm-staging/` 载荷 sha 与主仓 `release/extension/…` sha,不一致则拒绝构建 | stage 抓到旧文件 → 装出「没功能的旧插件」(本项目 2026-09-30 的历史故障) |
| 2 | `check-fork-readonly.ps1` | 主仓 `scripts/` | fork 构建前校验主仓 §7.1 保护清单文件 sha 未变 | stage 误改/误删主仓文件 |
| 3 | `check-identifier-unchanged.ps1` | fork `scripts/` | 断言 `identifier == com.sigma-file-manager.app` | 改名时手滑改了 identifier(全局替换的替身守卫) |
| 4 | `crop-icon.ps1` | fork `scripts/` | 候选图裁到 1024×1024,消除手工裁剪误差 | 手工操作不可复现 |

> 命名沿用 fork 现有惯例(`verify-nsis-warnings.ps1`、`verify-msix-store-compatibility.ps1`)。
> 所有 `.ps1` **一律纯 ASCII**(硬约束 10,已被坑 4 次)。

---

## 10. 决策状态

| # | 事项 | 状态 | 决定 |
|---|---|---|---|
| 1 | 产品名 | ✅ | **Alpha File Manager** |
| 2 | `identifier` | ✅ | 保持 `com.sigma-file-manager.app` **不变** |
| 3 | 图标 | ✅ | **候选 A** `afm-v3-2-solid-teal.jpg` |
| 4 | P3 方案 | ✅ | **方案 B** stage 进 fork 内部路径 |
| 5 | 旧整合包 | ✅ | **直接删除** |
| 6 | 两个互操作常量 | ✅ | **保持不动**,且**禁止全局替换** |
| 7 | S0 提交授权 | ⏳ **待你明示** | 硬约束 5 要求 commit 需明确授权 |
| 8 | P3 dry-run | ✅ | 采纳(成本极低,提前排除路径层级问题) |
| 9 | 重建锚点方法 | ✅ | **解包比对载荷 sha**,不用 `verify-deploy-sha.ps1` |

**唯一未决**:第 7 项 —— S0 的提交授权。其余全部就绪。

---

## 11. 我测了什么 / 没测什么

**测了**(可复现):
- 树形视图 55 文件的 A/M 分布(新增 10 / 修改 47;i18n 18 / 集成点约 29)
- 扩展 API `contributes` 允许键:**无布局插槽**
- `productName` / `identifier` / license 实际取值
- `"Sigma File Manager"` 在 `src-tauri` 8 处、`src/`(排除 i18n)7 个文件
- 两个互操作常量的位置与语义
- Sigma 现有图标(读图确认「文件夹 + 实心白 Σ」)
- 3 轮图标生成的失败与成功记录(每张实际读图核验字形)
- **`bundle.resources` 路径基准**:读构建产物 `target/release/nsis/x64/installer.nsi`
- `C:\Temp` 存在(793 项);两仓 `scripts/` 现有门禁清单与命名惯例

**没测 / 未验证**:
- ❌ **没有跑过任何构建**。P0.5 / P3 / P4 的可行性基于静态分析
- ❌ **没有验证整合包可由 fork 完整重建** —— P0.5 要做的事
- ❌ 「B 类依赖钉版本是编译必需」是**推断**,P0.5 首次构建可证实/证伪
- ❌ **`../afm-staging/` 能否被 Tauri 正确解析未实测**(P3 dry-run 要做的事)
- ❌ 改 `productName` 后现有用户升级行为未实测
- ❌ 候选 A 未裁到 1024×1024,未跑 `tauri icon`
- ❌ **LAN 共享 / 默认文件管理器接管功能在改名后的实际表现未验证**
  (我只能保证常量没被改,不能保证这两个功能本来就正常)
- ❌ GPL 第 6 条(同源代码传播)的具体落地形式未设计,只提出了要求
- ❌ 未核实 GitHub 仓库改名与域名可用性

**git 状态**:本轮未提交、未推送、未切分支;两仓被跟踪文件**零改动**。
新增内容仅:本文档(未跟踪)。图标候选目录与 v1/v2 文档亦为未跟踪新文件。
