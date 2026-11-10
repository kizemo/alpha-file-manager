# 交接文档 · Focus Sync 3.13.5 发版收口（2026-11-10）

> 本文件记录「3.13.5 对外发布」这一轮的完整状态。前两轮背景见：
>
> - `docs/handoff-2026-11-10-focus-sync-installer-cleanup.md`（主安装器侧：删登录计划任务）
> - `docs/handoff-2026-11-10-focus-sync-standalone-installer.md`（扩展独立安装器对齐新生命周期）
>
> 本轮之前，代码改造与安装器对齐**已完成并提交**；本轮做的是「找 bug → 修 → 重新打包 → 对外发布」。

---

## 1. 结论先说

| 项 | 状态 | 证据 |
| --- | --- | --- |
| 改造是否完成 | **完成** | 主进程 spawn 侧车 + 退出回收 + 独立安装器清理 legacy 任务，三处均已落代码并过门禁 |
| 是否有 bug | **查出并已修 5 条**，3 条归为清理项未修 | 见 §4 |
| 站点是否已发 3.13.5 | **已生效** | `release:verify` 退出码 0，源站 + 边缘 sha256 全一致 |
| GitHub release | **已创建** | https://github.com/kizemo/alpha-file-manager/releases/tag/v3.13.5 |
| git tag `v3.13.5` | **已打并推送**，落 `778c3455` | `git rev-parse 'v3.13.5^{commit}'` = `778c3455…` |
| main 是否推送 | **已推送** | `origin/main...main` 同步，0 ahead |
| 真机装机验收 | **未做** | 见 §6「未验证」 |

安装包：`Alpha_File_Manager_3.13.5_x64-setup.exe`，45,407,445 字节，
sha256 `c5b09252c42f7d1b078623c0c0855c06eda316bb6d1d536f8bbd19556e888502`。

---

## 2. 代码改造的最终形态（读代码时可对照）

- 侧车（Focus Sync 助手）**不再由登录计划任务拉起**，改由 AFM 主进程在启动时 spawn、
  退出时回收：`src-tauri/src/focus_sync_sidecar.rs` + `src-tauri/src/lib.rs`（`:609` stop / `:661` start）。
- 侧车命令行只剩 `--port 37421`，**不读注册表**、不自己找安装路径。
- **自愈**：新增 `src-tauri/src/focus_sync_supervisor.rs`（纯 `std`，不引用任何 `tauri` 类型），
  每 10 秒心跳，崩了就重启，连续最多 3 次；稳定运行 60 秒后计数清零。
- **legacy 清理**：主安装器与扩展独立安装器都会在**动二进制之前**删掉 `KizemoFocusSync`
  计划任务，并等旧进程真正退出后再替换文件。
- **交接闸门**：侧车 `state.rs::afm_handoff_allows` 改为 fail-closed ——
  `handoff.pid == 0 || dialog_pid == 0` 一律 `return false`，且下一 tick 重采样重试。

⚠️ **`focus_sync_supervisor.rs` 必须保持「不碰 tauri」**：`cargo test --lib` 的测试二进制
一旦链接到 `tauri::AppHandle` 或 `tauri::async_runtime::*`，会在**加载阶段**以
`STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉，一个测试都跑不了。这是本仓的红线。

---

## 3. 门禁实跑数据（本轮，全部为真实输出）

| 门禁 | 结果 | 日志 |
| --- | --- | --- |
| `npm run test:unit:run` | 299 files / **2840 passed / 0 failed** | `sigma-file-manager/.superpowers/gate-vitest.log` |
| `npm run ts` | exit 0 | `.superpowers/gate-ts.log` |
| `npm run lint:eslint:check` | **177 errors / 0 warnings**（与 HEAD 基线一致，未新增） | `.superpowers/gate-eslint.log` |
| `cargo test --lib` | **1174 passed / 0 failed / 1 ignored** | `.superpowers/sdd/test-lib.log` |
| 侧车单测 | **112 passed / 0 failed**，`EXIT=0` | `.superpowers/sdd/test-sidecar.log` |
| supervisor 单测 | **4 passed / 0 failed**（1175 filtered out） | `.superpowers/sdd/test-supervisor.log` |
| 安装器门禁 | **24/24 passed** | `npx vitest run src-tauri/installer/__tests__/focus-sync-standalone-installer.test.ts` |
| 侧车重编 | `BUILD_OK`，release 47.80s | `.superpowers/sdd/build-sidecar.log` |
| tauri build | `Finished release [optimized] in 3m 35s` | `.superpowers/sdd/tauri-build-3135b.log` |

---

## 4. verifier 只读复核：判 FAIL，本轮修了 5 条

复核范围：当时 8 个未推送提交（`c56b029f`..`1b620000` 一带）。

### 已修（全部在 `778c3455`，8 files / +622 / −81）

1. **（高危）安装器顺序断言误绿** —— `src-tauri/installer/__tests__/focus-sync-standalone-installer.test.ts:120-128`
   原来用「子串在全文中的位置」判先后，换行/缩进一变就能假通过。改为正则
   `powershell[^\r\n]*-File\s+"\$INSTDIR\\bin\\stop-sidecar-processes\.ps1"` 逐行锚定。
2. **（高危）侧车丢失自愈能力** —— legacy 计划任务有 `-RestartCount 3`，改写后只有 start/stop。
   新建 `focus_sync_supervisor.rs` 补回（10s 心跳 / 最多 3 次 / 60s 后清零），4 条单测覆盖。
3. **清理脚本漏规范路径且不等待** —— `extensions/kizemo.focus-sync/installer/remove-legacy-focus-task.ps1`
   patterns 补 `*\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`；
   `schtasks /End` 后加 **10 秒有界轮询**，确认进程真的退出。
4. **版本号硬编码 + 门禁正则大小写敏感** —— `register.ps1:70` 删掉写死的 `'0.5.9'`，
   改为「部署态 `package.json` → `..\package.json` → 仍无则 `FATAL` + `exit 1`」；门禁正则加 `i`。
5. **闸门 pid 取不到就整体放行** —— `sidecar/src/state.rs::afm_handoff_allows` 改 fail-closed；
   `sidecar/src/uia_event.rs` 增加 `handoff_retry_pending`，丢 pid 时下一 tick 重采样。新增 5 条单测。

### 未修，登记为清理项（发版前不动安装器内部行为）

| # | 位置 | 问题 | 为什么不修 |
| --- | --- | --- | --- |
| A | `src-tauri/tauri.conf.json:87-88` | 仍打包 `register-scheduled-task.ps1`（已停用） | `hooks.nsh:1219` 卸载段**仍在执行** `tools\unregister-scheduled-task.ps1`；移除资源会让卸载步骤找不到文件。要改必须连同卸载段一起改，且**无法在发版前真机验证卸载路径** |
| B | `.gitignore:37` | 整目录忽略 `extensions/kizemo.focus-sync/bin/` | 干净 checkout 无法直接产出安装包（2.8 MB 二进制不入库）。是否入库未决，倾向保持现状 |
| C | `hooks.nsh:1092/1095` | 残留的删除行（脚本已不在 resources 里） | 无害空操作；`hooks.nsh:604` 与 `kill-app-tree.ps1:267-285` 的 schtasks 禁用也重复，同样无害 |

> `installer/focus-sync-sidecar.exe` 曾是旧件（复核提过），**本轮已解决**：侧车重编后
> 三份副本哈希一致 —— `sidecar/target/release/spike.exe` =
> `extensions/kizemo.focus-sync/bin/focus-sync-sidecar.exe` =
> `extensions/kizemo.focus-sync/installer/focus-sync-sidecar.exe`，
> 均 2,873,344 B、sha256 前缀 `A8300AAB240F`。

---

## 5. 发布链路实况（照这条顺序复现）

```powershell
# 1) 侧车重编（改了 sidecar 源码就必须做，产物要刷三份副本）
F:\soft\00selfmade\filemanager\.superpowers\sdd\build-sidecar.cmd      # 日志末行 BUILD_OK

# 2) 重新打包（改了 Rust / 安装器就必须做）
F:\soft\00selfmade\filemanager\.superpowers\sdd\build-3135b.cmd        # 日志 tauri-build-3135b.log

# 3) 刷新站点清单（--bump none 表示版本号已 bump 好，别再加一版）
Push-Location F:\soft\00selfmade\filemanager\sigma-file-manager
npm run release:prepare -- --bump none --skip-build --notes-file "F:\soft\00selfmade\filemanager\sigma-file-manager\_notes-3.13.5.md"
Pop-Location

# 4) 先传安装包，再传页面与 version.json（顺序不能反，否则 sha256 不匹配 → 应用会删下载文件）
& 'E:\办公文件\L1网站\.venv-sftp\Scripts\python.exe' 'F:\soft\00selfmade\filemanager\.superpowers\sdd\upload-one.py' "E:\办公文件\L1网站\alpha-file-manager\Alpha_File_Manager_3.13.5_x64-setup.exe"
& 'E:\办公文件\L1网站\.venv-sftp\Scripts\python.exe' "E:\办公文件\L1网站\alpha-file-manager\_upload_alpha_files.py" --files-only

# 5) 公网回读（Node 走系统代理）
$env:NODE_USE_ENV_PROXY='1'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; $env:HTTP_PROXY='http://127.0.0.1:7897'
npm run release:verify            # VERIFY_EXIT=0

# 6) GitHub 兜底 release（notes 默认取站点源 _release-notes-v<版本>.md）
npm run release:github -- --tag v3.13.5

# 7) 推 main 与 tag（tag 必须落在实际发版提交上）
git -C F:\soft\00selfmade\filemanager\sigma-file-manager push origin main
git -C F:\soft\00selfmade\filemanager\sigma-file-manager tag -a v3.13.5 778c3455 -m "Alpha File Manager v3.13.5"
git -C F:\soft\00selfmade\filemanager\sigma-file-manager push origin v3.13.5
```

**本轮实测结果**

- `PREPARE_EXIT=0`。`latestInstaller` 同时扫 `release/` 与 `release/local-builds/` 按 mtime 取最新，
  因此自动选中新包；`release/` 里那个旧的 `Alpha File Manager_3.13.5_x64-setup.exe`
  （45,482,899 B / `a94f20a6…`，未含 Rust 修复）**没有被删**，只是不再被选中。
- 站点 `version.json`：`version=3.13.5`、`sha256=c5b09252…`、`pub_date=2026-11-10T05:10:02Z`、
  `history_count=20`、`history[0]=3.13.4`（同版本重跑**不会**重复归档，见 `release-publish.mjs:271`）。
- 上传 ① 安装包 `REMOTE_MATCH`（远端 md5 `604876f739fcb7fa2ad05ef74aaf68b8`，size 一致）；
  ② 4 个页面文件（`index.html` 5792 B / `.htaccess` 1449 B / `version.json` 55119 B / `download.php` 3124 B）
  服务器端 md5 + size 全部 OK；③ `release:verify` 四项一致，`CF-Cache-Status=MISS`，源站与边缘都已生效。
- `GH_EXIT=0`：GitHub 上原无 `v3.13.5`，本次 `gh release create` 并上传 45,407,445 B 资产。
  发布说明取自新建的 `E:\办公文件\L1网站\alpha-file-manager\_release-notes-v3.13.5.md`（2417 B）。

---

## 6. 已验证 / 推断 / 未验证

**已验证（有实跑输出）**
- 全部门禁数字（§3）、三个二进制的哈希一致、站点与 GitHub 的公网回读、tag 落点、main 已推送。

**推断（未直接观测，但依据明确）**
- `hooks.nsh` 卸载段仍引用 `unregister-scheduled-task.ps1` ⇒ 该资源不能从 `tauri.conf.json` 移除（依据是文件原文）。
- `latestInstaller` 按 mtime 取最新 ⇒ 旧包不会被误发（依据是 `release-publish.mjs:132-160` 源码）。

**未验证（本轮的空白）**
- **真机装机验收**：安装器行为改动（删任务时机、等进程退出、独立安装器清理）只有代码与
  单测证据，没有「装一遍、看端口/进程/计划任务」的运行时证据。
- 侧车自愈的**真机**表现（杀掉进程后 10 秒内是否被拉起）只有 4 条注入式单测，没有实机杀进程验证。
- 3.13.5 安装包在**已装 3.13.4 的机器**上覆盖升级的完整路径未实测。

> ⚠️ `tauri dev` 不能用于界面验收（所有路由页面内容区空白，dev 构建特有）。
> 要做验收只能装 `tauri build` 的打包产物。

---

## 7. 本轮新增的工具坑

- **`Get-ChildItem -Filter '_notes-3.13.5.md'` 会返回空**，而文件确实存在（1397 B）。
  换成 `-Filter '_*3.13.5*' -Recurse` 才找到。判「文件不存在」前先换一种写法复核。
  （同类：`Get-ChildItem -Recurse` 对空目录返回空行，`cmd /c dir /b` 更可靠。）
- **`git commit -F -` 不吃 stdin**：必须把信息写进临时文件再 `-F <file>`（本轮沿用）。
- **`.cmd` 运行期间不能编辑**：cmd.exe 按字节偏移逐行读脚本，中途改文件会让后续行错位、
  执行到别处的指令片段（历史上有过把版本号从 3.6.0 顶到 3.7.0 的代价）。要改就换新文件名。
- **后台构建不要挂 stdout 管道**（`Tee-Object` / `| Select-String`）：管道一断进程树跟着死。
  让 `.cmd` 内部 `> logfile 2>&1`，再单独读文件。
- **日志长时间零增长不等于失败**：Rust 的 codegen / link 阶段 2~3 分钟没有输出，
  判活性看 `cargo`/`rustc` 进程与 `target/release/*.exe` 的 mtime。

---

## 8. 外层仓 `F:\soft\00selfmade\filemanager` 的状态（需用户决策）

- `origin` 指向 **`https://github.com/kizemo/focus-sync.git`** —— **是另一个项目**，
  远端只有 `e649324` / `70c6f7d` 两个 Focus Sync 0.5.8 / 0.5.9 提交。
- 本地 `main` 相对 `origin/main`：**ahead 84 / behind 2**，且 **`merge-base` 为空**（历史不相交）。
  ⇒ 既不能快进，也不能 rebase；force push 会覆盖远端另一项目的提交。
- 本轮**未推送、未提交**外层仓，保持原状。
- 未跟踪文件（非本轮产生，未处理）：`docs/SFM-EXTENSION-DEVELOPMENT-GUIDE.md`（44,830 B）、
  `docs/blog/`、`docs/marketing/`（13 文件含 2 PNG）、`vt3.txt`（249 B）。

**建议**：给外层仓配一个正确的远端（或新建仓库），再决定这 84 个提交的去向；
`vt3.txt` 看起来是临时文件，入库前应删。

---

## 9. 下一步建议（优先级从高到低）

1. **真机装一次 3.13.5**（从 3.13.4 覆盖升级）：确认
   - 安装后 `Get-ScheduledTask -TaskName 'KizemoFocusSync'` 为空；
   - 启动应用后侧车进程在、退出应用后侧车进程消失；
   - 杀掉侧车进程，10 秒内被重新拉起（自愈）。
2. 决定清理项 A（`tauri.conf.json:87-88` + `hooks.nsh:1219` 一起改）、B（`bin/` 是否入库）。
3. 外层仓远端归位（§8）。
4. 若下版本要收尾，`hooks.nsh:1092/1095` 的残留删除行与 `:604` 的重复 schtasks 禁用可一并清掉。

---

## 10. 关键文件索引

| 用途 | 路径 |
| --- | --- |
| 侧车生命周期 | `src-tauri/src/focus_sync_sidecar.rs`、`src-tauri/src/lib.rs:41/609/661` |
| 自愈监督 | `src-tauri/src/focus_sync_supervisor.rs`（纯 std，勿引入 tauri） |
| 交接闸门 | `extensions/kizemo.focus-sync/sidecar/src/state.rs`、`.../uia_event.rs` |
| 主安装器钩子 | `src-tauri/installer/hooks.nsh`（`:584`/`:604`/`:1056`/`:1092`/`:1219`） |
| 独立安装器 | `extensions/kizemo.focus-sync/installer/installer.nsi`、`register.ps1`、`remove-legacy-focus-task.ps1` |
| 安装器门禁 | `src-tauri/installer/__tests__/focus-sync-standalone-installer.test.ts`（24 条） |
| 打包资源 | `src-tauri/tauri.conf.json:81-94` |
| 发布脚本 | `scripts/release-publish.mjs`（`:132-160` 选包、`:271` 归档、`:434-481` github） |
| 站点源 | `E:\办公文件\L1网站\alpha-file-manager\`（`version.json`、`_release-notes-v3.13.5.md`） |
| 构建脚本 | `.superpowers/sdd/build-sidecar.cmd`、`.superpowers/sdd/build-3135b.cmd` |