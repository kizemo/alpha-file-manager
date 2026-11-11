<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { SettingsItem } from '@/modules/settings';
import { Button } from '@/components/ui/button';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { ServerIcon, PlusIcon, TrashIcon } from '@lucide/vue';
import ConnectionForm from './connection-form.vue';
import NetdiskAddDialog from './netdisk-add-dialog.vue';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import { isNetdiskProtocol } from '@/types/remote';
import type { RemoteConnection, RemoteProtocol } from '@/types/remote';
import { sectionOfConnection, sectionOfProtocol } from './sections';
import type { RemoteSection } from './sections';

const { t } = useI18n();
const store = useRemoteConnectionsStore();

const editing = ref<RemoteConnection | null>(null);
const isCreating = ref(false);

/**
 * 「添加网盘」对话框（用户 2026-11-11）。
 *
 * **常驻挂载** —— 内嵌登录窗是 Rust 侧的独立窗口，它把登录进度推到
 * `afm://baidu-login` / `afm://quark-login` 频道；组件得一直在那儿听着，
 * 否则「登录成功」那一刻没人接，录入弹窗就永远不出现。
 */
const netdiskDialogRef = ref<InstanceType<typeof NetdiskAddDialog> | null>(null);

/**
 * 顶部两个标签（用户 2026-11-08）。
 *
 * 这一页现在**整个**按「网盘 / 远程连接」分成两栏，而不只是新建表单分栏：
 * 列表、添加按钮、表单全部跟着当前标签走。
 *
 * 判定口径只有一个 —— `isBaiduPanProtocol`。以后接入阿里云盘之类的网盘，
 * 只要把判定扩进去，这一页的分区自动跟上，不用再动模板。
 *
 * 分区类型与两个判定函数抽在 `./sections` 里：父级分类同样要靠分区结果
 * 决定「信任的主机」是否显示（用户 2026-11-08 需求 1），两处各写一份就会漂。
 */
const emit = defineEmits<{ 'section-change': [section: RemoteSection] }>();

/** 新建远程连接时预置的协议。网盘表单不认这个（形态已经定死）。 */
const createProtocol = ref<RemoteProtocol>('ftp');

const netdiskConnections = computed(() => store.connections.filter(connection => isNetdiskProtocol(connection.protocol)));
const remoteConnections = computed(() => store.connections.filter(connection => !isNetdiskProtocol(connection.protocol)));

/**
 * 用户手动选过的分区。
 *
 * 留空 = 用户还没动过标签 —— 这时**跟着用户已有的连接走**：有网盘就落网盘，
 * 只有远程连接就落远程连接，一个都没有才落网盘。
 *
 * 为什么不写死：分区会随连接增删变化。用户只有一条远程连接时进来，
 * 就该看见那条连接，而不是一个空列表让他以为连接没了。
 */
const sectionOverride = ref<RemoteSection | null>(null);

const defaultSection = computed<RemoteSection>(() => {
  if (netdiskConnections.value.length > 0) {
    return 'netdisk';
  }

  if (remoteConnections.value.length > 0) {
    return 'remote';
  }

  return 'netdisk';
});

/**
 * 当前分区。
 *
 * 可写 —— 标签页点一下就写进 `sectionOverride`，之后不再被默认策略抢走。
 */
const activeSection = computed<RemoteSection>({
  get: () => sectionOverride.value ?? defaultSection.value,
  set: (section: RemoteSection) => {
    sectionOverride.value = section;
  },
});

/** 当前标签下要列出来的连接。 */
const activeConnections = computed(() => (
  activeSection.value === 'netdisk' ? netdiskConnections.value : remoteConnections.value
));

/** 表单形态跟着当前标签走：网盘标签给网盘表单，远程连接标签给 FTP/SFTP 那套。 */
const activeVariant = computed<'netdisk' | 'remote'>(() => activeSection.value);

/**
 * 远程连接表单预置的协议。
 *
 * 百度网盘不在远程连接的协议下拉里 —— 上一条请求若是「添加网盘」，
 * 带着 `baidu_pan` 切到远程连接标签，表单会选出一个下拉里根本不存在的值。
 * 这里把它挡回默认协议。
 */
const remoteInitialProtocol = computed<RemoteProtocol>(() => (
  isNetdiskProtocol(createProtocol.value) ? 'ftp' : createProtocol.value
));

/**
 * 把当前分区报给父级分类（用户 2026-11-08 需求 1）。
 *
 * 父级据此决定「信任的主机」是否渲染 —— 那一块现在只在「远程连接」标签下出现。
 *
 * `immediate: true` 不能省：只配了远程连接的用户进来时默认分区就是 `remote`，
 * 没有这次立即上报，父级会停在 `null`，于是「信任的主机」整块永远不出现。
 */
watch(activeSection, section => emit('section-change', section), { immediate: true });

/**
 * 导航栏悬停面板上每条连接右侧的设置按钮，会先往 store 里放一个待编辑的
 * 连接 id，再跳到本分类（用户 2026-10-18 要求）。
 *
 * **挂载后和变化时都要看**：从别的页面跳过来时分类是**新挂载**的；
 * 而用户在设置页里又点一次那颗按钮时分类不会重新挂载，只有 watch 能接住。
 *
 * ⚠️ **查不到连接时不能立刻把请求丢掉**：`onMounted` 里 `store.init()` 是异步的，
 * 组件挂载那一刻 `connections` 可能还是空的。此时消费掉请求就等于把用户的
 * 那次点击吞掉，表单永远不打开 —— 表现是「点设置只是跳到了远程分类」。
 * 所以只有「已加载完还查不到」才认定连接真被删了，这时才清除请求。
 */
function applyPendingEdit(): void {
  const id = store.pendingEditId;

  if (!id) {
    return;
  }

  const target = store.getConnection(id);

  if (!target) {
    // 连接可能在这条请求排队期间被删了。找不到就不开表单 ——
    // 凭空弹一个空表单会让用户以为多了一条连接。
    if (store.isInitialized) {
      store.consumeRemoteEdit();
    }

    return;
  }

  store.consumeRemoteEdit();
  isCreating.value = false;
  editing.value = target;
  // 编辑表单得落在它自己的分区里 —— 远程连接不该出现在「网盘」标签下。
  activeSection.value = sectionOfConnection(target);
}

/**
 * 新建请求（2026-11-08）。
 *
 * 导航栏悬停面板里点「添加网盘」来的请求落到网盘页，点「添加远程连接」来的
 * 落到远程连接页 —— 请求里带的协议就是这两者唯一的区别。
 *
 * 默认落在**网盘**页：网盘是新增能力，也是用户第一次进这一页最可能要加的东西。
 */
function applyPendingCreate(): void {
  const requested = store.consumeRemoteCreate();

  if (!requested) {
    return;
  }

  activeSection.value = sectionOfProtocol(requested);
  editing.value = null;

  // 网盘走「登录 → 录入」的新流程（用户 2026-11-11）：
  // 直接开出对应 provider 的内嵌登录窗，登录成功后再弹录入表单。
  // 这里**不**开内联表单 —— 网盘表单带授权状态卡，与新增流程的形态冲突。
  if (isNetdiskProtocol(requested)) {
    isCreating.value = false;
    netdiskDialogRef.value?.startAdd(requested === 'quark_pan' ? 'quark_pan' : 'baidu_pan');
    return;
  }

  createProtocol.value = requested;
  isCreating.value = true;
}

/**
 * 「切到某个分区」的一次性请求（2026-11-08）。
 *
 * 主页「位置」页底部的「网盘」「远程连接」两个入口点了要直接落到对应标签上，
 * 而设置页的顶层分类机制（`openSettingsTab`）只认得到「远程」这一层分类，
 * 认不到分类内部的两个标签 —— 所以这里补一个瞬态值。
 *
 * **消费即清空**：否则用户下次自己进这一页，会被莫名其妙地拽回某个标签。
 */
function applyPendingSection(): void {
  const requested = store.consumeRemoteSection();

  if (!requested) {
    return;
  }

  activeSection.value = requested;
  editing.value = null;
  isCreating.value = false;
}

onMounted(async () => {
  await store.init();
  applyPendingEdit();
  applyPendingCreate();
  applyPendingSection();
});

watch(() => store.pendingEditId, applyPendingEdit);
watch(() => store.pendingCreate, applyPendingCreate);
watch(() => store.pendingSection, applyPendingSection);

function startCreate() {
  editing.value = null;
  isCreating.value = true;
}

function startEdit(connection: RemoteConnection) {
  isCreating.value = false;
  editing.value = connection;
  activeSection.value = sectionOfConnection(connection);
}

function closeForm() {
  isCreating.value = false;
  editing.value = null;
}

async function handleDelete(connection: RemoteConnection) {
  await store.deleteConnection(connection.id);

  if (editing.value?.id === connection.id) {
    closeForm();
  }
}

/**
 * 有存档密码时显示的掩码。
 *
 * **放在 script 而不是模板里**，是因为项目启用了 `@intlify/vue-i18n/no-raw-text`，
 * 模板里的裸文本一律报错。星号本身不该翻译（译成别的语言它还是一串星号），
 * 所以也不进语言包 —— 可读名称由 `settings.remote.password.saved` 承担。
 */
const PASSWORD_MASK = '***';

function describeProtocol(connection: RemoteConnection): string {
  return t(`settings.remote.protocol.${connection.protocol}`);
}

function describeProxy(connection: RemoteConnection): string {
  if (connection.proxy.kind === 'direct') {
    return t('settings.remote.proxy.direct');
  }

  return `${t(`settings.remote.proxy.${connection.proxy.kind}`)} · ${connection.proxy.host}:${connection.proxy.port}`;
}
</script>

<template>
  <SettingsItem
    class="remote-connections"
    :title="t('settings.remote.connections.title')"
    :icon="ServerIcon"
  >
    <!--
      顶部两个标签（用户 2026-11-08）：整个分类按「网盘 / 远程连接」分栏，
      列表、添加按钮与表单都跟着当前标签走。

      标签页**只负责切换**，内容不放在 `TabsContent` 里：放进去的话，
      两个标签各要一份列表与表单的模板（改一处要改两处），
      而且切标签会连带把用户正在填的表单卸载掉。
    -->
    <Tabs
      v-model="activeSection"
      class="remote-connections__tabs"
      data-test="remote-section-tabs"
    >
      <TabsList>
        <TabsTrigger
          value="netdisk"
          data-test="tab-netdisk"
        >
          {{ t('settings.remote.nav.netdisk') }}
        </TabsTrigger>
        <TabsTrigger
          value="remote"
          data-test="tab-remote"
        >
          {{ t('settings.remote.nav.remoteSection') }}
        </TabsTrigger>
      </TabsList>
    </Tabs>

    <!--
      分区介绍（用户 2026-11-11）。

      原来这段介绍挂在 `SettingsItem` 的 `description` 上，摆在**选项卡上方**；
      现在标题下面直接就是选项卡，介绍下沉到每个选项卡内部 ——
      讲的是当前这个分区的事，就该待在当前这个分区里。
    -->
    <p
      class="remote-connections__section-description"
      data-test="section-description"
    >
      {{ activeSection === 'netdisk'
        ? t('settings.remote.nav.netdiskDescription')
        : t('settings.remote.nav.remoteDescription') }}
    </p>

    <p
      v-if="activeConnections.length === 0 && !isCreating"
      class="remote-connections__empty"
      data-test="empty"
    >
      {{ activeSection === 'netdisk' ? t('settings.remote.nav.emptyNetdisk') : t('settings.remote.nav.emptyRemote') }}
    </p>

    <ul
      v-else
      class="remote-connections__list"
      data-test="list"
    >
      <li
        v-for="connection in activeConnections"
        :key="connection.id"
        class="remote-connections__entry"
        :data-test="`entry-${connection.id}`"
      >
        <div class="remote-connections__entry-main">
          <div class="remote-connections__entry-title">
            <span>{{ connection.name }}</span>
            <!--
              有存档密码就显示 `***`，没有就整块不渲染（用户 2026-10-18 要求）。
              用星号而不是「已保存密码」这种文案，是为了让「有没有密码」在一眼
              扫过去时就分得出来 —— 文案徽标要读，没星号不要读。

              `***` 是密码掩码，不该翻译（译成别的语言它就还是一串星号）；
              可读名称挂在 title/aria-label 上，屏幕阅读器照样念得出来。
            -->
            <span
              v-if="connection.credentialKey"
              class="remote-connections__badge"
              data-test="has-password"
              :title="t('settings.remote.password.saved')"
              :aria-label="t('settings.remote.password.saved')"
            >{{ PASSWORD_MASK }}</span>
          </div>
          <div class="remote-connections__entry-meta">
            <span>{{ describeProtocol(connection) }}</span>
            <span>{{ connection.host }}:{{ connection.port }}</span>
            <span v-if="connection.username">{{ connection.username }}</span>
            <span>{{ connection.initialPath }}</span>
            <span>{{ describeProxy(connection) }}</span>
            <span
              v-if="connection.localPath"
              class="remote-connections__entry-local"
              data-test="local-path"
            >
              {{ t('settings.remote.field.localPath') }} {{ connection.localPath }}
            </span>
          </div>
        </div>
        <div class="remote-connections__entry-actions">
          <Button
            variant="outline"
            size="sm"
            :data-test="`edit-${connection.id}`"
            @click="startEdit(connection)"
          >
            {{ t('settings.remote.action.edit') }}
          </Button>
          <Button
            variant="destructive"
            size="sm"
            :data-test="`delete-${connection.id}`"
            @click="handleDelete(connection)"
          >
            <TrashIcon :size="16" />
            {{ t('settings.remote.action.delete') }}
          </Button>
        </div>
      </li>
    </ul>

    <!--
      「添加网盘」（用户 2026-11-11）。

      原来这里是「provider 选择器 + 一颗『添加网盘』按钮」两段式；
      现在合成一块：标题下面直接摆两颗按钮，点哪颗就开哪家的内嵌登录窗，
      登录成功后再由 `NetdiskAddDialog` 弹出录入表单。
    -->
    <div
      v-if="activeSection === 'netdisk'"
      class="remote-connections__add-netdisk"
      data-test="add-netdisk"
    >
      <h4 class="remote-connections__add-netdisk-title">
        {{ t('settings.remote.nav.addNetdisk') }}
      </h4>
      <div class="remote-connections__add-netdisk-options">
        <Button
          variant="outline"
          data-test="add-netdisk-baidu_pan"
          @click="netdiskDialogRef?.startAdd('baidu_pan')"
        >
          {{ t('settings.remote.netdisk.baidu') }}
        </Button>
        <Button
          variant="outline"
          data-test="add-netdisk-quark_pan"
          @click="netdiskDialogRef?.startAdd('quark_pan')"
        >
          {{ t('settings.remote.netdisk.quark') }}
        </Button>
      </div>
    </div>

    <!--
      远程连接分区的「添加」按钮。

      网盘分区不再有这颗按钮 —— 它的入口就是上面那块「添加网盘」。
    -->
    <Button
      v-if="activeSection === 'remote' && !isCreating && editing === null"
      variant="outline"
      data-test="add"
      @click="startCreate"
    >
      <PlusIcon :size="16" />
      {{ t('settings.remote.nav.addRemote') }}
    </Button>

    <!--
      新建：形态跟着当前标签走。

      `:key` 让换标签时表单按新形态**重建** —— 表单内部是用 `variant` 决定
      形态的（`isNetdisk`），不重建的话上一张表单的判断会留在原地，
      出现「在网盘标签里看到远程连接的表单」这种错位。
    -->
    <ConnectionForm
      v-if="isCreating"
      :key="`create-${activeSection}`"
      :connection="null"
      :variant="activeVariant"
      :initial-protocol="remoteInitialProtocol"
      @cancel="closeForm"
      @saved="closeForm"
    />

    <!--
      编辑：按这条连接的类型选表单，**不给标签页** ——
      编辑一条 FTP 连接时，另一个标签页里能填的东西跟它毫无关系，
      摆出来只会让人以为要改两处。
    -->
    <ConnectionForm
      v-else-if="editing"
      :key="`edit-${editing.id}`"
      :connection="editing"
      :variant="sectionOfConnection(editing)"
      @cancel="closeForm"
      @saved="closeForm"
    />

    <!--
      「添加网盘」对话框**常驻挂载**：它要接内嵌登录窗推来的登录进度，
      挂在条件里就会在需要的时候消失。触发入口是上面那块「添加网盘」。
    -->
    <NetdiskAddDialog ref="netdiskDialogRef" />
  </SettingsItem>
</template>

<style scoped>
/*
  用户要求（2026-10-18）：标题文本单行显示，设置内容放在标题**下方**，
  而不是左右排版。

  SettingsItem 的 header 与内容区默认是 `flex-direction: row` 加
  `justify-content: space-between`，内容区被压成右侧一窄栏。放单个开关没问题，
  但连接表单有十几个字段，全挤在右栏会窄到没法用 —— 这正是实机验收抓到的。

  这里显式弹成竖排。**必须用 :deep() 限定影响**：`item.vue` 被另外 45 个设置项
  共用，「设为默认文件管理器」就依赖横排（标题在左、开关在右），直接改它会
  一次改掉全店。既有先例：`experimental/mcp-server.vue`。
*/

.remote-connections :deep(.settings-view-item__main) {
  flex-direction: column;
  align-items: stretch;
  gap: 0.75rem;
}

.remote-connections :deep(.settings-view-item__content) {
  min-width: 0;
  flex: 1;
}

/* 标题单行：长标题换行会把下面的表单推得更远，且和别的分类不一致。 */
.remote-connections :deep(.settings-view-item__title) {
  white-space: nowrap;
}

/*
  顶部的两个标签（2026-11-08）。

  标签列表拉满整行：两个标签的文字长度差很多（「网盘」两个字，
  「远程连接」四个字），不拉满的话两个标签的宽度不一样，
  看起来像两个不同控件。
*/

.remote-connections__tabs {
  margin-bottom: 0.75rem;
}

.remote-connections__tabs :deep(.sigma-ui-tabs-list) {
  display: flex;
  width: 100%;
  max-width: 100%;
}

/*
  分区介绍（2026-11-11）。

  跟着当前选项卡走 —— 介绍的是这个分区的事，就摆在这个分区里。
*/

.remote-connections__section-description {
  margin: 0 0 0.75rem;
  color: hsl(var(--muted-foreground));
  font-size: 0.875rem;
}

/*
  「添加网盘」区块（2026-11-11）。

  标题 + 两颗按钮平铺：这一栏本来就是「二选一」，平铺出来一眼可选，
  不必先展开下拉框才知道有几家网盘。
*/

.remote-connections__add-netdisk {
  display: flex;
  flex-direction: column;
  margin-top: 0.75rem;
  gap: 0.5rem;
}

.remote-connections__add-netdisk-title {
  margin: 0;
  color: hsl(var(--foreground));
  font-size: 0.875rem;
  font-weight: 600;
}

.remote-connections__add-netdisk-options {
  display: flex;
  gap: 0.5rem;
}

.remote-connections__list {
  display: flex;
  flex-direction: column;
  padding: 0;
  margin: 0;
  gap: 0.5rem;
  list-style: none;
}

.remote-connections__entry {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem;
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius);
  gap: 0.75rem;
}

.remote-connections__entry-main {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 0.25rem;
}

.remote-connections__entry-title {
  display: flex;
  align-items: center;
  color: hsl(var(--foreground));
  font-weight: 600;
  gap: 0.5rem;
}

.remote-connections__badge {
  padding: 0.05rem 0.4rem;
  border-radius: 999px;
  background-color: hsl(var(--muted));
  color: hsl(var(--muted-foreground));
  font-size: 0.7rem;
  font-weight: 400;
}

.remote-connections__entry-meta {
  display: flex;
  flex-wrap: wrap;
  color: hsl(var(--muted-foreground));
  font-size: 0.8rem;
  gap: 0.25rem 0.75rem;
}

/*
  本地目录带字段名渲染：裸路径混在协议/主机/初始目录里会被当成又一段远端信息，
  而它其实是本机路径，也是「会摆双屏」的唯一可见标志。
*/

.remote-connections__entry-local {
  color: hsl(var(--foreground));
}

.remote-connections__entry-actions {
  display: flex;
  gap: 0.5rem;
}

.remote-connections__empty {
  margin: 0;
  color: hsl(var(--muted-foreground));
  font-size: 0.875rem;
}
</style>
