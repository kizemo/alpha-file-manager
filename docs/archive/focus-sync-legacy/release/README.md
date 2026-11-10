# Focus Sync 独立安装包 —— 发布存档

> **2026-10-09 更新:** 本目录现在只存**已发布的安装包**,不再有源码。
> 插件源码与集成包构建已迁至 <https://github.com/kizemo/alpha-file-manager>
> 的 `extensions/kizemo.focus-sync/`。

---

## 1. 这里有什么

```
release/
├── kizemo.focus-sync-0.5.8-setup.exe    ← v0.5.8,用户实测通过的那一版
└── README.md                             ← 本文件
```

**整合包(文件管理器 + 插件预装)不再在本仓产出。** 它由品牌仓构建。

---

## 2. 这个安装包干什么

给**上游 Sigma File Manager 用户**装 Focus Sync 插件用的独立安装包。
标准 NSIS 安装器,装完插件自动注册、计划任务自动建好,无需手工拷文件。

装好后:打开 Sigma FM → **Extensions** → 找到 **Focus Sync** → **Enable**。

> ⚠️ **装之前先手工退出 Sigma FM。** 杀不掉运行中的程序时安装器会**直接中止**
> 并留下 `%TEMP%\focus-sync-install-FAILED.txt`,以免装出「注册丢失、插件静默
> 不加载」的坏状态。

**装完不能立刻测。** 扩展激活需 1–3 分钟,期间地址栏不同步是**预期行为**,
不是装坏了。判断方法:

```powershell
(Invoke-RestMethod http://127.0.0.1:37421/health) | Select-Object extension_alive
```

`extension_alive=false` → 还在冷启动,等;`true` 还不同步 → 才是真问题。

---

## 3. 需要重新发版时

源码与打包脚本都在品牌仓:

```powershell
cd <品牌仓>
cd extensions/kizemo.focus-sync/sidecar; cargo build --release
cd ../../..; powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build-with-sidecar.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File extensions\kizemo.focus-sync\installer\build.ps1
```

产出的 `kizemo.focus-sync-0.5.8-setup.exe` 复制到本目录存档。

---

## 4. 技术要点(仍然成立的部分)

### 4.1 装到哪、为什么

插件装在**唯一规范路径**:

```
%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\
    bin\focus-sync-sidecar\focus-sync-sidecar.exe
```

早前版本装在 `D:\Program Files\Sigma FM\tools\`,两套安装器因此互相覆盖
计划任务、留下孤儿副本,并使验收门禁**永远不可能通过**。现在只有一个规范路径,
per-user 安装器也因此不需要提权。

### 4.2 为什么预置 sidecar 在共享路径

文件管理器的扩展运行时会调 `get_shared_binary_path`。若该路径已存在文件,
它**直接复用而不重新下载** —— 预置即得到离线支持。

### 4.3 计划任务

sidecar 由 Windows 计划任务 `KizemoFocusSync`(登录时启动)拉起,
与文件管理器生命周期解耦。`register-scheduled-task.ps1` 检测到路径变化会自动
注销旧任务并重建。

---

## 5. 源码与许可证

| 组件 | 源码 | 许可证 |
|---|---|---|
| Focus Sync 插件 + sidecar | `alpha-file-manager` → `extensions/kizemo.focus-sync/` | **GPL-3.0-or-later** |
| 集成包(文件管理器本体) | `alpha-file-manager` | GPL-3.0-or-later |
| Sigma File Manager(上游基座) | `aleksey-hoffman/sigma-file-manager` | GPL-3.0-or-later |

> ⚠️ 插件此前在本仓以 MIT 分发。源码迁入 GPL-3 仓后,随二进制分发的组件
> **整体按 GPL-3 分发**。GPL-3 §6 要求分发二进制时同时提供对应源码,
> 获取方式见本仓根 `README.md` §4。