//! Multiplayer session sync.
//!
//! A [`serve`] process holds the authoritative [`Authority`] and relays
//! newline-framed JSON [`Message`]s to up to [`MAX_PLAYERS`] connected clients.
//! Each client keeps a local [`Session`] replica kept in lock-step by applying
//! the server's broadcast ops. Audio is rendered locally on every peer; only the
//! lightweight project document and transport travel over the wire.

use crate::daw::{Authority, Op, PlayerId, Session, MAX_PLAYERS};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// Wire protocol message (one JSON object per line).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// First message from a client after connecting.
    Hello { name: String },
    /// Server -> client: your assigned player id.
    Welcome { player: PlayerId },
    /// Server -> client: full project state (sent once on join).
    Snapshot(Session),
    /// A concrete, id-assigned edit. Server broadcasts; clients apply.
    Op(Op),
}

fn write_msg(stream: &mut TcpStream, msg: &Message) -> io::Result<()> {
    let mut line = serde_json::to_string(msg)?;
    line.push('\n');
    stream.write_all(line.as_bytes())
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

struct Peer {
    player: PlayerId,
    stream: TcpStream,
}

#[derive(Default)]
struct ServerState {
    authority: Authority,
    peers: Vec<Peer>,
    next_player: PlayerId,
}

impl ServerState {
    fn broadcast(&mut self, msg: &Message) {
        self.peers
            .retain_mut(|p| write_msg(&mut p.stream, msg).is_ok());
    }
}

/// Run the session server until the listener errors. Blocks the calling thread.
pub fn serve(addr: &str) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    println!("SoupMashine session server listening on {addr}");
    let state = Arc::new(Mutex::new(ServerState::default()));

    for incoming in listener.incoming() {
        let stream = match incoming {
            Ok(s) => s,
            Err(e) => {
                eprintln!("accept error: {e}");
                continue;
            }
        };
        let state = state.clone();
        std::thread::spawn(move || {
            if let Err(e) = handle_client(stream, state) {
                eprintln!("client disconnected: {e}");
            }
        });
    }
    Ok(())
}

fn handle_client(stream: TcpStream, state: Arc<Mutex<ServerState>>) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    // First line must be Hello.
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let name = match serde_json::from_str::<Message>(line.trim()) {
        Ok(Message::Hello { name }) => name,
        _ => "player".to_string(),
    };

    // Register the peer.
    let player = {
        let mut s = state.lock().unwrap();
        if s.peers.len() >= MAX_PLAYERS {
            let mut reject = stream;
            let _ = write_msg(&mut reject, &Message::Snapshot(s.authority.session.clone()));
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "session full",
            ));
        }
        let player = s.next_player;
        s.next_player = s.next_player.wrapping_add(1);

        let mut writer = stream.try_clone()?;
        write_msg(&mut writer, &Message::Welcome { player })?;
        write_msg(&mut writer, &Message::Snapshot(s.authority.session.clone()))?;
        s.peers.push(Peer {
            player,
            stream: writer,
        });

        // Announce the join to everyone.
        let join = s.authority.submit(Op::PlayerJoin {
            id: player,
            name: name.clone(),
        });
        s.broadcast(&Message::Op(join));
        player
    };

    // Relay this client's ops.
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break; // disconnected
        }
        if let Ok(Message::Op(op)) = serde_json::from_str::<Message>(line.trim()) {
            let mut s = state.lock().unwrap();
            let concrete = s.authority.submit(op);
            s.broadcast(&Message::Op(concrete));
        }
    }

    // Clean up on disconnect.
    let mut s = state.lock().unwrap();
    s.peers.retain(|p| p.player != player);
    let leave = s.authority.submit(Op::PlayerLeave { id: player });
    s.broadcast(&Message::Op(leave));
    Ok(())
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// A connected client holding a live replica of the shared session.
pub struct Client {
    stream: TcpStream,
    /// Local replica, updated by the background reader thread.
    pub session: Arc<Mutex<Session>>,
    /// This client's player id (set once Welcome arrives).
    pub player: Arc<Mutex<Option<PlayerId>>>,
}

impl Client {
    /// Connect to a server and start replicating its session.
    pub fn connect(addr: &str, name: &str) -> io::Result<Client> {
        let mut stream = TcpStream::connect(addr)?;
        write_msg(
            &mut stream,
            &Message::Hello {
                name: name.to_string(),
            },
        )?;

        let session = Arc::new(Mutex::new(Session::new()));
        let player = Arc::new(Mutex::new(None));

        let reader_stream = stream.try_clone()?;
        let session_bg = session.clone();
        let player_bg = player.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(reader_stream);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                match serde_json::from_str::<Message>(line.trim()) {
                    Ok(Message::Snapshot(s)) => *session_bg.lock().unwrap() = s,
                    Ok(Message::Op(op)) => session_bg.lock().unwrap().apply(&op),
                    Ok(Message::Welcome { player }) => *player_bg.lock().unwrap() = Some(player),
                    _ => {}
                }
            }
        });

        Ok(Client {
            stream,
            session,
            player,
        })
    }

    /// Submit an edit to the server. Local state updates when the server echoes
    /// it back (so all peers apply it in the same order).
    pub fn submit(&mut self, op: Op) -> io::Result<()> {
        write_msg(&mut self.stream, &Message::Op(op))
    }

    /// Snapshot the current replica.
    pub fn session(&self) -> Session {
        self.session.lock().unwrap().clone()
    }
}
