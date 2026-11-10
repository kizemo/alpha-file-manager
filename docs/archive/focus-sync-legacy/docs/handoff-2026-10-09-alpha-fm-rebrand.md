# HANDOFF —— Alpha File Manager 品牌化与双仓重构(2026-10-09 下午)

**交接时间**:2026-10-09 14:40
**上一份**:[`handoff-2026-10-09-packaging-closeout.md`](handoff-2026-10-09-packaging-closeout.md)(上午的打包收尾)
**本轮性质**:只做了**规划 + 3 个已完成的低风险动作**,打包链路本身一行未动
**状态**:✅ 仓库改名与配置入库完成 · ⬜ 构建/打包/装机验证**全部尚未开始**

---

## 0. 三十秒速览

| 项 | 值 |
|---|---|
| 产品名 | **Alpha File Manager**(代码里的 `productName` **尚未改**,见 P4) |
| 品牌仓(fork) | `F:\soft\00selfmade\filemanager\sigma-file-manager`,HEAD **`9e764a16`**,**已推送,与 origin 一致** |
| 品牌仓远程 | **`https://github.com/kizemo/alpha-file-manager.git`**(今天从 `sigma-file-manager` 改名而来) |
| sync 仓(主仓) | `F:\soft\00selfmade\filemanager`,HEAD **`2109d59`**,领先 origin **6 个提交,未推送** |
| 两仓工作区 | **被跟踪文件零改动**(fork 仅剩未跟踪的 `release/`) |
| 权威计划 | [`PLAN-2026-10-09-alpha-file-manager-v3.md`](PLAN-2026-10-09-alpha-file-manager-v3.md) |
| 装机现状 | **未重新构建、未安装**。跑着的还是上午那个已验证的安装包 |

---

## 1. 本轮最重要的一条:别跳过 P0.5

> **删除任何 `.exe` 之前,必须先跑一次真实构建并验证载荷 sha。**

主仓 `release/` 下有 **6 个整合包(16.7 MB×6)+ 2 个上游原版包** 待删,
它们**都未被 git 跟踪** —— 删了 `git clean` 也救不回来,只能重新构建。

上午的 handoff §5-7 里「卸载从未端到端验证」「H2 假成功」等旧问题依然有效,
但**本轮最高优先级是先把构建基线钉住**,否则后面所有删改都没有安全网。

---

## 2. 必读文档(按顺序)

| # | 文档 | 为什么读 |
|---|---|---|
| 1 | **本文** | 当前状态、约束、下一步 |
| 2 | [`PLAN-2026-10-09-alpha-file-manager-v3.md`](PLAN-2026-10-09-alpha-file-manager-v3.md) | **权威计划**。S0→P7 全流程、6 项锁定决策、门禁清单 |
| 3 | [`review-2026-10-09-plan-alpha-file-manager.md`](review-2026-10-09-plan-alpha-file-manager.md) | Claude 对 v2 的评审;v3 采纳了什么、**纠正了什么** |
| 4 | [`handoff-2026-10-09-packaging-closeout.md`](handoff-2026-10-09-packaging-closeout.md) | 上午的打包收尾。**§1 的 sha 验收标准、§3 的 12 条硬约束仍然全部有效** |
| 5 | [`packaging-findings-2026-10-08.md`](packaging-findings-2026-10-08.md) | 5 个历史根因的完整证据链 |
| 6 | [`../README.md`](../README.md) | sync 插件的架构、部署契约、冷启动预热说明 |

> **不要**读 `docs/superpowers/plans/` 里 100+ 篇历史文档。它们是失败记录,不是参考。

---

## 3. 硬约束(违反即任务失败)

上午 handoff §3 的 12 条**全部继续有效**,此处只列本轮**新增**的:

| # | 约束 |
|---|---|
| 13 | **`feat/dialog-focus-sync` 的上游追踪指向 `upstream/main`** —— 在这个分支上跑 `git push` 会把代码推到**上游仓库**。永远不要在那上面 push |
| 14 | **编 Sigma FM 必须 Rust 1.90**(硬约束 12 的延续)。主仓构建 sidecar 才用 1.85 |
| 15 | **凡是引用 fork 仓以外路径的文档,一律用绝对 URL。** `../xxx` 形式的相对链接在本机能打开(两仓是邻居目录),但 fork 单独 clone 后在 GitHub 上全部 404 |
| 16 | **产品名 ≠ `productName` 字段 ≠ `identifier` ≠ 仓库名**,四者独立。改名只改显示层,`identifier` 恒为 `com.sigma-file-manager.app` |
| 17 | **禁止全局替换 `Sigma File Manager`。** 有 2 个互操作常量碰不得(见 §4.4),而且 `verify-deploy-sha.ps1` 对它们的误改**照样 PASS** —— 典型的静默失效 |
| 18 | **推送前先 `git status` 看清暂存区**,不要 `git add -A`:fork 的 `release/` 里有 15 MB 未跟踪的安装包 |

---

## 4. 本轮做完了什么

### 4.1 S0 —— fork 打包配置入库(解决上午 §5-1)

```
bda28070  chore(packaging): fork 打包配置入库,关闭「有钩子但没接线」
          6 files changed, 1104 insertions(+), 1104 deletions(-)
```

用**带对照探针**的方式确认问题已关闭(同一探针跑改前/改后):

| 探针 | before | after |
|---|---|---|
| `installerHooks` | no | **YES** → `installer/hooks.nsh` |
| `bundle.resources` | no | **YES** → 8 条 |
| `installMode` | YES | YES(对照:证明探针能识别「存在」) |

这 6 个文件是 P3/P4/P5 的共同前提 —— 它们三个阶段都改 `tauri.conf.json`,
不入库就等于**改坏了无法 diff、无法回滚**。

### 4.2 仓库改名

```
kizemo/sigma-file-manager  →  kizemo/alpha-file-manager
```

改名后逐项验证:fork 关系保留(`parent` 仍是上游)、release 完好、
旧 URL 自动重定向、上游仓库未受影响、本地 `origin` 已更新并 fetch 验证通过。

### 4.3 README 品牌化(顺带修了 3 个实质问题)

| # | 问题 | 处理 |
|---|---|---|
| 1 | **4 条指向仓外的相对链接是死链** —— 本机能开只因两仓是邻居,fork 单独 clone 后 GitHub 上 404 | 改掉,不再引用仓外路径 |
| 2 | **Focus Sync 段描述的是已废弃架构** —— 原文教用户手动启动 `spike.exe`,那是 v0.3.0 之前的 spawn 模式 | 按当前实际行为重写(计划任务自动启动) |
| 3 | **缺 GPL-3 §6 同源代码义务** —— 原 README 完全没有源码获取方式 | 补三组件对照表,均已验证可达 |

另:标题区原先挂着上游的 Sigma logo —— 品牌化后继续挂它构成商标与背书暗示,**已移除**。

### 4.4 ⚠️ 两个碰不得的互操作常量(改名时)

| 文件:行 | 常量 | 碰了会怎样 |
|---|---|---|
| `src/lan_share/types.rs:15` | `MDNS_INSTANCE_NAME` | LAN 共享在别的设备上搜不到本机 |
| `src/windows_installation.rs:30` | `LEGACY_DEFAULT_FILE_MANAGER_DATA_DIR_NAME` | 破坏接管 Windows 默认文件管理器 |

### 4.5 跨仓路径事实(已由构建产物证实)

`bundle.resources` 的 8 条源路径以 `<fork>/src-tauri/` 为基准,**全部指向主仓**。
证据是上次构建生成的 `target/release/nsis/x64/installer.nsi` 里的 `File` 指令 ——
**不是路径推算,是构建产物本身**。P3 就是为消除这个依赖。

---

## 5. ⬜ 下一步:还没开始的全部工作

| # | 阶段 | 内容 | 阻塞? |
|---|---|---|---|
| **P0.5** | **构建基线验证** | Rust 1.90 跑 `npm run tauri build`,产物存 `C:\Temp\afm-p1-baseline-2026-10-09\`,**解包比对载荷 sha** | ⬅ **从这里开始** |
| P1 | 主仓瘦身 | 删 6 整合包 + 2 上游包 + 0.2.0 旧包 + canonical.exe;0.5.8 包入库;修 `.gitignore` | **依赖 P0.5** |
| P2 | fork 产品分支 | 开 `product/alpha-fm`;`.gitignore` 加 `release/` 与 `afm-staging/` | — |
| P2.5 | 树形视图代码归位 | 10 个自有文件迁入 `src/modules/navigator/tree/`;生成 `integration-points.yaml`。**只搬不改逻辑** | — |
| P3 | sync 载荷 stage 进 fork | dry-run → 建 `afm-staging/` → 改 `bundle.resources` → 扩展 `build-with-sidecar.ps1` | — |
| P4 | 改名 | 按 §4.4 逐文件改 12 处显示位,**禁止全局替换** | — |
| P5 | 图标落地 | 候选 A → 裁 1024×1024 → `npx tauri icon`(含前后两次 `git status` 快照) | — |
| P6 | 门禁与文档 | 4 个新门禁脚本 + GPL 声明 | — |
| P7 | 端到端重验 | 构建→安装→sha 门禁→**卸载**(顺带结掉旧 §5-4) | — |

### 5.1 待办杂项

- **主仓 6 个提交未推送**(按你一贯做法);要推请明示
- 图标候选在 `docs/brand/icon-candidates/afm-v3-2-solid-teal.jpg`(2048×2048,P5 需裁到 1024)
- README 的「Getting it」章节在**出首个新安装包后**需要更新
- 旧的 handoff §5-3(H2 假成功)、§5-5(激活耗时)未处理,优先级低于上面 8 项

---

## 6. 回退节点

| 位置 | 节点 | 说明 |
|---|---|---|
| **fork** | `9e764a16` | ★ 当前状态,**已全部推送到 GitHub** |
| fork | `bda28070` | S0 之前(配置未入库的状态) |
| fork | `4365281b` | 本轮开工前 |
| **主仓** | `2109d59` | ★ 当前状态(**未推送**) |
| 主仓 | `dc5f152` | 本轮开工前 |
| 主仓 tag | `v0.5.9-packaging-20261009` | 上午的打包收尾基线,**仍然有效** |

⚠️ **不要**回退到 `v0.5.8-packaging-baseline-20261008`(那个节点的安装包不可用)。
⚠️ fork 的 `git reset` 后需 `git push --force` 才能让 GitHub 同步;动手前先确认没有别人在用。

---

## 7. 运维命令

```powershell
# 构建(必须 Rust 1.90)
cd sigma-file-manager
$env:PATH = "C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;$env:PATH"
npm run tauri build

# 构建 sidecar(必须 Rust 1.85,别搞混)
$env:PATH = "C:\Users\Duanyi\.rustup\toolchains\1.85.0-x86_64-pc-windows-msvc\bin;$env:PATH"
cd sigma-listary-spike; cargo build --release; cargo test --release

# 构建 + 部署 sidecar 到两个打包输入目录(含沙箱门禁)
.\sigma-file-manager\scripts\build-with-sidecar.ps1 -RepoRoot $PWD

# NSIS 警告门禁(Tauri 会吞掉 makensis 的警告,必须绕开它)
powershell -NoProfile -ExecutionPolicy Bypass -File sigma-file-manager\scripts\verify-nsis-warnings.ps1

# 沙箱门禁
node scripts\scan-sandbox-dynamic.cjs release\extension\dist\index.js

# 装机 sha 验收(退出码 0 才算通过)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-deploy-sha.ps1

# 读日志(必须 UTF-8 共享读)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\read-sidecar-log.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\read-focus-trace.ps1
```

---

## 8. 给新会话的一句话 prompt

```
读 F:\soft\00selfmade\filemanager\docs\handoff-2026-10-09-alpha-fm-rebrand.md(必读),
再按它的 §2 顺序读完 1-6 号。当前状态已固化:fork 仓已改名 alpha-file-manager 并推送至
9e764a16,主仓 2109d59 未推送,两仓工作区干净。计划在 §2-2 的 v3 文档里,决策已全部锁定,
不要重新讨论名字/图标/方案。直接执行 §5 的 P0.5 构建基线验证 —— 在它通过之前不要删任何
.exe,那是 P1 的硬前置。动代码前务必读 §3 硬约束(注意第 13、15、17 条),并遵守 §9。
```

---

## 9. 本轮经验教训

1. **「本地能打开」不等于「线上能打开」。** README 里 4 条 `../xxx` 相对链接在本机全部
   正常,因为两仓恰好是邻居目录。**它们在 GitHub 上全是 404。** 判断一个链接是否有效,
   必须假设「只有这一个仓」。

2. **文档描述的架构可能早就废弃了。** README 教用户手动启动 `spike.exe`,
   而实际 v0.3.0 就改成计划任务自动启动了 —— **隔了一个大版本还在文档里躺着**。
   写文案前先查代码实际怎么跑,别信上一任自己写的文档。

3. **我自己的查询工具骗了我一次。** 验证三个 GitHub 仓库是否存在时,我写的 jq 表达式
   有语法问题,**导致三个全报「不存在」**,其中包括我几分钟前刚验证过的上游仓库。
   是自己发现前后矛盾才重测的。
   **已知为真的东西报假 ⇒ 尺子坏了,不是东西坏了。** 这是本项目的老毛病,又犯了一次。

4. **文档里没有的东西,通常是漏了不是不需要。** GPL-3 §6 要求分发二进制时提供源码,
   而原 README 一个字都没提 —— 这类义务不会因为「没写」就消失。

5. **改名时最容易出事的是全局替换。** `Sigma File Manager` 出现在 15 个文件里,
   其中 2 处是互操作常量。顺手替换掉**不会报错**,sha 门禁**照样 PASS**,
   后果是 LAN 共享与默认文件管理器静默失效。

6. **依赖顺序要提前想。** 本轮初版计划把「配置入库」排在后面,被 Claude 的评审点出:
   P3/P4/P5 三个阶段全都改同一个未入库的文件,不改就等于没有回滚点。
   **计划里每个阶段要改什么文件,先确认那些文件在不在版本控制里。**