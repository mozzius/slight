use crate::transport;
use gateway_protocol::{
    ClientHello, Frame, GatewayCommand, GatewayEvent, ProtocolError, ServerWelcome,
    PROTOCOL_VERSION,
};
use serde_json::Value;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use thiserror::Error;
use tungstenite::{Message, WebSocket};

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("websocket error: {0}")]
    WebSocket(#[from] tungstenite::Error),
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("server returned {code}: {message}")]
    Server { code: String, message: String },
    #[error("connection closed before a response arrived")]
    Closed,
}

#[derive(Debug, Clone)]
pub struct CallOutcome {
    pub result: Value,
    pub events: Vec<GatewayEvent>,
}

impl CallOutcome {
    pub fn result_as<T: serde::de::DeserializeOwned>(&self) -> Result<T, ProtocolError> {
        serde_json::from_value(self.result.clone())
            .map_err(|error| ProtocolError::MalformedFrame(error.to_string()))
    }
}

pub struct GatewayClient {
    socket: WebSocket<TcpStream>,
    next_request: u64,
    welcome: ServerWelcome,
}

impl GatewayClient {
    pub fn connect(addr: SocketAddr, hello: ClientHello) -> Result<Self, ClientError> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        let (socket, _response) =
            tungstenite::client(format!("ws://{addr}/"), stream).map_err(handshake_error)?;
        let mut client = Self {
            socket,
            next_request: 1,
            welcome: default_welcome(),
        };
        client.send(&Frame::Hello(hello))?;
        loop {
            match client.read()? {
                Frame::Welcome(welcome) => {
                    client.welcome = welcome;
                    return Ok(client);
                }
                Frame::Error(error) => {
                    return Err(ClientError::Server {
                        code: error.code,
                        message: error.message,
                    })
                }
                _ => {}
            }
        }
    }

    pub fn welcome(&self) -> &ServerWelcome {
        &self.welcome
    }

    pub fn next_request_id(&mut self) -> String {
        let id = format!("req-{}", self.next_request);
        self.next_request += 1;
        id
    }

    pub fn call(&mut self, command: GatewayCommand) -> Result<CallOutcome, ClientError> {
        let request_id = command.request_id.clone();
        self.send(&Frame::Command(command))?;
        let mut events = Vec::new();
        loop {
            match self.read()? {
                Frame::Ack(ack) if ack.request_id == request_id => {
                    if ack.ok {
                        return Ok(CallOutcome {
                            result: ack.result.unwrap_or(Value::Null),
                            events,
                        });
                    }
                    let (code, message) = ack
                        .error
                        .map(|error| (error.code, error.message))
                        .unwrap_or_else(|| ("unknown".to_string(), "request failed".to_string()));
                    return Err(ClientError::Server { code, message });
                }
                Frame::Event(event) => events.push(event),
                Frame::Ping(ping) => {
                    self.send(&Frame::Pong(gateway_protocol::PongFrame {
                        protocol_version: PROTOCOL_VERSION,
                        nonce: ping.nonce,
                    }))?;
                }
                Frame::Error(error) => {
                    return Err(ClientError::Server {
                        code: error.code,
                        message: error.message,
                    })
                }
                _ => {}
            }
        }
    }

    fn send(&mut self, frame: &Frame) -> Result<(), ClientError> {
        let message = transport::encode_message(frame)?;
        self.socket.send(message)?;
        Ok(())
    }

    fn read(&mut self) -> Result<Frame, ClientError> {
        loop {
            match self.socket.read() {
                Ok(Message::Text(text)) => return Ok(transport::decode_payload(text.as_bytes())?),
                Ok(Message::Binary(data)) => return Ok(transport::decode_payload(&data)?),
                Ok(Message::Close(_)) => return Err(ClientError::Closed),
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => continue,
                Err(error) if transport::is_closed(&error) => return Err(ClientError::Closed),
                Err(error) => return Err(error.into()),
            }
        }
    }
}

fn default_welcome() -> ServerWelcome {
    ServerWelcome {
        protocol_version: PROTOCOL_VERSION,
        connection_id: String::new(),
        server_name: String::new(),
        server_version: String::new(),
        host_id: None,
        capabilities: Default::default(),
        heartbeat_interval_ms: None,
        resync_required: false,
        resync_reason: None,
    }
}

fn handshake_error(
    error: tungstenite::HandshakeError<tungstenite::ClientHandshake<TcpStream>>,
) -> ClientError {
    match error {
        tungstenite::HandshakeError::Failure(error) => ClientError::WebSocket(error),
        tungstenite::HandshakeError::Interrupted(_) => ClientError::Closed,
    }
}
