# 启动路径数据流笔记(2026-09-25 摸清)

## StartupPage 类型定义

**位置**: `src/types/user-settings.ts:320`

```typescript
export type StartupPage = 'last' | 'home' | 'dashboard' | 'navigator';
```

## 5 个消费点

| # | 位置 | 角色 |
|---|---|---|
| 1 | `src/types/user-settings.ts:211` | `UserSettings` 接口里的字段:`startupPage: StartupPage` |
| 2 | `src/stores/storage/user-settings.ts:213` | 默认值 `startupPage: 'home'` |
| 3 | `src/modules/settings/ui/categories/general/startup-page.vue` | 设置 UI(下拉选项 + i18n) |
| 4 | `src/composables/use-init.ts:225` (`applyStartupPage`) | 应用启动时调用 |
| 5 | `src/utils/last-route.ts:77` (`resolveStartupRouteLocation`) | switch-case 核心分发 |

## 数据流

```
用户设置 → userSettingsStore.userSettings.startupPage
         ↓
应用启动 → applyStartupPage() in use-init.ts
         ↓
resolveStartupRouteLocation(startupPage, lastRoute)
         ↓
返回 StartupRouteLocation | null
         ↓
router.push(startupRoute)
```

## 5 个值的语义

| 值 | 行为 |
|---|---|
| `'last'` | 跳到上次的 `lastRoute`(`getStartupRouteLocation(lastRoute)`) |
| `'home'` | 返回 null,触发默认 home 路由(什么都不做) |
| `'dashboard'` | `{ name: 'dashboard' }` |
| `'navigator'` | `{ name: 'navigator' }`,空白 navigator,等用户导航 |

## 如何加 `customPath`

**最干净的位置**:`resolveStartupRouteLocation` 加 case,但它只能返回 route,不能直接打开路径。

**正确做法**:在 `applyStartupPage` (use-init.ts:225) 加分支,因为这里能同时拿到 router 和 userSettingsStore:

```typescript
if (startupPage === 'customPath') {
  const customPath = userSettingsStore.userSettings.customStartupPath;
  if (customPath) {
    openNavigatorPath(router, customPath);
  }
  return;
}
```

`resolveStartupRouteLocation` 的 switch 仍然要加 `'customPath'` case 满足 exhaustive check,但返回 null(`'home'` 模式)。

## 已有可复用 API

- `openNavigatorPath(router, path)` 在 `src/utils/open-navigator-directory.ts:96` — 已经做过路径检查和异步加载
- `openNavigatorPathInNewTab(router, path)` 在同文件 :88 — 新标签页打开

## 用户设置存储

需要加 `customStartupPath: string` 字段:
- 类型扩展:`src/types/user-settings.ts`
- 默认值:`src/stores/storage/user-settings.ts`
- 持久化:已经走 `userSettingsStore.set()` / `userSettings.getAll()`,无需额外机制

## i18n

i18n key 都在 `startup-page.vue` 的 `pageOptions` 里用,如 `t('settings.general.startupPage.last')`。新增的 'customPath' 选项同样遵循。

需要新增的 i18n key(在 `locales/*.json`):
- `settings.general.startupPage.customPath` → "Custom path..."
- `settings.general.startupPage.customPathPlaceholder` → "Pick a directory to open at startup"
- `settings.general.startupPage.browse` → "Browse..."

## 测试覆盖

- `src/composables/__tests__/use-init.test.ts` 有 startup page 的现有测试(line 80, 373, 684, 713, 749)
- 需要新增 test:
  - `applyStartupPage` 在 `startupPage='customPath'` 且 `customStartupPath` 有值时调用 `openNavigatorPath`
  - 在 `customStartupPath` 为空时降级到默认 home