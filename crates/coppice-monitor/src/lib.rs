//! The live-monitor wire protocol, as `docs/engineering-spec.md` §5 lays it
//! out: request and reply headers, the reply checks, and the payloads v1
//! reads (FULLTREE, STATUS, recording start, transition drain, and the
//! breakpoint notice), and the blackboard request's payload; and
//! [`Client`], which sends them over the publisher's request socket (§5.1)
//! and decodes the blackboard reply (F15).
//!
//! Every multi-byte field is little-endian. The publisher writes them in
//! its host's byte order, which is little-endian on every platform it is
//! built for in practice.

use std::fmt;

use coppice_btlog::{RECORD_LEN, Status, Transition};

mod client;

pub use client::{Client, ClientError, FullTree};

/// The protocol byte of every request and reply.
pub const PROTOCOL: u8 = 2;
/// The publisher's default request port; its notice port is the next one.
pub const DEFAULT_PORT: u16 = 1667;
/// The size of a request header.
pub const REQUEST_HEADER_LEN: usize = 6;
/// The size of a reply header.
pub const REPLY_HEADER_LEN: usize = 22;
/// The most records the publisher buffers between drains (spec §5.6).
pub const TRANSITION_BUFFER: usize = 1000;

/// A request type, by its tag byte (spec §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestType {
    /// `T`: the tree XML.
    FullTree,
    /// `S`: the status buffer.
    Status,
    /// `B`: blackboard dumps.
    Blackboard,
    /// `I`: insert hooks.
    HookInsert,
    /// `R`: remove a hook.
    HookRemove,
    /// `U`: release a breakpoint.
    BreakpointUnlock,
    /// `D`: list the hooks.
    HooksDump,
    /// `A`: remove every hook.
    RemoveAllHooks,
    /// `X`: disable every hook.
    DisableAllHooks,
    /// `r`: start or stop recording.
    ToggleRecording,
    /// `t`: drain the recorded transitions.
    GetTransitions,
}

impl RequestType {
    /// Every request type.
    pub const ALL: [RequestType; 11] = [
        RequestType::FullTree,
        RequestType::Status,
        RequestType::Blackboard,
        RequestType::HookInsert,
        RequestType::HookRemove,
        RequestType::BreakpointUnlock,
        RequestType::HooksDump,
        RequestType::RemoveAllHooks,
        RequestType::DisableAllHooks,
        RequestType::ToggleRecording,
        RequestType::GetTransitions,
    ];

    /// The tag byte.
    pub fn tag(self) -> u8 {
        match self {
            RequestType::FullTree => b'T',
            RequestType::Status => b'S',
            RequestType::Blackboard => b'B',
            RequestType::HookInsert => b'I',
            RequestType::HookRemove => b'R',
            RequestType::BreakpointUnlock => b'U',
            RequestType::HooksDump => b'D',
            RequestType::RemoveAllHooks => b'A',
            RequestType::DisableAllHooks => b'X',
            RequestType::ToggleRecording => b'r',
            RequestType::GetTransitions => b't',
        }
    }

    /// The request type a tag byte names, if any.
    pub fn from_tag(tag: u8) -> Option<RequestType> {
        RequestType::ALL.into_iter().find(|t| t.tag() == tag)
    }
}

/// The first frame of a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestHeader {
    /// What is asked.
    pub kind: RequestType,
    /// Echoed by the reply; the client picks it.
    pub unique_id: u32,
}

impl RequestHeader {
    /// The 6 header bytes: protocol, tag, `unique_id`.
    pub fn encode(&self) -> [u8; REQUEST_HEADER_LEN] {
        let id = self.unique_id.to_le_bytes();
        [PROTOCOL, self.kind.tag(), id[0], id[1], id[2], id[3]]
    }
}

/// A reply that passed every check of spec §5.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply<'a> {
    /// The publisher's identity, fixed for its lifetime.
    pub tree_id: [u8; 16],
    /// The frames after the header.
    pub payload: &'a [Vec<u8>],
}

/// Checks a reply's frames against the request it answers (spec §5.2):
/// an error reply, the header's length, protocol byte, echoed type and
/// echoed `unique_id`.
pub fn check_reply<'a>(
    request: &RequestHeader,
    frames: &'a [Vec<u8>],
) -> Result<Reply<'a>, ReplyError> {
    let Some(header) = frames.first() else {
        return Err(ReplyError::Empty);
    };
    if header.as_slice() == b"error" {
        return Err(ReplyError::Publisher {
            text: frames
                .get(1)
                .map(|t| String::from_utf8_lossy(t).into_owned())
                .unwrap_or_default(),
        });
    }
    if header.len() != REPLY_HEADER_LEN {
        return Err(ReplyError::HeaderLength {
            found: header.len(),
        });
    }
    if header[0] != PROTOCOL {
        return Err(ReplyError::Protocol { found: header[0] });
    }
    if header[1] != request.kind.tag() {
        return Err(ReplyError::Type {
            expected: request.kind.tag(),
            found: header[1],
        });
    }
    let unique_id = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    if unique_id != request.unique_id {
        return Err(ReplyError::UniqueId {
            expected: request.unique_id,
            found: unique_id,
        });
    }
    let mut tree_id = [0; 16];
    tree_id.copy_from_slice(&header[6..]);
    Ok(Reply {
        tree_id,
        payload: &frames[1..],
    })
}

impl<'a> Reply<'a> {
    /// The single payload frame, for a request whose reply carries one.
    pub fn one_frame(&self) -> Result<&'a [u8], ReplyError> {
        match self.payload {
            [frame] => Ok(frame),
            _ => Err(ReplyError::FrameCount {
                expected: 2,
                found: self.payload.len() + 1,
            }),
        }
    }

    /// Checks that the reply is the header alone, for a request whose
    /// reply carries nothing.
    pub fn no_frames(&self) -> Result<(), ReplyError> {
        if self.payload.is_empty() {
            Ok(())
        } else {
            Err(ReplyError::FrameCount {
                expected: 1,
                found: self.payload.len() + 1,
            })
        }
    }
}

/// Why a reply was not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyError {
    /// The reply has no frames.
    Empty,
    /// The publisher answered with an error reply.
    Publisher {
        /// Its text.
        text: String,
    },
    /// The header frame is not 22 bytes.
    HeaderLength {
        /// Its length.
        found: usize,
    },
    /// The protocol byte is not [`PROTOCOL`].
    Protocol {
        /// The byte found.
        found: u8,
    },
    /// The echoed tag is not the request's.
    Type {
        /// The request's tag.
        expected: u8,
        /// The tag found.
        found: u8,
    },
    /// The echoed `unique_id` is not the request's.
    UniqueId {
        /// The request's.
        expected: u32,
        /// The one found.
        found: u32,
    },
    /// The reply has the wrong number of frames for its type.
    FrameCount {
        /// Frames expected, the header included.
        expected: usize,
        /// Frames found, the header included.
        found: usize,
    },
}

impl fmt::Display for ReplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplyError::Empty => write!(f, "empty reply"),
            ReplyError::Publisher { text } => {
                write!(f, "the publisher replied with an error: {text}")
            }
            ReplyError::HeaderLength { found } => write!(
                f,
                "reply header of {found} bytes, expected {REPLY_HEADER_LEN}"
            ),
            ReplyError::Protocol { found } => {
                write!(f, "reply protocol {found}, expected {PROTOCOL}")
            }
            ReplyError::Type { expected, found } => write!(
                f,
                "reply type {:?}, expected {:?}",
                char::from(*found),
                char::from(*expected)
            ),
            ReplyError::UniqueId { expected, found } => {
                write!(f, "reply id {found}, expected {expected}")
            }
            ReplyError::FrameCount { expected, found } => {
                write!(f, "reply of {found} frames, expected {expected}")
            }
        }
    }
}

impl std::error::Error for ReplyError {}

/// Tracks the publisher's `tree_id` across replies. A changed `tree_id`
/// means the publisher restarted, and the tree must be fetched again
/// (spec §5.2).
#[derive(Debug, Clone, Default)]
pub struct PublisherIdentity {
    seen: Option<[u8; 16]>,
}

impl PublisherIdentity {
    /// Records `tree_id`; `true` if an earlier reply carried a different one.
    pub fn restarted(&mut self, tree_id: [u8; 16]) -> bool {
        self.seen.replace(tree_id).is_some_and(|old| old != tree_id)
    }
}

/// A node's entry in the status buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    /// The node's current status.
    Is(Status),
    /// The node went to IDLE from this status (bytes 11–14).
    IdleAfter(Status),
}

/// One node of the status buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeStatus {
    /// The node's uid.
    pub uid: u16,
    /// Its state.
    pub state: NodeState,
}

/// A status byte outside 0–4 and 11–14, read as IDLE (spec §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownStatus {
    /// Its offset in the buffer.
    pub offset: usize,
    /// The byte.
    pub byte: u8,
}

/// A decoded status buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBuffer {
    /// The nodes, in buffer order.
    pub nodes: Vec<NodeStatus>,
    /// Bytes that were read as IDLE.
    pub unknown: Vec<UnknownStatus>,
}

/// Decodes a STATUS payload: 3 bytes per node, `uint16 uid` then a status
/// byte (spec §5.4). 0–4 are the status itself and 11–14 IDLE after
/// RUNNING, SUCCESS, FAILURE or SKIPPED. Any other byte, 10 included, is
/// read as IDLE and reported, never refused. Only a length that is not a
/// multiple of 3 is an error.
pub fn decode_status(payload: &[u8]) -> Result<StatusBuffer, LengthError> {
    let (entries, rest) = payload.as_chunks::<3>();
    if !rest.is_empty() {
        return Err(LengthError {
            length: payload.len(),
            unit: 3,
        });
    }
    let mut buffer = StatusBuffer {
        nodes: Vec::with_capacity(entries.len()),
        unknown: Vec::new(),
    };
    for (index, entry) in entries.iter().enumerate() {
        let byte = entry[2];
        let state = match byte {
            0..=4 => Status::from_byte(byte).map(NodeState::Is),
            11..=14 => Status::from_byte(byte - 10).map(NodeState::IdleAfter),
            _ => None,
        };
        let state = state.unwrap_or_else(|| {
            buffer.unknown.push(UnknownStatus {
                offset: index * 3 + 2,
                byte,
            });
            NodeState::Is(Status::Idle)
        });
        buffer.nodes.push(NodeStatus {
            uid: u16::from_le_bytes([entry[0], entry[1]]),
            state,
        });
    }
    Ok(buffer)
}

/// A payload whose length is not a whole number of its units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LengthError {
    /// The payload's length.
    pub length: usize,
    /// The unit size.
    pub unit: usize,
}

impl fmt::Display for LengthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "payload of {} bytes is not a whole number of {}-byte entries",
            self.length, self.unit
        )
    }
}

impl std::error::Error for LengthError {}

/// A transition drain (spec §5.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drain {
    /// The records, in order, each in `.btlog` record layout.
    pub transitions: Vec<Transition>,
    /// The drain holds exactly [`TRANSITION_BUFFER`] records, so the
    /// publisher may have dropped older ones. A heuristic: the publisher
    /// gives no signal.
    pub possible_gap: bool,
}

/// Why a drain was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainError {
    /// Not a whole number of records.
    Length(LengthError),
    /// A status byte outside 0–4.
    Status {
        /// Its offset in the payload.
        offset: usize,
        /// The byte.
        found: u8,
    },
}

impl fmt::Display for DrainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DrainError::Length(e) => e.fmt(f),
            DrainError::Status { offset, found } => {
                write!(f, "status byte {found} at byte {offset} is not 0-4")
            }
        }
    }
}

impl std::error::Error for DrainError {}

/// Decodes a GET_TRANSITIONS payload.
pub fn decode_drain(payload: &[u8]) -> Result<Drain, DrainError> {
    let (records, rest) = payload.as_chunks::<RECORD_LEN>();
    if !rest.is_empty() {
        return Err(DrainError::Length(LengthError {
            length: payload.len(),
            unit: RECORD_LEN,
        }));
    }
    let mut transitions = Vec::with_capacity(records.len());
    for (index, r) in records.iter().enumerate() {
        let Some(status) = Status::from_byte(r[8]) else {
            return Err(DrainError::Status {
                offset: index * RECORD_LEN + 8,
                found: r[8],
            });
        };
        let mut time = [0; 8];
        time[..6].copy_from_slice(&r[..6]);
        transitions.push(Transition {
            offset_us: u64::from_le_bytes(time),
            uid: u16::from_le_bytes([r[6], r[7]]),
            status,
        });
    }
    Ok(Drain {
        possible_gap: transitions.len() == TRANSITION_BUFFER,
        transitions,
    })
}

/// The payload of a recording request.
pub fn recording_payload(start: bool) -> &'static [u8] {
    if start { b"start" } else { b"stop" }
}

/// The payload of a blackboard request: the names joined by `;`. A name
/// that is empty or holds a `;` cannot be sent, and is refused, as is an
/// empty list, which asks for nothing (the publisher answers it with a
/// MessagePack nil, spec §5.5).
pub fn blackboard_payload(names: &[&str]) -> Result<Vec<u8>, String> {
    if names.is_empty() {
        return Err(String::new());
    }
    if let Some(bad) = names.iter().find(|n| n.is_empty() || n.contains(';')) {
        return Err((*bad).to_owned());
    }
    Ok(names.join(";").into_bytes())
}

/// A decimal payload that did not parse: a recording start's time, or a
/// breakpoint notice's uid. Only ASCII digits are accepted, after an
/// optional `-` where the type is signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberError {
    /// The payload, lossily decoded.
    pub found: String,
}

impl fmt::Display for NumberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expected a decimal number, found {:?}", self.found)
    }
}

impl std::error::Error for NumberError {}

fn decimal<T: std::str::FromStr>(payload: &[u8]) -> Result<T, NumberError> {
    let bad = || NumberError {
        found: String::from_utf8_lossy(payload).into_owned(),
    };
    let text = std::str::from_utf8(payload).map_err(|_| bad())?;
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    text.parse().map_err(|_| bad())
}

/// The recording start reply: the publisher's epoch time in microseconds,
/// the recording's T0 (spec §5.6).
pub fn decode_recording_start(payload: &[u8]) -> Result<i64, NumberError> {
    decimal(payload)
}

/// The tag of the BREAKPOINT_REACHED notice, published on the notice port.
pub const BREAKPOINT_REACHED: u8 = b'N';

/// Decodes a BREAKPOINT_REACHED notice: a 6-byte request header with tag
/// `N`, then the node's uid as a decimal string (BT 4.9.0
/// `src/loggers/groot2_publisher.cpp:589-593`).
pub fn decode_breakpoint(frames: &[Vec<u8>]) -> Result<u16, NoticeError> {
    let [header, uid] = frames else {
        return Err(NoticeError::FrameCount {
            found: frames.len(),
        });
    };
    if header.len() != REQUEST_HEADER_LEN
        || header[0] != PROTOCOL
        || header[1] != BREAKPOINT_REACHED
    {
        return Err(NoticeError::Header {
            found: header.clone(),
        });
    }
    decimal(uid).map_err(NoticeError::Uid)
}

/// Why a notice was not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeError {
    /// Not two frames.
    FrameCount {
        /// Frames found.
        found: usize,
    },
    /// The first frame is not a BREAKPOINT_REACHED header.
    Header {
        /// Its bytes.
        found: Vec<u8>,
    },
    /// The uid is not a decimal `u16`.
    Uid(NumberError),
}

impl fmt::Display for NoticeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NoticeError::FrameCount { found } => {
                write!(f, "notice of {found} frames, expected 2")
            }
            NoticeError::Header { found } => write!(f, "not a breakpoint notice header: {found:?}"),
            NoticeError::Uid(e) => write!(f, "breakpoint notice uid: {e}"),
        }
    }
}

impl std::error::Error for NoticeError {}
