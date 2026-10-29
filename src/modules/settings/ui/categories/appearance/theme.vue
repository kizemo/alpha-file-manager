<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { BaseCombobox } from '@/components/base';
import { SettingsItem } from '@/modules/settings';
import {
  getAvailableThemeOptions,
  normalizeThemeSelection,
} from '@/modules/themes/registry';
import { isMacVariantThemeId, type MacVariantThemeId } from '@/modules/themes/mac-variants';
// FORK-MODIFICATION 2026-10-29（AFM UI 重设计）：三套品牌重设计主题。
import { isAfmThemeId, type AfmThemeId } from '@/modules/themes/afm-themes';
import { useExtensionsStorageStore } from '@/stores/storage/extensions';
import { useUserSettingsStore } from '@/stores/storage/user-settings';
import type { Theme } from '@/types/user-settings';
import { useI18n } from 'vue-i18n';
import { computed } from 'vue';
import { SunMoonIcon } from '@lucide/vue';
import type { ComboboxHighlightPayload } from '@/components/base/combobox-utils';

type ThemeSelectOption = {
  name: string;
  value: Theme;
  source: 'builtin' | 'extension';
};

type ThemeSelectOptionGroup = {
  heading?: string;
  options: ThemeSelectOption[];
};

const extensionsStorageStore = useExtensionsStorageStore();
const userSettingsStore = useUserSettingsStore();
const { t } = useI18n();

const availableThemeOptions = computed(() => {
  return getAvailableThemeOptions(extensionsStorageStore.extensionsData.installedExtensions);
});

// FORK-MODIFICATION 2026-10-29（AFM 3.6.5）：经典三套（亮色 / 暗色 / 跟随系统）。
// 内置变体主题带 CSS 变量覆盖（option.variables），所以这里用 variables 把两组分开。
const classicThemeOptions = computed<ThemeSelectOption[]>(() => {
  return availableThemeOptions.value.flatMap((option) => {
    if (option.source !== 'builtin' || option.variables) {
      return [];
    }

    return [{
      name: option.builtinThemeId === 'dark'
        ? t('settings.homeBannerEffects.theme.themeTypeRadio.dark')
        : option.builtinThemeId === 'light'
          ? t('settings.homeBannerEffects.theme.themeTypeRadio.light')
          : t('system'),
      value: option.id,
      source: option.source,
    }];
  });
});

// FORK-MODIFICATION 2026-10-29（AFM 3.6.5）：六套内置 macOS Finder 风格主题。
const macVariantThemeOptions = computed<ThemeSelectOption[]>(() => {
  return availableThemeOptions.value.flatMap((option) => {
    if (option.source !== 'builtin' || !option.variables) {
      return [];
    }

    if (!isMacVariantThemeId(option.builtinThemeId)) {
      return [];
    }

    return [{
      name: getMacVariantThemeName(option.builtinThemeId),
      value: option.id,
      source: option.source,
    }];
  });
});

function getMacVariantThemeName(themeId: MacVariantThemeId): string {
  switch (themeId) {
    case 'mac-classic-light':
      return t('settings.homeBannerEffects.theme.macVariants.classicLight');
    case 'mac-classic-dark':
      return t('settings.homeBannerEffects.theme.macVariants.classicDark');
    case 'mac-modern-light':
      return t('settings.homeBannerEffects.theme.macVariants.modernLight');
    case 'mac-modern-dark':
      return t('settings.homeBannerEffects.theme.macVariants.modernDark');
    case 'mac-glass-light':
      return t('settings.homeBannerEffects.theme.macVariants.glassLight');
    case 'mac-glass-dark':
      return t('settings.homeBannerEffects.theme.macVariants.glassDark');
  }

  return '';
}

// FORK-MODIFICATION 2026-10-29（AFM UI 重设计）：三套品牌重设计主题
// （暖纸浅色 / 暖纸暗色 / 跃动活力）。
const afmThemeOptions = computed<ThemeSelectOption[]>(() => {
  return availableThemeOptions.value.flatMap((option) => {
    if (option.source !== 'builtin' || !option.variables) {
      return [];
    }

    if (!isAfmThemeId(option.builtinThemeId)) {
      return [];
    }

    return [{
      name: getAfmThemeName(option.builtinThemeId),
      value: option.id,
      source: option.source,
    }];
  });
});

function getAfmThemeName(themeId: AfmThemeId): string {
  switch (themeId) {
    case 'afm-warm-light':
      return t('settings.homeBannerEffects.theme.afmThemes.warmLight');
    case 'afm-warm-dark':
      return t('settings.homeBannerEffects.theme.afmThemes.warmDark');
    case 'afm-vivid-light':
      return t('settings.homeBannerEffects.theme.afmThemes.vividLight');
  }

  return '';
}

const extensionThemeOptions = computed<ThemeSelectOption[]>(() => {
  return availableThemeOptions.value.flatMap((option) => {
    if (option.source === 'builtin') {
      return [];
    }

    return [{
      name: option.title,
      value: option.id,
      source: option.source,
    }];
  });
});

const themeOptions = computed<ThemeSelectOption[]>(() => [
  ...classicThemeOptions.value,
  ...macVariantThemeOptions.value,
  ...afmThemeOptions.value,
  ...extensionThemeOptions.value,
]);

const themeOptionGroups = computed<ThemeSelectOptionGroup[]>(() => {
  const groups: ThemeSelectOptionGroup[] = [
    {
      options: classicThemeOptions.value,
    },
  ];

  if (macVariantThemeOptions.value.length > 0) {
    groups.push({
      heading: t('settings.homeBannerEffects.theme.macVariantThemes'),
      options: macVariantThemeOptions.value,
    });
  }

  // FORK-MODIFICATION 2026-10-29（AFM UI 重设计）：三套品牌重设计主题单独一组，
  // 放在 macOS 风格主题之后、扩展主题之前。
  if (afmThemeOptions.value.length > 0) {
    groups.push({
      heading: t('settings.homeBannerEffects.theme.afmThemeThemes'),
      options: afmThemeOptions.value,
    });
  }

  if (extensionThemeOptions.value.length > 0) {
    groups.push({
      heading: t('settings.homeBannerEffects.theme.extensionThemes'),
      options: extensionThemeOptions.value,
    });
  }

  return groups;
});

const selectedThemeId = computed(() => {
  return normalizeThemeSelection(
    userSettingsStore.userSettings.theme,
    extensionsStorageStore.extensionsData.installedExtensions,
  );
});

const selectedTheme = computed({
  get: () => {
    return themeOptions.value.find(option => option.value === selectedThemeId.value)
      ?? themeOptions.value[0];
  },
  set: (option) => {
    if (option) {
      userSettingsStore.set('theme', option.value);
    }
  },
});

function setThemeTransitionOrigin(element: HTMLElement) {
  const { left, top, width, height } = element.getBoundingClientRect();

  userSettingsStore.setThemeTransitionOrigin({
    x: left + width / 2,
    y: top + height / 2,
  });
}

function onThemeComboboxKeyboardHighlight(payload: ComboboxHighlightPayload) {
  if (payload?.ref) {
    setThemeTransitionOrigin(payload.ref);
  }
}

function onThemeComboboxSelect(payload: { ref: HTMLElement }) {
  setThemeTransitionOrigin(payload.ref);
}
</script>

<template>
  <SettingsItem
    :title="t('settings.homeBannerEffects.theme.title')"
    :icon="SunMoonIcon"
  >
    <BaseCombobox
      v-model="selectedTheme"
      :options="themeOptions"
      :option-groups="themeOptionGroups"
      by="value"
      :search-placeholder="t('search')"
      :empty-text="t('noData')"
      :select-on-highlight="true"
      :close-on-select="false"
      @keyboard-highlight="onThemeComboboxKeyboardHighlight"
      @select="onThemeComboboxSelect"
    />
  </SettingsItem>
</template>
