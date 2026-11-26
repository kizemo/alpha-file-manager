# 批次 25 方案：手机媒体缩略图/文件大小 + U 盘安全弹出

创建：2026-11-19 16:50｜前置：批次 24 已收口（提交 `7ad71f68`）
承接：`docs/plan-2026-11-19-shell-devices-locations.md`、台账 `docs/handoff-2026-11-11-quark-pan.md` §20.20

---

## 0. 两个缺陷（用户 2026-11-19 16:13 提出）

| 编号 | 现象 | 期望 |
|---|---|---|
| B25-1 | 手机存储（MTP/WPD）中的**图片、视频不显示缩略图**，且**文件大小不显示** | 两者都正确显示 |
| B25-2 | AFM 运行时，U 盘无法通过任务栏托盘「安全弹出」，提示「被占用」 | 能弹出；若 AFM 打开着 U 盘目录，则关闭该标签或跳转回「位置」界面 |

---

## 1. B25-1 根因分析（源码事实，已坐实的与待真机判定的分开）

### 1.1 已坐实（静态代码事实）

- `src-tauri/src/image_thumbnails.rs` 整条链路是**文件系统 `Path` 语义**，不是 shell 语义：
  `validate_image_thumbnail_source:484` → `decode_image_thumbnail_source:580` →
  `generate_image_thumbnail_file:737` → `generate_image_thumbnail:831`；
  视频 `get_cached_video_thumbnail:875` / `cache_video_thumbnail:893`；
  缓存键 `thumbnail_cache_key:154` / `video_thumbnail_cache_key:164`；
  缓存目录 `thumbnail_cache_dir:193` / `video_thumbnail_cache_dir:200` / `init_thumbnail_cache_dir:1023`。
  MTP 的 `::{...}\...` 解析名不是 Win32 路径 ⇒ `std::fs` 必然失败 ⇒ 缩略图为空。**这是结构性缺陷，不需要真机验证。**
- 前端调用形状：
  `use-image-thumbnails.ts:198` → `invoke('generate_image_thumbnail', {path, modifiedTime, size, maxDimension})`；
  `use-video-thumbnails.ts:170,182` → 先 `get_cached_video_thumbnail`，未命中则**前端自己用 `<video>` 解码**再 `cache_video_thumbnail`。
  `convertFileSrc` 的 asset 协议同样打不开 MTP 路径 ⇒ 视频的「前端自解码」分支必须分流。
- 大小的取值点：`dir_reader/shell_namespace.rs:391-401` `item_size()` =
  `IShellItem2::GetProperty(&PKEY_SIZE)` → `PropVariantToUInt64`，**任一环节失败即 `return 0`，HRESULT 被静默吞掉**；
  `shell_item_to_dir_entry():431-442` 确实把 size 写进 DTO（`:436` `let size = if is_dir { 0 } else { item_size(item) };`）
  ⇒ **「大小不显示」不是没接线，而是运行时返回 0**（或 `IShellItem2` 取不到）。

### 1.2 必须真机判定（未判定，不许先写结论）

1. MTP 条目上 `GetProperty(&PKEY_SIZE)` 的**实际 HRESULT**。三种可能，修法各不相同：
   - 直接失败 ⇒ 需换 `SHGetPropertyStoreFromParsingName`，或走 MTP 的 `IMAPIProperties`；
   - 成功但 PropVariant 的实际 `varType` 不是 `VT_UI4|VT_UI8` ⇒ 需按 varType 取值；
   - 取到值但在 Rust DTO / IPC / 前端渲染某层被丢 ⇒ 修那一层。
2. **`VT_UI4` 截断**是独立缺陷：手机里 >4 GB 的文件（视频常见）会溢出 ⇒ 判据须含一个 >4GB 文件。
3. 「**完全不显示大小**」与「**显示 0 B**」是两种根因（字段缺失被跳过渲染 vs `size=0` 被原样渲染），
   必须用 UIA 读单元格文本原文区分，**不许靠截图猜**。
4. A 线 oracle 报出：一加 Ace 3 设备根 `count=1`、子项 `FILE ext= name=Items size=0 path=`（`path` 为空）。
   需判定是 oracle 自身误判，还是产品侧 `DirEntry.path` 对 MTP 条目真为空
   —— 后者会让缩略图与大小两条一起瞎（前端拿不到 shell 路径）。
   反证线索：批次 24 真机 UIA 能逐级下钻到 `SID-{10001,,1001925963776}`
   （`.superpowers\sdd\b24-final-d2-crumb-root.txt`、`b24-final-d3-center.txt`）。

### 1.3 修复方向（定契约，等真机数据再落码）

- **分流点定在 Rust 侧现有 command 内部**，按 `::{` 前缀判断（**A 线不新增 tauri command**，避免前后端同时改契约）。
- shell 路径分支：`IShellItemImageFactory::GetImage(size)` → WIC 编 PNG → 落进现有缓存目录 → 返回与文件系统分支同格式的 DTO。
- 视频：前端加「shell 路径不走 `<video>` 自解码」分流；缩略图统一由后端 `IShellItemImageFactory` 出图。
- 大小：按 1.2 的判定结果选路；**失败原因必须进 `tracing`，不得静默返回 0**。

---

## 2. B25-2 根因分析

### 2.1 已坐实（静态代码事实）

- 目录监听用 `notify::RecommendedWatcher`（Windows 后端 = `ReadDirectoryChangesW`，**持有目录句柄**）：
  `src-tauri/src/dir_watcher.rs:19` 实现、`:44` 全局 `OnceLock<Mutex<..>>`、`:48` `ACTIVE_WATCHERS: HashMap<PathBuf, WatchTarget>`；
  `watch_directory:446` / `unwatch_directory:547`，注册于 `lib.rs:546-547`。
  ⇒ 只要 watcher 未释放，卷句柄被占，Windows 判定「被占用」。
- 全库 `set_current_dir` **零命中** ⇒ 「进程 CWD 落在 U 盘」这条嫌疑已排除。
- 全库 `WM_DEVICECHANGE` / `DBT_` **零命中** ⇒ AFM 完全没有设备变更/移除监听，
  即 Windows 发起移除时 AFM 不会主动放手。**这是必须补的能力缺口，不依赖真机验证。**

### 2.2 待真机判定

占用者的**句柄级归因**（候选：`dir_watcher` / `global_search` 索引 / 缩略图缓存 / WebView2 asset 协议 / CWD / 卷句柄），
每条结论要求「句柄证据 + 代码行号」配对。B 线取证单 Step 0~5，**只有末步才真弹出**。

### 2.3 修复方向

新增 `src-tauri/src/device_events.rs`：
`CreateWindowExW(HWND_MESSAGE)` message-only window + `RegisterDeviceNotification(DBT_DEVTYP_VOLUME)` + `GetMessageW` 循环 →
解析 `DBT_DEVICEREMOVEPENDING` / `DBT_DEVICEARRIVAL` 的 `dbcv_unitMask`（位 → 盘符）→
按卷前缀**批量 unwatch** 并 `app.emit("device-remove-pending", {drive, root})` →
前端关闭对应标签或跳转「位置」界面。

---

## 3. 真机取证纪律（本批次硬性）

- **U 盘 `H:` 内是用户约 24 GB 真实数据 ⇒ 禁止在 H: 写/建/改名/删除任何文件**，只允许只读、句柄枚举、弹出尝试。
- 不得为造夹具往用户手机写文件；若必须造，只允许 push 到 `/sdcard/Download/`、前缀 `AFM_B25_PROBE_`、事后只删自己创建的那几个并回读确认。
- adb 是**真值参照系**（`C:\Tools\android\platform-tools\adb.exe`），产出对照表：
  `文件名 | adb 真值字节数 | System.Size 返回 | HRESULT | DirEntry.path`，图片/视频各 ≥3 个，含一个 >4 GB。
- **H: 作阳性对照**：同机同进程下「文件系统路径 vs shell 路径」。若 H: 侧也异常 ⇒ 存在共用层缺陷，优先级重排并立即上报。
- 合成点击探针前置三件套固定：`SetProcessDpiAwarenessContext(-4)` → `SetForegroundWindow($hwnd)` → 断言 `GetForegroundWindow() -eq $hwnd`。
- 取帧：按目标进程过滤 → 按面积降序取最大（候选混有 IME 0×0 与 `GDI+ Window` 1×1）→ 三重校验；优先 `PrintWindow(hwnd,hdc,2)`。
- 抓屏前先确认在 **Default 桌面**（影刀 RPA 虚拟桌面取帧无效）。
- 未判定项一律登记「未判定」，不写 PASS/FAIL。

## 4. 现场基线（2026-11-19 16:35–16:47 实测）

- 一加 Ace 3 在位：`class=WPD status=OK id=USB\VID_22D9&PID_2765&MI_00\6&337F5E7C&1&0000`（实例段与批次 24 权威路径逐字一致 ⇒ 未重插）；`MI_01 = ADB Interface` ⇒ USB 调试开着。
- U 盘：`H:` 卷标「小兵U盘」，Kingston DT microDuo 3C，`DriveType=2`，NTFS，`245862625280 B` / 剩余 `221715079168 B`；C:/D:/E:/F:/G: 全 `type=3`。
- AFM 装机版：`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，`pid=79340`，启动 15:57:55。

## 5. 门禁基线（收口须对齐）

```powershell
cd src-tauri; cargo test --lib    # 1436 passed / 0 failed / 1 ignored
cd ..; npm run check              # 315 files / 3336 tests / RESULT: VALID
npm run verify:sandbox            # RESULT: VALID (0 violations / 20 patterns)
npm run tauri:build               # 同版本重装会把新包落到 release\local-builds\
```

## 6. 交付物

- 真机取证：`.superpowers\sdd\b25-thumb-{probe.txt,size-oracle.txt,report.md}`、`.superpowers\sdd\b25-usb-{handles.txt,diagnosis.md,OPS-ORDER.md,*.ps1}`
- 结论回填：台账 `docs/handoff-2026-11-11-quark-pan.md` 新增 §20.21
- 提交：批次 25 全部改动一次提交（版本号继续折进 3.15.0，不 bump）

## 7. 红线

- A 线独占 `cargo`（避免争 target 锁）；A/B 文件所有权隔离。
- **探针代码不得遗留在产品源码里**；收口前 `git status --short` 核对，产品源码只剩正式修复。
- B 线在拿到句柄级证据前不得改产品代码；Step 5（真弹出）是否执行由根会话决定。
- 改注册表让 U 盘双击进 AFM（`Classes\Drive`）属系统行为改动，需先问用户。
