# Prompt — 验证 round 19d 修复

承接 handoff-2026-09-30-installer-a-wmi-fix.md。

**当前状态**:round 19d 修复已实施并 build 完成,installer 16.94 MiB 位于 `release/Sigma-File-Manager-2.2.0-focus-2-setup.exe`。

**下一步必交付**:
1. 让用户在 clean 机器重装验证 Focus Sync 出现
2. 若失败:看 installer 日志 Stage 0 的 RESOLVEDAPPDATA 值 + Stage 6 register.ps1 退出码
3. 若成功:commit 当前 5 处文件改动 + 删除 `release/extension/bin/focus-sync-sidecar-canonical.exe` 临时文件(可保留)

**禁止事项**:
- 不要回滚到 round 19c 的 setup.exe
- 不要在没有 user verification 前 commit
- 不要把 `release/Sigma-File-Manager-2.2.0-focus-1-setup.exe` 当作有效产物(已被 19d 替代)

**加速模式**:无特殊豁免,默认标准模式。