# §5-1 评审包 —— fork 打包配置入库(方案待定,**未提交任何东西**)

**时间**:2026-10-09 10:25
**对应**:`handoff-2026-10-09-packaging-closeout.md` §5 第 1 项
**本轮状态**:git 历史**未改动**(主仓 HEAD `dc5f152`,fork HEAD `4365281b`,均未提交、未推送)
**结论**:§5-1 的描述属实,但它**不是一个提交动作能解决的问题** —— 真正的卡点在下面第 3 节。

---

## 1. 已核实的事实(带对照探针)

§5-1 说「今天已把 `hooks.nsh` 入库,但引用它的配置还没入库」。已核实,**属实**。

```powershell
cd sigma-file-manager
$head = git show HEAD:src-tauri/tauri.conf.json
```

| 探针 | 结果 |
|---|---|
| 已入库的 `tauri.conf.json` 含 `installerHooks`? | **否** |
| 已入库的 `tauri.conf.json` 含 `bundle.resources`? | **否** |
| `src-tauri/installer/hooks.nsh` 本身已入库? | **是** |
| **对照探针**:同一方法能命中 `installerIcon` / `installMode`? | **是** ✅ |

> 对照探针是必须的:没有它,「查不到 `installerHooks`」无法区分「配置里真的没有」
> 和「我的查找方法坏了」。这一条直接来自 §9 经验教训 2。

⇒ **重新 clone 得到的仓库:`hooks.nsh` 在,但没有任何东西调用它。**
安装器会正常装上 Sigma FM,只是**静默跳过全部 focus-sync 部署** —— 无报错、无提示。

## 2. 6 个未提交文件分属两类

`git status` 实测(fork,6 改 + 1 未跟踪):

| 类 | 文件 | 内容 | 性质 |
|---|---|---|---|
| **A 打包接线** | `src-tauri/tauri.conf.json` | `bundle.resources`(8 条)+ `installerHooks` + `installMode: currentUser→perMachine` | §5-1 点名的就是这一个文件 |
| **B 构建兼容** | `src-tauri/Cargo.toml`<br>`src-tauri/Cargo.lock` | `tauri-plugin-*` 由 `"2"` 钉到 `"~2.4"`;`tauri-plugin-opener` `=2.5.4`;`windows-core 0.61.2→0.62.2`;`webview2-com 0.38.2→0.39.1` | 与打包无关,是**编译能不能过**的问题 |
| **B 构建兼容** | `package.json`<br>`package-lock.json` | 同批 JS 侧钉版本(`^2.11.1→^2.4.0` 等)+ 补文件末尾换行 | 同上 |
| **B 功能修复** | `src-tauri/src/extensions/extension_http.rs` | `.no_proxy()` —— focus-21 v0.3.7 的 `NO_PROXY` 修复 | 与打包**完全无关**,是插件功能修复 |

**B 类的来历**:`Cargo.toml` / `package.json` 的降级钉版本,是让 fork 能在 **Rust 1.90** 下编译的那批改动
(§3 硬约束 12;findings 文档记录 `stable` 1.97.1 会报 244 个第三方 crate 错误)。
`extension_http.rs` 的 `.no_proxy()` 则是插件「推送被代理拦掉」的历史修复。

> ⚠️ **这条因果关系是推断,不是实测。** 我没有跑过构建去证明「去掉 B 类就编不过」。
> 依据是 §7 运维命令与硬约束 12 的记载。若要作为决策依据,应先验证。

## 3. ★ 新发现:打包输入**全部在主仓**,fork 无法独立构建

这是 handoff §5-1 没有写、但会直接改变方案的一条。

`tauri.conf.json` 里 `bundle.resources` 的 8 条源路径全部是 `../../release/...`。
从 `<fork>\src-tauri\` 解析出来,**全部落在主仓里,一个都不在 fork 仓内**:

| 状态 | 解析结果 |
|---|---|
| 8 / 8 | `OUTSIDE fork → F:\soft\00selfmade\filemanager\release\...` |
| 8 / 8 | 文件都存在 |

同时已核实:主仓 `.gitignore` 第 2 行 `sigma-file-manager/` 把整个 fork 排除;
主仓 `git ls-files sigma-file-manager` = **0 个文件**;fork 有独立 remote
(`origin`=kizemo,`upstream`=aleksey-hoffman)。

### 这意味着什么

> **把 `tauri.conf.json` 提交进 fork,只能把仓库从「没接线」变成「接线了但指向不存在的文件」。**
> 单 clone fork 仓,`tauri build` 仍然拿不到任何 focus-sync 载荷。

两仓必须**并列摆放在固定相对位置**:

```
<任意目录>/
├── <主仓>/          ← release/extension/…  等 8 个打包输入在这里
└── <主仓>/sigma-file-manager/   ← fork 仓,靠 ../../ 找到上面那些
```

这个约束目前**在任何文档、注释、脚本里都没有写**,也没有门禁守着。
它比「配置没入库」更危险:配置入库后,失败会从「静默不部署」变成
「构建直接因缺文件失败」——后者反而是好事,但仍需有人知道该摆哪儿。

## 4. 附带发现:fork 的 `release/` 未被忽略

```
?? release/   →  Sigma-File-Manager-2.2.0-focus.1-x64-setup.exe  (15,881,329 字节)
                SHA256SUMS.txt / README.md
```
`.gitignore` 里**没有任何 `release` 规则**,`git check-ignore` 确认未被忽略。
是构建产物,不该入库,但目前会一直挂在 `git status` 里。

## 5. 供讨论的方案方向(**未实施**)

|  | 方案 | 能解决 | 代价 |
|---|---|---|---|
| **1** | 最小闭环:只提交 A 类 | §5-1 字面要求 | 得到「接线对但编不过」的中间态(若第 2 节的推断成立) |
| **2** | 6 个全提交 | 把**当前实测可用**的状态整体固化 | 跨仓依赖仍然存在且仍然没文档化;1971 行 lockfile 变动;依赖降级影响整个应用 |
| **3** | **让耦合显式化**:fork 的 `resources` 改指 fork 内部路径,由主仓构建脚本先把载荷 stage 进 fork(可复用已有的 `build-with-sidecar.ps1`,它本来就在做 staging + 沙箱门禁) | 两仓各自自洽;跨仓耦合变成**脚本里可见的一步**而不是 `../../` 的隐式约定 | 要改已验证的 `tauri.conf.json` 路径 ⇒ **必须重新构建 + 重跑验收门禁**;fork 与 upstream 的偏离变大 |
| **4** | 把打包输入搬进 fork 仓(子模块 / subtree) | fork 真正自洽 | 插件产物在两仓间产生同步问题;与「插件归主仓」的仓库边界冲突(README「仓库边界」一节) |

> 方案 3 是我倾向的方向,但它会**动到当前已验证可用的配置**,因此必须重新构建并重跑
> `verify-deploy-sha.ps1` 才能说它成立。在重新拟定方案时,这一条的成本要先摆出来。

## 6. 我测了什么 / 没测什么

**测了**(可复现):
- 已入库的 `tauri.conf.json` 不含 `installerHooks` / `bundle.resources`,含 `installerIcon` / `installMode`(对照)
- 8 条 `bundle.resources` 源路径全部解析到主仓,且文件均存在
- 主仓 `.gitignore` 排除 fork;主仓未跟踪 fork 任何文件;fork 有 origin + upstream
- fork `release/` 未被 `.gitignore` 覆盖
- 6 个未提交文件的完整 diff(已读,未改动)

**没测**:
- ❌ **没有跑过任何构建**。「B 类缺失就编不过」是推断,未实测
- ❌ **没有验证提交后重新 clone 是否真能产出可用安装包**(这正是 §5-1 想达到的目标,
  但在方案定下来之前无法验证)
- ❌ 没有动 git 历史,没有切分支,没有推送

**git 状态自证**:主仓 HEAD `dc5f152`、fork HEAD `4365281b`,与本轮开始时一致。
