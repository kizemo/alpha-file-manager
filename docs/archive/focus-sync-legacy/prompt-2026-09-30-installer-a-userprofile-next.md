# Prompt — verify round 19d fix v2 + commit

承接 handoff-2026-09-30-installer-a-userprofile-fix.md。

**当前状态**:round 19d 第二次修复 (PowerShell `$env:USERPROFILE` fallback) 已 build + silent install 验证成功。`release/Sigma-File-Manager-2.2.0-focus-6-setup.exe` (SHA1=6F7938EB6159513097E6FBB20985B664FC9CF386) 是当前 working build。

**下一步必交付**(用户在等):
1. 让用户重装 focus-6 → 验证 Installed tab 出现 Focus Sync
2. **清理 release/ 目录**:删除 focus-2/3/4/5 中间产物(只留 focus-1 旧,focus-6 current)
3. commit 4 处源文件改动(hooks.nsh + tauri.conf.json + register.ps1 + resolve_user_appdata.ps1)
4. 更新 memory:加 NSIS `$APPDATA`/`$USERPROFILE` gotcha 到 sigma_fm_extension_registration.md

**禁止事项**:
- 不要回到 focus-2/4/5 那几个 BUGGY build
- focus-3 是中间调试版(有 FileWrite debug 输出),commit 前要清掉 hooks.nsh 的 marker 写文件代码

**必读顺序**:
1. 本 prompt 指向的 handoff(本 round 全过程)
2. `handoff-2026-09-30-installer-a-wmi-fix.md`(前一轮尝试)
3. `memory/sigma_fm_extension_registration.md`(注册机制)

**加速模式**:无特殊豁免。