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

/** 与后端 `RemoteProtocol` 的五个变体一一对应。 */
export type RemoteProtocol = 'ftp' | 'ftps_explicit' | 'ftps_implicit' | 'sftp' | 'baidu_pan';

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
