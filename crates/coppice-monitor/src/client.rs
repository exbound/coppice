//! The blocking client over ZeroMQ (spec §5.1, design decision D4): one REQ
//! socket, one request at a time, each with a timeout.

use std::fmt;
use std::time::Duration;

use crate::{
    Drain, DrainError, LengthError, NumberError, ReplyError, RequestHeader, RequestType,
    StatusBuffer, blackboard_payload, check_reply, decode_drain, decode_recording_start,
    decode_status, recording_payload,
};

/// A FULLTREE reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullTree {
    /// The tree XML in the runtime form.
    pub xml: Vec<u8>,
    /// The publisher's identity differs from the last FULLTREE's: it
    /// restarted since.
    pub restarted: bool,
}

/// A connection to a publisher's request socket.
///
/// A request that gets no reply within the timeout leaves a REQ socket
/// unusable, so the client then closes it and connects a new one (spec
/// §5.2); the next request goes out on the new socket.
pub struct Client {
    context: zmq::Context,
    socket: zmq::Socket,
    endpoint: String,
    timeout: Duration,
    next_id: u32,
    /// The publisher's `tree_id` as of the last FULLTREE.
    tree_id: Option<[u8; 16]>,
}

/// Why a request did not give its result.
#[derive(Debug)]
pub enum ClientError {
    /// The socket could not be created, connected, written or read.
    Socket(String),
    /// No reply came within the timeout; the socket was replaced.
    Timeout(Duration),
    /// The reply failed a check of spec §5.2, or was an error reply.
    Reply(ReplyError),
    /// No FULLTREE has succeeded yet: the session is not up (spec §5.4),
    /// so nothing else is sent.
    NotStarted,
    /// The reply carried a `tree_id` other than the one of the last
    /// FULLTREE: the publisher restarted. Every request but a FULLTREE
    /// gets this until the tree is fetched again ([`Client::full_tree`]),
    /// since until then its statuses mean nothing.
    Restarted,
    /// The payload did not decode.
    Payload(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::Socket(e) => write!(f, "socket: {e}"),
            ClientError::Timeout(t) => write!(f, "no reply within {} ms", t.as_millis()),
            ClientError::Reply(e) => e.fmt(f),
            ClientError::NotStarted => write!(f, "no tree fetched yet"),
            ClientError::Restarted => write!(f, "the publisher restarted"),
            ClientError::Payload(e) => write!(f, "reply payload: {e}"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<ReplyError> for ClientError {
    fn from(e: ReplyError) -> Self {
        ClientError::Reply(e)
    }
}

fn socket_error(e: zmq::Error) -> ClientError {
    ClientError::Socket(e.to_string())
}

fn payload_error(e: impl fmt::Display) -> ClientError {
    ClientError::Payload(e.to_string())
}

impl From<LengthError> for ClientError {
    fn from(e: LengthError) -> Self {
        payload_error(e)
    }
}

impl From<DrainError> for ClientError {
    fn from(e: DrainError) -> Self {
        payload_error(e)
    }
}

impl From<NumberError> for ClientError {
    fn from(e: NumberError) -> Self {
        payload_error(e)
    }
}

fn request_socket(
    context: &zmq::Context,
    endpoint: &str,
    timeout: Duration,
) -> Result<zmq::Socket, ClientError> {
    // Whole milliseconds, rounded up, and at least 1: ZeroMQ reads 0 as
    // "do not wait".
    let millis = i32::try_from(timeout.as_micros().div_ceil(1000).max(1)).unwrap_or(i32::MAX);
    let socket = context.socket(zmq::REQ).map_err(socket_error)?;
    socket.set_linger(0).map_err(socket_error)?;
    socket.set_rcvtimeo(millis).map_err(socket_error)?;
    socket.set_sndtimeo(millis).map_err(socket_error)?;
    socket.connect(endpoint).map_err(socket_error)?;
    Ok(socket)
}

impl Client {
    /// Connects to the publisher's request socket at `host:port` (the
    /// default port is [`crate::DEFAULT_PORT`]). ZeroMQ connects lazily,
    /// so this does not wait for the publisher: the session is up once
    /// [`Client::full_tree`] succeeds (spec §5.4).
    ///
    /// `timeout` is rounded up to whole milliseconds, at least 1. Sending
    /// and receiving each wait up to it, so a request can take twice it.
    pub fn connect(host: &str, port: u16, timeout: Duration) -> Result<Client, ClientError> {
        let endpoint = format!("tcp://{host}:{port}");
        let context = zmq::Context::new();
        let socket = request_socket(&context, &endpoint, timeout)?;
        Ok(Client {
            context,
            socket,
            endpoint,
            timeout,
            next_id: 1,
            tree_id: None,
        })
    }

    /// Sends one request and returns its reply's payload frames, checked
    /// against spec §5.2, and the reply's `tree_id`. Only a FULLTREE
    /// (`fresh`) goes out before a FULLTREE has succeeded, and only it
    /// may carry a new `tree_id`; it is the caller's to adopt.
    fn request(
        &mut self,
        kind: RequestType,
        payload: Option<&[u8]>,
        fresh: bool,
    ) -> Result<(Vec<Vec<u8>>, [u8; 16]), ClientError> {
        if self.tree_id.is_none() && !fresh {
            return Err(ClientError::NotStarted);
        }
        let header = RequestHeader {
            kind,
            unique_id: self.next_id,
        };
        self.next_id = self.next_id.wrapping_add(1);
        let encoded = header.encode();
        let mut frames: Vec<&[u8]> = vec![&encoded];
        if let Some(p) = payload {
            frames.push(p);
        }
        let sent = self.socket.send_multipart(frames, 0);
        let received = sent.and_then(|()| self.socket.recv_multipart(0));
        let reply = match received {
            Ok(reply) => reply,
            Err(zmq::Error::EAGAIN) => {
                self.socket = request_socket(&self.context, &self.endpoint, self.timeout)?;
                return Err(ClientError::Timeout(self.timeout));
            }
            Err(e) => {
                self.socket = request_socket(&self.context, &self.endpoint, self.timeout)?;
                return Err(socket_error(e));
            }
        };
        let checked = check_reply(&header, &reply)?;
        if !fresh && self.tree_id != Some(checked.tree_id) {
            return Err(ClientError::Restarted);
        }
        Ok((checked.payload.to_vec(), checked.tree_id))
    }

    /// FULLTREE: the tree XML in the runtime form, with each node's
    /// `_uid`. A FULLTREE that succeeds starts the session, and accepts
    /// the publisher's current identity after a [`ClientError::Restarted`].
    /// [`FullTree::restarted`] says whether that identity is new: a
    /// recording started before it is then lost, since a restarted
    /// publisher is not recording.
    pub fn full_tree(&mut self) -> Result<FullTree, ClientError> {
        let (frames, tree_id) = self.request(RequestType::FullTree, None, true)?;
        let xml = one(frames)?;
        let restarted = self.tree_id.is_some_and(|known| known != tree_id);
        self.tree_id = Some(tree_id);
        Ok(FullTree { xml, restarted })
    }

    /// STATUS: every node's status, in tree order (F14).
    pub fn status(&mut self) -> Result<StatusBuffer, ClientError> {
        let (frames, _) = self.request(RequestType::Status, None, false)?;
        Ok(decode_status(&one(frames)?)?)
    }

    /// BLACKBOARD (F15): each named subtree's blackboard dump, as one JSON
    /// object keyed by name. `None` when no name matched: the publisher
    /// then replies with a MessagePack nil (spec §5.5). JSON has no binary
    /// type and no NaN: a MessagePack binary value refuses the reply, and
    /// a NaN or infinite float reads as null.
    pub fn blackboard(&mut self, names: &[&str]) -> Result<Option<serde_json::Value>, ClientError> {
        let payload = blackboard_payload(names).map_err(|bad| {
            payload_error(if bad.is_empty() {
                "no subtree names to ask for".to_owned()
            } else {
                format!("the name {bad:?} cannot be sent")
            })
        })?;
        let (frames, _) = self.request(RequestType::Blackboard, Some(&payload), false)?;
        let bytes = one(frames)?;
        let mut reader = std::io::Cursor::new(&bytes[..]);
        let value: serde_json::Value =
            rmp_serde::decode::from_read(&mut reader).map_err(payload_error)?;
        let read = usize::try_from(reader.position()).unwrap_or(usize::MAX);
        if read != bytes.len() {
            return Err(payload_error(format!(
                "{} bytes after the MessagePack value",
                bytes.len() - read
            )));
        }
        Ok(match value {
            serde_json::Value::Null => None,
            v => Some(v),
        })
    }

    /// Starts recording; returns the publisher's epoch time in
    /// microseconds, the recording's T0 (spec §5.6). Clears the
    /// publisher's transition buffer.
    pub fn start_recording(&mut self) -> Result<i64, ClientError> {
        let (frames, _) = self.request(
            RequestType::ToggleRecording,
            Some(recording_payload(true)),
            false,
        )?;
        Ok(decode_recording_start(&one(frames)?)?)
    }

    /// Stops recording. The records since the last drain stay in the
    /// publisher's buffer: drain once more before writing the file.
    pub fn stop_recording(&mut self) -> Result<(), ClientError> {
        let (frames, _) = self.request(
            RequestType::ToggleRecording,
            Some(recording_payload(false)),
            false,
        )?;
        if frames.is_empty() {
            Ok(())
        } else {
            Err(ReplyError::FrameCount {
                expected: 1,
                found: frames.len() + 1,
            }
            .into())
        }
    }

    /// GET_TRANSITIONS: every buffered record, which the read clears.
    pub fn drain(&mut self) -> Result<Drain, ClientError> {
        let (frames, _) = self.request(RequestType::GetTransitions, None, false)?;
        Ok(decode_drain(&one(frames)?)?)
    }
}

/// The single payload frame of a reply that carries one.
fn one(mut frames: Vec<Vec<u8>>) -> Result<Vec<u8>, ClientError> {
    if frames.len() == 1 {
        Ok(frames.remove(0))
    } else {
        Err(ReplyError::FrameCount {
            expected: 2,
            found: frames.len() + 1,
        }
        .into())
    }
}
