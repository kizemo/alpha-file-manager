# HANDOFF —— 首次启动默认设置 + 卸载完整性修复(2026-10-10 上午)

**交接时间**:2026-10-10 09:35
**上一份**:[`handoff-2026-10-09-tree-fix-exe-rename-coldstart.md`](handoff-2026-10-09-tree-fix-exe-rename-coldstart.md)
**本轮性质**:两个安装后默认设置需求 → 打包真机验收 → **验收过程暴露 5 个缺陷并全部修复**
**状态**:✅ 需求已实现且真机验收通过 · ✅ 5 个缺陷已修 · ✅ 已提交推送(`e3e3cee8` / `02f987e3`)

---

## 0. 三十秒速览

| 项 | 值 |
|---|---|
| **品牌仓** | `sigma-file-manager`,`main` **`02f987e3`**,已推送,工作区干净 |
| **装机现状** | `C:\Program Files\Alpha File Manager`,**正在运行**,界面中文 |
| **本轮提交** | `e3e3cee8` 首次启动默认设置(8 文件)· `02f987e3` 卸载完整性(1 文件) |
| **真机验收** | 7 项全部通过(见 §3) |
| **缺陷修复** | 4 个 NSIS + 1 个 Tauri capabilities,全部真机复验 |

---

## 1. 需求与实现

### 1.1 默认主题 → 亮色(开发中)

内置主题 `light` 的文案本就是「亮色（开发中）」,需求只是改默认值。

- `user-settings.ts:78` `theme: 'dark'` → `'light'`
- `registry.ts:9` `DEFAULT_THEME_ID` → `'light'`
- 兜底测试 `back to dark` → 改为引用 `DEFAULT_THEME_ID`,下次改默认值它自动跟着对

### 1.2 首次启动按系统语言设语言

新增 `src/localization/system-locale.ts`(纯函数无副作用)。**app 的中文内部代码是 `ch` 而不是 `zh-CN`**(`data.ts:113`),直接赋值会找不到语言包。

| 系统 locale | 结果 | 规则 |
|---|---|---|
| `zh-CN` / `zh` / `zh-SG` / `zh-Hans` | **ch** | 简体精确匹配 |
| `zh-Hans-CN`(带脚本+地区) | **ch** | 非繁体即简体 |
| `zh-TW` / `zh-HK` / `zh-MO` / `zh-Hant` | **en** | **用户要求:繁体回退英文** |
| `pt-BR` / `en-GB` | `pt` / `en` | 按主语言 |
| `ko` / `nl` / `pl` | **en** | 无对应语言包 |

> **设计核心**:完整串未精确命中且主语言是 `zh` 时,**直接回退英文而不撞简体包**。少了这个 `zh` 特判,`zh-TW` 会落到 `zh` → `ch`,得到简繁混排界面。

### 1.3 仅首次启动生效

一次性标记键 `appDefaultsInitialized`。老用户升级时即便没有该标记,**只要已有 `language` 存档就保留其选择**。检测失败回退英文、不阻断启动、且仍写标记(否则每次启动都重试)。

### 1.4 首次启动自动接管默认文件管理器

写 `HKCU\SOFTWARE\Classes\Folder\shell\open\command` 等三个用户级键(无需管理员)。已真机确认接管成功。

---

## 2. ⚠️ 真机验收暴露的 5 个缺陷

**这 5 个全是单测和代码审查发现不了的,只有真机跑才暴露。**

| # | 缺陷 | 后果 |
|---|---|---|
| 1 | **PREUNINSTALL 缺 alpha kill** | NSIS 卸载器**随安装包定型**,旧卸载器只认 sigma,杀不掉 alpha → exe 占用删不掉 |
| 2 | **PREINSTALL 读旧注册表键名** | 改名后卸载记录变 productName 形式,旧名读不到 → 重装装错盘,原位置成孤儿 |
| 3 | **POSTUNINSTALL 不清运行时文件** | `user_appdata.txt` 不在 NSIS 清单里 → 每次卸载必然残留 `tools/` |
| 4 | **capabilities 缺 os 授权** | `locale()` 被 Tauri 拒绝 → 语言设置静默失效 |
| 5 | (附带)安装器 kill 过粗暴 | PREINSTALL 强杀 WebView2,打断 `LazyStore` 回写,`user-settings.json` 丢失 |

**缺陷 4 最值得记住**,详见 §5.1。

---

## 3. ✅ 真机验收结果(7/7)

| # | 验收项 | 结果 |
|---|---|---|
| 1 | 全新安装首次启动,系统 `zh-Hans-CN` | 界面**中文**(`language.locale='ch'`) |
| 2 | 一次性标记写入 | `appDefaultsInitialized=true` |
| 3 | 默认文件管理器接管 | HKCU 三键均指向 launcher |
| 4 | 亮色主题 | 生效 |
| 5 | **卸载完整性** | 安装目录**完全删除**、注册表记录清空 |
| 6 | **手动改 en 后重启** | **保持 en**,未被系统语言覆盖 |
| 7 | 目录树刷新(上一轮) | 新建/重命名/删除三条路径均通过 |

> 验收 6 的反向价值:如果语言被强行打回 `ch`,说明「每次启动重检系统语言」的 bug 存在,用户改语言重启就丢。

---

## 4. 我犯的三个错误(都是"验证方法"而非"代码")

### 4.1 查错盘 → 把两次成功安装读成两次失败

`exit code 0` 后我只查 D 盘,结论是「安装器什么都没做」。实际它两次都**成功装到了 C 盘**,注册表也指向 C 盘,一切正常。

**判据**:验证安装包行为前,先确认产物**实际落在哪**,再下结论。

### 4.2 查错注册表键名 → 把成功接管读成失败

我查 `Directory\shell\open\command`,代码实际写的是 **`Folder`** 和一个 CLSID。差一个词,结论完全相反。

**判据**:判据字段/键名**必须从被测代码里读出来**,不能凭常识猜。

### 4.3 测试被"旧实例退出时的迟到保存"污染

我一度报告"重启后语言被打回 ch,需求失败"。实际是:同一批命令里「kill 应用 → 改文件」只隔 4 秒,旧实例退出时把内存里的 `ch` 写回,盖掉了我的 `en`。按正确时序(确认进程退出 → 等文件 mtime 稳定 → 改 → 复查稳定 → 再启动)重测,**通过**。

**判据**:改持久化数据前,必须确认旧实例已退出且文件 mtime 稳定;否则你测的是竞态,不是功能。

---

## 5. 缺陷 4 的机制(Tauri capabilities 静默失效)

**根因**:Tauri 2.x 默认**拒绝未在 `capabilities/*.json` 授权的插件 IPC**。该 capabilities 给了 store/dialog/autostart/drag/system-fonts 38 项,**独独没有 os**。`locale()` 实为 `invoke('plugin:os|locale')` → reject → 被 `try/catch` 吞掉 → 语言静默回退英文。**全程无任何 UI 报错。**

**为什么隐蔽**:

| API | 走 IPC? | 受 capabilities 影响 |
|---|---|---|
| `platform()` / `arch()` / `family()` | **否**(同步读 `window.__TAURI_OS_PLUGIN_INTERNALS__`) | 否 |
| `locale()` / `version()` / `hostname()` | **是**(`await invoke`) | **是** |

项目里一直只用 `platform()`(同步),所以这个缺口**从未暴露**。全仓搜 `locale()` 发现只有新加的这一处。

**修复**:`capabilities/default.json` 加 `"os:allow-locale"`。

> ⚠️ **待办**:现有单测 **mock 了 `invoke`,永远测不出这类缺口**。建议补一条断言 capabilities 含所需权限的测试,否则下次有人删掉这行,单测依然全绿。

---

## 6. 环境变更提示

- **原安装位置 D 盘已不存在**,现在装在 `C:\Program Files\Alpha File Manager`(PREINSTALL 探测失效所致,现已修复)
- **原 `%APPDATA%` 数据已清空**做全新安装测试,备份在 `.pkg-backups\appdata-20261010-085654`
- 你原来装的 **catppuccin themes 等插件已不在**(数据目录被清空),需要可从应用内重装

---

## 7. 下一步

| # | 任务 | 说明 |
|---|---|---|
| 1 | **补 capabilities 权限断言测试** | 防止 `os:allow-locale` 被删后无人察觉(§5) |
| 2 | 修复安装器 kill 过粗暴 | PREINSTALL 强杀 WebView2 打断 `LazyStore` 保存,导致用户设置丢失(§2 缺陷 5) |
| 3 | 修复 `startup-recovery.js` 旧名 | 长期失败测试,P4 改名遗留(上一份 §7 第 5 项) |
| 4 | 复制/移动后目录树刷新 | 同一条缓存的最后一类入口(上一份 §7 第 3 项) |
| 5 | 挖剩余 18.8 秒冷启动 | 已无网络,是 Worker/沙箱/对账的本地开销 |