<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { SettingsNavItem } from '@/modules/settings';
import { useSettingsStore } from '@/stores/runtime/settings';

const settingsStore = useSettingsStore();
</script>

<template>
  <div
    v-if="!settingsStore.search"
    class="settings-nav"
  >
    <div class="settings-nav__items">
      <SettingsNavItem
        v-for="tab in settingsStore.tabs"
        :key="tab.name"
        :name="tab.name"
        :label="tab.label"
      />
    </div>
  </div>
</template>

<style scoped>
.settings-nav {
  position: sticky;
  top: 0;
  display: flex;
  max-height: calc(100vh - var(--window-toolbar-height));
  flex-direction: column;
  align-self: start;
  padding-right: 1rem;

  /* FORK-MODIFICATION 2026-10-30：设置页左侧导航用抬起的面板色，与右侧内容区有层次差。 */
  border-right: 1px solid hsl(var(--border-subtle));
  background-color: hsl(var(--surface-panel));
  gap: 1rem;
  overflow-y: auto;
}

.settings-nav__items {
  display: flex;
  flex-direction: column;
  padding: 4px;
  gap: 0.25rem;
}

/* FORK-MODIFICATION 2026-10-30：导航项的悬浮 / 选中态跟着层次 token 走。 */
.settings-nav :deep(.settings-nav-item:hover) {
  background-color: hsl(var(--state-hover));
}

.settings-nav :deep(.settings-nav-item--active) {
  background-color: hsl(var(--state-selected));
}

@media (width <= 768px) {
  .settings-nav {
    position: static;
    max-height: none;
    padding-right: 0;
    border-right: none;
    overflow-y: visible;
  }
}
</style>
