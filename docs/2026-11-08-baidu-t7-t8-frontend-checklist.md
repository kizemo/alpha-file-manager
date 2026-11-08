# T7/T8 前端接入清单（百度网盘）

> 生成于 2026-11-08。背景见 `plan-2026-11-08-baidu-netdisk-phase1.md`。
> 本文件只记**勘察到的事实**与**逐文件改动清单**，不记推理过程。

## 一、T6 收口状态

- `cargo test --lib` → `test result: ok. 1142 passed; 0 failed; 1 ignored` （日志 `.superpowers/sdd/baidu-t6-test2.log`）
- 本轮补的 4 条单测：
  - `remote::baidu_provider::tests::stat_finds_an_entry_by_listing_its_parent_directory`
  - `remote::baidu_provider::tests::stat_of_a_missing_entry_is_reported_as_not_found`
  - `remote::connection::tests::the_baidu_pan_protocol_has_its_own_stable_name`
  - `remote::registry::tests::a_baidu_connection_resolves_without_a_password`
- 守卫测试 `the_invoke_handler_has_no_stale_remote_entries` 的白名单补了 3 条新 command：
  `remote_baidu_save_session` / `remote_baidu_auth_status` / `remote_baidu_delete_session`

## 二、后端已就位的前端可调用面（Tauri command）

| command | 入参 | 返回 | 用途 |
|---|---|---|---|
| `remote_baidu_save_session` | `app, connection_id, cookie` | �� | 保存整段 cookie（BDUSS+STOKEN）到 DPAPI 密文 |
| `remote_baidu_auth_status` | `app, connection_id` | `{ status, remaining_seconds, summary }` | 授权状态卡 |
| `remote_baidu_delete_session` | `app, connection_id` | — | 删除会话（退出登录） |
| `remote_test_connection` | 既有 | 既有 | 百度分支下 `password` 字段装整段 cookie |

`status` 取值：`missing` / `ready` / `stoken_expired` / `stoken_expiring_soon`。
`summary` 是后端 `describe_state` 出来的**脱敏**一句话，直接可展示。

## 三、前端镜像文件（已勘察）

### 3.1 `src/types/remote.ts`
- `RemoteProtocol = 'ftp' | 'ftps_explicit' | 'ftps_implicit' | 'sftp'` → **加 `'baidu_pan'`**
- `getRemoteProtocolDefaultPort()` switch → **加 `case 'baidu_pan': return 443;`**
- 文件头注释写着「与后端逐条镜像」，新增变体必须同步注释

### 3.2 `src/modules/settings/ui/categories/remote/connection-form.vue`
- `const PROTOCOLS: RemoteProtocol[] = ['ftp','ftps_explicit','ftps_implicit','sftp']` → 加 `'baidu_pan'`
- 协议下拉项文案键：`settings.remote.protocol.${item}`（**已存在这个取法**）
- 表单字段：name / protocol / host / port / username / password / initialPath / localPath / proxy
- `data-test` 属性：`field-name` `field-protocol` `field-host` `field-port` `field-username` `field-password` `field-initial-path` `field-local-path` `field-proxy-kind` `test-connection` `submit` `cancel` `test-result` `test-note-password` `form-error`
- 「测试连接」结论渲染在 `probeReport.outcome` 上，附 `passwordChecked === false` 时的 `test-note-password`
- TLS 未校验提示的显示条件里有 `protocol !== 'sftp'` ⇒ **百度网盘走 HTTPS，会有 TLS 提示**，需要把 `protocol !== 'sftp'` 改成「非 sftp 且非 baidu_pan」，或让百度走独立分支

### 3.3 `src/stores/storage/remote-connections.ts`
- 校验函数 `validateForm(form, id)`：`host` 必填、无空白、端口 1..65535 ⇒ **百度协议必须跳过 host/port 校验**，否则表单根本保存不了
- 保存函数 `saveConnection(form)`：**写顺序是硬约束**（先写 DPAPI 密文，后写 storage；密文写失败直接抛，storage 一行不动）
  ⇒ 百度分支必须遵守同一条：**先 `remote_baidu_save_session`，成功后才写 storage**
- `hasNewPassword = form.password.length > 0 && !form.clearPassword` ⇒ 百度用同一字段装 cookie
- `testConnection(form)` 走 `remote_test_connection`

### 3.4 设置页分类组件
`src/modules/settings/ui/categories/remote/remote-category.test.ts` 是现存测试，组件本体同目录。
- 存档密码标记：`[data-test="entry-<id>"] [data-test="has-password"]`，文案来自 `settings.remote.password.saved`，内容是 `***`
- 导航栏编辑请求：`store.requestRemoteEdit(id)` → `consumeRemoteEdit()`

### 3.5 语言包
- 17 个：`src/localization/messages/{ch,en,de,es,fa,fr,he,hi,hy,it,ja,pt,ru,sl,tr,ur,vi}.json`
- 中文 locale code 是 **`ch`**，不是 `zh`

## 四、逐文件改动清单（T7）

1. `src/types/remote.ts`
   - `RemoteProtocol` 加 `'baidu_pan'`
   - `getRemoteProtocolDefaultPort` 加 443 分支
   - 新增 `isBaiduPanProtocol(protocol)` 之类的判定助手（表单与 store 共用）
   - 新增百度授权状态类型镜像：`BaiduAuthStatusPayload { status: 'missing'|'ready'|'stoken_expired'|'stoken_expiring_soon'; remainingSeconds?: number; summary: string }`
2. `src/stores/storage/remote-connections.ts`
   - `validateForm`：百度协议跳过 host/port/username 相关校验（name 仍必填）
   - `saveConnection`：百度分支先 `invoke('remote_baidu_save_session', { connectionId, cookie })`，再写 storage；
     `credentialKey` 一律写成连接 id（与既有约定一致：`凭据键恒等于连接 id`）
   - 新增 `getBaiduAuthStatus(id)` / `deleteBaiduSession(id)` 两个 action
3. `src/modules/settings/ui/categories/remote/connection-form.vue`
   - `PROTOCOLS` 加 `'baidu_pan'`
   - 百度协议下：隐藏 host/port/username，把「密码」标签换成「Cookie」；host 自动填 `pan.baidu.com`、port 443、username 填账号昵称（保存时若为空用空串）
   - 新增**授权卡**：显示 `remote_baidu_auth_status` 的 status/summary + 「删除会话」按钮
   - TLS 提示条件排除 `baidu_pan`
4. 设置页分类组件：百度连接条目显示授权状态点/文案（可选，视时间）

## 五、语言包新增键（T8）

```
settings.remote.protocol.baidu_pan
settings.remote.field.cookie            # Cookie 输入框标签
settings.remote.baidu.hint              # 引导：去网页版复制整段 Cookie
settings.remote.baidu.status.missing
settings.remote.baidu.status.ready
settings.remote.baidu.status.stoken_expired
settings.remote.baidu.status.stoken_expiring_soon
settings.remote.baidu.action.deleteSession
settings.remote.baidu.action.refreshStatus
settings.remote.baidu.deleteConfirm      # 删除会话确认
```

其余 16 个语言包按同一键结构补齐（英文源 → 译文）。
**不得出现英文硬编码占位**：缺翻译的语言包回落到英文原文可以，但键必须齐全。

## 六、前端单测（T8）

- `src/types/remote.ts`：`baidu_pan` 默认端口 443
- `src/stores/storage/__tests__/`：百度连接保存走 `remote_baidu_save_session` 且**先于** storage 写入；
  校验不因百度协议要求 host
- `src/modules/settings/ui/categories/remote/remote-category.test.ts`：选 `baidu_pan` 时表单出现 cookie 字段、不出现 host 字段

## 七、已知坑

- 改 `.vue` / `.ts` / `.json` **一律用编辑工具**，绝不用 PowerShell `Get-Content -Raw` / `Set-Content`（中文 mojibake）
- 批量改 JSON 用 node 的 `fs.readFileSync/writeFileSync(..., 'utf8')`
- `eslint .` 基线随提交变，量的时候用 `git stash push -u` 量 HEAD 同一条命令
- `vitest --reporter=basic` 不存在，用默认 reporter
