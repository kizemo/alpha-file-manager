# OneDrive 支持：延期待办（2026-11-24 暂停，先做别的任务）

> 状态：**延期（代码已下线，资产全留）**
> 决策人：用户（2026-11-24 指令：「撰写待办文档 → 暂时从 app 代码去除 OneDrive → 转向其它任务」）
> 恢复入口：`src/constants/feature-flags.ts` 的 `ONEDRIVE_ENABLED`
> 相关交接文档：`docs/handoff-2026-11-22-batch38-onedrive-readonly.md`（§十、§十一、§十二）

---

## 0. 一句话结论

OneDrive 整条线**延期**：app 里已经下线（用户看不到入口、建不了连接、发不了授权），
**代码一行没删**，恢复只需翻一个开关 + 回填自建 Entra 应用的 client_id。
已发布的 3.20.1 安装包不受影响（发出去的包不会回溯），**下一次发版才不含 OneDrive**。

---

## 1. 为什么用「开关下线」而不是「删代码」

| 方案 | 恢复成本 | 风险 | 结论 |
|---|---|---|---|
| 硬删前端/后端/locale/测试 | 要重写一遍，改动面 20+ 文件 | 恢复时极易漏（漏一个 locale 就崩 i18n 校验）；验收装置的 `data-test` 断言全废 | ❌ 不采用 |
| **集中开关下线（本次采用）** | **1 行常量 + 回填 client_id** | 旧连接数据仍在，UI 不出现即可 | ✅ 采用 |

「从 app 代码中去除」的落地口径 = **用户界面里彻底没有 OneDrive**，
而不是把源码物理删除。用户要的「暂时」语义决定了这个口径。

---

## 2. 本轮改了什么（改动清单）

| # | 文件 | 改动 | 为什么 |
|---|---|---|---|
| 1 | `src/constants/feature-flags.ts`（**新增**） | 新增 `export const ONEDRIVE_ENABLED = false;`，附语义说明与恢复指引 | 全应用唯一的开关真相源 |
| 2 | `src/modules/settings/ui/categories/remote/connections-section.vue` | ① 新增开关 import；② 「添加网盘」里的 **OneDrive 按钮加 `v-if="ONEDRIVE_ENABLED"`**（整个不渲染）；③ 快捷弹窗里 `requested === 'onedrive_pan'` 分支加开关守卫 | 用户新建不了 OneDrive 连接 |
| 3 | `src/modules/settings/ui/categories/remote/connection-form.vue` | ① 新增开关 import；② OneDrive 表单区块 `v-else-if="isOnedriveNetdisk && ONEDRIVE_ENABLED"`；③ 通用（SFTP/FTP）表单分支由 `v-else` 改为 `v-else-if="!isOnedriveNetdisk"` | 旧 OneDrive 连接打开编辑表单时，**既不显示 OneDrive 授权区、也不会误显示 SFTP 字段** |
| 4 | `src/modules/settings/ui/categories/remote/connection-form.vue` | 旧连接底部的 OneDrive 会话状态块 `v-if="onedriveConnectionId"` → `v-if="ONEDRIVE_ENABLED && onedriveConnectionId"` | OneDrive 相关 UI 一律不渲染 |

**刻意没动的地方**（动了反而更麻烦）：

- `src/types/remote.ts` 的 `NETDISK_PROTOCOLS` **保留** `'onedrive_pan'`
  ⇒ 已存下来的 OneDrive 连接仍能被解析成网盘类型，列表里照常显示、照常能删，**不丢用户数据、不崩**；
  也让 `src/types/__tests__/remote-protocol-aliyun.test.ts:91` 那条断言不用改。
- `src-tauri/src/remote/onedrive_*.rs`、`src-tauri/src/onedrive_oauth.rs` **全部保留**
  ⇒ 恢复时不用重写后端，也不用重跑那一大堆 Rust 单测。
- 17 个语言包里的 OneDrive 文案**保留**（留着不占成本，删了要同步改 17 份）。
- `src/stores/storage/remote-connections.ts` 的 OneDrive 方法**保留**（界面不再调用）。

---

## 3. 下线后的实际表现（用户视角）

1. 设置 → 远程 →「添加网盘」只剩 **百度网盘 / 夸克网盘 / 阿里云盘**，没有 OneDrive。
2. 已经存在的 OneDrive 连接**仍然列在连接列表里**（可以查看、可以删除），但点进去的编辑表单里
   **没有任何 OneDrive 可操作项**，也不会误出现服务器/端口字段。
3. 后端 OneDrive 命令仍在二进制里，但**没有任何界面路径能触发**。
4. 全局搜索、位置页等处**不会**再出现 OneDrive 相关入口（它们本来只对已存在的连接生效）。

> ⚠️ **已知边界**：如果用户本地已经存在 OneDrive 连接，它的**图标/名称仍会出现在侧边栏与连接列表**里
> （这是有意保留的数据可见性，不是漏网）。要彻底看不见，需要额外做「按协议过滤列表」——本轮**不做**，
> 属于恢复时的可选项。

---

## 4. 恢复步骤（照做，编号即顺序）

| 步 | 动作 | 判据 / 命令 |
|---|---|---|
| **R1** | 翻开关 | `src/constants/feature-flags.ts`：`ONEDRIVE_ENABLED = false` → `true` |
| **R2** | 回填自建应用的 client_id | `src/constants/feature-flags.ts` 新增 `export const ONEDRIVE_DEFAULT_CLIENT_ID = '<GUID>';`；`connection-form.vue` 里 `const onedriveClientId = ref('')` 改成 `ref(ONEDRIVE_DEFAULT_CLIENT_ID)`；**输入框保留可覆盖**（高级用户可填自己的应用） |
| **R3** | 核对发布租户/应用还活着 | 登录 `portal.azure.com` 看得到发布租户；`entra.microsoft.com` 里应用还在（**别让租户进入不活跃状态**，见 §5） |
| **R4** | 真 GUID 登录验收 | `powershell.exe -NoProfile -ExecutionPolicy Bypass -File F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\b38\accept-real-credentials.ps1 -ClientId '<GUID>' -Phase all -ExePath 'D:\Program Files\Alpha File Manager\alpha-file-manager.exe' -TimeoutSeconds 600`（**必须 powershell.exe 5.1**；verify 阶段零用户交互） |
| **R5** | 写路径验收 | `powershell.exe -NoProfile -ExecutionPolicy Bypass -File F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\b41\accept-real-write2.ps1 -AllowWrite -FixtureViaGraph -ExePath 'D:\Program Files\Alpha File Manager\alpha-file-manager.exe' -TimeoutSeconds 300`（上次基线：`SUMMARY pass=10 fail=0 undetermined=0`） |
| **R6** | 清理审计 | `powershell.exe -NoProfile -ExecutionPolicy Bypass -File F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\b41\cleanup-probe-items.ps1`（判据：`found -eq deleted`） |
| **R7** | 门禁 + 升版 + 打包 + 装机 | `npm run check` → `node scripts/sync-version.js` → `npm run tauri:build` → 装机（**同版本重装必须带 `/FORCE`**，否则 same-build 防呆 `INSTALL_EXIT=2`） |
| **R8** | 发布形态核对 | 终端用户侧**零 GUID**（表单已预填）、零 Azure 账号、零订阅 |

> ⚠️ R4/R5 的装置脚本在 `.superpowers/sdd/` 下，**该目录被 gitignore** ⇒ 只存在于本机，
> 新克隆的仓库里没有。恢复时如果换机器，先确认这些脚本还在。

---

## 5. 恢复时必须一起处理的三件事（别只翻开关）

1. **发布租户/应用不能死**（否则全体用户一起掉线）
   - 官方口径：租户自计费周期起 **200 天不活跃 → AADSTS5000225 登录被 block → 自阻断起 20 天内**
     可联系微软支持重新激活，**超过 20 天永久删除、不可恢复**；
   - Azure 免费账号条款：**90 天无费用/无活动**微软保留移除计算资源的权利，
     「登录门户 / 访问一次 Azure API」即算活动；
   - ⇒ **每 60 天登录一次 `portal.azure.com`**（个人日历重复提醒）。
2. **client_id 内置后的单点**：全体用户的 refresh_token 绑定这一个 client_id，
   应用被删/被停 = 所有装 AFM 的用户一起登录不了。缓解手段：在**同一个发布租户**里再注册一个
   备用应用（出问题时切换 + 重发版），并把恢复码/client_id 交给第二持有人。
3. **`error_description` 没透传到前端**（批次 41 §十一·9 登记的非阻塞缺陷）：
   发布形态下登录失败只能看到粗粒度错误。**建议在恢复批次里第一件事就修掉它。**

---

## 6. 保留的资产（恢复时不要重做）

### 6.1 代码位置

| 层 | 文件 |
|---|---|
| 前端类型 | `src/types/remote.ts`（`RemoteProtocol` / `NETDISK_PROTOCOLS` / `isOnedrivePanProtocol` / `Onedrive*Payload`） |
| 前端 UI | `src/modules/settings/ui/categories/remote/connections-section.vue`、`connection-form.vue` |
| 前端 store | `src/stores/storage/remote-connections.ts`（OneDrive 登录/状态/删会话） |
| 后端 | `src-tauri/src/remote/onedrive_api.rs`、`onedrive_auth.rs`、`onedrive_provider.rs`、`src-tauri/src/onedrive_oauth.rs`、`src-tauri/src/remote/{commands,connection,connection_test,mod,registry}.rs`、`src-tauri/src/lib.rs`（command 注册） |

### 6.2 验收装置（**本机专属，gitignored**）

| 装置 | 用途 |
|---|---|
| `.superpowers/sdd/b38/accept-real-credentials.ps1` | 真 GUID 登录验收（R4） |
| `.superpowers/sdd/b41/accept-real-write2.ps1` | 写路径 W1–W5 验收（R5） |
| `.superpowers/sdd/b41/cleanup-probe-items.ps1` | 清理审计（R6） |
| `.superpowers/sdd/b41/accept-real-b41write-20261124-091714/` | 上次全绿的原始报告（`SUMMARY pass=10 fail=0 undetermined=0`） |

### 6.3 研究底稿（**本机专属，gitignored**；关键结论已内联到 §7）

`.superpowers/sdd/b41/` 下四份：`onedrive-cost-analysis.md`（成本 $0.00 论证）、
`entra-account-mfa-bootstrap.md`（企业账号 MFA 自举死锁与解锁通道）、
`entra-app-checklist.md`（应用注册 5 项设置）、`release-account-decoupling-runbook.md`（发布账号解耦 runbook）。

---

## 7. 内联要点：自建 Entra 应用怎么注册（照抄，别再重新研究）

桌面 **public client**（PKCE + 回环随机端口、**无 client_secret**）注册在：

| # | 设置项 | 取值 |
|---|---|---|
| 1 | 平台 | **Mobile and desktop applications**（移动和桌面应用程序）——⚠️ **不是 Web**，挂了 Web 会被当机密客户端要 secret |
| 2 | 自定义重定向 URI | **精确 `http://localhost`**（不带端口/路径/尾斜杠）；Entra 对它匹配**任意端口** |
| 3 | 不要做 | 不要把同一 URI 再挂 **Web / SPA** 平台；不要注册多个只有端口不同的 URI；**不要建 client secret** |
| 4 | Advanced settings | **Allow public client flows = Yes** |
| 5 | 账户类型 | **多租户 + 个人 Microsoft 账户**（`AzureADandPersonalMicrosoftAccount`）⇒ 授权端点用 `common` |
| 6 | API 权限（委托） | `Files.ReadWrite`（OneDrive）+ `offline_access`（OpenID Connect）——与代码硬编码的 `ONEDRIVE_SCOPE` 逐字一致 |

发布账号与公司账号**解耦**的四层（身份/租户/计费/设备）与保活口径见 §5.1 与
`.superpowers/sdd/b41/release-account-decoupling-runbook.md`。

---

## 8. 已验证 / 推断 / 未验证

**已验证**
- 写路径闭环（3.20.1 装机）：`SUMMARY pass=10 fail=0 undetermined=0`，Graph 独立复核 `PROBE_ITEMS_FOUND=0 PROBE_ITEMS_DELETED=0`。
- 成本实为 **$0.00**（官方定价页 + Microsoft Q&A 6068372：订阅内无可计费资源 ⇒ 无资源消耗费）。
- 应用注册平台口径：桌面 public client 应挂 **Mobile and desktop applications**，`http://localhost` 允许任意端口，
  同一 URI 不可再挂 Web/SPA（平台规则 + AADSTS50011 精确匹配）。
- 租户不活跃 → block → 20 天 → 永久删除（Learn Q&A 官方答复）。

**推断**
- 「保留绑定的支付方式有效」可降低被判定弃用的概率（非官方明文）。
- 旧 OneDrive 连接在列表里继续可见是「有意保留的数据可见性」，不是漏网。

**未验证**
- 本轮下线改动的**真机界面表现**（本轮只跑了门禁，**未打包、未装机**）。
- 新 MSA 注册 Azure 后默认租户的实际名称/ID。
- 免费账号升级 PAYG 在当前页面的确切按钮位置与文案。

---

## 9. 参考

- 批次 38 实施方案：`docs/plan-2026-11-22-batch38-onedrive-support.md`
- 阿里云盘同型实现：`docs/plan-2026-11-19-aliyun-onedrive.md`
- 交接文档：`docs/handoff-2026-11-22-batch38-onedrive-readonly.md`
- 项目约定：`AGENTS.md`
