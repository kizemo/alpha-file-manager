# 2026-11-09 批次 F 真机验收结论（3.13.3 装机实测）

## 一句话结论

- **反馈 1（测试连接诊断副行）**：✅ **实测通过**。
- **反馈 2（删除远程条目后列表立即更新）**：⛔ **本轮无法完成真机验收** ——
  阻塞点在**百度服务端/账号风控**这一层，不在应用的刷新逻辑；应用侧同时暴露了一个**独立的前端缺陷**（远程新建目录失败后「创建」按钮永久禁用且无错误提示）。

---

## 二、已验证（有证据）

### 2.1 装机与版本

- 安装包：`release\Alpha File Manager_3.13.3_x64-setup.exe`（45,451,695 字节 / 2026-11-09 14:55:11）。
- 已静默安装并运行：`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，FileVersion=3.13.3。
- 基线：`cargo test --lib` = **1172 passed / 0 failed / 1 ignored**；`eslint .` = **177 errors / 0 warnings**。

### 2.2 反馈 1 —— 通过

路径：设置 → 远程 → 百度网盘 → 编辑 → 「测试连接」。

- 主文案：**连接成功，服务器接受了当前设置。百度网盘连接正常，账号：风扇杰叔叔**
- 副行（`data-test="test-detail"`）：**本次未提交密码，因此不能据此确认密码正确。**
- 截图：`.superpowers\sdd\accept-3133\test-02.png`

⇒ F.3 的「无凭据不再误报成凭据被拒 + 后端诊断副行」在**真实装机产物**上成立。

### 2.3 验收后环境复原

- 被移开的用户窗口已复原（脚本 `.superpowers\sdd\restore-occluders-3133.ps1`，记录 `.superpowers\sdd\occluders-3133.json`）：
  - `Microsoft Store`（ApplicationFrameHost）→ `-11,-11 3862x2110` ✔
  - practilog「知行日志」（hwnd=16584818）→ `1020,414 1800x1260` ✔
  - 记录里另一个 hwnd=26808730 已失效（窗口重建过），按现存句柄复原。

---

## 三、阻塞：百度写链路在服务端/风控层被拒

### 3.1 真机现象

进入远程目录 `百度网盘/0000temp`（空目录，属性面板路径
`sfm://remote/00069babf4f42d1aef22c61aa89b0a9/0000temp`），
用「新建目录」造一个待删条目：

| 步骤 | 结果 |
|---|---|
| Ctrl+Shift+N 打开对话框 | ✔ |
| 输入名称 `afm3133del` | ✔（截图 `mk-02.png`，`type-3133.ps1` 用 `KEYEVENTF_UNICODE` 逐字键入） |
| 点「创建」（UIA Invoke，rect=1304,596） | 「创建」按钮**立刻变灰**（对比 `mk-02.png` 可点 vs `mk-03.png` 灰）⇒ 点击确实落地 |
| 等 25 s / 45 s / 135 s | 按钮**始终未恢复**，**没有任何 toast 错误提示**（`mk-05/07/11.png`） |
| 关闭对话框 | 列表仍 0 项 ⇒ **远程「新建目录」既没成功、也没报错** |

### 3.2 HTTP 层离线探针（自建自删目录，不碰用户既有数据）

脚本：`.superpowers\sdd\baidu-bdstoken-probe.ps1`、`.superpowers\sdd\baidu-create-body-probe.ps1`、
`.superpowers\sdd\baidu-write-matrix.ps1`（均为纯 ASCII，**用 Windows PowerShell 5.1 跑**
——本机 pwsh 7.6.9 有程序集装载故障，`-f` 格式化都会抛
`Could not load file or assembly 'System.Management.Automation, Version=7.6.0.500' … 0x80070057`）。

**第一轮（今日早些时候，同一凭据）**

| 请求 | 结果 |
|---|---|
| `GET /api/gettemplatevariable?...fields=["bdstoken"]` | 200，**errno=0**，bdstoken len=32 ⇒ 凭据/token 链路当时是通的 |
| `POST /api/create?a=commit&channel=chunlei&bdstoken=…&path=…&isdir=1&block_list=[]&web=1&app_id=250528`（**空 body**） | 200，**errno=2**（参数错误，keys=errno,path,request_id） |
| `POST /api/filemanager?opera=delete&async=2&onnest=fail&…` body `filelist=[{"path":"/0000temp/afm-write-test"}]` | 200，**errno=132**，响应含 `info` / `verify_scene` / **`authwidget`** ⇒ **百度风控拦截** |
| `POST /rest/2.0/xpan/file?method=create` | errno=9019 |
| `POST /rest/2.0/xpan/multimedia?method=filemanager` | HTTP 400 |

**第二轮（本轮，同一 vault 条目、同一 cookie）**

`GET /api/gettemplatevariable` 在**三种 header 组合下全部 errno=-6**：

| header 组合 | 结果 |
|---|---|
| 应用同款 UA `netdisk;P2SP;2.2.60.26` | 200，errno=**-6** |
| 同上 + `Referer: https://pan.baidu.com/disk/home` | 200，errno=**-6** |
| 不带 UA | 200，errno=**-6** |

⇒ 同一个 vault 条目（`0f0696babf4f42d1aef22c61aa89b0a9`，BDUSS len=192 / STOKEN len=64）
**两小时前还能取到 bdstoken，现在连 token 都取不到了** ⇒ **STOKEN/会话已失效或被风控作废**。

对照旧连接证据 `.superpowers\sdd\acc-20261109\baidu-token-source.txt`：
旧连接 `6c1d7da…` 对 REST 读接口与 gettemplatevariable 均 errno=-6 ⇒ **errno=-6 就是这套凭据形态下的「鉴权失败」**。

### 3.3 结论

1. 写链路今天早些时候就被百度拒（errno=2 参数错误、errno=132 风控 + authwidget），
   本轮进一步恶化到 **token 都取不到（errno=-6）**。
2. ⇒ **反馈 2 的成功路径（删除成功 → 列表立即消失）在当前账号/凭据状态下无法进入验收**：
   连「新建一个待删条目」这一步都做不到。
3. 应用侧能做的只有：把失败**如实告诉用户**（现在没有），而不是让按钮永久灰着。

---

## 四、待修缺陷（本轮新发现，与百度服务端无关）

### D1（前端，独立可修）远程写操作失败后无任何反馈

- 现象：点「创建」→ 按钮永久禁用 + 无 toast。
- 代码落点：`src/modules/navigator/components/file-browser/composables/use-file-browser-selection.ts:1196-1250`
  `createRemoteDirectoryIn` 已经有失败分支（`toast.custom(... failedToCreateNewDirectory ...)`），
  ⇒ **说明 invoke 既没 resolve 也没 reject**：Promise 悬着，按钮的 pending 态永不复位。
- 需要查清：对话框组件的 disabled/pending 态由谁持有、失败路径是否缺 `finally` 复位。
- 参考：本地分支 `createNewItem`（`:1310-1338`）把 invoke 包在 `try/catch` 里，
  **远程分支走的是 `createRemoteDirectory`（`src/modules/remote/utils/remote-write.ts:105-133`）**，
  那里的 `catch` 只覆盖 `await makeRemoteDirectory(target)`。

### D2（后端，请求形态）`/api/create` 空 body 很可能是 errno=2 的直接原因

- 代码落点：`src-tauri/src/remote/baidu_provider.rs:309-321`
  `create_dir` → `post_form(&self.client, session, &url, "")` ⇒ **body 是空的**。
- 社区实现（bypy 的 `mkdir`）在**同样的 URL** 上带 body `size=0&isdir=1&autoinit=1`。
- ⚠️ **未实证**：本轮 token 已失效（errno=-6），带 body 与否都没能跑出可比对的 errno，
  所以「加 body 就能过」目前**只是假设**。但空 body 确实是与社区实现唯一的形态差异，值得一试。

### D3（后端，可观测性）写/读路径零日志锚点

- 安装版日志 `%LOCALAPPDATA%\com.sigma-file-manager.app\logs\Alpha File Manager.log`
  里 `remote` / `baidu` 字样**零命中** ⇒ 后端对远程读列/写/删**没有任何日志**，
  导致「按钮灰了 135 秒」这类问题**无法从日志二分**（invoke 卡在哪一步完全看不见）。
- 这正是 F.8 预留的锚点，现在**必须提前落**，否则下一轮还是黑盒。

---

## 五、下一步（推荐执行顺序）

1. 落 F.8 的日志锚点（`remote_make_dir` / `remote_remove_path` / `remote_read_dir` 入口 + 出口 + 条目数），
   并把百度侧的 `errno` 一起打出来。
2. 修 D1：远程写失败必须**恢复按钮 + 出错误提示**（前端）。
3. 试 D2：`/api/create` body 改成 `size=0&isdir=1&autoinit=1`。
4. 出 **3.13.4**，装机；**需要用户重新登录一次百度网盘**（当前 STOKEN 已失效，
   否则任何写操作都会被 errno=-6 挡在门外），然后：
   - 复测反馈 2：删除远程条目 → 列表**立即**消失，不需手动刷新；
   - 若仍需刷新 ⇒ 用日志锚点二分「前端守卫早退」与「后端返回旧列表」。

> 注：重新登录是唯一需要用户动手的一步（内嵌登录窗要人扫码/输密码），应用无法自己完成。

---

## 六、证据路径

| 内容 | 路径 |
|---|---|
| 3.13.3 安装包 | `release\Alpha File Manager_3.13.3_x64-setup.exe` |
| 截图（测试连接、创建目录、按钮变灰） | `.superpowers\sdd\accept-3133\*.png` |
| HTTP 探针 | `.superpowers\sdd\baidu-bdstoken-probe.ps1`、`.superpowers\sdd\baidu-create-body-probe.ps1`、`.superpowers\sdd\baidu-write-matrix.ps1` |
| UIA/点击/输入脚本 | `.superpowers\sdd\act-3133.ps1`、`.superpowers\sdd\type-3133.ps1`、`.superpowers\sdd\scroll-3133.ps1` |
| 窗口遮挡记录与复原 | `.superpowers\sdd\occluders-3133.json`、`.superpowers\sdd\restore-occluders-3133.ps1` |
| 旧连接 token 证据 | `.superpowers\sdd\acc-20261109\baidu-token-source.txt` |
| 交接文档 | `docs\handoff-2026-11-09-baidu-embedded-login.md` |

---

## 七、2026-11-09 更新：D2 已实证并修复（不再是盲改）

| 原假设 | 实测结论 |
|---|---|
| 「`/api/create` body 改成 `size=0&isdir=1&autoinit=1` 试试」（上节第 3 步） | 方向对但参数不对：真正必需的是 **`path` / `isdir=1` / `block_list=[]` 放在表单 body**，不需要 `size` / `autoinit` |

带/不带 body 的 A/B（同一 DPAPI 会话）：

| 请求 | 结果 |
|---|---|
| `POST /api/create`，空 body | `{"errno":2,"path":""}` ⇒ 与装机版「创建目录失败」现象一致 |
| `POST /api/create`，表单 body | `errno:0`，**真实创建** `/0000temp/zz-accept-3134` |

⇒ 空 body 就是 D2 的直接原因（`errno=2` 在社区约定里是「参数或协议层错误」，与装机版 toast 文案完全吻合）。

**修复落点**：`src-tauri/src/remote/baidu_api.rs` 新增 `create_dir_form_body(path)`；
`src-tauri/src/remote/baidu_provider.rs` 的 `create_dir` 改用它构造 body（query 里的同名参数保留作兼容）。

**同时确认**：服务端**没有**索引延迟 —— `GET /api/list?dir=/0000temp` 里能立刻查到新建的目录。
所以「创建后列表不出现」若再出现，方向是前端乐观更新/回读，而不是百度侧。

细节与证据见 `docs/handoff-2026-11-09-baidu-embedded-login.md` 的 **F.11**。

---

## 八、2026-11-09 结案：3.13.4（含 D2 修复）装机验收通过

| 验收项 | 结果 |
|---|---|
| 应用内新建远程目录 | ✅ 成功（对话框关闭 + 内容区/目录树/右侧路径三处同时出现） |
| 删除远程条目后列表即时更新 | ✅ ≤0.4 s 内消失，8 s 后回读也没带回来 |
| 删除是否真的落到网盘 | ✅ 需先点确认框「删除远程项目」里的「删除」，点完后服务端 `/api/list` 里条目消失 |
| 门禁 | `cargo test --lib` 1174 passed / 0 failed / 1 ignored；`eslint .` 177 errors（与基线一致） |
| 用户网盘残留 | 无（两个测试目录都已删除） |

**口径提醒（重要）**：远程删除是「**先乐观更新列表 → 再弹确认框 → 确认后才真删**」。
所以「列表立刻变了」只证明乐观更新生效，**不能**当成服务端已删除；必须再复核 `/api/list`。

完整证据与截图见 `docs/handoff-2026-11-09-baidu-embedded-login.md` 的 **F.12**。
