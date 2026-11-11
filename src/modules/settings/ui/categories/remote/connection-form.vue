<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import {
  computed,
  onMounted,
  onUnmounted,
  ref,
  watch,
} from 'vue';
import { useI18n } from 'vue-i18n';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { Input } from '@/components/ui/input';
import { toast } from '@/components/ui/toaster';
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import { getRemoteProtocolDefaultPort, isBaiduPanProtocol, isNetdiskProtocol, isQuarkPanProtocol } from '@/types/remote';
import normalizePath from '@/utils/normalize-path';
import type {
  BaiduAuthStatusPayload, BaiduLoginEventPayload, QuarkAuthStatusPayload, QuarkLoginEventPayload, RemoteConnection, RemoteConnectionForm, RemoteConnectionTestReport, RemoteProtocol, RemoteProxyConfig,
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
  /**
   * 网盘分区选中的 **provider**（2026-11-11 夸克网盘）。
   *
   * 网盘协议不在协议下拉里（`PROTOCOLS` 刻意不含网盘协议），
   * 所以「选哪个网盘」由上游分区层决定，再通过这个 prop 传进来。
   *
   * **不传时默认百度** —— 既有调用点的行为逐字不变。
   */
  netdiskProtocol?: 'baidu_pan' | 'quark_pan';
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
 * 夸克网盘的固定主机。
 *
 * 与百度侧同构：界面上不显示主机输入框，固定写死这个值。
 */
const QUARK_PAN_HOST = 'pan.quark.cn';

/**
 * 内嵌登录（2026-11-08）。
 *
 * 登录窗由 Rust 侧创建（`src-tauri/src/baidu_login.rs`），cookie 也在
 * **Rust 侧**读回并暂存 —— 前端从头到尾拿不到明文，只拿得到状态与摘要。
 * 落盘发生在用户点「保存」时（`saveConnection` 里认领），因为只有走到
 * 那一步才有连接 id 可以当凭据键。
 */
type BaiduLoginViewState = 'idle' | 'waiting' | BaiduLoginEventPayload['status'];

const baiduLoginState = ref<BaiduLoginViewState>('idle');
const baiduLoginSummary = ref('');
const isStartingBaiduLogin = ref(false);

async function handleStartBaiduLogin() {
  if (isStartingBaiduLogin.value) return;

  isStartingBaiduLogin.value = true;
  baiduLoginSummary.value = '';
  baiduLoginState.value = 'waiting';

  try {
    await store.startBaiduLogin();
  }
  catch (error) {
    console.error('Failed to open embedded baidu login:', error);
    baiduLoginState.value = 'error';
  }
  finally {
    isStartingBaiduLogin.value = false;
  }
}

async function handleCancelBaiduLogin() {
  try {
    await store.cancelBaiduLogin();
  }
  catch (error) {
    console.error('Failed to cancel embedded baidu login:', error);
  }
  finally {
    baiduLoginState.value = 'idle';
    baiduLoginSummary.value = '';
  }
}

/**
 * 夸克网盘的内嵌登录（2026-11-11）。
 *
 * 与百度侧同构：登录窗由 Rust 侧建，登录态也只在 Rust 侧暂存 ——
 * 前端从头到尾拿不到明文，只拿得到状态与摘要。
 */
type QuarkLoginViewState = 'idle' | 'waiting' | 'ready' | 'closed' | 'timeout' | 'error';

const quarkLoginState = ref<QuarkLoginViewState>('idle');
const quarkLoginSummary = ref('');
const isStartingQuarkLogin = ref(false);

async function handleStartQuarkLogin() {
  if (isStartingQuarkLogin.value) return;

  isStartingQuarkLogin.value = true;
  quarkLoginSummary.value = '';
  quarkLoginState.value = 'waiting';

  try {
    await store.startQuarkLogin();
  }
  catch (error) {
    console.error('Failed to open embedded quark login:', error);
    quarkLoginState.value = 'error';
  }
  finally {
    isStartingQuarkLogin.value = false;
  }
}

async function handleCancelQuarkLogin() {
  try {
    await store.cancelQuarkLogin();
  }
  catch (error) {
    console.error('Failed to cancel embedded quark login:', error);
  }
  finally {
    quarkLoginState.value = 'idle';
    quarkLoginSummary.value = '';
  }
}

/**
 * 订阅登录进度。
 *
 * ⚠️ `listen` 是**异步注册**的，组件可能在它返回之前就卸载了 ——
 * 卸载时除了调 `unlisten`，还要用 `disposed` 兜住「注册晚到」这一手，
 * 否则每开一次表单就漏一个监听器。
 */
let unlistenBaiduLogin: UnlistenFn | null = null;
let baiduLoginDisposed = false;

onMounted(async () => {
  try {
    const unlisten = await listen<BaiduLoginEventPayload>('afm://baidu-login', (event) => {
      const payload = event.payload;
      const previousState = baiduLoginState.value;

      baiduLoginState.value = payload.status;
      baiduLoginSummary.value = payload.summary ?? payload.message ?? '';

      // `ready` 是内嵌登录唯一可用的成功信号：窗口一旦认领到 Cookie 就会自己关掉，
      // 而 Rust 侧的 settle() 会把 Destroyed 分支吃掉，不会再发 `closed`。
      // 所以成功反馈必须挂在 ready 的**第一次**到达上，重复推送不重播。
      if (payload.status === 'ready' && previousState !== 'ready') {
        toast({
          title: t('settings.remote.baidu.embedded.saved'),
          description: payload.summary || undefined,
        });
      }
      else if (payload.status !== previousState) {
        // closed / timeout / error：登录窗被关掉、等待超时、或读不到会话。
        // 这三种以前只在表单里换一行浅色小字 —— 用户当时多半正看着登录窗，
        // 根本没注意到，于是整轮登录「没有任何反馈」。改成可见的 toast。
        toast({
          title: t(`settings.remote.baidu.embedded.status.${payload.status}`),
          description: payload.summary || payload.message || undefined,
        });
      }
    });

    if (baiduLoginDisposed) {
      unlisten();
      return;
    }

    unlistenBaiduLogin = unlisten;
  }
  catch (error) {
    console.error('Failed to listen for embedded baidu login events:', error);
  }
});

/**
 * 夸克侧同构：订阅 `afm://quark-login`。
 *
 * 与百度**分开注册、分开清理** —— 两个事件名不同，混在一个监听器里
 * 就分不清「这条状态是谁发的」。
 */
let unlistenQuarkLogin: UnlistenFn | null = null;
let quarkLoginDisposed = false;

onMounted(async () => {
  try {
    const unlisten = await listen<QuarkLoginEventPayload>('afm://quark-login', (event) => {
      const payload = event.payload;
      const previousState = quarkLoginState.value;

      quarkLoginState.value = payload.status;
      quarkLoginSummary.value = payload.summary ?? payload.message ?? '';

      // `ready` 是内嵌登录唯一可用的成功信号：窗口一旦认领到 Cookie 就会自己关掉，
      // Rust 侧的 settle() 会把 Destroyed 分支吃掉，不会再发 `closed`。
      // 所以成功反馈必须挂在 ready 的**第一次**到达上，重复推送不重播。
      if (payload.status === 'ready' && previousState !== 'ready') {
        toast({
          title: t('settings.remote.quark.embedded.saved'),
          description: payload.summary || undefined,
        });
      }
      else if (payload.status !== previousState) {
        // closed / timeout / error：登录窗被关掉、等待超时、或读不到会话。
        toast({
          title: t(`settings.remote.quark.embedded.status.${payload.status}`),
          description: payload.summary || payload.message || undefined,
        });
      }
    });

    if (quarkLoginDisposed) {
      unlisten();
      return;
    }

    unlistenQuarkLogin = unlisten;
  }
  catch (error) {
    console.error('Failed to listen for embedded quark login events:', error);
  }
});

onUnmounted(() => {
  baiduLoginDisposed = true;
  unlistenBaiduLogin?.();
  unlistenBaiduLogin = null;
  quarkLoginDisposed = true;
  unlistenQuarkLogin?.();
  unlistenQuarkLogin = null;
});

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

  return isNetdiskProtocol(props.connection?.protocol ?? props.initialProtocol ?? 'ftp');
});

/**
 * 这张网盘表单是**夸克**还是**百度**。
 *
 * 编辑既有连接时以连接自身的协议为准；新建时看上游分区层选的 provider
 * （`netdiskProtocol` prop，不传则默认百度 —— 与既有行为一致）。
 */
const isQuarkNetdisk = computed(() => {
  if (props.connection) {
    return props.connection.protocol === 'quark_pan';
  }

  return props.variant === 'netdisk' && props.netdiskProtocol === 'quark_pan';
});

watch(
  () => props.connection,
  (connection) => {
    // 先收进局部常量，否则 `connection?.proxy.kind === 'direct' ? ... : connection.proxy.host`
    // 的 else 分支里 TS 不会把 `connection` 收窄成非空。
    const existingProxy = connection?.proxy;
    const isDirect = !existingProxy || existingProxy.kind === 'direct';
    // 网盘表单的协议由上游分区层选的 provider 决定（默认百度）。
    // ⚠️ **编辑既有连接时以连接自身的协议为准** —— 上游传进来的 provider
    // 只是「新建时预置什么」，不该把一条已有的夸克连接改写成百度。
    const nextProtocol = connection?.protocol
      ?? (props.variant === 'netdisk'
        ? (props.netdiskProtocol ?? 'baidu_pan')
        : (props.initialProtocol ?? 'ftp'));

    name.value = connection?.name ?? '';
    protocol.value = nextProtocol;
    host.value = connection?.host ?? '';
    port.value = connection?.port ?? getRemoteProtocolDefaultPort(nextProtocol);

    // 网盘不认自定义主机 —— 主机与端口恒定。
    // 老记录里若被写成了别的值，这里纠正回来。
    if (isBaiduPanProtocol(nextProtocol)) {
      host.value = BAIDU_PAN_HOST;
      port.value = getRemoteProtocolDefaultPort(nextProtocol);
    }
    else if (isQuarkPanProtocol(nextProtocol)) {
      host.value = QUARK_PAN_HOST;
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
  // 保存成功后这一表单回到「新建」，内嵌登录的提示也得跟着归零 ——
  // 否则下一条连接会带着上一条的「已登录」假象。
  baiduLoginState.value = 'idle';
  baiduLoginSummary.value = '';
  quarkLoginState.value = 'idle';
  quarkLoginSummary.value = '';
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

/**
 * 夸克网盘的授权状态卡。与百度侧同构：只显示状态，不显示凭据。
 */
const quarkStatus = ref<QuarkAuthStatusPayload | null>(null);
const isLoadingQuarkStatus = ref(false);

/** 只有「已保存的夸克连接」才有状态可查 —— 新建中（还没 id）没有存档。 */
const quarkConnectionId = computed(() => (
  isQuarkNetdisk.value ? props.connection?.id : undefined
));

async function refreshQuarkStatus() {
  if (!quarkConnectionId.value) {
    quarkStatus.value = null;
    return;
  }

  isLoadingQuarkStatus.value = true;

  try {
    quarkStatus.value = await store.getQuarkAuthStatus(quarkConnectionId.value);
  }
  catch (error) {
    console.error('Failed to load quark auth status:', error);
    quarkStatus.value = null;
  }
  finally {
    isLoadingQuarkStatus.value = false;
  }
}

/** 删掉已保存的夸克会话（退出登录）。删完状态卡立刻回到「未登录」。 */
async function handleDeleteQuarkSession() {
  if (!quarkConnectionId.value) return;

  try {
    await store.deleteQuarkSession(quarkConnectionId.value);
    quarkStatus.value = null;
  }
  catch (error) {
    console.error('Failed to delete quark session:', error);
  }
}

watch(
  () => [props.connection?.id, protocol.value],
  () => {
    void refreshBaiduStatus();
    void refreshQuarkStatus();
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
      内嵌登录（2026-11-08）：百度网盘的凭据是一整段 Cookie，手工抓取既难又危险，
      所以这里只留一条路 —— 应用内登录，登录成功后由 Rust 侧把会话写进待认领槽位，
      保存连接时再落盘。Cookie 明文永远不进前端，表单里也不再摆手工输入框。
    -->
    <div
      v-if="isNetdisk && !isQuarkNetdisk"
      class="remote-connection-form__row"
      data-test="baidu-embedded"
    >
      <Label>{{ t('settings.remote.baidu.embedded.title') }}</Label>
      <div class="remote-connection-form__inline">
        <Button
          type="button"
          variant="outline"
          data-test="baidu-embedded-start"
          :disabled="isStartingBaiduLogin"
          @click="handleStartBaiduLogin"
        >
          {{ t('settings.remote.baidu.embedded.start') }}
        </Button>
        <Button
          v-if="baiduLoginState !== 'idle' && baiduLoginState !== 'ready'"
          type="button"
          variant="ghost"
          data-test="baidu-embedded-cancel"
          @click="handleCancelBaiduLogin"
        >
          {{ t('settings.remote.baidu.embedded.cancel') }}
        </Button>
      </div>
      <p
        class="remote-connection-form__hint"
        data-test="baidu-embedded-hint"
      >
        {{ t('settings.remote.baidu.embedded.hint') }}
      </p>
      <p
        v-if="baiduLoginState !== 'idle'"
        class="remote-connection-form__hint"
        :data-status="baiduLoginState"
        data-test="baidu-embedded-status"
      >
        {{ baiduLoginState === 'waiting'
          ? t('settings.remote.baidu.embedded.status.waiting')
          : t(`settings.remote.baidu.embedded.status.${baiduLoginState}`) }}
      </p>
      <p
        v-if="baiduLoginSummary"
        class="remote-connection-form__hint"
        data-test="baidu-embedded-summary"
      >
        {{ baiduLoginSummary }}
      </p>
    </div>

    <!--
      夸克网盘的内嵌登录（2026-11-11）：与百度侧同构 ——
      凭据是一整段会话 Cookie，表单里不摆手工输入框，
      只给「用内嵌窗口登录」这一条路。
    -->
    <div
      v-else-if="isQuarkNetdisk"
      class="remote-connection-form__row"
      data-test="quark-embedded"
    >
      <Label>{{ t('settings.remote.quark.embedded.title') }}</Label>
      <div class="remote-connection-form__inline">
        <Button
          type="button"
          variant="outline"
          data-test="quark-embedded-start"
          :disabled="isStartingQuarkLogin"
          @click="handleStartQuarkLogin"
        >
          {{ t('settings.remote.quark.embedded.start') }}
        </Button>
        <Button
          v-if="quarkLoginState !== 'idle' && quarkLoginState !== 'ready'"
          type="button"
          variant="ghost"
          data-test="quark-embedded-cancel"
          @click="handleCancelQuarkLogin"
        >
          {{ t('settings.remote.quark.embedded.cancel') }}
        </Button>
      </div>
      <p
        class="remote-connection-form__hint"
        data-test="quark-embedded-hint"
      >
        {{ t('settings.remote.quark.embedded.hint') }}
      </p>
      <p
        v-if="quarkLoginState !== 'idle'"
        class="remote-connection-form__hint"
        :data-status="quarkLoginState"
        data-test="quark-embedded-status"
      >
        {{ quarkLoginState === 'waiting'
          ? t('settings.remote.quark.embedded.status.waiting')
          : t(`settings.remote.quark.embedded.status.${quarkLoginState}`) }}
      </p>
      <p
        v-if="quarkLoginSummary"
        class="remote-connection-form__hint"
        data-test="quark-embedded-summary"
      >
        {{ quarkLoginSummary }}
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

    <!-- 夸克网盘的授权卡：只显示状态，不显示凭据。 -->
    <div
      v-if="quarkConnectionId"
      class="remote-connection-form__baidu"
      data-test="quark-status"
    >
      <div class="remote-connection-form__baidu-head">
        <span
          class="remote-connection-form__baidu-dot"
          :data-status="quarkStatus?.status ?? 'missing'"
          data-test="quark-status-dot"
        />
        <span data-test="quark-status-text">
          {{ quarkStatus
            ? t(`settings.remote.quark.status.${quarkStatus.status}`)
            : t('settings.remote.quark.status.loading') }}
        </span>
      </div>
      <p
        v-if="quarkStatus?.summary"
        class="remote-connection-form__hint"
        data-test="quark-status-summary"
      >
        {{ quarkStatus.summary }}
      </p>
      <Button
        type="button"
        variant="outline"
        data-test="quark-delete-session"
        @click="handleDeleteQuarkSession"
      >
        {{ t('settings.remote.quark.action.deleteSession') }}
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
      <!-- 后端的 `message` 是「给用户看的一句话」，比按 outcome 查的通用文案
           具体得多。**只用于展示，不参与任何逻辑判断。**
           2026-11-09 用户反馈：百度网盘扫码登录成功，点「测试连接」却被告知
           「服务器拒绝了该用户名或密码」—— 当时后端的具体诊断（「还没有登录凭据」）
           被这里丢掉，只剩一句与事实相反的通用文案。 -->
      <span
        v-if="probeReport.message"
        class="remote-connection-form__probe-detail"
        data-test="test-detail"
      >
        {{ probeReport.message }}
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
          && protocol !== 'quark_pan'
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
  padding: 0.5rem 0.75rem;
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius);
  margin: 0;
  color: hsl(var(--foreground));
  font-size: 0.875rem;
  gap: 0.25rem;
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
  padding: 0.625rem 0.75rem;
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius);
  margin-top: 0.5rem;
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
  gap: 0.5rem;
  line-height: 1.5;
}

.remote-connection-form__cookie-help-login {
  height: auto;
  align-self: flex-start;
  padding: 0;
  font-size: 0.75rem;
}

.remote-connection-form__cookie-help-steps,
.remote-connection-form__cookie-help-notes {
  display: flex;
  flex-direction: column;
  padding-left: 1.125rem;
  margin: 0;
  gap: 0.25rem;
}

/*
  百度网盘的授权卡。刻意做得比旁边的输入框矮一档 ——
  它是**状态**，不是可以编辑的东西。
*/

.remote-connection-form__baidu {
  display: flex;
  flex-direction: column;
  padding: 0.5rem 0.75rem;
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius);
  gap: 0.5rem;
}

.remote-connection-form__baidu-head {
  display: flex;
  align-items: center;
  font-size: 0.875rem;
  gap: 0.5rem;
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
.remote-connection-form__baidu-dot[data-status='stoken_expiring_soon'],
.remote-connection-form__baidu-dot[data-status='stale'],
.remote-connection-form__baidu-dot[data-status='stale_soon'] {
  background: hsl(var(--destructive));
}

.remote-connection-form__actions {
  display: flex;
  gap: 0.5rem;
}
</style>
