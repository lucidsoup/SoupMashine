//! The shared DAW project model and the collaborative edit operations that act
//! on it.
//!
//! Multiplayer works by replicating a single authoritative [`Session`] document.
//! Clients submit [`Op`]s; the server assigns ids, applies them in a total
//! order, and broadcasts the concrete ops to every peer. Because [`Session::apply`]
//! is deterministic, applying the same ordered op stream makes all peers
//! converge to byte-identical state.

use serde::{Deserialize, Serialize};

pub type TrackId = u64;
pub type ClipId = u64;
pub type PlayerId = u8;

/// Maximum number of simultaneous players.
pub const MAX_PLAYERS: usize = 4;

/// A single MIDI note inside a clip. Times are in beats.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub pitch: u8,
    pub velocity: u8,
    pub start: f64,
    pub length: f64,
}

/// A clip on the timeline holding MIDI notes. (Audio clips come later.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: ClipId,
    pub start: f64,
    pub length: f64,
    pub notes: Vec<Note>,
}

/// A reference to a plugin in a track's device chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginRef {
    pub uri: String,
    pub name: String,
}

/// A mixer/arrangement track, optionally "owned" by one player.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub owner: Option<PlayerId>,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub clips: Vec<Clip>,
    pub plugins: Vec<PluginRef>,
}

impl Track {
    fn new(id: TrackId, name: String, owner: Option<PlayerId>) -> Self {
        Track {
            id,
            name,
            owner,
            volume: 0.8,
            pan: 0.0,
            muted: false,
            clips: Vec::new(),
            plugins: Vec::new(),
        }
    }
}

/// One connected player.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
}

/// The whole collaborative project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub tempo: f32,
    pub playing: bool,
    pub players: Vec<Player>,
    pub tracks: Vec<Track>,
    /// Monotonic revision, bumped on every applied op.
    pub revision: u64,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            tempo: 120.0,
            playing: false,
            players: Vec::new(),
            tracks: Vec::new(),
            revision: 0,
        }
    }
}

/// A collaborative edit. Ids on creation ops are assigned by the server (via
/// [`Authority::prepare`]) before broadcast, so every peer applies identical ops.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    SetTempo {
        bpm: f32,
    },
    SetPlaying {
        playing: bool,
    },
    PlayerJoin {
        id: PlayerId,
        name: String,
    },
    PlayerLeave {
        id: PlayerId,
    },
    AddTrack {
        id: TrackId,
        name: String,
        owner: Option<PlayerId>,
    },
    RemoveTrack {
        id: TrackId,
    },
    RenameTrack {
        id: TrackId,
        name: String,
    },
    SetVolume {
        track: TrackId,
        volume: f32,
    },
    SetPan {
        track: TrackId,
        pan: f32,
    },
    SetMuted {
        track: TrackId,
        muted: bool,
    },
    AddClip {
        track: TrackId,
        id: ClipId,
        start: f64,
        length: f64,
    },
    RemoveClip {
        track: TrackId,
        clip: ClipId,
    },
    AddNote {
        track: TrackId,
        clip: ClipId,
        note: Note,
    },
    AddPlugin {
        track: TrackId,
        uri: String,
        name: String,
    },
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    fn track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    /// Apply a concrete op. Deterministic: the same op stream yields the same
    /// state on every peer.
    pub fn apply(&mut self, op: &Op) {
        match op {
            Op::SetTempo { bpm } => self.tempo = bpm.clamp(20.0, 999.0),
            Op::SetPlaying { playing } => self.playing = *playing,
            Op::PlayerJoin { id, name } => {
                if !self.players.iter().any(|p| p.id == *id) {
                    self.players.push(Player {
                        id: *id,
                        name: name.clone(),
                    });
                }
            }
            Op::PlayerLeave { id } => self.players.retain(|p| p.id != *id),
            Op::AddTrack { id, name, owner } => {
                if self.track(*id).is_none() {
                    self.tracks.push(Track::new(*id, name.clone(), *owner));
                }
            }
            Op::RemoveTrack { id } => self.tracks.retain(|t| t.id != *id),
            Op::RenameTrack { id, name } => {
                if let Some(t) = self.track_mut(*id) {
                    t.name = name.clone();
                }
            }
            Op::SetVolume { track, volume } => {
                if let Some(t) = self.track_mut(*track) {
                    t.volume = volume.clamp(0.0, 2.0);
                }
            }
            Op::SetPan { track, pan } => {
                if let Some(t) = self.track_mut(*track) {
                    t.pan = pan.clamp(-1.0, 1.0);
                }
            }
            Op::SetMuted { track, muted } => {
                if let Some(t) = self.track_mut(*track) {
                    t.muted = *muted;
                }
            }
            Op::AddClip {
                track,
                id,
                start,
                length,
            } => {
                if let Some(t) = self.track_mut(*track) {
                    if !t.clips.iter().any(|c| c.id == *id) {
                        t.clips.push(Clip {
                            id: *id,
                            start: *start,
                            length: *length,
                            notes: Vec::new(),
                        });
                    }
                }
            }
            Op::RemoveClip { track, clip } => {
                if let Some(t) = self.track_mut(*track) {
                    t.clips.retain(|c| c.id != *clip);
                }
            }
            Op::AddNote { track, clip, note } => {
                if let Some(t) = self.track_mut(*track) {
                    if let Some(c) = t.clips.iter_mut().find(|c| c.id == *clip) {
                        c.notes.push(*note);
                    }
                }
            }
            Op::AddPlugin { track, uri, name } => {
                if let Some(t) = self.track_mut(*track) {
                    t.plugins.push(PluginRef {
                        uri: uri.clone(),
                        name: name.clone(),
                    });
                }
            }
        }
        self.revision += 1;
    }
}

/// Server-side authority: keeps the canonical session and assigns ids to
/// creation ops so all peers stay consistent.
pub struct Authority {
    pub session: Session,
    next_id: u64,
}

impl Default for Authority {
    fn default() -> Self {
        Authority::new()
    }
}

impl Authority {
    pub fn new() -> Self {
        Authority {
            session: Session::new(),
            next_id: 1,
        }
    }

    fn fresh_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Turn a client request into a concrete op (assigning ids), apply it to the
    /// canonical session, and return the op to broadcast to all peers.
    pub fn submit(&mut self, mut op: Op) -> Op {
        match &mut op {
            Op::AddTrack { id, .. } => *id = self.fresh_id(),
            Op::AddClip { id, .. } => *id = self.fresh_id(),
            _ => {}
        }
        self.session.apply(&op);
        op
    }
}
