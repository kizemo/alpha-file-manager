# 批次 43：网盘交互四项（添加检索选项 / 搜索页云盘检索 / 删除闪黑框 / 按钮选中态）

> 状态：**进行中**（2026-11-24 开工）
> 上一批：批次 42 = OneDrive 整条线延期（开关下线，代码全留）→ 见 `docs/plan-2026-11-24-onedrive-deferral-resume.md`
> 交接文档：`docs/handoff-2026-11-22-batch38-onedrive-readonly.md`（§十二 = 批次 42 收口）

---

## 0. 一句话结论

本批次处理用户 2026-11-24 提出的四项网盘交互需求。其中 **1 项已存在**（添加云盘时的「加入全局检索」勾选），
**1 项是根因明确的进程缺陷**（删除网盘闪黑框 = `icacls` 缺 `CREATE_NO_WINDOW`），
**2 项是新增 UI 能力**（搜索页按网盘类型列检索选项、添加网盘按钮选中态）。

---

## 1. 需求与现状对照

| # | 用户需求（2026-11-24） | 开工前现状（已坐实） | 本批动作 |
| --- | --- | --- | --- |
| 1 | 添加云盘时增加是否「添加检索」的选项，**默认不添加** | **已存在**：百度/夸克走 `netdisk-add-dialog.vue:426-440`，阿里走 `connection-form.vue:1388-1400`；两处默认值都是 `false`（`netdisk-add-dialog.vue:80`、`connection-form.vue:718/874`） | 确认 + 登记，不重复实现 |
| 2 | 设置 → 搜索页面的「云盘检索」增加百度网盘 / 阿里云盘 / 夸克网盘及未来云盘选项 | `global-search.vue:619-694` 只列**已勾选纳入检索的连接**，空态文案「暂无已加入检索的网盘」；**没有任何按网盘类型的选项** | 新增（形态待用户拍板，见 §3） |
| 3 | 删除云盘时不要弹出或闪现 cmd 窗口 | **根因坐实**：`src-tauri/src/mcp/auth.rs:121` 的 `restrict_to_current_user()` 拉起 `icacls` **没加 `CREATE_NO_WINDOW`**；release 主程序 `windows_subsystem = "windows"` 无控制台 ⇒ Windows 给 console 子进程新开 conhost ⇒ 黑框闪 2 次（临时文件 + 正式文件各一次） | 补 flag（Rust） |
| 4 | 点击「添加网盘」里的「百度网盘」等按钮时，选中的类型按钮要与未选中的有区别 | **完全不存在**：4 颗按钮全是 `variant="outline"`，无任何 active/selected 状态 | 新增选中态（Vue） |

---

## 2. 需求 3：删除网盘闪黑框（根因与修法）

### 2.1 链路

```
connections-section.vue:408  handleDelete()
  → store.deleteConnection(id)            remote-connections.ts:692-708
      → saveStorageData()                 （纯文件写，无进程）
      → invoke('remote_delete_credential')  remote-connections.ts:701
          → credentials::delete_credential()  credentials.rs:506-515
              → write_vault()                credentials.rs:301-329
                  → restrict_to_current_user(temp)  ← 第 1 次 icacls
                  → rename
                  → restrict_to_current_user(path)  ← 第 2 次 icacls
                      → std::process::Command::new("icacls")   ★ 缺 CREATE_NO_WINDOW
```

### 2.2 为什么是黑框而不是别的

- `src-tauri/src/main.rs:6`：`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` ⇒ release 包自己没有控制台；
- `icacls.exe` 是 console subsystem 程序 ⇒ Windows 必须给它新分配一个 conhost ⇒ 屏幕上黑框一闪即逝。

### 2.3 触发条件

- 只在**有 `credentialKey`** 的连接上出现（`remote-connections.ts:698` 提前 return）；
- 网盘（百度/夸克/阿里/OneDrive 的会话 JSON）**必现**；存过密码的 FTP/SFTP 同样必现；
- 无凭据连接不写盘、不闪。

### 2.4 修法

给 `icacls` spawn 补 Windows 专用的 `CREATE_NO_WINDOW`（模板见 `src-tauri/src/process_runner.rs:38-56`：
`use std::os::windows::process::CommandExt;` + `const CREATE_NO_WINDOW: u32 = 0x08000000;` +
`command.creation_flags(CREATE_NO_WINDOW);`，整块包在 `#[cfg(windows)] { ... }` 里）。

**同类隐患一并修**（同一类缺陷、同一修法、代价可逆）：

| 文件 | 位置 | 程序 | 触发场景 |
| --- | --- | --- | --- |
| `src-tauri/src/dir_reader/mount.rs` | 约 :100 | `net use X: /delete /y` | 右键「断开连接」 |
| `src-tauri/src/dir_reader/network_shares.rs` | 约 :76 | `net use` | 映射网络驱动器 |

### 2.5 安全边界（不动的东西）

- **不删 `icacls` 调用** —— 收紧 DACL 是安全加固，必须保留；
- **不改 `src-tauri/src/remote/`** —— 凭据删除逻辑本身正确；
- `restrict_to_current_user()` 是 MCP 凭据与远程凭据**共用**函数 ⇒ 本次改动同时影响 MCP 凭据写入路径，行为不变（只是不再弹控制台窗口）。

---

## 3. 需求 2：设置 → 搜索的「云盘检索」选项形态（**待用户拍板**）

### 3.1 现状代码锚点

- 区块：`src/modules/settings/ui/categories/search/global-search.vue:619-694`（`SettingsItem` + `#nested` 插槽）；
- 脚本：`global-search.vue:329-383`（`indexedRemoteConnections` / `isRebuildingRemoteIndex` / `remoteIndexedItemCount` / `rebuildRemoteIndex()`）；
- 勾选真相源：`RemoteConnection.includeInGlobalSearch`（`src/types/remote.ts:92`），store 过滤在 `remote-connections.ts:195-197`；
- 勾选入口（现状只在远程页）：`connection-form.vue:1388-1400`、`netdisk-add-dialog.vue:426-440`；
- 重建命令：`global_search_index_remote_connections`（**整体重建**，传当前全部已勾选 id，漏一条就抹掉那条）。

### 3.2 三个候选形态（已提交用户问卷）

| 形态 | 说明 | 代价 |
| --- | --- | --- |
| **A（推荐）** | 按网盘类型分组，每类下列出该类型的连接，可**直接勾选**纳入/移出检索 | 需新增轻量 toggle（不能复用 `saveConnection`，见 §3.3）+ 把「勾选变化自动重建」逻辑抽成共享 composable |
| B | 每个网盘类型一个总开关，一键纳入/移出该类型全部连接 | 同 A，但多一层 provider 级开关，与连接级 `includeInGlobalSearch` 有**优先级冲突**需要额外定义 |
| C | 只列出网盘类型名称作为支持提示，不可操作 | 改动最小，但没解决「在搜索页直接控制」的需求 |

### 3.3 实现要点（A 形态）

1. **必须新增轻量 toggle**，**不能**直接调 `store.saveConnection()`：
   `saveConnection` 对网盘协议有重副作用 —— 会走 `remote_baidu_save_session` / 登录槽位 claim 分支，
   槽位为空时抛 `baiduLoginRequired` / `quarkLoginRequired` / `aliyunLoginRequired`
   （`remote-connections.ts:470-640`）⇒ 从搜索页勾选会误报错。
   ⇒ 新增 `setConnectionIncludeInGlobalSearch(id, value)`：只改这一个字段 + `saveStorageData()`，**不碰凭据**。
2. **勾选变化要自动重建索引**：现在这段逻辑（`connections-section.vue:73-174`，含串行 drain + 失败不阻断保存）
   在远程页组件里，搜索页勾不到。⇒ 抽成共享 composable，两边复用。
   ⚠️ 后端是**整体重建**，两个重建并发会互相踩（`connections-section.vue:90-96`）⇒ 必须保持串行。
3. **网盘类型清单要动态**：从 `NETDISK_PROTOCOLS`（`src/types/remote.ts:31`）派生，
   减去被开关关掉的（OneDrive 由 `ONEDRIVE_ENABLED` 控制）⇒ 「未来会添加的云盘」自动出现。
   展示名用 `NETDISK_PROVIDER_NAME_KEYS`（`src/types/remote.ts:275-280`）。
4. 顺带修掉一个语义误导：现在每条连接那颗「重建」按钮调的是 `rebuildRemoteIndex()`，
   传的是**全部**已勾选 id（`global-search.vue:359`）⇒ 点哪一行效果完全相同。

---

## 4. 需求 4：添加网盘按钮选中态

落点：`src/modules/settings/ui/categories/remote/connections-section.vue:571-622`（按钮组）。

口径：

- 新增「当前选中的网盘类型」状态，点击 provider 按钮时同步设置；
- 选中项与未选中项**视觉上有明确区别**（variant 变化 + `aria-pressed`），颜色用项目主题变量，不写死；
- **点击行为不变**：点百度/夸克仍立刻开内嵌登录窗，点阿里仍开内联连接表单（**不做**两步式）；
- 现有 `data-test` 一个不删不改；OneDrive 的 `v-if="ONEDRIVE_ENABLED"` 守卫原样保留。

---

## 5. 验收计划

| 项 | 装置 | 判据 |
| --- | --- | --- |
| 门禁 | `npm run check` | 全绿（ts + eslint + 单测 + changelog 校验） |
| 单测 | 新增/扩展的 Vue 测试 | 默认无选中态；点百度后只有百度带选中标记；原有测试全过 |
| Rust | `cargo check --lib` | 通过（⚠️ 不跑 `cargo test --lib`：本仓库有 tauri 单态化红线） |
| **真机** | `npm run tauri:build` → 装机 | ① 删除网盘时**看不到**黑框；② 添加网盘按钮点击后选中项有区别；③ 搜索页云盘检索按类型列出 |

⚠️ 真机验收只能用 `tauri build` 的打包产物（`tauri dev` 下路由页内容区空白是已知特性）。
⚠️ release 产物上 `SFM_E2E_DATA_DIR` 不生效 ⇒ 真机验收必须「备份真实文件 → 灌夹具 → 验收 → 恢复」，
恢复前先确认 `Get-Process alpha-file-manager` 为空。

---

## 6. 本批刻意不做

- 不物理删除 OneDrive 相关代码（批次 42 已用集中开关下线，资产全留）；
- 不做「添加网盘」的两步式流程（先选类型再确认）；
- 不把 provider 级开关与连接级 `includeInGlobalSearch` 并存（除非用户明确选 B 且给出优先级规则）；
- 不改发布租户 / Entra 应用相关的一切（OneDrive 线整体挂起，见批次 42 待办文档）。

---

## 7. 批次 43 追加范围：3.21.0 装机后的真机反馈（2026-11-24 第二轮，出 3.21.1）

3.21.0 装机后用户手工截图反馈两条新缺陷，并把方向收窄：**撤回**「远程页编辑按钮 / 默认显示 provider
表单」那条（原话「在远程中，设置可以达到要求，只需要检查是否勾选是否真正做到是否检索网盘」），
所以本轮只剩两件事。

### 7.1 缺陷①：「设置→搜索」索引状态串台 + 取消勾选后不回落

| 现象 | 根因 |
| --- | --- |
| 点一条已勾选连接的「重建」，三行同时显示「正在索引云盘」 | 三行共用同一个 `isRebuildingRemoteIndex`（全局互斥锁）；后端语义是「按传入连接集合**整体重建**索引」，本就不存在「只重建一条」，所以显示「三条都在索引」比「只有被点的那条在索引」更贴近事实，但用户读到的意思完全错 |
| 取消勾选后该行仍显示「正在索引云盘」 | 未勾选行原本仍渲染重建按钮、仍读同一个全局锁；drain 结束后 `finally` 清锁前，`watch` 触发第二轮重建，取消勾选的连接又被塞回新一轮 |

**修复方式（本轮定稿）**：全局锁继续管**禁用**，新增**按连接的目标集合**管**文案**；未勾选行不渲染按钮。
不新增 i18n 键（未勾选行为什么没有按钮无需向用户解释，省掉 17 个语言包的同步成本）。
详见 `docs/handoff-2026-11-22-batch38-onedrive-readonly.md` 第十四章。

### 7.2 缺陷②：勾选是否真正控制网盘被检索（核查项，不是改动项）

结论（代码层实测）：闭环成立。`rebuild_remote_index(connection_ids)` 的集合由前端
`globalSearchConnectionIds` 现取，该值 = `includeInGlobalSearch === true` 的连接；后端只吃传入集合，
查询侧不做读时过滤 ⇒ 「保存时重建」就是勾选生效的唯一通路，勾选语义没有被绕过。

**唯一残留缺口**：重建失败时索引与勾选状态不一致（前端 `console.error` + `toast.danger`，不重试）。
本轮加了**失败自愈补跑**（下次进设置页自动重试），并把「要彻底消除该缺口需给 `RemoteIndexDoc` 加
connection 维度」登记为遗留。

**仍未实测**：装机后取消勾选的网盘是否真的不再被全局检索搜到——本项目拿不到 webview console，
CDP 注入与 UIA 自动化两条路都已在第十三章坐实走不通，界面级证据只能由用户手工截图提供。

### 7.3 本轮刻意不做

- 远程页编辑按钮与默认表单（用户已撤回该需求）。
- 「加入全局检索」勾选前置到类型选择一步（批次 42 已否决 B/C 方案）。
- `RemoteIndexDoc` 增加 connection 维度（改动量大，收益只是消除一个已有自愈措施的缺口）。
