//! abstraction for working with Websocket connections
//! based on any rust io Read/Write Stream

mod tcp;

use crate::transport::{
    error::{TransportError, TransportResult},
    protocol::{TransportCommand, TransportEvent},
    transport_trait::Transport,
    ConnectionId, ConnectionIdRef,
};
use lib3h_protocol::DidWork;
use std::{
    collections::VecDeque,
    io::{Read, Write},
    sync::{Arc, Mutex},
};

use url::Url;

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
type TlsMidHandshake<T> = native_tls::MidHandshakeTlsStream<BaseStream<T>>;

type BaseStream<T> = T;
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

type SocketMap<T> = std::collections::HashMap<String, WssInfo<T>>;

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
pub struct WssInfo<T: Read + Write + std::fmt::Debug> {
    id: ConnectionId,
    url: url::Url,
    last_msg: std::time::Instant,
    send_queue: Vec<Vec<u8>>,
    stateful_socket: WebsocketStreamState<T>,
}

impl<T: Read + Write + std::fmt::Debug> WssInfo<T> {
    pub fn close(&mut self) -> TransportResult<()> {
        if let WebsocketStreamState::ReadyWss(socket) = &mut self.stateful_socket {
            socket.close(None)?;
            socket.write_pending()?;
        }
        self.stateful_socket = WebsocketStreamState::None;
        Ok(())
    }

    pub fn new(id: ConnectionId, url: url::Url, socket: BaseStream<T>, is_server: bool) -> Self {
        WssInfo {
            id: id.clone(),
            url,
            last_msg: std::time::Instant::now(),
            send_queue: Vec::new(),
            stateful_socket: match is_server {
                false => WebsocketStreamState::Connecting(socket),
                true => WebsocketStreamState::ConnectingSrv(socket),
            },
        }
    }

    pub fn client(id: ConnectionId, url: url::Url, socket: BaseStream<T>) -> Self {
        Self::new(id, url, socket, false)
    }

    pub fn server(id: ConnectionId, url: url::Url, socket: BaseStream<T>) -> Self {
        Self::new(id, url, socket, true)
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

/// A factory callback for generating base streams of type T
pub type StreamFactory<T> = fn(uri: &str) -> TransportResult<T>;

pub trait IdGenerator {
    fn next_id(&mut self) -> ConnectionId;
}

#[derive(Clone)]
pub struct ConnectionIdFactory(Arc<Mutex<u64>>);
impl IdGenerator for ConnectionIdFactory {
    fn next_id(&mut self) -> ConnectionId {
        let mut n_id = self.0.lock().expect("could not lock mutex");
        let out = format!("ws{}", *n_id);
        *n_id += 1;
        out
    }
}

impl ConnectionIdFactory {
    pub fn new() -> Self {
        ConnectionIdFactory(Arc::new(Mutex::new(1)))
    }
}

/// A function that produces accepted sockets of type R wrapped in a TransportInfo
pub type Acceptor<T> = Box<FnMut(ConnectionIdFactory) -> TransportResult<WssInfo<T>>>;

/// A function that binds to a url and produces sockt acceptors of type T
pub type Bind<T> = Box<FnMut(&Url) -> TransportResult<Acceptor<T>>>;

/// An implememtation of Bind that accepts no connections.
fn noop_bind<T: std::fmt::Debug + std::io::Read + std::io::Write>(
    _url: &Url,
) -> TransportResult<Acceptor<T>> {
    Err(TransportError(
        "bind not configured (this is a noop impl)".into(),
    ))
}

/// A "Transport" implementation based off the websocket protocol
/// any rust io Read/Write stream should be able to serve as the base
pub struct TransportWss<T: Read + Write + std::fmt::Debug> {
    tls_config: TlsConfig,
    stream_factory: StreamFactory<T>,
    stream_sockets: SocketMap<T>,
    event_queue: Vec<TransportEvent>,
    n_id: ConnectionIdFactory,
    inbox: VecDeque<TransportCommand>,
    bind: Bind<T>,
    acceptor: TransportResult<Acceptor<T>>,
}

impl<T: Read + Write + std::fmt::Debug> Transport for TransportWss<T> {
    /// connect to a remote websocket service
    fn connect(&mut self, uri: &Url) -> TransportResult<ConnectionId> {
        let host_port = format!(
            "{}:{}",
            uri.host_str()
                .ok_or_else(|| TransportError("bad connect host".into()))?,
            uri.port()
                .ok_or_else(|| TransportError("bad connect port".into()))?,
        );
        let socket = (self.stream_factory)(&host_port)?;
        let id = self.priv_next_id();
        let info = WssInfo::client(id.clone(), uri.clone(), socket);
        self.stream_sockets.insert(id.clone(), info);
        Ok(id)
    }

    /// close a currently tracked connection
    fn close(&mut self, id: &ConnectionIdRef) -> TransportResult<()> {
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
    fn connection_id_list(&self) -> TransportResult<Vec<ConnectionId>> {
        Ok(self.stream_sockets.keys().map(|k| k.to_string()).collect())
    }

    /// get uri from a connectionId
    fn get_uri(&self, id: &ConnectionIdRef) -> Option<Url> {
        let res = self.stream_sockets.get(&id.to_string());
        res.map(|info| info.url.clone())
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
    fn send(&mut self, id_list: &[&ConnectionIdRef], payload: &[u8]) -> TransportResult<()> {
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

    fn bind(&mut self, url: &Url) -> TransportResult<Url> {
        let acceptor = (self.bind)(&url.clone());
        acceptor.map(|acceptor| {
            self.acceptor = Ok(acceptor);
            url.clone()
        })
    }
}

impl<T: Read + Write + std::fmt::Debug + std::marker::Sized> TransportWss<T> {
    pub fn new(stream_factory: StreamFactory<T>, bind: Bind<T>) -> Self {
        TransportWss {
            tls_config: TlsConfig::FakeServer,
            stream_factory,
            stream_sockets: std::collections::HashMap::new(),
            event_queue: Vec::new(),
            n_id: ConnectionIdFactory::new(),
            inbox: VecDeque::new(),
            bind,
            acceptor: Err(TransportError("acceptor not initialized".into())),
        }
    }

    /// create a new websocket "Transport" instance of type T for client connections
    pub fn client(stream_factory: StreamFactory<T>) -> Self {
        let bind: Bind<T> = Box::new(|url| noop_bind(url));
        Self::new(stream_factory, bind)
    }

    pub fn server(bind: Bind<T>) -> Self {
        Self::new(
            |_url| Err(TransportError("client connections unsupported".into())),
            bind,
        )
    }

    /// connect and wait for a Connect event response
    pub fn wait_connect(&mut self, uri: &Url) -> TransportResult<ConnectionId> {
        // Launch connection attempt
        let connection_id = self.connect(uri)?;
        // Wait for a successful response
        let mut out = Vec::new();
        let start = std::time::Instant::now();
        while (start.elapsed().as_millis() as usize) < DEFAULT_HEARTBEAT_WAIT_MS {
            let (_did_work, evt_lst) = self.process()?;
            for evt in evt_lst {
                match evt {
                    TransportEvent::ConnectResult(id) => {
                        if id == connection_id {
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
            connection_id, out
        )))
    }

    // -- private -- //

    // generate a unique id for
    fn priv_next_id(&mut self) -> ConnectionId {
        self.n_id.next_id()
    }

    fn priv_process_accept(&mut self) -> DidWork {
        match &mut self.acceptor {
            Err(err) => {
                println!("acceptor in error state: {:?}", err);
                false
            }
            Ok(acceptor) => (acceptor)(self.n_id.clone())
                .map(move |wss_info| {
                    let connection_id = wss_info.id.clone();
                    let _insert_result = self.stream_sockets.insert(connection_id, wss_info);
                    true
                })
                .unwrap_or_else(|err| {
                    println!("did not accept any connections: {:?}", err);
                    false
                }),
        }
    }

    // see if any work needs to be done on our stream sockets
    fn priv_process_stream_sockets(&mut self) -> TransportResult<DidWork> {
        let mut did_work = false;

        // accept some incoming connections
        did_work |= self.priv_process_accept();

        // take sockets out, so we can mut ref into self and it at same time
        let sockets: Vec<(String, WssInfo<T>)> = self.stream_sockets.drain().collect();

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
        info: &mut WssInfo<T>,
    ) -> TransportResult<()> {
        // move the socket out, to be replaced
        let socket = std::mem::replace(&mut info.stateful_socket, WebsocketStreamState::None);

        println!("transport_wss: socket={:?}", socket);
        std::io::stdout().flush().ok().expect("flush stdout");
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
        id: &ConnectionId,
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
        id: &ConnectionId,
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
        id: &ConnectionId,
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
        id: &ConnectionId,
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
