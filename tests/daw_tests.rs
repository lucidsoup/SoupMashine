use soupmashine::daw::{Authority, Note, Op, Session};

/// Drive a request stream through the server authority and confirm every one of
/// the 4 client replicas converges to the exact authoritative state.
fn run_and_converge(requests: Vec<Op>) -> Session {
    let mut authority = Authority::new();
    let mut clients = vec![Session::new(); 4];

    for req in requests {
        let concrete = authority.submit(req); // assigns ids, applies to canon
        for c in clients.iter_mut() {
            c.apply(&concrete);
        }
    }

    for (i, c) in clients.iter().enumerate() {
        assert_eq!(*c, authority.session, "client {i} diverged");
    }
    authority.session
}

#[test]
fn empty_session_defaults() {
    let s = Session::new();
    assert_eq!(s.tempo, 120.0);
    assert!(!s.playing);
    assert!(s.tracks.is_empty());
    assert_eq!(s.revision, 0);
}

#[test]
fn add_track_assigns_ids_and_converges() {
    let s = run_and_converge(vec![
        Op::AddTrack {
            id: 0,
            name: "Drums".into(),
            owner: Some(0),
        },
        Op::AddTrack {
            id: 0,
            name: "Bass".into(),
            owner: Some(1),
        },
    ]);
    assert_eq!(s.tracks.len(), 2);
    // Authority assigns sequential ids starting at 1.
    assert_eq!(s.tracks[0].id, 1);
    assert_eq!(s.tracks[1].id, 2);
    assert_eq!(s.tracks[0].name, "Drums");
    assert_eq!(s.tracks[1].owner, Some(1));
}

#[test]
fn clips_and_notes_converge() {
    let note = Note {
        pitch: 60,
        velocity: 100,
        start: 0.0,
        length: 1.0,
    };
    let s = run_and_converge(vec![
        Op::AddTrack {
            id: 0,
            name: "Keys".into(),
            owner: None,
        },
        // track id 1, clip id 2 (next id after the track)
        Op::AddClip {
            track: 1,
            id: 0,
            start: 0.0,
            length: 4.0,
        },
        Op::AddNote {
            track: 1,
            clip: 2,
            note,
        },
    ]);
    let track = s.track(1).unwrap();
    assert_eq!(track.clips.len(), 1);
    assert_eq!(track.clips[0].id, 2);
    assert_eq!(track.clips[0].notes, vec![note]);
}

#[test]
fn mixer_and_transport_ops() {
    let s = run_and_converge(vec![
        Op::SetTempo { bpm: 128.0 },
        Op::SetPlaying { playing: true },
        Op::AddTrack {
            id: 0,
            name: "Lead".into(),
            owner: None,
        },
        Op::SetVolume {
            track: 1,
            volume: 0.5,
        },
        Op::SetMuted {
            track: 1,
            muted: true,
        },
    ]);
    assert_eq!(s.tempo, 128.0);
    assert!(s.playing);
    assert_eq!(s.track(1).unwrap().volume, 0.5);
    assert!(s.track(1).unwrap().muted);
}

#[test]
fn revision_advances_per_op() {
    let mut s = Session::new();
    s.apply(&Op::SetTempo { bpm: 100.0 });
    s.apply(&Op::SetPlaying { playing: true });
    assert_eq!(s.revision, 2);
}

#[test]
fn players_join_and_leave() {
    let s = run_and_converge(vec![
        Op::PlayerJoin {
            id: 0,
            name: "alice".into(),
        },
        Op::PlayerJoin {
            id: 1,
            name: "bob".into(),
        },
        Op::PlayerLeave { id: 0 },
    ]);
    assert_eq!(s.players.len(), 1);
    assert_eq!(s.players[0].name, "bob");
}

#[test]
fn op_roundtrips_through_json() {
    let op = Op::AddNote {
        track: 3,
        clip: 7,
        note: Note {
            pitch: 64,
            velocity: 90,
            start: 1.5,
            length: 0.5,
        },
    };
    let json = serde_json::to_string(&op).unwrap();
    let back: Op = serde_json::from_str(&json).unwrap();
    assert_eq!(op, back);
}
