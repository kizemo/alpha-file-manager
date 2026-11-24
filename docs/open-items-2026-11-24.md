# 未完成事项台账（截至 2026-11-24，批次 43 / 3.21.1 收口后）

> **作用**：把散落在各批次 handoff / plan 里的「还没做完的事」汇成一张可执行清单，避免再靠记忆翻十份文档。
> **口径**：`OPEN` = 仍开着、可直接开工；`BLOCKED` = 等外部条件（真实凭据 / 上游服务 / 用户本人操作）；`DEFERRED` = 用户主动延期，不算遗漏；`CLOSED` = 已闭合。
> **本轮依据**：批次 43 第二轮收口（用户 3.21.1 手工复验通过）。详见 `docs/handoff-2026-11-22-batch38-onedrive-readonly.md` §十五。
> **最近更新**：2026-11-24

---

## 0. 一句话结论

批次 43 的用户可见缺陷**已全部闭合**（3.21.1 已装机、用户已手工复验通过）。

仍然开着 **6 项**（其中 3 项是我这边可直接动手的，3 项需要授权或用户本人操作），
另外 **6 项**属于等外部条件的阻塞项、**1 条**是用户主动延期的线（OneDrive）、**3 项**是已明确不再做的。

---

## 1. 本轮已闭合

| 项 | 结论 | 证据 |
| --- | --- | --- |
| 「正在索引云盘」三行同时亮 | 已修 + 用户复验通过 | commit `3dbcf57d`；`src/composables/use-remote-global-search-index.ts`；handoff §十四·2 |
| 取消勾选后状态不回落 | 已修 + 用户复验通过 | 同上：未勾选行不再渲染重建按钮，文案按「本轮重建实际包含的连接」判定 |
| 勾选是否真正控制「是否被检索」 | **代码级闭环 + 用户正向实测**（勾百度可搜到，夸克/阿里未同步开启） | handoff §十四·3；`src-tauri/src/global_search/commands.rs:74-118`；`src/stores/storage/remote-connections.ts:195-196` |
| 批次 43 四项网盘交互需求 | 已在 **3.21.0** 全部落地（检索默认勾选回归 / 搜索页按类型分组 / 删除闪黑框 / 按钮选中态） | commit `c97644a4`；`docs/plan-2026-11-24-batch43-netdisk-ux.md` §1–§4 |
| 门禁 | `npm run check` 全绿（`Test Files 333 passed` / `Tests 3588 passed`） | `.superpowers/sdd/b43/check-b43-3.log` |
| 打包 + 装机 | `release/Alpha File Manager_3.21.1_x64-setup.exe`；已装到 `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，`ProductVersion = 3.21.1` | `.superpowers/sdd/b43/install-status.txt`、`.superpowers/sdd/b43/build-status.txt` |

---

## 2. OPEN —— 仍开着，可直接开工

### O-1（最高优先）索引条目不记「它属于哪个网盘」，取消勾选在**失败路径**下不会立刻生效

- **现象**：正常路径没问题（用户已实测：只勾百度，夸克/阿里不会被索引）。
  但如果**重建中途失败**，旧索引条目还在，而查询侧无法按连接过滤 ⇒ 已被取消勾选的网盘**仍可能被搜到**。
- **根因**：`RemoteIndexDoc`（`src-tauri/src/global_search/remote_index.rs:138-209`）只存
  `path / name / is_dir / size / modified_time`，**没有连接维度**；全仓 `global_search` 模块不读
  `include_in_global_search` ⇒ 查询侧没有第二条过滤路径。
- **本轮缓解**：设置页进页面时自动补跑一次重建（`markReady()` 自愈），覆盖「上次失败」这一类残留。
- **彻底修法**：索引条目带上 `connection_id`，查询时按当前勾选集合过滤。
- **代价**：改索引 schema + 全量重建 + 一次小版本发版；**不涉及 17 个语言包**。
- **来源**：handoff §十四·3、§十四·7 第 1 条。

### O-2 自愈补跑在跨页切换时可能多跑一次

- 两个设置页的 `onMounted` 都会调 `markReady()`；`pendingRemoteIndexIds` 的合并只覆盖「正在跑」的期间，
  不覆盖「已 drain 完 + 失败」之后的两次进页 ⇒ 可能多跑一次重建（多一次网盘 API 调用）。
- **代价**：一次多余调用；**不修也不影响正确性**。
- **来源**：handoff §十四·7 第 2 条。

### O-3 搜索页模板没有自动化测试

- `src/modules/settings/ui/categories/search/global-search.vue` **没有单测文件**
  （同目录 `__tests__` 不存在）⇒ 本轮改的模板逻辑（按连接判文案、未勾选不渲染按钮）只有「整仓门禁 + 真机截图」两层证据，
  没有组件级回归网。
- **来源**：handoff §十四·2、§十四·7 第 3 条。

### O-4 界面级取证路线仍不通（长期卡住真机验收）

- 本项目拿不到 webview console（wry 不透传 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`，CDP 连不上），
  UIA 也抓不到 `data-test` ⇒ 界面级结论只能靠用户手工截图。
- **下一条可选出路**：临时改 `AdditionalBrowserArguments` 打一个**验收专用包**（不进 `release/`）。
- **来源**：handoff §十三·5、§十四·7 第 4 条。

### O-5 三个提交未推送（需要授权）

`git log origin/main..HEAD`：

| 提交 | 内容 |
| --- | --- |
| `1b6c2ba0` | OneDrive 支持整条线延期（界面下线，代码全留） |
| `c97644a4` | 批次 43 四项网盘交互收口（升版 3.21.0） |
| `3dbcf57d` | 修索引状态串台 + 核查勾选语义（升版 3.21.1） |

- 远端 `origin = https://github.com/kizemo/alpha-file-manager.git`，本地 `main` 领先 3 个提交。
- **推送属对外动作，需用户明确授权**（本仓一贯口径）。

### O-6 `reports/` 残留目录未跟踪

- 仓库根的 `reports/` 有 4 个文件（`patch-accept-real-device.cjs`、`patch-accept-real-device2.cjs`、
  `probe-readhost.cmd`、`task56-process-verify.txt`），是批次 38/41 的探针残留，
  `git status` 显示 `?? reports/` ⇒ **既没进版本库、也没被 gitignore 覆盖**。
- **处置**：归档到 `.superpowers/sdd/`（该目录已被忽略）或补一条 ignore 规则，二选一即可。

---

## 3. BLOCKED —— 等外部条件，不能单方推进

| # | 事项 | 等什么 | 来源 |
| --- | --- | --- | --- |
| B-1 | **取消勾选后旧索引条目是否真的从搜索结果消失**、多连接混合检索的排序 —— 真机未跑 | 真实网盘凭据（用户本机已有三条真实连接，条件已具备）+ 用户手工操作 | `docs/handoff-2026-11-22-batch37-cloud-search-favorite-alias.md` §7-2；与 O-1 同一根因 |
| B-2 | **阿里云盘上游 HTTP 400**（批次 35 遗留，未触碰） | 上游恢复；恢复后重跑 `.superpowers\sdd\b32\b32-accept-remote-ui.ps1` 取一份 `errors` 为空的干净报告 | `docs/handoff-2026-11-21-batch35-remote-thumbnail-delivery.md` §7-3；batch37 §7-1/§7-3 |
| B-3 | 17 个语言包**母语复核** | 母语者本人 | `docs/handoff-2026-11-13-quark-pan-batch13.md` §「未纳入本轮」R4 |
| B-4 | `Ctrl+Shift+P` 在**未被其他程序占用**的机器上复验 | 一台干净机器（本机该键被外部程序以 OS 级 `RegisterHotKey` 占用，非 AFM 缺陷；3.13.7 已加冲突检测徽标） | `docs/handoff-2026-11-11-p5b-shortcut-conflict-detection.md`；batch13 R5 |
| B-5 | shell 扩展 `Classes\Drive` / `Classes\Directory` **未注册** ⇒ U 盘盘符双击能否进 AFM 从未验证 | 动系统注册表 + 定长数组 + 快照/回滚改造，属架构级改动，需用户拍板 | `docs/handoff-2026-11-11-quark-pan.md` §20.19.5-7 |
| B-6 | 焦点同步安装器 NSIS **STAGE 7 失败路径**未覆盖 | 需要构造失败场景 | `docs/handoff-2026-11-10-focus-sync-installer-cleanup.md` §末 |

---

## 4. DEFERRED —— 用户主动延期（不是遗漏，别再当待办追）

### OneDrive 支持整条线

- **现状**：界面已下线（`ONEDRIVE_ENABLED = false`），**代码一行没删**，恢复只需翻一个开关 + 回填 client_id。
- **恢复八步**：`docs/plan-2026-11-24-onedrive-deferral-resume.md` §4（R1 翻开关 → R8 发布形态核对）。
- **硬缺口**：合法的 Azure 应用（客户端）ID（GUID 形态）—— 唯一外部依赖。
- **恢复批次必须一起做的三件事**（同文档 §5）：
  1. **发布租户保活**：租户自计费起 200 天不活跃会被 block，20 天内可申诉、**超 20 天永久删除**；
     另有「每 60 天登录一次 `portal.azure.com`」的硬动作。
  2. **client_id 单点**：全体用户的 refresh_token 绑这一个 client_id，应用被删/停 = 全体掉线；
     需在同一租户再注册一个备用应用。
  3. **登录失败原因不透传**（批次 41 §十一·9 登记的非阻塞缺陷）：发布形态下只看到粗粒度错误
     ⇒ 建议恢复批次第一件事就修。
- **已知边界**：用户本地已存在的 OneDrive 连接**仍会出现在列表/侧边栏**（有意保留数据可见性）；
  要彻底看不见需额外做「按协议过滤列表」，本轮刻意不做。

---

## 5. 已明确不做（用户已拍板，不要再提）

| 项 | 裁决 | 依据 |
| --- | --- | --- |
| 需求① 的 B/C 形态（把「加入全局检索」勾选提前到类型选择一步） | 否决 | plan §6、handoff §十四·7 第 5 条 |
| 远程页编辑按钮 / 默认显示 provider 表单 | 用户 2026-11-24 撤回 | plan §7 开头 |
| 物理删除 OneDrive 代码 | 改为「集中开关下线，资产全留」 | `docs/plan-2026-11-24-onedrive-deferral-resume.md` §1 |
| 给 `RemoteIndexDoc` 加 connection 维度 | 本轮不做，登记为遗留 | plan §7.3、handoff §十四·7 第 1 条 |

---

## 6. 下一阶段目标（**非技术表述**）

### 一句话

**让「勾了哪个网盘，就只搜哪个网盘」这件事真正可靠，并让这套规则以后不会被改坏。**

### 拆成三件用户能看懂的事

1. **把规则钉死到数据里（做 O-1）**
   现在的实情是：正常情况下你说得对，但有一种情况会漏 —— 一次「重建」中途失败时，
   旧目录还留在账上，而账本上没写「这条记录属于哪个网盘」，所以搜索时没法按你当前的勾选名单过滤。
   要做的是：**给每条记录写上它属于哪个网盘，搜索时只按你现在勾选的名单给结果。**
   这样「取消勾选 = 立刻搜不到」就是常态，不依赖「下次重建时顺手清掉」。
   代价是一次小版本升级（会改索引的存放格式，升级后第一次会重建一次索引）。

2. **给这套规则配上自动检查（做 O-3）**
   现在「设置→搜索」这一页没有任何自动测试，全靠人看界面。
   要做的是：**把「谁该显示正在索引、谁不该显示、取消勾选后立刻消失」写成自动检查**，
   以后再改这块代码，改坏了会立刻报错，而不是等装完机才发现。

3. **让成果真正进发布流程（做 O-5）**
   3.20.1 之后的三个版本（3.21.0 / 3.21.1、以及 OneDrive 下线那次）目前**只在这台电脑上**，
   还没推到对外的代码库。要做的是：**把这三个版本推上去**，让安装包、线上更新检查和对外下载都能拿到最新版本。
   这一步是对外动作，需要你点头。

### 顺带能很快收掉的一件（B-1，5 分钟、你自己就能做）

用你现有的三条真实网盘连接做一次：**先只勾百度 → 搜一个只有夸克/阿里才有的文件名 → 应该搜不到**；
再**把百度取消勾选 → 再搜同一个百度独有的文件名 → 应该立刻搜不到**。
这一步能一次性证明「勾选与取消」在真机上真的生效，也是第 1 条改造的验收标准。

### 不在下一阶段范围内

- OneDrive 整线（等你决定恢复，且需要 GUID 与 60 天保活动作）；
- U 盘双击进 AFM、17 语言包母语复核、阿里云盘上游 400（这三项等外部条件）；
- 已拍板否掉的 B/C 形态、远程页编辑按钮。

---

## 7. 证据路径

| 用途 | 路径 |
| --- | --- |
| 批次 43 计划（含 §7 追加范围与刻意不做） | `docs/plan-2026-11-24-batch43-netdisk-ux.md` |
| 主交接台账（§十二 = 批次 42 收口 / §十四 = 批次 43 修复 / §十五 = 本轮收口） | `docs/handoff-2026-11-22-batch38-onedrive-readonly.md` |
| OneDrive 延期与恢复步骤 | `docs/plan-2026-11-24-onedrive-deferral-resume.md` |
| 批次 43 门禁 / 打包 / 装机证据 | `.superpowers/sdd/b43/`（`check-b43-3.log`、`tauri-build3.log`、`install-status.txt`、`vitest-index-target.log`） |
| 本轮改动 | commit `3dbcf57d`（3.21.1）、`c97644a4`（3.21.0）、`1b6c2ba0`（OneDrive 下线） |
