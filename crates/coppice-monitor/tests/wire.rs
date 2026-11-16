//! The wire codec of spec §5, on byte buffers assembled here by hand.

use coppice_btlog::Status;
use coppice_monitor::{
    DEFAULT_PORT, DrainError, LengthError, NodeState, NoticeError, PROTOCOL, PublisherIdentity,
    ReplyError, RequestHeader, RequestType, TRANSITION_BUFFER, UnknownStatus, blackboard_payload,
    check_reply, decode_breakpoint, decode_drain, decode_recording_start, decode_status,
    recording_payload,
};

#[test]
fn tags_are_the_spec_table() {
    let table = [
        (RequestType::FullTree, b'T'),
        (RequestType::Status, b'S'),
        (RequestType::Blackboard, b'B'),
        (RequestType::HookInsert, b'I'),
        (RequestType::HookRemove, b'R'),
        (RequestType::BreakpointUnlock, b'U'),
        (RequestType::HooksDump, b'D'),
        (RequestType::RemoveAllHooks, b'A'),
        (RequestType::DisableAllHooks, b'X'),
        (RequestType::ToggleRecording, b'r'),
        (RequestType::GetTransitions, b't'),
    ];
    assert_eq!(table.len(), RequestType::ALL.len());
    for (kind, tag) in table {
        assert_eq!(kind.tag(), tag);
        assert_eq!(RequestType::from_tag(tag), Some(kind));
    }
    for tag in [0, b'N', b'x', 255] {
        assert_eq!(RequestType::from_tag(tag), None, "{tag}");
    }
    assert_eq!((PROTOCOL, DEFAULT_PORT), (2, 1667));
}

#[test]
fn a_request_header_is_protocol_tag_and_little_endian_id() {
    let h = RequestHeader {
        kind: RequestType::Status,
        unique_id: 0x0403_0201,
    };
    assert_eq!(h.encode(), [2, b'S', 1, 2, 3, 4]);
}

const TREE_ID: [u8; 16] = *b"0123456789abcdef";

fn reply(protocol: u8, tag: u8, id: u32, payload: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut header = vec![protocol, tag];
    header.extend_from_slice(&id.to_le_bytes());
    header.extend_from_slice(&TREE_ID);
    let mut frames = vec![header];
    frames.extend(payload.iter().map(|p| p.to_vec()));
    frames
}

const REQ: RequestHeader = RequestHeader {
    kind: RequestType::FullTree,
    unique_id: 77,
};

#[test]
fn a_matching_reply_is_accepted() {
    let frames = reply(2, b'T', 77, &[b"<root/>"]);
    let r = check_reply(&REQ, &frames).unwrap();
    assert_eq!(r.tree_id, TREE_ID);
    assert_eq!(r.one_frame().unwrap(), b"<root/>");
    assert_eq!(
        r.no_frames(),
        Err(ReplyError::FrameCount {
            expected: 1,
            found: 2
        })
    );
}

#[test]
fn a_header_only_reply_has_no_frames() {
    let frames = reply(2, b'T', 77, &[]);
    let r = check_reply(&REQ, &frames).unwrap();
    assert_eq!(r.no_frames(), Ok(()));
    assert_eq!(
        r.one_frame(),
        Err(ReplyError::FrameCount {
            expected: 2,
            found: 1
        })
    );
    let frames = reply(2, b'T', 77, &[b"a", b"b"]);
    assert!(check_reply(&REQ, &frames).unwrap().one_frame().is_err());
}

#[test]
fn every_mismatch_is_a_typed_error() {
    assert_eq!(check_reply(&REQ, &[]), Err(ReplyError::Empty));
    assert_eq!(
        check_reply(&REQ, &reply(3, b'T', 77, &[])),
        Err(ReplyError::Protocol { found: 3 })
    );
    assert_eq!(
        check_reply(&REQ, &reply(2, b'S', 77, &[])),
        Err(ReplyError::Type {
            expected: b'T',
            found: b'S'
        })
    );
    assert_eq!(
        check_reply(&REQ, &reply(2, b'T', 78, &[])),
        Err(ReplyError::UniqueId {
            expected: 77,
            found: 78
        })
    );
    // The request header alone, as a reply, is too short.
    let short = vec![REQ.encode().to_vec()];
    assert_eq!(
        check_reply(&REQ, &short),
        Err(ReplyError::HeaderLength { found: 6 })
    );
    let mut long = reply(2, b'T', 77, &[]);
    long[0].push(0);
    assert_eq!(
        check_reply(&REQ, &long),
        Err(ReplyError::HeaderLength { found: 23 })
    );
}

#[test]
fn an_error_reply_carries_its_text() {
    let frames = vec![b"error".to_vec(), b"must be 2 parts message".to_vec()];
    assert_eq!(
        check_reply(&REQ, &frames),
        Err(ReplyError::Publisher {
            text: "must be 2 parts message".to_owned()
        })
    );
    assert_eq!(
        check_reply(&REQ, &[b"error".to_vec()]),
        Err(ReplyError::Publisher {
            text: String::new()
        })
    );
}

#[test]
fn a_changed_tree_id_means_a_restart() {
    let mut p = PublisherIdentity::default();
    assert!(!p.restarted([1; 16]));
    assert!(!p.restarted([1; 16]));
    assert!(p.restarted([2; 16]));
    assert!(!p.restarted([2; 16]));
}

fn entry(uid: u16, byte: u8) -> [u8; 3] {
    let u = uid.to_le_bytes();
    [u[0], u[1], byte]
}

#[test]
fn status_bytes_decode_as_specified() {
    let mut buf = Vec::new();
    for (i, byte) in [0u8, 1, 2, 3, 4, 11, 12, 13, 14].into_iter().enumerate() {
        buf.extend_from_slice(&entry(0x0100 + i as u16, byte));
    }
    let s = decode_status(&buf).unwrap();
    assert!(s.unknown.is_empty());
    use NodeState::*;
    use Status::*;
    let states: Vec<_> = s.nodes.iter().map(|n| (n.uid, n.state)).collect();
    assert_eq!(
        states,
        [
            (0x100, Is(Idle)),
            (0x101, Is(Running)),
            (0x102, Is(Success)),
            (0x103, Is(Failure)),
            (0x104, Is(Skipped)),
            (0x105, IdleAfter(Running)),
            (0x106, IdleAfter(Success)),
            (0x107, IdleAfter(Failure)),
            (0x108, IdleAfter(Skipped)),
        ]
    );
}

#[test]
fn other_status_bytes_are_idle_with_a_report() {
    let mut buf = entry(1, 2).to_vec();
    for (uid, byte) in [(2, 10), (3, 5), (4, 9), (5, 15), (6, 255)] {
        buf.extend_from_slice(&entry(uid, byte));
    }
    let s = decode_status(&buf).unwrap();
    assert_eq!(s.nodes.len(), 6);
    assert_eq!(s.nodes[0].state, NodeState::Is(Status::Success));
    for n in &s.nodes[1..] {
        assert_eq!(n.state, NodeState::Is(Status::Idle), "uid {}", n.uid);
    }
    let unknown: Vec<_> = s.unknown.iter().map(|u| (u.offset, u.byte)).collect();
    assert_eq!(unknown, [(5, 10), (8, 5), (11, 9), (14, 15), (17, 255)]);
    assert_eq!(
        s.unknown[0],
        UnknownStatus {
            offset: 5,
            byte: 10
        }
    );
}

#[test]
fn a_ragged_status_buffer_is_refused() {
    assert!(decode_status(&[]).unwrap().nodes.is_empty());
    for len in [1, 2, 4, 5, 7] {
        assert_eq!(
            decode_status(&vec![0; len]),
            Err(LengthError {
                length: len,
                unit: 3
            })
        );
    }
}

fn record(offset: u64, uid: u16, status: u8) -> Vec<u8> {
    let mut r = offset.to_le_bytes()[..6].to_vec();
    r.extend_from_slice(&uid.to_le_bytes());
    r.push(status);
    r
}

#[test]
fn a_drain_decodes_records() {
    let mut buf = record(5, 0x0201, 1);
    buf.extend(record((1 << 48) - 1, 3, 0));
    let d = decode_drain(&buf).unwrap();
    assert!(!d.possible_gap);
    let got: Vec<_> = d
        .transitions
        .iter()
        .map(|t| (t.offset_us, t.uid, t.status))
        .collect();
    assert_eq!(
        got,
        [
            (5, 0x0201, Status::Running),
            ((1 << 48) - 1, 3, Status::Idle)
        ]
    );
    assert!(decode_drain(&[]).unwrap().transitions.is_empty());
}

#[test]
fn a_full_drain_may_have_a_gap() {
    for (count, gap) in [
        (TRANSITION_BUFFER - 1, false),
        (TRANSITION_BUFFER, true),
        (TRANSITION_BUFFER + 1, false),
    ] {
        let buf: Vec<u8> = (0..count).flat_map(|i| record(i as u64, 1, 1)).collect();
        assert_eq!(decode_drain(&buf).unwrap().possible_gap, gap, "{count}");
    }
}

#[test]
fn a_bad_drain_is_refused() {
    let mut buf = record(1, 1, 1);
    buf.extend(record(2, 1, 12));
    assert_eq!(
        decode_drain(&buf),
        Err(DrainError::Status {
            offset: 17,
            found: 12
        })
    );
    assert_eq!(
        decode_drain(&buf[..10]),
        Err(DrainError::Length(LengthError {
            length: 10,
            unit: 9
        }))
    );
}

#[test]
fn recording_payloads() {
    assert_eq!(recording_payload(true), b"start");
    assert_eq!(recording_payload(false), b"stop");
    assert_eq!(
        decode_recording_start(b"1794812542674028"),
        Ok(1_794_812_542_674_028)
    );
    for bad in [
        &b""[..],
        b"-5",
        b"+5",
        b" 5",
        b"5x",
        b"99999999999999999999",
        b"\xff",
    ] {
        assert!(decode_recording_start(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn blackboard_names_are_joined_by_semicolons() {
    assert_eq!(
        blackboard_payload(&["Main", "Inner"]).unwrap(),
        b"Main;Inner"
    );
    assert_eq!(blackboard_payload(&[]).unwrap(), b"");
    assert_eq!(blackboard_payload(&["a;b"]), Err("a;b".to_owned()));
    assert_eq!(blackboard_payload(&["a", ""]), Err(String::new()));
}

fn notice(uid: &[u8]) -> Vec<Vec<u8>> {
    vec![vec![2, b'N', 9, 9, 9, 9], uid.to_vec()]
}

#[test]
fn a_breakpoint_notice_names_its_node() {
    assert_eq!(decode_breakpoint(&notice(b"42")), Ok(42));
    assert_eq!(decode_breakpoint(&notice(b"65535")), Ok(65535));
    assert!(matches!(
        decode_breakpoint(&notice(b"65536")),
        Err(NoticeError::Uid(_))
    ));
    assert_eq!(
        decode_breakpoint(&notice(b"1")[..1]),
        Err(NoticeError::FrameCount { found: 1 })
    );
    for header in [
        vec![2, b'T', 0, 0, 0, 0],
        vec![3, b'N', 0, 0, 0, 0],
        vec![2, b'N', 0, 0, 0],
        vec![2, b'N', 0, 0, 0, 0, 0],
    ] {
        let frames = vec![header.clone(), b"1".to_vec()];
        assert_eq!(
            decode_breakpoint(&frames),
            Err(NoticeError::Header { found: header })
        );
    }
}
