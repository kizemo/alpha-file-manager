# 实现计划 —— 首次启动按系统语言设置默认界面语言

**修订**:2026-10-09 23:35,按用户指定的三点修正(简体严格映射 / 繁体回退英文 / 仅首次生效)

**需求**:软件安装后**第一次启动**时,根据运行环境的系统语言自动设置默认界面语言(本机中文 → 界面中文)。
**已定策略**(用户已确认):
1. `zh-CN` → `ch`(严格做好映射)
2. `zh-TW` / `zh-HK` **回退英文**(不归简体包)
3. **仅首次启动生效**,之后再启动绝不调整
4. 系统语言不在 17 个语言包内 → 回退英文

**不在本计划范围**:默认文件管理器开关(需单独确认,见 §6)

---

## 0. 现状(已勘察确认,非推测)

| 事实 | 位置 |
|---|---|
| i18n 硬编码 `locale: 'en'` | `src/localization/index.ts:10` |
| 用户设置内存默认 `language: { locale: 'en' }` | `src/stores/storage/user-settings.ts:72-77` |
| 启动时用存档覆盖 i18n | `user-settings.ts:624` `initLanguage()` |
| 语言包 17 个,中文的 locale code 是 **`ch`** | `src/localization/messages/`,`data.ts:113` |
| `@tauri-apps/plugin-os ^2.4.0` 已在依赖 | `package.json:57` |
| 插件已在 Rust 侧注册 | `src-tauri/src/lib.rs:256-271` |
| 前端已 import 过该插件 | `src/stores/storage/system-icon-cache.ts:6` |

**结论:零新增依赖**,纯前端改动。

---

## 1. ⚠️ 三个必须避开的坑

### 坑 1:app 的中文 locale code 是 `ch`,不是 `zh-CN`

`plugin-os` 的 `locale()` 在中文 Windows 返回 `zh-CN`。直接赋给 `i18n.global.locale` 会**找不到语言包**。必须经过映射。

### 坑 2:繁体不能回退到简体包

`zh-TW` / `zh-HK` / `zh-Hant` 若按主语言匹配会落到 `zh` → `ch`(简体),得到简繁混杂界面。
按用户要求:**繁体一律回退英文**。这要求匹配算法**不能只取主语言**——见 §2.2。

### 坑 3:绝不能每次启动都重跑

若不设一次性标记,用户手动改成英文后**重启会被打回中文**。
必须用一个**独立的一次性标记键**,而不是靠「存档有没有 language」这种隐式判断。

---

## 2. 实现设计

### 2.1 改动清单(4 个文件)

| # | 文件 | 动作 |
|---|---|---|
| 1 | `src/localization/system-locale.ts` | 新建:映射表 + 解析纯函数 |
| 2 | `src/localization/__tests__/system-locale.test.ts` | 新建:单测 |
| 3 | `src/stores/storage/user-settings.ts` | 改:首次启动逻辑 |
| 4 | `src/stores/storage/__tests__/` | 改/新建:首次启动行为测试 |

### 2.2 映射表与匹配算法

```ts
import type { AppLocale } from './data';

/** 不在 17 个语言包内时的回退目标(用户已确认:英文)。 */
export const FALLBACK_APP_LOCALE: AppLocale = 'en';

/** 精确匹配表。键为规范化后(小写)的完整 locale 串。
 *  ⚠️ 中文必须走精确匹配:繁体的 zh-tw / zh-hk / zh-hant 刻意【不】列在这里,
 *  它们会被下面的特殊分支挡掉并回退英文。 */
const EXACT: Record<string, AppLocale> = {
  // —— 简体中文(仅这四个) ——
  'zh': 'ch',
  'zh-cn': 'ch',
  'zh-sg': 'ch',
  'zh-hans': 'ch',
  // —— 其余语言,按主语言匹配 ——
  en: 'en', ru: 'ru', es: 'es', de: 'de', fr: 'fr', tr: 'tr',
  ja: 'ja', fa: 'fa', vi: 'vi', it: 'it', pt: 'pt', sl: 'sl',
  hi: 'hi', ur: 'ur', he: 'he', hy: 'hy',
};

export function resolveAppLocaleFromSystem(
  systemLocale: string | null | undefined,
): AppLocale {
  const normalized = (systemLocale ?? '').trim().toLowerCase().replace(/_/g, '-');
  if (!normalized) return FALLBACK_APP_LOCALE;

  // 1) 完整串精确匹配 —— zh-cn / zh-hans 在这里命中简体
  if (EXACT[normalized]) return EXACT[normalized];

  const language = normalized.split('-')[0];

  // 2) ⚠️ 中文特殊分支:走到这里说明是 zh-tw / zh-hk / zh-hant / zh-mo 等繁体,
  //    刻意【不】回退到 zh 的简体映射,直接回退英文。
  if (language === 'zh') return FALLBACK_APP_LOCALE;

  // 3) 其余语言按主语言匹配:en-us → en、pt-br → pt、fa-ir → fa
  return EXACT[language] ?? FALLBACK_APP_LOCALE;
}
```

**为什么这样写**:第 2 步那个 `zh` 特判是整个设计的核心。没有它,`zh-TW` 会走到第 3 步命中 `EXACT['zh']` → `ch`,得到简繁混杂界面——而这正是需求 2 要避免的。

### 2.3 「仅首次启动」的一次性标记

**不复用**「存档有没有 language 键」这种隐式判断,改为独立标记,语义明确且便于后续扩展:

```ts
const FIRST_RUN_MARKER_KEY = 'appDefaultsInitialized';

async function applyFirstRunDefaults() {
  const storage = userSettingsStorage.value;
  if (!storage) return;

  // 已初始化过 → 永不重跑(升级安装的用户也不会被改动)
  if (await storage.get(FIRST_RUN_MARKER_KEY)) return;

  try {
    const appLocale = resolveAppLocaleFromSystem(await locale());
    userSettings.value.language = getLanguage(appLocale) ?? englishDefault;
  }
  catch (error) {
    // 检测失败就保持英文默认值,绝不让启动流程失败
    console.error('[UserSettings] System locale detection failed:', error);
  }

  // 无论成功与否都要落标记,否则每次启动都会重试
  await storage.set(FIRST_RUN_MARKER_KEY, true);
}
```

调用点(`init()` 内,`initLanguage()` 之前):

```ts
initTheme();
themeTransitionsEnabled.value = true;
await ensureThemeChangeListener();
await applyFirstRunDefaults();   // ← 新增
initLanguage();
await initZoom();
```

**时序说明**:`init()` 本就是全 async 且已有多个 `await`,插入一次毫秒级 IPC 不改变结构,不额外推迟首屏。

**升级安装的行为**:标记随数据目录持久化,升级后仍为 `true`,所以升级用户**不会被改动**——符合「安装后第一次启动」的本意。

> ⚠️ **实施时需确认**:`user-settings.ts` 里存在 `allowedUserSettingsStorageKeys` 白名单(`:70`)。新增键可能需要一并加入白名单,否则写入会被拦截或不被持久化。这是本计划唯一的未知点,实施第一步就要验证。

---

## 3. 测试计划

### 3.1 单元测试 `system-locale.test.ts`

| 用例 | 输入 | 期望 |
|---|---|---|
| **简体精确** | `zh-CN` | `ch` |
| **简体裸串** | `zh` | `ch` |
| **简体其它地区** | `zh-SG`、`zh-Hans` | `ch` |
| **⚠️ 繁体回退** | `zh-TW`、`zh-HK`、`zh-MO`、`zh-Hant` | `en` |
| 大小写不敏感 | `EN-us`、`ZH-cn` | `en` / `ch` |
| 带地区/脚本 | `pt-BR`、`en-GB` | `pt` / `en` |
| 无对应语言包 | `ko-KR`、`nl-NL`、`pl-PL` | `en` |
| 下划线分隔 | `pt_BR` | `pt` |
| **空值/畸形** | `''`、`null`、`undefined`、`'-'`、`'  '` | `en` |
| RTL | `fa-IR`、`ur-PK`、`he-IL` | `fa`、`ur`、`he` |
| 非拉丁 | `hy-AM`、`hi-IN`、`ja-JP` | `hy`、`hi`、`ja` |

### 3.2 集成测试(首次启动行为)

| 场景 | 断言 |
|---|---|
| 标记不存在 + 系统 `zh-CN` | 语言被设为 `ch`,标记写为 `true` |
| 标记不存在 + 系统 `ko-KR` | 语言回退 `en` |
| **标记已存在** | **完全不改动语言**(核心防回归) |
| 标记不存在 + `locale()` 抛错 | 保持 `en`,**不阻断启动**,标记仍写为 `true` |
| 标记不存在 + 用户已有语言存档 | 见下方说明 |

> **一处待你拍板的边界**:若用户**已手动改过语言**、但标记键还不存在(例如从旧版本升级上来),
> 应当「保留用户设置」还是「按系统语言覆盖」?
> 我倾向**保留用户设置**(优先读 `language` 键,有则不动),因为那是用户明确表达过的意愿。
> 这条会写进验收。

### 3.3 真机验收(不可省略)

1. 中文 Windows **全新用户**(清空数据目录)首次启动 → 界面为**中文**
2. 手动改成英文 → **重启后仍为英文** ⭐ 这一条是坑 3 的唯一验收方式,单测测不到
3. 繁体系统(若有条件)首次启动 → 界面为**英文**
4. RTL 语言切换后 `dir="rtl"` 与布局镜像正常

---

## 4. 风险清单

| 风险 | 评估 | 处置 |
|---|---|---|
| 覆盖用户已有语言设置 | **高** | 一次性标记 + 优先读存档,列入真机必测 |
| 繁体回落到简体包 | **高** | `zh` 特判分支,单测锁定 4 个繁体用例 |
| locale code 写成 `zh-CN` 导致无语言包 | **高** | 映射表强制转换,单测锁定 |
| 存储键白名单拦截写入 | **中** | 实施第一步验证 `allowedUserSettingsStorageKeys` |
| 插件调用抛错拖垮启动 | 中 | `try/catch` 吞掉并保持英文 |
| 引入新依赖 | 无 | `plugin-os` 早已在用 |

---

## 5. 工作量与验收

约 1–2 小时,含单测与集成测试。改动面小、无风险外溢。

**验收标准**

1. `resolveAppLocaleFromSystem` 单测全绿(含 4 个繁体回退用例 + 空值畸形)
2. 标记已存在时不改动语言 —— 有集成测试断言
3. 中文 Windows 全新用户首次启动界面为中文
4. 手动改英文后重启**仍为英文**
5. `vue-tsc --build` 无输出;eslint 无新增问题

---

## 6. ⚠️ 待确认:「设为默认文件资源管理器」

**勘察结论:该功能已完整存在,无需新建。**

| 已有实现 | 位置 |
|---|---|
| 附图那一页 UI | `src/modules/settings/ui/categories/experimental/default-file-manager.vue` |
| 前端封装 | `src/utils/default-file-manager.ts` |
| Rust 注册表实现 | `src-tauri/src/default_file_manager.rs` + `default_file_manager/registry.rs` |
| 测试 | `experimental/default-file-manager.test.ts` |
| Tauri 命令 | `default_file_manager_available` / `is_default_file_manager` / `set_default_file_manager` |

**它的状态是实时读系统注册表的**(组件 `onMounted` 调 `isDefaultFileManager()`),
**不走 userSettings**,所以目前**没有「默认值」概念**——开关显示的就是系统当前状态。

若要让它在首次启动自动开启,技术上只需在首次启动时调用
`setDefaultFileManager(true)`,接口已就绪。**但这属于重大决策,需你确认后再做**,理由见随附说明。