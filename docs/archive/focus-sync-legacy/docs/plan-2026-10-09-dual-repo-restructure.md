# 双仓重构方案 —— sync 插件仓 / 二次开发软件仓

**时间**:2026-10-09 10:31
**性质**:**规划文档,未实施任何代码改动,未提交任何东西**
**基线**:主仓 `dc5f152` / fork `4365281b`(均未推送,fork main 领先 origin 21)

---

## 0. 结论先行

1. **要求 1(主仓只放 sync 插件包)可以做,纯搬运,低风险。** 主仓 `release/` 现在混了
   **6 个 Sigma FM 整合包(16.7 MB×6)+ 2 个上游原版安装包**,它们根本不属于 sync 插件项目。

2. **要求 2 里「独立的目录树」只对一部分内容成立。** 这不是意愿问题,是 Sigma 的扩展 API
   决定的 —— **扩展的 `contributes` 只支持 `commands` / `configuration` / `keybindings` /
   `iconThemes`,没有布局插槽**。所以:
   - ✅ **sync 插件**天然独立(已是标准扩展,本来就在自己的目录里)
   - ❌ **树形视图这类 UI 功能**必须侵入 upstream 的 `src/`,**无法抽离**成独立目录

3. **建议改 `productName`,不动 `identifier`。** 理由见 §3.2,这一条决定了改名成本是"低"还是"极高"。

4. **fork 内目前没有任何 focus-sync 源码** —— 只有 `tauri.conf.json` 的 8 条 `bundle.resources`
   指向主仓。这是 §5-1 风险的结构性来源。

---

## 1. 现状勘查(实测证据)

### 1.1 主仓 `filemanager`(focus-sync)

被跟踪文件分布:`docs` 96 / `release` 23 / `sigma-listary-spike` 22 / `scripts` 14 / 根 4 / `.github` 2

`release/` 实测混装情况:

| 内容 | 文件 | 体积 | 是否入库 |
|---|---|---|---|
| **sync 独立安装包(该留)** | `kizemo.focus-sync-0.5.8-setup.exe` | 955 KB | ❌ **未入库** |
| sync 旧包(该清) | `kizemo.focus-sync-0.2.0-setup.exe` | 821 KB | ✅ 入库 |
| **Sigma FM 整合包(该走)** | `Sigma-File-Manager-2.2.0-focus-15-setup.exe`<br>`…2.2.2-focus-21-v0.5.{3,4,5,8}-setup.exe` | **16.7 MB × 6** | ❌ 未入库 |
| **上游原版安装包(该走)** | `installer/Sigma File Manager_2.2.0_x64-setup.exe`<br>`installer/Sigma-File-Manager-2.2.0-focus-1-setup.exe` | 16.5 MB × 2 | ❌ 未入库 |
| **陈旧二进制(悬案)** | `extension/bin/focus-sync-sidecar-canonical.exe` | 2.5 MB | ✅ **入库了** |
| 插件源(该留) | `extension/{package.json,dist,locales}` | — | ✅ |
| 插件安装器源(该留) | `extension-installer/{installer.nsi, 9 个 PS}` | — | ✅ |

**两个版本控制倒挂**(风险点):
- 用户实测通过的那个 **0.5.8 安装包没入库**,反而是废弃的 0.2.0 入库了
- 2.5 MB 的 `focus-sync-sidecar-canonical.exe` 入库了,而 handoff §5-7 至今没定它的去留

### 1.2 fork `sigma-file-manager`

- `upstream` = `aleksey-hoffman/sigma-file-manager`,**领先 28 个提交**
- 偏离文件 63 个,其中 **55 个在 `src/`** —— 主体是树形视图功能(`feat/tree-sidebar-v6-1`,已并入 main)
- 本地分支:`main`(领先 21)、`feat/dialog-focus-sync`(领先 11)、`feat/tree-sidebar-v6-1`、
  `feat/tree-view`、`docs/kizemo-fork-readme`
- **`release/` 是构建产物且未被 `.gitignore` 覆盖**(15.8 MB 安装包一直挂在 `git status`)
- **fork 内没有任何 focus-sync 源码**。全仓搜索 `kizemo.focus-sync` 只命中
  `src-tauri/target/**`(历史构建残留)和 `tauri.conf.json`

### 1.3 身份标识

```
productName : Sigma File Manager        ← 品牌,改名成本低
identifier  : com.sigma-file-manager.app ← 技术标识,改名成本极高
license     : GPL-3.0-or-later
```

`identifier` 在主仓 58 个文件、fork 6 个文件出现。它决定:
`%APPDATA%\com.sigma-file-manager.app\extensions\…`(插件安装路径)、
`HKLM/HKCU\SOFTWARE\kizemo\focus-sync\SidecarPath`、
NSIS 生成的 `…\Uninstall\com.sigma-file-manager.app_is1` 注册表项。

**`identifier` 同时是 §1 验收门禁的锚点** —— `verify-deploy-sha.ps1` 走的就是这个路径。

---

## 2. 目标架构

```
┌─ 主仓:Focus Sync 插件项目 ────────────────┐
│  sigma-listary-spike/   sidecar 源码        │
│  release/extension/     插件本体            │
│  release/extension-installer/  插件独立 NSIS │
│  release/*.exe          ← 只放 sync 独立包   │
│  ★ 不再出现任何 Sigma FM 整合包              │
└──────────────────┬─────────────────────────┘
                   │ 构建期:载荷 stage(需显式化)
                   ▼
┌─ fork:二次开发软件仓 ──────────────────────┐
│  upstream 基座(28 commit 偏离)              │
│  + 侵入式功能(tree view 等,留在 src/)       │
│  + 独立功能树(扩展/插件,自己的目录)          │
│  release/<产品名>-v<版本>-setup.exe ← 整合包 │
└────────────────────────────────────────────┘
```

---

## 3. 关键约束分析

### 3.1 「独立目录树」的真实边界 ⚠️ 最重要

| 内容 | 能否放进独立目录 | 原因 |
|---|---|---|
| sync 插件 | ✅ **已经就是** | Sigma 标准扩展,装在 `extensions/kizemo.focus-sync/`,与主程序零源码耦合 |
| 侧栏树形视图 | ❌ **不能** | `contributes` 无布局插槽;需改 `FileBrowser*` 组件、布局解析、i18n 键 |
| 未来类似功能 | ⚠️ **看类型** | 命令/工具栏/主题/配置类 → 可插件化;UI 深度集成类 → 只能侵入 |

**这意味着「完全独立的目录树」无法 100% 达成。** 可达成的是:
> **可插件化的部分走独立目录;必须侵入的部分明确标记为「基座修改」并隔离在专用分支。**

如果你要的是**字面意义上的纯净 overlay**(基座零改动),成本是把 55 个 `src/` 改动重构成
「upstream 纯净层 + 自己的 overlay 层 + 构建期合并」,这需要 Sigma 支持运行时插槽 —— **它不支持**。
建议**不追求这个**,改为:用分支策略 + 门禁来管理侵入范围。

### 3.2 改名策略:改 productName,保留 identifier

| 项 | 建议 | 理由 |
|---|---|---|
| `productName` | ✅ **改** | 纯显示名,影响窗口标题、开始菜单、默认安装目录名 |
| `identifier` | ❌ **不改** | 改了 = 全新安装路径。要重装、重注册、重建计划任务、**并重写 §1 验收门禁的锚点** |
| 产物文件名 | ✅ **改** | `Sigma-File-Manager-…` → `<产品名>-<版本>-setup.exe` |

**注意**:改 `productName` 会改变**全新安装**的默认目录(`C:\Program Files\<productName>`)。
但 fork 的 PREINSTALL 钩子会从注册表读 `InstallLocation` 复用旧位置,所以**现有用户升级路径不受影响**。

### 3.3 许可证约束

fork 是 **GPL-3.0-or-later**,`LICENSE.md` 明文允许 "modify, rebrand, or distribute a derivative work"。
改名合法,但必须:
- 以 GPL-3 分发,附完整许可证与源码获取方式
- 保留上游版权声明 + 声明本产品与上游无官方关联
- **名称不能让人误以为是原版或官方版** —— 这也是选名时不用 "Sigma" 系的理由

另:主仓 focus-sync 是 MIT。MIT → GPL-3 组合法(MIT 许可宽松),组合后整体按 GPL-3 分发。
**建议把主仓 LICENSE 与产品实际分发条款对齐,避免混淆。**

---

## 4. 三个产品名候选

均已排除 "Sigma" 系(避免与上游混淆,GPL 义务 + 用户体验)。

| # | 名称 | 定位 | 优势 | 代价 |
|---|---|---|---|---|
| **A** | **Kizemo File Manager**(KFM) | 个人品牌型 | 与你的 GitHub handle 一致;作者归属一眼明确;GPL 声明最简单;后续加功能永不改名 | 品牌感偏弱,像个人项目而非产品 |
| **B** | **Prism File Manager**(棱镜) | 平台聚合型 | 一个基座折射出多种能力 —— **精准对应「Sigma 基座 + 持续叠加功能」的产品定义**;与上游零字面关联 | 无个人痕迹,辨识度要从零建立 |
| **C** | **Kestrel File Manager**(红隼) | 特征效率型 | 小型猛禽,俯冲精准、反应极快 —— 契合文件定位/跳转/效率;短、好读、有画面感、全球无同名冲突 | 与"树形/同步"的关联语义偏弱 |

**我的倾向:B(Prism)**。理由是它把「这是个会持续长功能的产品」写进了名字本身,
而 A 会把你锁死在个人项目形象上,C 的语义与实际功能关联最弱。

> 三者都不改变技术标识层(`identifier` 仍为 `com.sigma-file-manager.app`),
> 改名只发生在显示层与产物名。

---

## 5. 分阶段计划

每阶段独立可验证、独立可回滚(§3 硬约束 6)。**建议一次只推进一个阶段。**

### P0 · 决策固化(不写代码)
- 选定产品名
- 确认 identifier 策略(建议:不变)
- 确认版本号规则:建议 `<产品名>-<产品版本>-setup.exe`,产品版本独立于 upstream 版本
- 确认「发布分支」命名

**验证**:一份写死的决策记录。**回滚**:无(不改代码)。

### P1 · 主仓瘦身(对应要求 1)
- 移出 `release/Sigma-File-Manager-*.setup.exe`(6 个)+ `release/installer/`(2 个)
  → 建议**直接删除**,它们可由 fork 仓重新构建产出,且当前均未入库
- 决定 `kizemo.focus-sync-0.5.8-setup.exe` 是否入库(建议:入库,它是你实测通过的那个)
- 清理 `kizemo.focus-sync-0.2.0-setup.exe`(废弃)
- **结掉 handoff §5-7 悬案**:`focus-sync-sidecar-canonical.exe` 建议删除
  (已核实 `installer.nsi` 与 `tauri.conf.json` 都不引用它)
- 新增 `.gitignore` 规则,防止整合包再次混入

**验证**:`git ls-files release` 只剩插件相关;`release/` 目录里没有 `.exe` 大于 2 MB 的文件
**回滚**:纯文件移动,`git checkout` 即可

### P2 · fork 建立产品分支(对应要求 2)
- 从 `main` 开长期产品分支(如 `product/main` 或 `release/<name>-v1`)
- 确立规则:**`upstream/main` 的同步只进基座分支,产品分支只进自己的功能**
- 把 `release/` 加入 fork 的 `.gitignore`

**验证**:分支存在且基线可构建
**回滚**:删分支

### P3 · sync 载荷进入产品树(**架构决策点,需你选**)

| 方案 | 做法 | 成本 | 风险 |
|---|---|---|---|
| **A** | 维持现状(跨仓 `../../` 引用),只补文档与门禁 | 最低,0 改动 | 耦合仍是隐式的,两仓位置错了不报错 |
| **B** ⭐ | 主仓构建脚本先把 8 个载荷 stage 进 fork 内部目录,`tauri.conf.json` 改指 fork 内部路径 | 中,需改 `build-with-sidecar.ps1` + `tauri.conf.json` | 改了已验证配置 ⇒ **必须重新构建 + 重跑验收门禁** |
| **C** | fork 内建 `extensions/kizemo.focus-sync/` 目录,由主仓脚本同步过去 | 中高 | 两仓间产生同步问题,与「插件归主仓」的边界冲突 |

**推荐 B**:它让跨仓耦合从「靠 `../../` 的隐式约定」变成「脚本里可见的一步」,
并且可以复用已有的 `build-with-sidecar.ps1`(它本来就在做 staging + 沙箱门禁)。

**验证**:stage 后 `tauri build` 成功 + `verify-deploy-sha.ps1` PASS
**回滚**:恢复 `tauri.conf.json` 与构建脚本

### P4 · 改名(只改 productName)
- 改 fork `tauri.conf.json` 的 `productName`
- 改 NSIS 产物命名规则
- **不动 `identifier`**
- 同步更新 `verify-deploy-sha.ps1` 中的**显示名**引用(若有任何)

**验证**:构建产物文件名正确 + **安装后 `verify-deploy-sha.ps1` 仍 PASS**
(这一条是判别性检查:如果 identifier 被误改,门禁会立刻失败)
**回滚**:单文件改动

### P5 · 门禁与文档
- 加门禁:两仓必须并列(检查 8 个载荷路径可达)
- 加门禁:主仓 `release/` 不得出现 Sigma FM 整合包
- 更新主仓 README「仓库边界」一节 + fork README
- 明确「哪些功能可插件化 / 哪些必须侵入」的判定表

**验证**:故意制造一个错误路径,门禁必须 FAIL(判别性测试,防"平凡地绿")
**回滚**:纯新增

### P6 · 端到端重新验证
- 构建整合包 → 安装 → `verify-deploy-sha.ps1`
- 顺带结掉 handoff §5-4(**卸载路径从未端到端验证**)
- 更新 handoff §5,关闭已完成项

**验证**:sha 门禁 exit 0 + 卸载后 `KizemoFocusSync` 任务消失、扩展目录清空

---

## 6. 需要你决策的点

1. **产品名**选 A / B / C,还是另起?
2. **`identifier` 改不改?** 我建议不改(理由见 §3.2)。若你坚持改,P4 成本会从"低"升到"极高",且 §1 验收门禁要重写。
3. **P3 选哪个方案?** 我推荐 B。
4. **「独立目录树」的目标定到哪一档?**
   - 档位一(建议,可达成):可插件化部分独立 + 侵入部分用分支隔离
   - 档位二(不可行):基座零改动的纯净 overlay —— Sigma 扩展 API 不支持
5. **整合包旧文件**:删除,还是迁到 fork 的 `release/` 保留?

---

## 7. 我测了什么 / 没测什么

**测了**(可复现,均有命令输出支撑):
- 主仓 `release/` 的完整内容清单、体积、跟踪状态
- fork 相对 upstream 的提交数(28)与文件分布(63 个 / 55 个在 `src/`)
- fork 内**不存在** focus-sync 源码(全仓搜索仅命中 `target/` 构建残留与 `tauri.conf.json`)
- 扩展 API 的 `contributes` 允许的键:**无布局插槽**(这是 §3.1 的依据)
- `productName` / `identifier` / license 的实际取值,以及 identifier 的引用面(主仓 58 文件 / fork 6 文件)
- fork `.gitignore` 未覆盖 `release/`

**没测**:
- ❌ **没有跑过任何构建**。P3 各方案的可行性基于配置与路径解析的静态分析,未实测
- ❌ **没有实测改 `productName` 后的安装/升级行为**。§3.2 中「现有用户升级路径不受影响」
  是**基于 PREINSTALL 钩子读注册表 `InstallLocation` 的推断**,未验证
- ❌ **没有验证主仓 `release/` 里的整合包能否由 fork 仓完整重建**(删除前需确认可重建)
- ❌ 未评估 GitHub 仓库改名/域名可用性(需联网核实)

**git 状态**:本轮**未提交、未推送、未切分支、未改任何被跟踪文件**。
