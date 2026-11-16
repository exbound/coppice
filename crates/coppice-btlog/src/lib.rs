//! Behavior-tree recordings: the `.btlog` format (F11 reading, F12 writing),
//! as `docs/engineering-spec.md` §4 lays it out.
//!
//! ```text
//! offset    size  field
//! 0         18    ASCII "BTCPP4-FileLogger2", no NUL
//! 18        1     uint8 protocol version = 1
//! 19        4     int32 N, length of the XML
//! 23        N     UTF-8 tree XML
//! 23+N      8     int64 T0, microseconds since the Unix epoch
//! 31+N      9·k   k transition records, to end of file
//! ```
//!
//! A record is the low 48 bits of its offset from T0 in microseconds, the
//! node's `uint16` uid, and a `uint8` status. Everything is little-endian.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use coppice_doc::{Document, Node, ParseError};

/// The 18 bytes every log starts with.
pub const MAGIC: &[u8; 18] = b"BTCPP4-FileLogger2";
/// The only protocol version.
pub const VERSION: u8 = 1;
/// The size of one transition record.
pub const RECORD_LEN: usize = 9;
/// The bits of a record's time offset that the file keeps.
pub const OFFSET_MASK: u64 = (1 << 48) - 1;

/// Where the XML starts: after the magic, the version and the length.
pub const XML_OFFSET: usize = 23;

/// A node status, as a record stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// 0
    Idle,
    /// 1
    Running,
    /// 2
    Success,
    /// 3
    Failure,
    /// 4
    Skipped,
}

impl Status {
    /// The status a record byte stands for; `None` outside 0–4.
    pub fn from_byte(byte: u8) -> Option<Status> {
        match byte {
            0 => Some(Status::Idle),
            1 => Some(Status::Running),
            2 => Some(Status::Success),
            3 => Some(Status::Failure),
            4 => Some(Status::Skipped),
            _ => None,
        }
    }

    /// The record byte for this status.
    pub fn byte(self) -> u8 {
        match self {
            Status::Idle => 0,
            Status::Running => 1,
            Status::Success => 2,
            Status::Failure => 3,
            Status::Skipped => 4,
        }
    }
}

/// One record: a node changed to `status` at `offset_us` after T0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    /// Microseconds after [`Log::t0_us`]. A read value fits in 48 bits; on
    /// writing, only the low 48 bits are kept.
    pub offset_us: u64,
    /// The node's `_uid` in the XML.
    pub uid: u16,
    /// The status the node changed to. A record carries only the new status;
    /// see [`Log::with_previous`] for the one before it.
    pub status: Status,
}

/// A whole recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Log {
    /// The tree XML, kept byte for byte as found and written as given.
    pub xml: Vec<u8>,
    /// The start time, in microseconds since the Unix epoch.
    pub t0_us: i64,
    /// The records, in file order. The file is nearly, but not strictly,
    /// in time order, and the order is kept.
    pub transitions: Vec<Transition>,
}

impl Log {
    /// Reads a log from its bytes.
    ///
    /// The magic and version must match exactly, and every status byte must
    /// be 0–4. Bytes after the last whole record are an error that still
    /// carries every record read before them.
    pub fn from_bytes(bytes: &[u8]) -> Result<Log, ReadError> {
        let magic_len = bytes.len().min(MAGIC.len());
        if bytes[..magic_len] != MAGIC[..magic_len] {
            return Err(ReadError::Magic {
                found: bytes[..magic_len].to_vec(),
            });
        }
        take(bytes, 0, MAGIC.len())?;
        let version = take(bytes, 18, 1)?[0];
        if version != VERSION {
            return Err(ReadError::Version { found: version });
        }
        let mut length = [0; 4];
        length.copy_from_slice(take(bytes, 19, 4)?);
        let length = i32::from_le_bytes(length);
        let Ok(length) = usize::try_from(length) else {
            return Err(ReadError::XmlLength { found: length });
        };
        let xml = take(bytes, XML_OFFSET, length)?.to_vec();
        let t0_at = XML_OFFSET + length;
        let mut t0 = [0; 8];
        t0.copy_from_slice(take(bytes, t0_at, 8)?);
        let mut log = Log {
            xml,
            t0_us: i64::from_le_bytes(t0),
            transitions: Vec::new(),
        };
        let first = t0_at + 8;
        let body = &bytes[first..];
        let (records, rest) = body.as_chunks::<RECORD_LEN>();
        for (index, record) in records.iter().enumerate() {
            let offset = first + index * RECORD_LEN;
            let Some(status) = Status::from_byte(record[8]) else {
                return Err(ReadError::Status {
                    offset: offset + 8,
                    found: record[8],
                    partial: Box::new(log),
                });
            };
            let mut time = [0; 8];
            time[..6].copy_from_slice(&record[..6]);
            log.transitions.push(Transition {
                offset_us: u64::from_le_bytes(time),
                uid: u16::from_le_bytes([record[6], record[7]]),
                status,
            });
        }
        if !rest.is_empty() {
            return Err(ReadError::Trailing {
                offset: bytes.len() - rest.len(),
                found: rest.to_vec(),
                partial: Box::new(log),
            });
        }
        Ok(log)
    }

    /// The file bytes for this log. The XML is written exactly as given,
    /// and each offset is cut to its low 48 bits.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WriteError> {
        let Ok(length) = i32::try_from(self.xml.len()) else {
            return Err(WriteError::XmlTooLong {
                length: self.xml.len(),
            });
        };
        let mut out = Vec::with_capacity(
            XML_OFFSET + self.xml.len() + 8 + self.transitions.len() * RECORD_LEN,
        );
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&self.xml);
        out.extend_from_slice(&self.t0_us.to_le_bytes());
        for t in &self.transitions {
            out.extend_from_slice(&(t.offset_us & OFFSET_MASK).to_le_bytes()[..6]);
            out.extend_from_slice(&t.uid.to_le_bytes());
            out.push(t.status.byte());
        }
        Ok(out)
    }

    /// Writes the log to `path` so that the file appears complete or not at
    /// all: the bytes go to a new temporary file in the same directory,
    /// which is then renamed over `path`. An existing file at `path` is
    /// replaced. On failure, the temporary file is removed and `path` is
    /// untouched.
    pub fn write_file(&self, path: &Path) -> Result<(), WriteError> {
        let bytes = self.to_bytes()?;
        let name = path.file_name().ok_or_else(|| {
            WriteError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the path names no file",
            ))
        })?;
        let dir = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => Path::new("."),
        };
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let (temp, mut file) = loop {
            let temp = dir.join(format!(
                ".{}.{}-{}.tmp",
                name.to_string_lossy(),
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new().write(true).create_new(true).open(&temp) {
                Ok(file) => break (temp, file),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(WriteError::Io(e)),
            }
        };
        let written = file
            .write_all(&bytes)
            .and_then(|()| file.sync_all())
            .and_then(|()| {
                drop(file);
                fs::rename(&temp, path)
            });
        if let Err(e) = written {
            let _ = fs::remove_file(&temp);
            return Err(WriteError::Io(e));
        }
        Ok(())
    }

    /// The absolute time of a record, `T0 + offset`, in microseconds since
    /// the Unix epoch; `None` if it does not fit an `i64`.
    pub fn absolute_us(&self, transition: &Transition) -> Option<i64> {
        self.t0_us
            .checked_add(i64::try_from(transition.offset_us).ok()?)
    }

    /// Each record with the status its node had before it: IDLE for a node's
    /// first record, then the status of that node's previous record. This is
    /// how "idle after success" is told apart from a plain IDLE.
    pub fn with_previous(&self) -> impl Iterator<Item = (&Transition, Status)> + '_ {
        let mut last: HashMap<u16, Status> = HashMap::new();
        self.transitions.iter().map(move |t| {
            let previous = last.insert(t.uid, t.status).unwrap_or(Status::Idle);
            (t, previous)
        })
    }

    /// The file offset of record `index`.
    pub fn record_offset(&self, index: usize) -> usize {
        XML_OFFSET + self.xml.len() + 8 + index * RECORD_LEN
    }

    /// Conditions worth reporting that do not stop the log being read:
    /// an XML that does not parse, and uids no node in the XML carries
    /// (one diagnostic per uid, at its first record).
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let document = match Document::parse(&self.xml) {
            Ok(d) => d,
            Err(e) => {
                return vec![Diagnostic {
                    offset: XML_OFFSET,
                    kind: DiagnosticKind::XmlUnreadable(e),
                }];
            }
        };
        let mut uids = HashSet::new();
        let mut stack: Vec<&Node> = document.nodes().iter().collect();
        while let Some(node) = stack.pop() {
            if let Node::Element(e) = node {
                if let Some(uid) = e.attribute("_uid").and_then(|v| v.parse::<u16>().ok()) {
                    uids.insert(uid);
                }
                stack.extend(e.children.iter());
            }
        }
        let mut reported = HashSet::new();
        let mut out = Vec::new();
        for (index, t) in self.transitions.iter().enumerate() {
            if !uids.contains(&t.uid) && reported.insert(t.uid) {
                out.push(Diagnostic {
                    offset: self.record_offset(index),
                    kind: DiagnosticKind::UnknownUid { uid: t.uid },
                });
            }
        }
        out
    }
}

fn take(bytes: &[u8], offset: usize, needed: usize) -> Result<&[u8], ReadError> {
    offset
        .checked_add(needed)
        .and_then(|end| bytes.get(offset..end))
        .ok_or(ReadError::Truncated {
            offset,
            needed,
            found: bytes.len().saturating_sub(offset),
        })
}

/// Why a log could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// The file does not start with [`MAGIC`] (offset 0). `found` is the
    /// first 18 bytes, or all of them if fewer.
    Magic {
        /// The bytes found.
        found: Vec<u8>,
    },
    /// The version byte (offset 18) is not [`VERSION`].
    Version {
        /// The byte found.
        found: u8,
    },
    /// The XML length (offset 19) is negative.
    XmlLength {
        /// The length found.
        found: i32,
    },
    /// The file ends inside the header, the XML or T0.
    Truncated {
        /// Where the missing field starts.
        offset: usize,
        /// The bytes the field needs.
        needed: usize,
        /// The bytes left from `offset`.
        found: usize,
    },
    /// A record's status byte is not 0–4.
    Status {
        /// The offset of the status byte.
        offset: usize,
        /// The byte found.
        found: u8,
        /// Everything read before that record.
        partial: Box<Log>,
    },
    /// The file does not end on a record boundary.
    Trailing {
        /// Where the incomplete record starts.
        offset: usize,
        /// Its bytes.
        found: Vec<u8>,
        /// Everything read before it, every whole record included.
        partial: Box<Log>,
    },
}

impl ReadError {
    /// The byte offset the error is about.
    pub fn offset(&self) -> usize {
        match self {
            ReadError::Magic { .. } => 0,
            ReadError::Version { .. } => 18,
            ReadError::XmlLength { .. } => 19,
            ReadError::Truncated { offset, .. }
            | ReadError::Status { offset, .. }
            | ReadError::Trailing { offset, .. } => *offset,
        }
    }

    /// What was read before the error, when the error comes after the header.
    pub fn partial(&self) -> Option<&Log> {
        match self {
            ReadError::Status { partial, .. } | ReadError::Trailing { partial, .. } => {
                Some(partial)
            }
            _ => None,
        }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Magic { found } => write!(
                f,
                "not a .btlog: byte 0 starts {:?}, expected {:?}",
                String::from_utf8_lossy(found),
                String::from_utf8_lossy(MAGIC)
            ),
            ReadError::Version { found } => {
                write!(
                    f,
                    "unsupported .btlog version {found} at byte 18, expected {VERSION}"
                )
            }
            ReadError::XmlLength { found } => {
                write!(f, "negative XML length {found} at byte 19")
            }
            ReadError::Truncated {
                offset,
                needed,
                found,
            } => write!(
                f,
                "file ends early: {needed} bytes needed at byte {offset}, {found} present"
            ),
            ReadError::Status { offset, found, .. } => {
                write!(f, "status byte {found} at byte {offset} is not 0-4")
            }
            ReadError::Trailing { offset, found, .. } => write!(
                f,
                "{} trailing bytes at byte {offset}, not a whole {RECORD_LEN}-byte record",
                found.len()
            ),
        }
    }
}

impl std::error::Error for ReadError {}

/// Why a log could not be written.
#[derive(Debug)]
pub enum WriteError {
    /// The XML is longer than the length field can hold.
    XmlTooLong {
        /// Its length in bytes.
        length: usize,
    },
    /// The file could not be written.
    Io(io::Error),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::XmlTooLong { length } => {
                write!(f, "XML of {length} bytes does not fit a .btlog")
            }
            WriteError::Io(e) => write!(f, "cannot write the .btlog: {e}"),
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WriteError::Io(e) => Some(e),
            WriteError::XmlTooLong { .. } => None,
        }
    }
}

/// A condition reported without refusing the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The byte offset it is about: the XML's start, or a record's.
    pub offset: usize,
    /// What it is.
    pub kind: DiagnosticKind,
}

/// The kinds of [`Diagnostic`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// The XML does not parse; uids are not checked.
    XmlUnreadable(ParseError),
    /// No element in the XML has this `_uid`.
    UnknownUid {
        /// The uid.
        uid: u16,
    },
}
