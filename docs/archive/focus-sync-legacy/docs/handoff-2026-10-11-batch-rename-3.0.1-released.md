# 交接文档 —— 批量改名已发布 3.0.1，数据丢失缺陷已修复（2026-10-11 23:10）

## 1. 仓库与路径（改任何东西前先认清自己在哪个仓）

**两个独立 git 仓 + 一个仓外目录**，别搞混：

| 用途 | 路径 | git origin |
|---|---|---|
| 代码仓 | `F:\soft\00selfmade\filemanager\sigma-file-manager` | `kizemo/alpha-file-manager.git` |
| 文档仓（本文件所在地） | `F:\soft\00selfmade\filemanager\docs\` | `kizemo/focus-sync.git`（仓库名是历史遗留，内容已是 Alpha File Manager 文档） |
| 站点源（**不在任何 git 仓内**） | `E:\办公文件\L1网站\alpha-file-manager\` | 无 |

- 代码仓当前状态：分支 `main`，HEAD `711fb7cd`，**与 `origin/main` 同步**，工作区仅剩未跟踪的 `_notes.md`（发版说明草稿，按惯例不入库）。
- 项目硬约定见 `F:\soft\00selfmade\filemanager\AGENTS.md`（输出语言、最小化修改原则、受保护常量）。**新会话第一件事就读它。**

---

## 2. 本轮时间线：批量改名从 0 到 3.0.1

上一份交接（`handoff-2026-10-11-batch-rename-rust-phase.md`）结束时，Rust 侧全部完成、**待接前端**。本轮做完了剩下的全部：

1. **注册 Tauri command** —— `src-tauri/src/batch_rename/mod.rs` 末尾纯追加 4 个 command（`batch_rename_preview` / `_execute` / `_history` / `_undo`），`src-tauri/src/lib.rs` 末尾追加 4 行注册。
2. **i18n** —— 新增 81 个 `batchRename.*` key，17 个语言包对齐（`ch` 中文，其余 15 个英文占位 = 已定方案 A）。新增门禁 `src/localization/__tests__/messages-key-alignment.test.ts`。
3. **前端** —— 新建对话框、历史面板、composable、类型；纯追加接线到右键菜单、选中态、对话框状态机。
4. **用户实测反馈 5 个问题，全部修复**（见 §4.1）。
5. **发 3.0.0**（commit `7b6514fc`）。
6. **实测兑换循环/冲突阻断时，查出一条数据丢失缺陷并修复**（见 §5）。
7. **发 3.0.1**（commit `711fb7cd`）—— 当前线上版本。

### 线上现状（2026-10-11 23:05 已回读确认）

| 项 | 值 |
|---|---|
| 版本 | 3.0.1 |
| 安装包 | 18,152,910 字节 |
| SHA-256 | `467b75bcd6d0216e69fd11c29a08664a067ee8c6b7c3f72166a5bb9182f36bfa` |
| 站点 | https://www.aiec.fun/alpha-file-manager/ |
| GitHub release | https://github.com/kizemo/alpha-file-manager/releases/tag/v3.0.1 |
| 线上 `version.json` 的 `history[]` | `3.0.0, 2.2.2, 2.2.1` |

四方 sha256 一致：`release/` = 站点副本 = 线上回读 = `version.json` 声明。

---

## 3. 核心机制：三类冲突与「两阶段腾空」

批量改名是**破坏性操作**，漏检任何一类都会丢用户数据。整条安全链在 `src-tauri/src/batch_rename/conflict.rs`。

| 类别 | 场景 | 处理 |
|---|---|---|
| A `TargetExists` | 新名撞上**本批次之外**的已有文件 | **阻断**，要求用户改名 |
| B `DuplicateTarget` | 两个条目生成了**相同**的新名 | **阻断**，并指出是哪两个 |
| C 交换循环 | `a→b` 且 `b→a` | **自动**两阶段腾空，用户无感 |

### 为什么 C 不阻断

腾空法：先把要让位的条目全搬到唯一临时名（`__afm_tmp_{index}{ext}`），目标位置就空了，再统一落位。落位时目标位置要么是空的，要么只会被下一个条目填上——**任何映射形状都不会冲突**，二环三环自映射混合都不需要特判。

```
阶段一（腾空）:  a.txt → __afm_tmp_0.txt
                b.txt → __afm_tmp_1.txt
阶段二（落位）:  __afm_tmp_0.txt → b.txt
                __afm_tmp_1.txt → a.txt
```

实现见 `plan.rs::build_staging_plan`。**关键设计**：回调只返回**父目录前缀**，不返回完整目标路径。早先版本让回调返回目标全路径、临时名塞进 `old_name` 传，于是回调读 `new_name`——阶段一的目标被算成最终名，交换循环照样丢数据。

### 两条不同的回滚路径（别混）

- **阶段一失败**（目标是唯一临时名，彼此不冲突）→ 逆序还原 `done_stage_one`，简单。
- **阶段二失败**（目录状态是混合的：一部分在临时名、一部分已落位）→ **不能**逆序还原。要重新扫描现状重建归位计划，且归位本身也可能是交换，所以还得再走一次两阶段（`executor.rs::rollback_to_original`）。

---

## 4. 本轮踩过的坑（重蹈覆辙代价很大）

### 4.1 用户实测反馈的 5 个问题

| # | 现象 | 根因 |
|---|---|---|
| 1 | 线格式字段名不匹配（改扩展名硬报错、勾「包含扩展名」无反应） | `#[serde(rename_all = "camelCase")]` **只重命名变体名，不碰变体内部的字段**。必须加 `rename_all_fields = "camelCase"`（serde 1.0.229 支持）。带 `#[serde(default)]` 的字段静默变 false，没带的直接 `missing field` |
| 2 | 全选转全大写/全小写显示 0 项 | `unchanged` 曾按 `normalize_for_compare`（小写）比较。仅大小写不同是**真实且合法**的改名（改扩展名大小写、转 snake_case 都依赖它）。已改为**逐字节**比较 |
| 3 | `27_gri_r0_c0` → `27 Gri R 0 C 0` | Title case 把 `_`/`-` 当词边界换成了空格。已改为 `title_case()` 保留原分隔符、数字不作词边界 |
| 4 | 序号规则单独用无效 | **设计如此**（序号只为模板提供 `{n}`）。没偷偷改语义，而是加说明文字 + 「添加一条模板规则」按钮（预填 `{n}-{name}`） |
| 5 | `getUtcOffsetMinutes()` 在 UTC 主机返回 `-0` | 已修 |

### 4.2 `fs::rename` 在 Windows 上是**静默覆盖**，不是失败

这是本轮最贵的教训。

`executor.rs` 原先写着：「Windows 上 `rename` 对已存在的目标会直接失败（不同于 POSIX 的静默覆盖），这恰好是一层额外保护」。**该说法不成立。** 探针实测：

```
rename result = Ok(())
dst content   = Ok("SOURCE")     ← 目标原内容 DEST_OLD 没了
VERDICT: OVERWRITE-SILENT
```

Rust 在 Windows 上走 `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`，与 POSIX 一样是静默覆盖。

**唯一防线是 `conflict.rs` 的 `detect`，它必须在落盘前拦住所有必覆盖的场景。** 任何新增的改名路径都必须先过 `detect`。原注释已改写成警告。

### 4.3 Rust 产物不能用 sha256 判断「有没有更新」

同源码、同编译器、同机器，只换 `CARGO_TARGET_DIR`，二进制 sha 就变；NSIS 包整包 sha 每次打包都变。

判断产物是否真的更新，要用**内容判据**：
- 本轮用的是**主程序 exe 的体积变化**：45,605,888 → 45,614,080 字节，证明 3.0.1 的 changelog 文本确实嵌进去了。
- 手工维护的文件（无构建链的 JS/JSON/资源）可以做 sha 锚点。

### 4.4 `release-publish.mjs` 只在版本号变化时才归档旧版本

`scripts/release-publish.mjs:240`：

```js
if (oldVersion !== newVersion && manifest.version === oldVersion) { /* 归档进 history[] */ }
```

我曾手动把 `package.json` 升到 3.0.1 再跑 `--bump none`——`oldVersion === newVersion`，**3.0.0 就会从 history 里消失**。

**绕行做法**：先把 `package.json` 退回 3.0.0，再跑 `--bump patch` 升回 3.0.1，让归档条件成立。本轮就是这么做的，日志确认「旧版本 3.0.0 已归档进 history[]」。

### 4.5 `--write-notes` 极易遗漏，遗漏会静默发布空正文

`release:prepare` 有两种互斥模式：
- `--notes-file` 走完整流程
- `--write-notes <文件>` 只生成带构建信息的说明文件，路径为 `E:\办公文件\L1网站\alpha-file-manager\_release-notes-v{版本号}.md`（如 3.0.1 对应的 `_release-notes-v3.0.1.md`，1633 字节），供 `github` 子命令用

漏跑 `--write-notes` 时，`github` 找不到说明文件会**静默发布空正文**。上一版 3.0.0 就中过。本轮跑完后用 `gh release view --json body` 独立回读确认正文 737 字符。

### 4.6 更新说明不要手写大小和 sha256

脚本会自动追加构建信息。手写就会出现「release 说明里写着旧哈希」这类不一致。

### 4.7 vite watcher 会自动补 i18n

`en.json` 一变，开发服务器会自动跑 `npm run sync-i18n`，把缺失 key 以英文值补进其余 16 个语言包。**开发服务器不在时要手动执行。**

---

## 5. 3.0.1 的修复到底改了什么

### 5.1 缺陷

`conflict.rs` 原本只要「目标名落在本批次旧名集合里」就判 C 类让位、不阻断。**但如果被撞名的那一条是 `unchanged`（规则没改动它）条目，它根本不会参与任何 `MoveOp`**，会一直占着那个名字 —— 阶段二的 `fs::rename` 直接盖上去。

端到端复现的真实输出：

```
执行前: a_b.txt =「来自 a_b 的内容」  a-b.txt =「原本属于 a-b 的内容」
执行后: {"a-b.txt": "来自 a_b 的内容"}                        ← 只剩 1 个文件
detect: { conflicts: [], invalid_names: [], needs_staging: [0] }  ← 判定「无冲突、可执行」
```

原内容被销毁，**撤销也救不回来**（`history.rs::build_entry` 会过滤掉 `unchanged` 条目，历史里根本没有那个文件）。

UI 上可达：选中 `a_b.txt` 和 `a-b.txt`，套用「查找替换 `_` → `-`」即可。3.0.0 上点执行就丢数据。

### 5.2 修复

判据从「目标名是否在本批次旧名集合里」改成「**别人**会不会让位」，且**必须排除条目自身**：

1. 存在**另一条会搬走**的条目持有该旧名 → 它腾空后目标名就空了，走 C；
2. 只有 `unchanged` 条目持有该旧名 → 它永远腾不开，按 **A 类阻断**；
3. 唯一持有者就是本条目自己（`a.txt → A.TXT` 这类仅改大小写）→ 既不腾空也不阻断，交给外部存在性检查。

排除自身这一条是必需的：不排除的话，纯大小写改名的旧名规范化后等于自己的目标名，会被误判成撞上未变化条目而阻断，导致「把 .txt 改成 .TXT」这类合法操作直接不可用。

### 5.3 据实测更正的一处既有断言

原测试断言「纯大小写改名必须走腾空路径，理由是 Windows 上直接 rename 有失败风险」。**该前提经实测不成立**：

```
[1] a.txt -> A.TXT 直接 rename = Ok(())
[2] a.txt -> A.txt  直接 rename = Ok(())，目录 listing = ["A.txt"]，内容完好
```

Windows 的 `MoveFileExW` 原地改大小写完全正常，腾空只是平白多两次操作。已按实测更正断言。

### 5.4 前端无需改动

复用已有的 `ConflictKind::TargetExists` 文案（「新名已被目录内其它文件占用。」，对新场景同样准确），对话框的红色行（`--conflict` 类）、`batchRename.dialog.blocked` 提示、禁用执行按钮（`canExecute` 检查 `preview.executable`）全是现成的。

### 5.5 测试状态

**Rust 310 passed / 0 failed**（本轮开始前基线 301，新增 9 条）。
新增的关键测试：
- `conflict::target_hitting_unchanged_sibling_is_blocked_not_staged`
- `conflict::case_only_target_hitting_unchanged_sibling_is_blocked`
- `conflict::pure_case_only_rename_stays_allowed`（防修复写过头）
- `conflict::two_cycle_is_still_staged_not_blocked`（防一律阻断，把真正的交换变成死路）
- `executor::target_colliding_with_unchanged_sibling_must_not_overwrite_it`
- `executor::bypassing_detect_still_destroys_the_unchanged_sibling`（**锁死缺陷本身**，证明 `detect` 的阻断是承重的）
- `mod::sequence_with_descending_sort_produces_a_real_two_cycle` / `..._ascending_sort_is_identity_not_a_swap`
- `executor::ui_reachable_two_cycle_swaps_contents_on_real_fs`

### 5.6 一个反直觉的事实：UI 能构造出真正的 2 环交换

我曾判断「规则合成同一个函数，UI 构造不出 `f(a)=b 且 f(b)=a`」。**那个结论是错的** —— 漏了一点：**`{n}` 取决于条目在排序后的位置**，所以整条映射不是逐名一致的函数。

构造方法（已写成测试跑通，也是真机验证的操作说明书）：

> 选中 `1.txt`、`2.txt` → 规则区「编号顺序」选 **按名称降序** → 加一条**序号**规则（起始 1、步长 1）→ 再加一条**模板**规则 `{n}.txt`
> 降序让 `2.txt` 排前面拿到 n=1、`1.txt` 拿到 n=2，模板一渲染两者正好对调。
> 对照：改成**按名称升序**则是恒等映射，两条都「不变」。

---

## 6. 硬约束（改代码前必读）

1. **最小化修改**：改现有文件只允许**纯追加**，不改既有分支行为。
2. **零新依赖**（第一期已锁定）。
3. **绝不动**：`src-tauri/src/file_operations.rs` 的 `rename_item`、`file-browser-rename-dialog.vue`、`navigator.actions.rename` 现有行为、任何既有 i18n key 的值、`package.json`/`Cargo.toml` 依赖段。
4. **受保护常量不得改动**：mDNS 服务名、legacy 数据目录名、Tauri `identifier`（`com.sigma-file-manager.app`）。
5. **品牌名统一为 Alpha File Manager**，命名与图标决策已锁定，不要重新讨论。
6. **验证优先于流程**：不能只凭 `exit 0` / 脚本「已处理」就宣布完成，必须回读产物实际状态。
7. **判别性验证**：新测试必须先注入缺陷确认转红，再修复恢复转绿。
8. **观察窗口要够长**：本项目扩展冷启动需 3–5 分钟；`/S` 静默安装会跳过 MUI 全部向导页面，**不能用它验收向导类问题**。
9. **PowerShell 5.1 显示中文乱码是终端渲染问题**，非文件损坏；判断完整性用 Node 按 UTF-8 解码并检查 U+FFFD 数量。

---

## 7. 环境：坏掉的工具链与绕法

### 7.1 Rust stable 工具链损坏

必须指定 1.90.0，且做**进程级** PATH 注入：

```powershell
$tc = "C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin"
[Environment]::SetEnvironmentVariable('PATH', "$tc;" + $env:PATH, 'Process')
```

`release-publish.mjs` 内置了这段注入（约 210 行），走脚本时不用手动做。

### 7.2 系统 Python 大面积损坏

发版脚本要用时用 `E:\办公文件\L1网站\.venv-sftp\Scripts\python.exe`。

### 7.3 PowerShell 5.1 执行的中文脚本必须带 UTF-8 BOM

无 BOM 的 `.ps1` 按 GBK 解析，乱码会破坏语法且**报错行号与真实问题对不上**。写之前用：

```powershell
[IO.File]::WriteAllText($p, $t, (New-Object Text.UTF8Encoding($true)))
```

### 7.4 Node 20+ `spawnSync` 跑 `.cmd` 直接 EINVAL

`release-publish.mjs` 的 `run()` 已处理（`.cmd` shim 走 `cmd.exe /d /c`，`gh` 走 `shell: true`）。**不要为了省事把 npm 那条也改成 `shell: true`** —— 站点源在 `E:\办公文件\` 含中文路径，cmd.exe 重新拼接会打散参数（2026-10-10 实测 `gh release create` 因此报 `no matches found for 'File'`）。

---

## 8. 常用命令

### 8.1 门禁

```powershell
# Rust（须先做 7.1 的 PATH 注入）
& "$tc\cargo.exe" test --lib          # 当前基线 310 passed / 0 failed

# 前端
npm run ts                           # 真门禁是 vue-tsc --build，不是 npx vue-tsc --noEmit
npx vitest run                       # 1156 passed / 1 failed（use-navigator-item-icon 为已知 flaky，单跑通过）
```

### 8.2 发版一整套（顺序不能乱）

```powershell
# 0. 写更新说明（纯文本，放 _notes.md，不要手写构建信息）
# 1. 升版本 + 复制到站点源 + 写 version.json
npm run release:prepare -- --bump patch --notes-file ./_notes.md
#    只想重打不升版本： --bump none
#    不想重新构建：     --skip-build
#    ⚠ 见 §4.4：若已手动升过版本号，先退回上一版再 --bump patch，否则旧版本不进 history
# 2. 生成 GitHub 说明文件（极易遗漏，遗漏会静默发布空正文 —— 见 §4.5）
npm run release:prepare -- --write-notes ./_notes.md
# 3. 上传站点并回读校验
npm run release:upload
# 4. GitHub 兜底 release
npm run release:github
# 5. 公网回读
npm run release:verify
```

打包：`npm run tauri:build`（约 2–9 分钟，取决于是否要重编 Rust）。

### 8.3 本项目踩过的其他坑（跨轮次）

- `Move-Item` / `Copy-Item` **保留 LastWriteTime** → cargo 认为没变、不重编译。复制文件做判别性探针时**必须显式设 mtime**。
- 测试注入失败必须「**只失败一次**」，否则回滚本身也被打死，测到的不是回滚逻辑。
- 注入失败判断要**先取文件名**：`from.starts_with("__afm_tmp_")` 对完整路径永远为假。
- Rust 单测若不在 Rust 内部构造枚举，就永远测不到 JSON 线格式。
- 判别性探针是唯一能证明「执行者真的做了」的办法。**一次观察写一个文件** —— `FileOpen ... a` 反复写同一个文件会互相覆盖，只剩最后一行。

---

## 9. 未结事项与下阶段计划

### P0 · 3.0.0 用户可能已丢数据

**3.0.0 的用户仍然暴露在这个缺陷下，3.0.1 无法回溯保护已执行过的操作。** 已在发版说明里写了升级提示，但可以考虑：

- 在 GitHub 上给 v3.0.0 的 release 追加一条「已知问题」标记；
- 在应用内加一条针对 3.0.0 用户的告警。

### P1 · 15 个语言包仍是英文占位

`ch` 是中文，其余 15 个语言包的 `batchRename.*`（81 个 key）和 3.0.1 changelog 条目都是英文占位。这是**已定的方案 A 待翻译状态，不是 bug 也不是完成态**。需要有人工翻译投入。

### P2 · 批量改名的其余真机验收

用户已确认**冲突阻断**实测通过。其余场景尚未在真机上跑过：

- 3 环及以上交换（UI 暂难构造，Rust 层有测试覆盖）
- 撤销在各种失败路径下的还原
- 大批量（数百个文件）时的性能与界面响应

### P3 · 批量改名的功能边界（尚未讨论）

当前**不支持**：

- 跨目录批量改名（只作用于同一目录，`fs::rename` 不会跨卷，因此没有 `file_operations.rs` 那套 copy+delete 回退）
- 撤销时的部分还原（撤销是全批成功或失败的）
- 改名后立即打开文件 / 复制到别处 / 打包等后续动作

### P4 · 工具链修复

见 §7.1 / §7.2，Rust stable 与系统 Python 仍坏着，一直靠绕法。

---

## 10. 新会话必读文档清单（完整路径）

**按顺序读，前两份是必读。**

1. **`F:\soft\00selfmade\filemanager\AGENTS.md`** —— 项目硬约定（输出语言、最小化修改、受保护常量）
2. **本文件** —— `F:\soft\00selfmade\filemanager\docs\handoff-2026-10-11-batch-rename-3.0.1-released.md`
3. `F:\soft\00selfmade\filemanager\docs\PLAN-2026-10-11-batch-rename.md` —— 批量改名的原始设计
4. `F:\soft\00selfmade\filemanager\docs\handoff-2026-10-11-batch-rename-rust-phase.md` —— Rust 侧交接，含三类冲突判定的原始推理与两条回滚路径
5. `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\release-workflow.md` —— 发版流程
6. `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\version-info.md` —— 版本号信息
7. `F:\soft\00selfmade\filemanager\docs\handoff-2026-10-10-2.2.1-released-next-steps.md` —— 双仓结构、更新渠道迁移、NSIS 安装器故障根因
8. `F:\soft\00selfmade\filemanager\docs\handoff-2026-10-10-installer-instdir-rootcause.md` —— 安装器 `$INSTDIR` 归一化
9. `F:\soft\00selfmade\filemanager\docs\PLAN-2026-10-10-aiec-fun-update-channel.md` —— 更新渠道迁移到自建站
10. `F:\soft\00selfmade\filemanager\docs\handoff-2026-10-11-crash-observability.md` —— 崩溃可观测性
11. `F:\soft\00selfmade\filemanager\sigma-file-manager\CHANGELOG.md` —— 版本变更史

### 代码地图（批量改名）

| 文件 | 行数 | 职责 |
|---|---|---|
| `sigma-file-manager\src-tauri\src\batch_rename\mod.rs` | 481 | command 层、`to_planned`、`read_existing_names` |
| `sigma-file-manager\src-tauri\src\batch_rename\conflict.rs` | 483 | `detect` 三类冲突、非法名校验（**安全底线**） |
| `sigma-file-manager\src-tauri\src\batch_rename\executor.rs` | 685 | 两阶段落盘、两条回滚路径、`FsMoveBackend` |
| `sigma-file-manager\src-tauri\src\batch_rename\plan.rs` | 275 | `build_staging_plan`、`temp_name_for`、`invert` |
| `sigma-file-manager\src-tauri\src\batch_rename\rules.rs` | 927 | 规则求值、`title_case`、`apply_rules_with_timezone` |
| `sigma-file-manager\src-tauri\src\batch_rename\history.rs` | 477 | 最近 10 条、`undo` |
| `sigma-file-manager\src-tauri\src\batch_rename\template.rs` | 484 | 模板渲染 |
| `sigma-file-manager\src\types\batch-rename.ts` | — | 前端类型 |
| `sigma-file-manager\src\composables\use-batch-rename.ts` | — | 预览调度、200ms 防抖、时区偏移 |
| `sigma-file-manager\src\composables\__tests__\use-batch-rename.test.ts` | — | composable 单测 |
| `sigma-file-manager\src\modules\navigator\components\file-browser\file-browser-batch-rename-dialog.vue` | — | 主对话框 |
| `sigma-file-manager\src\modules\navigator\components\file-browser\file-browser-batch-rename-history-panel.vue` | — | 撤销历史面板 |
| `sigma-file-manager\src\localization\__tests__\messages-key-alignment.test.ts` | — | 17 语言包 key 对齐门禁 |

---

## 11. 工作方式约定

- **不要问能从代码/环境查出来的事。** 先查，再问。
- **功能描述必须查证后再写**，不能编造。
- **发现顺手可修的明显问题（过期文档、错误默认值、不一致配置），在同一工作范围内直接修掉**，不要甩问题回来问「要不要顺便修」。
- **汇报要说清「这组证据是怎么跑出来的」。** 测试跑了 ≠ 验证通过，尤其是走了另一条路径时。
- **不确定的事写成「不可知」，不要写成「已排除」。**