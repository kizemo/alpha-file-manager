<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { computed, onMounted, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { SettingsHeader, SettingsContent, SettingsSearch, SettingsActions } from '@/modules/settings';
import { usePageLayoutScrollRestoration } from '@/composables/use-page-layout-scroll-restoration';
import { PageDefaultLayout } from '@/layouts';
import { useSettingsStore } from '@/stores/runtime/settings';
import {
  getScrollRestorationKey,
  useScrollRestorationStore,
} from '@/stores/runtime/scroll-restoration';

const { t } = useI18n();
const settingsStore = useSettingsStore();
const scrollStore = useScrollRestorationStore();

const settingsScrollKey = computed(() => getScrollRestorationKey(
  scrollStore.getResetVersion('settings'),
  settingsStore.search ? 'search' : settingsStore.currentTab,
));
const { pageLayoutRef } = usePageLayoutScrollRestoration({
  pageKey: 'settings',
  activeTab: settingsScrollKey,
});

onMounted(() => {
  settingsStore.init();
});

watch(
  () => settingsStore.currentTab,
  (currentTab, previousTab) => {
    if (previousTab && currentTab !== previousTab) {
      scrollStore.resetScrollNamespace('settings');
    }
  },
);
</script>

<template>
  <PageDefaultLayout
    ref="pageLayoutRef"
    class="settings-page"
    :title="t('pages.settings')"
  >
    <SettingsActions />
    <SettingsSearch />
    <!-- FORK-MODIFICATION 2026-10-31（r15 需求 4）：「关于」页自带头部（图标 + 应用名 + 版本 + 更新日志），
         再叠一层通用简介头会变成两个图标、两个应用名，所以这一页把通用头收起来。 -->
    <SettingsHeader
      v-if="!settingsStore.search && settingsStore.currentTab !== 'about'"
    />
    <SettingsContent />
  </PageDefaultLayout>
</template>

<style scoped>
.settings-page {
  color: hsl(var(--foreground));
}
</style>
