# 根目录统一与清理计划（2026-11-10）

> 状态：**仅计划，未执行**。本轮未删除、未移动、未提交、未推送任何文件。
> 判定口径：不接受「路径同名」推断，全部结论基于 git blob / sha256 逐项比对与实测体积。

---

## 0. 一句话结论

`F:/soft/00selfmade/filemanager/sigma-file-manager` 是完整、可独立构建发版的源码仓，**不缺功能文件**；
外层 `F:/soft/00selfmade/filemanager` 是「旧 Focus Sync 文档仓 + 一次性探针 + 本机快照」，
**不含任何源码**。换根需要做的只有一件事：把外层的 **约 321 个文档/配置文件** 与 **3 个发版脚本** 迁进来；
迁完外层可回收约 **623 MB**（含决策项最多约 659 MB）。

---

## 1. 内层仓完整性核查 —— 结论：完整

### 1.1 实测证据

| 项目 | 实测值 |
| --- | --- |
| git 状态 | HEAD `dadf02fc`，`## main...origin/main`（与远端同步） |
| 跟踪文件数 | **1884** |
| 远端 | `origin` = kizemo/alpha-file-manager，`upstream` = aleksey-hoffman/sigma-file-manager |
| 源码树 | `src/`、`src-tauri/`、`src-tauri/installer/`、`extensions/kizemo.focus-sync/`、`scripts/`、`.github/workflows/` 全部存在 |
| 构建入口 | `package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 全部存在 |
| 文档 | `docs/` 216 文件（含 `docs/superpowers/{specs,plans}`、`docs/operations`、`docs/screenshots`、`docs/winget`） |
| 发行物 | `release/` 29 个 exe（3.6.4 → 3.13.5）；已发版包 `release/local-builds/Alpha File Manager_3.13.5_x64-setup.exe`，45,407,445 B，sha256 `c5b09252c42f7d1b078623c0c0855c06eda316bb6d1d536f8bbd19556e888502` |
| 未跟踪项 | 仅 1 个：`extensions/kizemo.focus-sync/sidecar/src/bin/repro_save_dialog.rs` |

**判定**：内层仓自成闭环——源码、构建脚本、扩展、文档、发行物、CI 配置齐全，可以只靠它完成一次完整发版。

### 1.2 内层缺、外层有的内容（换根必须补的缺口）

| 缺口 | 外层位置 | 体积 | 影响 |
| --- | --- | --- | --- |
| `AGENTS.md` | 外层根 | 110,287 B | **最严重**。内层无 `AGENTS.md` / `CLAUDE.md` / `.cursorrules`，当前生效的项目约定只存在于外层。换根后新会话会**丢失全部硬约束**（dev 构建不可验收、`.cmd` 必须纯 ASCII、双击验收口径、受保护常量等） |
| `build-sidecar.cmd` | 外层 `.superpowers/sdd/` | 小 | 侧车构建入口，3.13.5 发版流程在用 |
| `build-3135b.cmd` | 外层 `.superpowers/sdd/` | 小 | 3.13.5 打包入口 |
| `upload-one.py` | 外层 `.superpowers/sdd/` | 小 | 发版上传 |
| 历史文档约 280 份 | 外层 `docs/` + 根目录 | 17.5 MB + 少量 | 只影响历史可追溯性，不影响构建 |

注：`.superpowers/sdd/` 自身被 `.superpowers/sdd/.gitignore`（内容 `*`）自我忽略，**两仓都不跟踪它**
⇒ 这 3 个脚本是**磁盘上的唯一副本**，删外层目录即永久消失。

### 1.3 一个必须先知道的历史包袱

内层有 **89 个已跟踪文件、711 处**硬编码外层路径 `F:\soft\00selfmade\filemanager`。分布：

| 目录 | 文件数 |
| --- | --- |
| `docs/` | 82 |
| `extensions/kizemo.focus-sync/` | 5 |
| 根 `.gitignore` | 1 |
| `src/` | 1 |

非 markdown 的 7 个已逐处核验性质：

- `sidecar/src/reader.rs`（3 处）：文档注释 + `#[test] parse_address_label(...)` 断言夹具
- `sidecar/src/state.rs`（1 处）：`#[cfg(test)]` 里的 `const AFM_PATH`
- `sidecar/src/uia_inject.rs`（8 处）：`#[test] folder_readback_matches(...)` 夹具
- `sidecar/src/writer.rs`（2 处）：`#[test]` 内的 Save As 目标路径
- `src/modules/navigator/components/file-browser/__tests__/address-bar-editor-utils.test.ts`：单测夹具
- `sidecar/scripts/manual-detect-test.ps1`：手工探针
- `.gitignore`：注释

**判定：没有任何生产运行时代码依赖外层目录。** 删掉外层不会让产品失效，也不会让 `cargo test` 失败
（夹具是常量字符串，不读磁盘）。但 82 份 `docs/` 里的路径在换根后会指向不存在的目录——见 Phase 3 的处理选项。

---

## 2. 外层价值分级

### 2.1 外层总览（实测）

| 路径 | 跟踪状态 | 文件数 | 体积 |
| --- | --- | --- | --- |
| `sigma-file-manager/` | 被外层 `.gitignore` 第 2 行忽略 | 248,804 | 55,930.7 MB |
| `.git/` | — | 629 | 34.6 MB |
| `.superpowers/sdd/` | 自我忽略（`*`） | 523 | 341.7 MB |
| `_acceptance/` | 4 个 md 跟踪，其余忽略 | 116 | 121.8 MB |
| `.other/` | 忽略 | 108 | 102.4 MB |
| `.pkg-backups/` | 忽略 | 19 | 54.7 MB |
| `docs/` | 228 跟踪 | 244 | 17.5 MB |
| `scripts/` | 28 跟踪 | 30 | 9.4 MB |
| `release/` | 2 跟踪 | 2 | 0.9 MB |
| 根目录文件 | 57 跟踪 | 61 | 约 2.9 MB |
| `$outDir/` | 未跟踪 | 0 | 0 |
| `node_modules/.vite/vitest/…/results.json` | 忽略 | 1 | 约 0 |

外层仓 HEAD `90e87e6`，跟踪 **321** 个文件 = docs 228 + scripts 28 + 根 57 + `_acceptance` 4 + `release` 2 + `.github` 2。
`origin` 指向 **另一项目** `kizemo/focus-sync`，本地相对 `origin/main` **ahead 84 / behind 2**，`merge-base` 为空（历史不相交）。

### 2.2 A 类 —— 有价值，需迁入内层

| 内容 | 体积 | 跟踪 | 说明 |
| --- | --- | --- | --- |
| `AGENTS.md` | 110,287 B | 是 | **唯一生效的项目约定**，最高优先级 |
| `docs/` 全部 244 文件 | 17.5 MB | 228 跟踪 + 16 未跟踪 | 含 AFM 关键历史：`FORK-KEEP-LIST.md`、`FORK-WORKFLOW.md`、`MONTHLY-SYNC-CHECKLIST.md`、`PLAN-2026-10-09/10/11-*.md`、`PR-LOG.md`、`STARTUP-FLOW-NOTES.md`、`baseline-2026-10-09-build-anchors.md`、约 100 份 focus-sync 时代的 `docs/superpowers/plans/*` |
| 根目录 51 份 `handoff-*.md` / `prompt-*.md` + `task.md` / `README.md` | 约 0.6 MB | 是 | focus-sync 时代的交接与接力记录，文件名与 `docs/` 下**零重名、零同内容** |
| `docs/brand/afm-icon-1024.png` | 773,825 B | 是 | 品牌母版图标 |
| `docs/marketing/`（含 `drafts/`） | 约 0.5 MB / 13 文件 | **否** | 推广文案 5 篇 + 2 张 social preview PNG（`social-preview-afm.png` 164,272 B、`social-preview-weavepage.png` 190,447 B）+ 草稿 6 篇 |
| `docs/blog/2026-10-14-alpha-file-manager.md` | 6,923 B | **否** | 发布博客稿 |
| `docs/SFM-EXTENSION-DEVELOPMENT-GUIDE.md` | 44,830 B | **否** | 扩展开发指南，内层没有对应文件 |
| `_acceptance/*.md` 4 份 | 约 0.04 MB | 是 | 真正的根因分析：`ROOT-CAUSE-self-access-false-positive.md`、`rollback-2026-10-13/ROLLBACK.md`、`version-node-audit-2026-10-13/{DELIVERY,VERSION-NODE-AUDIT}.md` |
| `.superpowers/sdd/{build-sidecar.cmd, build-3135b.cmd, upload-one.py}` | 小 | **否** | 现行发版脚本，磁盘唯一副本 |
| `scripts/` 中未被内层覆盖者 | 9.4 MB / 28 文件 | 是 | 与内层 23 个脚本**零重名**。已可确认有长期价值：`sync-upstream.sh`、`scan-sandbox.cjs`、`verify-ui.ps1` + `setup-baseline.json`；其余多为一次性探针（`monitor-issue-499.*`、`_extract_cc_haha_sessions*.py`、`install-10pct*.ps1`、`cc-haha-sidebar-rebuild.ps1`、多张 UI 截图 PNG）⇒ 逐个判定，见 Phase 2 |

> 未跟踪的 10 项（`docs/marketing/**` 7 项、`docs/blog/**` 1 项、`SFM-EXTENSION-DEVELOPMENT-GUIDE.md`、`vt3.txt`）
> **只存在于磁盘**：直接删外层目录 = 永久丢失，无 git 可恢复。

### 2.3 B 类 —— 一次性产物，可回收（约 623 MB）

| 内容 | 体积 | 可恢复性 |
| --- | --- | --- |
| `.superpowers/sdd/` 其余 520 个文件 | 341.7 MB | 不可；但属一次性探针与日志 |
| `_acceptance/` 非 md 的 112 个文件（`probe-*.ps1`、`*.rs`、`shot-*.png`、exe 备份） | 约 121.8 MB | 不可；一次性探针 |
| `.other/`（`audit` 40.9 MB、`src` 0.4 MB 40 文件、`doc`） | 102.4 MB | 不可；`src` 是历史快照 |
| `.pkg-backups/`（`installdir-*` 42.7 MB、`appdata-*` 2.8 MB、验收 PNG） | 54.7 MB | 不可；「本机后悔药」，但已被内层 `release/` 的安装包取代 |
| `.analysis-all.log` | 1,320,400 B | 不可 |
| `.baseline-all.log` | 1,376,667 B | 不可 |
| `kill-app-tree.ps1`（根副本） | 9,504 B | 是——但**与内层副本不是逐字相同**（外层 sha16 `285D2092C32FB6C1`，内层 `src-tauri/installer/kill-app-tree.ps1` 为 `B15D2452BD39A575`）⇒ 外层是旧试验版，内层为权威版 |
| `node_modules/.vite/vitest/da39a3ee…/results.json` | 极小 | 是 |
| `$outDir/` | 0（空目录） | — |
| `vt3.txt` | 249 B | 不可；内容为临时草稿，`docs/handoff` 里已写「入库前删除」 |

### 2.4 C 类 —— 需拍板

| 内容 | 体积 | 争议点 |
| --- | --- | --- |
| `.git/` | 34.6 MB | 含 **84 个无远端的提交**。删目录 = 历史永久消失 |
| `release/kizemo.focus-sync-0.5.8-setup.exe` | 978,468 B | 旧版插件独立安装器；插件已随 AFM 3.13.5 内嵌发布，是否仍需分发旧版 |
| `_memory-audit-e.csv` | 206,619 B | 记忆审计产物，跟踪；价值低但体积极小 |
| `README.md` / `LICENSE`（外层根） | 5,115 B | 外层仓定位说明；迁入 archive 后即可废弃 |

---

## 3. 外层仓 git 的处置

事实：`origin = kizemo/focus-sync`（另一项目）、本地 ahead 84 / behind 2、`merge-base` 为空、**禁止 force push**。

### 建议：冻结 + bundle 存档，不做任何推送

1. **不做 cherry-pick**。两仓无共同祖先，84 个提交会引入大量与外层路径绑定的无关变更；
   而且这 84 个提交的实质产出就是 `docs/` 里那些文件，内容迁移已等价于历史迁移。
2. **`git bundle` 存档**：`git bundle create <仓外路径>\outer-focus-sync-history.bundle --all`
   —— 单文件带走全部 84 个提交，体积远小于 34.6 MB 的 `.git`。这是「删目录仍可回溯」的唯一保险。
3. **防误推**：若要短期保留外层目录，执行
   `git -C F:\soft\00selfmade\filemanager remote set-url --push origin DISABLED_NO_PUSH`。
4. **不做**：新建远端并推送 84 个提交（用户本轮已把外层重定义为「误解」，无需给它一个正式归宿）。

---

## 4. 执行计划（6 阶段，本次不执行）

### Phase 0 · 预检（只读，零风险）

- 确认无占用：`alpha-file-manager` / `msedgewebview2` / `cargo` / `node` 进程为空（本项目有过「进程还在时恢复数据被回写」的教训）。
- 两仓 `git status` 与本文记录一致（内层仅 1 个既有未跟踪文件；外层仅 10 项未跟踪）。
- **保底清单先落盘**（这是唯一的「丢了还能查」的凭证）：
  - `git -C <外层> ls-tree -r HEAD > <仓外>\outer-tracked-manifest.txt`
  - `git -C <外层> status --porcelain --untracked-files=all >> 同上文件`

### Phase 1 · 备份（可逆性来源，必须最先做）

目标目录：**仓库之外**，如 `F:\soft\00selfmade\_archive-2026-11-10\`（同盘复制快）。

1. `git bundle create` 外层全历史（见 §3）。
2. 复制外层 **321 个跟踪文件**（按 `ls-tree` 清单，保留相对路径）。
3. 单独复制**未跟踪但唯一**的 10 项：`docs/marketing/**`、`docs/blog/**`、`docs/SFM-EXTENSION-DEVELOPMENT-GUIDE.md`。
4. 单独复制 **3 个发版脚本**，并立即用 sha256 与源比对（`.superpowers` 无版本控制，这是唯一校验手段）。
5. `vt3.txt` 不入备份（已知无价值）。

### Phase 2 · 迁移（只 copy，不 move，不删源）

目标根：`F:\soft\00selfmade\filemanager\sigma-file-manager\`

| 源 | 目标 |
| --- | --- |
| 外层 `AGENTS.md` | 内层根 `AGENTS.md` |
| 外层 `docs/` 全部 + 根 `handoff-*.md` / `prompt-*.md` / `task.md` / `README.md` | `docs/archive/focus-sync-legacy/`（保留原相对路径） |
| 外层 `docs/brand/afm-icon-1024.png` | `docs/brand/afm-icon-1024.png` |
| 外层 `docs/marketing/**` | `docs/marketing/` |
| 外层 `docs/blog/**` | `docs/blog/` |
| 外层 `docs/SFM-EXTENSION-DEVELOPMENT-GUIDE.md` | `docs/SFM-EXTENSION-DEVELOPMENT-GUIDE.md` |
| 外层 `_acceptance/**.md`（4 份） | `docs/archive/root-cause/` |
| 外层 `scripts/` 经判定保留者 | `scripts/archive-focus-sync/` |
| `build-sidecar.cmd` / `build-3135b.cmd` / `upload-one.py` | 内层 `.superpowers/sdd/`（保持交接文档 §5 的调用不变）**并**另存去绝对路径版本到 `scripts/release/`（入库） |

脚本去绝对路径化要点（实测这 3 个脚本共 6 处硬编码）：

- `build-sidecar.cmd` L4 `cd /d F:\soft\00selfmade\filemanager\sigma-file-manager\extensions\kizemo.focus-sync\sidecar` → `cd /d "%~dp0..\..\..\extensions\kizemo.focus-sync\sidecar"`
- 同名 L5 / L9 / L12 的日志重定向目标 `F:\soft\00selfmade\filemanager\.superpowers\sdd\*.log` → 同目录相对形式
- `build-3135b.cmd` L8 `cd /d …\sigma-file-manager` → `cd /d "%~dp0..\..\.."`，L9 同上
- `upload-one.py` 无硬编码命中，无需改
- 改完必须仍是**纯 ASCII**（`cmd.exe` 按 ANSI 读 `.cmd`，中文注释会被当命令执行）

`scripts/` 逐项判定子任务：28 个文件中，保留 `sync-upstream.sh`、`scan-sandbox.cjs`、`verify-ui.ps1`、`setup-baseline.json`、`README-monitor.md`；其余标记为一次性，只进 Phase 1 备份不迁入。

### Phase 3 · 内层接入与适配

1. **`AGENTS.md` 路径改写**：把指向外层根的所有路径改为内层根；删除只对外层仓成立的段落（外层 `.gitignore` 三规则、`_acceptance` 忽略规则、外层远端处置）；**保留全部硬约束原文**。
2. **新增 `docs/archive/README.md`**：说明这批文件来自旧 Focus Sync 文档仓、按当时路径记录、不再维护。
3. **82 份历史 docs 里的外层路径**：默认**不改**，靠 archive README 声明即可（它们是历史记录，改写会破坏原始性）。若要求可点击可用，用 node 脚本批量做
   `F:\soft\00selfmade\filemanager\X` → `F:\soft\00selfmade\filemanager\sigma-file-manager\X` 的**仅限非 archive 文档**映射。
4. 内层 `.gitignore` **不要**忽略 `docs/archive/`——这批内容必须入库。

### Phase 4 · 验证（不通过即回滚）

1. **内容对账**：外层 321 个跟踪文件逐个在内层找到副本，用 `git hash-object` / sha256 比对**内容**（不比路径，因为目标路径已变）。
2. `git -C <内层> status` 只应出现计划内的新增，不得有意外修改。
3. 门禁复位：
   - `npm run lint:eslint:check` → 期望与 HEAD 基线一致（177 errors / 0 warnings；数字会随提交变，量之前用 `git stash` 对 HEAD 复核）
   - `npm run test:unit:run` → 期望 299 files / 2840 passed
4. 抽查 5 个迁移文件的内容哈希与源一致（防编码损坏）。
5. 确认 3 个发版脚本的 sha256 与备份一致，且 `build-3135b.cmd` 在**不修改任何文件**的前提下能进入 cargo 编译阶段（可只跑 5 秒后中止，验证 `cd` 路径有效）。

### Phase 5 · 清理外层（**必须二次确认后才执行**）

**不用 `Remove-Item`，先移动**：`robocopy /MOVE` 到 `F:\soft\00selfmade\_trash-2026-11-10\`（同盘移动秒级且可逆）。

顺序：
1. B 类（约 623 MB）
2. C 类（用户决策后：`.git`、旧安装器、`_memory-audit-e.csv`）
3. 最后才考虑删除外层目录本身

**观察期建议 1 周**，期间至少完整跑通一次 `tauri build` 与一次发版流程，再真删 `_trash-2026-11-10`。

红线：
- 绝不 `git add .` 进外层仓（会把 341 MB 灌进版本库）
- 不 force push 到 `focus-sync`
- 任何时候不编辑正在运行的 `.cmd`

### Phase 6 · 收口

- 写 `docs/handoff-2026-11-10-root-unification.md`（新目录结构 + 迁移清单 + 回收体积 + 遗留项）
- 复核 `AGENTS.md` 里已无外层路径（`grep -F '00selfmade\filemanager\.'` 应只命中 archive 与测试夹具）
- 记录 P2 待办：内层目录总体积 **55,930.7 MB / 248,804 文件**（含 `node_modules`、`src-tauri/target`、内层 `.superpowers` 4.36 GB），单独一轮处理

---

## 5. 风险表

| 风险 | 触发条件 | 防护 |
| --- | --- | --- |
| `.superpowers/sdd` 三个发版脚本永久丢失 | 删外层前未复制 / 复制后未校验 | Phase 1 复制后立即 sha256 比对；Phase 4 再比对一次 |
| `docs/marketing/**`、`docs/blog/**`、`SFM-EXTENSION-DEVELOPMENT-GUIDE.md` 永久丢失 | 直接删目录（它们**未跟踪**） | Phase 1 单独备份；Phase 4 内容对账 |
| 外层 84 个提交历史永久消失 | 直接删 `.git` | `git bundle --all` 存档 |
| **换根后按失效路径做验收，产出假结论** | `AGENTS.md` 未改写就换根 | Phase 3 逐条改写 + Phase 6 `grep` 复核。本项目已有多次「路径/口径错导致假结论」的先例（DPI 口径、物理/逻辑像素、`eslint` 口径） |
| 迁移把中文文档写成乱码 | 用 PowerShell `Get-Content -Raw` / `Set-Content` 处理 | 只用编辑工具或 node `fs` 显式 `utf8`；Phase 4 抽查哈希 |
| 删外层影响内层仓 | — | 不会：内层被外层 `.gitignore` 第 2 行忽略，两仓互不感知；内层 `.git` 位于 `sigma-file-manager/.git`，与外层 `.git` 无关 |
| 误删仍在使用的内层构建产物 | 清理范围外溢到内层 | 本轮清理范围**严格限定外层**；内层体积问题列为 P2 |

---

## 6. 需要拍板的 5 项决策（附推荐）

| # | 决策 | 推荐 |
| --- | --- | --- |
| 1 | 外层 321 跟踪 + 10 未跟踪的唯一内容如何处置 | **全量归档进内层 `docs/archive/focus-sync-legacy/`**（内容零重叠、体积仅 17.5 MB，没理由拆散） |
| 2 | 外层 `.git`（34.6 MB / 84 提交 / 无远端） | **`git bundle --all` 存档到仓外，随目录一起清理** |
| 3 | `.superpowers/sdd`（341.7 MB） | **捞 3 个脚本后整目录移入回收目录** |
| 4 | `_acceptance` 121.8 MB / `.other` 102.4 MB / `.pkg-backups` 54.7 MB | **移回收目录，观察 1 周后删**（同盘移动可逆，成本几乎为零） |
| 5 | `release/kizemo.focus-sync-0.5.8-setup.exe`（978 KB） | **随外层清理**（插件已内嵌进 AFM 3.13.5，旧独立安装器不再分发）；若仍需支持旧版用户则迁入 `release/legacy/` |

---

## 7. 执行边界

截至 2026-11-10，本文件仅为计划：

- 未删除任何文件
- 未移动任何文件
- 未 `git add` / `commit` / `push`（两仓均无新提交）
- 未打包、未装包

外层仓工作区与内层仓状态保持原样，唯一新增是本文件
`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\plan-2026-11-10-root-unification-and-cleanup.md`（未跟踪，待批准后连同迁移一起提交）。