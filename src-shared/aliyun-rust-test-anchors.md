# 阿里云盘只读后端 · Rust 测试锚点清单（契约 §6.1）

生成时间：2026-11-19 · 后端 worker
基线 `cargo test --lib` 1463 passed / 1 ignored → 本轮 **1529 passed / 0 failed / 1 ignored**（净增 66 条）

## 一、契约 §6.1 的 15 条锚点 → 落地测试

| # | 锚点 | 测试函数（文件） |
|---|------|------------------|
| 1 | 协议名锁死 `aliyun_pan` | `the_aliyun_pan_protocol_has_its_own_stable_name`（remote/connection.rs） |
| 2 | 默认端口 443 | `protocols_carry_their_registered_default_ports`（remote/connection.rs，含 AliyunPan 断言） |
| 3 | registry 不出 password | `an_aliyun_connection_resolves_without_a_password`（remote/registry.rs） |
| 4 | 分页 next_marker | `the_next_marker_decides_whether_another_page_is_needed`（remote/aliyun_api.rs）、`a_missing_next_marker_means_the_listing_is_complete`、`the_listing_follows_the_cursor_until_the_marker_runs_out`（remote/aliyun_api.rs + remote/aliyun_provider.rs 各一） |
| 5 | limit 夹 [1,100] | `the_page_limit_is_clamped_into_the_supported_range`、`the_list_body_carries_the_clamped_limit_and_omits_an_empty_marker`（remote/aliyun_api.rs） |
| 6 | type 判目录/文件 | `the_entry_type_decides_whether_the_entry_is_a_directory`、`a_directory_can_not_be_read_as_a_file`（remote/aliyun_provider.rs） |
| 7 | updated_at ISO8601→Unix 毫秒 | `the_iso8601_timestamp_becomes_unix_millis`、`the_epoch_itself_is_parsed_correctly`（remote/aliyun_api.rs） |
| 8 | 错误体→RemoteErrorKind | `the_error_body_maps_onto_the_right_error_kind`、`the_failure_body_is_parsed_leniently`（remote/aliyun_api.rs） |
| 9 | 错误文案不含 token/code/refresh_token | `the_error_text_never_leaks_the_secrets`、`the_debug_output_never_leaks_the_tokens`、`the_description_never_leaks_the_tokens` |
| 10 | 过期判定与临期窗口 | `a_session_without_an_expiry_is_never_expired`、`a_session_is_expired_once_the_expiry_moment_arrives`、`the_expiry_window_marks_a_session_as_expiring_soon`、`the_status_follows_the_expiry_and_the_warning_window`、`the_remaining_seconds_saturate_at_zero` |
| 11 | PKCE S256 + verifier 长度 | `the_code_verifier_length_is_inside_the_rfc_range`、`the_code_challenge_is_the_s256_base64url_of_the_verifier`、`the_code_challenge_is_stable_and_verifier_dependent`、`the_state_is_thirty_two_hex_characters_and_unpredictable` |
| 12 | 写类方法全 Unsupported | `every_write_operation_reports_that_it_is_unsupported`、`a_write_operation_never_reports_success`、`a_failed_upload_never_triggers_a_progress_callback`（remote/aliyun_provider.rs） |
| 13 | read_dir 路径完整 `sfm://remote/<conn-id>/...` | `the_remote_entry_carries_the_full_remote_path`、`a_nested_entry_keeps_the_parent_folder_in_its_path`（remote/aliyun_api.rs）、`the_listing_entries_carry_the_full_remote_path`（remote/aliyun_provider.rs） |
| 14 | 过期先刷新再重试 | `an_expired_session_is_refreshed_before_the_request_is_retried`、`a_server_side_auth_failure_triggers_one_refresh_and_one_retry`、`a_failed_refresh_asks_the_user_to_authorize_again`、`an_expired_session_without_a_refresh_token_asks_the_user_to_authorize`（remote/aliyun_provider.rs） |
| 15 | 零网络 fake `AliyunApi` | 全部 provider 测试走 `FakeApi`（无任何网络调用），如 `a_file_is_downloaded_through_its_direct_link`、`a_nested_directory_is_resolved_by_walking_down_from_the_root` |

## 二、本轮新增测试函数全清单（66 条）

### `src-tauri/src/remote/aliyun_auth.rs`（17 条）
`a_session_without_an_expiry_is_never_expired`、`a_session_is_expired_once_the_expiry_moment_arrives`、`the_expiry_window_marks_a_session_as_expiring_soon`、`the_status_follows_the_expiry_and_the_warning_window`、`the_remaining_seconds_saturate_at_zero`、`the_code_verifier_length_is_inside_the_rfc_range`、`the_code_challenge_is_the_s256_base64url_of_the_verifier`、`the_code_challenge_is_stable_and_verifier_dependent`、`the_state_is_thirty_two_hex_characters_and_unpredictable`、`the_debug_output_never_leaks_the_tokens`、`the_description_never_leaks_the_tokens`、`encoding_and_decoding_round_trip`、`decoding_garbage_is_classified_as_auth`、`decoding_an_unknown_version_is_rejected_without_panicking`、`a_stored_blob_is_recognised_as_an_aliyun_session`、`the_authorization_header_uses_the_recorded_token_type`、`a_refresh_token_is_only_considered_present_when_it_is_not_blank`

### `src-tauri/src/remote/aliyun_api.rs`（29 条）
`the_next_marker_decides_whether_another_page_is_needed`、`the_list_payload_is_parsed_from_a_cursor_page`、`a_missing_next_marker_means_the_listing_is_complete`、`one_broken_item_does_not_sink_the_whole_page`、`a_body_that_is_not_an_object_is_rejected`、`the_page_limit_is_clamped_into_the_supported_range`、`the_list_body_carries_the_clamped_limit_and_omits_an_empty_marker`、`the_entry_type_decides_whether_the_entry_is_a_directory`、`the_iso8601_timestamp_becomes_unix_millis`、`the_epoch_itself_is_parsed_correctly`、`the_error_body_maps_onto_the_right_error_kind`、`the_failure_body_is_parsed_leniently`、`the_error_text_never_leaks_the_secrets`、`the_token_payload_is_parsed_leniently`、`a_token_response_without_an_access_token_is_an_auth_problem`、`the_authorization_code_form_carries_the_pkce_fields`、`the_refresh_form_uses_the_refresh_grant`、`the_authorize_url_carries_every_required_parameter`、`the_code_is_extracted_from_the_callback_url`、`the_callback_state_must_match`、`the_download_url_is_read_from_either_field_name`、`the_file_metadata_is_read_from_the_get_endpoint`、`the_user_info_is_read_leniently`、`the_remote_entry_carries_the_full_remote_path`、`a_dot_prefixed_entry_is_marked_as_hidden`、`a_nested_entry_keeps_the_parent_folder_in_its_path`、`a_single_page_reports_whether_another_page_follows`、`a_listing_follows_the_cursor_until_the_marker_runs_out`、`a_listing_that_never_ends_is_reported_instead_of_being_truncated`

### `src-tauri/src/remote/aliyun_provider.rs`（18 条）
`every_write_operation_reports_that_it_is_unsupported`、`a_write_operation_never_reports_success`、`a_failed_upload_never_triggers_a_progress_callback`、`the_listing_entries_carry_the_full_remote_path`、`the_listing_follows_the_cursor_until_the_marker_runs_out`、`a_nested_directory_is_resolved_by_walking_down_from_the_root`、`a_missing_connection_is_reported_as_not_found`、`a_path_with_control_characters_is_rejected_before_any_request`、`a_connection_without_a_session_asks_the_user_to_authorize`、`a_connection_without_a_credential_key_asks_the_user_to_authorize`、`an_expired_session_is_refreshed_before_the_request_is_retried`、`a_server_side_auth_failure_triggers_one_refresh_and_one_retry`、`a_failed_refresh_asks_the_user_to_authorize_again`、`an_expired_session_without_a_refresh_token_asks_the_user_to_authorize`、`a_file_is_downloaded_through_its_direct_link`、`a_directory_can_not_be_read_as_a_file`、`a_missing_drive_id_is_fetched_from_the_user_info_endpoint`、`a_drive_id_that_cannot_be_found_asks_the_user_to_authorize`

### `src-tauri/src/remote/connection.rs`（1 条新增 + 1 条扩充）
`the_aliyun_pan_protocol_has_its_own_stable_name`（新增）、`protocols_carry_their_registered_default_ports`（扩充 AliyunPan ⇒ 443）

### `src-tauri/src/remote/registry.rs`（1 条）
`an_aliyun_connection_resolves_without_a_password`

## 三、验收命令（真实输出尾部）

```
cargo test --lib
test result: ok. 1529 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 7.24s
TEST_EXIT=0

cargo clippy --all-targets
warning: `alpha-file-manager` (lib) generated 91 warnings (run `cargo clippy --fix --lib -p alpha-file-manager` to apply 43 suggestions)
warning: `alpha-file-manager` (lib test) generated 113 warnings (79 duplicates) (run `cargo clippy --fix --lib -p alpha-file-manager --tests` to apply 18 suggestions)
CLIPPY_EXIT=0
```

lib 目标 91 warnings = 改动前基线 91（零新增）；阿里云盘相关文件 0 warning。

## 四、必须留到真机验的事项

1. 用户未注册阿里云盘开放平台应用、无真实 `client_id` ⇒ 真实 OAuth 授权流（`/oauth/authorize` → 回调 → `/oauth/access_token`）未验证。
2. 无真实 token ⇒ `openFile/list`、`openFile/get`、`getDownloadUrl`、`/oauth/users/info` 的真实响应形状未验证（代码全部容错，字段名两处兜底：`download_url`/`url`、`file`/`item` 包裹）。
3. `on_navigation` 在 WebView2 上能否拦下回调 URL 未实证（三重保险：导航拦截 + 本地 TCP 监听 + `eval` 轮询）。
4. `state` 校验、`refresh_token` 单次有效（刷新后覆盖旧值）的真实服务端行为未验证。
