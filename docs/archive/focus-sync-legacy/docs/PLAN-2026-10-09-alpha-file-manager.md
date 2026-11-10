# Alpha File Manager —— 双仓重构与品牌化完整方案(v2)

**时间**:2026-10-09 10:50
**性质**:**规划文档。本轮未修改任何被跟踪文件,未提交、未推送、未切分支。**
**基线**:主仓 `dc5f152` / fork `4365281b`(均未推送)
**取代**:`plan-2026-10-09-dual-repo-restructure.md`(本文为权威版本,已按你的反馈重写)

---

## ★ 决策已锁定(2026-10-09 11:15)

| # | 事项 | 决定 |
|---|---|---|
| 1 | 图标 | **候选 A** —— `afm-v3-2-solid-teal.jpg`(实心 α + 钢青灰文件夹) |
| 2 | P3 载荷入库 | **方案 B** —— 主仓构建脚本 stage 进 fork 内部路径 |
| 3 | 旧整合包 | **直接删除**(不迁移到 fork 保留) |

本文已按此三项决策更新,§8「待确认」清空。**剩余唯一前置:P1 删除前的可重建性验证。**

---

## 0. 结论

1. **树形视图:我上一版说错了,已更正。** 它不仅完成了,而且**大部分代码是可以搬进独立目录的** ——
   55 个文件里 10 个是新增、47 个是修改,其中 18 个只是 i18n 语言包。核心逻辑是全新的,
   只有约 29 处 1–14 行的薄集成点挂在 upstream 文件上。**不需要返工。**

2. **改名 Alpha File Manager 可以走低成本路径。** 改 `productName` + 窗口标题,
   **不动 `identifier`**。实测需改 10–12 处显示位;**有 2 个互操作常量绝对不能碰**。

3. **图标已选定候选 A**(`afm-v3-2-solid-teal.jpg`)。但要如实说明:多模态模型在第 1、2 轮
   都画错了 α 的字形,第 3 轮改成「实心字形」描述后才拿到正确的 α。过程与失败记录见 §4。

4. **sync 插件仓全程不被破坏。** §6 给出逐文件的「可删 / 必保」清单。

5. **跨仓路径依赖已由构建产物直接证实**(不是路径推算):上一次构建生成的
   `src-tauri/target/release/nsis/x64/installer.nsi` 里,资源 `File` 指令的源路径是
   `…\sigma-file-manager\src-tauri\..\..\release\extension\…`
   ⇒ 解析基准 = `<fork>/src-tauri/`,目标落在**主仓**。这为 P3 的改造提供了确切基线。

---

## 1. 更正:关于侧栏树形视图

### 1.1 我没有质疑它,是我表述错了

我上一版的表格写的是「侧栏树形视图 ❌ **不能** 独立成目录」。
这句话有两个问题:

- **它读起来像在说这个功能没做完** —— 我没有这个意思,你已经装包验证通过了,我也没有任何
  证据说它有问题。
- **「不能」是绝对判断,这个判断下得太重。** 我当时的依据只是「Sigma 扩展的 `contributes`
  没有布局插槽」,由此推断 UI 功能必须侵入。但**侵入 ≠ 不能整理**。

### 1.2 实测数据(这是更正的事实基础)

`git diff --name-status upstream/main...8caf14ae`(树形视图分支):

| 分类 | 数量 | 说明 |
|---|---|---|
| **新增文件(A)** | **10** | 全部是我们自己的代码 |
| **修改 upstream 已有文件(M)** | **47** | 见下细分 |
| ├ i18n 语言包 | **18** | 只是各语言加一个 `tree` 标签 |
| └ 代码集成点 | **约 29** | **每个仅 1–14 行**,做参数透传 |
| 合计 | 55 | 2023 增 / 117 删 |

核心逻辑(全部是**新增**,天然可搬):

```
src/modules/navigator/composables/use-file-tree.ts              359 行  ← 树数据/懒加载
src/components/file-browser/file-browser-tree-view.vue           233 行  ← 树渲染
src/stores/runtime/folder-tree.ts                                174 行  ← Pinia store
src/modules/navigator/types/file-tree.ts                          26 行
```

最大的单点侵入是 `navigator.vue`(+245 行),其余集成点都是薄透传。

### 1.3 修正后的结论

> **树形视图可以做到「自有代码集中在专属目录 + 集成点降到最小且被登记」。**
> 做不到的是「基座零改动」—— 因为 Sigma 没有布局扩展插槽,`navigator.vue` 那一类接线
> 在当前 API 下消不掉。

**所以计划里这一项从「不可行」改为「可整理」**,新增阶段 P2.5:把 10 个自有文件迁入
`src/modules/navigator/tree/`,并生成一份**集成点登记表**(文件:行号 → 改了什么),
让将来同步 upstream 时,冲突面一目了然。

**你的树形视图代码本身不需要任何返工。**

---

## 2. 改名:Alpha File Manager(低成本路径)

### 2.1 核心决策:改 `productName`,不动 `identifier`

```
productName : Sigma File Manager  →  Alpha File Manager      ← 改(显示层)
identifier  : com.sigma-file-manager.app  →  不动           ← 技术契约层
```

`identifier` 决定插件安装路径、注册表键、NSIS 卸载注册表项,**并且是 §1 验收门禁
`verify-deploy-sha.ps1` 的比对锚点**。改它 ⇒ 全新安装路径 ⇒ 要重装、重注册、重建计划任务、
重写门禁锚点。这就是「低成本」与「高成本」的分界线。

### 2.2 实测:需要改的显示位(逐项)

| # | 文件 | 位置 | 改什么 |
|---|---|---|---|
| 1 | `src-tauri/tauri.conf.json` | L3 | `productName` |
| 2 | `src-tauri/tauri.conf.json` | L16 | 窗口 `title` |
| 3 | `index.html` | `<title>` | 窗口标题 |
| 4 | `src-tauri/Cargo.toml` | L3 | `description`(纯描述,可改) |
| 5–11 | `src/utils/auxiliary-windows.ts`<br>`src/stores/runtime/extensions.ts`<br>`src/stores/runtime/quick-view.ts`<br>`src/modules/print-view/pages/print-view.vue`<br>`src/modules/quick-view/pages/quick-view.vue`<br>`src/utils/module-load-recovery.constants.ts` | — | 辅助窗口标题 / 扩展显示名 |
| 12 | NSIS 产物命名 | 构建配置 | `Alpha-File-Manager-<ver>-setup.exe` |

### 2.3 ⚠️ 两个绝对不能改的互操作常量

| 文件:行 | 常量 | 为什么不能改 |
|---|---|---|
| `src/lan_share/types.rs:15` | `MDNS_INSTANCE_NAME = "Sigma File Manager"` | **mDNS 网络发现的服务名**。改了会让 LAN 共享在其他设备上搜不到本机 |
| `src/windows_installation.rs:30` | `LEGACY_DEFAULT_FILE_MANAGER_DATA_DIR_NAME` | **接管 Windows 默认文件管理器的数据目录名**。改了会破坏「设为默认程序」的兼容性 |

另有一处 `default_file_manager_launcher/src/launcher.rs:149` 用作**临时目录名**,
改不改都行,建议**不动**(无收益)。

> 这两处与「品牌」无关,属于**对外互操作契约**。这是本节最容易被漏掉的地方 ——
> 全局替换 `Sigma File Manager` 会把这两个一起改掉,而且不会报错,只会静默失效。

### 2.4 改名的连锁影响与验证

| 影响 | 判定 |
|---|---|
| 全新安装的默认目录 | 从 `…\Sigma File Manager\` 变成 `…\Alpha File Manager\`。**符合预期** |
| 现有用户升级 | PREINSTALL 钩子读注册表 `InstallLocation` 复用旧位置 ⇒ **推断不受影响,未实测** |
| 插件安装路径 | **完全不变**(由 identifier 决定)⇒ 升级不丢插件 |
| 计划任务 `KizemoFocusSync` | 不变 |
| 验收门禁 | `verify-deploy-sha.ps1` 不需要改 ⇒ **可作为改名是否误伤的判别性检查** |

**判别性验证(防「平凡地绿」)**:改名后若 `identifier` 被误改,门禁会**立即 FAIL**。
若门禁仍 PASS,才说明改对了。**「门禁 PASS」本身不是证据 —— 必须确认 identifier 确实没被动过。**

---

## 3. 品牌与命名

### 3.1 Alpha File Manager 与 Sigma 的对应关系

| | Sigma File Manager | Alpha File Manager |
|---|---|---|
| 字母 | Σ(Sigma,求和) | α(Alpha,第一/首位) |
| 定位 | 上游基座 | 二次开发产品 |
| 关系 | — | **明确的衍生关系,字母表上紧邻且同为希腊字母** |

语义上也自洽:σ 是「求和」(把文件聚在一起),α 是「首位/最初」。

### 3.2 GPL-3 合规(必做)

fork 许可证为 **GPL-3.0-or-later**,`LICENSE.md` 明文允许 rebrand。改名后**必须**:

- 以 GPL-3 分发,附完整许可证与源码获取方式
- 保留上游版权声明
- 在 README / 首次运行说明中声明**本产品与上游无官方关联**
- 不得使用 Sigma 的商标、logo 或暗示官方背书的措辞
- **图标不得是 Sigma 图标的像素级复制**(见 §4,三个候选均为独立设计)

主仓 focus-sync 为 MIT,MIT→GPL-3 组合法,组合后整体按 GPL-3 分发。
建议把主仓 `LICENSE` 与产品实际分发条款对齐,避免混淆。

---

## 4. 图标:3 个候选

### 4.1 生成过程(含失败记录)

我必须如实说明,因为这直接影响到你能信多少:

| 轮次 | 做法 | 结果 |
|---|---|---|
| 第 1 轮(3 张) | 常规描述「文件夹 + α」 | ❌ **全部失败**。α 被画成带小旗的拉丁式 "ɑ";#3 还多了一条横杠和闪电状伪影;还带棋盘格透明底 |
| 第 2 轮(3 张) | 改为**几何化描述**(环 + 右上斜笔画 + 尾钩),并明确「不要横杠/不要顶旗/不要长降部」 | ⚠️ **1 张成功**(#1 正确)、**1 张可接受**(#2 顶部有小缺口)、**1 张失败**(#3 笔画穿透圆环,读作 ♂) |
| 第 3 轮(2 张) | 找到根因:我前两轮写的是「thick even weight **strokes**」,把模型引向了**描边/环形**构造;而 Sigma 的图标用的是**实心白色字形**。改写为「SOLID FILLED letterform,不是 outline」 | ✅ **2 张全部正确** |

**根因值得记下**:模型对「字形」的渲染不可靠,但对「实心色块」的渲染很可靠。
以后任何需要准确字形的资产,都应该直接要求实心字形,而不是描述笔画。

### 4.2 三个候选与选定结果(均在 `docs/brand/icon-candidates/`)

| # | 文件 | 构造 | 配色 | 评价 |
|---|---|---|---|---|
| **A** ✅ **已选** | `afm-v3-2-solid-teal.jpg` | **实心 α**,与 Sigma 的实心 Σ 完全同构 | 钢青灰文件夹 + 浅底 | **最贴近「与 Sigma 对应」** —— 同一套视觉语法,只换了字母。结构最干净,32px 下可读 |
| B | `afm-v3-3-solid-indigo.jpg` | 实心 α | 靛紫文件夹 + 深底 | 与 A 同构但配色完全不同,辨识度最高;深色环境下更突出 |
| C | `afm-v2-1-steel.png` | **描边式**(环 + 尾) | 钢青灰 + 浅底 | 构造与 A/B 不同,提供视觉多样性;但描边在小尺寸下更易糊 |

> B、C 保留在候选目录中,若 A 落地后在真实任务栏/资源管理器里辨识度不足,可直接回退。

被淘汰的 4 张(`alpha-icon-1/2/3`、`afm-v2-3-flat.png`)**仍留在同一目录**,
文件名保持原样。我尝试过把它们移到 `rejected/` 子目录,但被本机安全策略拦截。
**请以本表的 ✅ 标记为准,不要按文件名或排列顺序判断。**

### 4.3 落地步骤(选定后执行)

前置:候选图是 **2048×2048**,而 `tauri icon` 需要 **1024×1024**。

```powershell
cd sigma-file-manager

# 1. 裁到 1024x1024(保持居中,alpha 字母约占画面 45%)
# 2. 处理 Windows 图标遮罩:浅色底可保留;若要透明底需去除背景
# 3. 生成全套图标
npx tauri icon <裁好的 1024x1024 PNG>
```

`tauri icon` 会一次性生成 `icon.ico` / `icon.icns` / 各尺寸 PNG / Windows Store 方形图,
**与现有 `src-tauri/icons/` 的文件清单完全对应**,无需手工增删。

⚠️ 落地后必须 `git status` 核对 `src-tauri/icons/` 的变更清单,
确认没有意外删除现有图标(回滚点:`git checkout src-tauri/icons/`)。

---

## 5. 双仓目标架构

```
┌─ 主仓 filemanager = Focus Sync 插件项目(独立) ──────────┐
│  sigma-listary-spike/        sidecar 源码                 │
│  release/extension/          插件本体  ★受保护            │
│  release/extension-installer/ 插件独立 NSIS + 9 个 PS ★受保护│
│  release/kizemo.focus-sync-*.exe   只放 sync 独立包        │
│  scripts/  docs/             门禁 + 文档 ★受保护           │
│  ★ 永不出现 Sigma FM / Alpha FM 整合包                     │
└──────────────────────┬──────────────────────────────────┘
                       │ 构建期 stage(必须显式化 + 有门禁)
                       ▼
┌─ fork sigma-file-manager = Alpha File Manager 产品仓 ────┐
│  upstream 基座 + 28 commit 偏离                            │
│  src/modules/navigator/tree/   树形视图自有代码(新迁入)     │
│  src/modules/*/…                其余侵入式集成点(登记在册) │
│  release/Alpha-File-Manager-<ver>-setup.exe   整合安装包   │
└──────────────────────────────────────────────────────────┘
```

---

## 6. sync 插件仓的「必保 / 可删」清单

这一节直接回答「不要破坏这个项目在本项目目录中的文件」。

### 6.1 绝对保护(任何阶段都不得删改)

| 路径 | 为什么 |
|---|---|
| `release/extension/package.json`、`dist/`、`locales/` | 插件本体,`bundle.resources` 的输入 |
| `release/extension/bin/focus-sync-sidecar.exe` | 插件侧 sidecar 二进制,打包输入 |
| `release/extension-installer/installer.nsi` + 9 个 `.ps1` + `README.md` | 插件独立安装器全部源码 |
| `scripts/*.cjs`、`scripts/*.ps1` | 沙箱门禁、部署 sha 门禁、日志读取 |
| `sigma-listary-spike/` | sidecar Rust 源码 |
| `docs/` | 全部文档与 handoff |

### 6.2 可删(对应你的要求 1)

| 路径 | 体积 | 理由 |
|---|---|---|
| `release/Sigma-File-Manager-*.setup.exe` × 6 | **16.7 MB × 6** | 整合包不属于插件项目,应由 fork 仓产出 |
| `release/installer/`(2 个上游安装包) | **16.5 MB × 2** | 同上 |
| `release/kizemo.focus-sync-0.2.0-setup.exe` | 821 KB | 废弃旧包,反而被误入库了 |
| `release/extension/bin/focus-sync-sidecar-canonical.exe` | 2.5 MB | 已核实 `installer.nsi` 与 `tauri.conf.json` **均不引用**;结掉 handoff §5-7 悬案 |

### 6.3 应当入库(修复版本控制倒挂)

| 路径 | 理由 |
|---|---|
| `release/kizemo.focus-sync-0.5.8-setup.exe` | **你实测通过的那个包,现在没入库**;反而废弃的 0.2.0 入库了 |

### 6.4 需注意的连带影响

删除 `release/` 里的整合包**不影响** fork 仓的构建能力 —— 那些包本来就是 fork 仓
`npm run tauri build` 的产物,删掉可以重新生成。
但 ⚠️ **这一点我没有实测**:删除前建议先执行一次 fork 构建,确认能复现,再删。

---

## 7. 分阶段计划

每阶段独立可验证、独立可回滚(硬约束 6)。**一次只推进一个阶段。**

### P0 · 决策固化(不写代码)
- 确认产品名 **Alpha File Manager**
- 确认 `identifier` **不变**
- 选定图标 A / B / C
- 确认产品版本号规则(建议独立于 upstream 版本)

**验证**:写死的决策记录 · **回滚**:无

### P1 · 主仓瘦身(对应要求 1,删除已确认)
1. **【硬前置】先验证可重建**:在 fork 仓执行一次 `npm run tauri build`(Rust 1.90),
   确认整合包能产出。**这一步不做完不许删** —— 这些 `.exe` 都未被 git 跟踪,
   删掉后 `git clean` 也救不回来
2. 删除 §6.2 的 4 类文件(**直接删除,不迁移到 fork 保留**)
3. 把 0.5.8 独立包纳入版本控制(§6.3)
4. 修 `.gitignore`,防止整合包再次混入(建议加 `release/Sigma-File-Manager-*.exe` 与 `release/installer/`)

**验证**:`git ls-files release` 只剩插件相关;§6.1 清单逐项仍在
**回滚**:第 2 步删除的文件未被跟踪 ⇒ **只能靠第 1 步重新构建恢复**

### P2 · fork 建立产品分支(对应要求 2)
- 从 `main` 开长期产品分支(如 `product/alpha-fm`)
- 规则:`upstream/main` 的同步只进基座分支,产品功能只进产品分支
- fork `.gitignore` 加 `release/`

**验证**:分支存在且基线可构建 · **回滚**:删分支

### P2.5 · 树形视图代码归位(**本次新增,来自 §1 的更正**)
- 把 10 个自有文件迁入 `src/modules/navigator/tree/`
- 生成**集成点登记表**:文件:行号 → 改动摘要(约 29 处)
- **只做搬迁,不改逻辑** —— 功能已完成且验证通过,任何逻辑改动都是引入风险

**验证**:搬迁后 `npx vue-tsc` / 构建通过;树形视图功能手动回归一次
**回滚**:纯文件移动

### P3 · sync 载荷 stage 进 fork(方案 B,已选定)

**目标**:把跨仓耦合从「靠 `../../` 的隐式约定」变成「脚本里可见的一步」。

**基线事实**(来自构建产物 `src-tauri/target/release/nsis/x64/installer.nsi`):

```
File /a "/oname=tools\focus-sync-sidecar.exe" "…\sigma-file-manager\src-tauri\..\..\release\extension\bin\focus-sync-sidecar.exe"
```

⇒ 现状:`bundle.resources` 的源路径经 `<fork>/src-tauri/` 解析后**指向主仓**。

**改造后**:源路径改为 fork 内部,复制动作由脚本完成。

**① 新增 staging 目录(在 fork 仓根,不在 src-tauri 内)**

```
sigma-file-manager/afm-staging/          ← 必须进 .gitignore
├── extension/{package.json, dist/, locales/}
├── installer/{register.ps1, resolve_user_appdata.ps1,
│              register-scheduled-task.ps1, unregister-scheduled-task.ps1}
└── bin/focus-sync-sidecar.exe
```

> ⚠️ **必须 gitignore。** 一旦提交,等于把插件二进制复制进 fork 仓,
> 仓库边界(README「仓库边界」一节)就作废了。stage 目录是**产物,不是源码**。

**② 改 `tauri.conf.json` 的 `bundle.resources`**(源路径改指 fork 内部)

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

**右侧目标路径完全不变** ⇒ hooks.nsh 的 POSTINSTALL 逻辑一行都不用改。

**③ 改主仓的 `sigma-file-manager/scripts/build-with-sidecar.ps1`**
(该脚本已存在,本来就在做 staging + 沙箱门禁,直接扩展):

- 构建 sidecar(沿用现有逻辑,含沙箱门禁)
- **新增:把 8 项载荷从主仓 `release/` 复制到 `<fork>/afm-staging/`**
- **复制前逐项校验源存在且 sha 非空** —— 缺一项就中止,不留半份 stage

**④ 两仓隔离红线(本阶段最重要的一条)**

> stage 操作**只允许「读主仓、写 fork」**。
> 绝不允许移动、重命名、修改或删除主仓 §6.1「绝对保护」清单里的任何文件。
> 主仓对 fork 的依赖方向永远是**单向只读**。

**验证**(三条,缺一不可):
1. 跑一次构建,检查**新生成的** `target/release/nsis/x64/installer.nsi`,
   `File` 指令的源路径应指向 `…\sigma-file-manager\afm-staging\…`
2. `verify-deploy-sha.ps1` **PASS**(判别性:载荷字节必须与主仓一致)
3. `git status` 确认 `afm-staging/` **未出现在待提交列表**(证明 gitignore 生效)

**回滚**:恢复 `tauri.conf.json` 与构建脚本两处;`afm-staging/` 直接删目录即可。

### P4 · 改名(只改 `productName`)
- 改 §2.2 的 12 处显示位
- **不动 §2.3 的两个互操作常量**
- 产物名改 `Alpha-File-Manager-<ver>-setup.exe`

**验证**:
- `git diff` 确认 `lan_share/types.rs` 与 `windows_installation.rs` **未被改动**
- 确认 `identifier` 仍是 `com.sigma-file-manager.app`
- 构建 + 安装 + `verify-deploy-sha.ps1` PASS(**判别性**:误改 identifier 必 FAIL)

### P5 · 图标落地(候选 A 已定)
- 把 `afm-v3-2-solid-teal.jpg` 裁到 1024×1024,处理 Windows 图标遮罩
- `npx tauri icon`(细节见 §4.3)

**验证**:构建后任务栏 / 开始菜单 / 资源管理器图标正确;`git status` 核对 `src-tauri/icons/` 变更清单
**回滚**:`git checkout src-tauri/icons/`

### P6 · 门禁与文档
- 门禁:两仓必须并列(8 个载荷路径可达)
- 门禁:主仓 `release/` 不得出现整合包
- 门禁:改名后 `identifier` 必须是原值(防手滑)
- 更新主仓 README「仓库边界」+ fork README + GPL 声明

**验证**:**判别性测试** —— 故意制造错误路径,门禁必须 FAIL(防「平凡地绿」)

### P7 · 端到端重验
- 构建 → 安装 → `verify-deploy-sha.ps1` → 卸载
- 顺带结掉 handoff §5-4(**卸载路径从未端到端验证**)
- 更新 handoff §5

**验证**:sha 门禁 exit 0;卸载后计划任务消失、扩展目录清空

---

## 8. 决策状态

| # | 事项 | 状态 | 决定 |
|---|---|---|---|
| 1 | 产品名 | ✅ 已定 | **Alpha File Manager** |
| 2 | `identifier` | ✅ 已定 | **保持 `com.sigma-file-manager.app` 不变** |
| 3 | 图标 | ✅ 已定 | **候选 A**(`afm-v3-2-solid-teal.jpg`) |
| 4 | P3 载荷入库 | ✅ 已定 | **方案 B**(stage 进 fork 内部路径) |
| 5 | 旧整合包 | ✅ 已定 | **直接删除**,不迁移保留 |
| 6 | §2.3 两个互操作常量 | ✅ 已定 | **保持不动**(`lan_share/types.rs:15`、`windows_installation.rs:30`) |

**执行顺序建议**:P0(已由上述决策完成)→ **P1** → P2 → P2.5 → P3 → P4 → P5 → P6 → P7。

**唯一未决前置**:P1 第 1 步的可重建性验证。**它没通过之前,不要执行任何删除。**

---

## 9. 我测了什么 / 没测什么

**测了**(可复现):
- 树形视图 55 个文件的 A/M 分布(新增 10 / 修改 47,i18n 18 / 代码集成点约 29)
- 扩展 API `contributes` 的允许键:**无布局插槽**
- `productName` / `identifier` / license 实际取值
- `"Sigma File Manager"` 在 `src-tauri` 的 8 处、`src/`(排除 i18n)的 7 个文件
- 两个互操作常量的位置与语义(`lan_share/types.rs:15`、`windows_installation.rs:30`)
- Sigma 现有图标(已读图确认「文件夹 + 实心白 Σ」视觉语法)
- 3 轮图标生成的失败与成功记录(每张都已实际读图核验字形)
- **`bundle.resources` 的路径解析基准**:读构建产物
  `src-tauri/target/release/nsis/x64/installer.nsi`(2026-10-09 07:40 生成),
  其 `File` 指令源路径为 `…\sigma-file-manager\src-tauri\..\..\release\extension\…`
  ⇒ 基准 = `<fork>/src-tauri/`,目标在主仓。**这是构建产物本身,不是我的路径推算。**

**没测 / 未验证**:
- ❌ **没有跑过任何构建**。P1/P3/P4 的可行性基于静态分析
- ❌ **没有验证整合包可由 fork 完整重建** —— 这是 P1 删除前的硬前置,必须先做
- ❌ **没有实测改 `productName` 后现有用户的升级行为**(§2.4 表格中标注「推断」处)
- ❌ **没有把候选 A 裁成 1024×1024、没有跑 `tauri icon`**
- ❌ **没有验证 LAN 共享 / 默认文件管理器接管功能**在改名后的实际表现
  (我只能保证常量没被改,不能保证这两个功能本来就正常)
- ❌ 没有实测新 `bundle.resources` 路径(`../afm-staging/…`)能否被 Tauri 正确解析
  —— 现有 8 条用的是 `../../`,新方案层级不同,**必须在 P3 首次构建时确认**
- ❌ 未核实 GitHub 仓库改名与域名可用性

**git 状态**:本轮未提交、未推送、未切分支;两仓被跟踪文件**零改动**。
新增内容仅为本文档与 `docs/brand/icon-candidates/` 下的候选图(均为未跟踪新文件)。
