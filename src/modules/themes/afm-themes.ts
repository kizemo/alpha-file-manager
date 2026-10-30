// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-10-29（AFM UI 重设计）：三套品牌重设计主题。
//
// 设计稿来源：`.superpowers/sdd/design-demos/` 下的 D1 / D2 / D3（真实产品骨架上的三种皮肤），
// 方向确认见同目录 `direction-approved.md`（第 6 节记录了用户逐字选择原话）。
//
// 约束（改这个文件前务必先读下面几条说明）：
//
// 1. 所有颜色 token 必须是裸 HSL 三元组（`240 30% 99%`），**不能自带 alpha**。
//    全仓有大量 `hsl(var(--token) / N%)` 的「二次加 alpha」写法。
//    唯一例外是 `--popover` 与 `--input`。
//
// 2. 只写颜色 token 与形状 token。带 FORK-MODIFICATION 注释的**尺寸类变量一律不动**
//    （`--nav-sidebar-width: 42px` 是用户明确要求保留的硬约束；
//      `--navigator-list-view-entry-height: 42px` 是运行时注入判据）。
//
// 3. 深色变体必须覆盖 `.dark` 块里**全部**颜色 token。
//
// 4. 落地方式是「基础明暗 class + CSS 变量覆盖」，与扩展主题走同一条通道：
//    `use-theme.ts` 切 `documentElement.classList` 的 `dark`，再把 variables 写成内联样式。
//
// 5. **可读性是硬门槛**：正文与小字对比度必须 ≥ 4.5:1。
//    设计稿初版的三个值没过线（暖纸浅色的三级文字、跃动活力版的珊瑚红与柠檬黄），
//    已由 `.superpowers/sdd/afm-palette-solve.cjs` 求解出等色相的合规色再落进下面的 token。
//
// 2026-10-30 主题精简之后，这三套是内置主题里唯一「带 CSS 变量覆盖」的一组，
// id 前缀 `afm-`，与经典主题（暗色 / 跟随系统）并存，可在设置里切换。

export type AfmThemeId
  = | 'afm-warm-light'
    | 'afm-warm-dark'
    | 'afm-vivid-light'
    | 'afm-cool-light';

export type AfmThemeDefinition = {
  id: AfmThemeId;
  /** 决定 `documentElement` 是否带上 `dark` class（即 `.dark` 里的基础 token）。 */
  baseTheme: 'light' | 'dark';
  /**
   * 写入 `<html data-afm-theme="...">` 的值。
   * 少量「token 表达不了的形状特征」（衬线小标题、彩色左边条、彩色图标块）
   * 靠这个属性在全局样式表 `src/styles/afm-themes.css` 里做作用域覆盖。
   */
  cssThemeId: string;
  /** 覆盖用的 CSS 变量集合。 */
  variables: Record<`--${string}`, string>;
};

export const AFM_THEME_IDS: AfmThemeId[] = [
  'afm-warm-light',
  'afm-warm-dark',
  'afm-vivid-light',
  'afm-cool-light',
];

const AFM_THEME_ID_SET = new Set<string>(AFM_THEME_IDS);

export function isAfmThemeId(theme: string): theme is AfmThemeId {
  return AFM_THEME_ID_SET.has(theme);
}

/**
 * 衬线字族：暖纸两套用它做小标题，让「编辑式」气质落到字上而不只是落到颜色上。
 * 放在 token 里而不是写死在样式表里，是为了让「换字体」也走同一条变量通道。
 */
const SERIF_FONT_STACK
  = '"Iowan Old Style", "Palatino Linotype", Palatino, Georgia, "Songti SC", "Source Han Serif SC", "Noto Serif CJK SC", serif';

/** 无衬线字族：跃动活力版用它，保证小字号下的清晰度。 */
const SANS_FONT_STACK
  = 'system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", "PingFang SC", "Microsoft YaHei", sans-serif';

// ---------------------------------------------------------------------------
// D1 —— 暖纸（浅色）
//
// 暖米色纸底 + 墨色正文 + 赤陶点缀 + 品牌钢蓝。近直角卡片，
// 品牌钢蓝落在 `--primary` 上，于是图标轨激活图标、主按钮、焦点环自然带上品牌色。
// ---------------------------------------------------------------------------
const AFM_WARM_LIGHT_VARIABLES: Record<`--${string}`, string> = {
  '--background': '38.2 40.7% 94.7%',
  '--background-2': '40 39.6% 89.6%',
  '--background-3': '39.2 41.9% 87.8%',
  '--window-toolbar': '39.2 41.9% 87.8%',
  '--icon': '32.7 11.5% 37.6%',
  '--foreground': '30 16.7% 14.1%',
  '--muted': '39.1 34.3% 86.9%',
  '--muted-foreground': '32.7 11.5% 37.6%',
  '--popover': '45 57.1% 97.3%',
  '--popover-foreground': '30 16.7% 14.1%',
  '--card': '45 57.1% 97.3%',
  '--card-foreground': '30 16.7% 14.1%',
  '--border': '37.8 27.8% 81.0%',
  '--input': '40 39.6% 89.6%',
  '--primary': '200 16.9% 45.3%',
  '--primary-foreground': '0 0% 100%',
  '--secondary': '39.2 41.9% 87.8%',
  '--secondary-foreground': '30 16.7% 14.1%',
  '--destructive': '15.9 45.9% 43.5%',
  '--destructive-foreground': '0 0% 100%',
  '--ring': '200 18.8% 37.6%',
  '--success': '112.7 18.2% 35.5%',
  '--dangerous': '15.9 45.9% 43.5%',
  '--warning': '39.1 50.0% 36.1%',
  '--radius': '0.25rem',
  '--shadow-md': '0 1px 2px rgb(42 36 30 / 6%), 0 4px 12px rgb(42 36 30 / 8%)',
  '--shadow-lg': '0 12px 32px rgb(42 36 30 / 14%)',
  '--backdrop-filter-blur': '32px',
  // 品牌与点缀
  '--brand': '200 16.9% 45.3%',
  '--brand-ink': '200 18.8% 37.6%',
  '--brand-foreground': '0 0% 100%',
  '--accent': '15.9 45.9% 43.5%',
  '--accent-foreground': '0 0% 100%',
  '--accent-ink': '15.9 45.9% 42.5%',
  '--accent-muted': '19.3 41.2% 86.7%',
  // 左侧图标轨（用户要求保持 42px 宽，这里只提供颜色）
  '--nav-rail-background': '30 16.7% 14.1%',
  '--nav-rail-foreground': '39.1 34.3% 86.9%',
  '--nav-rail-muted-foreground': '41.1 15.8% 56.7%',
  // 层次面 / 状态层 / 描边 / 标签 / 滚动条
  '--surface-sidebar': '30 16.7% 14.1%',
  '--surface-panel': '45 57.1% 97.3%',
  '--surface-stripe': '39.1 43.4% 89.6%',
  '--state-hover': '40 39.6% 89.6%',
  '--state-selected': '41.1 38.8% 90.4%',
  '--state-active': '39.3 27.6% 79.4%',
  '--border-subtle': '37.8 27.8% 81.0%',
  '--tab-surface-active': '45 57.1% 97.3%',
  '--tab-indicator': '112.7 18.2% 35.5%',
  '--scrollbar-track': '40 39.6% 89.6%',
  '--scrollbar-thumb': '39.3 27.6% 79.4%',
  '--scrollbar-thumb-hover': '37.8 27.8% 74%',
  // 字族
  '--font-display': SERIF_FONT_STACK,
  // 线条图标色 / 低饱和点缀色（用户要求：低饱和绿 + 低饱和红）
  '--icon-stroke': '36.4 13% 42%',
  '--moss': '112.7 18.2% 35.5%',
  '--moss-soft': '78.8 22.9% 86.3%',
  '--terra': '15.9 45.9% 43.5%',
  '--terra-soft': '19.3 41.2% 86.7%',

  // 目录 / 文件区隔底色（任务 4）
  '--row-dir-bg': '39.1 43.4% 89.6%',
  '--row-file-bg': '42.9 53.8% 97.5%',

  // 标签栏（任务 2b / 2c / 任务 3）
  '--tab-idle-bg': '40 34.3% 86.3%',
  '--tab-idle-border': '39.3 27.6% 79.4%',

  // 列表头（任务 1c / 2e）
  '--list-header-bg': '39.2 41.9% 84%',
  '--list-header-border': '39.4 23.5% 68%',
  '--list-header-foreground': '32.7 11.5% 37.1%',

  // 右上角功能族分色（任务 1a）
  '--family-split': '173 45% 29%',
  '--family-tree': '113 18% 34.5%',
  '--family-info': '200 17% 37%',
  '--family-more': '31 9% 44%',
  '--family-extension': '251 60% 60%',
  '--family-search': '42 85% 28.5%',
  '--family-create': '173 45% 29%',
  '--family-filter': '349 55% 48%',

  // 地址栏导航按钮分色（任务 1b）
  '--nav-back': '205 15% 38%',
  '--nav-forward': '203 16% 44%',
  '--nav-up': '349 55% 48%',
  '--nav-location': '200 17% 37%',
  '--nav-refresh': '173 45% 29%',
  '--nav-view': '39 50% 34%',
  '--nav-thumbnail': '349 55% 52%',
  '--nav-more': '31 9% 40.5%',
};

// ---------------------------------------------------------------------------
// D2 —— 暖纸（暗色）
//
// 与 D1 完全同构，只把每一档换成暖黑纸底并把赤陶提亮。
// 品牌钢蓝在暗底上要提亮到 `#8FA8B5` 才够对比度。
// ---------------------------------------------------------------------------
const AFM_WARM_DARK_VARIABLES: Record<`--${string}`, string> = {
  '--background': '30 17.4% 9%',
  '--background-2': '30 17.2% 11.4%',
  '--background-3': '27 17% 10.4%',
  '--window-toolbar': '27 17% 10.4%',
  '--icon': '33 14.1% 59.8%',
  '--foreground': '38 42.9% 89%',
  '--muted': '30 15.9% 17.3%',
  '--muted-foreground': '33 14.1% 59.8%',
  '--popover': '34 21.9% 12.5%',
  '--popover-foreground': '38 42.9% 89%',
  '--card': '34 21.9% 12.5%',
  '--card-foreground': '38 42.9% 89%',
  '--border': '30 16% 19.6%',
  '--input': '30 15.9% 17.3%',
  '--primary': '201 20.4% 63.5%',
  '--primary-foreground': '30 17.4% 9%',
  '--secondary': '27 17% 10.4%',
  '--secondary-foreground': '38 42.9% 89%',
  '--destructive': '18 55% 62%',
  '--destructive-foreground': '30 17.4% 9%',
  '--ring': '200 27.2% 70.4%',
  '--success': '110 25% 62%',
  '--dangerous': '18 55% 62%',
  '--warning': '38 74% 56%',
  '--radius': '0.25rem',
  '--shadow-md': '0 1px 2px rgb(0 0 0 / 34%), 0 4px 14px rgb(0 0 0 / 38%)',
  '--shadow-lg': '0 14px 36px rgb(0 0 0 / 52%)',
  '--backdrop-filter-blur': '32px',
  '--brand': '201 20.4% 63.5%',
  '--brand-ink': '201 20.4% 63.5%',
  '--brand-foreground': '30 17.4% 9%',
  '--accent': '18 66.3% 63.9%',
  '--accent-foreground': '30 17.4% 9%',
  '--accent-ink': '17 69.7% 71.6%',
  '--accent-muted': '18 30% 20%',
  '--nav-rail-background': '27 17% 7.6%',
  '--nav-rail-foreground': '33 14.1% 59.8%',
  '--nav-rail-muted-foreground': '33 10% 52%',
  '--surface-sidebar': '27 17% 7.6%',
  '--surface-panel': '34 21.9% 12.5%',
  '--surface-stripe': '30 17% 11%',
  '--state-hover': '30 15.9% 14%',
  '--state-selected': '30 15.9% 17.3%',
  '--state-active': '30 15.9% 20%',
  '--border-subtle': '30 16% 16%',
  '--tab-surface-active': '34 21.9% 12.5%',
  '--tab-indicator': '110 25% 62%',
  '--scrollbar-track': '27 17% 10%',
  '--scrollbar-thumb': '30 14% 42%',
  '--scrollbar-thumb-hover': '30 16% 52%',
  '--font-display': SERIF_FONT_STACK,
  // 线条图标色 / 低饱和点缀色（用户要求：低饱和绿 + 低饱和红）
  '--icon-stroke': '36 14% 74%',
  '--moss': '110 25% 62%',
  '--moss-soft': '110 18% 24%',
  '--terra': '18 55% 62%',
  '--terra-soft': '18 35% 24%',

  // 目录 / 文件区隔底色（任务 4）
  '--row-dir-bg': '30 15% 13%',
  '--row-file-bg': '34 20% 13%',

  // 标签栏（任务 2b / 2c / 任务 3）
  '--tab-idle-bg': '30 15% 15%',
  '--tab-idle-border': '30 14% 22%',

  // 列表头（任务 1c / 2e）
  '--list-header-bg': '28 16% 14%',
  '--list-header-border': '30 14% 24%',
  '--list-header-foreground': '33 14% 70%',

  // 右上角功能族分色（任务 1a）
  '--family-split': '173 45% 50%',
  '--family-tree': '113 18% 50%',
  '--family-info': '200 17% 50%',
  '--family-more': '31 9% 50%',
  '--family-extension': '251 60% 60.5%',
  '--family-search': '42 85% 50%',
  '--family-create': '173 45% 50%',
  '--family-filter': '349 55% 55.5%',

  // 地址栏导航按钮分色（任务 1b）
  '--nav-back': '205 15% 51.5%',
  '--nav-forward': '203 16% 50%',
  '--nav-up': '349 55% 55.5%',
  '--nav-location': '200 17% 50%',
  '--nav-refresh': '173 45% 50%',
  '--nav-view': '39 50% 50%',
  '--nav-thumbnail': '349 55% 51.5%',
  '--nav-more': '31 9% 50%',
};

// ---------------------------------------------------------------------------
// D3 —— 跃动活力（浅色）
//
// 冷调浅底 + 纯白面板与卡片 + 电光紫 / 跳跳青 / 柠檬 / 珊瑚四色点缀。
// 12px 圆角，彩色左边条与彩色图标块由 `src/styles/afm-themes.css` 按
// `html[data-afm-theme='afm-vivid-light']` 作用域落地。
// 左侧图标轨换成深钢蓝底（品牌色系），白图标压在上面 —— 这是本套最有辨识度的一处。
// ---------------------------------------------------------------------------
const AFM_VIVID_LIGHT_VARIABLES: Record<`--${string}`, string> = {
  '--background': '220 21.4% 94.5%',
  '--background-2': '220 27.3% 91.4%',
  '--background-3': '218 28.6% 94.5%',
  '--window-toolbar': '218 28.6% 94.5%',
  '--icon': '220 10.3% 39.8%',
  '--foreground': '225 13.3% 11.8%',
  '--muted': '218 28.6% 94.5%',
  '--muted-foreground': '220 10.3% 39.8%',
  '--popover': '0 0% 100%',
  '--popover-foreground': '225 13.3% 11.8%',
  '--card': '0 0% 100%',
  '--card-foreground': '225 13.3% 11.8%',
  '--border': '220 27.3% 91.4%',
  '--input': '218 28.6% 94.5%',
  '--primary': '251 100% 63.9%',
  '--primary-foreground': '0 0% 100%',
  '--secondary': '218 28.6% 94.5%',
  '--secondary-foreground': '225 13.3% 11.8%',
  '--destructive': '349 55.6% 45%',
  '--destructive-foreground': '0 0% 100%',
  '--ring': '251 100% 63.9%',
  '--success': '172 100% 25.1%',
  '--dangerous': '349 55.6% 45%',
  '--warning': '42 61.5% 35.7%',
  '--radius': '0.75rem',
  '--shadow-md': '0 1px 2px rgb(26 28 34 / 5%), 0 6px 18px rgb(26 28 34 / 8%)',
  '--shadow-lg': '0 16px 40px rgb(26 28 34 / 16%)',
  '--backdrop-filter-blur': '32px',
  '--brand': '200 16.9% 45.3%',
  '--brand-ink': '200 16.9% 41%',
  '--brand-foreground': '0 0% 100%',
  '--accent': '251 100% 63.9%',
  '--accent-foreground': '0 0% 100%',
  '--accent-ink': '252 79.3% 56.5%',
  '--accent-muted': '251 100% 95%',
  '--nav-rail-background': '201 21.5% 25.5%',
  '--nav-rail-foreground': '210 20% 92%',
  '--nav-rail-muted-foreground': '210 14% 70%',
  '--surface-sidebar': '201 21.5% 25.5%',
  '--surface-panel': '0 0% 100%',
  '--surface-stripe': '220 27.3% 91.4%',
  '--state-hover': '220 27.3% 91.4%',
  '--state-selected': '251 100% 95%',
  '--state-active': '218 28.6% 87%',
  '--border-subtle': '220 27.3% 88%',
  '--tab-surface-active': '0 0% 100%',
  '--tab-indicator': '251 100% 63.9%',
  '--scrollbar-track': '218 28.6% 88%',
  '--scrollbar-thumb': '220 20% 46%',
  '--scrollbar-thumb-hover': '220 24% 36%',
  '--font-display': SANS_FONT_STACK,
  // 线条图标色 / 低饱和点缀色（用户要求：低饱和绿 + 低饱和红）
  '--icon-stroke': '220 10.3% 39.8%',
  '--moss': '150 30% 32%',
  '--moss-soft': '150 35% 92%',
  '--terra': '349 55.6% 45%',
  '--terra-soft': '349 60% 94%',

  // 目录 / 文件区隔底色（任务 4）
  '--row-dir-bg': '220 27.3% 91.4%',
  '--row-file-bg': '0 0% 100%',

  // 标签栏（任务 2b / 2c / 任务 3）
  '--tab-idle-bg': '220 27.3% 91.4%',
  '--tab-idle-border': '220 27.3% 88%',

  // 列表头（任务 1c / 2e）
  '--list-header-bg': '220 26% 89%',
  '--list-header-border': '220 24% 78%',
  '--list-header-foreground': '220 10.3% 39.8%',

  // 右上角功能族分色（任务 1a）
  '--family-split': '173 45% 29.5%',
  '--family-tree': '113 18% 35.5%',
  '--family-info': '200 17% 38%',
  '--family-more': '31 9% 45%',
  '--family-extension': '251 60% 60%',
  '--family-search': '42 85% 29.5%',
  '--family-create': '173 45% 29.5%',
  '--family-filter': '349 55% 49%',

  // 地址栏导航按钮分色（任务 1b）
  '--nav-back': '205 15% 39%',
  '--nav-forward': '203 16% 45%',
  '--nav-up': '349 55% 49%',
  '--nav-location': '200 17% 38%',
  '--nav-refresh': '173 45% 29.5%',
  '--nav-view': '39 50% 35%',
  '--nav-thumbnail': '349 55% 53%',
  '--nav-more': '31 9% 41.5%',
};

// ---------------------------------------------------------------------------
// D4 —— 冷杉（浅色，静态固定）
//
// 「跟随系统」不再是可选项之后，需要一套静态冷色主题顶替它的位置：
// 冷调蓝绿底 + 品牌钢蓝主色 + 近直角卡片 + 无衬线工具感字族。
// 品牌钢蓝落在 --brand 上（R2：每套主题至少一处承载品牌主色 #607A87）。
// baseTheme 固定 light —— 这套主题**不跟随系统**，明暗都由它自己决定。
// ---------------------------------------------------------------------------
const AFM_COOL_LIGHT_VARIABLES: Record<`--${string}`, string> = {
  '--background': '210 30% 96%',
  '--background-2': '210 28% 92%',
  '--background-3': '210 30% 94%',
  '--window-toolbar': '210 30% 94%',
  '--icon': '205 16% 38%',
  '--foreground': '205 24% 14%',
  '--muted': '210 26% 90%',
  '--muted-foreground': '205 14% 40%',
  '--popover': '210 40% 99%',
  '--popover-foreground': '205 24% 14%',
  '--card': '210 40% 99%',
  '--card-foreground': '205 24% 14%',
  '--border': '210 24% 84%',
  '--input': '210 26% 90%',
  '--primary': '200 16.9% 45.3%',
  '--primary-foreground': '0 0% 100%',
  '--secondary': '210 26% 90%',
  '--secondary-foreground': '205 24% 14%',
  '--destructive': '4 62% 45%',
  '--destructive-foreground': '0 0% 100%',
  '--ring': '200 16.9% 45.3%',
  '--success': '174 60% 30%',
  '--dangerous': '4 62% 45%',
  '--warning': '38 70% 42%',
  '--radius': '0.25rem',
  '--shadow-md': '0 1px 2px rgb(20 30 38 / 6%), 0 4px 12px rgb(20 30 38 / 8%)',
  '--shadow-lg': '0 12px 32px rgb(20 30 38 / 14%)',
  '--backdrop-filter-blur': '32px',
  '--brand': '200 16.9% 45.3%',
  '--brand-ink': '200 16.9% 41%',
  '--brand-foreground': '0 0% 100%',
  '--accent': '174 55% 32%',
  '--accent-foreground': '0 0% 100%',
  '--accent-ink': '174 50% 28%',
  '--accent-muted': '174 40% 92%',
  '--nav-rail-background': '205 24% 24%',
  '--nav-rail-foreground': '200 30% 92%',
  '--nav-rail-muted-foreground': '200 18% 70%',
  '--surface-sidebar': '205 24% 24%',
  '--surface-panel': '210 40% 99%',
  '--surface-stripe': '210 28% 93%',
  '--state-hover': '210 28% 93%',
  '--state-selected': '200 30% 90%',
  '--state-active': '210 26% 87%',
  '--border-subtle': '210 22% 88%',
  '--tab-surface-active': '210 40% 99%',
  '--tab-indicator': '200 16.9% 45.3%',
  '--scrollbar-track': '210 28% 92%',
  '--scrollbar-thumb': '205 18% 48%',
  '--scrollbar-thumb-hover': '205 22% 38%',
  '--font-display': SANS_FONT_STACK,
  // 线条图标色 / 低饱和点缀色（用户要求：低饱和绿 + 低饱和红）
  '--icon-stroke': '205 16% 38%',
  '--moss': '160 35% 32%',
  '--moss-soft': '160 35% 93%',
  '--terra': '4 62% 45%',
  '--terra-soft': '4 55% 94%',

  // 目录 / 文件区隔底色（任务 4）
  '--row-dir-bg': '210 28% 94%',
  '--row-file-bg': '210 40% 99%',

  // 标签栏（任务 2b / 2c / 任务 3）
  '--tab-idle-bg': '210 26% 90%',
  '--tab-idle-border': '210 24% 84%',

  // 列表头（任务 1c / 2e）
  '--list-header-bg': '210 28% 90%',
  '--list-header-border': '210 24% 78%',
  '--list-header-foreground': '205 14% 40%',

  // 右上角功能族分色（任务 1a）
  '--family-split': '173 45% 30.5%',
  '--family-tree': '113 18% 36.5%',
  '--family-info': '200 17% 39%',
  '--family-more': '31 9% 46%',
  '--family-extension': '251 60% 60%',
  '--family-search': '42 85% 30%',
  '--family-create': '173 45% 30.5%',
  '--family-filter': '349 55% 50%',

  // 地址栏导航按钮分色（任务 1b）
  '--nav-back': '205 15% 40%',
  '--nav-forward': '203 16% 46.5%',
  '--nav-up': '349 55% 50%',
  '--nav-location': '200 17% 39%',
  '--nav-refresh': '173 45% 30.5%',
  '--nav-view': '39 50% 35.5%',
  '--nav-thumbnail': '349 55% 54.5%',
  '--nav-more': '31 9% 42.5%',
};

export const AFM_THEMES: AfmThemeDefinition[] = [
  {
    id: 'afm-warm-light',
    baseTheme: 'light',
    cssThemeId: 'afm-warm-light',
    variables: AFM_WARM_LIGHT_VARIABLES,
  },
  {
    id: 'afm-warm-dark',
    baseTheme: 'dark',
    cssThemeId: 'afm-warm-dark',
    variables: AFM_WARM_DARK_VARIABLES,
  },
  {
    id: 'afm-vivid-light',
    baseTheme: 'light',
    cssThemeId: 'afm-vivid-light',
    variables: AFM_VIVID_LIGHT_VARIABLES,
  },
  {
    id: 'afm-cool-light',
    baseTheme: 'light',
    cssThemeId: 'afm-cool-light',
    variables: AFM_COOL_LIGHT_VARIABLES,
  },
];

export function getAfmTheme(themeId: string): AfmThemeDefinition | undefined {
  return AFM_THEMES.find(theme => theme.id === themeId);
}
