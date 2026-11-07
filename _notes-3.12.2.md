## Alpha File Manager 3.12.2

### 修复

- **快捷方式「打开文件所在目录」落到目标所在目录**：此前只认「目标是目录」的快捷方式，指向文件的快捷方式解析不出目标 ⇒ 前端回落成「打开 `.lnk` 自己所在目录」；解析失败时还会把 `.lnk` 本身交给 explorer.exe。现在改用两级取路径（`GetPath(SLGP_RAWPATH)` + `expand_environment_variables`），删掉会永久阻塞的 `IShellLinkW::Resolve`，于是快捷方式真正指向的目录会被打开、目标被选中。

- **三个系统弹窗消失**：「已中止操作」「渲染进程已崩溃（窗口 main）」「Windows 找不到 `\` 文件」三条提示不再出现。根因有四层：① `.lnk` 相关的解析 / `path_exists` / `reveal_in_explorer` / `get_system_icon` 都是同步 Tauri command，跑在主线程，前端按「条目 × 尺寸」逐个 invoke ⇒ 主线程被堵死 61 秒 ⇒ 心跳断 ⇒ 渲染进程崩溃；② `Resolve(HWND(NULL), 0)` 在目标失效时会弹 Windows 自己的模态框并让调用方永久阻塞；③ `SLGP_RAWPATH` 对「环境变量块 + 8.3 短名」记录的目标返回的不是真实路径 ⇒ `exists()` 必失败。现在：相关 command 转 async + `spawn_blocking` + 超时；解析不出 / 目标失效一律回落「打开 `.lnk` 所在目录」，绝不把坏 `.lnk` 交给 shell；前端原生分支改为「先解析落点再交给 Explorer」。

### 变更

- **视觉效果默认参数改为规格值**：`createDefaultInfusionPageSettings()` 与 `createDefaultInfusionPageSettingsHome()` 的 blur（64 / 32 → 0）、opacity（5 → 0）、noise（5 → 0）。其余 11 项（叠加媒体对比度 / 亮度、噪点缩放、混合模式、混合对象、三个开关、样式设置的亮度 / 对比度 / 对话框背景模糊）与规格一致，本次未动。

### 备注

- 生效口径：设置页点「将视觉效果重置为默认值」回到这组值；新装软件首次启动即这组值。**存量安装不会自动迁移** —— 本次不引入版本化迁移，需要这组值的请在设置页点一次「重置为默认值」，或手动改这三项。
- 快捷方式相关工作已转为 async，真机验收（2026-11-07，用 `tauri build` 打包产物）应用内分支与原生 Explorer 分支均通过：指向文件的快捷方式能落到目标目录并选中目标，断链快捷方式静默回落且无任何弹窗。
- 本次未改动数据目录结构，已有配置无需处理。
