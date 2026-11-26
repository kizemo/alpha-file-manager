# Alpha File Manager · 夸克网盘/focus-sync 批次 21 · 交接卡

- 生成时间：`2026-11-18`（批次 21 收尾）
- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`
- 本卡覆盖范围：批次 20 → 批次 21
- 版本：`3.14.1`（**不换版本号，原地重建**，理由见 §三.1）
- 台账真源：`docs/handoff-2026-11-11-quark-pan.md`。批次 21 = §20.16，批次 20 = §20.15，批次 19 = §20.14。
- 入口配方：`AGENTS.md` §2.8（真机验收须隔离桌面）、§2.10（禁用三类假判据）+ 批次 20 入口卡 `docs/handoff-2026-11-18-quark-pan-batch20.md`

> 本轮唯一任务 = 批次 20 遗留项 **⑬**：重打 3.14.1 装机件 + 算 sha + 更新 winget 清单 + `check:winget` + 重跑装机链复验。
> **⑬ 已闭合（含装机侧证据）**。⑪（真机计数级）、⑤（争用源）不在本轮范围，状态见 §五。

---

## 〇、结论一览（本轮实测，全部有落盘证据）

| 判据 | 结果 |
|---|---|
| 新包 | `Alpha File Manager_3.14.1_x64-setup.exe` **45,489,382 B**，sha256 `5F07F5B914DFEF9FBF22E040BC3C2B67D13167B3EBA633B6A20C5BCB47F88E58` |
| 包内载荷 = v0.6.4 修复侧车 | `VERIFY_OK reason=payload-carries-expected-sidecar-and-new-extension`（侧车 sha `480FDE62…06CF` match=True；扩展 manifest+dist 在位、`dist/index.js` markers_ok=True） |
| winget 清单 | `InstallerSha256` 已改为 `5F07F5B9…`；`npm run check:winget` ⇒ **`WINGET_CHECK_EXIT=0`**，8 条全绿 |
| 装机链 | `INSTALL_EXIT=0` + `install-afm-verify.ps1` ⇒ `VERIFY_OK reason=install-refreshed-appdata-sidecar`；AppData 侧车 sha `480FDE62…` = v0.6.4 |
| 装机 app exe 就是新构建载荷 | `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`：`SIZE_EQUAL=True`、**`NORM_EQUAL=True`**、`RAW_EQUAL=False` ⇒ `VERDICT_INSTALLED_IS_NEW_PAYLOAD=True` |
| 未闭合 | ⑪ 真机计数级 `w1=1 w2=0`（v0.6.4 上从未取到有效数据）；GitHub Release 资产 + winget PR = 对外动作，需用户授权 |

---

## 一、接手时的状态（继承批次 20，**含本轮两处更正**）

| 项 | 批次 20 结束时的状态 | 本轮更正 |
|---|---|---|
| 焦点修复代码 | 已提交：`5b0db31f` 侧车写入闸门 v0.6.4 + `run-p`→`run-s` + 13 locale changelog；`18f54134` 台账 §20.15 + 批次 20 入口卡 | — |
| 版本 / 门禁 | `3.14.1`；`npm run check` `GATE_EXIT=0`（vitest 311 files / 3243 tests） | — |
| 发版材料 | `release/` 顶层 3.14.1 包**不含**批次 20 的 v0.6.4 闸门 ⇒ 账面与代码不一致 | **更正 A**：该包实测是**批次 19 产物** `45,478,740 B / 1DD82EEED36127F21595FFB852FF1C65474498B44A99995CEB3B00BA3F6D2A83`（mtime 2026-11-18 16:43:01），**不是**批次 20 卡里写的 `82,772,736 B / 554D0564…` |
| winget 清单 | `InstallerSha256` 指向批次 19 的包 | 真实路径是 `docs\winget\manifests\k\kizemo\AlphaFileManager\3.14.1\kizemo.AlphaFileManager.installer.yaml`（**不是** `docs/winget/com.sigma-file-manager.installer.yaml`；键名是 `InstallerSha256`，不是 `Sha256`） |
| **装机侧车** | 批次 20 判定「AppData 装机侧车仍是旧版 `A9478986…` ⇒ 装机版不含修复」 | **更正 B（重要）**：本轮装前备份实测 sha **已经是 `480FDE62…` = v0.6.4**，AppData 只有一份侧车、mtime `2026-11-18 20:47:58` ⇒ 批次 20 之后（20:47）那次装机已把侧车刷新过。**「装机版不含修复」在侧车层早已不成立，真正的缺陷只在「安装包 + 清单账面」层** —— 也就是 ⑬ 本身。下一轮不要再去修「装机侧车是旧的」这个已消失的问题。 |
| 真机级证据 | ⑩ 扩展 toast PASS（`b20-toast5-run3.log` `VERDICT=PASS toasts=1 tip=True toaster=True`）；⑪ `UNDETERMINED` | — |

---

## 二、构建环境配方（**本轮证伪了批次 21 中途写的错误版本，以此节为准**）

1. **必须显式用 rust `1.90.0` + 必须先 `call vcvars64.bat`**。历次成功构建（b5…b20）逐字相同：
   ```bat
   @echo off
   cd /d "F:\soft\00selfmade\filemanager\sigma-file-manager"
   set "LOG=%~dp0b21-final.log"
   call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" > nul 2>&1
   set "PATH=C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;%PATH%"
   call npm run tauri:build >> "%LOG%" 2>&1
   echo BUILD_EXIT=%ERRORLEVEL% >> "%LOG%"
   echo ALL_DONE >> "%LOG%"
   ```
   已落盘 `.superpowers\sdd\b21-final.cmd`，本轮 `BUILD_EXIT=0`。启动姿势（已验证）：
   `Start-Process cmd.exe -ArgumentList '/c','<绝对路径 .cmd>' -WindowStyle Hidden -PassThru`
2. **`stable-x86_64-pc-windows-msvc`（rustc 1.97.1）编不动本项目发布版** ⇒ 在 `log v0.4.34` 爆 **251 个 `E0463 can't find crate for std`**。批次 21 中途那版「prepend stable 工具链」的脚本（`.superpowers\sdd\b21-run-build.ps1`）**已证伪、作废，不要抄**。
3. `C:\Users\Duanyi\.rustup\bin` 是**空的**（没有 rustup shim）；`.cargo\bin` 只有 `cargo-tauri.exe`；`toolchains` 下只有 `1.85.0 / 1.90.0 / stable`；仓库**没有** `rust-toolchain*`、**没有** `.cargo\config.toml`。⇒ 不 prepend 路径就没有 `cargo.exe`，但**正确的那个是 1.90.0**。
4. `E:\Users\Duanyi\.minimax\.minimax\maintenance\ps-prebuild.ps1` **不存在**，记忆里那条已失效。
5. bash 工具权限坑：`Set-Location` + 相对路径写文件会被 `Local hard safety policy blocked bash: write/delete targets cannot be verified safely…` 挡掉 ⇒ 脚本用绝对路径写，内部再 `Set-Location`，用 `pwsh -NoProfile -File <绝对路径>` 跑。
6. 多行 `pwsh -Command '<整段脚本>'` 在本机偶发**静默返回空输出 + exit 1**（本轮 `b21-installed-path` 第一次就这样）。判据取不齐时**直接落成 `.ps1` 再 `-File` 跑**，不要反复试引号。
7. **装机根目录在 D 盘**：`D:\Program Files\Alpha File Manager`（`C:\Program Files\Alpha File Manager` 也存在但**没有 app exe**）。批次 21 中途那版脚本猜了 4 个 C 盘候选 ⇒ `INSTALLED_EXE_NOT_FOUND`。真实路径的两个可靠取法：安装器日志 `detectedLocation=` / 卸载注册表 `InstallLocation`，均已固化进 `.superpowers\sdd\b21-installed-path.ps1`。

---

## 三、本轮动作与结果

### 3.1 重新打包
- **决策更正**：批次 20 的默认建议是「不重打包」。本轮推翻它 —— `3.14.1` 已占发版号，而包体与清单里**没有**批次 20 的写入闸门修复，属**账面缺陷**（用户装到的版本与代码声称的能力不一致）⇒ 原地重建 `3.14.1`，不换版本号。
- 命令链：`npm run tauri:build` = `sync-version` → `verify:sandbox` → `tauri build` → `publish-bundle-to-release.mjs`。**只复制到本地 `release/`，不上传。**
- 结果：`BUILD_EXIT=0` / `ALL_DONE`；`cargo 1.90.0 (840b83a10 2025-07-30)`；`Compiling alpha-file-manager v3.14.1`；`Finished release profile in 5m 59s`；`Finished 1 bundle ... (43.38 MiB)`。日志 `.superpowers\sdd\b21-final.log`（250,747 B），尾部只有 `dead_code` 警告（`enum QuarkCode`），无 error。
- 载荷核验：`.superpowers\sdd\verify-installer-payload.ps1` 跑两轮（`payload-20261118-223655`、`payload-20261118-223842`）均 `VERIFY_OK reason=payload-carries-expected-sidecar-and-new-extension`。

### 3.2 sha 与包身份
- 新安装包 `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.14.1_x64-setup.exe`
  = **45,489,382 B** / `2026-11-18 22:36:36` / sha256 **`5F07F5B914DFEF9FBF22E040BC3C2B67D13167B3EBA633B6A20C5BCB47F88E58`**
- `release/` 根目录：覆盖前 `45,478,740 B / 1DD82EEED…`（批次 19）→ 覆盖后 `45,489,382 B / 5F07F5B9…`。
  `publish-bundle-to-release.mjs` 的防呆会**拒绝**覆盖同名文件并自动改写入 `release\local-builds\`（同 sha 副本留在那里）⇒ 必须显式 `Copy` 覆盖根目录。这不是 bug，是设计；每次重打包都要多做这一步。
- 裸构建产物（归一化比对的基准侧）：`src-tauri\target\release\alpha-file-manager.exe` = 82,764,288 B / `DACDBBC7CC686B5BE53043641A8D99EDE3F2D6AA4E532AA285480CABD7E4320D`；归一化后 `806515579181D24F8606E1484C2613BF0F64A19C0F6B55AC17087A13CDFE66A3`。

### 3.3 winget 清单更新
- 文件：`docs\winget\manifests\k\kizemo\AlphaFileManager\3.14.1\kizemo.AlphaFileManager.installer.yaml`
- `InstallerSha256`：`1DD82EEED36127F21595FFB852FF1C65474498B44A99995CEB3B00BA3F6D2A83` → **`5F07F5B914DFEF9FBF22E040BC3C2B67D13167B3EBA633B6A20C5BCB47F88E58`**
- `ReleaseDate: 2026-11-18` 与 `InstallerUrl`（v3.14.1 资产名）**无需改动**。
- 门禁：`node ./docs/winget/check-manifest.mjs`（= `npm run check:winget`）⇒ **`WINGET_CHECK_EXIT=0`**，8 条全绿，含 `InstallerSha256 matches release\Alpha File Manager_3.14.1_x64-setup.exe (45489382 bytes)`。
- ⚠️ **该门禁只校验本地 `release/` 副本**，**不证明 GitHub Release `v3.14.1` 上那个资产已是新包**。资产未替换前，若提 winget PR 会哈希不匹配。

### 3.4 装机链复验
- 脚本 `.superpowers\sdd\b21-install-chain.ps1` → 摘要 `b21-install-chain.txt`，安装细节 `b21-install-verify.txt`。
- 装前备份侧车 `.superpowers\sdd\old-sidecar-b21-preinstall.exe`（sha = v0.6.4，见 §一 更正 B）。
- 安装：`<setup.exe> /S /FORCE` ⇒ `INSTALL_EXIT=0`；`install-afm-verify.ps1` ⇒ `INSTALL_VERIFY_EXIT=0` + `VERIFY_OK reason=install-refreshed-appdata-sidecar`；装后 AppData 侧车 sha `480FDE62…`，`APPDATA_SIDECAR_IS_V064=True`；`SIDECAR_PROCS=0`。
  同版本重装**必须带 `/FORCE`**，否则项目防呆直接 `EXIT=2 reason=same-build`。
- 装机 app exe 身份（`b21-installed-path.ps1`）：`D:\Program Files\Alpha File Manager\alpha-file-manager.exe` = 82,764,288 B / mtime `2026-11-18 22:35:54` / fv=pv=`3.14.1` / sha `F03E02E7…8F6F`；
  bundle-type 标记归一化后 = `80651557…66A3` = 裸构建产物归一化值 ⇒ **`VERDICT_INSTALLED_IS_NEW_PAYLOAD=True`**（`RAW_EQUAL=False` 只反映 `__TAURI_BUNDLE_TYPE_VAR_NSS__` vs `…_UNK__` 那 29 字节标记，符合 AGENTS §2.10 / 既有记忆）。
- ⚠️ **残留（非发版链缺陷，但要如实记录）**：装机时及应用当前有 **1 个 AFM 进程自 `2026-11-18 14:07:51` 起一直在跑**（PID 924772），它跑的是**装前镜像**。盘上的 exe 已是新载荷、AppData 侧车已是 v0.6.4，但**用户界面上要重启应用才吃到新 exe**。本轮**没有**为取证而杀用户进程（越权风险），故「运行中进程 = 新载荷」这一条 = **未判定**；若批次 22 需要，先征得用户同意再关闭后重装复验。

---

## 四、本轮产物清单（全路径，`.superpowers\` 按惯例不入库）

- `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\b21-final.cmd` / `b21-final.log`（构建配方与日志）
- `…\.superpowers\sdd\b21-build-wait.ps1`（轮询等构建，状态外置）
- `…\.superpowers\sdd\b21-verify.txt`（载荷核验两轮输出）
- `…\.superpowers\sdd\verify-installer-payload.ps1` `payload-20261118-223655\` `payload-20261118-223842\`
- `…\.superpowers\sdd\b21-install-chain.ps1` / `b21-install-chain.txt` / `b21-install-verify.txt` / `old-sidecar-b21-preinstall.exe`
- `…\.superpowers\sdd\b21-installed-path.ps1`（装机根发现 + 归一化比对 + 进程实况；本轮新增）
- `…\src-tauri\target\release\bundle\nsis\Alpha File Manager_3.14.1_x64-setup.exe`（新包）
- `…\release\Alpha File Manager_3.14.1_x64-setup.exe`（新包已落位）+ `…\release\local-builds\`（防呆隔离的同 sha 副本）
- 入库改动：`docs\winget\manifests\k\kizemo\AlphaFileManager\3.14.1\kizemo.AlphaFileManager.installer.yaml`、本卡、台账 §20.16

---

## 五、本轮**未做**的事 / 遗留项

1. **⑪ 真机计数级收口（唯一硬阻塞）**：仍 `UNDETERMINED`。跨全部 `dup` 日志 grep `VERDICT=|w1=|w2=` 的实测结论：唯一一次写入真正落地是 `b20-dup-count3-run.log:21` `[20:30:05] VERDICT=FAIL w1=2 w2=0` —— 那是**修复前 v0.6.3 的基线**；v4/v5/v6 全轮 `w1=0 w2=0` = 夹具/送达问题，**不得**当修复后证据。⇒ 需要 v0.6.4 上取到 `w1=1 w2=0` 才算闭合。
   批次 20 配方 `b20-dup-count4.ps1`（子进程 `AllowSetForegroundWindow` → 接收方自己抬 AFM 类窗口约 1.6 s → 交回对话框 → push）是唯一未试的最优路径。属前台语义取证，**只能交互桌面**（AGENTS §2.8 偏离须如实记录）：夹具窗口做小、判完立即关、退出前恢复原前台。
2. **对外发布三件事，需用户明确授权，本轮未做**：① 用新包替换 GitHub Release `v3.14.1` 资产；② 提 winget manifest PR（前置：§2.10 提醒 3.9.1 PR #467381 仍 OPEN）；③ upstream 移植。
3. **运行中进程 = 新载荷**未判定（见 §三.4 ⚠️）。
4. **⑤ 争用源定位**：低优先，未动。
5. `src-tauri\tauri.conf.json` 的 `additionalBrowserArgs` **未注入** CDP 旗标（本轮不需要；若批次 22 做 CDP 取证再注入并记得回滚；端口用 9333，9222 被 ecaw 占用勿杀）。

---

## 六、下一批次（批次 22）建议入口

1. 先读本卡（**§〇 结论一览 + §一 两处更正 + §二 配方**）+ 台账 §20.16 + `AGENTS.md` §2.8、§2.10。
2. §二 的 7 条直接抄，**不要重新试工具链**（`stable` 1.97.1 已证伪）。
3. §一 更正 B 已消失的问题（装机侧车过期）**不要再去修**。
4. 唯一硬阻塞 = **⑪**，用 §五.1 配方；其余等用户授权才动对外发布。
