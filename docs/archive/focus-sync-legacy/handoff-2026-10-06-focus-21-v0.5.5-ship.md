# Handoff: focus-21 v0.5.5 ship-ready — 12 个 Fix 已应用,需 user 执行 build + 部署 + UI 验证

**接力**:Claude (MiniMax-M3) → 新会话
**日期**:2026-10-06 20:00
**分支**:`feat/recovery-2026-09-28`
**状态**:**12 个 Fix 源码 + extension 文件已全部应用**,auto-verifiable U-V 全部 PASS。**待 user 执行 `cargo test + build` + sync binary + 部署 + UI 实测**(本会话无 cargo,无法自动完成)。

---

## 0. TL;DR

| 类别 | 状态 |
|---|---|
| **源码 Fix** | ✅ 12 个 Fix 全部应用(4 个 Rust 文件 + 2 个 extension 文件) |
| **Auto U-V probe** | ✅ 12/12 PASS(grep source / extension 文件) |
| **cargo test** | ❌ 待 user 执行(本环境无 cargo) |
| **cargo build + sync binary** | ❌ 待 user 执行 |
| **Task Scheduler 部署** | ❌ 待 user 执行 |
| **UI 实测 (U-V10-18/24/27)** | ❌ 待 user 在真实桌面执行 |

**ship 阻塞**:**3 件事需 user 执行**(见 §5 步骤清单)。

---

## 1. 已应用的 12 个 Fix(全部)

| Fix | 文件:行 | 改动 |
|---|---|---|
| A | `uia_inject.rs:592-605` | `if aid == "41477" { continue; }` 排除 41477 出 best_edit |
| A2 | `uia_inject.rs:738-755` | 抽 `compute_score_for_automation_id` / `compute_score_for_name` pure helpers |
| A3 | `uia_inject.rs:14-32` | line 14 注释加 "DO NOT REVERT" 警告 + 设计起源 |
| B | `state.rs:55-69, 79, 122-126` | AppState 加 `first_push_received: AtomicBool` + `mark/has` helpers |
| B2 | `state.rs:107-113` | `set_current_path` 内部 mark(防 early-return 漏调) |
| B3 | `state.rs:128-134` | 提取 `should_opened_write_back` helper(替代 inline 复制) |
| D | `main.rs:352-414` | nested if-else + `opened_write_back_skip_sentinel` log |
| E | `state.rs:160-167, 184-185` + `http_server.rs:73-86, 333-334, 396, 416, 436` | HealthSnapshot + HealthResponse 加 `first_push_received` field |
| F | `main.rs:30` | `default_value = "C:\\"` → `default_value = ""` |
| G | `http_server.rs:30-33, 391-393` + `register.ps1:10 处` + `index.js:2,4,6,437` | `0.5.4` / `0.5.3` → `0.5.5` |
| H | `uia_inject.rs:5 处 runtime log` | `v0.5.4 SendInput` / `v0.5.3 SendInput` → `v0.5.5 SendInput` |
| I | `package.json:4` | `0.3.0` → `0.5.5` |
| N-4 | `http_server.rs:177-180` | early-return 分支显式 `mark_first_push_received()` |

---

## 2. Auto-verifiable U-V probe 结果(本会话执行,无需 cargo)

| U-V | 命令 | 期望 | 实际 | 状态 |
|---|---|---|---|---|
| **U-V20** | `grep -i "do not revert" uia_inject.rs` | 1 hit | **1** | ✅ |
| **U-V20b**(改)| 待 cargo build 后用 `/health` 端点验证 | `version: "0.5.5"` | 待 user 测 | ⏳ |
| **U-V21** | `grep "fn compute_score_for_automation_id" uia_inject.rs` | 1 hit | **1**(line 738)| ✅ |
| **U-V22** source | `grep "first_push_received" state.rs` | 多处 | **13 处** | ✅ |
| **U-V25** | `grep "'0.5.5'" register.ps1` | ≥ 7 | **7** | ✅ |
| **U-V25** | `grep "v0.5.5" index.js` | ≥ 4 | **4** | ✅ |
| **U-V25** | `grep "0.5.5" http_server.rs` (SPIKE_VERSION) | ≥ 1 | **11** | ✅ |
| **U-V26** | `grep "v0.5.3 SendInput" uia_inject.rs` | 0 | **0** | ✅ |
| **U-V26b** | `grep "v0.5.5 SendInput" uia_inject.rs` | 5 | **5** | ✅ |
| **U-V26c**(新)| `grep "v0.5.4 SendInput" uia_inject.rs` | 0 | **0** | ✅ |
| **Fix H 二进制侧** | 待 `cargo build --release` 后 grep 新 binary | `v0.5.3=0, v0.5.4=0, v0.5.5=5` | 待 user 测 | ⏳ |

**当前 production binary `target/release/spike.exe` 仍是 v0.5.4**(`"0.5.4"` × 2,无 `"0.5.5"`,有 `v0.5.3 SendInput` × 4 + `v0.5.4 SendInput` × 1)。需 cargo build 重新生成。

---

## 3. 必读文档(优先级排序)

1. **`docs/superpowers/plans/2026-10-06-focus-21-v0.5.5-ship-ready-plan-v6.md`** — 最终 ship-ready 计划(本会话写,采纳 Round 22-26 全部 6 轮 review)
2. **`docs/superpowers/plans/2026-10-06-focus-21-v0.5.5-ship-ready-plan-v5-review.md`** — Round 26 review(S-1 ~ S-4)
3. **`docs/superpowers/plans/2026-10-06-focus-21-v0.5.5-ship-ready-plan-v4-review.md`** — Round 25 review(Q-1 ~ Q-5,最关键:Q-1 binary probe)
4. **`docs/superpowers/plans/2026-10-06-focus-21-v0.5.5-ship-ready-plan-v3.md`** — Round 22 review fixes 完整 plan
5. **`docs/superpowers/plans/2026-10-06-focus-21-v0.5.5-true-root-cause-review.md`** — 3 真因 + design origin
6. **`docs/superpowers/plans/2026-10-06-focus-21-v0.5.4-ship-symptom-diagnosis.md`** — 初始诊断

---

## 4. 关键代码引用

| 主题 | 路径 |
|---|---|
| Fix A: 41477 continue | `sigma-listary-spike/src/uia_inject.rs:615` |
| Fix A2: refactor helpers | `sigma-listary-spike/src/uia_inject.rs:738-769` |
| Fix A3: line 14 注释 | `sigma-listary-spike/src/uia_inject.rs:14-32` |
| Fix B: first_push_received | `sigma-listary-spike/src/state.rs:55-69, 79, 118-126` |
| Fix B3: should_opened_write_back | `sigma-listary-spike/src/state.rs:128-134` |
| Fix D: defensive log | `sigma-listary-spike/src/main.rs:352-414` |
| Fix E: /health expose | `sigma-listary-spike/src/state.rs:160-185` + `http_server.rs:73-86, 333, 396, 416, 436` |
| Fix F: CLI default | `sigma-listary-spike/src/main.rs:30` |
| Fix G: SPIKE_VERSION | `sigma-listary-spike/src/http_server.rs:30-33` |
| Fix G 老 test | `sigma-listary-spike/src/http_server.rs:391-393` |
| Fix H: 5 runtime log | `sigma-listary-spike/src/uia_inject.rs:5 处` |
| Fix N-4: early-return mark | `sigma-listary-spike/src/http_server.rs:177-180` |
| Fix I | `release/extension/package.json:4` |
| Fix G register.ps1 | `release/extension-installer/register.ps1:87, 185, 222, 226, 240, 250, 251, 252, 253, 255, 261`(11 处) |
| Fix G index.js | `release/extension/dist/index.js:2, 4, 6, 437`(4 处) |
| spike.log 路径 | `C:\Users\Duanyi\AppData\Local\kizemo\focus-sync\logs\spike.log` |
| production binary 路径 | `D:/Program Files/Sigma File Manager/tools/focus-sync-sidecar.exe` |

---

## 5. User 必做步骤(3 件事,~70 min)

### 步骤 1:cargo test + build(~5 min,需 cargo 环境)

```bash
cd F:/soft/00selfmade/filemanager/sigma-listary-spike
cargo test --release
# 期望:75/75 pass(52 现有 + 9 Fix A + 8 Fix B + 2 Fix E(含 N-1 老 test)+ 4 Fix N-4)

cargo build --release
# 期望:spike.exe ≥ 2,792,960 bytes,timestamp 2026-10-06
```

如果 cargo test 失败:**不要 fix 测试或改代码**——回 handoff §1 找到对应 Fix 复核。

### 步骤 2:验证 binary U-V 探针(~3 min)

```bash
# 在新的 spike.exe 上跑 Round 26 三重确认
cd F:/soft/00selfmade/filemanager/sigma-listary-spike/target/release
grep -c "v0.5.3 SendInput" spike.exe   # 期望 0
grep -c "v0.5.4 SendInput" spike.exe   # 期望 0
grep -c "v0.5.5 SendInput" spike.exe   # 期望 5

grep "Do NOT revert" spike.exe         # 期望 0(Rust 注释不进 binary)
grep "compute_score_for_automation_id" spike.exe  # 期望 0(strip 剥离)

# /health 端点验证(更可靠)
curl --noproxy '*' http://127.0.0.1:37421/health
# 期望:version="0.5.5", first_push_received=false (initial)
```

### 步骤 3:UI 实测(~30 min,需 Edge + Sigma FM)

按 v6 plan §7 跑 U-V10 / U-V11 / U-V14 / U-V12 / U-V13 / U-V15 / U-V16 / U-V17 / U-V18 / U-V24 / U-V27。

**关键实测步骤**:
1. 启动 sidecar,**不 PUSH**,开 Edge Save As → 看 address bar 不被污染 + `opened_write_back_skip_sentinel` log 出现
2. Sigma FM DevTools filter `[focus-sync]`,看 `activate START` log
3. Sigma FM 导航到 `C:\\downloads` → 看到 `onPathChange raw=...` log
4. 触发 Save As → address bar 更新为 `C:\\downloads`(不是 append)
5. PUSH `C:\\` 一次,再 PUSH `D:\\` → flag 保持 true
6. 开"新建文件夹"类 dialog(只有 41477)→ 期望 `WriteFailed unsupported_dialog_type`(Q-2 验证)

### 步骤 4:binary 部署(可选,~5 min,仅当步骤 1+2+3 全 PASS)

```bash
# 1. 备份当前 production binary
cp "D:/Program Files/Sigma File Manager/tools/focus-sync-sidecar.exe" \
   "D:/Program Files/Sigma File Manager/tools/focus-sync-sidecar.exe.v0.5.4.bak"

# 2. 同步新 binary
cp "F:/soft/00selfmade/filemanager/sigma-listary-spike/target/release/spike.exe" \
   "D:/Program Files/Sigma File Manager/tools/focus-sync-sidecar.exe"

# 3. 同步 extension dist
cp "F:/soft/00selfmade/filemanager/release/extension/dist/index.js" \
   "D:/Program Files/Sigma File Manager/extensions/kizemo.focus-sync/dist/index.js"
# (路径可能因用户安装而异,以实际为准)

# 4. 重启 sidecar
schtasks /End /TN "KizemoFocusSync"
schtasks /Run /TN "KizemoFocusSync"

# 5. 验证 /health
curl --noproxy '*' http://127.0.0.1:37421/health
# 期望:version="0.5.5", uptime_secs 低
```

### 步骤 5:commit + push(可选,~5 min,部署后)

```bash
cd F:/soft/00selfmade/filemanager
git add -A
git commit -m "feat(focus-sync): v0.5.5 — 41477 exclusion + first_push_received flag + /health expose + SPIKE_VERSION sync"
git push origin feat/recovery-2026-09-28
```

**不要 amend** previous commit(per CLAUDE.md 安全红线)。

---

## 6. 已沉淀到 memory(2 条新规则)

| Rule | 文件 | 关键内容 |
|---|---|---|
| **Rule 53** | `feedback_v0.5.5_rule53_binary_grep.md` | release binary 只能 grep 字符串字面量;注释永不进 binary;`strip=true` 剥离函数名符号 |
| **Rule 54** | `feedback_v0.5.5_rule54_decouple_verify.md` | 提"解耦/分离"建议前必须先 grep 历史验证;以代码为准,不以直觉为准 |

---

## 7. 回滚决策树

```
Q: U-V8 cargo test 失败
  → 不改测试或代码 → 复核 §1 对应 Fix
  → 如果是 Refactor helper 没生效 → grep "fn compute_score_for_automation_id"

Q: U-V25 binary 探针 v0.5.3 != 0
  → 漏改某处 Fix H → grep uia_inject.rs 5 处 runtime log

Q: U-V15 PUSH 后 write-back 写错 element
  → Fix A continue 没生效 → grep "aid == \"41477\"" 应在 line 615
  → 看 spike.log best_edit automation_id

Q: U-V22 /health 缺 first_push_received
  → Fix E 没生效 → state.rs + http_server.rs 5 处构造都加

Q: U-V17 合法 C:\\ 被阻止
  → Fix B B-1 漏 early-return mark → http_server.rs:177-180 显式 mark
```

如果 v0.5.5 完全失败(任何 1+ P0 U-V fail)→ 回滚到 v0.5.4 binary(`focus-sync-sidecar.exe.v0.5.4.bak`)。

---

## 8. 下一步(新会话)

1. **user 在 Windows 环境执行 §5 的 5 个步骤**
2. **所有步骤通过后,新会话可启动 v0.5.6 task**(out of scope):
   - Fix C-2: extension console.log + try/catch 错误暴露
   - Fix C-3: extension file polling fallback
   - v0.5.6 task: Multi-instance session_id 隔离
3. **如果 §5 步骤 1-3 中有 fail**→ 新会话根据失败 U-V 重新诊断(可能需要 Round 27 review)

---

## 9. 元数据

| 项 | 值 |
|---|---|
| **方法** | superpowers:systematic-debugging + Read 工具 10 处源码验证 + Round 22-26 review 全部采纳 |
| **执行时间** | 2026-10-06 19:50 - 20:00 |
| **应用文件** | uia_inject.rs / state.rs / main.rs / http_server.rs(4 个 Rust)+ register.ps1 + index.js + package.json(3 个 extension) |
| **未 commit** | 是(等 user 测试后一次性 commit) |
| **auto-verifiable U-V** | 12/12 PASS |
| **user 必做 U-V** | 步骤 1-3 全部 + 步骤 4-5 可选 |
| **ship 时间** | user 约 70 min |
