//! abstraction for working with Websocket connections
//! based on any rust io Read/Write Stream

mod tcp;

use crate::transport::{
    error::{TransportError, TransportResult},
    protocol::{TransportCommand, TransportEvent},
    transport_trait::Transport,
    TransportId, TransportIdRef,
};
use lib3h_protocol::DidWork;
use std::{
    collections::VecDeque,
    io::{Read, Write},
};

static FAKE_PKCS12: &'static [u8] = include_bytes!("fake_key.p12");
static FAKE_PASS: &'static str = "hello";

// -- some internal types for readability -- //

type TlsConnectResult<T> = Result<TlsStream<T>, native_tls::HandshakeError<T>>;
type WsHandshakeError<T> =
    tungstenite::handshake::HandshakeError<tungstenite::handshake::client::ClientHandshake<T>>;
type WsConnectResult<T> =
    Result<(WsStream<T>, tungstenite::handshake::client::Response), WsHandshakeError<T>>;
type WsSrvHandshakeError<T> = tungstenite::handshake::HandshakeError<
    tungstenite::handshake::server::ServerHandshake<T, tungstenite::handshake::server::NoCallback>,
>;
type WsSrvAcceptResult<T> = Result<WsStream<T>, WsSrvHandshakeError<T>>;
type WssHandshakeError<T> = tungstenite::handshake::HandshakeError<
    tungstenite::handshake::client::ClientHandshake<TlsStream<T>>,
>;
type WssConnectResult<T> =
    Result<(WssStream<T>, tungstenite::handshake::client::Response), WssHandshakeError<T>>;
type WssSrvHandshakeError<T> = tungstenite::handshake::HandshakeError<
    tungstenite::handshake::server::ServerHandshake<
        TlsStream<T>,
        tungstenite::handshake::server::NoCallback,
    >,
>;
type WssSrvAcceptResult<T> = Result<WssStream<T>, WssSrvHandshakeError<T>>;

type BaseStream<T> = T;
type TlsMidHandshake<T> = native_tls::MidHandshakeTlsStream<BaseStream<T>>;
type TlsSrvMidHandshake<T> = native_tls::MidHandshakeTlsStream<BaseStream<T>>;
type TlsStream<T> = native_tls::TlsStream<BaseStream<T>>;
type WsMidHandshake<T> = tungstenite::handshake::MidHandshake<tungstenite::ClientHandshake<T>>;
type WsSrvMidHandshake<T> = tungstenite::handshake::MidHandshake<
    tungstenite::ServerHandshake<T, tungstenite::handshake::server::NoCallback>,
>;
type WssMidHandshake<T> =
    tungstenite::handshake::MidHandshake<tungstenite::ClientHandshake<TlsStream<T>>>;
type WssSrvMidHandshake<T> = tungstenite::handshake::MidHandshake<
    tungstenite::ServerHandshake<TlsStream<T>, tungstenite::handshake::server::NoCallback>,
>;
type WsStream<T> = tungstenite::protocol::WebSocket<T>;
type WssStream<T> = tungstenite::protocol::WebSocket<TlsStream<T>>;

type SocketMap<T> = std::collections::HashMap<String, TransportInfo<T>>;

// an internal state sequence for stream building
#[derive(Debug)]
enum WebsocketStreamState<T: Read + Write + std::fmt::Debug> {
    None,
    Connecting(BaseStream<T>),
    #[allow(dead_code)]
    ConnectingSrv(BaseStream<T>),
    TlsMidHandshake(TlsMidHandshake<T>),
    TlsSrvMidHandshake(TlsSrvMidHandshake<T>),
    TlsReady(TlsStream<T>),
    TlsSrvReady(TlsStream<T>),
    WsMidHandshake(WsMidHandshake<T>),
    WsSrvMidHandshake(WsSrvMidHandshake<T>),
    WssMidHandshake(WssMidHandshake<T>),
    WssSrvMidHandshake(WssSrvMidHandshake<T>),
    ReadyWs(Box<WsStream<T>>),
    ReadyWss(Box<WssStream<T>>),
}

/// how often should we send a heartbeat if we have not received msgs
pub const DEFAULT_HEARTBEAT_MS: usize = 2000;

/// when should we close a connection due to not receiving remote msgs
pub const DEFAULT_HEARTBEAT_WAIT_MS: usize = 5000;

/// Represents an individual connection
#[derive(Debug)]
struct TransportInfo<T: Read + Write + std::fmt::Debug> {
    id: TransportId,
    url: url::Url,
    last_msg: std::time::Instant,
    send_queue: Vec<Vec<u8>>,
    stateful_socket: WebsocketStreamState<T>,
}

impl<T: Read + Write + std::fmt::Debug> TransportInfo<T> {
    pub fn close(&mut self) -> TransportResult<()> {
        if let WebsocketStreamState::ReadyWss(socket) = &mut self.stateful_socket {
            socket.close(None)?;
            socket.write_pending()?;
        }
        self.stateful_socket = WebsocketStreamState::None;
        Ok(())
    }
}

pub struct TlsCertificate {
    pkcs12_data: Vec<u8>,
    passphrase: String,
}

pub enum TlsConfig {
    Unencrypted,
    FakeServer,
    SuppliedCertificate(TlsCertificate),
}

/// a factory callback for generating base streams of type T
pub type StreamFactory<T> = fn(uri: &str) -> TransportResult<T>;

/// A "Transport" implementation based off the websocket protocol
/// any rust io Read/Write stream should be able to serve as the base
pub struct TransportWss<T: Read + Write + std::fmt::Debug> {
    tls_config: TlsConfig,
    stream_factory: StreamFactory<T>,
    stream_sockets: SocketMap<T>,
    event_queue: Vec<TransportEvent>,
    n_id: u64,
    inbox: VecDeque<TransportCommand>,
}

impl<T: Read + Write + std::fmt::Debug> Transport for TransportWss<T> {
    /// connect to a remote websocket service
    fn connect(&mut self, uri: &str) -> TransportResult<TransportId> {
        let uri = url::Url::parse(uri)?;
        let host_port = format!(
            "{}:{}",
            uri.host_str()
                .ok_or_else(|| TransportError("bad connect host".into()))?,
            uri.port()
                .ok_or_else(|| TransportError("bad connect port".into()))?,
        );
        let socket = (self.stream_factory)(&host_port)?;
        let id = self.priv_next_id();
        let info = TransportInfo {
            id: id.clone(),
            url: uri,
            last_msg: std::time::Instant::now(),
            send_queue: Vec::new(),
            stateful_socket: WebsocketStreamState::Connecting(socket),
        };
        self.stream_sockets.insert(id.clone(), info);
        Ok(id)
    }

    /// close a currently tracked connection
    fn close(&mut self, id: &TransportIdRef) -> TransportResult<()> {
        if let Some(mut info) = self.stream_sockets.remove(id) {
            info.close()?;
        }
        Ok(())
    }

    /// close all currently tracked connections
    fn close_all(&mut self) -> TransportResult<()> {
        let mut errors: Vec<TransportError> = Vec::new();

        while !self.stream_sockets.is_empty() {
            let key = self
                .stream_sockets
                .keys()
                .next()
                .expect("should not be None")
                .to_string();
            if let Some(mut info) = self.stream_sockets.remove(&key) {
                if let Err(e) = info.close() {
                    errors.push(e);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.into())
        }
    }

    /// get a list of all open transport ids
    fn transport_id_list(&self) -> TransportResult<Vec<TransportId>> {
        Ok(self.stream_sockets.keys().map(|k| k.to_string()).collect())
    }

    /// get uri from a transportId
    fn get_uri(&self, id: &TransportIdRef) -> Option<String> {
        let res = self.stream_sockets.get(&id.to_string());
        res.map(|info| info.url.as_str().to_string())
    }

    fn post(&mut self, command: TransportCommand) -> TransportResult<()> {
        self.inbox.push_back(command);
        Ok(())
    }

    /// this should be called frequently on the event loop
    /// looks for incoming messages or processes ping/pong/close events etc
    fn process(&mut self) -> TransportResult<(DidWork, Vec<TransportEvent>)> {
        let did_work = self.priv_process_stream_sockets()?;

        Ok((did_work, self.event_queue.drain(..).collect()))
    }

    /// send a message to one or more remote connected nodes
    fn send(&mut self, id_list: &[&TransportIdRef], payload: &[u8]) -> TransportResult<()> {
        for id in id_list {
            if let Some(info) = self.stream_sockets.get_mut(&id.to_string()) {
                info.send_queue.push(payload.to_vec());
            }
        }

        Ok(())
    }

    /// send a message to all remote nodes
    fn send_all(&mut self, payload: &[u8]) -> TransportResult<()> {
        for info in self.stream_sockets.values_mut() {
            info.send_queue.push(payload.to_vec());
        }
        Ok(())
    }

    fn bind(&mut self, _url: &str) -> TransportResult<String> {
        // FIXME
        Ok(String::new())
    }
}

impl<T: Read + Write + std::fmt::Debug> TransportWss<T> {
    /// create a new websocket "Transport" instance of type T
    pub fn new(stream_factory: StreamFactory<T>) -> Self {
        TransportWss {
            tls_config: TlsConfig::FakeServer,
            stream_factory,
            stream_sockets: std::collections::HashMap::new(),
            event_queue: Vec::new(),
            n_id: 1,
            inbox: VecDeque::new(),
        }
    }

    /// connect and wait for a Connect event response
    pub fn wait_connect(&mut self, uri: &str) -> TransportResult<TransportId> {
        // Launch connection attempt
        let transport_id = self.connect(&uri)?;
        // Wait for a successful response
        let mut out = Vec::new();
        let start = std::time::Instant::now();
        while (start.elapsed().as_millis() as usize) < DEFAULT_HEARTBEAT_WAIT_MS {
            let (_did_work, evt_lst) = self.process()?;
            for evt in evt_lst {
                match evt {
                    TransportEvent::ConnectResult(id) => {
                        if id == transport_id {
                            return Ok(id);
                        }
                    }
                    _ => out.push(evt),
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        // Timed out
        Err(TransportError::new(format!(
            "ipc wss connection attempt timed out for '{}'. Received events: {:?}",
            transport_id, out
        )))
    }

    // -- private -- //

    // generate a unique id for
    fn priv_next_id(&mut self) -> String {
        let out = format!("ws{}", self.n_id);
        self.n_id += 1;
        out
    }

    // see if any work needs to be done on our stream sockets
    fn priv_process_stream_sockets(&mut self) -> TransportResult<bool> {
        let mut did_work = false;

        // take sockets out, so we can mut ref into self and it at same time
        let sockets: Vec<(String, TransportInfo<T>)> = self.stream_sockets.drain().collect();

        for (id, mut info) in sockets {
            if let Err(e) = self.priv_process_socket(&mut did_work, &mut info) {
                self.event_queue
                    .push(TransportEvent::TransportError(info.id.clone(), e));
            }
            if let WebsocketStreamState::None = info.stateful_socket {
                self.event_queue.push(TransportEvent::Closed(info.id));
                continue;
            }
            if info.last_msg.elapsed().as_millis() as usize > DEFAULT_HEARTBEAT_MS {
                if let WebsocketStreamState::ReadyWss(socket) = &mut info.stateful_socket {
                    socket.write_message(tungstenite::Message::Ping(vec![]))?;
                }
                if let WebsocketStreamState::ReadyWs(socket) = &mut info.stateful_socket {
                    socket.write_message(tungstenite::Message::Ping(vec![]))?;
                }
            } else if info.last_msg.elapsed().as_millis() as usize > DEFAULT_HEARTBEAT_WAIT_MS {
                self.event_queue.push(TransportEvent::Closed(info.id));
                info.stateful_socket = WebsocketStreamState::None;
                continue;
            }
            self.stream_sockets.insert(id, info);
        }

        Ok(did_work)
    }

    // process the state machine of an individual socket stream
    fn priv_process_socket(
        &mut self,
        did_work: &mut bool,
        info: &mut TransportInfo<T>,
    ) -> TransportResult<()> {
        // move the socket out, to be replaced
        let socket = std::mem::replace(&mut info.stateful_socket, WebsocketStreamState::None);

        match socket {
            WebsocketStreamState::None => {
                // stream must have closed, do nothing
                Ok(())
            }
            WebsocketStreamState::Connecting(socket) => {
                info.last_msg = std::time::Instant::now();
                *did_work = true;
                match &self.tls_config {
                    TlsConfig::Unencrypted => {
                        info.stateful_socket = self.priv_ws_handshake(
                            &info.id,
                            tungstenite::client(info.url.clone(), socket),
                        )?;
                    }
                    _ => {
                        let connector = native_tls::TlsConnector::builder()
                            .danger_accept_invalid_certs(true)
                            .danger_accept_invalid_hostnames(true)
                            .build()
                            .expect("failed to build TlsConnector");
                        info.stateful_socket =
                            self.priv_tls_handshake(connector.connect(info.url.as_str(), socket))?;
                    }
                }
                Ok(())
            }
            WebsocketStreamState::ConnectingSrv(socket) => {
                info.last_msg = std::time::Instant::now();
                *did_work = true;
                if let &TlsConfig::Unencrypted = &self.tls_config {
                    info.stateful_socket =
                        self.priv_ws_srv_handshake(&info.id, tungstenite::accept(socket))?;
                    return Ok(());
                }
                let ident = match &self.tls_config {
                    TlsConfig::Unencrypted => unimplemented!(),
                    TlsConfig::FakeServer => {
                        native_tls::Identity::from_pkcs12(FAKE_PKCS12, FAKE_PASS)?
                    }
                    TlsConfig::SuppliedCertificate(cert) => {
                        native_tls::Identity::from_pkcs12(&cert.pkcs12_data, &cert.passphrase)?
                    }
                };
                let acceptor = native_tls::TlsAcceptor::builder(ident)
                    .build()
                    .expect("failed to build TlsAcceptor");
                info.stateful_socket = self.priv_tls_srv_handshake(acceptor.accept(socket))?;
                Ok(())
            }
            WebsocketStreamState::TlsMidHandshake(socket) => {
                info.stateful_socket = self.priv_tls_handshake(socket.handshake())?;
                Ok(())
            }
            WebsocketStreamState::TlsSrvMidHandshake(socket) => {
                info.stateful_socket = self.priv_tls_srv_handshake(socket.handshake())?;
                Ok(())
            }
            WebsocketStreamState::TlsReady(socket) => {
                info.last_msg = std::time::Instant::now();
                *did_work = true;
                info.stateful_socket = self
                    .priv_wss_handshake(&info.id, tungstenite::client(info.url.clone(), socket))?;
                Ok(())
            }
            WebsocketStreamState::TlsSrvReady(socket) => {
                info.last_msg = std::time::Instant::now();
                *did_work = true;
                info.stateful_socket =
                    self.priv_wss_srv_handshake(&info.id, tungstenite::accept(socket))?;
                Ok(())
            }
            WebsocketStreamState::WsMidHandshake(socket) => {
                info.stateful_socket = self.priv_ws_handshake(&info.id, socket.handshake())?;
                Ok(())
            }
            WebsocketStreamState::WsSrvMidHandshake(socket) => {
                info.stateful_socket = self.priv_ws_srv_handshake(&info.id, socket.handshake())?;
                Ok(())
            }
            WebsocketStreamState::WssMidHandshake(socket) => {
                info.stateful_socket = self.priv_wss_handshake(&info.id, socket.handshake())?;
                Ok(())
            }
            WebsocketStreamState::WssSrvMidHandshake(socket) => {
                info.stateful_socket = self.priv_wss_srv_handshake(&info.id, socket.handshake())?;
                Ok(())
            }
            WebsocketStreamState::ReadyWs(mut socket) => {
                // This seems to be wrong. Messages shouldn't be drained.
                let msgs: Vec<Vec<u8>> = info.send_queue.drain(..).collect();
                for msg in msgs {
                    // TODO: fix this line! if there is an error, all the remaining messages will be lost!
                    socket.write_message(tungstenite::Message::Binary(msg))?;
                }

                match socket.read_message() {
                    Err(tungstenite::error::Error::Io(e)) => {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            info.stateful_socket = WebsocketStreamState::ReadyWs(socket);
                            return Ok(());
                        }
                        Err(e.into())
                    }
                    Err(tungstenite::error::Error::ConnectionClosed(_)) => {
                        // close event will be published
                        Ok(())
                    }
                    Err(e) => Err(e.into()),
                    Ok(msg) => {
                        info.last_msg = std::time::Instant::now();
                        *did_work = true;
                        let qmsg = match msg {
                            tungstenite::Message::Text(s) => Some(s.into_bytes()),
                            tungstenite::Message::Binary(b) => Some(b),
                            _ => None,
                        };

                        if let Some(msg) = qmsg {
                            self.event_queue
                                .push(TransportEvent::Received(info.id.clone(), msg));
                        }
                        info.stateful_socket = WebsocketStreamState::ReadyWs(socket);
                        Ok(())
                    }
                }
            }
            WebsocketStreamState::ReadyWss(mut socket) => {
                // This seems to be wrong. Messages shouldn't be drained.
                let msgs: Vec<Vec<u8>> = info.send_queue.drain(..).collect();
                for msg in msgs {
                    // TODO: fix this line! if there is an error, all the remaining messages will be lost!
                    socket.write_message(tungstenite::Message::Binary(msg))?;
                }

                match socket.read_message() {
                    Err(tungstenite::error::Error::Io(e)) => {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            info.stateful_socket = WebsocketStreamState::ReadyWss(socket);
                            return Ok(());
                        }
                        Err(e.into())
                    }
                    Err(tungstenite::error::Error::ConnectionClosed(_)) => {
                        // close event will be published
                        Ok(())
                    }
                    Err(e) => Err(e.into()),
                    Ok(msg) => {
                        info.last_msg = std::time::Instant::now();
                        *did_work = true;
                        let qmsg = match msg {
                            tungstenite::Message::Text(s) => Some(s.into_bytes()),
                            tungstenite::Message::Binary(b) => Some(b),
                            _ => None,
                        };

                        if let Some(msg) = qmsg {
                            self.event_queue
                                .push(TransportEvent::Received(info.id.clone(), msg));
                        }
                        info.stateful_socket = WebsocketStreamState::ReadyWss(socket);
                        Ok(())
                    }
                }
            }
        }
    }

    // process tls handshaking
    fn priv_tls_handshake(
        &mut self,
        res: TlsConnectResult<T>,
    ) -> TransportResult<WebsocketStreamState<T>> {
        match res {
            Err(native_tls::HandshakeError::WouldBlock(socket)) => {
                Ok(WebsocketStreamState::TlsMidHandshake(socket))
            }
            Err(e) => Err(e.into()),
            Ok(socket) => Ok(WebsocketStreamState::TlsReady(socket)),
        }
    }

    // process tls handshaking
    fn priv_tls_srv_handshake(
        &mut self,
        res: TlsConnectResult<T>,
    ) -> TransportResult<WebsocketStreamState<T>> {
        match res {
            Err(native_tls::HandshakeError::WouldBlock(socket)) => {
                Ok(WebsocketStreamState::TlsSrvMidHandshake(socket))
            }
            Err(e) => Err(e.into()),
            Ok(socket) => Ok(WebsocketStreamState::TlsSrvReady(socket)),
        }
    }

    // process websocket handshaking
    fn priv_ws_handshake(
        &mut self,
        id: &TransportId,
        res: WsConnectResult<T>,
    ) -> TransportResult<WebsocketStreamState<T>> {
        match res {
            Err(tungstenite::HandshakeError::Interrupted(socket)) => {
                Ok(WebsocketStreamState::WsMidHandshake(socket))
            }
            Err(e) => Err(e.into()),
            Ok((socket, _response)) => {
                self.event_queue
                    .push(TransportEvent::ConnectResult(id.clone()));
                Ok(WebsocketStreamState::ReadyWs(Box::new(socket)))
            }
        }
    }

    // process websocket handshaking
    fn priv_wss_handshake(
        &mut self,
        id: &TransportId,
        res: WssConnectResult<T>,
    ) -> TransportResult<WebsocketStreamState<T>> {
        match res {
            Err(tungstenite::HandshakeError::Interrupted(socket)) => {
                Ok(WebsocketStreamState::WssMidHandshake(socket))
            }
            Err(e) => Err(e.into()),
            Ok((socket, _response)) => {
                self.event_queue
                    .push(TransportEvent::ConnectResult(id.clone()));
                Ok(WebsocketStreamState::ReadyWss(Box::new(socket)))
            }
        }
    }

    // process websocket srv handshaking
    fn priv_ws_srv_handshake(
        &mut self,
        id: &TransportId,
        res: WsSrvAcceptResult<T>,
    ) -> TransportResult<WebsocketStreamState<T>> {
        match res {
            Err(tungstenite::HandshakeError::Interrupted(socket)) => {
                Ok(WebsocketStreamState::WsSrvMidHandshake(socket))
            }
            Err(e) => Err(e.into()),
            Ok(socket) => {
                self.event_queue
                    .push(TransportEvent::Connection(id.clone()));
                Ok(WebsocketStreamState::ReadyWs(Box::new(socket)))
            }
        }
    }

    // process websocket srv handshaking
    fn priv_wss_srv_handshake(
        &mut self,
        id: &TransportId,
        res: WssSrvAcceptResult<T>,
    ) -> TransportResult<WebsocketStreamState<T>> {
        match res {
            Err(tungstenite::HandshakeError::Interrupted(socket)) => {
                Ok(WebsocketStreamState::WssSrvMidHandshake(socket))
            }
            Err(e) => Err(e.into()),
            Ok(socket) => {
                self.event_queue
                    .push(TransportEvent::Connection(id.clone()));
                Ok(WebsocketStreamState::ReadyWss(Box::new(socket)))
            }
        }
    }
}
