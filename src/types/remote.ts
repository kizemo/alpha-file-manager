// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 远程连接的前端类型。与 `src-tauri/src/remote/connection.rs` 和
// `src-tauri/src/remote/transport.rs` **逐条镜像** —— 序列化名必须逐字对得上：
// 后端写进 `user-remote-connections.json` 的形状，就是这里读出来的形状。
//
// 镜像关系：
// - `RemoteProtocol` ← `RemoteProtocol`（snake_case）
// - `RemoteProxyConfig` ← `ProxyConfig`（`#[serde(tag = "kind", rename_all = "snake_case")]`）
// - `RemoteConnection` ← `RemoteConnection`（camelCase）

/** 与后端 `RemoteProtocol` 的八个变体一一对应。 */
export type RemoteProtocol
  = | 'ftp'
    | 'ftps_explicit'
    | 'ftps_implicit'
    | 'sftp'
    | 'baidu_pan'
    | 'quark_pan'
    | 'aliyun_pan'
    | 'onedrive_pan';

/**
 * 网盘协议清单（百度 / 夸克 / 阿里云盘 / OneDrive）。
 *
 * **「网盘」是一个类，不是一个协议** —— 分区判定、图标、主机写死、保存分派
 * 全都按这张清单走。每接一家网盘只改这一处，别在别处写 `protocol === 'xxx_pan'`。
 */
export const NETDISK_PROTOCOLS = ['baidu_pan', 'quark_pan', 'aliyun_pan', 'onedrive_pan'] as const;

/** 网盘协议。`RemoteProtocol` 里属于网盘的那几个变体。 */
export type NetdiskProtocol = (typeof NETDISK_PROTOCOLS)[number];

/**
 * 代理出口。
 *
 * **这里有密码字段**（`password`），它存的是代理认证信息，走 Rust 的
 * `remote-credentials.json`（DPAPI 加密），**绝不**写进 storage 链。
 * 与连接自身的密码是两条独立的密文记录。
 */
export type RemoteProxyConfig
  = | { kind: 'direct' }
    | {
      kind: 'socks5';
      host: string;
      port: number;
      username?: string;
      password?: string;
    }
    | {
      kind: 'http_connect';
      host: string;
      port: number;
      username?: string;
      password?: string;
    };

/** 一条已保存的远程连接。**不含密码**，只有指向密文的 `credentialKey`。 */
export interface RemoteConnection {
  /** uuid v4 simple。不得含正反斜杠 —— 它要嵌进 `sfm://remote/<id>/...`。 */
  id: string;
  name: string;
  protocol: RemoteProtocol;
  host: string;
  port: number;
  /** 可为空（匿名 FTP / 某些 SFTP）。 */
  username: string;
  /** 打开该连接时进的目录。恒以 `/` 开头。 */
  initialPath: string;
  /**
   * 可选的本地目录。保存了它之后，打开该连接会自动摆成左右双屏：
   * 左格是这个本地目录，右格是远端目录。
   *
   * **可选** —— 绝大多数用户只想在远端浏览。缺失或空串都表示「不摆双屏」，
   * 不要给它编默认值。
   */
  localPath?: string;
  proxy: RemoteProxyConfig;
  /** DPAPI 密文的键。没有密码的连接没有这个字段。 */
  credentialKey?: string;
  /**
   * 这条连接的文件要不要进全局检索索引（批次 37）。
   *
   * 与后端 `RemoteConnection::include_in_global_search`（`#[serde(default)]`）
   * 逐字镜像 —— JSON 键就是 `includeInGlobalSearch`。
   *
   * **缺省是「不加入」**：给云盘建索引要走网络往返、耗时且吃网盘 API 配额，
   * 默认打开等于替用户做决定。老记录里没有这个键时读出来是 `false`。
   */
  includeInGlobalSearch?: boolean;
}

/**
 * 新建 / 编辑连接的表单。
 *
 * `password` 空串的语义是「不碰已存的密码」，不是「清空密码」——
 * 清空要走显式的 `clearPassword`，否则用户改个端口顺手清空了密码框
 * 就会把密码丢掉，而界面上看不出任何区别。
 */
export interface RemoteConnectionForm {
  id?: string;
  name: string;
  protocol: RemoteProtocol;
  host: string;
  port: number;
  username: string;
  initialPath: string;
  /** 可选。空串表示「不摆双屏」。 */
  localPath: string;
  proxy: RemoteProxyConfig;
  password: string;
  clearPassword: boolean;
  /**
   * 要不要把这条连接的文件加进全局检索索引（批次 37）。
   *
   * 与 `RemoteConnection.includeInGlobalSearch` 同一个键、同一套语义：
   * **缺省不勾选**（`undefined` / `false` 都读成「不加入」）。
   */
  includeInGlobalSearch?: boolean;
}

/**
 * 「这条连接要不要摆双屏」的判定。
 *
 * 缺失、空串、纯空白一律当作「不要」—— 用户在表单里清空这一栏就是
 * 想关掉双屏，不是想让它退化成摆一个空目录。
 */
export function hasRemoteLocalPath(connection: Pick<RemoteConnection, 'localPath'>): boolean {
  return typeof connection.localPath === 'string' && connection.localPath.trim().length > 0;
}

/**
 * Rust `RemoteError` 的前端镜像。
 *
 * `fingerprint` / `expectedFingerprint` 只在主机密钥类错误里出现，
 * 其余错误的 JSON 里**没有这两个键**（Rust 侧 `skip_serializing_if`）。
 * 前端要弹「信任这台主机吗」时拿 `fingerprint` 填确认框，
 * 绝不靠解析 `message` 掏指纹。
 */
export interface RemoteErrorPayload {
  kind:
    | 'auth'
    | 'host_key_unknown'
    | 'host_key_changed'
    | 'unreachable'
    | 'permission'
    | 'not_found'
    | 'protocol'
    | 'encoding'
    | 'disconnected'
    | 'unsupported'
    | 'unknown';
  message: string;
  /** 本次连接**实际收到**的主机指纹，`SHA256:...`。 */
  fingerprint?: string;
  /** 已记录、但与本次不一致的指纹。只有 `host_key_changed` 才有。 */
  expectedFingerprint?: string;
  /** 本次收到的主机公钥算法名（如 `Ed25519`）。 */
  keyType?: string;
}

/** 主机密钥类错误的两个 kind。抽出来是为了让调用点只能走「确认流程」。 */
export const HOST_KEY_UNKNOWN_KIND = 'host_key_unknown';
export const HOST_KEY_CHANGED_KIND = 'host_key_changed';

export function isHostKeyError(
  error: unknown,
): error is RemoteErrorPayload & { fingerprint: string } {
  if (typeof error !== 'object' || error === null) {
    return false;
  }

  const kind = (error as { kind?: unknown }).kind;
  const fingerprint = (error as { fingerprint?: unknown }).fingerprint;

  return (kind === HOST_KEY_UNKNOWN_KIND || kind === HOST_KEY_CHANGED_KIND)
    && typeof fingerprint === 'string'
    && fingerprint.length > 0;
}

/** IANA 登记的默认端口，与后端 `RemoteProtocol::default_port` 对齐。 */
export function getRemoteProtocolDefaultPort(protocol: RemoteProtocol): number {
  switch (protocol) {
    case 'ftp':
    case 'ftps_explicit':
      return 21;
    case 'ftps_implicit':
      return 990;
    case 'sftp':
      return 22;
    case 'baidu_pan':
      // 百度网盘走 HTTPS。
      return 443;
    case 'quark_pan':
      // 夸克网盘同样走 HTTPS。
      return 443;
    case 'aliyun_pan':
      // 阿里云盘开放平台只提供 HTTPS 端点。
      return 443;
    case 'onedrive_pan':
      // OneDrive 走 Microsoft Graph，同样只有 HTTPS。
      return 443;
    default:
      return 21;
  }
}

/**
 * 是不是百度网盘协议。
 *
 * 百度网盘的连接**没有**服务器地址与端口可言 —— 主机恒为 `pan.baidu.com`，
 * 凭据也不是口令而是一整段 Cookie。
 * 表单校验、保存分派、界面分支都靠它分流，所以抽成一个函数而不是到处写
 * `protocol === 'baidu_pan'`。
 */
export function isBaiduPanProtocol(protocol: RemoteProtocol): boolean {
  return protocol === 'baidu_pan';
}

/**
 * 是不是夸克网盘协议。
 *
 * 与百度网盘同构：没有「自选服务器」可言 —— 主机恒为 `pan.quark.cn`，
 * 凭据也不是口令而是一整段 Cookie（核心是 `__pus`）。
 */
export function isQuarkPanProtocol(protocol: RemoteProtocol): boolean {
  return protocol === 'quark_pan';
}

/**
 * 是不是阿里云盘协议。
 *
 * 与百度 / 夸克同构：没有「自选服务器」可言 —— 主机恒为 `openapi.alipan.com`；
 * 凭据也不是口令，而是一整套 OAuth 会话（access_token / refresh_token），
 * 由后端加密落盘，前端只拿得到状态与摘要。
 */
export function isAliyunPanProtocol(protocol: RemoteProtocol): boolean {
  return protocol === 'aliyun_pan';
}

/**
 * 是不是 OneDrive 协议（批次 38，只读 A 级）。
 *
 * 与前三家同构：没有「自选服务器」可言 —— 主机恒为 `graph.microsoft.com`；
 * 凭据是一整套 Microsoft OAuth 会话，由后端加密落盘，
 * `client_id` 也由用户自填、只存进会话 blob，前端不写进连接表。
 */
export function isOnedrivePanProtocol(protocol: RemoteProtocol): boolean {
  return protocol === 'onedrive_pan';
}

/**
 * 是不是**网盘**协议（百度 / 夸克 / 阿里云盘都算）。
 *
 * 分区判定（「网盘」标签 vs「远程连接」标签）必须用这个口径，
 * 而不是 `isBaiduPanProtocol` —— 后者只认百度，
 * 其余网盘连接会被误分到「远程连接」标签下。
 *
 * 同时是**类型守卫**：调用点据此把 `RemoteProtocol` 收窄成 `NetdiskProtocol`，
 * 省掉「三元式选 provider」那种写法（接第三家网盘时最容易漏掉的那类分支）。
 */
export function isNetdiskProtocol(protocol: RemoteProtocol): protocol is NetdiskProtocol {
  return (NETDISK_PROTOCOLS as readonly RemoteProtocol[]).includes(protocol);
}

/**
 * 网盘 provider 的界面名（i18n 键）。
 *
 * 三个键**都已存在**（`settings.remote.netdisk.baidu` / `.quark` / `.aliyun`），
 * 所以预填名字不需要任何新语言包 —— 而且预填出来的名字必须与按钮上的那个
 * 一模一样，用户不该在同一个界面里看到两种叫法。
 *
 * **导出**是因为「设置 → 搜索」的云盘检索按网盘类型分组，组标题要走同一张表
 * —— 网盘类型清单是动态派生的（接第四家网盘时不用再改界面）。
 */
export const NETDISK_PROVIDER_NAME_KEYS: Record<NetdiskProtocol, string> = {
  baidu_pan: 'settings.remote.netdisk.baidu',
  quark_pan: 'settings.remote.netdisk.quark',
  aliyun_pan: 'settings.remote.netdisk.aliyun',
  onedrive_pan: 'settings.remote.netdisk.onedrive',
};

/**
 * 「新建连接」时名称栏的默认值（批次 37）。
 *
 * 三种情况，顺序不能换：
 *
 * 1. **编辑既有连接**（`existingName` 是字符串）⇒ 原样返回。
 *    预填**绝不覆盖**用户存下来的名字 —— 那是用户自己起的。
 * 2. **新建网盘**（`existingName` 是 `undefined`）⇒ 返回 provider 的显示名
 *    （百度网盘 / 夸克网盘 / 阿里云盘）。
 * 3. **新建 FTP / SFTP** ⇒ 空串。它们不是网盘，
 *    给它们塞一个中文网盘名是错的。
 *
 * `translate` 由调用方传进来（`useI18n()` 的 `t`），本函数保持纯函数、可单测。
 */
export function resolveRemoteConnectionNameDefault(
  protocol: RemoteProtocol,
  existingName: string | undefined | null,
  translate: (key: string) => string,
): string {
  if (typeof existingName === 'string') {
    return existingName;
  }

  if (!isNetdiskProtocol(protocol)) {
    return '';
  }

  return translate(NETDISK_PROVIDER_NAME_KEYS[protocol]);
}

/**
 * 勾选了「加入全局检索」的那些连接的 id（批次 37）。
 *
 * 建云盘索引时传的就是这个集合 —— **必须是当前全部已勾选的连接**：
 * 后端按传进来的集合整体重建云盘索引，漏掉一条就等于把那条的条目从索引里抹掉。
 */
export function getGlobalSearchConnectionIds(
  connections: Pick<RemoteConnection, 'id' | 'includeInGlobalSearch'>[],
): string[] {
  return connections
    .filter(connection => connection.includeInGlobalSearch === true)
    .map(connection => connection.id);
}

/**
 * 百度网盘会话的授权状态。与后端 `BaiduAuthStatusPayload` 逐条镜像。
 *
 * `status` 是**分类**，不是文案 —— 前端按它分支，文案另走 i18n。
 * `summary` 是后端 `describe_state` 出来的脱敏一句话，可直接展示，
 * 但**不能**拿它做逻辑判断（会随版本改措辞）。
 */
export type BaiduAuthStatusKind
  = | 'missing'
    | 'ready'
    | 'stoken_expired'
    | 'stoken_expiring_soon';

export interface BaiduAuthStatusPayload {
  status: BaiduAuthStatusKind;
  /** STOKEN 还剩多少秒。只在「即将过期」时才有。 */
  remainingSeconds?: number;
  /** 脱敏状态说明。 */
  summary: string;
}

/**
 * 「测试连接」的结论。与后端 `ProbeOutcome` 逐条镜像。
 *
 * **前端只能按这个分支渲染，绝不去解析 `message`。**
 * `message` 是给用户看的一句话，会随版本改措辞；拿它做判断等于把文案当协议。
 */
export type RemoteProbeOutcome
  /** 协议握手与凭据都过了。 */
  = | 'verified'
  /** 连上了、协议也对，但服务器拒了这组凭据 —— 去改密码。 */
    | 'auth_rejected'
  /** 服务器要凭据，但这次没提供。匿名 FTP 不算。 */
    | 'missing_credential'
  /** 连上了，但这不是该协议的服务。 */
    | 'wrong_protocol'
  /** DNS 失败、连接被拒、超时、代理失败。 */
    | 'unreachable'
  /** 认得协议，但后续交互出不来预期结果。 */
    | 'protocol_error';

/** 一次测试的结果。与后端 `ConnectionTestReport` 逐条镜像（camelCase）。 */
export interface RemoteConnectionTestReport {
  outcome: RemoteProbeOutcome;
  /** 服务端自报身份：SFTP 是主机公钥，FTP 是 220 那行。 */
  serverIdentity?: string;
  /**
   * 本次**是否真的提交并核验了密码**。
   *
   * 少了它，「通过」会被读成「密码没问题」—— 而免密/匿名连接同样会通过。
   */
  passwordChecked: boolean;
  /** TLS 证书**没有**校验，恒为 `false`。别当成已校验。 */
  tlsVerified: boolean;
  /** 给用户看的一句话。不要拿它做逻辑判断。 */
  message: string;
}

/** 测试请求。与后端 `RemoteTestConnectionRequest` 镜像。 */
export interface RemoteTestConnectionRequest {
  protocol: RemoteProtocol;
  host: string;
  port: number;
  username: string;
  /** `undefined` = 这次不打算提交凭据，不是「空密码」。 */
  password?: string;
  /**
   * 已保存会话的键（= 连接 id）。
   *
   * **只有百度网盘用得上**：它的凭据是一整段 Cookie，空密码框的语义是
   * 「用已存的那份会话去测」，不是「没有凭据」。其余协议忽略这个字段。
   */
  credentialKey?: string;
  /** 缺省即直连。 */
  proxy?: RemoteProxyConfig;
}

/**
 * 生成一个新的连接 id / 凭据键。
 *
 * **凭据键就等于连接 id** —— 一条连接一份凭据，不另起一套编号。
 * 用 uuid v4 simple 的 32 位十六进制：不含正反斜杠，长度固定，没有 `/` 就能安全嵌进
 * `sfm://remote/<id>/...`。
 */
export function createRemoteConnectionId(): string {
  const globalCrypto = globalThis.crypto;

  if (globalCrypto?.randomUUID) {
    return globalCrypto.randomUUID().replace(/-/g, '');
  }

  // 极端降级路径。正式运行在 Tauri 的 WebView2 里，randomUUID 一定存在；
  // 这里只是为了让单测与老环境不至于直接抛错。
  const bytes = new Uint8Array(16);

  if (globalCrypto?.getRandomValues) {
    globalCrypto.getRandomValues(bytes);
  }

  return Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
}

/**
 * 内嵌登录的「认领」结果（2026-11-08）。与后端
 * `src-tauri/src/baidu_login.rs::BaiduLoginClaimPayload` 镜像。
 *
 * `claimed === false` **不是错误**：用户没走内嵌登录就直接点保存时，
 * 后端没有待认领的会话，认领是一次空操作。
 */
export interface BaiduLoginClaimPayload {
  claimed: boolean;
  /** 只有 cookie 的名字与长度，不含任何明文片段。 */
  summary: string;
}

/**
 * 内嵌登录的进度事件（2026-11-08）。与后端
 * `src-tauri/src/baidu_login.rs::BaiduLoginEvent` 镜像，由
 * `afm://baidu-login` 频道推送。
 *
 * - `ready`：已经读回可用的登录态，**此时还没落盘** —— 落盘发生在
 *   「保存」时（那时才有连接 id），所以界面文案要说清「填名字再保存」。
 * - `closed`：用户自己关掉了登录窗且当时还没读到登录态。
 * - `timeout` / `error`：放弃等待。
 */
export interface BaiduLoginEventPayload {
  status: 'ready' | 'closed' | 'timeout' | 'error';
  /** STOKEN 是否也拿到了（只影响上传加速，不是登录成功的前提）。 */
  hasStoken: boolean;
  /** 只有 cookie 的名字与长度，不含任何明文片段。 */
  summary: string | null;
  /** 面向用户的短说明，可直接展示。 */
  message: string | null;
}

/**
 * 夸克网盘会话的授权状态。与后端 `QuarkAuthStatusPayload` 逐条镜像。
 *
 * `status` 是**分类**，不是文案 —— 前端按它分支，文案另走 i18n。
 * `summary` 是后端 `describe_state` 出来的脱敏一句话，可直接展示，
 * 但**不能**拿它做逻辑判断（会随版本改措辞）。
 */
export type QuarkAuthStatusKind = 'missing' | 'ready' | 'stale' | 'stale_soon';

export interface QuarkAuthStatusPayload {
  status: QuarkAuthStatusKind;
  /** 信任窗口还剩多少秒。`undefined` = 不知道上次验活时间。 */
  remainingSeconds?: number;
  /** 脱敏状态说明。 */
  summary: string;
}

/** 夸克登录窗是否已打开。与后端 `QuarkLoginStartPayload` 镜像。 */
export interface QuarkLoginStartPayload {
  opened: boolean;
}

/** 有没有待认领的夸克登录态。与后端 `QuarkLoginPendingPayload` 镜像。 */
export interface QuarkLoginPendingPayload {
  pending: boolean;
  /** 脱敏描述（只有 cookie 的名字与长度，没有内容）。 */
  summary: string;
}

/**
 * 夸克内嵌登录的「认领」结果。与后端 `QuarkLoginClaimPayload` 镜像。
 *
 * `claimed === false` **不是错误**：用户没走内嵌登录就直接点保存时，
 * 后端没有待认领的会话，认领是一次空操作。
 */
export interface QuarkLoginClaimPayload {
  claimed: boolean;
  /** 脱敏描述。 */
  summary: string;
}

/**
 * 夸克内嵌登录的进度事件。与后端
 * `src-tauri/src/quark_login.rs::QuarkLoginEvent` 镜像，由
 * `afm://quark-login` 频道推送。
 *
 * - `ready`：已经读回可用的登录态（`__pus` 已到手），**此时还没落盘** ——
 *   落盘发生在「保存」时（那时才有连接 id），所以界面文案要说清「填名字再保存」。
 * - `closed`：用户自己关掉了登录窗且当时还没读到登录态。
 * - `timeout` / `error`：放弃等待。
 */
export interface QuarkLoginEventPayload {
  status: 'ready' | 'closed' | 'timeout' | 'error';
  /** 只有 cookie 的名字与长度，不含任何明文片段。 */
  summary: string | null;
  /** 面向用户的短说明，可直接展示。 */
  message: string | null;
}

/**
 * 阿里云盘会话的授权状态。与后端 `AliyunAuthStatusPayload` 逐条镜像。
 *
 * `status` 是**分类**，不是文案 —— 前端按它分支，文案另走 i18n。
 * `summary` 是后端 `describe_state` 出来的脱敏一句话，可直接展示，
 * 但**不能**拿它做逻辑判断（会随版本改措辞）。
 *
 * ⚠️ **没有 `loading`**：后端只认「还没存 / 有效 / 过期 / 即将过期」四种。
 * 「正在读取」是**前端本地状态**（等后端回答时自己显示），
 * 别把它当成后端的返回值去等。
 */
export type AliyunAuthStatusKind
  = | 'missing'
    | 'ready'
    | 'stale'
    | 'stale_soon';

export interface AliyunAuthStatusPayload {
  status: AliyunAuthStatusKind;
  /** 访问凭证还剩多少秒。`undefined` = 不知道。 */
  remainingSeconds?: number;
  /** 脱敏状态说明。 */
  summary: string;
  /** 开放平台应用 ID。**不是密钥** —— 可以展示。 */
  clientId?: string;
  /** 有没有配应用密钥。`false` = PKCE 公开客户端模式。 */
  hasClientSecret?: boolean;
  /** 已授权的 scope。 */
  scope?: string;
  /** 访问凭证的到期时刻（Unix 秒）。 */
  expiresAt?: number;
  /** 空间 ID。 */
  driveId?: string;
  /** 用户昵称。 */
  nickName?: string;
}

/**
 * 阿里云盘扫码登录的状态。与后端 `AliyunQrPollPayload` 的 `status` 镜像。
 *
 * `new` / `scanned` 是中间态（前端继续轮询），其余四个是终态。
 */
export type AliyunQrStatusKind
  = | 'new'
    | 'scanned'
    | 'confirmed'
    | 'expired'
    | 'canceled'
    | 'error';

/** 二维码票据。与后端 `AliyunQrStartPayload` 镜像。 */
export interface AliyunQrStartPayload {
  /** 二维码内容（前端用 `qrcode` 库渲染成图片）。 */
  codeContent: string;
  /** 轮询用的会话标识。 */
  ck: string;
  /** 轮询用的时间戳。 */
  t?: number | null;
}

/** 扫码轮询的结果。与后端 `AliyunQrPollPayload` 镜像。 */
export interface AliyunQrPollPayload {
  status: AliyunQrStatusKind;
  /** 面向用户的短说明（**不含任何凭据**）。 */
  message?: string | null;
  /** 是不是终态（前端据此停止轮询）。 */
  finalStatus: boolean;
}

/** 有没有待认领的阿里云盘登录态。与后端 `AliyunLoginPendingPayload` 镜像。 */
export interface AliyunLoginPendingPayload {
  pending: boolean;
  /** 脱敏描述（不含任何 token 片段）。 */
  summary: string;
}

/**
 * 阿里云盘内嵌登录的「认领」结果。与后端 `AliyunLoginClaimPayload` 镜像。
 *
 * `claimed === false` **不是错误**：用户没走内嵌登录就直接点保存时，
 * 后端没有待认领的会话，认领是一次空操作。
 */
export interface AliyunLoginClaimPayload {
  claimed: boolean;
  /** 脱敏描述。 */
  summary: string;
}

/**
 * 内嵌真实登录页的启动回执（批次 29，与后端 start 命令返回值镜像）。
 * reused = 登录窗已经开着，后端只做了聚焦。
 */
export interface AliyunLoginStartPayload {
  started: boolean;
  reused: boolean;
}

/**
 * OneDrive 登录授权的启动回执（批次 38）。
 *
 * `authUrl` 由前端交给系统浏览器打开（Microsoft 的授权页有自己的登录态与多因素认证，
 * 不适合塞进应用内嵌窗口）；`redirectUri` 是后端起的**环回地址**，
 * 用户授权后 Microsoft 会把浏览器打回这里，授权码由后端接走。
 */
export interface OnedriveLoginStartPayload {
  authUrl: string;
  redirectUri: string;
}

/**
 * OneDrive 登录的进度（批次 38）。
 *
 * `state` 是**阶段**而不是文案：`pending` 时前端继续轮询，其余三个都不用再轮。
 * `error` 只在 `failed` 时有值，且是后端写的**可展示**文案（不含任何凭据）。
 */
export interface OnedriveLoginPendingPayload {
  state: 'idle' | 'pending' | 'completed' | 'failed';
  error?: string | null;
  authorized: boolean;
}

/**
 * OneDrive 授权状态（批次 38）。
 *
 * ⚠️ 与阿里云的 `AliyunAuthStatusPayload` **不是同一个形状** ——
 * OneDrive 侧只有「有没有授权 / 什么时候过期 / 哪个账号」三件事，
 * 后端没有 `status` 分类字段，别照阿里云那套去读。
 */
export interface OnedriveAuthStatusPayload {
  authorized: boolean;
  /** 访问凭证到期时刻（Unix 秒）。`null` / 缺省 = 不知道。 */
  expiresAt?: number | null;
  /** 已授权账号（脱敏显示名）。 */
  account?: string | null;
}
