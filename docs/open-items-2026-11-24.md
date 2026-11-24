# 未完成事项台账（截至 2026-11-24，批次 44 / 3.22.0 收口后）

> **作用**：把散落在各批次 handoff / plan 里的「还没做完的事」汇成一张可执行清单，避免再靠记忆翻十份文档。
> **口径**：`OPEN` = 仍开着、可直接开工；`BLOCKED` = 等外部条件（真实凭据 / 上游服务 / 用户本人操作）；`DEFERRED` = 用户主动延期，不算遗漏；`CLOSED` = 已闭合。
> **本轮依据**：批次 44 收口（O-1 / O-2 / O-3 / O-6 全部闭合，升版 3.22.0 已装机）。详见 `docs/handoff-2026-11-22-batch38-onedrive-readonly.md` §十六。
> **最近更新**：2026-11-24

---

## 0. 一句话结论

批次 44 把「勾了哪个网盘，就只搜哪个网盘」这条规则**钉进了索引数据里**（O-1 闭合），
并给它配上了**自动检查**（O-3 闭合）、修掉了自愈补跑的多跑一次（O-2 闭合）、清掉了 `reports/` 残留（O-6 闭合）。

仍然开着 **2 项**：其中 1 项要用户点头（推送 origin，O-5），1 项是界面级取证的**半通**路线（O-4）；
另外 **6 项**属于等外部条件的阻塞项、**1 条**是用户主动延期的线（OneDrive）、**3 项**是已明确不再做的。

---

## 1. 本轮已闭合

| 项 | 结论 | 证据 |
| --- | --- | --- |
| O-1 索引条目记「它属于哪个网盘」，取消勾选立刻生效 | **已闭合**（3.22.0 已装机）：云盘索引条目新增 `connection_id`，查询侧按当前勾选集合在索引层过滤 | commit `1b0b32e3`；`src-tauri/src/global_search/{index,remote_index,query}.rs`；handoff §十六·2 |
| O-2 自愈补跑跨页切换可能多跑一次 | **已闭合**：模块级闸门 `remoteIndexResyncAttempted`，一次失败最多自动补跑一次 | commit `1b0b32e3`；`src/composables/use-remote-global-search-index.ts` |
| O-3 搜索页模板没有自动化测试 | **已闭合**：补组件级单测 6 例（分组/逐行/勾选态、未勾选不渲染按钮、正在索引只给本轮集合、全部重建只传已勾选集合、一个没勾选时提示无按钮、取消勾选当场落库且按钮消失） | `src/modules/settings/ui/categories/search/__tests__/global-search-remote-index.test.ts` |
| O-6 `reports/` 残留目录未跟踪 | **已闭合**：4 个批次 38/41 探针文件归档到 `.superpowers/sdd/b38/reports-archive/`（该目录已被忽略） | `.superpowers/sdd/b38/reports-archive/` |
| 门禁 | `npm run check` 全绿（`Test Files 334 passed` / `Tests 3597 passed`）；`cargo test --lib` 1691 passed / 0 failed / 3 ignored | `.superpowers/sdd/b44/check3.log`、`.superpowers/sdd/b44-cargo-test.log` |
| 打包 + 装机 | `release/Alpha File Manager_3.22.0_x64-setup.exe`（46,151,697 B）；已装到 `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，`ProductVersion = 3.22.0` | `.superpowers/sdd/b44/tauri-build.log`、`.superpowers/sdd/b44/install-status.txt` |
| 真机：升级后旧索引确实是「格式过期」 | **已验证**：3.21.1 建的 `remote-index/meta.json` 只有 7 个字段、无 `connection_id` ⇒ 升级后判 `stale` 属实；应用启动（恢复到「设置-搜索」路由）后界面出现「正在索引云盘」、进程与网盘 API 保持 Established 连接 ⇒ 重建**已被触发且在扫描中**（索引文件改写尚未观测到） | `.superpowers/sdd/b44/uia-report.txt`、`.superpowers/sdd/b44/uia-observe.txt` |

---

## 2. OPEN —— 仍开着，可直接开工

### O-5 四个提交未推送（需要授权）

`git log origin/main..HEAD`：

| 提交 | 内容 |
| --- | --- |
| `1b6c2ba0` | OneDrive 支持整条线延期（界面下线，代码全留） |
| `c97644a4` | 批次 43 四项网盘交互收口（升版 3.21.0） |
| `3dbcf57d` | 修索引状态串台 + 核查勾选语义（升版 3.21.1） |
| `5409b9c4` | 批次 43 第二轮收口文档（跨批次台账） |
| `1b0b32e3` | 批次 44 收口（O-1/O-2/O-3/O-6，升版 3.22.0） |
| 「docs(batch44) 收口文档提交，紧跟 `1b0b32e3` 之后」 | 批次 44 收口文档（台账更新 + handoff §十六） |

- 远端 `origin = https://github.com/kizemo/alpha-file-manager.git`，本地 `main` 领先 **6 个提交**。
- **推送属对外动作，需用户明确授权**（本仓一贯口径）。本批次未做任何推送动作。

### O-4 界面级取证路线：**通了 UI 这一半，console/CDP 那一半仍不通**

- **已通（批次 44 新发现）**：WebView2 会把页面的**可访问性树**暴露给 UI Automation（UIA）——
  能读到元素名（`设置` / `百度网盘` / `正在索引云盘`）**和 CSS class 名**
  （`global-search-settings__remote-index-item` 等）；`InvokePattern` 不可用，但
  `GetClickablePoint()` + 鼠标点击**可用** ⇒ 可以驱动界面、可以按元素状态取证。
- **仍不通**：`data-test` 属性 UIA 看不到（它不是可访问性属性）；webview console 仍然拿不到；
  **CDP 连不上**——`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 被 wry 传入的
  `AdditionalBrowserArguments` **覆盖**（实测：进程命令行里没有 `--remote-debugging-port`，9222 端口无监听）。
- **下一条可选出路**：改 `AdditionalBrowserArguments` 打一个**验收专用包**（不进 `release/`）。
- **来源**：handoff §十三·5、§十四·7 第 4 条、§十六·5。

---

## 3. BLOCKED —— 等外部条件，不能单方推进

| # | 事项 | 等什么 | 来源 |
| --- | --- | --- | --- |
| B-1 | **取消勾选后旧索引条目是否真的从搜索结果消失**、多连接混合检索的排序 —— 真机未跑 | 用户本机三条真实连接（百度/夸克/阿里，条件已具备）+ 用户手工操作 | `docs/handoff-2026-11-22-batch37-cloud-search-favorite-alias.md` §7-2；与 O-1 同一根因 |
| B-2 | **阿里云盘上游 HTTP 400**（批次 35 遗留，未触碰） | 上游恢复；恢复后重跑 `.superpowers\sdd\b32\b32-accept-remote-ui.ps1` 取一份 `errors` 为空的干净报告 | `docs/handoff-2026-11-21-batch35-remote-thumbnail-delivery.md` §7-3；batch37 §7-1/§7-3 |
| B-3 | 17 个语言包**母语复核** | 母语者本人 | `docs/handoff-2026-11-13-quark-pan-batch13.md` §「未纳入本轮」R4 |
| B-4 | `Ctrl+Shift+P` 在**未被其他程序占用**的机器上复验 | 一台干净机器（本机该键被外部程序以 OS 级 `RegisterHotKey` 占用，非 AFM 缺陷；3.13.7 已加冲突检测徽标） | `docs/handoff-2026-11-11-p5b-shortcut-conflict-detection.md`；batch13 R5 |
| B-5 | shell 扩展 `Classes\Drive` / `Classes\Directory` **未注册** ⇒ U 盘盘符双击能否进 AFM 从未验证 | 动系统注册表 + 定长数组 + 快照/回滚改造，属架构级改动，需用户拍板 | `docs/handoff-2026-11-11-quark-pan.md` §20.19.5-7 |
| B-6 | 焦点同步安装器 NSIS **STAGE 7 失败路径**未覆盖 | 需要构造失败场景 | `docs/handoff-2026-11-10-focus-sync-installer-cleanup.md` §末 |

### B-1 的口径更新（批次 44 之后）

O-1 把语义从「取消勾选要等下次重建才清干净」改成「**查询时按当前勾选集合过滤**」，
所以验收动作比原来更简单、更即时：

1. 先等 3.22.0 的云盘索引重建跑完（本机 16:26 起的那一轮，扫描 2 万条，网盘 API 慢）；
2. 只勾百度 ⇒ 搜一个只有夸克/阿里才有的文件名 ⇒ **应该搜不到**；
3. 把百度取消勾选 ⇒ 再搜同一个百度独有的文件名 ⇒ **应该立刻搜不到**（不需要等任何重建）。

> ⚠️ **升级后请打开一次「设置」页**：格式自愈（stale ⇒ 自动重建）只在进设置页时触发一次；
> 从没进过设置页的话，云盘索引会停留在旧格式，云盘检索会为空。

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
     要在同一租户再注册一个备用应用。
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
| 给 `RemoteIndexDoc` 加 connection 维度 | ~~本轮不做，登记为遗留~~ → **批次 44 已做**（台账上一行作废） | plan §7.3、handoff §十四·7 第 1 条 |

---

## 6. 下一阶段目标（**非技术表述**）

### 一句话

**规则已经钉死了，接下来只差两件收尾：把成果推上去，以及你亲手确认一次「取消勾选 = 立刻搜不到」。**

### 拆成三件用户能看懂的事

1. **（已完成）把规则钉死到数据里**
   每条云盘记录现在都写明「它属于哪个网盘」，搜索时只按你**现在**勾选的名单给结果
   ⇒ 「取消勾选 = 立刻搜不到」不再依赖「等下次重建顺手清掉」。

2. **（已完成）给这套规则配上自动检查**
   「设置→搜索」这一页现在有组件级自动测试了：谁该显示正在索引、谁不该显示、
   取消勾选后立刻消失，都写成了断言；以后再改这块代码，改坏了会立刻报错。

3. **（待你点头）让成果真正进发布流程**
   3.21.0 / 3.21.1 / OneDrive 下线那次 / 3.22.0 这几个版本目前**只在这台电脑上**，
   还没推到对外的代码库。要做的是：**把这五个提交推上去**，让安装包、线上更新检查和对外下载都能拿到最新版本。
   这一步是对外动作，需要你点头。

### 顺带能很快收掉的一件（B-1，你自己就能做）

用你现有的三条真实网盘连接做一次：
**取消勾选百度 → 搜一个百度独有的文件名 → 应该立刻搜不到**（不需要等重建）。
再**只勾百度 → 搜一个只有夸克/阿里才有的文件名 → 应该搜不到**。

### 不在下一阶段范围内

- OneDrive 整线（等你决定恢复，且需要 GUID 与 60 天保活动作）；
- U 盘双击进 AFM、17 语言包母语复核、阿里云盘上游 400（这三项等外部条件）；
- 已拍板否掉的 B/C 形态、远程页编辑按钮。

---

## 7. 证据路径

| 用途 | 路径 |
| --- | --- |
| 批次 43 计划（含 §7 追加范围与刻意不做） | `docs/plan-2026-11-24-batch43-netdisk-ux.md` |
| 主交接台账（§十二 = 批次 42 收口 / §十四 = 批次 43 修复 / §十五 = 批次 43 收口 / §十六 = 批次 44 收口） | `docs/handoff-2026-11-22-batch38-onedrive-readonly.md` |
| OneDrive 延期与恢复步骤 | `docs/plan-2026-11-24-onedrive-deferral-resume.md` |
| 批次 43 门禁 / 打包 / 装机证据 | `.superpowers/sdd/b43/`（`check-b43-3.log`、`tauri-build3.log`、`install-status.txt`、`vitest-index-target.log`） |
| 批次 44 门禁 / 打包 / 装机 / 真机证据 | `.superpowers/sdd/b44/`（`check3.log`、`tauri-build.log`、`install-status.txt`、`uia-report.txt`、`uia-observe.txt`、`rebuild-poll2.txt`） |
| 批次 44 Rust 单测日志 | `.superpowers/sdd/b44-cargo-test.log` |
| 本轮改动 | commit `1b0b32e3`（3.22.0） |
