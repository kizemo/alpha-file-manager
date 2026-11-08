<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { ref } from 'vue';
import { resetSettingsItemStaggerIndex } from '@/utils/settings-item-stagger';
import ConnectionsSection from './connections-section.vue';
import TrustedHostsSection from './trusted-hosts-section.vue';
import type { RemoteSection } from './sections';

resetSettingsItemStaggerIndex();

/**
 * 当前分区（用户 2026-11-08）。
 *
 * 「信任的主机」只在**远程连接**标签下显示，所以这一层必须知道当前分区 ——
 * 而分区状态住在 `ConnectionsSection` 里（标签页在它内部）。
 *
 * 初值刻意是 `null` 而不是 `'netdisk'`：还没收到子组件的第一份报告时，
 * 我们并不知道它落在哪个标签上。猜 `'netdisk'` 会让「只有远程连接」的用户
 * 在挂载瞬间看见「信任的主机」闪一下再消失。
 */
const activeSection = ref<RemoteSection | null>(null);
</script>

<template>
  <div class="settings-category-remote">
    <ConnectionsSection @section-change="activeSection = $event" />
    <!--
      「信任的主机」并进远程连接标签（用户 2026-11-08）。

      为什么不放进 `ConnectionsSection` 里：那个组件的内容都在自己的
      `SettingsItem` 面板内部，把另一个自带 `SettingsItem` 的组件塞进去
      会形成面板套面板。「分区信号往上抛」比「把两个组件揉成一个」改动面小得多。
    -->
    <TrustedHostsSection v-if="activeSection === 'remote'" />
  </div>
</template>

<style scoped>
.settings-category-remote {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
</style>
