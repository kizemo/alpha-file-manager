# 全盘搜索增量索引（批次 4 / 任务 F）设计

- 撰写时间：2026-11-04
- 对应计划：`docs/plan-2026-11-02-action-registry-and-incremental-index.md` §3（批次 4）
- 对应收益：`docs/benefit-2026-11-03-retrieval-incremental-index.md`
- 上游交接：`docs/handoff-2026-11-03-dir-load-hang-fix.md` §8.3
- 状态：**设计已定稿**，实现按 §7 的顺序落码

---

## 一、要解决的问题

现状：全局搜索的索引**每轮全量重建**。单轮约 191 万文档 / 12.2–12.5 min（四轮基线 728,966 / 748,192 / 730,123 / 750,079 ms）。前端 `autoScanPeriodMinutes` 默认 60 ⇒ 每小时一次十几分钟的全量重建。

目标：**首次全量，之后 watcher 驱动增量**；任何前提不满足就回落全量，功能不许退化。

---

## 二、现状事实（2026-11-04 逐条核实）

| 事实 | 证据 |
|---|---|
| 全量入口唯一 | `global_search/scan.rs` `global_search_start_scan()`：staging 建索引 → `replace_index_dir` → 重开 live |
| 已有「写活索引」范式 | `global_search/scan.rs` `global_search_index_paths_blocking()`：`delete_term` → `add_path_doc` → `commit` → `merge_index_segments` → `reader.reload()` |
| **该范式的子树删除是无效的** | tantivy 0.26 的 `IndexWriter::delete_term` 是**精确 term**；`path` 字段是 `STRING`（raw tokenizer）⇒ 每个完整路径是一个 term ⇒ `delete_term("C:\foo\")` 只能删掉路径**恰好等于** `C:\foo\` 的文档，子树文档（`C:\foo\bar.txt`）删不掉 |
| tantivy 0.26 没有前缀删除 API | `tantivy-0.26.2/src/indexer/index_writer.rs` 只有 `delete_term` / `delete_query` / `delete_all_documents`；**没有** `delete_terminator` / `delete_prefix` |
| 前缀删除的正解 | `tantivy::query::RegexQuery::from_pattern(pattern, field)` + `IndexWriter::delete_query(Box<dyn Query>)` |
| 现成 watcher 不能复用 | `dir_watcher.rs`：`RecursiveMode::NonRecursive`、只发 `dir-change` 给前端、强耦合 `AppHandle` / `Emitter` |
| 依赖已就位 | `Cargo.toml`：`notify = "8"`、`walkdir = "2.5"`、`tantivy = "0.26"` |
| meta 已有盘根集合 | `GlobalSearchMeta.indexed_drive_roots`（`#[serde(default)]`）⇒ 可判定「盘符集合是否一致」 |
| `cargo test --lib` 红线 | 被测链接到的代码不得触碰 `tauri::AppHandle` / `tauri::async_runtime::*`，否则加载期 `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)` |

### 需要修正的计划假设

`plan-2026-11-02` §3 与 `benefit-2026-11-03` §二 写「**复用**已有 watcher `dir_watcher.rs`，把它接到 tantivy writer 上」。

该文件是 **NonRecursive**、只服务文件浏览器刷新、且强耦合 `AppHandle` —— **不能**直接接到索引上。
本设计改为**新增面向索引的递归 watcher**：与 `dir_watcher.rs` 共用 `notify` 依赖与去抖参数，但代码分开。
「一套事件源两处复用」这条收益**不成立**，写进结论时要改口。

---

## 三、架构

### 3.1 模块划分

| 模块 | 职责 | 碰 tauri？ |
|---|---|---|
| `global_search/incremental.rs`（新） | 变更计划：事件 → `ChangeSet` → 索引操作序列；纯逻辑 | **否** ⇒ 可进 `cargo test --lib` |
| `global_search/index_watcher.rs`（新） | 面向索引的递归 watcher：每根一个通道、去抖、周期 flush | 只用 `notify` + 通道，**不碰 AppHandle** |
| `global_search/scan.rs`（改） | 拆分耗时埋点；新增 `global_search_apply_incremental_blocking()` | 沿用既有 `*_blocking` 命名 |
| `global_search/index.rs`（改） | `apply_prefix_delete()`（RegexQuery）；`GlobalSearchMeta` 增字段 | 否 |
| `global_search/state.rs`（改） | watcher 健康状态 | 否 |

### 3.2 数据流

```
notify 事件 ──去抖 300ms──▶ ChangeSet（按路径去重，保留最新事件）
                              │
                              ├── RemoveTree(path) ── RegexQuery 删子树 ──┐
                              ├── Remove(path)    ── delete_term 精确删 ──┤
                              └── Upsert(path)    ── 重新枚举该路径子树 ──┤
                                                                        ▼
                                     活索引 writer：commit → merge → reader.reload()
```

### 3.3 变更计划（`incremental.rs`）

```rust
pub enum ChangeKind { Upsert, Remove, RemoveTree }
pub struct Change { pub path: String, pub kind: ChangeKind }
```

- **去重**：同一路径只保留最后一个事件。
- **父子折叠**：`RemoveTree(dir)` 吸收 `dir` 之下的 `Upsert`（目录没了，里面不用再写）。
- **忽略路径**：`IgnoredPathMatcher` 在**规划阶段**就滤掉被忽略的路径，避免把 `$Recycle.Bin` 之类写进索引。
- **Upsert 一个目录** ⇒ 枚举该目录子树并逐个 `Upsert`（复用 `scan_drive` 的遍历逻辑）。

### 3.4 一致性前提（任一不满足 ⇒ 回落全量）

1. `meta.schema_version == SCHEMA_VERSION`
2. `meta.indexed_drive_roots` 与当前设置的盘根集合**完全一致**
3. 活索引目录存在且 `open_or_create_index` 成功
4. 上一次全量扫描成功（`last_scan_outcome == Success`）
5. 每个盘根的 watcher 处于「已建立且未报错」
6. 上一次增量应用成功（没有半途失败）

### 3.5 竞态处理

- **watch 先建、扫描后跑**：全量扫描**之前**就建立 watcher ⇒ 扫描期间的变化进队列；扫描完成后 flush（重复处理是幂等覆盖，结果一致）。
- **幂等**：同一路径重复 `Upsert` ⇒ 先删后加 ⇒ 结果一致。
- **删除 / 新增同路径**：以最后一个事件为准。

### 3.6 降级触发

| 触发 | 行为 |
|---|---|
| watcher 建立失败（盘不存在 / 权限不足） | 该根标记不可用 ⇒ 回落全量 |
| 增量应用失败（tantivy 错误） | 标记索引「需重建」⇒ 下一轮走全量 |
| 用户手动点「重建索引」（`ScanReason::Manual`） | 永远走全量 |
| `meta` 缺失 / 版本不符 / 盘根集合变了 | 回落全量 |

---

## 四、耗时拆分埋点（回答「12 min 花在哪」）

`scan_drive` 现有的 `scan root timings`（`scan.rs:346`）只有**总耗时**。
本轮拆成两项分别累计：

- `enumerate_ms`：遍历目录 + 读 metadata
- `index_write_ms`：`add_path_doc`

两项之和应当 ≈ 总耗时。有了它才能判断大头在枚举还是在写索引 —— **验收报告必须给出这个拆分**。

---

## 五、验收口径（来自 `benefit-2026-11-03` §6.6，三条必须**同时**满足）

1. 首轮 ≤ 6.3 min（四盘并行确实生效）
2. 稳态（增量）轮 **显著低于** 12.2 min
3. **增量更新后查询结果与全量重建一致** —— 同一批已知路径做交叉验证

第 3 条是关键：增量方案最大的风险是「漏事件 ⇒ 索引里留下脏数据 / 缺失条目」。

- **四轮基线必须用同一条探针重跑**：`.superpowers/sdd/task3-search-accept-20261102-v2.ps1`，埋点 `src-tauri/src/global_search/scan.rs:346`。
- **验收计时期间不跑任何编译类命令**（抢 CPU 会让读数虚高）。
- 真机验收必须用 `tauri build` 的打包产物；`tauri dev` 的界面结论一律作废。

---

## 六、风险与对策

| 风险 | 对策 |
|---|---|
| 整盘递归 watch 在 Windows 上可能漏事件 | 全量兜底 + 「增量结果 = 全量结果」交叉验证把住 |
| `RegexQuery` 遍历整个 path term 字典（191 万） | 只在**目录删除**时用；文件删除走精确 `delete_term` |
| `delete_query` 与 `add_document` 同批次 | tantivy 语义正确：先应用删除 opstamp，再加文档 |
| `cargo test --lib` 链接红线 | 新模块一律不碰 `tauri::AppHandle` / `tauri::async_runtime::*`；对外只暴露 `*_blocking` 同步入口 |
| 增量 flush 与全量扫描并发写索引 | 二者互斥（同一把写锁） |
| `delete_term` 子树删除失效（既有缺陷） | 本批次顺带修掉：`global_search_index_paths_blocking` 改用 `apply_prefix_delete` |

---

## 七、落地顺序

1. `incremental.rs` 纯逻辑 + 单测（RED → GREEN）
2. `index.rs` 的 `apply_prefix_delete` + 单测（**顺带修掉既有子树删除缺陷**）
3. `scan.rs` 拆分耗时埋点
4. `index_watcher.rs` 递归 watcher
5. 前端分流（`src/stores/runtime/global-search.ts`）
6. 门禁：`cargo test --lib` / vitest / vue-tsc / eslint 基线
7. release 打包 + 真机 4 轮扫描验收
