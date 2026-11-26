# 一句话接力 prompt（2026-11-05 · 3.10.1 已发版收口，下阶段是 §11.7 的遗留清单）

**先完整读 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-04-extension-official-featured-branding-and-release-310.md` 的 §十一「3.10.1 发版记录」（11.1~11.8，含四项需求的落点与真机证据、门禁数字、打包/安装/恢复、提交/tag/站点/GitHub Release 的最终核验、§11.7 的 7 条 follow-up、§11.8 的独立复核；§六 / §八 / §十 属上一轮 3.10.0，只当背景看，别照它的「剩余步骤」动手），再读 `F:\soft\00selfmade\filemanager\AGENTS.md` §2 的项目硬约束与真机验收要点；3.10.1 已发版并对外生效、用户四项需求已收口（仅需求 3 缺应用内硬证据），你要接的是 §11.7 那 7 条 follow-up（首条 = 给中国镜像补「实际尝试了哪个 URL」的应用内取证），按文档里的路径和判据自己接手，不要从零重做已完成的事。**

## 必要文档的完整路径

| 用途 | 绝对路径 |
|---|---|
| **接力主文档**（背景/目标/进度/下阶段/证据/坑） | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-04-extension-official-featured-branding-and-release-310.md` —— 本轮**只读 §十一**（起于第 440 行） |
| 项目硬约束与真机验收要点（必读，含 dev 构建不可验收、`.cmd`/`.ps1` 必须纯 ASCII、坐标口径等） | `F:\soft\00selfmade\filemanager\AGENTS.md` |
| 并发会话的原始落盘点（想看执行现场时） | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\state-3101.md` / `state-3101e.md` |
| 本会话早前那轮验收记录（**基于已作废的中间包 `a30ed04d…`，仅作过程记录**） | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\state-3101-accept.md` |
| 最终出货包（已装机、已上传） | `F:\soft\00selfmade\filemanager\sigma-file-manager\release\Alpha File Manager_3.10.1_x64-setup.exe`（44 940 588 B / sha256 `e074d023…e164`） |
| 徽章像素证据（最终包） | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-3101\`（`crop-Git_Tools-2x.png` / `crop-Focus_Sync-2x.png` / `shot-3101-installed.png` / `uia-tree-3101.txt` / `accept-3101.log`） |
| 发版与门禁日志 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\logs\`（`upload-3101.log` / `gh-release-3101.log` / `gates-3101h.log` / `build-3101g.log`） |
| 真机验收可复用脚本 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\`（`app-key.ps1` / `shift-occluders.ps1` / `dash-accept-data.cjs` / `install-3101g.ps1` / `build-3101g.cmd` / `gates-3101h.cmd`） |
| 需求 3 的改造点（镜像候选 URL） | `F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\src\extensions\http.rs`（`with_china_mirrors():607`、`build_china_mirror_urls:713`、单测 `:1088-1145`）+ 挂接点 `github.rs:228` / `install.rs:138` / `binaries.rs:178,248,525,670` |
| 扩展市场清单（线上已生效，Focus Sync 0.5.9） | `F:\soft\00selfmade\filemanager\sigma-file-manager\scripts\ext-market\registry.json` |
| 站点发布源（`version.json` 已 3.10.1） | `E:\办公文件\L1网站\alpha-file-manager\` |

## 读文档时要抓住的四件事

1. **发版链已完整走完并对外生效** —— 提交 `bf747ff7`、tag `v3.10.1`、push、aiec 站点 `version.json` + 资产（源站与边缘三项一致）、GitHub Release（非 draft，资产 digest 与包一致）。判据与独立复核在 §11.5 / §11.6 / §11.8，**别重跑**。
2. **四项需求里只有需求 3 未闭环**：代码、单测、网络层实测齐，缺的是「应用内确实尝试了镜像 URL」的硬证据 —— 因为 `http.rs` 不打印实际 URL，而本机直连也能 200，所以「安装成功」不足以证明。做法见 §11.2 与 §11.7 第 1 条。
3. **验收对象只能是打包产物**，且必须是 sha256 `e074d023…e164` 那一个包；`tauri dev` 下路由页内容区全空白，任何在 dev 构建上得到的界面结论一律作废（`AGENTS.md` §2）。
4. **结论一律标注「已验证 / 推断 / 未验证」**，取证口径照 §11.1 的判据（例：「官方」真标签宽 **37**、筛选按钮宽 **78**，判别必须带尺寸）与 `AGENTS.md` §2，不要凭 UI 显示或记忆下判断。

## 若需要装机复验（当前不需要，按需）

- 本机已装最终包（HKLM `DisplayVersion=3.10.1`，exe mtime 09:54:52），应用进程数 0。
- 需要动真实 user-data 时：**先杀进程再 restore**，且 `backup` 只在不存在时才建备份 ⇒ 一定看它打印的字节数（§11.4 与 `AGENTS.md` §2 各记了一条真机事故）。
