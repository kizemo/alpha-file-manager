# PR 跟踪

## feat/custom-startup-path → PR #546
- PR: https://github.com/aleksey-hoffman/sigma-file-manager/pull/546
- Issue: aleksey-hoffman/sigma-file-manager#528
- 状态: **已开 PR,等维护者回复**
- 提交: c0fc7cb0 "feat(settings): allow custom directory as startup page"
- 改动统计: 22 文件,+169/-21
- 合并后动作:
  - 删本地 `feat/custom-startup-path` 标签(没单独建,直接 main 提交)
  - 跑月度同步脚本验证无冲突
  - 后续 Phase 1.8 任务等合并后再处理

## 上游 review 注意事项(给维护者参考)

- 已包含 i18n 同步(via vite-plugin-run 监听 en.json 自动生成 17 个 locale 文件)
- 已自测:vue-tsc 通过、lint 通过、相关 vitest 测试 25/25 通过
- 上游 CI 跑 `npm run check:win` 包含 E2E(webdriver),本地我**没有跑** E2E(需要 Tauri 编译 + Edge driver),维护者 CI 应该会跑