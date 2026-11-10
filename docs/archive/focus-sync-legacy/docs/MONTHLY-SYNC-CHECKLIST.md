# Sigma Fork 月度同步清单

每月底(建议每月 25 号左右)执行一次,约 30~60 分钟。

## 1. Fork 仓库同步

```bash
# 一键跑
bash /f/soft/00selfmade/filemanager/scripts/sync-upstream.sh
```

或手动:
```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git checkout main
git fetch upstream
git merge upstream/main --ff-only
git push origin main
```

## 2. 验证

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager

# 类型检查 + lint + 单元测试
npm run check

# (可选)完整 Tauri build 验证
# npm run tauri:build
```

## 3. 文档更新

- [ ] 看 sigma GitHub Releases,记录版本号到 FORK-WORKFLOW.md 的"当前 fork 版本"
- [ ] 检查 upstream 的 issue #499(文件树)和 #528(自定义启动目录)状态
- [ ] 如果某个特性上游已实现 → 评估是否迁移到上游方案 + 删除 FORK-KEEP-LIST.md 中的对应条目
- [ ] 如果 PR 已合并 → 更新 PR-LOG.md 状态为 `merged` + 考虑下次 fork 同步时清理

## 4. 插件仓库检查(`sigma-batch-rename`)

```bash
cd /f/soft/00selfmade/sigma-batch-rename

# 检查 API 兼容性
npm install
npm run build
npm test

# 若 @sigma-file-manager/api 升级,需要适配新 API
# 当前依赖: ^1.11.0
```

- [ ] `npm test` 通过
- [ ] 在最新 sigma dev 模式中手动测试一次
- [ ] 若改了 engine 或 UI,加新测试 + 更新 CHANGELOG.md

## 5. 本月同步产出

完成后简短记录到本文件末尾:

```markdown
## YYYY-MM 同步记录

- upstream 版本:vX.Y.Z
- 本地 commit 数 delta:+N
- 冲突:N 处
- 处理方式:...
- 下月计划:...
```

---

## 加速模式规则(项目级)

如果在 1h 内能完成的轻量同步(仅 fast-forward,无冲突),不需要走完整清单。
完整清单用于首次同步、含冲突同步、含上游特性合入的同步。
