# 实现计划 —— 目录内文件批量改名（分两期）

**创建**:2026-10-11
**状态**:待用户确认,尚未动代码
**依据**:2026-10-11 调研(见 §9)。结论是**自研**,不移植任何外部项目。

---

## 0. ⚠️ 贯穿全文的最高约束:最小化修改

用户明确要求「不要影响现有功能、代码,不要造成开发成功的部分出现新的 bug,不要造成功能回滚」。

本计划的所有设计都受这条约束支配,具体体现为:

1. **一律新建,不改现有文件的行为。** 新功能走独立模块、独立命令、独立组件,现有单文件改名的代码路径一行不动。
2. **改现有文件时,只允许「新增分支」,不允许「修改既有分支」。** 例如右键菜单可见性只加一条新规则,不改任何现有规则的判断。
3. **每个阶段独立可回滚。** 任一阶段出问题,删掉新建文件 + 回退新增的那几行即可,不需要回滚既有逻辑。
4. **不改任何现有 i18n key 的值。** 只允许新增 key。
5. **第一期不引入任何新依赖。** 依赖变更全部推到第二期,避免影响当前稳定构建。

§4 有一份显式的**「不改动清单」**,是本计划的硬边界。

---

## 1. 现状(已勘察确认,非推测)

| 事实 | 位置 |
|---|---|
| 单文件改名 Rust 命令 | `src-tauri/src/file_operations.rs:1828` `rename_item` |
| 单文件改名对话框 | `src/modules/navigator/components/file-browser/file-browser-rename-dialog.vue` |
| Windows 非法字符校验(可复用) | 同上 `:61`,`/[<>:"/\\\|?*\x00-\x1f]/` |
| 冲突对话框组件(可参考,不复用) | `file-browser-conflict-dialog.vue`、`file-browser-top-level-conflict-dialog.vue` |
| 异步任务 + 进度范式 | `copy_move_job.rs`、`delete_job.rs` |
| 跨盘改名回退(现有逻辑) | `file_operations.rs:791` `try_rename_or_copy_delete` |
| 右键菜单可见性策略 | `utils/context-menu-action-visibility.ts` |
| 语言包 17 个,中文是 `ch` | `src/localization/messages/*.json` |
| **i18n 无 key 对齐测试** | `__tests__/messages-json.test.ts` **只验证 JSON 可解析** |
| **i18n 无 fallbackLocale** | `src/localization/index.ts:9-13`,`createI18n` 未配该字段 |
| **项目无通用撤销机制** | Rust 侧 `undo\|rollback` 命中项全为注册表操作专用 |
| 现有 rename 相关 i18n | `navigator.renameSelectedItems`、`dialogs.renameDirItemDialog.*`、`navigator.actions.rename` |

### 1.1 由上述勘察推出的三个必避坑

**坑 1:i18n 缺 key 不会报错,只会静默显示 key 原文。**
`createI18n` 没配 `fallbackLocale`,`messages-json.test.ts` 也不比对 key。
漏改一个语言包 = 那个语言的界面直接显示 `batchRename.title` 这种字符串,**CI 全绿**。
→ 必须自建 key 对齐门禁,见 §6。

**坑 2:项目没有撤销机制可复用,这是从零新建。**
批量改名的价值有一半在「能反悔」。这不能当成锦上添花往后推。

**坑 3:交换循环是静默数据丢失。**
`a.txt→b.txt` 且 `b.txt→a.txt` 这类映射,顺序执行必然有一个文件被覆盖。
不处理就是用户文件消失。→ 见 §5,必须两阶段临时名。

---

## 2. 分期范围(用户已确认)

**第一期(可独立发版)**:P0 全部 + P1 的模板语法与排序编号。

**第二期**:媒体元数据 —— 图片 EXIF / 视频 / 音频读取,以及音频 ID3 **写入**。
(拖拽排序、TXT 清单导入、规则预设保存推迟到第二期之后。)

---

## 3. 第一期功能清单

### 3.1 规则能力

| 能力 | 说明 |
|---|---|
| 查找替换 | 区分大小写开关;正则模式开关 |
| 序号替换 | 起始值、步长、补零位数 |
| 大小写转换 | upper / lower / title / camelCase / snake_case / kebab-case |
| 扩展名批量修改 | 统一改后缀 |
| 模板语法 | `{name}` `{ext}` `{n}` `{date}` `{parent}` `{size}` |
| 排序编号 | 按当前列表顺序编号;按扩展名过滤处理范围 |

### 3.2 安全能力(缺一不可,这是「敢不敢用」的分界线)

| 能力 | 说明 |
|---|---|
| 实时预览 | 原名→新名双列对照,未变化行置灰 |
| 冲突检测 A | 新名撞上目录内**批次外**的已有文件 → 阻断并提示 |
| 冲突检测 B | 两个条目生成**相同**新名 → 阻断并指出是哪两个 |
| 冲突检测 C | **交换循环** → 自动两阶段中转,用户无感 |
| 非法字符校验 | 复用现有正则规则,新增批量版 |
| 撤销 | 操作历史 + 一键全量回滚 |
| 部分失败可回滚 | 一批改到第 K 个失败,已改的能全部还原 |

---

## 4. 第一期改动清单

### 4.1 新建文件(可独立回滚)

**Rust 侧** —— 全新模块,不侵入 `file_operations.rs`:

```
src-tauri/src/batch_rename/
├── mod.rs              模块声明与命令注册
├── rules.rs            规则模型 + 求值(纯函数,无 IO)
├── template.rs         模板变量解析与渲染
├── conflict.rs         三类冲突检测
├── executor.rs         两阶段执行 + 部分失败回滚
└── history.rs          操作日志读写 + 撤销
```

**前端**:

```
src/modules/navigator/components/file-browser/file-browser-batch-rename-dialog.vue
src/composables/use-batch-rename.ts          状态与 invoke 封装
src/utils/batch-rename-rules.ts             前端预览(与 Rust 侧共用规则语义)
src/utils/__tests__/batch-rename-*.test.ts  单测
```

### 4.2 需要改动的现有文件(共 4 处,均为纯新增)

| # | 文件 | 改动 | 风险 |
|---|---|---|---|
| 1 | `src-tauri/src/lib.rs` | 注册新 command + `mod batch_rename;` | 纯追加 |
| 2 | `src/modules/navigator/components/file-browser/file-browser-*.vue` | 新增对话框挂载点 | 纯追加 |
| 3 | `utils/context-menu-action-visibility.ts` | 新增一条「多选时显示批量改名」规则 | 纯追加,不改现有规则 |
| 4 | 17 个 `messages/*.json` | 新增 key | 纯追加 |

### 4.3 ⛔ 不改动清单(硬边界)

以下文件/行为**本计划期间一律不动**:

- `file_operations.rs` 的 `rename_item` 及其调用链
- `file-browser-rename-dialog.vue`(单文件改名对话框)的任何一行
- `navigator.actions.rename`(右键菜单单项改名)的现有行为
- 任何现有 i18n key 的值
- `package.json` / `Cargo.toml` 的依赖段(第一期零新依赖)
- 受保护常量:mDNS 服务名、legacy 数据目录名、Tauri `identifier`
- 已有的 flaky 测试(`use-navigator-item-icon` / `use-init`),不顺手修

---

## 5. 核心算法:交换循环的两阶段处理

这是本功能唯一有技术含量的地方。

**问题**:`a.txt→b.txt`、`b.txt→a.txt`。顺序执行时,第一个把 a 改成 b(b 还不存在,成功),第二个把 b 改成 a —— 但 b 已经是第一个的结果,原 b 已被覆盖。数据丢失。

**不采用**「检测路径图中的环」——算法复杂且易漏。

**采用两阶段腾空法**,它对环是天然免疫的:

```
阶段 1(腾空):
  对每个条目,若其目标名已被本批次中任何条目占用,
  先改成一个唯一临时名 __afm_tmp_<uuid>.<原扩展名>

阶段 2(落位):
  全部从临时名改成最终名
```

举例 `a.txt→b.txt`、`b.txt→a.txt`:

```
阶段 1: a.txt → __afm_tmp_1.txt
       b.txt → __afm_tmp_2.txt
阶段 2: __afm_tmp_1.txt → b.txt
       __afm_tmp_2.txt → a.txt
```

`a→b→a→b→a` 三环同样正确,**不需要检测环的存在**。

**关键区分**:只有「目标名撞上本批次内的条目」才走腾空;若目标名撞上的是**批次外**的已有文件,不走腾空、直接报冲突阻断。腾空只能腾本批次的位置,不能侵占外部文件。

**撤销时同样跑一遍两阶段**,因为反向映射 `b→a`、`a→b` 又是一个交换。

---

## 6. i18n key 清单与同步门禁

### 6.1 预估 key 数量

约 55 个 key × 17 语言包 ≈ **935 条翻译**。

分组:`batchRename.dialog.*`(标题/按钮)、`batchRename.rules.*`(规则名)、
`batchRename.conflicts.*`(冲突提示)、`batchRename.templates.*`(模板语法说明)、
`batchRename.history.*`(撤销)、`navigator.actions.batchRename`(菜单入口)。

**具体 key 清单在动工时随 UI 一起定稿**,先冻结分组,避免边写边加导致语言包反复返工。

### 6.2 ⚠️ 必须自建 key 对齐门禁

现状:`messages-json.test.ts` 只验证 JSON 可解析,不做 key 对齐;`createI18n` 无 `fallbackLocale`。
**漏改语言包不会有任何报错,只会静默显示 key 原文。**

所以第一期必须新增一个测试:`src/localization/__tests__/messages-key-alignment.test.ts`

- 遍历 `messages/*.json`,递归收集所有 key 路径
- 断言**每个**语言包的 key 集合与 `en.json` **完全相等**
- 输出缺失/多余的具体 key 路径,便于定位

这个门禁是**新增测试文件**,不改现有测试,符合最小化修改原则。
它同时会暴露历史遗留的 key 不对齐(如果存在),届时需评估是否阻塞 —— 建议第一版只**报告不失败**在明细里,或以 `en` 为基准单方向校验,避免误伤。

---

## 7. 验收标准

### 7.1 单元测试(Rust 侧,基线 204 passed / 0 failed 不得下降)

- 规则求值:每种规则 × 典型输入 × 边界(空文件名、超长名、全角字符)
- 模板渲染:未知变量、嵌套花括号、`{n}` 补零
- 冲突检测 A/B/C **各至少 3 个用例**,含 `a↔b` 交换、`a→b→c→a` 三环、`x→x` 自映射
- 判别性验证:**故意注入一个缺陷**(如把「跳过自身」写成「不跳过」),确认测试会红 —— 防止写出恒真的假绿测试

### 7.2 前端测试

- 前端预览与 Rust 侧求值结果**逐条一致**(同一批输入,两边算出的新名必须相同)
- i18n key 对齐门禁全绿

### 7.3 真机手工验证(不可省略)

**不能用静默路径代替**。批量改名是破坏性操作,必须真机跑:
- 建一个测试目录,复制 20 个真实文件
- 覆盖:普通替换、序号编号、交换循环(`a↔b`)、制造冲突(`x→y` 而 y 已存在)
- 确认:交换循环文件**内容未丢**;冲突被**阻断**而非静默覆盖;撤销能**完整还原**

### 7.4 回归门禁

```
npx vue-tsc --noEmit                          # exit 0
npx eslint <改动文件> --max-warnings 0         # exit 0
npx stylelint <改动文件> --max-warnings 0      # exit 0
npx vitest run                                 # 不低于基线
cargo check --lib / cargo test --lib           # 不低于 204 passed
```

已知 flaky(`use-navigator-item-icon`、`use-init`)若红,须**逐文件复跑定性**,不得只看总数。

---

## 8. 第二期概要(不在本期实现)

| 步骤 | 内容 | 风险 |
|---|---|---|
| 2-A | 媒体元数据抽象 trait + 图片 EXIF + 视频(MP4/MOV)+ 音频 ID3 **读取**,接入第一期已有的模板变量 | 低(只读) |
| 2-B | 音频 ID3 **写入** + 四项安全机制 | **高(破坏性)** |

### 8.1 第一期就必须做对的预留设计

**模板变量的取值来源要做成 trait 抽象**,第一期只实现「文件系统属性」,第二期加「媒体属性」实现同一 trait。
否则第一期把模板写成字符串拼接,第二期 2-A 必须返工重写。

### 8.2 2-B 的四项安全机制(不可省)

1. **占用跳过**:文件被播放器/预览占用时跳过并继续,最后汇总「成功 N / 失败 M」并给重试入口,**不得整批中止**
2. **标签级回滚**:每次写入前把原标签内容记入操作日志(标签块很小,无需备份整个音频文件)
3. **mtime 还原**:写标签会改文件修改时间,写完应还原原值,否则用户「按修改时间排序」的列表会突然重排
4. **写后回读校验**:写完重新读一次比对,不一致即标记失败并回滚 —— ID3 编码有超长字符串、非法字符等边界,静默损坏比报错更糟

### 8.3 已知技术边界

`.mp4`/`.mov` 可读;**`.mkv`/`.webm`/`.avi` 不支持**(容器格式不同,覆盖需引入 FFmpeg 级依赖)。
该边界需在 UI 上明说,不能让用户选了一堆 MKV 然后静默跳过。

### 8.4 新增依赖(第二期)

`kamadak-exif`、`id3`、`mp4` —— 均为轻量纯 Rust 库,对安装包体积影响可控。

---

## 9. 调研依据:为什么不移植

数据取自 GitHub / crates.io API 原始接口,非搜索摘要。

| 项目 | 星标 | Fork | 许可 | 技术栈 | 结论 |
|---|---|---|---|---|---|
| [PowerToys](https://github.com/microsoft/PowerToys)/PowerRename | 139,170 | 8,636 | MIT | C++/WinUI3 | 仅作设计标杆,代码不可用 |
| [Files](https://github.com/files-community/Files) | 45,893 | 2,953 | MIT | C#/WinUI3 | 仅作设计标杆,代码不可用 |
| [f2](https://github.com/ayoisaiah/f2) | 2,454 | 66 | MIT | Go/CLI | 抄设计(预览默认开、变量系统、操作日志) |
| [regex-batch-renamer](https://github.com/junyou1998/regex-batch-renamer) | 5 | **0** | MIT | Tauri+Vue3 | **陷阱**:技术栈一致但默认分支名为 `backup/beta-pre-cleanup-*`,作者自述未完成,不抄 |
| [bulk-rename-rs](https://github.com/fredyw/bulk-rename-rs) | 1 | 0 | Apache-2.0 | Rust | **陷阱**:自称 library 但 crates.io 分类是 `command-line-utilities`、`lib_links: null`(零反向依赖)、总下载 214 次;核心卖点 Python 脚本会拖进 `rustpython-vm` |

**结论**:高星项目全是独立应用/CLI,抠核心逻辑等于用另一种语言重写,成本与自研持平还多背一层翻译风险。
批量改名本质是纯函数 `规则 + 文件名 → 新文件名`,无状态无 IO,Rust 侧核心约 300–500 行,自研成本可控。

---

## 10. 风险与回滚

| 风险 | 缓解 |
|---|---|
| 交换循环导致数据丢失 | §5 两阶段腾空,单测覆盖 2 环与 3 环 |
| 部分失败留下半改状态 | executor 记录已完成的映射,失败时反向回滚 |
| i18n 漏改语言包且无人察觉 | §6.2 新增 key 对齐门禁 |
| 改动波及现有改名功能 | §4.3 不改动清单 + 现有 rename 路径零改动 |
| 大批量操作卡 UI | 走 `copy_move_job.rs` 的既有 job 范式,不阻塞主线程 |