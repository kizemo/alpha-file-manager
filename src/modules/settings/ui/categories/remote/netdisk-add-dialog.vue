<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<!-- FORK-MODIFICATION: 用户 2026-11-11 要求改造「添加网盘」流程。

     旧流程：分区层先选 provider（百度 / 夸克）→ 点「添加网盘」→ 弹一张内嵌表单
     → 用户在表单里点「用内嵌窗口登录」→ 登录 → 点「保存」。
     新流程：点「百度网盘」/「夸克网盘」→ **直接**开出内嵌登录窗 → 登录成功、
     登录窗自己关掉 → 才弹出「名称 / 本地目录 / 初始目录」录入弹窗 → 显示「已登入」
     → 点保存才落盘（只有到这一步才有连接 id 可以认领后端的待认领登录态）。

     **本组件必须常驻挂载。** 内嵌登录窗是 Rust 侧的独立窗口，它把进度推到
     `afm://baidu-login` / `afm://quark-login` 频道；组件得一直在那儿听着，
     否则「登录成功」那一刻没人接，录入弹窗就永远不出现。 -->

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { open as openDirectoryDialog } from '@tauri-apps/plugin-dialog';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import { toast } from '@/components/ui/toaster';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import { getRemoteProtocolDefaultPort } from '@/types/remote';
import type {
  BaiduLoginEventPayload,
  QuarkLoginEventPayload,
  RemoteConnectionForm,
} from '@/types/remote';
import normalizePath from '@/utils/normalize-path';

/** 走这条「登录 → 录入」流程的两种网盘协议。 */
type NetdiskProtocol = 'baidu_pan' | 'quark_pan';

/** 内嵌登录窗推上来的状态。 */
type LoginStatus = 'ready' | 'closed' | 'timeout' | 'error';

/**
 * 整个流程的阶段。
 *
 * - `idle`：没有登录在等。
 * - `waiting`：内嵌登录窗已开，等用户登录。
 * - `form`：登录成功，录入弹窗开着（登录态已被后端暂存，等保存时认领）。
 * - `saved`：已保存 —— 登录态已被认领走，**不能再调 cancel**，否则会把它丢掉。
 */
type LoginPhase = 'idle' | 'waiting' | 'form' | 'saved';

const { t } = useI18n();
const store = useRemoteConnectionsStore();

const isOpen = ref(false);
const protocol = ref<NetdiskProtocol>('baidu_pan');
const loginPhase = ref<LoginPhase>('idle');
const isStarting = ref(false);
const isSaving = ref(false);
const errorCode = ref<string | null>(null);

const name = ref('');
const localPath = ref('');
const initialPath = ref('/');

/** provider → i18n 前缀。百度与夸克的文案键结构一样，只是前缀不同。 */
function i18nPrefix(target: NetdiskProtocol): 'baidu' | 'quark' {
  return target === 'baidu_pan' ? 'baidu' : 'quark';
}

/**
 * 丢弃后端「待认领槽位」里的登录态。
 *
 * 后端只有**一份**槽位（按 provider 各一份），留着会让下一次保存认领到错的会话。
 * 所以取消流程一定要清掉它 —— 哪怕登录已经成功、弹窗都开了。
 */
async function cancelLogin(target: NetdiskProtocol): Promise<void> {
  try {
    if (target === 'baidu_pan') {
      await store.cancelBaiduLogin();
    }
    else {
      await store.cancelQuarkLogin();
    }
  }
  catch (error) {
    console.error('Failed to cancel embedded netdisk login:', error);
  }
}

/**
 * 开始添加一个网盘：直接开出对应 provider 的内嵌登录窗。
 *
 * **先开登录窗、后填表单** —— 登录态由后端暂存在「待认领槽位」里，
 * 只有落盘时（那时才有连接 id）才认领；所以录入信息放在登录成功之后。
 */
async function startAdd(next: NetdiskProtocol): Promise<void> {
  if (isStarting.value) {
    return;
  }

  // 换 provider 时先把上一个没收尾的登录态丢掉。
  if (loginPhase.value !== 'idle' && loginPhase.value !== 'saved' && protocol.value !== next) {
    await cancelLogin(protocol.value);
  }

  isStarting.value = true;
  errorCode.value = null;
  protocol.value = next;
  loginPhase.value = 'waiting';
  name.value = '';
  localPath.value = '';
  initialPath.value = '/';

  try {
    if (next === 'baidu_pan') {
      await store.startBaiduLogin();
    }
    else {
      await store.startQuarkLogin();
    }
  }
  catch (error) {
    console.error('Failed to open embedded netdisk login window:', error);
    loginPhase.value = 'idle';
    toast({
      title: t(`settings.remote.${i18nPrefix(next)}.embedded.status.error`),
    });
  }
  finally {
    isStarting.value = false;
  }
}

/**
 * 处理内嵌登录窗推来的状态。
 *
 * **只认「当前正等着这个 provider」的流程** —— 上一次登录迟到的事件
 * 不能把这一次的状态改掉（否则会出现「刚点了夸克，弹的却是百度的表单」）。
 */
function applyLoginEvent(
  target: NetdiskProtocol,
  status: LoginStatus,
  summary: string | null,
): void {
  if (protocol.value !== target || loginPhase.value !== 'waiting') {
    return;
  }

  if (status === 'ready') {
    // 登录成功。Rust 侧的登录窗此时已经自己关掉，
    // 登录态在「待认领槽位」里等着被保存动作认领。
    loginPhase.value = 'form';
    isOpen.value = true;
    toast({
      title: t('settings.remote.netdisk.signedIn'),
      description: summary || undefined,
    });
    return;
  }

  // closed / timeout / error：登录窗被关掉、等待超时、或读不到会话。
  loginPhase.value = 'idle';
  toast({
    title: t(`settings.remote.${i18nPrefix(target)}.embedded.status.${status}`),
    description: summary || undefined,
  });
}

/** 选一个本地目录。留空即可不摆双屏，所以这里不做任何默认填充。 */
async function handleBrowseLocalPath(): Promise<void> {
  const selected = await openDirectoryDialog({
    directory: true,
    multiple: false,
    defaultPath: localPath.value || undefined,
  });

  if (selected) {
    localPath.value = normalizePath(selected);
  }
}

/**
 * 取消添加。
 *
 * 无论停在哪个阶段都要清掉待认领的登录态 —— 否则它会一直躺在后端的槽位里，
 * 下一次添加别的网盘时可能被误认领。
 */
async function handleCancel(): Promise<void> {
  const target = protocol.value;

  // **先改 phase 再关弹窗**：`isOpen` 变化会引发 `update:open`，
  // 那时 phase 已经不是 `form` 了，不会二次触发取消流程。
  loginPhase.value = 'idle';
  isOpen.value = false;
  errorCode.value = null;
  name.value = '';
  localPath.value = '';
  initialPath.value = '/';

  await cancelLogin(target);
}

/** 弹窗被关掉（右上角 X / 遮罩 / ESC）—— 一律当作取消。 */
function handleOpenChange(next: boolean): void {
  if (next) {
    return;
  }

  if (loginPhase.value === 'form') {
    void handleCancel();
    return;
  }

  loginPhase.value = 'idle';
}

/**
 * 保存：落盘一条网盘连接。
 *
 * 登录态由后端的「待认领槽位」带过来，**不经过前端** ——
 * 前端从头到尾拿不到 Cookie 明文，所以密码恒为空串。
 */
async function handleSave(): Promise<void> {
  if (isSaving.value) {
    return;
  }

  const trimmedName = name.value.trim();

  if (!trimmedName) {
    errorCode.value = 'nameRequired';
    return;
  }

  errorCode.value = null;
  isSaving.value = true;

  const target = protocol.value;
  const form: RemoteConnectionForm = {
    name: trimmedName,
    protocol: target,
    host: target === 'baidu_pan' ? 'pan.baidu.com' : 'pan.quark.cn',
    port: getRemoteProtocolDefaultPort(target),
    username: '',
    initialPath: initialPath.value.trim() || '/',
    localPath: localPath.value.trim(),
    proxy: { kind: 'direct' },
    password: '',
    clearPassword: false,
  };

  try {
    await store.saveConnection(form);
    // 保存成功 = 登录态已被认领。此时**不能**再调 cancelLogin ——
    // 那会把刚认领走的会话丢掉。
    loginPhase.value = 'saved';
    isOpen.value = false;
    name.value = '';
    localPath.value = '';
    initialPath.value = '/';
  }
  catch (error) {
    // store 抛出的就是校验码（`nameRequired` / `quarkLoginRequired` 等），
    // 直接映射成文案，不去解析 message。
    errorCode.value = error instanceof Error ? error.message : 'unknown';
  }
  finally {
    isSaving.value = false;
  }
}

/**
 * 订阅登录进度。
 *
 * ⚠️ `listen` 是**异步注册**的，组件可能在它返回之前就卸载了 ——
 * 卸载时除了调 `unlisten`，还要用 `disposed` 兜住「注册晚到」这一手。
 */
let unlistenBaiduLogin: UnlistenFn | null = null;
let unlistenQuarkLogin: UnlistenFn | null = null;
let disposed = false;

onMounted(async () => {
  try {
    const unlisten = await listen<BaiduLoginEventPayload>('afm://baidu-login', (event) => {
      const payload = event.payload;
      applyLoginEvent('baidu_pan', payload.status, payload.summary ?? payload.message ?? null);
    });

    if (disposed) {
      unlisten();
      return;
    }

    unlistenBaiduLogin = unlisten;
  }
  catch (error) {
    console.error('Failed to listen for embedded baidu login events:', error);
  }

  try {
    const unlisten = await listen<QuarkLoginEventPayload>('afm://quark-login', (event) => {
      const payload = event.payload;
      applyLoginEvent('quark_pan', payload.status, payload.summary ?? payload.message ?? null);
    });

    if (disposed) {
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
  disposed = true;
  unlistenBaiduLogin?.();
  unlistenBaiduLogin = null;
  unlistenQuarkLogin?.();
  unlistenQuarkLogin = null;
});

defineExpose({ startAdd });
</script>

<template>
  <Dialog
    :open="isOpen"
    @update:open="handleOpenChange"
  >
    <DialogContent class="netdisk-add-dialog">
      <DialogHeader>
        <DialogTitle>{{ t('settings.remote.nav.addNetdisk') }}</DialogTitle>
      </DialogHeader>

      <!--
        「已登入」徽标：登录已经成功，登录态在后端暂存着，保存时才落盘。
        摆在这里是让用户知道「前面那一步已经成了」，不用去猜。
      -->
      <p
        class="netdisk-add-dialog__signed-in"
        data-test="signed-in"
      >
        {{ t('settings.remote.netdisk.signedIn') }}
      </p>

      <p
        v-if="errorCode"
        class="netdisk-add-dialog__error"
        data-test="form-error"
      >
        {{ t(`settings.remote.error.${errorCode}`) }}
      </p>

      <div class="netdisk-add-dialog__row">
        <Label for="netdisk-add-name">{{ t('settings.remote.field.name') }}</Label>
        <Input
          id="netdisk-add-name"
          v-model="name"
          data-test="field-name"
        />
      </div>

      <div class="netdisk-add-dialog__row">
        <Label for="netdisk-add-local-path">{{ t('settings.remote.field.localPath') }}</Label>
        <div class="netdisk-add-dialog__row-inline">
          <Input
            id="netdisk-add-local-path"
            v-model="localPath"
            data-test="field-local-path"
          />
          <Button
            variant="outline"
            size="sm"
            data-test="browse-local-path"
            @click="handleBrowseLocalPath"
          >
            {{ t('settings.remote.action.browse') }}
          </Button>
        </div>
        <p class="netdisk-add-dialog__hint">
          {{ t('settings.remote.localPath.hint') }}
        </p>
      </div>

      <div class="netdisk-add-dialog__row">
        <Label for="netdisk-add-initial-path">{{ t('settings.remote.field.initialPath') }}</Label>
        <Input
          id="netdisk-add-initial-path"
          v-model="initialPath"
          data-test="field-initial-path"
        />
      </div>

      <DialogFooter>
        <Button
          variant="outline"
          data-test="cancel"
          @click="handleCancel"
        >
          {{ t('settings.remote.action.cancel') }}
        </Button>
        <Button
          :disabled="isSaving"
          data-test="save"
          @click="handleSave"
        >
          {{ t('settings.remote.action.save') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
.netdisk-add-dialog {
  width: 420px;
  max-width: calc(100vw - 32px);
  box-sizing: border-box;
  overflow-x: hidden;
}

.netdisk-add-dialog > * {
  min-width: 0;
}

/*
  「已登入」徽标：用强调色的小标签，与普通说明文字区分开 ——
  它是**状态**，不是提示语，两者混在一段小字里会被当成一句说明读过去。
*/

.netdisk-add-dialog__signed-in {
  align-self: flex-start;
  padding: 0.05rem 0.5rem;
  border-radius: 999px;
  margin: 0;
  background-color: hsl(var(--secondary));
  color: hsl(var(--foreground));
  font-size: 0.75rem;
  font-weight: 600;
}

.netdisk-add-dialog__error {
  margin: 0;
  color: hsl(var(--destructive));
  font-size: 0.75rem;
}

.netdisk-add-dialog__row {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 0.25rem;
}

.netdisk-add-dialog__row-inline {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 0.5rem;
}

.netdisk-add-dialog__row-inline > :first-child {
  min-width: 0;
  flex: 1;
}

.netdisk-add-dialog__hint {
  margin: 0;
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
}
</style>
