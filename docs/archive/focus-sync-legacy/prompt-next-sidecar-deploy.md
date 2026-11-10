# prompt-next-sidecar-deploy.md

承接 `handoff-2026-09-29-sidecar-flash-focus-fix.md`(round 18)。

## 一句话状态

Round 18 改动全部完成并 build/test/smoke PASS,**新 binary 在 `C:\Temp\focus-sync-round18\spike.exe`,未覆盖 staging**。等用户授权 deployment + 真机验证。

## 当前分支/基线

- Working dir: `F:\soft\00selfmade\filemanager\`
- Branch: `main` (round-17 后所有改动未 commit)
- HEAD: `286287f docs(recovery): pivot Phase 2a from Tauri sidecar to Sigma plugin route`
- Toolchain: 必须 `cargo +1.85.0 ...`(默认 1.97.1 报 245 个依赖编译错)

## 必交付清单

### 高优(下一会话第一件事)

1. **问用户授权 deployment**(因 CLAUDE.md 安全红线"覆盖 staging = destructive 操作"):
   ```
   copy C:\Temp\focus-sync-round18\spike.exe \
        F:\soft\00selfmade\filemanager\release\extension\bin\focus-sync-sidecar.exe
   ```
   **只有用户 yes 后才执行。**

2. **重新打包 extension installer**(NSIS / Tauri 工具链,可能要询问):
   ```
   F:\soft\00selfmade\filemanager\release\extension-installer\kizemo.focus-sync-0.2.0-setup.exe
   F:\soft\00selfmade\filemanager\release\kizemo.focus-sync-0.2.0-setup.exe
   ```

3. **Sandbox 真机验证**(Sigma FM foreground,Edge 触发 Save dialog):
   - 观察 filename 输入框是否还闪动 → 期望稳定在 Sigma FM 当前目录
   - 用户能否 Alt-Tab 到其他软件 → 期望不被抢焦点
   - TCP `set_path` 是否正常触发写入

### 中优(技术债清理,可推迟)

4. Phase 2 spec 补 `poll_path_changes` 实现(spec 写了,代码缺)
5. `state.last_known_path` 改名为 `last_read_path`
6. `app_whitelist` CLI flag 验证(round 18 没改)
7. 加 integration test:mock sender → 模拟 "Sigma 切路径 → IPC set_path → spike 写 dialog"

## 禁止事项

- ❌ 不要恢复 `src/fg_bypass.rs`(已删,git history 保留 attribution)
- ❌ 不要在 main.rs select! 的 dialog event 分支加回 `write_path`(round-17 闪动复发)
- ❌ 不要用 default stable 1.97.1 cargo(必须 `+1.85.0`)
- ❌ 不要碰 `release/extension/bin/` 下的 round-17 binary(等用户授权覆盖)

## 加速模式规则

- 适用:无
- 默认按 superpowers 流程
- 涉及 deployment/destructive 操作必须先问用户

## 必读文件顺序

1. `handoff-2026-09-29-sidecar-flash-focus-fix.md`(本轮 round 18)
2. `handoff-2026-09-29-filename-flash.md`(round 17,被 round 18 推翻)
3. `sigma-listary-spike/src/uia_event.rs`(round 18 dedup + DialogEvent)
4. `sigma-listary-spike/src/main.rs:96-160`(round 18 select! match)
5. `sigma-listary-spike/src/writer.rs`(round 18 撤 fg_bypass wrapper)
6. `sigma-listary-spike/README.md`(round 18 attribution)

## 任务列表(TaskList ID 续用)

- #1 writer.rs 撤 fg_bypass ✅
- #2 写 handoff + prompt-next(本任务进行中)
- #3 cargo check + cargo test ✅
- #4 部署到 sandbox(完成 staging 至 C:\Temp\,未覆盖 release/)
- #5 uia_event.rs 去重 push ✅
- #6 main.rs None 分支清理 registry ✅
- #7 main.rs 删除 write_path 块 ✅
- #8 cargo +1.85.0 check ✅
- #9 DialogInfo enum 化 ✅
- #10 删 fg_bypass.rs + lib.rs ✅
- #11 uia_event.rs regression test ✅