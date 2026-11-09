// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 远程连接的**元数据**由本 store 经 tauri-plugin-store 拥有（落在
// `user-data/user-remote-connections.json`），密码由 Rust 侧经 DPAPI 拥有
// （落在 app-data 目录根的 `remote-credentials.json`）。
//
// 两条链刻意分开。合并成一条的后果是密文要跟着前端 schema 迁移走，还要塞进读白名单 ——
// 本项目已经因为「读写两侧不同步导致重启即丢」吃过一次亏。
//
// 但分开不等于可以随便写：**写顺序有硬约束**，见 `saveConnection`。

import { defineStore } from 'pinia';
import { LazyStore } from '@tauri-apps/plugin-store';
import { invoke } from '@tauri-apps/api/core';
import { ref, watch } from 'vue';
import type {
  BaiduAuthStatusPayload,
  BaiduLoginClaimPayload,
  RemoteConnection,
  RemoteConnectionForm,
  RemoteConnectionTestReport,
  RemoteProxyConfig,
  RemoteProtocol,
} from '@/types/remote';
import { createRemoteConnectionId, isBaiduPanProtocol } from '@/types/remote';
import { normalizeRemotePath } from '@/utils/remote-paths';
import { setRemoteLocationRefs } from '@/utils/remote-location-refs';
import { useUserPathsStore } from './user-paths';
import {
  canUseStartupStorageFastPath,
  getStartupStorageFile,
  getStartupStorageRecord,
  type StartupStorageFileBootstrap,
} from './utils/startup-storage-bootstrap';

/** storage 文件里的键。与后端 `src-tauri/src/user_storage_files_config.rs` 无关，两边各存各的。 */
const CONNECTIONS_KEY = 'connections';

/**
 * 百度网盘的固定主机与端口。
 *
 * 百度网盘没有「自选服务器」这回事 —— 用户填什么 host 都会被后端忽略，
 * 但连接表里那个字段是必填的（Rust 侧 `RemoteConnection::validate` 要求非空），
 * 所以前端统一写死这两个值，别让用户去填一个填了也没用的输入框。
 */
const BAIDU_PAN_HOST = 'pan.baidu.com';
const BAIDU_PAN_PORT = 443;

export const useRemoteConnectionsStore = defineStore('remoteConnections', () => {
  const userPathsStore = useUserPathsStore();

  const storage = ref<LazyStore | null>(null);
  const connections = ref<RemoteConnection[]>([]);

  /**
   * 把连接列表同步给「位置」页的注册表（用户 2026-11-08 需求 3）。
   *
   * 方向刻意是「store 推、`virtual-locations.ts` 读快照」：那个模块是纯工具，
   * 被工具栏、地址栏与大量单测引入，让它反过来 import 本 store 会把
   * `@tauri-apps/plugin-store` 与 `@/data/user-paths` 拖进每个使用方的模块图，
   * 实测拖坏 `use-file-browser-drag-session.test.ts` 的 8 条用例。
   *
   * `immediate: true` 不能省：连接表可能在本 store 初始化之前就已就绪
   * （启动存储快路径），否则注册表会一直是空的。
   */
  watch(connections, value => setRemoteLocationRefs(value), { immediate: true });
  const isInitialized = ref(false);

  /**
   * 「请打开某条连接的编辑表单」的一次性请求。
   *
   * 需求来源（2026-10-18）：导航栏悬停面板上每条连接右侧加一个设置按钮，
   * 点它直接进那条连接的编辑界面。
   *
   * 为什么是 store 上的瞬态值而不是路由参数：设置页的分类切换已经有自己的
   * 机制（`openSettingsTab`），再加一个 query 参数就要让分类、面板、滚动位置
   * 三处都认识它。而这里要的只是「到了远程分类后，把这条连接的表单打开」，
   * 一个消费即清空的 id 就够 —— **消费方必须清空**，否则用户关掉表单后再切回
   * 远程分类，表单会凭空自己弹出来。
   */
  const pendingEditId = ref<string | null>(null);

  function requestRemoteEdit(id: string): void {
    if (!isValidConnectionId(id)) {
      return;
    }

    pendingEditId.value = id;
  }

  /** 取走一次请求。取走即清空，避免下次进设置页时误开表单。 */
  function consumeRemoteEdit(): string | null {
    const id = pendingEditId.value;
    pendingEditId.value = null;
    return id;
  }

  /**
   * 「请新建一条连接」的一次性请求（2026-11-08）。
   *
   * 需求来源：用户反馈「左侧导航里找不到添加百度网盘的入口」。
   * 左侧导航的云图标本身只展开一个**悬停面板**，对第一次用这个功能的人来说太隐蔽，
   * 所以图标轨上再加一颗常驻的「+」按钮，点它直接跳到设置页的远程分类并打开新建表单。
   *
   * 与 `pendingEditId` 同理是 store 上的瞬态值：**消费即清空**，
   * 否则用户关掉表单后再切回远程分类，表单会凭空自己弹出来。
   */
  const pendingCreate = ref<RemoteProtocol | null>(null);

  function requestRemoteCreate(protocol: RemoteProtocol = 'ftp'): void {
    pendingCreate.value = protocol;
  }

  /** 取走一次新建请求，连带要预置的协议。取走即清空，避免下次进设置页时误开表单。 */
  function consumeRemoteCreate(): RemoteProtocol | null {
    const requested = pendingCreate.value;
    pendingCreate.value = null;
    return requested;
  }

  /**
   * 「请把远程分类切到某个分区」的一次性请求（2026-11-08）。
   *
   * 需求来源：位置页（主页）底部新增了「网盘」「远程连接」两个入口，
   * 点它们要直接落到设置页远程分类的**对应标签**上。
   *
   * 设置页的**顶层**分类由 `openSettingsTab('remote')` 负责，
   * 但那一层只认得到「远程」这个分类，认不到分类内部的「网盘 / 远程连接」两个标签，
   * 所以这里补一个瞬态值。
   *
   * 与 `pendingEditId` / `pendingCreate` 同理：**消费即清空**，
   * 否则用户下次自己进这一页，会被莫名其妙地拽回某个标签。
   */
  const pendingSection = ref<'netdisk' | 'remote' | null>(null);

  function requestRemoteSection(section: 'netdisk' | 'remote'): void {
    pendingSection.value = section;
  }

  /** 取走一次分区请求。取走即清空。 */
  function consumeRemoteSection(): 'netdisk' | 'remote' | null {
    const section = pendingSection.value;
    pendingSection.value = null;
    return section;
  }

  function isValidConnectionId(id: string): boolean {
    return id.length > 0 && !id.includes('/') && !id.includes('\\');
  }

  /**
   * 逐条校验，与后端 `RemoteConnection::validate` 对齐。
   *
   * 前端也要校验，不是为了代替后端，而是为了**在写盘之前**挡住明显错误 ——
   * 后端校验发生在凭据已经写进 DPAPI 之后，那时用户看到的是「保存失败」，
   * 而密码已经留下了。前端这一层是提前给出可读提示。
   */
  function validateForm(form: RemoteConnectionForm, id: string): string | null {
    if (!isValidConnectionId(id)) {
      return 'connectionIdInvalid';
    }

    if (form.name.trim().length === 0) {
      return 'nameRequired';
    }

    // 百度网盘没有「服务器地址」与「端口」的概念 —— 主机恒为 pan.baidu.com，
    // 凭据是一整段 Cookie。拿 FTP 那套规则去卡它，用户根本填不出一个能保存的表单。
    if (!isBaiduPanProtocol(form.protocol)) {
      if (form.host.trim().length === 0) {
        return 'hostRequired';
      }

      if (/\s/.test(form.host)) {
        return 'hostHasWhitespace';
      }

      if (!Number.isInteger(form.port) || form.port < 1 || form.port > 65535) {
        return 'portInvalid';
      }
    }

    if (normalizeRemotePath(form.initialPath) === null) {
      return 'initialPathInvalid';
    }

    // 本地目录**只查「像不像路径」**，不查存在性 ——
    // 校验发生在写盘之前，去 stat 一个还没建好的目录只会误报。
    // 与 Rust 侧 `RemoteConnection::validate` 同规则。
    if (form.localPath.trim().length > 0 && /\s/.test(form.localPath)) {
      return 'localPathHasWhitespace';
    }

    if (form.proxy.kind !== 'direct') {
      if (form.proxy.host.trim().length === 0) {
        return 'proxyHostRequired';
      }

      if (/\s/.test(form.proxy.host)) {
        return 'proxyHostHasWhitespace';
      }

      if (!Number.isInteger(form.proxy.port) || form.proxy.port < 1 || form.proxy.port > 65535) {
        return 'proxyPortInvalid';
      }
    }

    return null;
  }

  function applyRecord(record: Record<string, unknown>): void {
    const stored = record[CONNECTIONS_KEY];

    connections.value = Array.isArray(stored) ? (stored as RemoteConnection[]) : [];
  }

  async function loadStorageData(): Promise<void> {
    if (!storage.value) return;

    try {
      const stored = await storage.value.get<RemoteConnection[]>(CONNECTIONS_KEY);
      applyRecord({ [CONNECTIONS_KEY]: stored });
    }
    catch (error) {
      console.error('Failed to load remote connections storage:', error);
    }
  }

  function loadFromBootstrap(bootstrapFile?: StartupStorageFileBootstrap): boolean {
    if (!canUseStartupStorageFastPath(bootstrapFile)) {
      return false;
    }

    const bootstrapRecord = getStartupStorageRecord(bootstrapFile);

    if (!bootstrapRecord) {
      return true;
    }

    applyRecord(bootstrapRecord);
    return true;
  }

  async function saveStorageData(): Promise<void> {
    // **刻意不静默 no-op。** 仓库里既有的 store 在这里写的是
    // `if (!storage.value) return;`，后果是 init 失败时每次保存都悄悄丢掉：
    // 界面立刻显示新连接、内存里也有，重启即无，全程零报错。
    // 密码的写入顺序依赖这一层「要么写成功、要么明确失败」，
    // 静默返回会让调用方以为已经落盘。
    if (!storage.value) {
      throw new Error('remoteConnectionsStorageNotReady');
    }

    try {
      await storage.value.set(CONNECTIONS_KEY, connections.value);
      await storage.value.save();
    }
    catch (error) {
      console.error('Failed to save remote connections storage:', error);
      throw error;
    }
  }

  async function init(bootstrapFile?: StartupStorageFileBootstrap): Promise<void> {
    if (isInitialized.value) return;

    try {
      const resolvedBootstrapFile = bootstrapFile ?? await getStartupStorageFile('remoteConnections');
      storage.value = await new LazyStore(userPathsStore.customPaths.appUserDataRemoteConnectionsPath);
      await storage.value.save();

      if (!loadFromBootstrap(resolvedBootstrapFile)) {
        await loadStorageData();
      }

      isInitialized.value = true;
    }
    catch (error) {
      console.error('Failed to initialize remote connections storage:', error);
    }
  }

  function getConnection(id: string): RemoteConnection | undefined {
    return connections.value.find(connection => connection.id === id);
  }

  /**
   * 新建或更新一条连接。
   *
   * ## 写顺序是硬约束，不能调换
   *
   * 1. **先**写 DPAPI 密文，**后**写 storage。密文写失败就直接抛错返回，
   *    storage 一行都不动。反过来的话，storage 里会留下一条 `credentialKey`
   *    指向不存在的密文 —— 界面显示「已保存密码」，实际连上去必然认证失败，
   *    而且没有任何报错可查。
   * 2. storage 写失败不回滚密文。那只是一条没有引用的加密数据块，
   *    用户重新保存一次覆盖掉即可；反过来回滚则要在失败路径上再发一次 invoke，
   *    反而制造出「invoke 成功但 store 没更新」这种更难查的状态。
   *
   * ## 密码框空串 = 「不碰已存的密码」
   *
   * 用户改个端口顺手清空密码框是常事，那**不等于**要清密码。
   * 真正要清必须显式传 `clearPassword`。
   */
  async function saveConnection(form: RemoteConnectionForm): Promise<RemoteConnection> {
    const id = form.id ?? createRemoteConnectionId();
    const validationError = validateForm(form, id);

    if (validationError) {
      throw new Error(validationError);
    }

    const existing = getConnection(id);
    const hasNewPassword = form.password.length > 0 && !form.clearPassword;
    const normalizedPath = normalizeRemotePath(form.initialPath) ?? '/';
    const trimmedLocalPath = form.localPath.trim();

    // 第一步：密文先行。
    let credentialKey = existing?.credentialKey;

    if (isBaiduPanProtocol(form.protocol)) {
      // 百度网盘的凭据不是口令，是一整段会话 Cookie（BDUSS + STOKEN）。
      // 走**独立**的 command：后端要解析出这两个值，并拒绝格式不合法的 cookie。
      // 塞进通用的 `remote_save_credential` 等于把没解析过的垃圾也存成「已登录」，
      // 界面上还显示「已保存密码」—— 用户会一直连不上却不知道原因。
      if (hasNewPassword) {
        await invoke('remote_baidu_save_session', {
          connectionId: id,
          cookie: form.password,
        });
        credentialKey = id;
      }
      else if (form.clearPassword) {
        await invoke('remote_baidu_delete_session', {
          connectionId: id,
        });
        credentialKey = undefined;
      }
      else {
        // 内嵌登录（2026-11-08）：登录态先暂存在后端的「待认领槽位」里，
        // 因为它比连接本身先产生 —— 用户是先登录、后点保存。
        // 认领必须放在这里：只有走到这一步才会有 id 可以落盘。
        // 槽位为空时后端返回 claimed:false，是一次空操作，不会覆盖已存会话。
        const claimed = await invoke<BaiduLoginClaimPayload>('remote_baidu_login_claim', {
          connectionId: id,
        });

        if (claimed.claimed) {
          credentialKey = id;
        }
        else if (!credentialKey) {
          // 会话槽位是空的，而且这条连接本来也没有凭据 —— 保存下去会得到一条
          // 「看起来加成功了、实际连不上」的网盘连接，界面上没有任何提示。
          // 2026-11-09 用户实测就是这么丢掉一次扫码登录的。
          // 抛 i18n key，由表单按既有约定渲染成中文提示。
          throw new Error('baiduLoginRequired');
        }
      }
    }
    else if (hasNewPassword) {
      await invoke('remote_save_credential', {
        key: id,
        secret: form.password,
      });
      credentialKey = id;
    }
    else if (form.clearPassword) {
      await invoke('remote_delete_credential', {
        key: id,
      });
      credentialKey = undefined;
    }

    const connection: RemoteConnection = {
      id,
      name: form.name.trim(),
      protocol: form.protocol,
      // 百度网盘不认自定义主机 —— 恒为 pan.baidu.com:443。
      // 写成别的值会让「远程位置」里那条连接连到别处去。
      host: isBaiduPanProtocol(form.protocol)
        ? BAIDU_PAN_HOST
        : form.host.trim(),
      port: isBaiduPanProtocol(form.protocol)
        ? BAIDU_PAN_PORT
        : form.port,
      username: form.username,
      initialPath: normalizedPath,
      // 空串 / 纯空白 → 字段整个不写出去，保持与「不摆双屏」同义。
      ...(trimmedLocalPath
        ? { localPath: trimmedLocalPath }
        : {}),
      proxy: form.proxy,
      ...(credentialKey
        ? { credentialKey }
        : {}),
    };

    // 第二步：storage。走到这里密文已经在盘上。
    if (existing) {
      connections.value = connections.value.map(item => (item.id === id ? connection : item));
    }
    else {
      connections.value = [...connections.value, connection];
    }

    await saveStorageData();

    return connection;
  }

  /**
   * 删掉一条连接，并把它的密文一并清掉。
   *
   * storage 先删、密文后删，且**密文删失败不抛错**：连接已经不存在了，
   * 残留的密文只是一块没人引用的加密数据。反过来的话，凭据没了而连接还在，
   * 用户会看到一个「有密码」的连接但怎么都连不上。
   */
  async function deleteConnection(id: string): Promise<void> {
    const existing = getConnection(id);

    connections.value = connections.value.filter(connection => connection.id !== id);
    await saveStorageData();

    if (!existing?.credentialKey) return;

    try {
      await invoke('remote_delete_credential', {
        key: existing.credentialKey,
      });
    }
    catch (error) {
      console.error('Failed to delete remote credential:', error);
    }
  }

  /** 代理密码也走 DPAPI，与连接密码是两条独立记录。 */
  async function saveProxySecret(
    connectionId: string,
    proxy: RemoteProxyConfig,
  ): Promise<RemoteProxyConfig> {
    if (proxy.kind === 'direct' || !proxy.password) {
      return proxy;
    }

    await invoke('remote_save_credential', {
      key: `${connectionId}:proxy`,
      secret: proxy.password,
    });

    return proxy;
  }

  /**
   * 拿一条已保存连接的密码，供「测试连接」用。
   *
   * **只在内存里过一趟，不写回任何地方。** 测试连接是个纯读操作：
   * 测一次不该改变下次测试的输入，也不该动 DPAPI 存档。
   *
   * 返回 `null` 表示「没存过」—— 调用方据此把 `password` 传成
   * `undefined`（= 不打算提交凭据），而不是传空串。
   */
  async function loadCredentialForTest(key: string | undefined): Promise<string | null> {
    if (!key) {
      return null;
    }

    return invoke<string | null>('remote_load_credential', { key });
  }

  /**
   * 真去连一次，把结论报回来。**不写任何盘上的状态。**
   *
   * `password` 传 `undefined` 表示「这次不提交凭据」，与空串是两件事：
   * 匿名 FTP 会回 `verified` + `passwordChecked: false`，
   * 传空串则会让后端把空口令真的发出去。
   *
   * ⚠️ **`clearPassword` 的优先级与 `saveConnection` 保持一致**：
   * 那里是 `hasNewPassword = password.length > 0 && !clearPassword`，
   * 即「勾了移除，键入的密码也不算数」。这里必须同规则，
   * 否则同一张表单里这个勾选框会有两种含义 —— 用户勾掉密码、点测试，
   * 却拿刚要被删掉的那条去认证并回一句「密码没问题」。
   */
  async function testConnection(
    form: RemoteConnectionForm,
  ): Promise<RemoteConnectionTestReport> {
    const isBaidu = isBaiduPanProtocol(form.protocol);
    let password = form.password.length > 0 && !form.clearPassword ? form.password : undefined;

    // 百度网盘**不走** `loadCredentialForTest`：那边取回来的是存档里的会话
    // JSON（不是原始 cookie），当成 cookie 发过去必然解析失败。
    // 这里留空即可 —— 后端会按 `credentialKey` 自己去读存档里的会话。
    if (password === undefined && form.id && !form.clearPassword && !isBaidu) {
      // 编辑既有连接且密码框留空：空串的语义是「不碰已存的密码」，
      // 所以要把存过的那条取出来用，否则测的其实是「空密码」。
      password = (await loadCredentialForTest(form.id)) ?? undefined;
    }

    // FORK-MODIFICATION 2026-11-09：百度网盘的会话是按**连接 id** 存进保险库的。
    // 未保存的表单没有 id，就没有可读的会话 —— 后端只会回一句「还没有登录凭据」，
    // 而用户刚刚扫码成功过，读起来像「登录白做了」。这里直接说清楚缺的那一步。
    // （手工粘贴了 cookie 的情况不拦：`password` 有值就照常发出去测。）
    if (isBaidu && password === undefined && !form.id) {
      return {
        outcome: 'missing_credential',
        passwordChecked: false,
        tlsVerified: false,
        message: '这条百度网盘连接还没有保存，请先点「保存」，再点「测试连接」。',
      };
    }

    return invoke<RemoteConnectionTestReport>('remote_test_connection', {
      request: {
        protocol: form.protocol,
        // 百度网盘用存档里的会话去测；其余协议不需要这个字段。
        credentialKey: isBaidu ? form.id : undefined,
        host: isBaidu ? BAIDU_PAN_HOST : form.host.trim(),
        port: isBaidu ? BAIDU_PAN_PORT : form.port,
        username: form.username,
        password,
        proxy: form.proxy,
      },
    });
  }

  /**
   * 百度网盘会话的当前状态，给设置页的授权卡用。
   *
   * **只返回状态，不返回凭据** —— 与后端一致：摘要里只有 cookie 的名字与长度，
   * 没有任何明文片段。
   */
  async function getBaiduAuthStatus(
    connectionId: string,
  ): Promise<BaiduAuthStatusPayload> {
    return invoke<BaiduAuthStatusPayload>('remote_baidu_auth_status', {
      connectionId,
    });
  }

  /** 删掉已保存的百度网盘会话（退出登录）。键不存在是成功的空操作。 */
  async function deleteBaiduSession(connectionId: string): Promise<void> {
    await invoke('remote_baidu_delete_session', {
      connectionId,
    });
  }

  /**
   * 打开**内嵌**百度网盘登录窗口（2026-11-08）。
   *
   * 窗口由 Rust 侧建（`src-tauri/src/baidu_login.rs`），前端只发起动作：
   * 这样登录页跑在应用自己的 WebView2 里，cookie 直接落在应用的 cookie 罐，
   * 后端 `cookies_for_url` 读得到 `HttpOnly` 的 `BDUSS` ——
   * 走系统浏览器则完全读不回来（`HttpOnly` 就是防这个的）。
   *
   * 重复调用只会聚焦已经打开的窗口，不会开出第二个。
   */
  async function startBaiduLogin(): Promise<void> {
    await invoke('remote_baidu_login_start');
  }

  /** 放弃内嵌登录：关窗口并丢弃已读到但没人认领的会话。 */
  async function cancelBaiduLogin(): Promise<void> {
    await invoke('remote_baidu_login_cancel');
  }

  return {
    storage,
    connections,
    isInitialized,
    pendingEditId,
    pendingCreate,
    pendingSection,

    init,
    loadStorageData,
    saveStorageData,
    getConnection,
    saveConnection,
    deleteConnection,
    saveProxySecret,
    loadCredentialForTest,
    testConnection,
    getBaiduAuthStatus,
    deleteBaiduSession,
    startBaiduLogin,
    cancelBaiduLogin,
    requestRemoteEdit,
    consumeRemoteEdit,
    requestRemoteCreate,
    consumeRemoteCreate,
    requestRemoteSection,
    consumeRemoteSection,
  };
});
