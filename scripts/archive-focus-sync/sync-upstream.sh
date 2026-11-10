#!/usr/bin/env bash
# Sigma File Manager fork — 月度同步脚本
#
# 跑这个脚本同步 fork 与上游 aleksey-hoffman/sigma-file-manager。
# 预期是 fast-forward merge;若有本地 main commit 阻塞会 fail,提示你去清理。
#
# Usage: bash scripts/sync-upstream.sh

set -euo pipefail

FORK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/sigma-file-manager"

if [ ! -d "$FORK_DIR/.git" ]; then
  echo "ERROR: $FORK_DIR 不是 git 仓库。请确认 fork 仓库已克隆。"
  exit 1
fi

cd "$FORK_DIR"

echo "==> 当前分支: $(git symbolic-ref --short HEAD)"
current_branch="$(git symbolic-ref --short HEAD)"

if [ "$current_branch" != "main" ]; then
  echo "ERROR: 必须在 main 分支上跑(当前: $current_branch)。"
  echo "       先 git checkout main 再重跑。"
  exit 1
fi

echo "==> 抓取上游"
git fetch upstream

echo "==> 检查 fast-forward merge"
if ! git merge upstream/main --ff-only; then
  cat <<EOF

!!!  Fast-forward merge 失败。可能原因:
    1. 你的 main 分支有本地 commit 没推到 origin(说明有 feat/* 分支没合并进来)
    2. 远端拒绝了 push(检查 git remote -v)

    建议:
    - 检查本地 main 的相对 upstream 的 commit: git log upstream/main..main --oneline
    - 若有本地 commit,把它们 cherry-pick 到对应 feat/* 分支,再 reset main 到 upstream
    - 然后重跑本脚本

EOF
  exit 2
fi

echo "==> 推送到 origin"
git push origin main

echo ""
echo "==> Done. 下一步检查项:"
echo "    1. 上游 issue #499/#528 等是否有新进展?更新 docs/PR-LOG.md"
echo "    2. FORK-KEEP-LIST.md 中的文件是否需要适配?"
echo "    3. 跑 cd $FORK_DIR && npm run check 确认所有改动通过 typecheck + lint + tests"
echo "    4. 在 sigma 中手动测试 fork 特有功能(目前:自定义启动目录)"
