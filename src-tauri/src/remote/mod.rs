// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// ---------------------------------------------------------------------------
// 远程空间（FTP / FTPS / SFTP）——阶段 0 VFS 骨架
//
// 本模块被 `cargo test --lib` 的测试二进制链接，因此**不得出现
// `tauri::AppHandle`、`tauri::async_runtime::*` 或 `tauri::` 的任何路径**。
// 一旦碰到，Tauri 全局运行时的默认类型参数 `Wry` 被单态化，把 tao 整条窗口链
// 拖进测试二进制；测试二进制没有 application manifest 声明 comctl32 v6，
// `TaskDialogIndirect` 解析不到，进程在加载阶段以
// `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉 —— 一个测试都跑不了，
// 且报错与本模块毫无关系。详见 `src-tauri/src/dir_reader.rs` 顶部的同类说明。
//
// 阶段 0 **刻意不注册任何 Tauri command**：command 里必然出现 `spawn_blocking`，
// 那正是这条红线的正中间。command 在阶段 1 才接入。
// ---------------------------------------------------------------------------

mod baidu_api;
mod baidu_auth;
mod baidu_provider;
mod commands;
mod connection;
mod connection_test;
mod credentials;
mod dir_adapter;
mod error;
mod fake;
mod ftp_parsing;
mod ftp_provider;
mod ftps_provider;
mod known_hosts;
mod path;
mod provider;
mod quark_api;
mod quark_auth;
mod quark_provider;
mod registry;
mod sftp_provider;
mod transfer;
mod transport;

pub use baidu_api::{
    baidu_error_kind_coverage, baidu_seconds_to_unix_millis, build_client, build_download_client,
    clamp_page_limit, classify_baidu_failure, classify_transport, dlink_url, download_file,
    encode_query_component, entry_name, failure_from_errno, failure_from_http_status, fetch_dlink,
    fetch_user_info, filemetas_url, get_text, list_dir, list_dir_url, next_page,
    parse_dlink_response, parse_filemetas_response, parse_list_response, parse_uinfo_response,
    sanitize_cookie_value, to_remote_entries, to_remote_entry, to_remote_error, transport_shape_of,
    uinfo_url, BaiduDlink, BaiduErrorKindCoverage, BaiduFailure, BaiduFileEntry, BaiduSession,
    BaiduUserInfo, ListPage, TransportErrorShape, BAIDU_CONNECT_TIMEOUT, BAIDU_COOKIE_BDUSS,
    BAIDU_COOKIE_STOKEN, BAIDU_DOWNLOAD_TIMEOUT, BAIDU_LIST_PAGE_LIMIT, BAIDU_OPERATION_TIMEOUT,
    BAIDU_PAN_APP_ID, BAIDU_PAN_BASE_URL, BAIDU_PAN_HOST, BAIDU_PAN_USER_AGENT,
};
pub use baidu_api::{
    create_dir_url, delete_filelist, encode_form_body, fetch_bdstoken, filemanager_url,
    parse_bdstoken, parse_write_response, post_form, rename_filelist, BaiduWriteError,
    BAIDU_BDSTOKEN_TTL, BAIDU_PAN_HOME_URL, BAIDU_PAN_REFERER,
};
pub use baidu_auth::{
    baidu_auth_status, decode_session, delete_baidu_session, describe_state, encode_session,
    is_baidu_session_blob, is_expired, load_baidu_session, parse_cookie_header,
    parse_manual_cookie_input, remaining_seconds, save_baidu_session, scan_cookies, scan_error,
    unix_now, BaiduAuthStatus, BaiduCookieOutcome, BaiduCookieRecord, BaiduCookieScan,
    BaiduSessionState, BaiduStokenState, BAIDU_SESSION_VERSION, BAIDU_STOKEN_EXPIRY_WARN_SECONDS,
};
pub use baidu_provider::{
    BaiduApi, BaiduRemoteProvider, BaiduSessionStore, DpapiBaiduSessionStore, HttpBaiduApi,
    BAIDU_DIRECTORY_TIMEOUT, BAIDU_FILE_TIMEOUT, BAIDU_MAX_DOWNLOAD_BYTES, BAIDU_MAX_LIST_PAGES,
};
pub use connection::{
    connection_error_hint, validate_proxy, NewRemoteConnection, RemoteConnection, RemoteProtocol,
};
// 必须是 glob：`#[tauri::command]` 会额外生成 `__cmd__<name>` 与
// `__tauri_command_name_<name>` 两个隐藏项，`generate_handler!` 要按名字找它们。
// 具名再导出只搬函数本身，于是 `remote::__cmd__remote_save_credential` 找不到，
// 编译报 E0433。`dir_reader` 同样用的是 glob。
pub use commands::*;
// `provider_with_protocol` 是 `pub(crate)`，glob 具名再导出**不会**带上它
// （glob 只搬 `pub` 的项），所以这里单独放一行给 `compare` 模块用。
pub(crate) use commands::provider_with_protocol;
pub use connection_test::{parse_ftp_reply_code, ConnectionTestReport, ProbeOutcome, ProbeTarget};
pub use credentials::{
    credentials_path_in, delete_credential, load_credential, save_credential,
    REMOTE_CREDENTIALS_FILE,
};
pub use dir_adapter::{extension_of, to_dir_contents, to_dir_entry};
pub use error::{HostKeyIdentity, RemoteError, RemoteErrorKind};
pub use fake::FakeRemoteProvider;
pub use ftp_parsing::{
    civil_to_unix_millis, parse_epsv_port, parse_ftp_timestamp, parse_list_line, parse_mlsd_line,
    parse_pasv_endpoint, FtpRecord, MlsdKind,
};
pub use ftp_provider::{
    map_ftp_reply_code, parse_listing, pasv_host_to_use, FtpRemoteProvider, FTP_OPERATION_TIMEOUT,
};
pub use ftps_provider::{FtpsMode, FtpsRemoteProvider, PinnedCertificate, FTPS_OPERATION_TIMEOUT};
pub use known_hosts::{
    decide as decide_host_key, describe_server_key, forget as forget_host_key,
    load as load_known_hosts, trust as trust_host_key, HostKeyDecision, KnownHostEntry, TrustKind,
    REMOTE_KNOWN_HOSTS_FILE,
};
pub use path::{
    build_remote_connection_path, build_remote_path, contains_control_characters,
    get_remote_parent_path, is_remote_connection_root_path, is_remote_path,
    join_remote_parent_and_name, normalize_remote_path, parse_remote_path,
    reject_path_control_characters, split_remote_parent_and_name, RemotePath, REMOTE_SCHEME,
};
pub use provider::{RemoteEntry, RemoteProvider};
// 夸克会话层。裸名（`encode_session` / `decode_session` / `scan_cookies` / `describe_state` /
// `is_expired` / `remaining_seconds` / `scan_error` / `parse_cookie_header`）已被上面的
// `baidu_auth` 段占用，直接 `pub use` 会撞 E0252 ⇒ 统一加 `quark_` 前缀别名导出。
// 夸克 API 纯函数层。与百度侧同名的裸名（`clamp_page_limit` / `next_page` /
// `parse_list_response` / `sanitize_cookie_value` / `to_remote_entry` / `to_remote_entries` /
// `to_remote_error` / `classify_transport` / `encode_query_component` /
// `failure_from_http_status`）一律加 `quark_` 前缀别名导出，避免 E0252。
pub use quark_api::{
    classify_quark_failure, classify_transport as quark_classify_transport,
    clamp_page_limit as quark_clamp_page_limit,
    encode_query_component as quark_encode_query_component, failure_from_code,
    failure_from_http_status as quark_failure_from_http_status, next_page as quark_next_page,
    parse_account_info_response, parse_download_response, parse_list_payload,
    parse_list_response as quark_parse_list_response, quark_account_info_url,
    quark_client_version_from_user_agent, quark_download_is_expired,
    quark_download_request_body, quark_download_url,
    quark_entry_is_directory, quark_entry_name, quark_error_kind_coverage, quark_list_dir_url,
    quark_millis_to_unix_millis, quark_seconds_to_unix_millis, quark_size_to_bytes,
    quark_timestamp_to_unix_millis, sanitize_cookie_jar,
    sanitize_cookie_value as quark_sanitize_cookie_value,
    to_remote_entries as quark_to_remote_entries, to_remote_entry as quark_to_remote_entry,
    to_remote_error as quark_to_remote_error, QuarkDownloadLink, QuarkErrorKindCoverage,
    QuarkFailure, QuarkFileEntry, QuarkListPage, QuarkListPayload, QuarkSession, QuarkSizeUnit,
    QuarkTimeUnit, QuarkTransportShape, QuarkUserInfo, QUARK_CLIENT_ID, QUARK_CLIENT_VERSION,
    QUARK_CONNECT_TIMEOUT, QUARK_DEFAULT_SIZE_UNIT, QUARK_DEFAULT_TIME_UNIT,
    QUARK_DIRECTORY_TIMEOUT, QUARK_DOWNLOAD_REFRESH_WINDOW_SECONDS, QUARK_DRIVE_BASE_URL,
    QUARK_FILE_TIMEOUT, QUARK_FILE_TYPE_DIRECTORY, QUARK_FR, QUARK_LIST_PAGE_SIZE,
    QUARK_LIST_SORT, QUARK_MAX_DOWNLOAD_BYTES, QUARK_OPERATION_TIMEOUT, QUARK_PAN_HOST, QUARK_PR,
    QUARK_SESSION_COOKIE_PUS, QUARK_SYS, QUARK_UC_PARAM_STR, QUARK_USER_AGENT,
};
pub use quark_auth::{
    decode_session as quark_decode_session, delete_quark_session,
    describe_state as quark_describe_state, encode_session as quark_encode_session,
    is_expired as quark_is_expired, is_quark_session_blob, load_quark_session,
    parse_cookie_header as quark_parse_cookie_header, quark_auth_status,
    remaining_seconds as quark_remaining_seconds, sanitize_quark_cookie_value,
    save_quark_session, scan_cookies as quark_scan_cookies, scan_error as quark_scan_error,
    session_to_cookie_header as quark_session_to_cookie_header, trust_deadline_unix,
    QuarkAuthSession, QuarkAuthStatus, QuarkCookieOutcome, QuarkCookieRecord, QuarkCookieScan,
    QuarkSessionState, QUARK_COOKIE_PUS, QUARK_LAST_ALIVE_WARN_SECONDS,
    QUARK_SESSION_TRUST_SECONDS, QUARK_SESSION_VERSION,
};
pub use quark_provider::{
    default_device_model as quark_default_device_model, default_ut as quark_default_ut,
    DpapiQuarkSessionStore, HttpQuarkApi, QuarkApi, QuarkRemoteProvider, QuarkSessionStore,
    UnavailableQuarkApi, QUARK_MAX_LIST_PAGES, QUARK_ROOT_FID,
};
pub use registry::{
    connections_path_in, find_connection, load_connections, resolve_connection, ConnectionsFile,
    ResolvedConnection, REMOTE_CONNECTIONS_FILE,
};
pub use sftp_provider::{
    is_hidden_remote_name, map_sftp_error, map_sftp_status, sftp_seconds_to_unix_millis,
    SftpRemoteProvider, SFTP_OPERATION_TIMEOUT,
};
pub use transfer::{
    chmod, download, format_octal_mode, join_local_child, join_remote_child, mode_is_valid,
    mode_to_permission_triplets, permission_triplets_to_mode, unsupported_set_permissions, upload,
    RemoteChmodReport, RemoteTransferFailure, RemoteTransferReport, PERMISSION_MASK,
};
pub use transport::{
    build_http_connect_request, is_http_connect_success, map_socks5_reply_to_kind, ProxyConfig,
    Socks5CommandKind, TransportStream,
};
