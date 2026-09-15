mod support;

use gateway_protocol::{
    command_name, decode_text, encode_text, CreateSessionParams, Frame, GatewayCommand, PingFrame,
    SessionCreateResult, SessionListResult, PROTOCOL_VERSION,
};
use gateway_server::GatewayClient;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};
use tungstenite::{Message, WebSocket};

/// A raw WebSocket client that speaks gateway frames directly. This exercises
/// the transport the way `URLSessionWebSocketTask` does, without going through
/// `GatewayClient`.
struct RawClient {
    socket: WebSocket<TcpStream>,
}

impl RawClient {
    fn connect(addr: SocketAddr) -> Self {
        let stream = TcpStream::connect(addr).expect("tcp connect");
        stream.set_nodelay(true).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let (socket, _response) =
            tungstenite::client(format!("ws://{addr}/"), stream).expect("websocket handshake");
        Self { socket }
    }

    fn send_frame(&mut self, frame: &Frame) {
        let text = encode_text(frame).expect("encode frame");
        self.socket.send(Message::text(text)).expect("send frame");
    }

    fn send_raw_text(&mut self, text: &str) {
        self.socket
            .send(Message::text(text.to_string()))
            .expect("send text");
    }

    fn send_raw_binary(&mut self, data: &[u8]) {
        self.socket
            .send(Message::binary(data.to_vec()))
            .expect("send binary");
    }

    fn send_binary_frame(&mut self, frame: &Frame) {
        let text = encode_text(frame).expect("encode frame");
        self.socket
            .send(Message::binary(text.into_bytes()))
            .expect("send binary frame");
    }

    fn read_frame(&mut self) -> Frame {
        loop {
            match self.socket.read().expect("read message") {
                Message::Text(text) => return decode_text(text.as_str()).expect("decode frame"),
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
                Message::Binary(data) => panic!("unexpected binary message: {data:?}"),
                Message::Close(frame) => panic!("unexpected close: {frame:?}"),
            }
        }
    }

    fn authenticate(&mut self) {
        self.send_frame(&Frame::Hello(test_support::sample_hello("test")));
        match self.read_frame() {
            Frame::Welcome(welcome) => assert_eq!(welcome.server_name, "slight-host"),
            other => panic!("expected welcome, got {other:?}"),
        }
    }
}

#[test]
fn websocket_handshake_and_hello_round_trip() {
    let server = support::spawn_server();
    let mut raw = RawClient::connect(server.addr);
    raw.send_frame(&Frame::Hello(test_support::sample_hello("test")));
    match raw.read_frame() {
        Frame::Welcome(welcome) => {
            assert_eq!(welcome.protocol_version, PROTOCOL_VERSION);
            assert!(!welcome.connection_id.is_empty());
        }
        other => panic!("expected welcome, got {other:?}"),
    }

    raw.send_frame(&Frame::Command(GatewayCommand::new(
        "req-1",
        command_name::SESSION_LIST,
    )));
    match raw.read_frame() {
        Frame::Ack(ack) => {
            assert!(ack.ok);
            assert_eq!(ack.request_id, "req-1");
        }
        other => panic!("expected ack, got {other:?}"),
    }
}

#[test]
fn duplicate_request_id_replays_ack_without_repeating_command() {
    let server = support::spawn_server();
    let mut raw = RawClient::connect(server.addr);
    raw.authenticate();

    let command = GatewayCommand::new("req-create", command_name::SESSION_CREATE).with_params(
        &CreateSessionParams {
            agent: "fake".to_string(),
            model: Some("auto".to_string()),
            effort: Some("medium".to_string()),
            working_directory_label: "~/work".to_string(),
            initial_prompt: None,
        },
    );
    raw.send_frame(&Frame::Command(command.clone()));
    let first = match raw.read_frame() {
        Frame::Ack(ack) => ack,
        other => panic!("expected create ack, got {other:?}"),
    };
    let first_result: SessionCreateResult =
        serde_json::from_value(first.result.expect("create result")).unwrap();

    raw.send_frame(&Frame::Command(command));
    let second = match raw.read_frame() {
        Frame::Ack(ack) => ack,
        other => panic!("expected replayed ack, got {other:?}"),
    };
    let second_result: SessionCreateResult =
        serde_json::from_value(second.result.expect("replayed result")).unwrap();
    assert_eq!(first_result.session.id, second_result.session.id);

    raw.send_frame(&Frame::Command(GatewayCommand::new(
        "req-list",
        command_name::SESSION_LIST,
    )));
    let list = match raw.read_frame() {
        Frame::Ack(ack) => {
            serde_json::from_value::<SessionListResult>(ack.result.expect("list result")).unwrap()
        }
        other => panic!("expected list ack, got {other:?}"),
    };
    assert_eq!(list.sessions.len(), 1);
}

#[test]
fn plain_http_request_is_rejected() {
    let server = support::spawn_server();
    let mut stream = TcpStream::connect(server.addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let mut buffer = [0u8; 256];
    let read = stream.read(&mut buffer).unwrap_or(0);
    assert_eq!(read, 0, "server should close non-websocket clients");

    // The listener survives a rejected upgrade.
    let client = GatewayClient::connect(server.addr, test_support::sample_hello("test"))
        .expect("still serving websocket clients");
    assert_eq!(client.welcome().server_name, "slight-host");
}

#[test]
fn malformed_and_binary_frames_are_non_fatal() {
    let server = support::spawn_server();
    let mut raw = RawClient::connect(server.addr);
    raw.authenticate();

    raw.send_raw_text("not-json");
    match raw.read_frame() {
        Frame::Error(error) => assert_eq!(error.code, "malformed_frame"),
        other => panic!("expected malformed_frame error, got {other:?}"),
    }

    raw.send_raw_text("{\"type\":\"not_a_real_frame\"}");
    match raw.read_frame() {
        Frame::Error(error) => assert_eq!(error.code, "malformed_frame"),
        other => panic!("expected malformed_frame error, got {other:?}"),
    }

    // Invalid UTF-8 in a binary message is rejected but does not poison the
    // connection.
    raw.send_raw_binary(&[0xff, 0xfe, 0xfd]);
    match raw.read_frame() {
        Frame::Error(error) => assert_eq!(error.code, "malformed_frame"),
        other => panic!("expected malformed_frame error, got {other:?}"),
    }

    // Framing errors do not poison the connection.
    raw.send_frame(&Frame::Command(GatewayCommand::new(
        "req-after",
        command_name::SESSION_LIST,
    )));
    match raw.read_frame() {
        Frame::Ack(ack) => assert!(ack.ok),
        other => panic!("expected ack, got {other:?}"),
    }
}

#[test]
fn urlsession_binary_json_messages_are_accepted() {
    let server = support::spawn_server();
    let mut raw = RawClient::connect(server.addr);
    raw.authenticate();

    // URLSessionWebSocketTask sends JSON as `.data`, i.e. a binary frame.
    raw.send_binary_frame(&Frame::Command(GatewayCommand::new(
        "req-binary",
        command_name::SESSION_LIST,
    )));
    match raw.read_frame() {
        Frame::Ack(ack) => {
            assert!(ack.ok);
            assert_eq!(ack.request_id, "req-binary");
        }
        other => panic!("expected ack, got {other:?}"),
    }
}

#[test]
fn bad_credential_and_version_return_error_frames() {
    let server = support::spawn_server();

    let mut bad_credential = RawClient::connect(server.addr);
    bad_credential.send_frame(&Frame::Hello(test_support::sample_hello("nope")));
    match bad_credential.read_frame() {
        Frame::Error(error) => assert_eq!(error.code, "unauthenticated"),
        other => panic!("expected unauthenticated error, got {other:?}"),
    }

    let mut bad_version = RawClient::connect(server.addr);
    let mut hello = test_support::sample_hello("test");
    hello.protocol_version = PROTOCOL_VERSION + 1;
    bad_version.send_frame(&Frame::Hello(hello));
    match bad_version.read_frame() {
        Frame::Error(error) => assert_eq!(error.code, "invalid_version"),
        other => panic!("expected invalid_version error, got {other:?}"),
    }
}

#[test]
fn command_before_hello_is_rejected() {
    let server = support::spawn_server();
    let mut raw = RawClient::connect(server.addr);
    raw.send_frame(&Frame::Command(GatewayCommand::new(
        "req-early",
        command_name::SESSION_LIST,
    )));
    match raw.read_frame() {
        Frame::Ack(ack) => {
            assert!(!ack.ok);
            let error = ack.error.expect("error body");
            assert_eq!(error.code, "unauthenticated");
        }
        other => panic!("expected error ack, got {other:?}"),
    }
}

#[test]
fn close_handshake_completes() {
    let server = support::spawn_server();
    let mut raw = RawClient::connect(server.addr);
    raw.authenticate();

    raw.socket.close(None).expect("send close");
    let mut acknowledged = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match raw.socket.read() {
            Ok(Message::Close(_)) => {
                acknowledged = true;
                break;
            }
            Ok(_) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                acknowledged = true;
                break;
            }
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(error) => panic!("unexpected error during close: {error:?}"),
        }
    }
    assert!(acknowledged, "server should complete the closing handshake");
}

#[test]
fn protocol_ping_is_answered_with_pong() {
    let server = support::spawn_server();
    let mut raw = RawClient::connect(server.addr);
    raw.socket
        .send(Message::Ping(tungstenite::Bytes::from_static(b"probe")))
        .expect("send protocol ping");
    loop {
        match raw.socket.read().expect("read pong") {
            Message::Pong(payload) => {
                assert_eq!(&payload[..], b"probe");
                break;
            }
            Message::Ping(_) => continue,
            other => panic!("unexpected message: {other:?}"),
        }
    }
}

#[test]
fn application_ping_gets_pong() {
    let server = support::spawn_server();
    let mut raw = RawClient::connect(server.addr);
    raw.send_frame(&Frame::Ping(PingFrame {
        protocol_version: PROTOCOL_VERSION,
        nonce: "nonce-1".to_string(),
    }));
    match raw.read_frame() {
        Frame::Pong(pong) => assert_eq!(pong.nonce, "nonce-1"),
        other => panic!("expected pong, got {other:?}"),
    }
}

#[test]
fn server_sends_heartbeat_pings() {
    let config = gateway_server::GatewayConfig {
        heartbeat_interval_ms: Some(50),
        ..Default::default()
    };
    let server = support::spawn_server_with(config);
    let mut raw = RawClient::connect(server.addr);
    match raw.read_frame() {
        Frame::Ping(ping) => assert!(!ping.nonce.is_empty()),
        other => panic!("expected heartbeat ping, got {other:?}"),
    }
}

#[test]
fn attached_session_streams_events() {
    let server = support::spawn_server();
    let mut raw = RawClient::connect(server.addr);
    raw.authenticate();

    let create = test_support::create_session_command("req-create", "fake");
    raw.send_frame(&Frame::Command(create));
    let session_id = match raw.read_frame() {
        Frame::Ack(ack) => {
            let result: SessionCreateResult = ack.result_as().expect("create result");
            result.session.id
        }
        other => panic!("expected create ack, got {other:?}"),
    };

    let attach = test_support::attach_command("req-attach", &session_id, None);
    raw.send_frame(&Frame::Command(attach));
    match raw.read_frame() {
        Frame::Ack(ack) => assert!(ack.ok),
        other => panic!("expected attach ack, got {other:?}"),
    }

    let input = GatewayCommand::new("req-input", command_name::SESSION_INPUT)
        .for_session(session_id.clone())
        .with_params(&gateway_protocol::SendInputParams {
            text: "next".to_string(),
        });
    raw.send_frame(&Frame::Command(input));

    let mut saw_ack = false;
    let mut saw_event = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !(saw_ack && saw_event) {
        match raw.read_frame() {
            Frame::Ack(ack) => {
                assert!(ack.ok);
                saw_ack = true;
            }
            Frame::Event(event) => {
                assert_eq!(event.session_id, session_id);
                saw_event = true;
            }
            other => panic!("unexpected frame: {other:?}"),
        }
    }
    assert!(saw_ack && saw_event, "expected ack and streamed event");
}
