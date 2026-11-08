<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Input } from '@/components/ui/input';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import { getRemoteProtocolDefaultPort, isBaiduPanProtocol } from '@/types/remote';
import normalizePath from '@/utils/normalize-path';
import type {
  BaiduAuthStatusPayload, RemoteConnection, RemoteConnectionForm, RemoteConnectionTestReport, RemoteProtocol, RemoteProxyConfig,
} from '@/types/remote';

type Props = {
  /** 传 `null` 表示新建。 */
  connection: RemoteConnection | null;
  /**
   * 新建时预置的协议（2026-11-08）。
   *
   * 不传则退回原来的默认值 `ftp`，行为保持不变。
   */
  initialProtocol?: RemoteProtocol;
  /**
   * 表单形态（2026-11-08）。
   *
   * - `netdisk`：**网盘**表单 —— 没有服务器地址、没有端口、没有用户名，
   *   凭据是一整段 Cookie，录入项按网盘重新组织过。
   * - `remote`：**远程连接**表单 —— FTP / FTPS / SFTP 那套主机、端口、用户名。
   * - `auto`（默认）：由 `connection.protocol` / `initialProtocol` 推断，
   *   与拆标签页之前的行为一致。
   */
  variant?: 'auto' | 'netdisk' | 'remote';
};

const props = defineProps<Props>();

const emit = defineEmits<{
  saved: [connection: RemoteConnection];
  cancel: [];
}>();

const { t } = useI18n();
const store = useRemoteConnectionsStore();

/**
 * 远程连接表单可选的协议。
 *
 * **不含百度网盘**：网盘有自己的标签页与专属的录入项，
 * 把它塞进这个下拉等于给用户一条「选了却填不了对应字段」的死路。
 */
const PROTOCOLS: RemoteProtocol[] = ['ftp', 'ftps_explicit', 'ftps_implicit', 'sftp'];
const PROXY_KINDS: RemoteProxyConfig['kind'][] = ['direct', 'socks5', 'http_connect'];

/**
 * 百度网盘的固定主机。
 *
 * 百度网盘没有「自选服务器」这回事，用户填什么 host 都会被后端忽略，
 * 而连接表里那个字段是必填的（Rust 侧 `RemoteConnection::validate` 要求非空）。
 * 所以界面上干脆不显示主机输入框，固定写死这个值。
 */
const BAIDU_PAN_HOST = 'pan.baidu.com';

/**
 * 「如何获取 Cookie」帮助块（2026-11-08）。
 *
 * 百度没有给第三方应用的 OAuth，Cookie 只能从**已登录的浏览器会话**里抠，
 * 本应用替不了用户登录。所以这里做两件事：
 * ① 一个直达登录页的外链，省掉手敲域名；
 * ② 把 F12 → 网络 → 复制 BDUSS 的步骤写在表单里，免得用户另开搜索。
 *
 * 步骤口径与后端解析规则严格对齐（`src-tauri/src/remote/baidu_auth.rs`）：
 * `BDUSS` 必需且**大小写敏感**，`STOKEN` 可选（只影响上传加速），
 * 裸值与整串 Cookie 两种粘法都接受 —— 说明里必须只写做得到、且被代码允许的动作。
 */
const BAIDU_PAN_URL = 'https://pan.baidu.com/';

/**
 * 「如何获取 Cookie」帮助块默认**展开**（2026-11-08）。
 *
 * 之前默认收起是为了不让这张已经很长的表单更长；现在网盘表单只剩四五个字段，
 * 而「Cookie 从哪儿抄」是这一步**唯一**的新知识 —— 藏起来等于让用户
 * 开着浏览器自己去搜教程。
 */
const cookieHelpOpen = ref(true);

function openBaiduLoginPage() {
  openUrl(BAIDU_PAN_URL);
}

const name = ref('');
const protocol = ref<RemoteProtocol>('ftp');
const host = ref('');
const port = ref<number>(21);
const username = ref('');
const password = ref('');
const clearPassword = ref(false);
const initialPath = ref('/');
const localPath = ref('');
const proxyKind = ref<RemoteProxyConfig['kind']>('direct');
const proxyHost = ref('');
const proxyPort = ref<number>(1080);
const proxyUsername = ref('');
const proxyPassword = ref('');

const hasSavedPassword = computed(() => Boolean(props.connection?.credentialKey));

/**
 * 这张表单是**网盘**表单还是**远程连接**表单。
 *
 * 网盘的录入项与 FTP 完全不同：没有主机、没有端口、没有用户名，
 * 凭据是一整段 Cookie 而不是口令 —— 所以形态由调用方直接指定（`variant`），
 * 只有没人指定时（`auto`）才从协议推断。
 *
 * ⚠️ **不能拿 `protocol.value` 判断**：改协议这个动作本身只有远程连接表单才有
 * （网盘表单根本没有协议下拉），拿它当判据会把形态判断绕进循环。
 */
const isNetdisk = computed(() => {
  if (props.variant === 'netdisk') {
    return true;
  }

  if (props.variant === 'remote') {
    return false;
  }

  return isBaiduPanProtocol(props.connection?.protocol ?? props.initialProtocol ?? 'ftp');
});

watch(
  () => props.connection,
  (connection) => {
    // 先收进局部常量，否则 `connection?.proxy.kind === 'direct' ? ... : connection.proxy.host`
    // 的 else 分支里 TS 不会把 `connection` 收窄成非空。
    const existingProxy = connection?.proxy;
    const isDirect = !existingProxy || existingProxy.kind === 'direct';
    // 网盘表单的协议恒为 `baidu_pan`：形态已经定了，协议没有第二个选择。
    const nextProtocol = props.variant === 'netdisk'
      ? 'baidu_pan'
      : (connection?.protocol ?? props.initialProtocol ?? 'ftp');

    name.value = connection?.name ?? '';
    protocol.value = nextProtocol;
    host.value = connection?.host ?? '';
    port.value = connection?.port ?? getRemoteProtocolDefaultPort(nextProtocol);

    // 百度网盘不认自定义主机 —— 主机与端口恒定。
    // 老记录里若被写成了别的值，这里纠正回来。
    if (isBaiduPanProtocol(nextProtocol)) {
      host.value = BAIDU_PAN_HOST;
      port.value = getRemoteProtocolDefaultPort(nextProtocol);
    }

    username.value = connection?.username ?? '';
    initialPath.value = connection?.initialPath ?? '/';
    localPath.value = connection?.localPath ?? '';
    password.value = '';
    clearPassword.value = false;

    proxyKind.value = isDirect ? 'direct' : existingProxy.kind;
    proxyHost.value = isDirect ? '' : existingProxy.host;
    proxyPort.value = isDirect ? 1080 : existingProxy.port;
    proxyUsername.value = isDirect ? '' : (existingProxy.username ?? '');
    proxyPassword.value = '';
  },
  { immediate: true },
);

function buildProxy(): RemoteProxyConfig {
  if (proxyKind.value === 'direct') {
    return {
      kind: 'direct',
    };
  }

  return {
    kind: proxyKind.value,
    host: proxyHost.value,
    port: proxyPort.value,
    ...(proxyUsername.value
      ? { username: proxyUsername.value }
      : {}),
    ...(proxyPassword.value
      ? { password: proxyPassword.value }
      : {}),
  };
}

function reset() {
  name.value = '';
  host.value = '';
  port.value = getRemoteProtocolDefaultPort(protocol.value);
  username.value = '';
  password.value = '';
  clearPassword.value = false;
  initialPath.value = '/';
  localPath.value = '';
  proxyKind.value = 'direct';
  proxyHost.value = '';
  proxyPort.value = 1080;
  proxyUsername.value = '';
  proxyPassword.value = '';
}

/** 选一个本地目录。留空即可不摆双屏，所以这里不做任何默认填充。 */
async function handleBrowseLocalPath() {
  const selected = await open({
    directory: true,
    multiple: false,
    defaultPath: localPath.value || undefined,
  });

  if (selected) {
    localPath.value = normalizePath(selected);
  }
}

/** store 抛出的就是校验码，直接映射成文案，不去解析 message。 */
const errorCode = ref<string | null>(null);

/**
 * 「测试连接」的结果。
 *
 * 两个字段都要留着：
 * - `report.outcome` 决定配色与结论措辞；
 * - `report.passwordChecked === false` 必须显示出来，否则「通过」会被读成
 *   「密码没问题」，而免密 / 匿名连接同样会通过。
 */
const probeReport = ref<RemoteConnectionTestReport | null>(null);
const isProbing = ref(false);

function buildForm(): RemoteConnectionForm {
  return {
    id: props.connection?.id,
    name: name.value,
    protocol: protocol.value,
    host: host.value,
    port: port.value,
    username: username.value,
    initialPath: initialPath.value,
    localPath: localPath.value,
    // 网盘走的是应用自己封的 HTTPS 通道，没有代理这一说 —— 恒为直连。
    proxy: isNetdisk.value ? { kind: 'direct' as const } : buildProxy(),
    password: password.value,
    clearPassword: clearPassword.value,
  };
}

/**
 * 真去连一次。
 *
 * **不写任何盘上的状态** —— 所以「测完了不保存」不会留下痕迹，
 * 也不会因为没保存就把存的密码搞坏。
 */
async function handleTestConnection() {
  isProbing.value = true;
  probeReport.value = null;
  errorCode.value = null;

  try {
    probeReport.value = await store.testConnection(buildForm());
  }
  catch (error) {
    // ⚠️ 这里**不能**沿用 `handleSubmit` 的 `error instanceof Error` 判定。
    // 保存路径抛的是 `new Error(校验码)`，走 instanceof 没问题；
    // 而这里抛的是 Tauri 把 `RemoteError` 序列化后的**普通对象**
    // `{ kind, message }` —— `instanceof` 为 false，于是落进 'unknown'，
    // 界面上蹦出「保存连接失败。」：一条测试错误，顶着保存的文案。
    // 这里只有一个失败模式（入参非法），所以统一成一条码。
    console.error('Failed to probe remote connection:', error);
    errorCode.value = 'testInvalidInput';
  }
  finally {
    isProbing.value = false;
  }
}

/**
 * 百度网盘的授权状态卡。
 *
 * **只显示状态，不显示凭据** —— 后端给回来的摘要里只有 cookie 的名字与长度，
 * 没有任何明文片段。没有这张卡，用户只能靠「测试连接」去猜自己的会话还活着没。
 */
const baiduStatus = ref<BaiduAuthStatusPayload | null>(null);
const isLoadingBaiduStatus = ref(false);

/** 只有「已保存的百度连接」才有状态可查 —— 新建中（还没 id）没有存档。 */
const baiduConnectionId = computed(() => (isNetdisk.value ? props.connection?.id : undefined));

async function refreshBaiduStatus() {
  if (!baiduConnectionId.value) {
    baiduStatus.value = null;
    return;
  }

  isLoadingBaiduStatus.value = true;

  try {
    baiduStatus.value = await store.getBaiduAuthStatus(baiduConnectionId.value);
  }
  catch (error) {
    console.error('Failed to load baidu auth status:', error);
    baiduStatus.value = null;
  }
  finally {
    isLoadingBaiduStatus.value = false;
  }
}

/** 删掉已保存的会话（退出登录）。删完状态卡立刻回到「未登录」。 */
async function handleDeleteBaiduSession() {
  if (!baiduConnectionId.value) return;

  try {
    await store.deleteBaiduSession(baiduConnectionId.value);
    baiduStatus.value = null;
  }
  catch (error) {
    console.error('Failed to delete baidu session:', error);
  }
}

watch(
  () => [props.connection?.id, protocol.value],
  () => {
    void refreshBaiduStatus();
  },
  { immediate: true },
);

function onProxyKindChange(value: unknown) {
  const next = String(value ?? '') as RemoteProxyConfig['kind'];

  if (!PROXY_KINDS.includes(next)) return;

  proxyKind.value = next;
}

async function handleSubmit() {
  const form = buildForm();

  errorCode.value = null;

  try {
    const saved = await store.saveConnection(form);
    reset();
    emit('saved', saved);
  }
  catch (error) {
    errorCode.value = error instanceof Error ? error.message : 'unknown';
  }
}

function onProtocolChange(value: unknown) {
  const next = String(value ?? '') as RemoteProtocol;

  if (!PROTOCOLS.includes(next)) return;

  const wasDefault = port.value === getRemoteProtocolDefaultPort(protocol.value);

  protocol.value = next;

  // 只在端口还是上一个协议的默认值时才跟着换 ——
  // 用户手填过 2121 就不该因为选了协议被改掉。
  if (wasDefault) {
    port.value = getRemoteProtocolDefaultPort(next);
  }

  // 百度网盘的主机与端口是固定值：用户没得选，
  // 也不该被上一个协议的默认值带偏。
  if (isBaiduPanProtocol(next)) {
    host.value = BAIDU_PAN_HOST;
    port.value = getRemoteProtocolDefaultPort(next);
  }
}
</script>

<template>
  <form
    class="remote-connection-form"
    @submit.prevent="handleSubmit"
  >
    <p
      v-if="errorCode"
      class="remote-connection-form__error"
      data-test="form-error"
    >
      {{ t(`settings.remote.error.${errorCode}`) }}
    </p>

    <div class="remote-connection-form__row">
      <Label for="remote-name">{{ t('settings.remote.field.name') }}</Label>
      <Input
        id="remote-name"
        v-model="name"
        data-test="field-name"
      />
    </div>

    <!--
      网盘表单没有「协议」这一项：形态已经定了，协议没有第二个选择。
      摆一个只有一项的下拉框只会让用户以为还能选别的东西。
    -->
    <div
      v-if="!isNetdisk"
      class="remote-connection-form__row"
    >
      <Label for="remote-protocol">{{ t('settings.remote.field.protocol') }}</Label>
      <Select
        :model-value="protocol"
        @update:model-value="onProtocolChange"
      >
        <SelectTrigger data-test="field-protocol">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="item in PROTOCOLS"
            :key="item"
            :value="item"
          >
            {{ t(`settings.remote.protocol.${item}`) }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <div
      v-if="!isNetdisk"
      class="remote-connection-form__row"
    >
      <Label for="remote-host">{{ t('settings.remote.field.host') }}</Label>
      <Input
        id="remote-host"
        v-model="host"
        data-test="field-host"
      />
    </div>

    <div
      v-if="!isNetdisk"
      class="remote-connection-form__row"
    >
      <Label for="remote-port">{{ t('settings.remote.field.port') }}</Label>
      <Input
        id="remote-port"
        v-model.number="port"
        type="number"
        data-test="field-port"
      />
    </div>

    <div
      v-if="!isNetdisk"
      class="remote-connection-form__row"
    >
      <Label for="remote-username">{{ t('settings.remote.field.username') }}</Label>
      <Input
        id="remote-username"
        v-model="username"
        data-test="field-username"
      />
    </div>

    <!--
      网盘的凭据是一整段 Cookie：用多行文本框，用户粘完能一眼看完有没有粘全。
      密码框在网盘这一栏不成立 —— 那不是口令，是一串会话凭据。
    -->
    <div
      v-if="isNetdisk"
      class="remote-connection-form__row"
    >
      <Label for="remote-cookie">{{ t('settings.remote.field.cookie') }}</Label>
      <textarea
        id="remote-cookie"
        v-model="password"
        class="remote-connection-form__cookie"
        rows="4"
        autocomplete="off"
        spellcheck="false"
        data-test="field-cookie"
        data-baidu="cookie"
        :placeholder="t('settings.remote.baidu.cookiePlaceholder')"
      />
      <p
        class="remote-connection-form__hint"
        data-test="hint-cookie"
      >
        {{ t('settings.remote.baidu.hint') }}
      </p>
    </div>

    <div
      v-else
      class="remote-connection-form__row"
    >
      <Label for="remote-password">{{ t('settings.remote.field.password') }}</Label>
      <Input
        id="remote-password"
        v-model="password"
        type="password"
        autocomplete="off"
        data-test="field-password"
      />
      <p
        v-if="hasSavedPassword"
        class="remote-connection-form__hint"
      >
        {{ t('settings.remote.password.savedHint') }}
      </p>
    </div>

    <!--
      获取 Cookie 的步骤说明（2026-11-08）：只跟着百度协议出现。
      默认收起 —— 这张表单已经够长，第一次填的人要的是「怎么弄」，
      回来换 Cookie 的人不想再读一遍步骤。
    -->
    <div
      v-if="isNetdisk"
      class="remote-connection-form__cookie-help"
      data-test="baidu-cookie-help"
    >
      <Collapsible v-model:open="cookieHelpOpen">
        <CollapsibleTrigger as-child>
          <Button
            type="button"
            variant="ghost"
            size="xs"
            data-test="baidu-cookie-help-toggle"
          >
            {{ t('settings.remote.baidu.help.toggle') }}
          </Button>
        </CollapsibleTrigger>
        <CollapsibleContent class="remote-connection-form__cookie-help-body">
          <Button
            type="button"
            variant="link"
            size="xs"
            data-test="baidu-open-login"
            @click="openBaiduLoginPage"
          >
            {{ t('settings.remote.baidu.help.openLogin') }}
          </Button>
          <ol class="remote-connection-form__cookie-help-steps">
            <li>{{ t('settings.remote.baidu.help.step1') }}</li>
            <li>{{ t('settings.remote.baidu.help.step2') }}</li>
            <li>{{ t('settings.remote.baidu.help.step3') }}</li>
            <li>{{ t('settings.remote.baidu.help.step4') }}</li>
            <li>{{ t('settings.remote.baidu.help.step5') }}</li>
          </ol>
          <ul class="remote-connection-form__cookie-help-notes">
            <li>{{ t('settings.remote.baidu.help.noteRequired') }}</li>
            <li>{{ t('settings.remote.baidu.help.noteExpiry') }}</li>
            <li>{{ t('settings.remote.baidu.help.notePrivacy') }}</li>
          </ul>
        </CollapsibleContent>
      </Collapsible>
    </div>

    <!-- 百度网盘的授权卡：只显示状态，不显示凭据。
         没有它，用户只能靠「测试连接」去猜自己的会话还有没有过期。 -->
    <div
      v-if="baiduConnectionId"
      class="remote-connection-form__baidu"
      data-test="baidu-status"
    >
      <div class="remote-connection-form__baidu-head">
        <span
          class="remote-connection-form__baidu-dot"
          :data-status="baiduStatus?.status ?? 'missing'"
          data-test="baidu-status-dot"
        />
        <span data-test="baidu-status-text">
          {{ baiduStatus
            ? t(`settings.remote.baidu.status.${baiduStatus.status}`)
            : t('settings.remote.baidu.status.loading') }}
        </span>
      </div>
      <p
        v-if="baiduStatus?.summary"
        class="remote-connection-form__hint"
        data-test="baidu-status-summary"
      >
        {{ baiduStatus.summary }}
      </p>
      <Button
        type="button"
        variant="outline"
        data-test="baidu-delete-session"
        @click="handleDeleteBaiduSession"
      >
        {{ t('settings.remote.baidu.action.deleteSession') }}
      </Button>
    </div>

    <div
      v-if="hasSavedPassword"
      class="remote-connection-form__row remote-connection-form__row--inline"
    >
      <Checkbox
        id="remote-clear-password"
        v-model="clearPassword"
        data-test="field-clear-password"
      />
      <Label for="remote-clear-password">
        {{ t('settings.remote.password.clear') }}
      </Label>
    </div>

    <div class="remote-connection-form__row">
      <Label for="remote-initial-path">{{ t('settings.remote.field.initialPath') }}</Label>
      <Input
        id="remote-initial-path"
        v-model="initialPath"
        data-test="field-initial-path"
      />
    </div>

    <div class="remote-connection-form__row">
      <Label for="remote-local-path">{{ t('settings.remote.field.localPath') }}</Label>
      <div class="remote-connection-form__inline">
        <Input
          id="remote-local-path"
          v-model="localPath"
          data-test="field-local-path"
        />
        <Button
          type="button"
          variant="outline"
          data-test="browse-local-path"
          @click="handleBrowseLocalPath"
        >
          {{ t('settings.remote.action.browse') }}
        </Button>
      </div>
      <p class="remote-connection-form__hint">
        {{ t('settings.remote.localPath.hint') }}
      </p>
    </div>

    <!-- 网盘走应用自己封的 HTTPS 通道，没有代理这一说 —— 整块不渲染。 -->
    <div
      v-if="!isNetdisk"
      class="remote-connection-form__row"
    >
      <Label for="remote-proxy-kind">{{ t('settings.remote.field.proxy') }}</Label>
      <Select
        :model-value="proxyKind"
        @update:model-value="onProxyKindChange"
      >
        <SelectTrigger data-test="field-proxy-kind">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="item in PROXY_KINDS"
            :key="item"
            :value="item"
          >
            {{ t(`settings.remote.proxy.${item}`) }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <template v-if="!isNetdisk && proxyKind !== 'direct'">
      <div class="remote-connection-form__row">
        <Label for="remote-proxy-host">{{ t('settings.remote.field.proxyHost') }}</Label>
        <Input
          id="remote-proxy-host"
          v-model="proxyHost"
          data-test="field-proxy-host"
        />
      </div>

      <div class="remote-connection-form__row">
        <Label for="remote-proxy-port">{{ t('settings.remote.field.proxyPort') }}</Label>
        <Input
          id="remote-proxy-port"
          v-model.number="proxyPort"
          type="number"
          data-test="field-proxy-port"
        />
      </div>

      <div class="remote-connection-form__row">
        <Label for="remote-proxy-username">{{ t('settings.remote.field.proxyUsername') }}</Label>
        <Input
          id="remote-proxy-username"
          v-model="proxyUsername"
          data-test="field-proxy-username"
        />
      </div>

      <div class="remote-connection-form__row">
        <Label for="remote-proxy-password">{{ t('settings.remote.field.proxyPassword') }}</Label>
        <Input
          id="remote-proxy-password"
          v-model="proxyPassword"
          type="password"
          autocomplete="off"
          data-test="field-proxy-password"
        />
      </div>
    </template>

    <div class="remote-connection-form__actions">
      <Button
        type="button"
        variant="outline"
        data-test="test-connection"
        :disabled="isProbing"
        @click="handleTestConnection"
      >
        {{ isProbing ? t('settings.remote.test.testing') : t('settings.remote.action.test') }}
      </Button>
      <Button
        type="submit"
        data-test="submit"
      >
        {{ t('settings.remote.action.save') }}
      </Button>
      <Button
        type="button"
        variant="outline"
        data-test="cancel"
        @click="emit('cancel')"
      >
        {{ t('settings.remote.action.cancel') }}
      </Button>
    </div>

    <!-- 结果区放在按钮**下方**：按钮先出现，用户点完往下看结论，
         不用在按钮和结论之间来回找视线。 -->
    <p
      v-if="probeReport"
      class="remote-connection-form__probe"
      :class="`remote-connection-form__probe--${probeReport.outcome}`"
      data-test="test-result"
      :data-outcome="probeReport.outcome"
    >
      <span class="remote-connection-form__probe-message">
        {{ t(`settings.remote.test.outcome.${probeReport.outcome}`) }}
      </span>
      <!-- 没核验密码就不能让「通过」裸奔。 -->
      <span
        v-if="probeReport.outcome === 'verified' && !probeReport.passwordChecked"
        class="remote-connection-form__probe-note"
        data-test="test-note-password"
      >
        {{ t('settings.remote.test.passwordNotChecked') }}
      </span>
      <!-- ⚠️ 只在 FTPS 上显示。SFTP 没有 TLS，跳过的是**主机密钥校验**，
           挂一条「TLS 证书未经验证」纯属张冠李戴。
           百度网盘走的是应用自己封的 HTTPS 通道，
           同样不该在这里挂「TLS 证书未经验证」。 -->
      <span
        v-if="probeReport.outcome === 'verified'
          && probeReport.tlsVerified === false
          && protocol !== 'sftp'
          && protocol !== 'baidu_pan'
          && probeReport.message"
        class="remote-connection-form__probe-note"
      >
        {{ t('settings.remote.test.tlsNotVerified') }}
      </span>
    </p>
  </form>
</template>

<style scoped>
.remote-connection-form {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.remote-connection-form__row {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

/*
  测试结论的配色：只有「通过」是绿的，其余一律中性偏警示。
  刻意不给「认证被拒」单独的红 —— 那不是程序故障，是用户要改密码，
  用报错色会让人以为应用出了问题。
*/
.remote-connection-form__probe {
  display: flex;
  flex-direction: column;
  margin: 0;
  gap: 0.25rem;
  padding: 0.5rem 0.75rem;
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius);
  color: hsl(var(--foreground));
  font-size: 0.875rem;
}

.remote-connection-form__probe--verified {
  border-color: hsl(var(--foreground) / 25%);
}

.remote-connection-form__probe-note {
  color: hsl(var(--muted-foreground));
  font-size: 0.8rem;
}

.remote-connection-form__row--inline {
  flex-direction: row;
  align-items: center;
  gap: 0.5rem;
}

.remote-connection-form__inline {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.remote-connection-form__hint {
  margin: 0;
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
}

.remote-connection-form__error {
  margin: 0;
  color: hsl(var(--destructive));
  font-size: 0.875rem;
}

/*
  网盘的 Cookie 输入框（2026-11-08）。

  用原生 textarea 而不是单行 Input：Cookie 是一长串，粘进去之后要能一眼
  看全没看全。样式跟着 `Input` 组件走（同样的边框、圆角、字号），
  免得两个控件摆在一起像两个不同世界。
*/
.remote-connection-form__cookie {
  width: 100%;
  padding: 0.5rem 0.75rem;
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius);
  background-color: hsl(var(--background));
  color: hsl(var(--foreground));
  font-family: inherit;
  font-size: 0.875rem;
  line-height: 1.5;
  resize: vertical;
}

.remote-connection-form__cookie:focus {
  outline: 2px solid hsl(var(--ring) / 50%);
  outline-offset: var(--ring-outline-offset);
}

/*
  「如何获取 Cookie」帮助块（2026-11-08）。
  收起时只剩一行触发按钮；展开后的正文比输入框矮一档、字号小一号 ——
  它是说明，不该比要填的字段更显眼。
*/
.remote-connection-form__cookie-help {
  display: flex;
  flex-direction: column;
}

.remote-connection-form__cookie-help-body {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin-top: 0.5rem;
  padding: 0.625rem 0.75rem;
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius);
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
  line-height: 1.5;
}

.remote-connection-form__cookie-help-login {
  align-self: flex-start;
  height: auto;
  padding: 0;
  font-size: 0.75rem;
}

.remote-connection-form__cookie-help-steps,
.remote-connection-form__cookie-help-notes {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  margin: 0;
  padding-left: 1.125rem;
}

/*
  百度网盘的授权卡。刻意做得比旁边的输入框矮一档 ——
  它是**状态**，不是可以编辑的东西。
*/
.remote-connection-form__baidu {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.5rem 0.75rem;
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius);
}

.remote-connection-form__baidu-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.875rem;
}

.remote-connection-form__baidu-dot {
  width: 0.5rem;
  height: 0.5rem;
  border-radius: 50%;
  background: hsl(var(--muted-foreground));
}

.remote-connection-form__baidu-dot[data-status='ready'] {
  background: hsl(var(--foreground) / 60%);
}

.remote-connection-form__baidu-dot[data-status='stoken_expired'],
.remote-connection-form__baidu-dot[data-status='stoken_expiring_soon'] {
  background: hsl(var(--destructive));
}

.remote-connection-form__actions {
  display: flex;
  gap: 0.5rem;
}
</style>
