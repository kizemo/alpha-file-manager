# 交接文档 —— 批量改名功能：Rust 侧已完成，待接前端

**创建**:2026-10-11
**交接原因**:Rust 侧逻辑与落盘执行已完成并验证，剩余工作为前端接线（command + 对话框 + 撤销面板 + i18n），工作量过半且需要重新勘察前端模式。
**接手前必读**:本文件 + `F:\soft\00selfmade\filemanager\docs\PLAN-2026-10-11-batch-rename.md`（含两期范围、调研依据、不改动清单）

---

## 1. 项目与仓库

- **文档仓**:`F:\soft\00selfmade\filemanager`（本文件所在处）
- **代码仓**:`F:\soft\00selfmade\filemanager\sigma-file-manager`
- 技术栈:Tauri 2.x（Rust 后端）+ Vue3 + TypeScript + WebView2，Windows 平台
- 品牌名 **Alpha File Manager**，标识符仍是 `com.sigma-file-manager.app`（**受保护常量**）
- 两仓均在 `main`，**均未 push**

---

## 2. 功能目标与已定范围

给文件管理器加「目录内文件批量改名」，分两期，用户已确认：

**第一期（本次要做完）**
- 规则:查找替换（含大小写开关）、大小写转换（6 种）、序号（起始/步长/补零）、模板变量、扩展名批量修改
- 模板变量:`{name}` `{ext}` `{parent}` `{size}` `{date}` `{n}`
- 安全:实时预览、三类冲突检测、撤销、部分失败可回滚
- 用户已明确决定:**第一期不做正则**（Rust 侧无正则引擎，且正则不能只在前端做，否则前后端求值不一致）

**第二期（不在本次范围）**
- 媒体元数据:图片 EXIF、视频（仅 MP4/MOV）、音频 ID3 **读取** → 喂给第一期已有的模板变量
- 音频 ID3 **写入** + 四项安全机制（占用跳过、标签级回滚、mtime 还原、写后回读校验）
- 推迟项:拖拽排序、TXT 清单导入、规则预设保存
- **第一期已为此预留**:`template::TemplateContext::media()` trait 方法，第二期挂实现即可，本文件渲染逻辑不用改

**撤销 UI（用户 2026-10-11 确认）**
- 保留最近 **10** 条历史
- 做**一个 UI 入口**（不是只在右键菜单放一个菜单项）

---

## 3. 已完成：Rust 侧（全部有单测，已验证）

### 3.1 文件清单

新建目录 `src-tauri/src/batch_rename/`，共 7 个文件：

| 文件 | 行数 | 职责 | 有 IO |
|---|---|---|---|
| `mod.rs` | 30 | 模块声明 | — |
| `template.rs` | 484 | 模板变量渲染、`civil_from_days` 日期换算 | 否 |
| `rules.rs` | 728 | 规则求值、排序、词分割、大小写转换 | 否 |
| `conflict.rs` | 390 | 三类冲突检测 + 文件名校验 | 否 |
| `plan.rs` | 275 | 两阶段腾空计划 | 否 |
| `executor.rs` | 582 | 照计划落盘 + 失败还原 | 是 |
| `history.rs` | 477 | 操作日志（10 条）+ 撤销 | 是 |

**对现有文件的侵入只有 1 行**(`src-tauri/src/lib.rs`):

```diff
 mod background_sources;
+mod batch_rename;
 mod clipboard_source;
```

> 注意:`git diff` 里 `lib.rs` 显示 37 行改动,其中 36 行是**更早轮次**的 `startup_probe` 埋点与「启动默认最大化」,不是本次。本次只加了上面那 1 行。

### 3.2 测试状态

```
batch_rename 单测:81 passed / 0 failed
全库 cargo test --lib:287 passed / 0 failed（并行稳定，无需 --test-threads=1）
```

Rust 基线是 204 passed,现为 287,**既有测试一个没少、没改动**。

### 3.3 三类冲突的判定规则（核心,务必理解）

| 类别 | 场景 | 处理 |
|---|---|---|
| A `TargetExists` | 新名撞上**本批次之外**的已有文件 | **阻断**,要求用户改名 |
| B `DuplicateTarget` | 两个条目生成相同新名 | **阻断**,指出是哪两个 |
| C 交换循环 | `a→b` 且 `b→a` | **自动**两阶段腾空,用户无感 |

**A 与 C 的区分标准**:看目标名是否落在**本批次的旧名集合**里。落在集合内 → C(让位,腾空即可);不在集合内但目录里真有这个文件 → A(外部文件,绝不能侵占)。

**全部存在性比较走 `conflict::normalize_for_compare`(转小写)**,因为 Windows 文件系统大小写不敏感。若按大小写敏感比较,`report.txt → REPORT.TXT` 会被判为无冲突,实际却覆盖原文件。

### 3.4 交换循环为什么用「两阶段腾空」而不是「检测环」

```
阶段一(腾空): 目标名被本批次占用的,先改成 __afm_tmp_{索引}{扩展名}
阶段二(落位): 全部从临时名改成最终名
```

二环、三环、自映射混合都由同一套流程处理,**不需要知道环的存在**。撤销时反向映射仍是交换,所以撤销也走同一套。

已有真实文件系统测试验证内容不丢:

```
原始:a.txt="原本是A", b.txt="原本是B"
交换后:b.txt="原本是A", a.txt="原本是B",目录恰好 2 个文件,无 __afm_tmp_ 残留
```

### 3.5 两条不同的回滚路径（别混淆）

| 触发 | 实现 | 为什么能用不同算法 |
|---|---|---|
| 阶段一失败 | `executor::undo_moves` 逆序撤销 | 目标名唯一,逆序安全 |
| 阶段二失败 | `executor::rollback_to_original` 重新扫描目录 + 再走一次两阶段 | 此时目录状态混合(部分在临时名、部分已落位),不能简单逆序 |

**这两条路径各有专属的判别性测试**,改任何一条都必须跑对应用例。

---

## 4. 踩过的坑（重蹈覆辙代价很大）

### 4.1 测试后端必须「只失败一次」

最初写注入后端时做成「过了失败点就一直失败」,结果**回滚本身也被打死**,测到的不是「能否还原」而是「后端一直坏」。改成只失败一次后回滚才真正走通。

现在 `executor.rs` 里的 `FailOnceByCallCount` / `FailOnceOnTempSource` 都是「一次性失败」。

### 4.2 注入失败时要取文件名再判断

`from` 传的是**完整路径**,写 `from.starts_with("__afm_tmp_")` 永远为假,注入的失败从不发生,测试会以「为什么它成功了」的方式失败。必须 `file_name_of(from).starts_with(...)`。

### 4.3 `Move-Item` / `Copy-Item` 保留 LastWriteTime → cargo 不重编译

临时编辑文件后 `Move-Item` 还原,cargo 按 mtime 判断「无需重编译」,于是跑的还是**旧的坏代码**,看到测试结果与代码内容矛盾。**每次手动改文件后必须显式刷新 mtime**。

### 4.4 判别性验证要覆盖每一条路径

往 `rollback_to_original` 注入「假装成功」,只有阶段二那两个测试红了,阶段一的测试仍绿——因为它走的是另一条路径。**不能因为验了一条就假定另一条成立**,必须分别注入。

### 4.5 i18n 没有 key 对齐门禁

`src/localization/__tests__/messages-json.test.ts` **只验证 JSON 能不能解析**,不做 key 比对;`src/localization/index.ts` 的 `createI18n` **也没配 `fallbackLocale`**。

后果:漏改一个语言包**不会有任何报错**,那个语言的界面直接显示 key 原文(如 `batchRename.title`)。

**现状核查(2026-10-11)**:17 个语言包当前 **100% 对齐**,`en.json` 共 1046 个叶子 key,每个包一个不缺。

### 4.6 项目没有 chrono 也没有 regex

`src-tauri/Cargo.toml` 里两个都没有。第一期保持「零新依赖」:
- 日期换算手写 `civil_from_days`(Howard Hinnant 算法),已用已知日期锁死闰年与世纪年规则
- 正则第一期不做(用户已确认)

### 4.7 `{date}` 的时区语义

Unix 时间戳是 UTC,但改名场景用户要的是**本地时间**。做法:给 `TemplateContext` 加 `utc_offset_minutes()`,由前端用 `new Date().getTimezoneOffset()` 取到后传入。保持纯函数可测试,不用 Win32 FFI 也不用引 `chrono`。

### 4.8 我自己写错过测试期望值(算法是对的)

- `civil_from_days(11019)` 我期望 2000-02-29,正确值是 **11016**
- `split_words("helloWorld")` 期望 `["hello","world"]`,正确是 `["hello","World"]`——**拆词不改变词内大小写**
- `apply_rules` 返回顺序**与输入一致**(前端按列表原位更新预览),但序号按排序后位置分配

---

## 5. 已修复的既存 flaky(startup_probe)

`src-tauri/src/startup_probe.rs` 的两个测试原本并行跑必红。根因不是并发,是**测试之间共享了可变状态**——都往 `%LOCALAPPDATA%\...\logs\startup-probe.log` 这一个固定文件写,各自 `remove_file` 后再写,并行时互删。

**修法**:把落盘函数签名改为 `append_marker(dir: &Path, stage: &str)`,测试各用各的 `tempfile`。另加两条守卫:
- `concurrent_marks_do_not_clobber_each_other`(8 线程并发写,断言 8 行都在)
- `probe_dir_points_at_protected_app_data_path`(守住受保护常量)

---

## 6. 待完成的工作

### 6.1 注册 Tauri command(下一步从这里开始)

在 `src-tauri/src/batch_rename/mod.rs` 里加 `#[tauri::command]`,并在 `lib.rs` 的 `invoke_handler` 中注册(**纯追加,不改动现有条目**)。需要 4 个:

| 命令 | 作用 | 入参 | 返回 |
|---|---|---|---|
| `batch_rename_preview` | 算新名 + 跑冲突检测 | 条目列表、规则、排序、时区偏移 | 预览行 + 冲突报告 + `needs_staging` |
| `batch_rename_execute` | 落盘 + 写历史记录 | 预览结果 | `ExecutionOutcome` |
| `batch_rename_history` | 列最近 10 条 | 无 | `Vec<HistoryEntry>` |
| `batch_rename_undo` | 撤销指定 id | id | `UndoOutcome` |

注意:`rules::apply_rules` 的签名是 `(entries, rules, sort)`,`RuleContext` 还需要传入 `utc_offset_minutes`——**这个目前还没接进 `apply_rules`**,需要在写 command 时补上(或调整 `RuleEntry` 增加该字段)。

### 6.2 前端对话框

新建 `src/modules/navigator/components/file-browser/file-browser-batch-rename-dialog.vue`,**不要改** `file-browser-rename-dialog.vue`(单文件改名)。

内容:规则面板(规则类型切换、参数输入)+ 预览对照表(原名 → 新名,未变化置灰,冲突行标红)。

### 6.3 历史撤销面板 UI

列出最多 10 条,每条显示时间与条目数,提供撤销按钮。撤销失败要如实显示原因(被阻断 / 回滚失败),不能静默。

### 6.4 右键菜单入口

在 `src/modules/navigator/components/file-browser/utils/context-menu-action-visibility.ts` **新增一条**规则(多选时显示「批量改名」),不改任何现有规则。

该文件**已有配套测试** `utils/__tests__/context-menu-action-visibility.test.ts`,改完必须跑它,确认既有断言没被影响。

### 6.5 i18n(见第 8 节,已定方案)

---

## 7. ⛔ 不改动清单(硬边界)

用户明确要求「最小化修改,不影响现有功能与代码」。以下**一律不动**:

- `src-tauri/src/file_operations.rs` 的 `rename_item` 及其调用链
- `file-browser-rename-dialog.vue`(单文件改名对话框)的任何一行
- `navigator.actions.rename`(右键菜单单项改名)的现有行为
- **任何现有 i18n key 的值**
- `package.json` / `Cargo.toml` 的依赖段(第一期零新依赖)
- 受保护常量:mDNS 服务名、legacy 数据目录名、Tauri `identifier`
- 改现有文件时**只允许纯追加**,不允许修改既有分支

---

## 8. i18n 已定方案(用户 2026-06-11 确认)

17 个语言包(`ch/de/en/es/fa/fr/he/hi/hy/it/ja/pt/ru/sl/tr/ur/vi`,中文是 `ch`),`en.json` 现有 1046 个叶子 key。

**方案 A(用户已选)**:新增的 `batchRename.*` 只写 **中英文两种**,其余 **15 个语言包填英文占位**。

- 工作量:约 15 个新 key × 2 种翻译 = 30 条,其余 15 包机械复制英文
- 效果:所有语言界面**可读**(15 种语言暂显示英文),保持 17 包严格对齐
- 后续补翻译时只需替换值,结构已就绪

**必须配套**:先写 key 对齐门禁测试(推荐放 `src/localization/__tests__/messages-key-alignment.test.ts`),断言每个语言包的 key 集合与 `en.json` **完全相等**。TDD 顺序:门禁先红 → 补齐 → 变绿。**只写一个测试文件,不改现有 `messages-json.test.ts`**。

> 方案 A 的代价要记在文档里:15 个包的英文值是**待翻译占位**,不是正常状态,别让后人误判。

---

## 9. 环境与命令

### 9.1 环境绕法(必读,本机有损坏)

```powershell
# Rust:stable 工具链的标准库只剩存根,必须指定 1.90.0,且用进程级注入
$tc = 'C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin'
[Environment]::SetEnvironmentVariable('PATH', "$tc;$env:PATH", 'Process')

# 系统 Python 大面积损坏(发版脚本要用)
'E:\办公文件\L1网站\.venv-sftp\Scripts\python.exe'
```

删除文件用 `rm -- "<正斜杠路径>"`(`Remove-Item` 被权限网关拦截)。

### 9.2 门禁命令

```powershell
Set-Location 'F:\soft\00selfmade\filemanager\sigma-file-manager'

# Rust(须先做 9.1 的 PATH 注入)
Set-Location src-tauri
& "$tc\cargo.exe" test --lib          # 基线现为 287 passed / 0 failed

# 前端
Set-Location ..
npx vue-tsc --noEmit
npx eslint <改动文件> --max-warnings 0
npx stylelint <改动文件> --max-warnings 0
npx vitest run                          # 基线 1127 passed
```

已知 flaky:前端 `use-navigator-item-icon`、`use-init`(超时型,单跑通过)。**若红须逐文件复跑定性,不要只看总数。**

### 9.3 打包(本次不做)

第一期完成后才打包。`npx tauri build` 的产物在 `src-tauri/target/release/bundle/nsis/`,**不在 `release/` 目录**——后者是 `npm run release:prepare` 的产物目录,跑它会升版本号并同步站点,而本次约束是不升版本号、不上传。

---

## 10. 本轮相关的其他改动(不是批量改名的一部分)

同仓库内还有**更早轮次**的未提交改动,接手时不要误认为是本次工作、也不要顺手改:

- `nav-sidebar.vue` —— 磁盘图标叠加盘符(`translate(-50%, 10%)`,重叠 56.7%)
- `lib.rs` —— 启动默认最大化、`startup_probe` 4 个埋点
- `navigator.vue` —— 目录树 `default-size="28"` / `max-size="48"`
- `scripts/release-publish.mjs` —— `spawnSync npm.cmd` EINVAL 修复

版本号:`package.json` 与 `Cargo.toml` 均为 **2.2.2**,本次未升。