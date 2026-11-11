<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<!--
  左侧导航栏「远程」按钮的悬停面板。

  结构（2026-11-08 按用户要求重排）：
  1. **远程连接设置** —— 跳到设置页的「远程」分类；
  2. **网盘**分区 —— 百度网盘这类**网盘连接**，列表在上；
  3. **远程连接**分区 —— FTP / FTPS / SFTP 这类**协议连接**，列表在下。

  网盘与远程连接**分区**而不是混在一张列表里：两者的录入方式完全不同
  （网盘要 Cookie，FTP 要主机端口），后续还要加阿里云盘等更多网盘，
  混排会让「网盘」越来越难认出来。每个分区自带一颗添加按钮，
  点它直接落到设置页的对应标签页（网盘 / 远程连接）。

  连接列表**直接读 store**，不在这里缓存一份：
  面板可能在设置页改完连接后还开着（悬停面板本来就常驻），
  缓存的副本会列出已经删掉或改名的连接。

  每条连接右侧还有一颗**设置按钮**（用户 2026-10-18 要求）：点它直接进那条
  连接的编辑界面，而不是落到「远程」分类让用户自己再找一次。
  行主体与按钮是**两个并列的 button**，不是按钮套按钮 —— 嵌套 button 是无效
  HTML，而且外层的 click 会一起触发，等于点设置也把连接打开了。
-->

<script setup lang="ts">
import { computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { PlusIcon, ServerIcon, SettingsIcon } from '@lucide/vue';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import { hasRemoteLocalPath, isNetdiskProtocol } from '@/types/remote';
import type { RemoteConnection, RemoteProtocol } from '@/types/remote';

const emit = defineEmits<{
  /** 新建请求，附带要预置的协议（网盘分区给 `baidu_pan`）。 */
  add: [protocol: RemoteProtocol];
  openSettings: [];
  open: [connection: RemoteConnection];
  edit: [connection: RemoteConnection];
}>();

const { t } = useI18n();
const store = useRemoteConnectionsStore();

const connections = computed(() => store.connections);

/**
 * 两个分区的定义。
 *
 * **顺序就是界面顺序**：网盘在上、远程连接在下（用户 2026-11-08 要求）。
 * 分区靠 `protocol` 归类而不是靠连接名 —— 新增阿里云盘时只要把它的协议
 * 标成网盘一类的，这里不用再改。
 */
const sections = computed(() => [
  {
    key: 'netdisk',
    label: t('settings.remote.nav.netdisk'),
    addLabel: t('settings.remote.nav.addNetdisk'),
    emptyLabel: t('settings.remote.nav.emptyNetdisk'),
    protocol: 'baidu_pan' as RemoteProtocol,
    /**
     * 该分区**除默认 provider 之外**还能预置的 provider。
     *
     * 网盘协议不在表单的协议下拉里，所以「添加夸克网盘」只能从这一层发起点进去 ——
     * 否则用户点「添加网盘」永远只会得到百度的表单。
     */
    extraProviders: [
      {
        protocol: 'quark_pan' as RemoteProtocol,
        label: t('settings.remote.netdisk.quark'),
      },
    ],
    connections: connections.value.filter(connection => isNetdiskProtocol(connection.protocol)),
  },
  {
    key: 'remote',
    label: t('settings.remote.nav.remoteSection'),
    addLabel: t('settings.remote.nav.addRemote'),
    emptyLabel: t('settings.remote.nav.emptyRemote'),
    protocol: 'ftp' as RemoteProtocol,
    extraProviders: [],
    connections: connections.value.filter(connection => !isNetdiskProtocol(connection.protocol)),
  },
]);

function describeSplitTag(connection: RemoteConnection): string | null {
  // 双屏是这个连接的属性，不是全局开关，必须跟着连接一起显示，
  // 否则用户不知道点下去会开一格还是两格。
  return hasRemoteLocalPath(connection) ? t('settings.remote.nav.split') : null;
}

onMounted(async () => {
  // 导航栏可能先于设置页被打开，这里自己确保 store 已从盘上读过一遍。
  await store.init();
});
</script>

<template>
  <div class="remote-nav-panel">
    <button
      type="button"
      class="remote-nav-panel__row"
      data-test="remote-nav-settings"
      @click="emit('openSettings')"
    >
      <SettingsIcon :size="14" />
      <span>{{ t('settings.remote.nav.openSettings') }}</span>
    </button>

    <div
      v-for="section in sections"
      :key="section.key"
      class="remote-nav-panel__section"
      :data-test="`remote-nav-section-${section.key}`"
    >
      <div class="remote-nav-panel__section-head">
        <span class="remote-nav-panel__label">{{ section.label }}</span>
        <div class="remote-nav-panel__section-actions">
          <button
            type="button"
            class="remote-nav-panel__section-add"
            :data-test="`remote-nav-add-${section.key}`"
            @click="emit('add', section.protocol)"
          >
            <PlusIcon :size="12" />
            <span>{{ section.addLabel }}</span>
          </button>
          <button
            v-for="provider in section.extraProviders"
            :key="provider.protocol"
            type="button"
            class="remote-nav-panel__section-add"
            :data-test="`remote-nav-add-${provider.protocol}`"
            @click="emit('add', provider.protocol)"
          >
            <PlusIcon :size="12" />
            <span>{{ provider.label }}</span>
          </button>
        </div>
      </div>

      <p
        v-if="section.connections.length === 0"
        class="remote-nav-panel__empty"
        :data-test="`remote-nav-empty-${section.key}`"
      >
        {{ section.emptyLabel }}
      </p>

      <div
        v-for="connection in section.connections"
        :key="connection.id"
        class="remote-nav-panel__entry"
        data-test="remote-nav-connection-row"
        :data-protocol="connection.protocol"
      >
        <button
          type="button"
          class="remote-nav-panel__row remote-nav-panel__row--open"
          data-test="remote-nav-connection"
          @click="emit('open', connection)"
        >
          <ServerIcon :size="14" />
          <span class="remote-nav-panel__name">{{ connection.name }}</span>
          <span
            v-if="describeSplitTag(connection)"
            class="remote-nav-panel__tag"
            data-test="remote-nav-split-tag"
          >
            {{ describeSplitTag(connection) }}
          </span>
        </button>

        <button
          type="button"
          class="remote-nav-panel__edit"
          :title="t('settings.remote.nav.editConnection')"
          :aria-label="`${t('settings.remote.nav.editConnection')}: ${connection.name}`"
          data-test="remote-nav-edit"
          @click="emit('edit', connection)"
        >
          <SettingsIcon :size="13" />
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.remote-nav-panel {
  display: flex;
  width: 240px;
  flex-direction: column;
  gap: 2px;
}

.remote-nav-panel__row {
  display: flex;
  align-items: center;
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  background-color: transparent;
  color: hsl(var(--foreground));
  cursor: pointer;
  gap: 8px;
  text-align: left;
}

.remote-nav-panel__row:hover {
  background-color: hsl(var(--foreground) / 5%);
}

/*
 * 分区（网盘在上、远程连接在下）。
 *
 * 分区之间用一条细线隔开，而不是只靠一个小标题 —— 标题小到 0.7rem，
 * 两块列表贴在一起时眼睛分不清哪条列表归哪个分区。
 */

.remote-nav-panel__section {
  display: flex;
  flex-direction: column;
  padding-top: 4px;
  gap: 2px;
}

.remote-nav-panel__section + .remote-nav-panel__section {
  border-top: 1px solid hsl(var(--border) / 60%);
  margin-top: 4px;
}

.remote-nav-panel__section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 8px;
  gap: 8px;
}

/*
 * 分区标题右侧的添加按钮。
 *
 * 刻意做成「文字 + 图标」的小按钮而不是纯图标：图标轨上的纯图标按钮
 * 已经被证明太隐蔽（用户反馈「找不到添加入口」），这里是面板内的第二道
 * 入口，宁可显眼一点也不藏着。
 */

.remote-nav-panel__section-add {
  display: flex;
  align-items: center;
  padding: 2px 4px;
  border-radius: var(--radius-sm);
  background-color: transparent;
  color: hsl(var(--primary));
  cursor: pointer;
  font-size: 0.7rem;
  gap: 3px;
}

.remote-nav-panel__section-add:hover {
  background-color: hsl(var(--foreground) / 8%);
}

/*
 * 分区标题右侧那一排按钮（默认 provider + 额外 provider）的容器。
 *
 * 容器化而不是给两颗按钮各自加 margin：provider 数量会随网盘种类继续增加，
 * 之后加一颗按钮不需要再回来改边距。
 */

.remote-nav-panel__section-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

/*
 * 行 = 「打开这一格」的主按钮 + 右侧一颗设置按钮。
 * 两者并列而不是嵌套：button 里再放 button 是无效 HTML，
 * 而且外层的 click 会一起触发 —— 点设置等于把连接也打开了。
*/

.remote-nav-panel__entry {
  display: flex;
  align-items: center;
  border-radius: var(--radius-sm);
  gap: 2px;
}

.remote-nav-panel__row--open {
  min-width: 0;
  flex: 1;
}

.remote-nav-panel__entry:hover {
  background-color: hsl(var(--foreground) / 5%);
}

.remote-nav-panel__entry:hover .remote-nav-panel__row--open {
  background-color: transparent;
}

.remote-nav-panel__edit {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  padding: 6px;
  border-radius: var(--radius-sm);
  background-color: transparent;
  color: hsl(var(--muted-foreground));
  cursor: pointer;
}

.remote-nav-panel__edit:hover {
  background-color: hsl(var(--foreground) / 10%);
  color: hsl(var(--foreground));
}

.remote-nav-panel__name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.remote-nav-panel__tag {
  padding: 0 6px;
  border-radius: 999px;
  margin-left: auto;
  background-color: hsl(var(--muted));
  color: hsl(var(--muted-foreground));
  font-size: 0.65rem;
}

.remote-nav-panel__label {
  color: hsl(var(--muted-foreground));
  font-size: 0.7rem;
  font-weight: 500;
}

.remote-nav-panel__empty {
  padding: 2px 8px 6px;
  margin: 0;
  color: hsl(var(--muted-foreground));
  font-size: 0.8rem;
}
</style>
