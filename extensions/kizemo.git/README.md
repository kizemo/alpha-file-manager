# Git Tools（kizemo.git）

在 Alpha File Manager 里直接做 Git 操作的扩展：初始化、克隆、抓取、拉取、推送、同步，
浏览分支与提交，查看受控文件，并把仓库交给外部 IDE 打开。

## 能力清单

| 命令 | 作用 |
| --- | --- |
| `status` | 显示当前分支、上游跟踪分支、改动条目（按 M/A/D/R/C/U/?/! 分类） |
| `init` | 在当前目录执行 `git init` |
| `clone` | 输入仓库地址与目标目录后克隆，带进度回报 |
| `fetch` | `git fetch --all`，带进度回报 |
| `pull` | `git pull`，带进度回报 |
| `push` | `git push`，带进度回报 |
| `sync` | 先 `fetch --all` 再 `pull` |
| `branches` | 列出本地与远端分支，当前分支高亮 |
| `log` | 最近 50 条提交（短 hash / 作者 / 日期 / 标题） |
| `browse` | 列出 HEAD 的受控条目（类型 / 大小 / 路径） |
| `openInIde` | 把仓库目录交给配置的编辑器命令 |
| `configureIde` | 设置编辑器命令（默认 `code`） |

右键菜单另有两条目录项：`Git - Open in IDE`、`Git - Initialize repository here`。

## 运行时约束

这些是宿主扩展沙箱的硬约束，直接决定了实现方式：

1. **只能在 Web Worker 里跑**，没有 DOM、没有 `fetch`、没有定时器以外的后门。
   所有能力通过 `sigma` 命名空间访问。
2. **`sigma.shell.run` 只接受裸命令名**。宿主拒绝绝对路径的可执行文件
   （`C:\Program Files\...\git.exe` 会被判 Access denied），所以 git 一律用
   `git`，IDE 一律用命令名（`code` / `cursor`）。
3. **`sigma.shell.run` 没有 cwd 参数**，宿主不会调用 `current_dir()`，
   因此所有 git 调用都要带 `-C <目录>`。
4. **`sigma.shell.runWithProgress` 逐行回传输出**，适合放进
   `sigma.ui.withProgress` 的状态栏进度里。降级路径（宿主没有进度 API）也只会
   启动一次 git 进程，不会重复执行同一条命令。
5. **`sigma.context.getCurrentPath()` 可能返回 null**，
   所以扩展额外维护了 `onPathChange` 记下来的上一次路径，
   并在仍然拿不到时弹窗询问用户。
6. **扩展必须自带打包产物**。宿主把 `dist/index.js` 当 Blob 模块导入，
   相对 import 可以，裸包名 import 不行，所以用 esbuild 打成单文件 ESM。

## 目录结构

```
kizemo.git/
  package.json            清单：命令 id 用短 id，宿主会加 "kizemo.git." 前缀
  build.mjs               构建入口：生成文案回退表 + esbuild 打包
  src/index.js            扩展入口，导出具名 activate / deactivate
  src/generated-fallbacks.js  由 build.mjs 从 locales/zh-CN.json 生成
  locales/zh-CN.json      中文文案
  locales/en.json         英文文案（key 与中文完全一致）
  dist/index.js           打包产物（宿主真正加载的文件）
  icon.png                扩展图标
```

## 构建

```powershell
cd extensions/kizemo.git
node build.mjs
```

esbuild 来自仓库根的 `node_modules`。`build.mjs` 会先把 `locales/zh-CN.json`
写成 `src/generated-fallbacks.js`，再把 `src/index.js` 打成 `dist/index.js`。
这样即使宿主语言包合并失败，扩展界面仍然显示中文而不是裸的 key。

## 自检

```powershell
node ../../scripts/scan-sandbox-dynamic.cjs dist/index.js
```

期望输出 `VALID (0 violations / 19 patterns)`。
注意这个扫描器**不剥离注释**，源码与注释里都不能出现 19 条违禁词。

## 安装到本地宿主

扩展不会自动出现在市场里 —— 市场列表来自远端 `sfm-marketplace` 的
`registry.json`，本地放目录不生效。要在本地试用，把整个目录拷到

```
%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.git\
```

并在 `user-extensions.json` 里补一条：

```json
{
  "enabled": true,
  "isLocal": true,
  "localSourcePath": "<上面的目录绝对路径>",
  "installPendingDependencies": false,
  "autoUpdate": false
}
```

然后重装扩展（清单变更后必须重装才会生效）。
