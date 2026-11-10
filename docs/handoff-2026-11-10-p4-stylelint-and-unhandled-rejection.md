# P4 交接：stylelint 174 错清零 + 地址栏 unhandled rejection 归零

- 日期：2026-11-10
- 分支：`main`，基线 HEAD `070303a9`（P3 收口交接）
- 版本：仍为 **3.13.5**（本轮是 lint / 健壮性改动，不动版本号）
- 品牌：**Alpha File Manager**（未改）
- 受保护常量（mDNS 服务名、legacy 数据目录名、Tauri `identifier`）：**未改**

---

## 1. 本轮目标与结果

| # | 目标 | 结果 |
|---|------|------|
| 1 | stylelint 174 错清零，且证明渲染未变 | ✅ 174 → 0；静态语义等价 + 24 文件 template/script 逐字节未变 + 4 路由像素 diff |
| 2 | 查清并修掉 `address-bar-editor-utils.ts` 的 unhandled rejection | ✅ 根因坐实（两处），生产侧 + 测试侧均已修；全量单测 0 unhandled |

---

## 2. 结论一：stylelint 174 错清零

### 2.1 修复内容

- `npm run lint:stylelint`（= `stylelint "src/**/*.{css,vue}" --fix`）自动修复 **173** 处，覆盖 **24** 个 `.vue`。
- 唯一不可自动修的 1 处是 `no-duplicate-selectors`：`compare-sync-dialog.vue` 有两条同名
  `.compare-sync-dialog__progress-bar` 规则。**按 CSS 层叠，后一条覆盖前一条的 `height` 与
  `background-color`**，因此合并为一条并保留最终生效取值
  （`overflow/height/border-radius/background/transition` 全部按后一条为准），并留中文注释说明。
- 顺带 `npm run lint:eslint`（= `eslint . --fix`）自动清掉 4 个 `@stylistic/indent` 错。

### 2.2 「渲染未变」的三层证明

**第一层 · 静态语义等价**（`.superpowers/sdd/p4-css-impact.cjs`，日志 `p4-css-impact-final.log`）

- 435 条规则逐条比对 HEAD ↔ 工作树，**结构变化 1**（即上面那次有意的规则数 57 → 56）。
- **语义差异 7 处，全部是等价值归一化**，无一处改变层叠结果：
  - `hsl(38 92% 50%)` → `hsl(38deg 92% 50%)`
  - `hsl(var(--x) / 0.12)` → `hsl(var(--x) / 12%)`（0.12 ≡ 12%）
  - `currentColor` → `currentcolor`（大小写归一）
- 顺序敏感点已人工核对：`quick-access-section.vue` 的 `padding` / `padding-inline-start` 相对顺序未变；
  `nav-sidebar.vue` 重复 `height` 的先后未变；`compare-sync-dialog.vue` 合并后的层叠结果与 HEAD 等价。

**第二层 · 只动了 `<style>`**（`.superpowers/sdd/p4-vue-region-check.cjs`，日志 `p4-vue-region-check.log`）

- 24 个 `.vue` 文件逐个比对：`<template>` 块与 `<script>` 块**与 HEAD 逐字节相同**，
  `outside_style_diffs=0`。即不可能有模板或逻辑变化。

**第三层 · 打包真机像素 diff**（打包产物装机后跑，见 §4）

---

## 3. 结论二：地址栏 unhandled rejection 归零

### 3.1 根因（两处，都坐实了）

**根因 A · 生产侧：readDir 载荷可能没有 `entries` 数组**

`resolveAddressBarSuggestions` 直接取 `parentContents.entries`。该载荷来自 IPC 边界，
不保证带 `entries` 数组；缺失时会对非数组调 `.filter()` 抛 `TypeError`。
调用链是 fire-and-forget 的异步链 ⇒ 抛错变成 **unhandled rejection**（不报错码、不中断流程）。

修复：`requireDirEntries()` 校验载荷，非法即抛错 → 落进既有 catch 分支 → 回退为「读取当前目录」，
语义与降级行为不变。`searchEntries` 初始化改用 `Array.isArray` 兜底，
`deduplicateDirEntriesByPath` 跳过无 string `path` 的候选。

**根因 B · 测试侧：dialog 测试的 store mock 不完整**

`address-bar-editor-dialog.test.ts` 把 `useDirSizesStore` mock 成空对象 `{}`，
而地址栏 quick-search 匹配器会调 `dirSizesStore.getSize(entry.path)`，
异步链里抛 `TypeError: dirSizesStore.getSize is not a function` → 同样是 unhandled rejection。

修复：mock 补齐 `getSize / getSizeValue / getStatus / isLoading`。

### 3.2 复现与验证过程（重要）

- 在 **HEAD** 上跑全量单测：300 files / 3141 tests 全绿、**无** unhandled —— 该现象非稳定复现。
- 改动后第一轮跑：3142 passed，但出现 1 个 unhandled，报错文案与交接文档记录的
  `reading 'filter'` **不同**，是 `getSize is not a function`，来源
  `__tests__/address-bar-editor-dialog.test.ts`。⇒ 说明当时有两个不同的未捕获来源，
  本轮两个都修掉了。
- 补完 mock 后复跑全量：**300 files / 3142 tests 全绿，0 unhandled，EXIT=0**。

### 3.3 回归测试

新增用例「treats a readDir payload without entries as a failed listing instead of throwing」：
`mode: 'path'` + 无 `entries` 的 readDir 载荷 ⇒ 断言 `state.kind === 'search'` 且 0 items。

---

## 4. 验证证据（已验证）

### 4.1 门禁

| 命令 | 结果 |
|------|------|
| `npm run ts` | **EXIT=0** |
| `npm run lint:eslint:check`（`eslint . --max-warnings 0`） | **EXIT=0** |
| `npm run lint:stylelint:check`（`stylelint --max-warnings 0`） | **EXIT=0** |
| `npm run test:unit:run` | **EXIT=0**，Test Files 300 passed (300)，Tests 3142 passed (3142)，无 unhandled |

日志：`.superpowers/sdd/p4-gates.log`、`.superpowers/sdd/p4-vitest-final.log`。

### 4.2 打包与装机（真机证据来自打包产物，不是 `tauri dev`）

- 打包：`npm run tauri:build`，产物 `src-tauri/target/release/bundle/nsis/Alpha File Manager_3.13.5_x64-setup.exe`
  = **45,400,147 B @ 2026-11-10 20:28:40**（P3 旧包 45,407,027 B，已备份到
  `.superpowers/sdd/p3-installer-backup/`）。
- `src-tauri/target/release/alpha-file-manager.exe` = 82,334,208 B @ 20:28:41。
- 静默装机（`/S`）后：`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`
  = 82,334,208 B，**尺寸与构建产物一致**，`VERDICT=INSTALLED`，
  SHA256 `1B0AAE54F338BF76038BA4C21BBE900F468E098038339A304523B3B9ABEE9842`。
- 脚本：`.superpowers/sdd/p4-build-run.ps1`、`.superpowers/sdd/p4-install-run.ps1`。

### 4.3 视觉回归（修复前基线 vs 修复后装机产物）

基线：`.superpowers/sdd/shots/p3-p4base-{home,navigator,dashboard,settings}-0-baseline.png`
修复后：`.superpowers/sdd/shots/p3-p4after-*-0-baseline.png`
（两轮都用隔离沙箱桌面 + 固定 seed `--route=<r> --tree=true`，seed 目录 `C:/Users/Public/afm-demo-390`）

分区像素差分（`p4-diff.ps1`，容差 30）：

| 路由 | 全图差异像素 | 占比 | 判定 |
|------|--------------|------|------|
| settings | **0** | 0% | 完全一致 |
| home | 119 | 0.011% | 全部落在 navtree 的**磁盘占用仪表读数**（74%/108.4 GB → 75%/106.7 GB），构建写盘导致实时值变化 |
| navigator | 99 | 0.009% | 全部落在信息面板的**文件时间戳**（20:08 → 20:30） |
| dashboard | 83 | 0.008% | 落在近期条目行的 3 个字形簇（x≈192/326/591），差异像素均为**灰阶抗锯齿边缘**，字形形状与位置逐像素目视一致 |

差异掩码与放大裁剪：`shots/p4-mask-*.png`、`shots/p4-crop-*.png`；差分日志
`.superpowers/sdd/p4-pixel-diff.log`。

### 4.4 同构建自比（用来区分「样式位移」与「运行间抖动」）

同一个**新构建**再采一组截图（`p4after2`），与 `p4after` 自比，日志
`.superpowers/sdd/p4-selfdiff.log`：

| 路由 | 同构建自比差异 | 判定 |
|------|----------------|------|
| dashboard | **83 px，包围盒 (192,703)-(595,711)** —— 与 before/after 的 83 px **完全同址** | ⇒ 该差异是**运行间抖动**（同构建两次跑出同样的字形 AA 差），与本次改动无关 |
| home | **0 px** | ⇒ 该路由稳定；before/after 的 119 px 确属磁盘占用实时读数变化 |
| navigator | 37,323 px (3.4%)，statusbar 32.9% | ⇒ 该路由截图**带悬停态**：第二次运行出现了悬停提示气泡与高亮行（探针注入鼠标的落点随窗口内容变化），属探针固有噪声 |

⇒ 结论：navigator 的 before/after 差异（99 px）比它自身的运行间噪声（37,323 px）小两个数量级，
不足以支撑任何样式变化的结论；dashboard 的 83 px 已证明是同址抖动；settings 为严格 0 差异。

---

## 5. 未验证 / 勿当已验证

- **键盘路径**：沙箱桌面没有键盘通道，未覆盖。
- **UIA / DOM 断言不可用**：Rust 重索引期间 UIA `FindFirst` 永久阻塞，只能用像素取证。
- **未被截图覆盖的界面**：compare-sync 弹窗、file-lock 弹窗、settings 子分类
  （mcp-server / connection-form / trusted-hosts）、quick-access hover 面板 —— 这些只有静态分析结论
  （template/script 逐字节未变 + CSS 层叠等价），没有像素证据。
- **未 push**：`2ab78e1b`、`070303a9` 及本轮提交全部只在本地，推送需明确授权。

---

## 6. 交接文档遗留项中仍未获授权的三件事

1. **push 提交**（需用户明确授权）。
2. **打包场景下把 scanner / sandbox 一起入包做门禁校验**（另立需求）。
3. **沙箱键盘通道验收**（要改 `tauri.conf.json` 并重打两次包，代价高，本轮放弃）。

---

## 7. 本轮踩到的工具坑（留给下一个人）

1. **`npm exec -- vitest run <file>` 会把 vitest 根解析到父目录** ⇒ `@/` 别名与 `@vitejs/plugin-vue` 失效，
   报 `Cannot find package '@/utils/normalize-path'` / `invalid JS syntax`。
   **必须用项目自带脚本** `npm run test:unit:run`（= `node ./scripts/vitest-run.mjs run`）。
2. **`git commit -F -` 不读 stdin**（`-F` 只接受文件名）。长提交信息写临时文件再 `-F <file>`。
3. **后台构建不要挂 stdout 管道**，用 `.cmd`/`.ps1` 内部单次 `>` 重定向；
   **判成功看产物 mtime，不要看退出码**（NSIS `/S` 可能退出 0 但没换掉文件）。
4. **`p4-diff.ps1` 的 `whole` 行哨兵值必须是 `rx = -1`**：写成 `rx=0, rw=-1` 会让循环体一次都不执行，
   全图行恒报 0 changed（这个坑本轮真实踩到，掩盖过 home/navigator 的差异）。
5. **不要用 PowerShell `Get-Content -Raw` / `Set-Content` 改带中文注释的源文件**；
   批量改 JSON 用 node `fs` 显式 utf8。
6. `.superpowers/sdd/*.cmd|*.ps1` 必须**纯 ASCII**（PS 5.1 按 ANSI 读无 BOM 文件）。

---

## 8. 本轮新增脚本索引

| 脚本 | 作用 |
|------|------|
| `.superpowers/sdd/p4-css-impact.cjs` | HEAD ↔ 工作树 CSS 规则逐条语义比对（含 shorthand 展开与 per-selector 合并比较） |
| `.superpowers/sdd/p4-vue-region-check.cjs` | 校验 .vue 的 template/script 块逐字节未变 |
| `.superpowers/sdd/p4-baseline-sweep.ps1` | 4 条主路由批量截图（`-Prefix` 可换前缀采不同批次） |
| `.superpowers/sdd/p4-diff.ps1` | 分区像素差分 + 输出差异掩码 PNG |
| `.superpowers/sdd/p4-diff-pixels.ps1` | 列出差异像素的坐标与两侧颜色（判定字形/布局位移） |
| `.superpowers/sdd/p4-mask-box.ps1` | 差异掩码的包围盒 |
| `.superpowers/sdd/p4-crop-compare.ps1` | 同一矩形从 A/B 各裁一块上下拼接放大 |
| `.superpowers/sdd/p4-build-run.ps1` | 唯一可靠的 `tauri:build` 后台形态（注入 vcvars + rustup 1.90.0） |
| `.superpowers/sdd/p4-install-run.ps1` | 静默装机 + 按装机副本尺寸/mtime 判定是否真的换掉 |
