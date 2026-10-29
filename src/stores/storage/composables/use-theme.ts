// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  ref, computed, type Ref, type ComputedRef, watchEffect,
} from 'vue';
import type { BuiltinThemeId, Theme } from '@/types/user-settings';
import { DEFAULT_THEME_ID, findThemeOption, parseThemeId } from '@/modules/themes/registry';
import { useExtensionsStorageStore } from '@/stores/storage/extensions';

export type ThemeTransitionOrigin = {
  x: number;
  y: number;
};

type ViewTransitionDocument = Document & {
  startViewTransition?: (callback: () => void) => {
    ready: Promise<void>;
    skipTransition: () => void;
  };
};

const THEME_TRANSITION_DURATION_MS = 500;
let activeViewTransition: ReturnType<NonNullable<ViewTransitionDocument['startViewTransition']>> | null = null;
let activeViewTransitionAnimation: Animation | null = null;
let activeViewTransitionId = 0;

export function useTheme(
  themeSettingRef: Ref<Theme> | ComputedRef<Theme>,
  transitionOriginRef?: Ref<ThemeTransitionOrigin | null> | ComputedRef<ThemeTransitionOrigin | null>,
  transitionsEnabledRef?: Ref<boolean> | ComputedRef<boolean>,
) {
  const extensionsStorageStore = useExtensionsStorageStore();
  const currentTheme = ref<'light' | 'dark'>('dark');
  const isDark = computed(() => currentTheme.value === 'dark');
  const appliedThemeVariables = new Set<string>();
  let hasAppliedTheme = false;
  let appliedTheme: Theme | null = null;

  function getSystemPreference(): 'light' | 'dark' {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }

  function clearThemeVariables() {
    if (typeof document === 'undefined' || !document.documentElement) {
      return;
    }

    for (const variableName of appliedThemeVariables) {
      document.documentElement.style.removeProperty(variableName);
    }

    appliedThemeVariables.clear();
  }

  function applyBaseTheme(theme: 'light' | 'dark') {
    currentTheme.value = theme;
    document.documentElement.classList.toggle('dark', currentTheme.value === 'dark');
  }

  function resolveBuiltinTheme(theme: BuiltinThemeId): 'light' | 'dark' {
    if (theme === 'system') {
      return getSystemPreference();
    }

    if (theme === 'light' || theme === 'dark') {
      return theme;
    }

    return 'dark';
  }

  function applyThemeVariables(variables: Record<`--${string}`, string>) {
    for (const [variableName, variableValue] of Object.entries(variables)) {
      document.documentElement.style.setProperty(variableName, variableValue);
      appliedThemeVariables.add(variableName);
    }
  }

  /**
   * FORK-MODIFICATION 2026-10-29（AFM UI 重设计）：把主题标识写到 `<html data-afm-theme>`。
   *
   * 少量形状特征（衬线小标题、彩色左边条、彩色图标块）用 CSS 变量表达不了，
   * 靠这个属性在 `src/styles/afm-themes.css` 里做作用域覆盖。
   * 非 AFM 主题必须把属性**删掉**，否则会残留上一套主题的形状特征。
   */
  function applyCssThemeId(cssThemeId: string | undefined) {
    if (typeof document === 'undefined' || !document.documentElement) {
      return;
    }

    if (cssThemeId) {
      document.documentElement.dataset.afmTheme = cssThemeId;
    }
    else {
      delete document.documentElement.dataset.afmTheme;
    }
  }

  function getTransitionOrigin(): ThemeTransitionOrigin {
    return transitionOriginRef?.value ?? {
      x: window.innerWidth,
      y: 0,
    };
  }

  function animateViewTransition(transitionId: number) {
    if (transitionId !== activeViewTransitionId) {
      return;
    }

    const { x, y } = getTransitionOrigin();
    const maxRadius = Math.hypot(
      Math.max(x, window.innerWidth - x),
      Math.max(y, window.innerHeight - y),
    );

    activeViewTransitionAnimation = document.documentElement.animate(
      {
        clipPath: [
          `circle(0px at ${x}px ${y}px)`,
          `circle(${maxRadius}px at ${x}px ${y}px)`,
        ],
      },
      {
        duration: THEME_TRANSITION_DURATION_MS,
        easing: 'ease-in-out',
        pseudoElement: '::view-transition-new(root)',
      },
    );

    activeViewTransitionAnimation.finished
      .catch(() => undefined)
      .finally(() => {
        if (transitionId === activeViewTransitionId) {
          activeViewTransitionAnimation = null;
          activeViewTransition = null;
        }
      });
  }

  function canUseViewTransition(): boolean {
    if (typeof document === 'undefined' || typeof window === 'undefined') {
      return false;
    }

    const viewTransitionDocument = document as ViewTransitionDocument;

    return typeof window.matchMedia === 'function'
      && typeof viewTransitionDocument.startViewTransition === 'function'
      && typeof document.documentElement.animate === 'function'
      && !window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  }

  function cancelActiveViewTransition() {
    activeViewTransitionAnimation?.cancel();
    activeViewTransitionAnimation = null;
    activeViewTransition?.skipTransition();
    activeViewTransition = null;
  }

  function runThemeTransition(applyThemeChange: () => void) {
    if (!canUseViewTransition()) {
      applyThemeChange();
      return;
    }

    cancelActiveViewTransition();

    const viewTransitionDocument = document as ViewTransitionDocument;
    const transitionId = activeViewTransitionId + 1;
    activeViewTransitionId = transitionId;

    try {
      activeViewTransition = viewTransitionDocument.startViewTransition?.(applyThemeChange) ?? null;
    }
    catch {
      activeViewTransition = null;
      applyThemeChange();
      return;
    }

    activeViewTransition?.ready
      .then(() => animateViewTransition(transitionId))
      .catch(() => undefined);
  }

  function applyTheme(theme: Theme) {
    clearThemeVariables();

    const themeOption = findThemeOption(
      theme,
      extensionsStorageStore.extensionsData.installedExtensions,
    );

    if (!themeOption) {
      applyBaseTheme('dark');
      applyCssThemeId(undefined);
      return;
    }

    if (themeOption.source === 'builtin') {
      // FORK-MODIFICATION 2026-10-29：内置变体主题（AFM 品牌三套）
      // 带 CSS 变量覆盖，走「基础明暗 class + 变量覆盖」这条通道，
      // 与扩展主题完全同构，所以 `.dark` 选择器仍然生效。
      if (themeOption.variables && themeOption.baseTheme) {
        applyBaseTheme(themeOption.baseTheme);
        applyThemeVariables(themeOption.variables);
        applyCssThemeId(themeOption.cssThemeId);
        return;
      }

      const parsedTheme = parseThemeId(theme);
      const resolvedTheme = parsedTheme?.source === 'builtin'
        ? resolveBuiltinTheme(parsedTheme.builtinThemeId)
        : 'dark';

      applyBaseTheme(resolvedTheme);
      applyCssThemeId(undefined);
      return;
    }

    applyBaseTheme(themeOption.baseTheme);
    applyThemeVariables(themeOption.variables);
    applyCssThemeId(undefined);
  }

  function setTheme(theme: Theme) {
    if ((transitionsEnabledRef?.value ?? true) && hasAppliedTheme && theme !== appliedTheme) {
      runThemeTransition(() => applyTheme(theme));
    }
    else {
      applyTheme(theme);
    }

    hasAppliedTheme = true;
    appliedTheme = theme;
  }

  function toggleTheme() {
    return setTheme(currentTheme.value === 'dark' ? DEFAULT_THEME_ID : 'dark');
  }

  function handleSystemThemeChange(event: MediaQueryListEvent) {
    const parsedTheme = parseThemeId(themeSettingRef.value);

    if (parsedTheme?.source === 'builtin' && parsedTheme.builtinThemeId === 'system') {
      if (transitionsEnabledRef?.value ?? true) {
        runThemeTransition(() => applyBaseTheme(event.matches ? 'dark' : 'light'));
      }
      else {
        applyBaseTheme(event.matches ? 'dark' : 'light');
      }
    }
  }

  function init() {
    setTheme(themeSettingRef.value);
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    mediaQuery.addEventListener('change', handleSystemThemeChange);
  }

  watchEffect(() => {
    setTheme(themeSettingRef.value);
  });

  init();

  return {
    isDark,
    currentTheme,
    toggleTheme,
    setTheme,
  };
}
