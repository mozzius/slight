---
# slight-7gdj
title: investigate host 100% cpu from debug logs
status: completed
type: bug
priority: normal
created_at: 2026-09-15T15:16:32Z
updated_at: 2026-09-15T15:19:09Z
---

Read the host debug logs, identify the source of the 100% CPU loop, and report or fix the root cause.\n\n- [x] Locate current host logs\n- [x] Identify the repeating activity and source code path\n- [x] Verify the diagnosis and summarize findings

## Summary of Changes\n\n- Host logs showed repeated  reconnects every ~0.3 seconds, each failing with  on websocket writes.\n- A live sample of PID 2653 showed connection handlers spinning around tungstenite reads and .\n- Fixed accepted gateway sockets to return to blocking mode after accepting from the nonblocking listener.
running 1 test
test server::tests::unsupported_capability_maps_to_stable_code ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 6 tests
test unknown_command_returns_error ... ok
test agent_discovery_unknown_agent_is_invalid_params ... ok
test pairing_command_round_trips ... ok
test agent_session_import_requires_create_scope ... ok
test agent_sessions_discovery_and_import ... ok
test loopback_session_flow ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s


running 12 tests
test close_handshake_completes ... ok
test malformed_and_binary_frames_are_non_fatal ... ok
test protocol_ping_is_answered_with_pong ... ok
test application_ping_gets_pong ... ok
test command_before_hello_is_rejected ... ok
test urlsession_binary_json_messages_are_accepted ... ok
test websocket_handshake_and_hello_round_trip ... ok
test duplicate_request_id_replays_ack_without_repeating_command ... ok
test attached_session_streams_events ... ok
test bad_credential_and_version_return_error_frames ... ok
test plain_http_request_is_rejected ... ok
test server_sends_heartbeat_pings ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s passes all 19 tests.

## Corrected Notes

- The active host process is still the pre-fix binary, so CPU and EAGAIN logs continue until it is restarted.
- The fix is in rust/gateway-server/src/server.rs: accepted sockets are explicitly made blocking before handler threads start.
