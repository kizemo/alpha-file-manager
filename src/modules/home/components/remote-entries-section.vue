<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRouter } from 'vue-router';
import { CloudIcon, ServerIcon } from '@lucide/vue';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import { useWorkspacesStore } from '@/stores/storage/workspaces';
import { openRemoteConnection } from '@/modules/remote/utils/open-remote-connection';
import { openSettingsTab } from '@/modules/settings/utils/open-settings';
import { isBaiduPanProtocol } from '@/types/remote';
import type { RemoteConnection } from '@/types/remote';

const { t } = useI18n();
const router = useRouter();
const remoteConnectionsStore = useRemoteConnectionsStore();
const workspacesStore = useWorkspacesStore();

/**
 * 位置页（主页）的「网盘」「远程连接」（用户 2026-11-08 需求 2）。
 *
 * 第一版是两个固定入口，点了**跳设置页**。用户明确否掉了这个行为：
 * 「点击会跳到远程设置页面，应该显示配置好的网盘、远程连接的服务器，
 * 显示名称，点击在导航页视图中打开对应的网盘和服务器」。
 *
 * 所以这里现在**按连接列条目**，每条显示连接自己的名字，点击就是连它。
 * 分区判定仍然只有 `isBaiduPanProtocol` 一个口径（与设置页、导航栏悬停面板一致），
 * 以后接入阿里云盘之类的网盘只要扩这个判定，这一页自动跟上。
 */
const netdiskConnections = computed(() => (
  remoteConnectionsStore.connections.filter(connection => isBaiduPanProtocol(connection.protocol))
));

const remoteConnections = computed(() => (
  remoteConnectionsStore.connections.filter(connection => !isBaiduPanProtocol(connection.protocol))
));

/** 一条连接都没有时只给提示与一个设置入口，不摆两个空分组。 */
const hasConnections = computed(() => remoteConnectionsStore.connections.length > 0);

/**
 * 连一个连接：切到导航器页，开一组标签页（左本地右远端）。
 *
 * 判定与顺序都封在 `openRemoteConnection` 里 —— 与导航栏悬停面板
 * （`nav-sidebar.vue`）走的是同一条路径，两处行为不会漂。
 * **不在这里自己 if 一次**「有没有本地路径」：那是两份真相。
 */
async function handleOpen(connection: RemoteConnection): Promise<void> {
  await openRemoteConnection(connection, {
    navigateToNavigator: () => router.push({ name: 'navigator' }),
    openPanes: paths => workspacesStore.openNewTabGroupWithPaths(paths),
  });
}

/**
 * 空状态下的设置入口。
 *
 * 这不是「回到第一版」：第一版是**唯一的入口**只有跳设置，用户否掉的是那个。
 * 这里是一条独立的、可负担的入口 —— 没有任何连接时，位置页本身没法给出
 * 任何可点的目标，只剩「去加一条」这一个有意义的动作。
 */
async function openRemoteSettings(): Promise<void> {
  await openSettingsTab('remote');
}
</script>

<template>
  <section class="remote-entries-section">
    <div
      v-if="!hasConnections"
      class="remote-entries-section__empty"
      data-test="empty"
    >
      <p class="remote-entries-section__empty-text">
        {{ t('settings.remote.nav.homeEmpty') }}
      </p>
      <button
        type="button"
        class="remote-entries-section__settings-button"
        data-test="open-settings"
        @click="openRemoteSettings"
      >
        {{ t('settings.remote.nav.openSettings') }}
      </button>
    </div>

    <template v-else>
      <div
        v-if="netdiskConnections.length > 0"
        class="remote-entries-section__group"
        data-test="group-netdisk"
      >
        <h3 class="remote-entries-section__group-title">
          <CloudIcon :size="14" />
          <span>{{ t('settings.remote.nav.netdisk') }}</span>
        </h3>

        <div class="remote-entries-section__grid">
          <button
            v-for="connection in netdiskConnections"
            :key="connection.id"
            type="button"
            class="remote-entries-section__entry"
            :data-test="`netdisk-${connection.id}`"
            @click="handleOpen(connection)"
          >
            <CloudIcon
              :size="20"
              class="remote-entries-section__icon"
            />
            <span class="remote-entries-section__name">
              {{ connection.name }}
            </span>
          </button>
        </div>
      </div>

      <div
        v-if="remoteConnections.length > 0"
        class="remote-entries-section__group"
        data-test="group-remote"
      >
        <h3 class="remote-entries-section__group-title">
          <ServerIcon :size="14" />
          <span>{{ t('settings.remote.nav.remoteSection') }}</span>
        </h3>

        <div class="remote-entries-section__grid">
          <button
            v-for="connection in remoteConnections"
            :key="connection.id"
            type="button"
            class="remote-entries-section__entry"
            :data-test="`remote-${connection.id}`"
            @click="handleOpen(connection)"
          >
            <ServerIcon
              :size="20"
              class="remote-entries-section__icon"
            />
            <span class="remote-entries-section__name">
              {{ connection.name }}
            </span>
          </button>
        </div>
      </div>
    </template>
  </section>
</template>

<style>
/*
  位置页的「网盘 / 远程连接」连接列表（2026-11-08）。

  卡片外观与「驱动」分区里的盘符卡保持一致（同样的背景、圆角与悬停反馈），
  否则两个分区看起来像出自两个应用。
*/

.remote-entries-section {
  margin-bottom: 16px;
}

.remote-entries-section__group {
  margin-bottom: 16px;
}

/*
  分区标题的字号、字距、大写与下边距必须与「用户目录」「驱动」分区的标题逐字一致
  （user-directories-section__title + 其 header 的 margin-bottom: 12px），
  否则同一页上两组标题看起来像出自两个应用。
*/

.remote-entries-section__group-title {
  display: flex;
  align-items: center;
  margin-bottom: 12px;
  color: hsl(var(--muted-foreground));
  font-size: 13px;
  font-weight: 500;
  gap: 6px;
  letter-spacing: 0.5px;
  text-transform: uppercase;
}

.remote-entries-section__grid {
  display: grid;
  gap: 16px;
  grid-auto-rows: 64px;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
}

.remote-entries-section__entry {
  display: grid;
  overflow: hidden;
  width: 100%;
  height: 100%;
  align-items: center;
  padding: 0;
  border: none;
  border-radius: var(--radius);
  background-color: hsl(var(--card));
  color: hsl(var(--card-foreground));
  cursor: pointer;
  gap: 0;
  grid-template-columns: 56px 1fr;
  text-align: start;
  transition: background-color var(--hover-transition-duration-out) var(--hover-transition-easing-out);
}

.remote-entries-section__entry:hover {
  background-color: hsl(var(--muted));
  transition: background-color var(--hover-transition-duration-in) var(--hover-transition-easing-in);
}

.remote-entries-section__entry:focus-visible {
  outline: 2px solid hsl(var(--ring) / 20%);
  outline-offset: var(--ring-outline-offset);
}

.remote-entries-section__icon {
  margin: 0 auto;
  color: hsl(var(--muted-foreground));
}

.remote-entries-section__name {
  overflow: hidden;
  color: hsl(var(--foreground));
  font-size: 13px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.remote-entries-section__empty {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  padding: 16px;
  border-radius: var(--radius);
  background-color: hsl(var(--card));
  gap: 12px;
}

.remote-entries-section__empty-text {
  margin: 0;
  color: hsl(var(--muted-foreground));
  font-size: 13px;
}

.remote-entries-section__settings-button {
  height: 32px;
  padding: 0 12px;
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius);
  background-color: hsl(var(--background));
  color: hsl(var(--foreground));
  cursor: pointer;
  font-size: 13px;
  transition: background-color var(--hover-transition-duration-in) var(--hover-transition-easing-in);
}

.remote-entries-section__settings-button:hover {
  background-color: hsl(var(--muted));
}

.remote-entries-section__settings-button:focus-visible {
  outline: 2px solid hsl(var(--ring) / 20%);
  outline-offset: var(--ring-outline-offset);
}
</style>
