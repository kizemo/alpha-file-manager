# 批次 18 交接卡 · focus sync 空路径契约（R1）＋ 四个次生缺陷 ＋ 版本 3.14.1 ＋ 装机链判别性实验

- 生成时间：2026-11-18
- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`（分支 `main`）
- 台账真源：`docs/handoff-2026-11-11-quark-pan.md` **§20.13**（本卡是入口卡，详细论证看台账）
- 上一卡：`docs/handoff-2026-11-18-quark-pan-batch17.md`（批次 17 = 台账 §20.12）

---

## 一、背景与目标

批次 17（台账 §20.12）修掉了 focus sync 的「闸门恒拒」与「日志刷屏」，并完成端到端真机验收（路径真的写进了对话框）。
它留下 5 项遗留，本批次逐项处理：

| # | 遗留项 | 本批次处理 |
| --- | --- | --- |
| ① | `git push origin main` 未做 | **已完成并核验**（本地 HEAD = `origin/main` = 远程 `refs/heads/main`） |
| ② | 装机链未验证（台账 §20.12.7 未验证项⑦：NSIS 包是否携带新侧车、装机是否刷新 APPDATA 旧侧车） | **本批次核心实验**（见第四节） |
| ③ | 路径 A「祖先后代分支」真机覆盖 | **仍登记未验证**，本轮不安排触发时序 |
| ④ | §20.11.7 登记的 4 个次生缺陷 | **已修**（见第二节表） |
| ⑤ | 版本号 + 全套发版材料 | **已做**（3.14.1，见第三节） |

**范围声明**：只做**仓库版本与发版材料**，**不做对外发布**（GitHub Release / winget PR 需用户授权）。
未触碰用户真实数据。

---

## 二、这批次改了什么

### 2.1 R1：空路径契约（根因修复）

**现象**：扩展停在虚拟位置（收藏/最近/「此电脑」等非真实目录）时仍推虚拟路径；侧车 `/set_path` 对空路径走 400
`path is empty` 且**同步阻塞约 400 ms**；扩展按旧契约把这次 400 当成 `ok:true` ⇒ 「路径已送达」是假阳性。

**定稿路线**：**空路径 = 显式清空侧车记忆路径**（明确**不用**「推虚拟路径」回退）。

| 位置 | 改动 |
| --- | --- |
| `sidecar/src/state.rs` | 新增 `AppState::clear_current_path()`：只清 `current_path`，**不动 `first_push_received`** |
| `sidecar/src/http_server.rs` | `/set_path` 空路径 → 记 `set_path_cleared` 事件 → 回 **200 `{"ok":true}`**；不再 400、不再等 400 ms 对话框探测；`SPIKE_VERSION` `0.6.0` → `0.6.1` |
| `extensions/kizemo.focus-sync/dist/index.js` | 新增 `sendClearToSidecar()`（POST `/set_path`, body `{"path":""}`）；`pushNow()` 遇非本地路径时主动清空并返回 `{ok,status,reason,wrote}`；toast 诚实化（200/404/disabled/error 各有措辞）；`N15.repush.after_restart` 改用 `traceReplace`；版本串 `v0.5.5` → `v0.6.2` |

**安全前提**（为什么可以放心清空）：侧车写回闸门要求 `is_writable_local_path(&current)`；`current` 为空 ⇒ `path_ok=false`
⇒ 落 `opened_write_back_skip_non_local_path` 分支，**一个字节都不写**（`main.rs:420` 附近）。

**口径**：侧车 200 **只代表「路径已送达/已清空记忆」**，**不代表已写进对话框**（对非前台弹窗只登记不写）⇒ toast 必须如实表述。

### 2.2 §20.11.7 的四个次生缺陷

| # | 缺陷 | 修法 |
| --- | --- | --- |
| A | commit 槽泄漏（写失败后不归还 ⇒ 一次失败烧掉该对话框的提交资格） | `writer.rs` v0.5.23：`try_mark_committed` 先占槽后写入；写失败且**本次确实占到了槽**才 `unmark_committed` 归还 |
| B | `/set_path` 空路径 400 ms 阻塞 + 假 `ok:true` | 见 2.1（空路径 → 立即 200 明确清空信号） |
| C | `__focus_sync_trace` 被 health 轮询刷爆 | `index.js`：trace 采样改有界替换 `traceReplace`；`N15.repush.after_restart` 改用 `traceReplace` |
| D | 虚拟路径仍推送 | `index.js`：非本地路径改发清空信号（`sendClearToSidecar`），留标记串 `non-local-path` |

**已知收益边界**：`should_commit` 被忽略、`commit_save_button` 是死代码 ⇒ 缺陷 A 的修复**无害但用户可见收益不可观测**。

---

## 三、门禁与产物

| 门禁 | 结果 | 证据 |
| --- | --- | --- |
| `cargo test`（1.90.0 toolchain） | **138 passed / 0 failed**（基线 135，+3） | `.superpowers\sdd\sidecar-test-r1.log` |
| `node scripts/scan-sandbox-dynamic.cjs` | `RESULT: VALID (0 violations / 20 patterns)` | stdout |
| `node scripts/verify-extension-sandbox-gate.mjs` | PASS/OK | stdout |
| `node --check dist/index.js` | SYNTAX_OK | stdout |
| 侧车重建 `scripts\release\build-sidecar.cmd` | `BUILD_OK` | `.superpowers\sdd\build-sidecar.log`（旧日志备份 `build-sidecar-b17.log`） |
| `npm run check`（并发 `run-p`） | **不可靠**：三轮红三种红法（详见「第五节 · 门禁备注」） | `.superpowers\sdd\check-b18.log`、`check-b18b.log` |
| 同一批 5 脚本**串行**跑（`.superpowers\sdd\run-check-b18c.cmd`） | **五步全绿**：`ts`=0、`lint:check`=0、`test:unit:run`=0（**311 文件 / 3241 用例全过**）、`verify-changelog`=0、`verify:sandbox`=0 | `.superpowers\sdd\g-{ts,lint,test,changelog,sandbox}.log` |
| `npm run tauri:build` | `TauriBuildExit=0`（cargo 6m23s ＋ makensis） | `.superpowers\sdd\build-afm-b18.log` |
| 安装包载荷校验 | `VERIFY_OK reason=payload-carries-expected-sidecar-and-new-extension` | `.superpowers\sdd\payload-20261118-151156\` |

**侧车新二进制**（本轮新件）：sha256 **`74E5957F5328BF1AFE679F075E16C73691307AB5F6435B2C6BC537FAE7721CCF`** / 2,895,360 B
（旧件 `552C85C4…735FD8` 作废）。

**版本 3.14.1 与发版材料**：`package.json` + 3 个 `Cargo.toml`（`node ./scripts/sync-version.js` ⇒ `Synced 3 Cargo manifests to 3.14.1`）；
根 `CHANGELOG.md` + **14 个 locale changelog** + `README.md` + `README.zh-CN.md` + `_notes-3.14.1.md`。
locale 校验：`node .superpowers\sdd\check-locales-3141.mjs` ⇒ 14 PASS + `LOCALES_OK`；`node .superpowers\sdd\verify-locale-changelog.mjs` ⇒ 14/14 PASS。
根 CHANGELOG / `_notes` 补了 R1 细则，**14 locale 未同步该句**（措辞仍准确）。

---

## 四、判别性装机实验（台账未验证项⑦）

**要证明什么**：NSIS 安装包**携带**新侧车，且安装会把 APPDATA 里的**旧侧车刷新成新侧车**。

**判据（两段式，缺一不可）**：

| 阶段 | 判据 |
| --- | --- |
| 装机前 | APPDATA 侧车 = **旧件 `A8300AAB240F468A8E27BDCA0DD0E8079DAD642F60B53BA5C8ADD3C47D52E462`**（3.14.0 装机件，2,873,344 B） |
| 装机后 | APPDATA 侧车 = **新件 `74E5957F…21CCF`** ＋ 装机目录 `D:\Program Files\Alpha File Manager\tools\focus-sync-sidecar.exe` 同为新件 |

- **只判「安装后 sha == 期望值」是假阳性**（两边本来就都该是新件）⇒ 必须加「安装前确实是旧件」的前置。
- 流程：`prepare-old-sidecar.ps1`（前提不成立就 exit 2）→ `install-afm-verify.ps1 -Setup <3.14.1 setup> -ExpectedSidecarSha256 74E5957F…21CCF`
  → 期望 `VERIFY_OK reason=install-refreshed-appdata-sidecar`。
- **安装器静默模式不会自动拉起 AFM** ⇒ 无抢焦点风险，**文件级 sha 判定即足够**，不需要真机 UI 取证。

**实验结果（2026-11-18 15:14–15:15，本轮实测）**：

| 阶段 | 观察值 |
| --- | --- |
| 装机前 · 装机目录 `tools\focus-sync-sidecar.exe` | `A8300AAB…E462`（3.14.0 旧件） |
| 装机前 · APPDATA 侧车 | `A8300AAB…E462`（由 `prepare-old-sidecar.ps1` 从批次 17 件 `0DC1B563…E8D` 回退成旧件） |
| 安装 | `INSTALL_EXIT 0`；日志 `PREINSTALL installerVersion=3.14.1 detectedVersion=3.14.0` → `POSTINSTALL` |
| 装机后 · 装机目录 | `74E5957F…21CCF`（新件） |
| 装机后 · APPDATA 侧车 | `74E5957F…21CCF`（新件） |

⇒ **`VERIFY_OK reason=install-refreshed-appdata-sidecar`** ⇒ **批次 17 未验证项⑦关闭**。

**同轮载荷校验**（`verify-installer-payload.ps1`，解包目录 `.superpowers\sdd\payload-20261118-151156\`）：
安装包 sha256 `EF5D6DE0EAAF3E6EB43F206F014728F1BF907ABADD6996CF8CD6E69807AC0C23`（45,478,832 B）；
包内侧车 sha = `74E5957F…21CCF`（match=True）；包内扩展 `dist\index.js` sha = `93439CAE…7CF5E8`，
标记串 `traceReplace` / `non-local-path` 齐全（markers_ok=True）⇒ **新二进制 + 新扩展**两侧都对。

**安装包位置**：`release\Alpha File Manager_3.14.1_x64-setup.exe`
（`publish-bundle-to-release.mjs` 对**同名但内容不同**的历史包会改写到 `release\local-builds\`，3.14.1 是新版本号 ⇒ 直接落在 `release\` 根）。

---

## 五、已验证 / 推断 / 未验证矩阵

| # | 结论 | 状态 |
| --- | --- | --- |
| ① | 空路径 `/set_path` = 显式清空 + 200，不再 400、不再阻塞 | **已验证** |
| ② | `clear_current_path` 不动 `first_push_received` | **已验证** |
| ③ | 清空后写回闸门必然 skip，不会误写 | **已验证** |
| ④ | commit 槽写失败归还（缺陷 A） | **已验证** |
| ⑤ | 扩展侧语法/沙箱自检通过 | **已验证** |
| ⑥ | 侧车新二进制可产出且 sha 稳定 | **已验证** |
| ⑦ | 装机器会把旧侧车刷新成新侧车 | **已验证（见第四节）** |
| ⑧ | 虚拟位置停留时的端到端真机体验（toast 文案、确实忘掉旧目录） | **未验证（本轮未做真机 UI）** |
| ⑨ | 路径 A 祖先后代分支真机覆盖 | **未验证（沿用批次 17）** |
| ⑩ | path_update 与侧车双写去重 | **本轮只分析不实现** |
| ⑪ | `should_commit` / `auto_confirm` 不门控任何行为 | **已知，收益不可观测** |

**门禁备注**：`npm run check`（`run-p` 并发）在本机连红三轮，红法各不相同，**全部与本轮改动无关**：

| 轮 | 跑法 | 红项 | 单文件复跑 |
| --- | --- | --- | --- |
| ① | 并发 | `binary-health-check.test.ts` hook 10 s 超时（3239 passed / 1 failed） | 2 passed / 2.06 s |
| ② | 并发 | **stylelint 零报错退出 1** ⇒ run-p 连带杀掉 vitest ⇒ 日志里连测试总结都没有 | stylelint 单跑全绿 |
| ③ | 串行 | `home-banner-click-opens-background-manager.test.ts` test 30 s 超时（3240 passed / 1 failed） | 3 passed / 24.74 s（jsdom 环境占 18.23 s ⇒ 并发下必撞 30 s `testTimeout`） |
| ④ | **串行复跑** | **无红项：311 文件 / 3241 用例全过** | — |

⇒ **判据**：偶发红项先单文件复跑；**连续命中不同文件**即判定为仓库既有的负载敏感 flake。
⇒ **未做（建议留到下一批）**：给这两个测试文件显式 `hookTimeout` / `testTimeout`，或让 `test:unit:run` 降并发。

---

## 六、证据路径

| 内容 | 路径 |
| --- | --- |
| 台账真源（§20.13） | `docs/handoff-2026-11-11-quark-pan.md` |
| 本卡 | `docs/handoff-2026-11-18-quark-pan-batch18.md` |
| 侧车测试日志 | `.superpowers\sdd\sidecar-test-r1.log` |
| 侧车构建日志 | `.superpowers\sdd\build-sidecar.log` |
| 前端门禁日志 | `.superpowers\sdd\check-b18b.log`、`.superpowers\sdd\vitest-binary-health.log` |
| 打包脚本 / 日志 | `.superpowers\sdd\build-afm-b18.cmd`、`.superpowers\sdd\build-afm-b18.log` |
| 载荷校验脚本 | `.superpowers\sdd\verify-installer-payload.ps1` |
| 装机实验脚本 | `.superpowers\sdd\prepare-old-sidecar.ps1`、`.superpowers\sdd\install-afm-verify.ps1` |
| locale 校验脚本 | `.superpowers\sdd\check-locales-3141.mjs`、`.superpowers\sdd\verify-locale-changelog.mjs` |
| 本轮新增脚本 | `.superpowers\sdd\run-sidecar-test-r1.ps1`、`.superpowers\sdd\build-sidecar-r1.cmd`、`.superpowers\sdd\run-check-b18.cmd`、`.superpowers\sdd\run-check-b18b.cmd` |

---

## 七、下一步与遗留

1. **对外发布需用户授权**：GitHub Release、winget 3.14.1 清单与 PR
   （`docs/winget/manifests/k/kizemo/AlphaFileManager/3.14.0/` 已存在；AGENTS 记录 `#467381`(3.9.1) 仍 OPEN，**同包 ID 不能并行提 PR**）。
2. **path_update 与侧车双写去重**：本轮只分析不实现，留作下一批。
3. **路径 A 祖先后代分支真机覆盖**：需要构造「对话框属主是接收方 helper 子进程」的时序，本轮未安排。
4. **虚拟位置端到端真机体验**（toast 文案 + 侧车确实忘掉旧目录）：本轮只做文件级与单测级判定。
5. **`npm run check` 的 10 s hook 超时**：若后续频繁出现，应给该测试文件加显式 `hookTimeout`，而不是每次手工复跑。

---

## 八、工具坑

1. `npm run check` 全量并发下 `binary-health-check.test.ts` 偶发 10 s hook 超时 ⇒ 单独复跑判真伪。
2. `cd` 之后写文件会被安全策略拦 ⇒ 跨目录写操作一律先写 `.cmd` / `.ps1` 再执行。
3. bash 里 `$` 会被先展开 ⇒ 内联 `powershell -Command "$p=…"` 会变空串报 ParserError；含变量的 PowerShell 一律落 `.ps1`。
4. 本机 bash 是 PowerShell ⇒ `cd /d "F:\…"` 非法，用 `Set-Location -LiteralPath 'F:\…'`。
5. `cargo` 不在默认 PATH，`stable` 1.97.1 的 std 不可用（E0463）⇒ 用
   `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin\cargo.exe`；**不要与 `npm run check` 并发**。
6. 同文件并行 edit 会互相覆盖 ⇒ 源码改动单文件串行。
7. `is_writable_local_path` 要求目录**真实存在** ⇒ 单测里不能编造 `D:\stale` 这类路径，用盘符根 `C:\`。
8. 侧车 `/set_path` 返回 200 只代表「路径已送达/已清空记忆」，不代表已写进对话框。
9. `eslint.config.ts` 全局忽略 `**/dist/**` ⇒ 对 `dist/index.js` 跑 eslint 必然 exit 1（基线事实，非回归）。

---

## 九、接力 prompt

见本会话最终回答正文（按用户偏好，prompt 直接写在回答里，不另落文件）。
