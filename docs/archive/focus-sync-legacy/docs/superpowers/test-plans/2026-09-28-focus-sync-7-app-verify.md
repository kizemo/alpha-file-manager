# 7-app GUI Verification Template — focus-sync v0.2.0

**日期**: 2026-09-28
**Plugin**: `kizemo.focus-sync` v0.2.0 (release 2026-09-27)
**Sidecar**: focus-sync-sidecar-windows-x64.zip
**测试方法**: 见 `handoff-2026-09-28-plugin-route.md` 阶段 1

---

## 安装步骤

1. 从 https://github.com/kizemo/focus-sync/releases/tag/v0.2.0 下载 `focus-sync-sidecar-windows-x64.zip`
2. 解压到 `D:\mcp-output\`（或任何固定路径）
3. 打开 Sigma File Manager (>= v2.2.0)
4. 菜单: **Extensions → Add from URL**
5. 粘贴: `https://github.com/kizemo/focus-sync/releases/latest/download/package.json`
6. Sigma 自动下载 sidecar 到 `%APPDATA%\com.sigma-file-manager.app\binaries\focus-sync-sidecar\0.2.0\`
7. 重启 Sigma
8. 工具栏应出现 **Focus Sync** 下拉按钮

---

## 测试矩阵（每个 app 一个 section）

### App 1: Chrome（Chromium 下载对话框）

**测试时间**: _________
**状态**: ☐ PASS  ☐ FAIL  ☐ MARGINAL

**测试步骤**：
1. Chrome 打开任意下载链接（任意 PDF/图片）
2. Chrome 弹出"另存为"对话框（Chromium 类型）
3. 切到 Sigma，切到 `D:\test-folder\`
4. 切回 Chrome 对话框
5. 观察：是否跳转到 `D:\test-folder\`？还是保留原位置？

**观察记录**：
- 地址栏显示路径: _________
- 对话框焦点位置: _________
- sidecar 日志（如果有）: _________

---

### App 2: Edge（Chromium 下载对话框）

**测试时间**: _________
**状态**: ☐ PASS  ☐ FAIL  ☐ MARGINAL

**测试步骤**: 同 Chrome

**观察记录**: _________

---

### App 3: Word（Win32 OpenFileDialog）

**测试时间**: _________
**状态**: ☐ PASS  ☐ FAIL  ☐ MARGINAL

**测试步骤**：
1. Word 打开任意文档 → 菜单 File → Save As
2. Win32 "另存为" 对话框弹出
3. 切到 Sigma，切到 `D:\test-folder\`
4. 切回 Word 对话框
5. 观察：地址栏是否跳转到 `D:\test-folder\`？

**观察记录**: _________

---

### App 4: VS Code（Electron Open Folder 对话框）

**测试时间**: _________
**状态**: ☐ PASS  ☐ FAIL  ☐ MARGINAL

**测试步骤**：
1. VS Code 菜单 File → Open Folder
2. Electron "打开文件夹" 对话框
3. 切到 Sigma，切到 `D:\test-folder\`
4. 切回 VS Code 对话框
5. 观察：当前路径是否变？

**观察记录**: _________

---

### App 5: 钉钉（Chromium 发送文件）

**测试时间**: _________
**状态**: ☐ PASS  ☐ FAIL  ☐ MARGINAL

**测试步骤**：
1. 钉钉打开任意聊天 → 点 + → 发送文件
2. 弹出文件选择对话框
3. 切到 Sigma → `D:\test-folder\`
4. 切回钉钉对话框

**观察记录**: _________

---

### App 6: 飞书（Chromium 上传文件）

**测试时间**: _________
**状态**: ☐ PASS  ☐ FAIL  ☐ MARGINAL

**测试步骤**: 同钉钉

**观察记录**: _________

---

### App 7: 微信（Chromium 发送文件）

**测试时间**: _________
**状态**: ☐ PASS  ☐ FAIL  ☐ MARGINAL

**测试步骤**：
1. 微信打开任意聊天 → 点 + → 发送文件
2. 弹出文件选择对话框（Chromium 类）
3. 切到 Sigma → `D:\test-folder\`
4. 切回微信对话框

**观察记录**: _________

---

## 综合判定

**通过数**: _____/7

| 结果 | 行动 |
|---|---|
| ≥5/7 PASS | 进入 v0.3.0 release + 上游 PR + fork 整合 |
| 3-4/7 PASS | 修复后再提交（v0.3.0 + 重测） |
| ≤2/7 PASS | STOP — 重评 Listary 路线可行性 |

---

## 失败案例记录（如果 FAIL）

**App**: _________
**失败现象**: _________
**sidecar 日志**: 
```
[paste here]
```
**Sigma 版本**: _________
**Windows 版本**: _________
**复现步骤**: _________

---

## 备注

- 测试时关闭其他可能干扰的窗口（OneDrive / Dropbox / WPS 等）
- 测试前先打开一次目标 app，确认对话框能正常关闭
- 如果某 app 始终不工作，看是否启用了系统级 Chromium 屏蔽（fg_bypass 可能让 dialog 直接禁用）