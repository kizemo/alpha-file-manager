# Plan Review — Alpha File Manager 双仓重构 v2 优化建议

**日期**:2026-10-09 11:35
**审阅对象**:`docs/PLAN-2026-10-09-alpha-file-manager.md`(v2 版)
**取代**:`docs/plan-2026-10-09-dual-repo-restructure.md`(v1 版,已存档)
**基线**:主仓 `dc5f152` / fork `4365281b`(均未推送)
**本轮性质**:纯书面反馈,未修改任何被跟踪文件,未提交、未切分支
**接收方**:MiniMax(供优化 v2 计划参考)

---

## 0. 总体评价

v2 相比 v1 有本质提升:
- §1 把「树形视图不能独立」改为「可整理 + 集成点登记」,新增 P2.5 阶段
- §2.3 识别了 2 个绝对不能改的互操作常量
- §4.1 诚实地记录了 3 轮图标生成的失败过程
- §9 明确区分了"测了什么 / 没测什么"

但有 5 处可优化点,按风险排序如下。

---

## 1. 关键问题(按风险排序)

### 1.1 [关键风险] P1 删除前的"实测可重建"是硬前置,不是建议

v2 §7 P1 第 1 步的"先跑 fork 构建"—— §9 又承认没跑过任何构建。

历史教训(Rule 55–67)反复证明:**静态分析可行 ≠ 实际跑起来可行**。要删的 6+2 个 `.exe` 都是未入库文件,**删完 `git clean` 救不回来**。

**建议**:
1. v2 P0 与 P1 之间插入 **P0.5 · 构建基线验证**
2. 实际跑一次 fork 构建,产物存到 `C:\Temp\afm-p1-baseline-2026-10-09\`(用户硬性临时目录约定)
3. 把 `verify-deploy-sha.ps1` 反过来用——「基线构建产出 + 门禁 PASS」作为重建锚点

### 1.2 [重要风险] P3 stage 进 fork 的路径解析需要先 dry-run

v2 §7 P3 把 sync 载荷 stage 到 fork 仓内的 `afm-staging/`。层级从 `../../` 变成 `../`,**v2 §9 也承认没实测**。

**建议**:
1. P3 第 ① 步前加一行 dry-run:用现有的 8 条路径加 1 条 `../afm-staging/test.txt` 做构建,看 NSIS 能否解析
2. 在 `verify-deploy-sha.ps1` 加一条:比对 stage 目录里载荷的 sha 与主仓 `release/extension/...` 的 sha,不一致就不允许构建
3. "只读主仓,只写 fork" 红线(§7 P3 第④)必须落到门禁里,建议 `scripts/check-fork-readonly.ps1` 在 fork 仓构建前跑一次,校验主仓 §6.1 清单文件 sha

### 1.3 [优化建议] P2.5"集成点登记表"是亮点,但需要约定维护节奏

**建议**:
1. 存 `sigma-file-manager/docs/integration-points.md`(随项目走)
2. **每次同步 upstream 前后都重读核对**
3. 字段:文件路径 + 行号 + 改动类型(注入/调用/导入) + 关联 upstream commit(如有)
4. 用 YAML 而非自然语言表格(便于将来用脚本批量对比)

### 1.4 [建议] §4 图标落地步骤缺工具

§4.3 手动裁剪易出错。

**建议**:
1. 一次性脚本 `scripts/crop-icon.ps1`,把候选 A 裁成 1024×1024(中心 45%)
2. P5 第 1 步先 `git status` 看 `src-tauri/icons/` 起点,跑完 `tauri icon` 再 diff,避免「图标被替换了但不知道」的隐性失败

### 1.5 [建议] §3 GPL 合规少了一个关键点

§3.2 列了 5 条义务,但漏了 **GPL-3 的同源代码传播触发**:

> fork 是 GPL-3,任何把 fork 二进制分发给用户的动作,必须同时提供对应源码或源码获取方式(URL)

主仓 focus-sync 是 MIT,组合后整体按 GPL-3 分发,但 README 必须明确:
- "本软件按 GPL-3 分发,源码在此:`<url>`"
- 不写等同 Rule 65 类型的实际产品 bug(Content-Type 不带 charset 是真实产品 bug)

---

## 2. 建议的执行顺序调整

v2 原顺序:P0 → P1 → P2 → P2.5 → P3 → P4 → P5 → P6 → P7

**建议改为**:

1. **(handoff §5 第 1 项)**fork 的打包配置入库 —— 这是 v2 P1 第 1 步"先跑构建"的硬前置
2. **(v2 P0.5 新增)**fork 仓实际跑一次 `npm run tauri build`(Rust 1.90),产物存到 `C:\Temp\`
3. **(v2 P1)**主仓瘦身(必须 1 + 2 都通过才执行删除)
4. (v2 P2 → P2.5 → P3 → P4 → P5 → P6 → P7)后续不变

理由:现在 fork 仓 `tauri.conf.json` 没入库,跑构建也只会修改未跟踪文件,无法 commit,无法回滚。

---

## 3. 上下文同步:MiniMax 最近的沉淀(给后续协作)

### 3.1 项目结构

| 仓 | 路径 | 作用 | HEAD |
|---|---|---|---|
| 主仓(focus-sync) | `F:\soft\00selfmade\filemanager` | focus-sync 插件项目,独立可装 | `dc5f152` |
| fork(Sigma FM) | `…\sigma-file-manager` | 二次开发的文件管理器产品 | `4365281b` |
| fork 偏离 upstream | 28 commits / 55 文件改动 | 10 新增 / 47 修改(其中 18 个是 i18n) |

### 3.2 MiniMax 已完成的关键工作(commit 历史)

1. **打包任务(2026-10-08 → 09)**:`v0.5.9-packaging-20261009`(`35cfe8b`)
   - 5 个历史根因全修:内嵌旧插件、sidecar 路径分裂、NSIS 静默失效、入口裸检查挡闸门、manifest 版本谎报
   - 新增两道门禁:`verify-deploy-sha.ps1`(部署 sha)、`verify-nsis-warnings.ps1`(NSIS 警告)
2. **焦点同步插件 v0.5.x 反复迭代**(2026-09 → 10-06)
   - v0.5.5 Fix B + Fix D + Fix F 三 Fix 共谋,导致 opened_write_back 永不触发(Rule 55–67 沉淀)
   - **v0.5.6 修复方案**:~15 行,保留 v0.5.4 fallback
3. **树形视图**:55 个文件已合入 main,装包验证通过

### 3.3 回滚锚点(黄金基线)

- **`v0.5.9-packaging-20261009`(`35cfe8b`)** —— 现在的稳定状态
- **不要回退到 `v0.5.8-packaging-baseline-20261008`**(该节点安装包不可用,装着没功能)

### 3.4 关键硬约束(违反即任务失败)

> 来源:`docs/handoff-2026-10-09-packaging-closeout.md` §3

1. 回滚点 = `v0.5.9-packaging-20261009`(`35cfe8b`)
2. 安装后必须跑 `verify-deploy-sha.ps1`(退出码 0 才算通过)
3. 改插件后必须跑沙箱扫描:`VALID (0 violations / 19 patterns)`
4. **注释同样会被沙箱扫描**(已被坑 108 分钟)
5. **不要 commit / push git**,除非用户明确要求
6. 一次只改一处
7. 每次改动后回读源码确认落地(`String.Replace` 锚点不匹配是静默 no-op)
8. 不要在用户重启 Sigma FM 的窗口期部署(extension 启动加载需 1–3 分钟)
9. **不要用 `Get-Content` 默认编码读日志**(必须 UTF8,且共享读)
10. **写 .ps1 / .nsi 一律纯 ASCII**(已被坑 4 次)
11. `D:\Program Files\` 受保护,不能手动写入或删除
12. **编 Sigma FM 必须 Rust 1.90**(不是 1.85)

### 3.5 未完成工作(7 项,接 v2 P1 之前的现实)

1. **fork 的打包配置完全没进版本控制**(高风险)
   - `sigma-file-manager\src-tauri\tauri.conf.json`(`bundle.resources` + `installerHooks` + `installMode: perMachine`)
   - `extension_http.rs`、`Cargo.toml`、`package.json` 均未提交
   - 已入库的 `hooks.nsh` 没接线 —— 重新 clone 会得到「有钩子但没接线」的仓库
2. 主仓 4 个提交未推送(含 `v0.5.9` 标签)
3. H2 语义假成功(样本仅 2 次)
4. 卸载路径从未端到端验证(卸载器的 `RMDir` 刚修过,未实测)
5. 扩展激活耗时 1–3 分钟根因未查
6. `tools\user_appdata.txt` 残留(46 字节,在受保护路径 `D:\Program Files\` 里)
7. `focus-sync-sidecar-canonical.exe`(2.45 MB,旧二进制,已确认不被引用)

### 3.6 关键互操作常量(改名时绝对不能动)

来自 v2 §2.3,与历史教训相关(Rule 53–54:"提解耦建议前必须先 grep 历史"):

| 文件:行 | 常量 | 用途 |
|---|---|---|
| `src/lan_share/types.rs:15` | `MDNS_INSTANCE_NAME = "Sigma File Manager"` | **mDNS 网络发现的服务名**(改了会让 LAN 共享搜不到本机) |
| `src/windows_installation.rs:30` | `LEGACY_DEFAULT_FILE_MANAGER_DATA_DIR_NAME` | **接管 Windows 默认文件管理器的数据目录名**(改了破坏兼容性) |

**特别提示**:**全局替换 `Sigma File Manager` 会把这两个一起改掉,而且不会报错,只会静默失效**。建议在 v2 §2.2 改动清单里直接写「不要全局替换,逐文件审核」,而不是只标注「这两个不动」。

---

## 4. 给 MiniMax 的协作建议(总结)

1. **v2 计划本身的诊断质量是好的,落地需要补 1 个 P0.5 + 4 个 P5 级工具脚本**。
2. **顺序调整**:先做 handoff §5 第 1 项(fork 打包配置入库),再做 v2 P1 第 1 步(构建基线),才能安全执行 v2 P1 的删除。
3. **§2.3 提到的两个互操作常量**:v2 标了"⚠️ 绝对不能改",但没标"**全局替换会误伤**"—— 建议在 §2.2 改动清单里直接写「不要全局替换,逐文件审核」。
4. **未来阶段(P6 / P7)** 的"判别性测试"思路非常对(Rule 56–60 沉淀的延伸)—— 把"门禁 PASS 本身不是证据、必须确认「真的做了」"这个原则,继续贯穿。
5. **v2 §4.1 第 3 轮的"实心字形 vs 描边"教训**很有价值,建议沉淀到 memory(类似 Rule 53 的"测量工具不能验证自己的结论"—— AI 模型对字形的渲染不可靠,对实心色块渲染可靠)。

---

## 5. 文档归档位置建议

- **本文件**:`docs/review-2026-10-09-plan-alpha-file-manager.md`(与 v2 计划并列,便于交叉引用)
- **v2 计划是否更新**:取决于用户决定 —— 如果接受本反馈,v2 文档需要更新为 v3
- **执行顺序调整**是建议性的,最终决策权在用户