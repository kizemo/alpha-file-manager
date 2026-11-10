# Fork 维护工作流

## 仓库布局

```
F:\soft\00selfmade\filemanager\                    ← 本目录(meta 仓库,放 workflow 文档)
├── docs\
│   ├── FORK-WORKFLOW.md                            ← 本文件
│   ├── FORK-KEEP-LIST.md                           ← fork 永久保留改动清单
│   ├── PR-LOG.md                                   ← PR 状态跟踪
│   ├── STARTUP-FLOW-NOTES.md                       ← 启动路径数据流笔记
│   └── superpowers\plans\                          ← 实施计划文档
├── sigma-file-manager\                             ← fork 仓库(Sigma File Manager)
│   ├── .git\
│   ├── src\                                       ← Vue 3 前端
│   ├── src-tauri\                                 ← Rust 后端
│   └── ...
├── sigma-batch-rename\                             ← 独立插件仓库(将来)
└── scripts\
    └── sync-upstream.sh                            ← 月度同步脚本
```

## Fork 仓库远端

| 名称 | URL | 角色 |
|---|---|---|
| `origin` | `https://github.com/kizemo/alpha-file-manager.git` | 推送到自己的 fork |
| `upstream` | `https://github.com/aleksey-hoffman/sigma-file-manager.git` | 同步上游 Aleksey Hoffman 的官方仓库 |

## Git 身份

依赖 `C:/Users/Duanyi/.gitconfig` 全局配置:
- `user.name = kizemo`
- `user.email = kizemo@users.noreply.github.com`

**不要修改全局配置**(会影响所有项目)。若需要为某个仓库覆盖身份,用 `git config --local`。

## 月度同步步骤

```bash
# 1. 进入 fork 仓库
cd /f/soft/00selfmade/filemanager/sigma-file-manager

# 2. 确保在 main 分支
git checkout main

# 3. 抓取上游最新
git fetch upstream

# 4. 同步(预期 fast-forward)
git merge upstream/main --ff-only

# 5. 推到 origin
git push origin main
```

或者一键跑脚本:`/f/soft/00selfmade/filemanager/scripts/sync-upstream.sh`

## 冲突解决原则

1. **永远 rebase/merge 到 main,不要在特性分支上直接 merge upstream**
2. 冲突优先用 upstream 版本(因为本地 fork 改动应该走 PR,不该长期存在)
3. 真正需要保留的本地改动用 `feat/*` 分支承载,加 `[fork-keep]` 注释,并在 `docs/FORK-KEEP-LIST.md` 登记

## 提交规范

- Conventional Commits:`feat:` / `fix:` / `chore:` / `docs:` / `test:` / `refactor:`
- 涉及 fork 长期保留的改动,commit message 加 `[fork-keep]` 后缀

## PR 流程

1. 从最新 upstream/main 拉特性分支:`git checkout -b feat/<name> upstream/main`
2. 改完先跑 `npm run check` + `npm run tauri:build`
3. push 到 origin
4. `gh pr create --repo aleksey-hoffman/sigma-file-manager`,描述里引用对应 issue
5. 把 PR 链接填到 `docs/PR-LOG.md`

## 验证清单(每次完成改动后)

- [ ] `npm run check` 通过
- [ ] `npm run tauri:build` 通过
- [ ] 改动文件加测试(单元测试优先,E2E 仅关键路径)
- [ ] commit message 符合 Conventional Commits
- [ ] 没有引入大体积依赖