# Sigma File Manager Fork + 插件 整体实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 Fork Sigma File Manager 的基础上,完成 3 个用户需求(自定义启动目录、文件树视图、批量改名插件),并建立长期维护工作流。

**Architecture:**
- **Fork**(`sigma-file-manager` 仓库 fork 版):承载需要改内核的特性(启动目录、文件树)
- **独立插件仓库**(`sigma-batch-rename`):批量改名作为 API extension,完全独立发布
- **上游 PR 优先**:可被合并的特性(启动目录)直接走 PR,合进主线后零成本享受
- **月度同步**:建立 fork 与 upstream 的低摩擦同步流程

**Tech Stack:**
- Tauri 2.x + Rust 后端
- Vue 3 + TypeScript 前端
- Pinia 状态管理
- Vitest(单元测试)+ WebdriverIO(E2E 测试)
- Sigma 扩展 API `@sigma-file-manager/api` v1.5+

## Global Constraints

- 仓库根目录:`F:\soft\00selfmade\filemanager`(windows),`/f/soft/00selfmade/filemanager`(bash)
- 主分支:`main`(跟随 upstream)
- Fork 特性分支命名:`feat/<short-name>`
- 插件仓库命名:`F:\soft\00selfmade\sigma-batch-rename`
- Node 版本:见 `package.json > engines.node`(当前要求 Node ≥ 20)
- Rust 版本:见 `.github/CONTRIBUTING.md` 推荐的 stable
- 提交规范:Conventional Commits(`feat:`, `fix:`, `chore:`, `docs:`)
- 许可证:GPL-3.0-or-later(fork 修改必须保持兼容)
- 平台目标:Windows 11 优先(用户的实际使用环境)
- **绝对不动**:Tauri 的 capability 配置、安全相关 Rust 代码、签名流程

---

## Phase 0:Fork 基础设施(地基)

### Task 0.1:Fork 仓库 + 本地克隆

**Files:**
- Create: `F:\soft\00selfmade\filemanager\sigma-file-manager\`(clone 目录)
- Create: `F:\soft\00selfmade\filemanager\docs\FORK-WORKFLOW.md`(长期维护手册)

**为什么独立目录**:用户的 filemanager 目录目前是空的,以后还会有插件目录、文档等。

- [ ] **Step 1:在 GitHub 上 fork**

打开 https://github.com/aleksey-hoffman/sigma-file-manager 点 Fork,目标账号用用户自己的 GitHub 账号。假设 fork URL 是 `https://github.com/<your-name>/sigma-file-manager`。

- [ ] **Step 2:克隆 fork 到本地**

```bash
cd /f/soft/00selfmade/filemanager
git clone https://github.com/<your-name>/sigma-file-manager.git
cd sigma-file-manager
```

- [ ] **Step 3:配置 upstream 远程**

```bash
git remote add upstream https://github.com/aleksey-hoffman/sigma-file-manager.git
git remote -v
# 应该看到:
# origin    https://github.com/<your-name>/sigma-file-manager.git (fetch)
# origin    https://github.com/<your-name>/sigma-file-manager.git (push)
# upstream  https://github.com/aleksey-hoffman/sigma-file-manager.git (fetch)
# upstream  https://github.com/aleksey-hoffman/sigma-file-manager.git (push)
```

- [ ] **Step 4:首次同步验证**

```bash
git fetch upstream
git checkout main
git merge upstream/main --ff-only
# 第一次 fork 应该 fast-forward,无冲突
git log --oneline -5
git push origin main
```

- [ ] **Step 5:本地安装依赖并验证 dev 模式启动**

```bash
npm install
npm run tauri:dev
```

预期:应用窗口弹出,能浏览本地文件。关闭它继续。

- [ ] **Step 6:写 FORK-WORKFLOW.md**

文件路径:`F:\soft\00selfmade\filemanager\docs\FORK-WORKFLOW.md`

内容(简要版):
```markdown
# Fork 维护工作流

## 仓库
- Origin: https://github.com/<your-name>/sigma-file-manager
- Upstream: https://github.com/aleksey-hoffman/sigma-file-manager

## 月度同步步骤
\`\`\`bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git checkout main
git fetch upstream
git merge upstream/main        # 有冲突时见下节
git push origin main
\`\`\`

## 冲突解决原则
1. **永远 rebase/merge 到 main,不要在特性分支上直接 merge upstream**
2. 冲突优先用 upstream 版本(因为本地 fork 改动应该走 PR,不该长期存在)
3. 真正需要保留的本地改动用 `feat/*` 分支承载,加 `FORK-MODIFICATION:` 注释

## 提交规范
- Conventional Commits
- 涉及 fork 本地保留的改动,标题加 `[fork-keep]` 前缀

## PR 流程
1. 从最新 upstream/main 拉特性分支
2. 改完先在本分支跑 `npm run check` + `npm run tauri:build`
3. push 到 origin,开 PR 到 aleksey-hoffman/sigma-file-manager
4. PR 描述引用对应 issue(#499 / #528 等)
```

- [ ] **Step 7:首次同步提交**

```bash
cd /f/soft/00selfmade/filemanager
git add docs/FORK-WORKFLOW.md
git commit -m "docs: initial fork workflow documentation"
```

**Task 0.1 完成标志**:能 `git fetch upstream && git merge upstream/main` 无冲突,且 `npm run tauri:dev` 启动成功。

---

### Task 0.2:分支策略与 PR 模板

**Files:**
- Create: `.github/PULL_REQUEST_TEMPLATE.md`(在 fork 仓库内)

- [ ] **Step 1:创建 PR 模板**

文件路径:`F:\soft\00selfmade\filemanager\sigma-file-manager\.github\PULL_REQUEST_TEMPLATE.md`

```markdown
## 关联 Issue
- <!-- 引用上游 issue,如 Fixes #528 -->

## 改动概述
<!-- 1-3 句话 -->

## 测试证据
- [ ] `npm run check` 通过
- [ ] `npm run tauri:build` 通过
- [ ] 手动测试: <!-- 描述步骤 -->

## 上游兼容性
- [ ] 不引入新的运行时依赖
- [ ] 不修改 Rust 后端的能力/权限配置
- [ ] 改动可独立 revert 不影响其他特性

## 截图(如适用)
<!-- 拖入截图 -->
```

- [ ] **Step 2:提交**

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git add .github/PULL_REQUEST_TEMPLATE.md
git commit -m "docs: add PR template"
git push origin main
```

---

## Phase 1:快速胜利 — 自定义启动目录(可 PR 上游)

**对应上游 Issue #528**,维护者已规划 v2.3.0,**合并概率最高**。先做这个是为了验证整个 fork → PR 流程。

### Task 1.1:理解现有 startup 数据流

**Files:**
- Read: `src/types/user-settings.ts`(StartupPage 类型)
- Read: `src/modules/settings/ui/categories/general/startup-page.vue`(设置 UI)
- Read: `src/stores/storage/utils/startup-storage-bootstrap.ts`(启动加载逻辑)
- Read: `src-tauri/src/startup_storage_bootstrap.rs`(Rust 侧启动读取)

- [ ] **Step 1:读 user-settings.ts 找到 StartupPage 定义**

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
grep -n "StartupPage" src/types/user-settings.ts
```

记录当前定义的类型。预期是 `'last' | 'home' | 'dashboard' | 'navigator'`。

- [ ] **Step 2:读 startup-page.vue 找 pageOptions 数组**

```bash
grep -n "pageOptions\|startupPage" src/modules/settings/ui/categories/general/startup-page.vue
```

记录现有下拉选项的 i18n key。

- [ ] **Step 3:跟读启动路径消费点**

```bash
grep -rn "startupPage\|StartupPage" src/ src-tauri/src/ --include="*.ts" --include="*.vue" --include="*.rs"
```

预期找到所有读取 `userSettings.startupPage` 的位置。这是后续修改的影响面。

- [ ] **Step 4:写笔记**

文件路径:`F:\soft\00selfmade\filemanager\docs\STARTUP-FLOW-NOTES.md`

内容:列出 StartupPage 类型定义位置、当前支持值、所有消费点、UI 选项数组位置。

- [ ] **Step 5:提交笔记**

```bash
cd /f/soft/00selfmade/filemanager
git add docs/STARTUP-FLOW-NOTES.md
git commit -m "docs: notes on startup page data flow"
```

### Task 1.2:写类型扩展的失败测试

**Files:**
- Modify: `src/types/user-settings.ts`(暂只加注释 + 占位)
- Create: `src/types/__tests__/user-settings-startup-page.test.ts`

- [ ] **Step 1:创建测试文件**

文件路径:`F:\soft\00selfmade\filemanager\sigma-file-manager\src\types\__tests__\user-settings-startup-page.test.ts`

```typescript
import { describe, it, expect } from 'vitest';
import type { StartupPage } from '@/types/user-settings';

describe('StartupPage type', () => {
  it('includes the new custom-path option', () => {
    // TypeScript 编译期测试:这些字面量必须可赋值给 StartupPage
    const _validValues: StartupPage[] = ['last', 'home', 'dashboard', 'navigator', 'customPath'];

    // 运行时断言:枚举全集
    const expected = new Set<StartupPage>(['last', 'home', 'dashboard', 'navigator', 'customPath']);
    const actual = new Set(_validValues);

    expect(actual).toEqual(expected);
  });

  it('rejects unknown values at compile time', () => {
    // @ts-expect-error - 'unknownStartup' is not a valid StartupPage
    const _invalid: StartupPage = 'unknownStartup';
    void _invalid;
  });
});
```

- [ ] **Step 2:运行测试,确认 RED**

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
npx vitest run src/types/__tests__/user-settings-startup-page.test.ts
```

预期:FAIL,提示 `'customPath'` 不是合法的 StartupPage。

- [ ] **Step 3:提交 RED 测试**

```bash
git add src/types/__tests__/user-settings-startup-page.test.ts
git commit -m "test(user-settings): add failing test for customPath StartupPage"
```

### Task 1.3:扩展 StartupPage 类型

**Files:**
- Modify: `src/types/user-settings.ts`(StartupPage 联合类型)

- [ ] **Step 1:定位类型定义**

```bash
grep -n "StartupPage" src/types/user-settings.ts
```

预期看到:
```typescript
export type StartupPage = 'last' | 'home' | 'dashboard' | 'navigator';
```

- [ ] **Step 2:加 customPath 选项**

修改为:
```typescript
export type StartupPage = 'last' | 'home' | 'dashboard' | 'navigator' | 'customPath';
```

- [ ] **Step 3:跑测试,确认 GREEN**

```bash
npx vitest run src/types/__tests__/user-settings-startup-page.test.ts
```

预期:PASS。

- [ ] **Step 4:跑类型检查**

```bash
npm run check 2>&1 | head -50
# 或单独 tsc:
npx tsc --noEmit -p tsconfig.app.json
```

预期:无报错(若其他地方有穷尽 StartupPage 的 switch,后面 Task 1.4 会暴露)。

- [ ] **Step 5:提交**

```bash
git add src/types/user-settings.ts
git commit -m "feat(user-settings): add customPath StartupPage variant"
```

### Task 1.4:扩展启动路径存储 schema

**Files:**
- Modify: `src-shared/user-storage-files.json`(若存 schema)
- Modify: `src-tauri/src/startup_storage_bootstrap.rs`(读 custom path)
- Modify: `src/stores/storage/utils/startup-storage-bootstrap.ts`(TS 侧消费)

- [ ] **Step 1:读 Rust bootstrap 现状**

```bash
cat src-tauri/src/startup_storage_bootstrap.rs
```

确认现有字段。

- [ ] **Step 2:加 customStartupPath 字段**

参考现有字段(如 `startupPage` 附近的字段),添加:
```rust
#[serde(default)]
pub startup_page: Option<String>,

// ↓↓↓ 新增 ↓↓↓
#[serde(default)]
pub custom_startup_path: Option<String>,
// ↑↑↑ 新增 ↑↑↑
```

- [ ] **Step 3:同步 TS 类型**

在 `src/stores/storage/utils/startup-storage-bootstrap.ts` 对应位置加:
```typescript
interface StartupStorageData {
  startupPage?: string;
  // ↓↓↓ 新增 ↓↓↓
  customStartupPath?: string;
  // ↑↑↑ 新增 ↑↑↑
}
```

- [ ] **Step 4:跑类型检查**

```bash
npx tsc --noEmit -p tsconfig.app.json
npx cargo check --manifest-path src-tauri/Cargo.toml
```

预期:无报错。

- [ ] **Step 5:提交**

```bash
git add src-tauri/src/startup_storage_bootstrap.rs src/stores/storage/utils/startup-storage-bootstrap.ts
git commit -m "feat(storage): add customStartupPath field"
```

### Task 1.5:扩展启动 page 设置 UI

**Files:**
- Modify: `src/modules/settings/ui/categories/general/startup-page.vue`

- [ ] **Step 1:读现状确认下拉项结构**

```bash
grep -n "pageOptions\|t('pages\|SelectItem" src/modules/settings/ui/categories/general/startup-page.vue | head -30
```

- [ ] **Step 2:加 customPath 选项**

在 `pageOptions` 数组末尾追加(注意先备份原文):
```typescript
const pageOptions = [
  { name: t('settings.general.startupPage.last'), value: 'last' },
  { name: t('settings.general.startupPage.home'), value: 'home' },
  { name: t('settings.general.startupPage.dashboard'), value: 'dashboard' },
  { name: t('settings.general.startupPage.navigator'), value: 'navigator' },
  // ↓↓↓ 新增 ↓↓↓
  { name: t('settings.general.startupPage.customPath'), value: 'customPath' },
  // ↑↑↑ 新增 ↑↑↑
];
```

- [ ] **Step 3:加 customPath 路径选择 UI**

在 `</Select>` 后、`<style scoped>` 前插入:
```vue
<div
  v-if="selectedPage?.value === 'customPath'"
  class="startup-page-custom-path"
>
  <Input
    v-model="customPathValue"
    :placeholder="t('settings.general.startupPage.customPathPlaceholder')"
  />
  <Button
    variant="secondary"
    @click="onPickCustomPath"
  >
    {{ t('settings.general.startupPage.browse') }}
  </Button>
</div>
```

并在 `<script setup>` 加:
```typescript
import { ref, watch } from 'vue';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { dialog } from '@/modules/native'; // 或 '@tauri-apps/plugin-dialog',按项目实际

const customPathValue = ref(userSettingsStore.userSettings.customStartupPath ?? '');

watch(customPathValue, async (value) => {
  await userSettingsStore.set('customStartupPath', value);
});

async function onPickCustomPath() {
  // 调用 Tauri 的打开目录对话框,具体 API 按 sigma 当前用法
  const picked = await dialog.open({ directory: true, multiple: false });
  if (typeof picked === 'string' && picked) {
    customPathValue.value = picked;
  }
}
```

> **注意**:这里的具体 API 调用(`dialog.open` 的导入路径)需要在实施时查 sigma 当前用的方式 — 这是计划里少数需要"实施时确认"的地方,会在执行任务时核实。

- [ ] **Step 4:加 i18n**

在 `src/locales/en/translation.json`(或对应的 .ts 文件,看 sigma 当前格式)追加:
```json
"customPath": "Custom path...",
"customPathPlaceholder": "Choose any directory to open at startup",
"browse": "Browse..."
```

中文字典同步加。

- [ ] **Step 5:跑 check**

```bash
npm run check
```

预期:通过。

- [ ] **Step 6:本地验证**

```bash
npm run tauri:dev
```

打开 Settings → General → Startup page,确认能看到 "Custom path..." 选项,选完能弹文件夹选择。

- [ ] **Step 7:提交**

```bash
git add src/modules/settings/ui/categories/general/startup-page.vue src/locales/
git commit -m "feat(settings): add customPath option to startup page"
```

### Task 1.6:让启动消费点读 customStartupPath

**Files:**
- Modify: `src/stores/storage/utils/startup-storage-bootstrap.ts`(或真正消费 startupPage 的 store)

- [ ] **Step 1:grep 所有消费点**

```bash
grep -rn "startupPage" src/ --include="*.ts" --include="*.vue"
```

- [ ] **Step 2:在每个消费点加 case**

典型模式(在 switch 或 if 链中):
```typescript
// 原有
if (page === 'home') navigateToHome();
else if (page === 'dashboard') navigateToDashboard();
else if (page === 'navigator') navigateToNavigator();
else /* last */

// ↓↓↓ 新增 ↓↓↓
else if (page === 'customPath') {
  const customPath = userSettingsStore.userSettings.customStartupPath;
  if (customPath) {
    openNavigatorPath(router, customPath);
  } else {
    // 没填路径则降级到 home
    navigateToHome();
  }
}
// ↑↑↑ 新增 ↑↑↑
```

> **重要**:实际消费点的位置和写法需要 grep 结果出来后调整,这里是模板。

- [ ] **Step 3:本地验证**

重启 dev 模式,设置 startup page 为 customPath 并填路径,重启 app,确认能直接打开指定目录。

- [ ] **Step 4:写 E2E 测试**

文件路径:`e2e-webdriver/test/specs/startup-custom-path.e2e.js`

```js
describe('Startup custom path', () => {
  it('opens the configured custom path on launch', async () => {
    // 1. 打开设置
    await openSettings();
    // 2. 选 customPath 并设值
    await selectStartupPage('Custom path...');
    await setCustomPath('C:\\Users\\Public');
    // 3. 重启 app(手动或通过 test helper)
    await restartApp();
    // 4. 验证 navigator 当前路径
    const path = await getCurrentNavigatorPath();
    expect(path).toBe('C:\\Users\\Public');
  });
});
```

> **占位**:具体 webdriver API 要在实施时按 sigma 现有 e2e 模式调整。

- [ ] **Step 5:提交**

```bash
git add .
git commit -m "feat(navigator): honor customStartupPath when startup page is customPath"
git commit -m "test(e2e): add startup-custom-path test" --allow-empty
```

### Task 1.7:开 PR 上游

**Files:** N/A(纯流程)

- [ ] **Step 1:从最新 upstream 拉特性分支**

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git fetch upstream
git checkout -b feat/custom-startup-path upstream/main
```

- [ ] **Step 2:把本地 commit cherry-pick 过来**

```bash
# 查看本地 main 相对于 upstream/main 的 commit
git log upstream/main..main --oneline
# 把相关 commit 一个个 cherry-pick
git cherry-pick <commit-sha-1> <commit-sha-2> ...
```

如有冲突,优先保留 fork 改动(因为这是我们要 PR 的内容)。

- [ ] **Step 3:在特性分支跑完整 check**

```bash
npm run check
npm run tauri:build
```

预期:都通过。

- [ ] **Step 4:push 并开 PR**

```bash
git push origin feat/custom-startup-path
gh pr create --repo aleksey-hoffman/sigma-file-manager \
  --title "feat(settings): allow custom directory as startup page" \
  --body "Fixes #528\n\nImplemented as: extends StartupPage type with 'customPath', adds a directory picker in Settings > General > Startup page, and resolves it on app launch.\n\nTested on Windows 11, builds clean with \`npm run tauri:build\`."
```

- [ ] **Step 5:记录 PR 链接**

文件路径:`F:\soft\00selfmade\filemanager\docs\PR-LOG.md`

```markdown
# PR 跟踪

## feat/custom-startup-path
- PR: <!-- 填链接 -->
- Issue: #528
- 状态: <!-- draft / review / merged / closed -->
- 合并后动作: 删除本地 `feat/custom-startup-path` 分支
```

**Task 1.7 完成标志**:PR 链接填入 PR-LOG.md,等维护者回复。

### Task 1.8:PR 合并后的清理

**Files:** N/A

- [ ] **Step 1:merge main**

PR 合并后:
```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git checkout main
git fetch upstream
git merge upstream/main --ff-only
git push origin main
```

- [ ] **Step 2:删特性分支**

```bash
git branch -d feat/custom-startup-path
git push origin --delete feat/custom-startup-path
```

- [ ] **Step 3:更新 PR-LOG.md**

把状态改为 `merged` 并记录合并 commit。

---

## Phase 2:文件树视图(Fork 内长期特性)

**对应上游 Issue #499**(维护者误关闭,实际未实现)。本阶段改动**留在 fork**,即使将来同步冲突也优先保留。

### Task 2.1:设计文件树数据模型

**Files:**
- Create: `src/modules/navigator/types/file-tree.ts`
- Create: `src/modules/navigator/types/__tests__/file-tree.test.ts`

- [ ] **Step 1:写类型测试(失败)**

文件路径:`src/modules/navigator/types/__tests__/file-tree.test.ts`

```typescript
import { describe, it, expect } from 'vitest';
import type { FileTreeNode, FileTreeFlatRow } from '@/modules/navigator/types/file-tree';

describe('FileTreeNode', () => {
  it('represents an expandable directory', () => {
    const node: FileTreeNode = {
      path: 'C:/work',
      name: 'work',
      isDirectory: true,
      isExpanded: false,
      isLoaded: false,
      depth: 0,
      children: null,
    };
    expect(node.isDirectory).toBe(true);
    expect(node.children).toBeNull();
  });

  it('represents a leaf file', () => {
    const node: FileTreeNode = {
      path: 'C:/work/readme.md',
      name: 'readme.md',
      isDirectory: false,
      isExpanded: false,
      isLoaded: true,
      depth: 1,
      children: null,
    };
    expect(node.isDirectory).toBe(false);
  });
});

describe('FileTreeFlatRow', () => {
  it('flattens a node tree into a list view', () => {
    const root: FileTreeNode = {
      path: 'C:/work', name: 'work', isDirectory: true,
      isExpanded: true, isLoaded: true, depth: 0,
      children: [
        { path: 'C:/work/a.md', name: 'a.md', isDirectory: false, isExpanded: false, isLoaded: true, depth: 1, children: null }
      ]
    };
    const rows: FileTreeFlatRow[] = [
      { path: root.path, name: root.name, depth: 0, isDirectory: true, hasChildren: true, isExpanded: true },
      { path: root.children![0].path, name: root.children![0].name, depth: 1, isDirectory: false, hasChildren: false, isExpanded: false },
    ];
    expect(rows).toHaveLength(2);
  });
});
```

- [ ] **Step 2:跑测试确认 RED**

```bash
npx vitest run src/modules/navigator/types/__tests__/file-tree.test.ts
```

预期:FAIL(找不到模块)。

- [ ] **Step 3:写最小实现**

文件路径:`src/modules/navigator/types/file-tree.ts`

```typescript
/** 树节点(可展开目录或叶子文件) */
export interface FileTreeNode {
  path: string;
  name: string;
  isDirectory: boolean;
  isExpanded: boolean;
  isLoaded: boolean;
  depth: number;
  children: FileTreeNode[] | null;
}

/** 扁平化的一行(用于虚拟滚动) */
export interface FileTreeFlatRow {
  path: string;
  name: string;
  depth: number;
  isDirectory: boolean;
  hasChildren: boolean;
  isExpanded: boolean;
}
```

- [ ] **Step 4:跑测试确认 GREEN**

```bash
npx vitest run src/modules/navigator/types/__tests__/file-tree.test.ts
```

预期:PASS。

- [ ] **Step 5:提交**

```bash
git add src/modules/navigator/types/
git commit -m "feat(navigator): add FileTreeNode and FileTreeFlatRow types"
```

### Task 2.2:文件树 composable(展开/折叠/懒加载)

**Files:**
- Create: `src/modules/navigator/composables/use-file-tree.ts`
- Create: `src/modules/navigator/composables/__tests__/use-file-tree.test.ts`

- [ ] **Step 1:写 composable 测试**

文件路径:`src/modules/navigator/composables/__tests__/use-file-tree.test.ts`

```typescript
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useFileTree } from '@/modules/navigator/composables/use-file-tree';

describe('useFileTree', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('initializes a tree rooted at a single path', () => {
    const tree = useFileTree({ rootPaths: ['C:/work'] });
    expect(tree.rows.value).toHaveLength(1);
    expect(tree.rows.value[0].path).toBe('C:/work');
    expect(tree.rows.value[0].isExpanded).toBe(false);
  });

  it('toggles expansion without loading children when collapsing', async () => {
    const tree = useFileTree({ rootPaths: ['C:/work'] });
    await tree.toggle(tree.rows.value[0].path);
    expect(tree.rows.value[0].isExpanded).toBe(true);
    await tree.toggle(tree.rows.value[0].path);
    expect(tree.rows.value[0].isExpanded).toBe(false);
    expect(tree.rows.value[0].hasChildren).toBe(true);
  });

  it('loads children lazily on first expand', async () => {
    const readDir = vi.fn().mockResolvedValue([
      { path: 'C:/work/a.md', name: 'a.md', isDirectory: false, size: 0, modifiedAt: 0 },
      { path: 'C:/work/sub', name: 'sub', isDirectory: true, size: 0, modifiedAt: 0 },
    ]);

    const tree = useFileTree({ rootPaths: ['C:/work'], deps: { readDir } });
    await tree.toggle('C:/work');

    expect(readDir).toHaveBeenCalledOnce();
    expect(tree.rows.value).toHaveLength(3); // root + 2 children
    expect(tree.rows.value[1].depth).toBe(1);
  });
});
```

- [ ] **Step 2:跑测试确认 RED**

```bash
npx vitest run src/modules/navigator/composables/__tests__/use-file-tree.test.ts
```

- [ ] **Step 3:写最小实现**

文件路径:`src/modules/navigator/composables/use-file-tree.ts`

```typescript
import { ref, computed, type Ref, type ComputedRef } from 'vue';
import type { FileTreeNode, FileTreeFlatRow } from '../types/file-tree';

export interface UseFileTreeDeps {
  readDir: (path: string) => Promise<Array<{
    path: string;
    name: string;
    isDirectory: boolean;
    size?: number;
    modifiedAt?: number;
  }>>;
}

export interface UseFileTreeOptions {
  rootPaths: string[];
  deps?: UseFileTreeDeps;
}

export interface UseFileTreeApi {
  rows: ComputedRef<FileTreeFlatRow[]>;
  toggle: (path: string) => Promise<void>;
  refresh: (path: string) => Promise<void>;
  expand: (path: string) => Promise<void>;
}

export function useFileTree(options: UseFileTreeOptions): UseFileTreeApi {
  const deps = options.deps ?? defaultDeps();
  const nodes = ref<FileTreeNode[]>(buildRoots(options.rootPaths));

  async function loadChildren(node: FileTreeNode): Promise<void> {
    if (!node.isDirectory || node.isLoaded) return;
    const entries = await deps.readDir(node.path);
    node.children = entries.map(entry => ({
      path: entry.path,
      name: entry.name,
      isDirectory: entry.isDirectory,
      isExpanded: false,
      isLoaded: !entry.isDirectory,
      depth: node.depth + 1,
      children: entry.isDirectory ? null : null,
    }));
    node.isLoaded = true;
  }

  async function findAndMutate(path: string, mutator: (n: FileTreeNode) => Promise<void> | void): Promise<void> {
    async function walk(list: FileTreeNode[]): Promise<boolean> {
      for (const n of list) {
        if (n.path === path) { await mutator(n); return true; }
        if (n.children && (await walk(n.children))) return true;
      }
      return false;
    }
    await walk(nodes.value);
  }

  async function toggle(path: string): Promise<void> {
    await findAndMutate(path, async (n) => {
      if (!n.isDirectory) return;
      if (n.isExpanded) {
        n.isExpanded = false;
      } else {
        await loadChildren(n);
        n.isExpanded = true;
      }
    });
  }

  async function expand(path: string): Promise<void> {
    await findAndMutate(path, async (n) => {
      if (!n.isDirectory) return;
      await loadChildren(n);
      n.isExpanded = true;
    });
  }

  async function refresh(path: string): Promise<void> {
    await findAndMutate(path, async (n) => {
      if (!n.isDirectory) return;
      n.isLoaded = false;
      n.children = null;
      n.isExpanded = false;
    });
  }

  const rows = computed<FileTreeFlatRow[]>(() => {
    const out: FileTreeFlatRow[] = [];
    function walk(list: FileTreeNode[]) {
      for (const n of list) {
        out.push({
          path: n.path,
          name: n.name,
          depth: n.depth,
          isDirectory: n.isDirectory,
          hasChildren: n.isDirectory,
          isExpanded: n.isExpanded,
        });
        if (n.isExpanded && n.children) walk(n.children);
      }
    }
    walk(nodes.value);
    return out;
  });

  return { rows, toggle, refresh, expand };
}

function buildRoots(paths: string[]): FileTreeNode[] {
  return paths.map((p) => ({
    path: p,
    name: p.split(/[\\/]/).pop() ?? p,
    isDirectory: true,
    isExpanded: false,
    isLoaded: false,
    depth: 0,
    children: null,
  }));
}

function defaultDeps(): UseFileTreeDeps {
  // 用 sigma 已有的 fs API,具体路径实施时确认
  return {
    readDir: async (path: string) => {
      const mod = await import('@/modules/native'); // 占位
      return mod.readDir(path);
    },
  };
}
```

- [ ] **Step 4:跑测试确认 GREEN**

```bash
npx vitest run src/modules/navigator/composables/__tests__/use-file-tree.test.ts
```

预期:PASS。

- [ ] **Step 5:提交**

```bash
git add src/modules/navigator/composables/use-file-tree.ts src/modules/navigator/composables/__tests__/use-file-tree.test.ts
git commit -m "feat(navigator): add useFileTree composable with lazy loading"
```

### Task 2.3:文件树视图组件

**Files:**
- Create: `src/modules/navigator/components/file-browser/file-browser-tree-view.vue`

- [ ] **Step 1:写组件**

文件路径:`src/modules/navigator/components/file-browser/file-browser-tree-view.vue`

```vue
<!-- FORK-MODIFICATION: new file (tree view). Keep during upstream sync.
     Tracking issue: aleksey-hoffman/sigma-file-manager#499 -->
<script setup lang="ts">
import { computed } from 'vue';
import { ChevronRightIcon, ChevronDownIcon, FolderIcon, FileIcon } from '@lucide/vue';
import { useFileTree } from '@/modules/navigator/composables/use-file-tree';

const props = defineProps<{
  rootPaths: string[];
}>();

const emit = defineEmits<{
  activate: [path: string];
}>();

const { rows, toggle } = useFileTree({ rootPaths: props.rootPaths });

function onClick(row: { path: string; isDirectory: boolean; isExpanded: boolean }) {
  if (row.isDirectory) {
    toggle(row.path);
  } else {
    emit('activate', row.path);
  }
}

const expandedSet = computed(() => new Set(rows.value.filter(r => r.isExpanded).map(r => r.path)));
</script>

<template>
  <div class="file-tree-view" data-e2e-root="file-tree-view">
    <div
      v-for="row in rows"
      :key="row.path"
      class="file-tree-row"
      :style="{ paddingLeft: `${row.depth * 16 + 8}px` }"
      @click="onClick(row)"
    >
      <component
        :is="row.isDirectory && row.isExpanded ? ChevronDownIcon : ChevronRightIcon"
        v-if="row.isDirectory"
        :size="14"
      />
      <span v-else class="file-tree-row__spacer" />
      <component
        :is="row.isDirectory ? FolderIcon : FileIcon"
        :size="14"
      />
      <span class="file-tree-row__name">{{ row.name }}</span>
    </div>
  </div>
</template>

<style scoped>
.file-tree-view {
  overflow-y: auto;
  height: 100%;
  font-size: 13px;
}

.file-tree-row {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px 2px 0;
  cursor: pointer;
  user-select: none;
}

.file-tree-row:hover {
  background-color: hsl(var(--muted) / 50%);
}

.file-tree-row__spacer {
  display: inline-block;
  width: 14px;
}

.file-tree-row__name {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
```

- [ ] **Step 2:本地验证(dev 模式临时预览)**

为了不阻塞后续 Task,在 dev 模式下临时改 `file-browser.vue` import 这个组件并显示它。验证可视化和展开/折叠正常后,**还原 file-browser.vue**,这个集成留到 Task 2.4。

```bash
# 临时改 file-browser.vue 加入测试
npm run tauri:dev
# 浏览器/窗口中手动验证后还原
```

- [ ] **Step 3:提交**

```bash
git add src/modules/navigator/components/file-browser/file-browser-tree-view.vue
git commit -m "feat(navigator): add FileBrowserTreeView component"
```

### Task 2.4:把 tree view 接入 file-browser

**Files:**
- Modify: `src/modules/navigator/components/file-browser/file-browser.vue`

- [ ] **Step 1:读 file-browser.vue 找 layout 切换处**

```bash
grep -n "view\|layout\|grid\|list" src/modules/navigator/components/file-browser/file-browser.vue | head -30
```

- [ ] **Step 2:加 layout 枚举扩展**

参考 `src/types/view.ts`(或类似的 view 类型文件),加 `'tree'` 选项。**这是 fork 改动,加 `[fork-keep]` 注释**:

```typescript
// FORK-MODIFICATION: add 'tree' to view layout
export type ForkViewLayout = 'compact-list' | 'list' | 'grid' | 'tree';
```

- [ ] **Step 3:在 file-browser.vue 接入**

在 layout 派发处加(具体位置按实际代码调整):
```vue
<!-- 原有: -->
<component :is="listViewComponent" />
<component :is="gridViewComponent" />

<!-- ↓↓↓ FORK-MODIFICATION: tree view integration ↓↓↓ -->
<component
  v-else-if="layout === 'tree'"
  :is="treeViewComponent"
  :root-paths="[currentPath]"
  @activate="onTreeActivate"
/>
<!-- ↑↑↑ FORK-MODIFICATION ↑↑↑ -->
```

并在 `<script setup>` 加:
```typescript
import FileBrowserTreeView from './file-browser-tree-view.vue';

const treeViewComponent = FileBrowserTreeView;

function onTreeActivate(path: string) {
  openNavigatorPath(router, path);
}
```

- [ ] **Step 4:加 layout 切换 UI**

找一个 layout 切换按钮的现有位置,加一个 Tree 按钮:
```vue
<!-- FORK-MODIFICATION: tree layout button -->
<button
  v-if="'tree' in layoutOptions"
  class="layout-toggle-btn"
  :class="{ active: layout === 'tree' }"
  @click="layout = 'tree'"
>
  Tree
</button>
```

- [ ] **Step 5:跑 check + 启动验证**

```bash
npm run check
npm run tauri:dev
```

验证:
- 设置里能看到 Tree 选项
- 切到 Tree 后显示当前路径的树
- 点击目录能展开
- 双击文件能跳到该文件目录

- [ ] **Step 6:提交**

```bash
git add .
git commit -m "feat(navigator): integrate tree view as a layout option"
```

### Task 2.5:文件树视图的 E2E 测试

**Files:**
- Create: `e2e-webdriver/test/specs/navigator-tree-view.e2e.js`

- [ ] **Step 1:写 E2E 测试骨架**

```js
describe('Navigator tree view', () => {
  it('expands a directory when clicked', async () => {
    // 假设 webdriver 已能导航
    await openNavigator('C:/work');
    await switchLayout('tree');
    const row = await $('.file-tree-row');
    await row.click();
    await expect(row).toHaveAttribute('aria-expanded', 'true'); // 占位,具体断言按实际 DOM 调整
  });

  it('navigates to a file directory on activation', async () => {
    await openNavigator('C:/work');
    await switchLayout('tree');
    await expandPath('sub');
    await activateRow('a.md');
    const breadcrumb = await getBreadcrumbText();
    expect(breadcrumb).toContain('sub');
  });
});
```

- [ ] **Step 2:运行 E2E(若环境就绪)**

```bash
cd e2e-webdriver
npm install
npm run test:navigator-tree-view
```

> **占位**:具体 E2E 运行命令按 sigma 当前 wdio 配置推。

- [ ] **Step 3:提交**

```bash
git add e2e-webdriver/test/specs/navigator-tree-view.e2e.js
git commit -m "test(e2e): add navigator tree view specs"
```

### Task 2.6:tree view 改动加同步保护标记

**Files:**
- Modify: `docs/superpowers/plans/2026-09-24-sigma-fork-and-plugin.md`(本文件)
- Create: `F:\soft\00selfmade\filemanager\docs\FORK-KEEP-LIST.md`

- [ ] **Step 1:写 fork-keep 清单**

文件路径:`F:\soft\00selfmade\filemanager\docs\FORK-KEEP-LIST.md`

```markdown
# Fork 保留清单(FORK-KEEP-LIST)

这些文件/区域**永远用 fork 版本**,即使 upstream 重写。

## 文件树视图(对应 issue #499)

### 核心改动文件
- `src/modules/navigator/types/file-tree.ts` — 类型
- `src/modules/navigator/composables/use-file-tree.ts` — composable
- `src/modules/navigator/components/file-browser/file-browser-tree-view.vue` — 组件
- `src/modules/navigator/components/file-browser/file-browser.vue` — 接入点(找 `FORK-MODIFICATION` 注释)
- `src/types/view.ts` — layout 枚举扩展(找 `FORK-MODIFICATION` 注释)

### 同步策略
- 若 upstream 新增 tree view,**对比接口**,若实现差异大则继续保留本地版本
- 若 upstream 重构 file-browser.vue,优先保留本地 layout 接入点
```

- [ ] **Step 2:提交**

```bash
cd /f/soft/00selfmade/filemanager
git add docs/FORK-KEEP-LIST.md
git commit -m "docs: fork keep list for tree view changes"
```

---

## Phase 3:批量改名插件(独立项目)

**独立仓库,完全不和 fork 耦合**。

### Task 3.1:插件工程脚手架

**Files:**(新仓库 `F:\soft\00selfmade\sigma-batch-rename\`)

- [ ] **Step 1:克隆 Excalidraw 模板**

```bash
cd /f/soft/00selfmade
git clone https://github.com/sigma-hub/sfm-extension-excalidraw.git sigma-batch-rename
cd sigma-batch-rename
```

- [ ] **Step 2:清空模板内容,改 manifest**

修改 `package.json` 的关键字段:
```json
{
  "$schema": "./node_modules/@sigma-file-manager/api/manifest.schema.json",
  "id": "sigma.batch-rename",
  "name": "Batch Rename",
  "version": "0.1.0",
  "publisher": { "name": "<your-name>", "url": "https://github.com/<your-name>" },
  "repository": "https://github.com/<your-name>/sigma-batch-rename",
  "license": "MIT",
  "extensionType": "api",
  "type": "module",
  "main": "dist/index.js",
  "icon": "icon.png",
  "permissions": [
    "commands",
    "contextMenu",
    "fs.read",
    "fs.write",
    "dialogs",
    "notifications"
  ],
  "activationEvents": [
    "onStartup",
    "onCommand:sigma.batch-rename.openDialog",
    "onCommand:sigma.batch-rename.runLastRule"
  ],
  "contributes": {
    "commands": [
      {
        "id": "openDialog",
        "title": "Batch Rename...",
        "description": "Open the batch rename dialog",
        "shortcut": "F2"
      },
      {
        "id": "runLastRule",
        "title": "Apply last batch rename rule",
        "description": "Re-apply the most recent rule to current selection"
      }
    ],
    "contextMenu": [
      {
        "id": "batchRename",
        "title": "Batch Rename...",
        "when": { "selectionType": "multiple", "entryType": "any" }
      }
    ]
  },
  "engines": { "sigmaFileManager": ">=2.2.0" },
  "devDependencies": {
    "@sigma-file-manager/api": "^1.5.0",
    "esbuild": "^0.27.4",
    "typescript": "~5.8.0"
  }
}
```

- [ ] **Step 3:安装依赖**

```bash
npm install
```

- [ ] **Step 4:写 src/index.ts 骨架**

```typescript
import type { ExtensionActivationContext } from '@sigma-file-manager/api';

export async function activate(ctx: ExtensionActivationContext): Promise<void> {
  await sigma.i18n.mergeFromPath('locales');

  sigma.commands.registerCommand(
    { id: 'sigma.batch-rename.openDialog', title: 'Batch Rename...', shortcut: 'F2' },
    () => import('./open-dialog').then(m => m.openDialog()),
  );

  sigma.commands.registerCommand(
    { id: 'sigma.batch-rename.runLastRule', title: 'Run last rule' },
    () => import('./run-last-rule').then(m => m.runLastRule()),
  );

  sigma.contextMenu.registerItem(
    { id: 'batchRename', title: 'Batch Rename...' },
    () => import('./open-dialog').then(m => m.openDialogFromContextMenu()),
  );
}

export async function deactivate(): Promise<void> {}
```

> 命名约定:命令 id 前缀 `sigma.batch-rename.` 以避免冲突。

- [ ] **Step 5:tsconfig + esbuild 配置**

保留模板的 `tsconfig.json`、`scripts/build.mjs`,只改入口输出文件名匹配 manifest 的 `main`。

- [ ] **Step 6:首次构建**

```bash
npm run build
```

预期:`dist/index.js` 生成。

- [ ] **Step 7:在 sigma 中本地加载**

```bash
# 把插件目录软链/复制到 sigma 的扩展加载路径(具体路径按 sigma 文档)
mkdir -p ~/.local/share/sigma-file-manager/extensions
ln -s /f/soft/00selfmade/sigma-batch-rename ~/.local/share/sigma-file-manager/extensions/sigma.batch-rename
```

打开 sigma → Extensions,确认能看到 Batch Rename 插件并启用。

- [ ] **Step 8:提交**

```bash
git add .
git commit -m "feat: bootstrap batch rename extension"
```

### Task 3.2:改名规则引擎(纯函数 + 测试)

**Files:**
- Create: `src/rename-engine.ts`
- Create: `src/__tests__/rename-engine.test.ts`

- [ ] **Step 1:写规则引擎测试**

文件路径:`src/__tests__/rename-engine.test.ts`

```typescript
import { describe, it, expect } from 'vitest';
import { applyRule, type RenameRule, type PreviewInput } from '../rename-engine';

const baseInput: PreviewInput = {
  originalPath: 'C:/work/photo.jpg',
  originalName: 'photo.jpg',
  isDirectory: false,
};

describe('applyRule: find-and-replace', () => {
  it('replaces literal substring', () => {
    const rule: RenameRule = { kind: 'replace', find: 'photo', replace: 'image' };
    expect(applyRule(baseInput, rule).newName).toBe('image.jpg');
  });

  it('preserves extension when match is in basename without ext', () => {
    const rule: RenameRule = { kind: 'replace', find: 'photo', replace: 'image', scope: 'nameOnly' };
    expect(applyRule(baseInput, rule).newName).toBe('image.jpg');
  });
});

describe('applyRule: sequence numbering', () => {
  it('pads numbers with zeros', () => {
    const rule: RenameRule = { kind: 'sequence', template: 'img_{n:03}', startAt: 1, pad: 3 };
    expect(applyRule(baseInput, rule, { index: 5 }).newName).toBe('img_005.jpg');
  });

  it('preserves extension by default', () => {
    const rule: RenameRule = { kind: 'sequence', template: 'img_{n}', startAt: 1, pad: 1 };
    expect(applyRule(baseInput, rule, { index: 1 }).newName).toBe('img_1.jpg');
  });
});

describe('applyRule: regex', () => {
  it('applies a regex replacement', () => {
    const rule: RenameRule = { kind: 'regex', pattern: '^(\\w+)', replace: 'prefix_$1' };
    expect(applyRule(baseInput, rule).newName).toBe('prefix_photo.jpg');
  });
});

describe('applyRule: case transform', () => {
  it('converts to lowercase', () => {
    const rule: RenameRule = { kind: 'case', mode: 'lower' };
    expect(applyRule({ ...baseInput, originalName: 'Photo.JPG' }, rule).newName).toBe('photo.jpg');
  });
});

describe('applyRule: insert / remove substrings', () => {
  it('inserts a prefix', () => {
    const rule: RenameRule = { kind: 'insert', position: 'prefix', value: 'new_' };
    expect(applyRule(baseInput, rule).newName).toBe('new_photo.jpg');
  });

  it('inserts a suffix before extension', () => {
    const rule: RenameRule = { kind: 'insert', position: 'suffix', value: '_v2' };
    expect(applyRule(baseInput, rule).newName).toBe('photo_v2.jpg');
  });
});

describe('applyRule: invalid filename detection', () => {
  it('flags names with forbidden characters', () => {
    const rule: RenameRule = { kind: 'replace', find: 'photo', replace: 'a/b' };
    const result = applyRule(baseInput, rule);
    expect(result.isValid).toBe(false);
    expect(result.error).toContain('/');
  });
});
```

- [ ] **Step 2:跑测试确认 RED**

```bash
cd /f/soft/00selfmade/sigma-batch-rename
npx vitest run src/__tests__/rename-engine.test.ts
```

- [ ] **Step 3:实现引擎**

文件路径:`src/rename-engine.ts`

```typescript
export type RenameRule =
  | { kind: 'replace'; find: string; replace: string; scope?: 'full' | 'nameOnly' }
  | { kind: 'sequence'; template: string; startAt: number; pad: number }
  | { kind: 'regex'; pattern: string; flags?: string; replace: string }
  | { kind: 'case'; mode: 'lower' | 'upper' | 'title' }
  | { kind: 'insert'; position: 'prefix' | 'suffix'; value: string };

export interface PreviewInput {
  originalPath: string;
  originalName: string;
  isDirectory: boolean;
}

export interface PreviewOutput {
  newName: string;
  isValid: boolean;
  error?: string;
}

const FORBIDDEN = /[<>:"/\\|?*\x00-\x1f]/;

function splitName(name: string): { stem: string; ext: string } {
  const i = name.lastIndexOf('.');
  if (i <= 0) return { stem: name, ext: '' };
  return { stem: name.slice(0, i), ext: name.slice(i) };
}

function joinName(stem: string, ext: string): string {
  return ext ? `${stem}${ext}` : stem;
}

function validate(name: string): { isValid: boolean; error?: string } {
  if (!name) return { isValid: false, error: 'empty name' };
  if (name === '.' || name === '..') return { isValid: false, error: 'reserved name' };
  if (FORBIDDEN.test(name)) return { isValid: false, error: `forbidden character in: ${name}` };
  return { isValid: true };
}

export function applyRule(input: PreviewInput, rule: RenameRule, opts: { index?: number } = {}): PreviewOutput {
  const index = opts.index ?? 0;
  let stem: string;
  let ext: string;

  switch (rule.kind) {
    case 'replace': {
      ({ stem, ext } = splitName(input.originalName));
      const target = rule.scope === 'full' ? input.originalName : stem;
      const replaced = target.split(rule.find).join(rule.replace);
      if (rule.scope === 'full') return finalize(replaced);
      return finalize(joinName(replaced, ext));
    }
    case 'sequence': {
      ({ stem, ext } = splitName(input.originalName));
      const n = String(rule.startAt + index).padStart(rule.pad, '0');
      const templated = rule.template.replace(/\{n(:0(\d+))?\}/g, (_match, _fmt, len) => {
        return len ? String(rule.startAt + index).padStart(parseInt(len, 10), '0') : n;
      });
      return finalize(joinName(templated, ext));
    }
    case 'regex': {
      const re = new RegExp(rule.pattern, rule.flags ?? '');
      return finalize(input.originalName.replace(re, rule.replace));
    }
    case 'case': {
      ({ stem, ext } = splitName(input.originalName));
      const apply = (s: string) => {
        switch (rule.mode) {
          case 'lower': return s.toLowerCase();
          case 'upper': return s.toUpperCase();
          case 'title': return s.replace(/\w\S*/g, w => w[0].toUpperCase() + w.slice(1).toLowerCase());
        }
      };
      return finalize(joinName(apply(stem), ext));
    }
    case 'insert': {
      ({ stem, ext } = splitName(input.originalName));
      const newStem = rule.position === 'prefix' ? rule.value + stem : stem + rule.value;
      return finalize(joinName(newStem, ext));
    }
  }
}

function finalize(name: string): PreviewOutput {
  const v = validate(name);
  return { newName: name, ...v };
}

export function applyRuleToMany(inputs: PreviewInput[], rule: RenameRule): PreviewOutput[] {
  return inputs.map((input, idx) => applyRule(input, rule, { index: idx }));
}
```

- [ ] **Step 4:跑测试确认 GREEN**

```bash
npx vitest run src/__tests__/rename-engine.test.ts
```

预期:全部 PASS。

- [ ] **Step 5:提交**

```bash
git add src/rename-engine.ts src/__tests__/rename-engine.test.ts
git commit -m "feat: rename engine with replace/sequence/regex/case/insert rules"
```

### Task 3.3:弹窗 UI(form + list-detail)

**Files:**
- Create: `src/open-dialog.ts`

- [ ] **Step 1:写弹窗逻辑**

文件路径:`src/open-dialog.ts`

```typescript
import { applyRuleToMany, type RenameRule } from './rename-engine';
import type { ExtensionContextEntry } from '@sigma-file-manager/api';

type RuleKind = RenameRule['kind'];

const RULE_KINDS: { value: RuleKind; label: string }[] = [
  { value: 'replace', label: 'Find & Replace' },
  { value: 'sequence', label: 'Sequence Numbering' },
  { value: 'regex', label: 'Regex' },
  { value: 'case', label: 'Case Transform' },
  { value: 'insert', label: 'Insert Prefix/Suffix' },
];

export async function openDialog() {
  const entries = sigma.context.getSelectedEntries();
  if (entries.length === 0) {
    sigma.ui.showNotification({ title: 'Batch Rename', description: 'No files selected', type: 'warning' });
    return;
  }

  const handle = sigma.ui.createModal({
    title: 'Batch Rename',
    width: 720,
    layout: 'listDetail',
    content: [
      sigma.ui.select({
        id: 'ruleKind',
        label: 'Rule',
        options: RULE_KINDS.map(k => ({ value: k.value, label: k.label })),
        value: 'replace',
      }),
      sigma.ui.input({ id: 'find', label: 'Find', placeholder: 'photo' }),
      sigma.ui.input({ id: 'replace', label: 'Replace', placeholder: 'image' }),
      sigma.ui.checkbox({ id: 'preserveExt', label: 'Preserve extension' }),
    ],
    listDetail: {
      items: entries.map((e, i) => ({
        id: e.path,
        title: e.name,
        subtitle: e.path,
        icon: e.isDirectory ? 'files' : 'text',
      })),
      selectedItemId: entries[0]?.path ?? null,
      searchQuery: '',
      filterValue: 'all',
      filterOptions: [
        { value: 'all', label: 'All' },
        { value: 'changed', label: 'Changed' },
        { value: 'unchanged', label: 'Unchanged' },
      ],
      detail: { type: 'text', text: '' },
      detailFields: [],
    },
    buttons: [
      { id: 'preview', label: 'Preview' },
      { id: 'apply', label: 'Apply', variant: 'primary' },
      { id: 'cancel', label: 'Cancel' },
    ],
  });

  let currentRule: RenameRule = { kind: 'replace', find: '', replace: '' };

  handle.onValueChange((elementId, value, allValues) => {
    if (elementId === 'ruleKind') {
      // 更新 UI:切换规则类型时,重置默认值;具体字段集按实际 sigma createModal 行为调整
      currentRule = buildRuleFromForm(allValues);
    } else {
      currentRule = buildRuleFromForm(allValues);
    }
    refreshPreview(handle, entries, currentRule);
  });

  handle.onSubmit(async (values, buttonId) => {
    if (buttonId === 'cancel') return true; // 关闭
    if (buttonId === 'preview') {
      refreshPreview(handle, entries, buildRuleFromForm(values));
      return false; // 不关
    }
    if (buttonId === 'apply') {
      const rule = buildRuleFromForm(values);
      const previews = applyRuleToMany(entries as any, rule);
      await applyRenames(entries, previews, rule);
      return true;
    }
    return false;
  });
}

function buildRuleFromForm(values: Record<string, unknown>): RenameRule {
  const kind = (values.ruleKind as RuleKind) ?? 'replace';
  switch (kind) {
    case 'replace':
      return { kind: 'replace', find: String(values.find ?? ''), replace: String(values.replace ?? ''), scope: values.preserveExt ? 'nameOnly' : 'full' };
    case 'sequence':
      return { kind: 'sequence', template: String(values.template ?? '{n}'), startAt: Number(values.startAt ?? 1), pad: Number(values.pad ?? 1) };
    case 'regex':
      return { kind: 'regex', pattern: String(values.pattern ?? ''), flags: String(values.flags ?? ''), replace: String(values.regexReplace ?? '') };
    case 'case':
      return { kind: 'case', mode: (values.caseMode as 'lower' | 'upper' | 'title') ?? 'lower' };
    case 'insert':
      return { kind: 'insert', position: (values.position as 'prefix' | 'suffix') ?? 'prefix', value: String(values.insertValue ?? '') };
  }
}

function refreshPreview(handle: ReturnType<typeof sigma.ui.createModal>, entries: ExtensionContextEntry[], rule: RenameRule) {
  const previews = applyRuleToMany(entries.map(e => ({
    originalPath: e.path,
    originalName: e.name,
    isDirectory: e.isDirectory,
  })), rule);

  handle.setListDetail({
    items: entries.map((e, i) => ({
      id: e.path,
      title: previews[i].newName === e.name ? `${e.name}` : `${e.name} → ${previews[i].newName}`,
      subtitle: e.path,
      icon: previews[i].isValid ? 'text' : 'files',
    })),
    detail: {
      type: 'text',
      text: previews.map((p, i) => `${entries[i].name} → ${p.newName}${p.isValid ? '' : ' [INVALID: ' + p.error + ']'}`).join('\n'),
    },
  });
}

async function applyRenames(
  entries: ExtensionContextEntry[],
  previews: ReturnType<typeof applyRuleToMany>,
  rule: RenameRule,
) {
  const validOps = entries
    .map((e, i) => ({ entry: e, preview: previews[i] }))
    .filter(({ entry, preview }) => preview.isValid && preview.newName !== entry.name);

  await sigma.ui.withProgress(
    { subtitle: `Renaming ${validOps.length} files`, location: 'notification', cancellable: true },
    async (progress, token) => {
      let i = 0;
      for (const { entry, preview } of validOps) {
        if (token.isCancellationRequested) break;
        const dir = entry.path.slice(0, entry.path.length - entry.name.length);
        const newPath = dir + preview.newName;
        try {
          // sigma.fs 没有直接 rename,需用 shell 或读+写+删;具体方式实施时确认
          await renameFile(entry.path, newPath);
          progress.report({ subtitle: `${entry.name} → ${preview.newName}` });
          i++;
          progress.report({ increment: 100 / validOps.length });
        } catch (err) {
          sigma.ui.showNotification({ title: 'Rename failed', description: `${entry.name}: ${err}`, type: 'error' });
        }
      }
      sigma.ui.showNotification({ title: 'Batch rename complete', description: `Renamed ${i} files`, type: 'success' });
    },
  );

  // 记忆最近一条规则,供 runLastRule 用
  await sigma.storage.set('lastRule', rule);
}

async function renameFile(from: string, to: string) {
  // sigma.fs 没有直接 rename 接口,优先用平台自带的 move 命令
  // Windows: cmd /c move, Unix: mv
  const isWindows = sigma.platform.isWindows;
  const cmd = isWindows ? 'cmd' : 'mv';
  const args = isWindows
    ? ['/c', 'move', '/Y', from, to]
    : [from, to];

  const result = await sigma.shell.run(cmd, args);
  if (result.code !== 0) {
    throw new Error(`rename failed (${result.code}): ${result.stderr || result.stdout}`);
  }
}

export async function openDialogFromContextMenu() {
  return openDialog();
}
```

- [ ] **Step 2:占位实现 run-last-rule**

文件路径:`src/run-last-rule.ts`

```typescript
export async function runLastRule() {
  const rule = await sigma.storage.get<RenameRule>('lastRule');
  if (!rule) {
    sigma.ui.showNotification({ title: 'Batch Rename', description: 'No previous rule saved', type: 'warning' });
    return;
  }
  const { openDialog } = await import('./open-dialog');
  // 复用 openDialog 但用上次的 rule 初始化;具体实现可在 Phase 4 迭代
  return openDialog();
}
```

> 注意:`run-last-rule` 需要传入 rule 覆盖,目前是占位,等下个迭代完善。

- [ ] **Step 3:构建并加载**

```bash
npm run build
# 在 sigma 中重启或重载扩展
```

- [ ] **Step 4:手动验证**

在 sigma 中:
1. 选中 3~5 个文件
2. 触发 Batch Rename 命令(右键或 F2)
3. 设置一个规则,点 Preview,确认预览正确
4. 点 Apply,确认文件被改名

- [ ] **Step 5:写 vitest 集成测试(可选,先 unit 覆盖)**

文件路径:`src/__tests__/open-dialog.test.ts`

```typescript
import { describe, it, expect, vi } from 'vitest';

// 模拟 sigma 全局
declare global {
// @ts-expect-error - mock in test
var sigma: any;
}

import { openDialog } from '../open-dialog';

describe('openDialog', () => {
  it('shows a warning notification when no files are selected', async () => {
    globalThis.sigma = {
      context: { getSelectedEntries: () => [] },
      ui: {
        showNotification: vi.fn(),
        createModal: vi.fn(),
      },
    };

    await openDialog();

    expect(globalThis.sigma.ui.showNotification).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'warning' }),
    );
  });
});
```

- [ ] **Step 6:跑测试**

```bash
npx vitest run
```

预期:PASS(若 mock 不全可能需要简化)。

- [ ] **Step 7:提交**

```bash
git add .
git commit -m "feat: batch rename dialog with preview and apply"
```

### Task 3.4:i18n + README + 发布

**Files:**
- Create: `locales/en.json`, `locales/zh-CN.json`
- Modify: `README.md`

- [ ] **Step 1:写英文 locale**

文件路径:`locales/en.json`

```json
{
  "extension.batchRename.title": "Batch Rename",
  "extension.batchRename.find": "Find",
  "extension.batchRename.replace": "Replace",
  "extension.batchRename.preview": "Preview",
  "extension.batchRename.apply": "Apply"
}
```

- [ ] **Step 2:写中文 locale**

文件路径:`locales/zh-CN.json`

```json
{
  "extension.batchRename.title": "批量改名",
  "extension.batchRename.find": "查找",
  "extension.batchRename.replace": "替换",
  "extension.batchRename.preview": "预览",
  "extension.batchRename.apply": "应用"
}
```

- [ ] **Step 3:在 src/index.ts 加载 locales**

确认 `activate` 里有:
```typescript
await sigma.i18n.mergeFromPath('locales');
```

- [ ] **Step 4:写 README**

文件路径:`README.md`

```markdown
# Sigma File Manager — Batch Rename Extension

> 批量改名插件,支持查找替换、序列编号、正则、批量改名大小写、前后缀插入。

## 安装
在 sigma 内置市场中搜索 "Batch Rename",或本地加载。

## 用法
1. 选中多个文件
2. 按 F2 或右键 → "Batch Rename..."
3. 设置规则,点 Preview 预览
4. 点 Apply 执行

## 规则类型
- **Find & Replace**:字面量替换,可保留扩展名
- **Sequence Numbering**:用 `{n}` 或 `{n:03}` 模板生成序列号
- **Regex**:正则替换
- **Case Transform**:lower/upper/title
- **Insert**:在文件名前/后插入文本

## 开发
\`\`\`bash
npm install
npm run build    # 产出 dist/index.js
npm run test     # 跑 vitest
npm run dev      # tsc watch + 自动重建
\`\`\`

## 仓库
https://github.com/<your-name>/sigma-batch-rename
```

- [ ] **Step 5:打 tag + push**

```bash
git add .
git commit -m "feat: i18n + initial release"
git tag v0.1.0
git push origin main --tags
```

- [ ] **Step 6:发 GitHub release**

```bash
gh release create v0.1.0 --title "v0.1.0 — Initial Release" --notes "First public version of the batch rename extension."
```

---

## Phase 4:长期维护工作流

### Task 4.1:fork 同步自动化脚本

**Files:**
- Create: `F:\soft\00selfmade\filemanager\scripts\sync-upstream.sh`

- [ ] **Step 1:写同步脚本**

文件路径:`F:\soft\00selfmade\filemanager\scripts\sync-upstream.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail

FORK_DIR="/f/soft/00selfmade/filemanager/sigma-file-manager"
cd "$FORK_DIR"

echo "==> Fetching upstream"
git fetch upstream

echo "==> Checking current branch"
current_branch="$(git symbolic-ref --short HEAD)"
if [ "$current_branch" != "main" ]; then
  echo "ERROR: must be on main branch (currently: $current_branch)"
  exit 1
fi

echo "==> Merging upstream/main"
if ! git merge upstream/main --ff-only; then
  echo ""
  echo "!!!  Fast-forward failed. Likely upstream has commits you don't have locally."
  echo "    If you have local commits on main that should be in feat/* branches, abort and clean up first."
  echo "    Otherwise, rebase / non-ff merge is required."
  exit 2
fi

echo "==> Pushing to origin"
git push origin main

echo "==> Done. Open questions for you:"
echo "    1. Did upstream close any issue we were tracking? -> Update PR-LOG.md"
echo "    2. Are there conflicts in FORK-KEEP files? -> Resolve preserving fork version"
echo "    3. Are there new extension API changes? -> Update sigma-batch-rename if affected"
```

- [ ] **Step 2:加可执行权限**

```bash
chmod +x /f/soft/00selfmade/filemanager/scripts/sync-upstream.sh
```

- [ ] **Step 3:第一次跑**

```bash
/f/soft/00selfmade/filemanager/scripts/sync-upstream.sh
```

预期:fast-forward merge,推送成功。

- [ ] **Step 4:提交**

```bash
cd /f/soft/00selfmade/filemanager
git add scripts/sync-upstream.sh
git commit -m "chore: add upstream sync script"
```

### Task 4.2:月度同步检查清单

**Files:**
- Create: `F:\soft\00selfmade\filemanager\docs\MONTHLY-SYNC-CHECKLIST.md`

- [ ] **Step 1:写清单**

```markdown
# 月度同步清单

每月底(每月 25 号左右)执行一次。

## Fork 仓库

- [ ] 跑 `/f/soft/00selfmade/filemanager/scripts/sync-upstream.sh`
- [ ] 跑 `cd /f/soft/00selfmade/filemanager/sigma-file-manager && npm run check`
- [ ] 跑 `cd /f/soft/00selfmade/filemanager/sigma-file-manager && npm run tauri:build`
- [ ] 检查 FORK-KEEP-LIST.md 中的文件,对比 upstream,决定是否需要适配
- [ ] 更新 PR-LOG.md,记录状态变化

## 插件仓库

- [ ] 检查 sigma 是否发了新版本,决定 `@sigma-file-manager/api` 是否升级
- [ ] `cd /f/soft/00selfmade/sigma-batch-rename && npm run test`
- [ ] 在新版本 sigma 中手动测试一次插件核心功能
- [ ] 若发现 API 变更,在 CHANGELOG 记录

## 文档维护

- [ ] 更新本清单中的过时项
- [ ] 若 fork-keep 列表变化,同步更新 FORK-KEEP-LIST.md

## 下个月计划

- [ ] 看 sigma 路线图,决定是否开始下个特性
```

- [ ] **Step 2:提交**

```bash
git add docs/MONTHLY-SYNC-CHECKLIST.md
git commit -m "docs: monthly sync checklist"
```

### Task 4.3:首次完整同步执行(任务完成后)

- [ ] **Step 1:在 fork 仓库跑 sync 脚本**

```bash
/f/soft/00selfmade/filemanager/scripts/sync-upstream.sh
```

- [ ] **Step 2:跑完整 check**

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
npm run check
npm run tauri:build
```

预期:都通过(若有失败,记录并修复)。

- [ ] **Step 3:在插件仓库跑测试**

```bash
cd /f/soft/00selfmade/sigma-batch-rename
npm install
npm run test
npm run build
```

预期:都通过。

- [ ] **Step 4:打月度同步 tag(可选)**

```bash
cd /f/soft/00selfmade/filemanager
git tag sync-$(date +%Y%m)
git push origin sync-$(date +%Y%m)
```

---

## 整体里程碑

| 里程碑 | 时间估计 | 标志 |
|---|---|---|
| **M0:基础设施就绪** | 0.5 天 | Fork 克隆,sync 脚本可用,能在 sigma dev 模式跑 |
| **M1:启动目录 PR 合入上游** | 1~2 天 | 自定义路径在 sigma 设置可见,PR 被合并 |
| **M2:文件树 MVP** | 3~5 天 | 在 fork 上能看到 tree view,展开/折叠可用 |
| **M3:批量改名插件 v0.1.0** | 2~3 天 | 插件可装,核心规则工作,有测试覆盖 |
| **M4:第一次月度同步** | M3 后 ~30 天 | 跑完同步脚本,无冲突或冲突已解 |

---

## 风险与备选

### 风险 1:启动目录 PR 被拒
- **备选**:fork 永久保留这部分改动,通过 `FORK-KEEP-LIST.md` 标记

### 风险 2:文件树改动太大,fork 同步越来越痛
- **触发条件**:连续 2 次月度同步冲突超过 30 分钟
- **应对**:把文件树改成独立插件(损失:不是 navigator 集成,是 sidebar 全屏页面)

### 风险 3:插件 API 升级破坏兼容
- **监测**:每次 sigma 新版本发布,跑插件 smoke test
- **应对**:锁定 `@sigma-file-manager/api` 到当前版本,等下一个 minor 升级再适配

### 风险 4:fork 大版本升级时(比如 sigma v3)冲突不可调和
- **应对**:把 fork 升级到新 major,**重做 PR-LOG 中的所有 PR**(但 PR-LOG 应该已经合并完了)
- **应急**:暂停本地 fork 工作,等上游稳定后再追

---

## 完成定义(Definition of Done)

整个计划"完成"需要满足:

1. **M0**: ✅ Fork 仓库可以本地 build,可以在本地开发模式下看到 sigma 主窗口
2. **M1**: ✅ 在 sigma 设置里能看到自定义启动路径选项,选完重启能直接进入该目录,且 PR 已开(即使没合并)
3. **M2**: ✅ 文件树视图在 fork 仓库的 dev 模式下能正常工作(展开/折叠/激活)
4. **M3**: ✅ 批量改名插件在 sigma 中能安装,核心规则(rename/replace/sequence)能工作
5. **M4**: ✅ 月度同步脚本可用,FORK-KEEP-LIST.md 已建立,MONTHLY-SYNC-CHECKLIST.md 已就绪
6. **持续**: ✅ PR-LOG.md 跟踪所有 PR 状态,每月同步无重大冲突