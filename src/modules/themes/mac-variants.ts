// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-10-29（AFM 3.6.5）：内置 macOS Finder 风格主题变体。
// 三族 × 明暗共六套，全部以内置主题 id 形式提供，用户在「设置 → 主题」里自由切换。
//
// 设计约束（改这个文件前务必先读，踩过的坑都在这里）：
//
// 1. 所有颜色 token 必须是裸 HSL 三元组（`240 30% 99%`），**不能自带 alpha**。
//    全仓有大量 `hsl(var(--token) / N%)` 的「二次加 alpha」写法，例如
//    `hsl(var(--background) / 80%)`。token 一旦自带 alpha，这些声明会变成
//    `hsl(240 30% 99% / 78% / 80%)` 这种非法值而被浏览器整体丢弃，
//    表现为「深色模式下卡片没有背景色」。
//    唯一例外是 `--popover` 与 `--input` —— 全仓没有对它们做二次加 alpha，
//    所以它们可以带 alpha（半透明玻璃主题就靠这两个透出底图）。
//
// 2. 只写颜色 token 与形状 token（`--radius` / `--shadow-md` / `--shadow-lg` /
//    `--backdrop-filter-blur`）。带 FORK-MODIFICATION 注释的**尺寸类变量一律不动**
//    （`--navigator-list-view-entry-height: 42px` 是运行时注入判据）。
//
// 3. 深色变体必须覆盖 `.dark` 块里**全部**颜色 token。若漏掉某个，
//    它会从 `:root` 继承浅色值，深色主题就会露出浅色碎片。
//
// 4. 落地方式是「基础明暗 class + CSS 变量覆盖」，与扩展主题（extension theme）
//    走同一条通道：`use-theme.ts` 会先切 `documentElement.classList` 的 `dark`，
//    再把这些 variables 写成内联样式。所以 `.dark` 选择器仍然生效，
//    `infusion.vue` 与 `dashboard-hero.vue` 里那两条 `.dark` 规则不用动。

export type MacVariantThemeId
  = | 'mac-classic-light'
    | 'mac-classic-dark'
    | 'mac-modern-light'
    | 'mac-modern-dark'
    | 'mac-glass-light'
    | 'mac-glass-dark';

export type MacVariantThemeDefinition = {
  id: MacVariantThemeId;
  /** 决定 `documentElement` 是否带上 `dark` class（即 `.dark` 里的基础 token）。 */
  baseTheme: 'light' | 'dark';
  /** 覆盖用的 CSS 变量集合。 */
  variables: Record<`--${string}`, string>;
};

export const MAC_VARIANT_THEME_IDS: MacVariantThemeId[] = [
  'mac-classic-light',
  'mac-classic-dark',
  'mac-modern-light',
  'mac-modern-dark',
  'mac-glass-light',
  'mac-glass-dark',
];

const MAC_VARIANT_THEME_ID_SET = new Set<string>(MAC_VARIANT_THEME_IDS);

export function isMacVariantThemeId(theme: string): theme is MacVariantThemeId {
  return MAC_VARIANT_THEME_ID_SET.has(theme);
}

// 变体 A —— 原生还原（macOS Sonoma Finder）
// 纯白窗口、浅灰侧栏、系统蓝主色、发丝级分隔线、圆角偏小、阴影极轻。
const MAC_CLASSIC_LIGHT_VARIABLES: Record<`--${string}`, string> = {
  '--background': '0 0% 100%',
  '--background-2': '240 5% 96%',
  '--background-3': '240 5% 98%',
  '--window-toolbar': '240 5% 96%',
  '--icon': '240 5% 35%',
  '--foreground': '240 6% 10%',
  '--muted': '240 5% 94%',
  '--muted-foreground': '240 4% 45%',
  '--popover': '0 0% 100%',
  '--popover-foreground': '240 6% 10%',
  '--card': '0 0% 100%',
  '--card-foreground': '240 6% 10%',
  '--border': '240 5% 88%',
  '--input': '240 5% 96%',
  '--primary': '211 100% 50%',
  '--primary-foreground': '0 0% 100%',
  '--secondary': '240 5% 95%',
  '--secondary-foreground': '240 6% 20%',
  '--destructive': '0 72% 51%',
  '--destructive-foreground': '0 0% 100%',
  '--ring': '211 100% 50%',
  '--success': '145 70% 35%',
  '--dangerous': '4 82% 58%',
  '--warning': '38 92% 50%',
  '--radius': '0.375rem',
  '--shadow-md': '0 1px 2px rgb(0 0 0 / 4%), 0 4px 12px rgb(0 0 0 / 6%)',
  '--shadow-lg': '0 8px 24px rgb(0 0 0 / 12%)',
};

const MAC_CLASSIC_DARK_VARIABLES: Record<`--${string}`, string> = {
  '--background': '240 6% 11%',
  '--background-2': '240 5% 15%',
  '--background-3': '240 6% 13%',
  '--window-toolbar': '240 5% 14%',
  '--icon': '0 0% 65%',
  '--foreground': '0 0% 92%',
  '--muted': '240 5% 19%',
  '--muted-foreground': '0 0% 62%',
  '--popover': '240 6% 16%',
  '--popover-foreground': '0 0% 92%',
  '--card': '240 6% 15%',
  '--card-foreground': '0 0% 92%',
  '--border': '240 5% 22%',
  '--input': '240 5% 17%',
  '--primary': '211 100% 60%',
  '--primary-foreground': '240 10% 8%',
  '--secondary': '240 5% 20%',
  '--secondary-foreground': '0 0% 85%',
  '--destructive': '0 63% 50%',
  '--destructive-foreground': '0 0% 100%',
  '--ring': '211 100% 60%',
  '--success': '145 55% 45%',
  '--dangerous': '4 82% 60%',
  '--warning': '43 90% 58%',
  '--radius': '0.375rem',
  '--shadow-md': '0 1px 2px rgb(0 0 0 / 30%), 0 4px 14px rgb(0 0 0 / 35%)',
  '--shadow-lg': '0 8px 28px rgb(0 0 0 / 45%)',
};

// 变体 B —— 精致现代（Apple 营销页 / Big Sur+ 视觉）
// 保留 AFM 自己的近黑主色（不换品牌识别），把层次、圆角、阴影做得更「苹果官网」。
const MAC_MODERN_LIGHT_VARIABLES: Record<`--${string}`, string> = {
  '--background': '0 0% 100%',
  '--background-2': '240 8% 97%',
  '--background-3': '240 10% 99%',
  '--window-toolbar': '0 0% 100%',
  '--icon': '240 6% 32%',
  '--foreground': '240 10% 8%',
  '--muted': '240 8% 96%',
  '--muted-foreground': '240 4% 46%',
  '--popover': '0 0% 100%',
  '--popover-foreground': '240 10% 8%',
  '--card': '0 0% 100%',
  '--card-foreground': '240 10% 8%',
  '--border': '240 8% 91%',
  '--input': '240 10% 97%',
  '--primary': '240 12% 10%',
  '--primary-foreground': '0 0% 100%',
  '--secondary': '240 8% 96%',
  '--secondary-foreground': '240 10% 14%',
  '--destructive': '0 84.2% 60.2%',
  '--destructive-foreground': '210 20% 98%',
  '--ring': '240 8% 50%',
  '--success': '174 100% 27%',
  '--dangerous': '4 82% 58%',
  '--warning': '38 70% 50%',
  '--radius': '0.625rem',
  '--shadow-md': '0 2px 6px rgb(0 0 0 / 4%), 0 10px 28px rgb(0 0 0 / 8%)',
  '--shadow-lg': '0 12px 40px rgb(0 0 0 / 14%)',
};

const MAC_MODERN_DARK_VARIABLES: Record<`--${string}`, string> = {
  '--background': '240 8% 7%',
  '--background-2': '240 7% 10%',
  '--background-3': '240 6% 12%',
  '--window-toolbar': '240 7% 9%',
  '--icon': '0 0% 66%',
  '--foreground': '0 0% 94%',
  '--muted': '240 6% 14%',
  '--muted-foreground': '0 0% 60%',
  '--popover': '240 7% 11%',
  '--popover-foreground': '0 0% 94%',
  '--card': '240 6% 13%',
  '--card-foreground': '0 0% 94%',
  '--border': '240 6% 18%',
  '--input': '240 6% 15%',
  '--primary': '0 0% 96%',
  '--primary-foreground': '240 10% 8%',
  '--secondary': '240 6% 16%',
  '--secondary-foreground': '0 0% 88%',
  '--destructive': '0 62.8% 50%',
  '--destructive-foreground': '210 20% 98%',
  '--ring': '0 0% 70%',
  '--success': '174 100% 32%',
  '--dangerous': '4 82% 60%',
  '--warning': '43 70% 56%',
  '--radius': '0.625rem',
  '--shadow-md': '0 2px 6px rgb(0 0 0 / 30%), 0 10px 30px rgb(0 0 0 / 40%)',
  '--shadow-lg': '0 12px 40px rgb(0 0 0 / 55%)',
};

// 变体 C —— 半透明玻璃（Big Sur / Liquid Glass）
// 受约束 1 限制，底面 token 不能带 alpha，所以玻璃感是这样来的：
//   · `--popover` / `--input` 带 alpha（这两个没有二次加 alpha 的用法），
//     浮层因此能透出主页封面图；
//   · `--backdrop-filter-blur` 从 32px 提到 48px，让已有 backdrop-filter 的
//     浮层与网格卡糊得更「厚」；
//   · `--border` 用极淡的冷灰模拟发丝白边，`--card` 比底面亮一档模拟玻璃面板。
const MAC_GLASS_LIGHT_VARIABLES: Record<`--${string}`, string> = {
  '--background': '240 30% 99%',
  '--background-2': '240 30% 97%',
  '--background-3': '240 30% 98%',
  '--window-toolbar': '240 30% 98%',
  '--icon': '240 5% 34%',
  '--foreground': '240 10% 8%',
  '--muted': '240 25% 95%',
  '--muted-foreground': '240 4% 44%',
  '--popover': '240 30% 99% / 72%',
  '--popover-foreground': '240 10% 8%',
  '--card': '240 30% 99%',
  '--card-foreground': '240 10% 8%',
  '--border': '240 20% 88%',
  '--input': '240 25% 96% / 55%',
  '--primary': '211 100% 50%',
  '--primary-foreground': '0 0% 100%',
  '--secondary': '240 25% 95%',
  '--secondary-foreground': '240 8% 18%',
  '--destructive': '0 72% 51%',
  '--destructive-foreground': '0 0% 100%',
  '--ring': '211 100% 50%',
  '--success': '145 70% 35%',
  '--dangerous': '4 82% 58%',
  '--warning': '38 92% 50%',
  '--radius': '0.75rem',
  '--shadow-md': '0 2px 8px rgb(0 0 0 / 5%), 0 12px 32px rgb(0 0 0 / 8%)',
  '--shadow-lg': '0 16px 48px rgb(0 0 0 / 16%)',
  '--backdrop-filter-blur': '48px',
};

const MAC_GLASS_DARK_VARIABLES: Record<`--${string}`, string> = {
  '--background': '240 12% 9%',
  '--background-2': '240 12% 13%',
  '--background-3': '240 12% 11%',
  '--window-toolbar': '240 12% 11%',
  '--icon': '0 0% 68%',
  '--foreground': '0 0% 94%',
  '--muted': '240 12% 17%',
  '--muted-foreground': '0 0% 62%',
  '--popover': '240 12% 15% / 70%',
  '--popover-foreground': '0 0% 94%',
  '--card': '240 12% 14%',
  '--card-foreground': '0 0% 94%',
  '--border': '240 10% 24%',
  '--input': '240 12% 16% / 55%',
  '--primary': '211 100% 62%',
  '--primary-foreground': '240 10% 8%',
  '--secondary': '240 12% 18%',
  '--secondary-foreground': '0 0% 86%',
  '--destructive': '0 63% 52%',
  '--destructive-foreground': '0 0% 100%',
  '--ring': '211 100% 62%',
  '--success': '145 55% 48%',
  '--dangerous': '4 82% 62%',
  '--warning': '43 90% 60%',
  '--radius': '0.75rem',
  '--shadow-md': '0 2px 8px rgb(0 0 0 / 35%), 0 12px 32px rgb(0 0 0 / 45%)',
  '--shadow-lg': '0 16px 48px rgb(0 0 0 / 60%)',
  '--backdrop-filter-blur': '48px',
};

export const MAC_VARIANT_THEMES: MacVariantThemeDefinition[] = [
  {
    id: 'mac-classic-light',
    baseTheme: 'light',
    variables: MAC_CLASSIC_LIGHT_VARIABLES,
  },
  {
    id: 'mac-classic-dark',
    baseTheme: 'dark',
    variables: MAC_CLASSIC_DARK_VARIABLES,
  },
  {
    id: 'mac-modern-light',
    baseTheme: 'light',
    variables: MAC_MODERN_LIGHT_VARIABLES,
  },
  {
    id: 'mac-modern-dark',
    baseTheme: 'dark',
    variables: MAC_MODERN_DARK_VARIABLES,
  },
  {
    id: 'mac-glass-light',
    baseTheme: 'light',
    variables: MAC_GLASS_LIGHT_VARIABLES,
  },
  {
    id: 'mac-glass-dark',
    baseTheme: 'dark',
    variables: MAC_GLASS_DARK_VARIABLES,
  },
];

export function getMacVariantTheme(
  themeId: string,
): MacVariantThemeDefinition | undefined {
  return MAC_VARIANT_THEMES.find(theme => theme.id === themeId);
}
