//! F11 and F12 on synthetic buffers (spec §4). The buffers are assembled
//! here byte by byte, not by the writer under test.

use std::fs;
use std::path::PathBuf;

use coppice_btlog::{
    DiagnosticKind, Log, MAGIC, OFFSET_MASK, ReadError, Status, Transition, WriteError,
};

const XML: &[u8] = b"<root BTCPP_format=\"4\"><BehaviorTree ID=\"Main\" _fullpath=\"\">\
<Sequence _uid=\"1\"><AlwaysSuccess _uid=\"2\"/></Sequence></BehaviorTree></root>";

/// A log file, built by hand from the layout in spec §4.
fn file(xml: &[u8], t0: i64, records: &[(u64, u16, u8)]) -> Vec<u8> {
    let mut b = b"BTCPP4-FileLogger2".to_vec();
    b.push(1);
    b.extend_from_slice(&(xml.len() as i32).to_le_bytes());
    b.extend_from_slice(xml);
    b.extend_from_slice(&t0.to_le_bytes());
    for &(offset, uid, status) in records {
        b.extend_from_slice(&offset.to_le_bytes()[..6]);
        b.extend_from_slice(&uid.to_le_bytes());
        b.push(status);
    }
    b
}

fn first_record() -> usize {
    23 + XML.len() + 8
}

const T0: i64 = 1_794_812_542_674_028;

fn records() -> Vec<(u64, u16, u8)> {
    vec![
        (0, 1, 1),
        (3, 2, 2),
        (OFFSET_MASK, 2, 0),
        (0x0102_0304_0506, 1, 3),
        (7, 0x0201, 4),
    ]
}

#[test]
fn status_bytes_are_the_plain_enum() {
    let all = [
        (0, Status::Idle),
        (1, Status::Running),
        (2, Status::Success),
        (3, Status::Failure),
        (4, Status::Skipped),
    ];
    for (byte, status) in all {
        assert_eq!(Status::from_byte(byte), Some(status));
        assert_eq!(status.byte(), byte);
    }
    for byte in 5..=u8::MAX {
        assert_eq!(Status::from_byte(byte), None, "{byte}");
    }
}

#[test]
fn reads_every_field() {
    let log = Log::from_bytes(&file(XML, T0, &records())).unwrap();
    assert_eq!(log.xml, XML);
    assert_eq!(log.t0_us, T0);
    let expected = [
        (0, 1, Status::Running),
        (3, 2, Status::Success),
        (OFFSET_MASK, 2, Status::Idle),
        (0x0102_0304_0506, 1, Status::Failure),
        (7, 0x0201, Status::Skipped),
    ];
    let got: Vec<_> = log
        .transitions
        .iter()
        .map(|t| (t.offset_us, t.uid, t.status))
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn keeps_file_order() {
    let log = Log::from_bytes(&file(XML, T0, &[(9, 1, 1), (4, 2, 1), (6, 2, 2)])).unwrap();
    let offsets: Vec<_> = log.transitions.iter().map(|t| t.offset_us).collect();
    assert_eq!(offsets, [9, 4, 6]);
}

#[test]
fn a_header_with_no_records_is_a_log() {
    let log = Log::from_bytes(&file(XML, T0, &[])).unwrap();
    assert!(log.transitions.is_empty());
    let log = Log::from_bytes(&file(b"", -5, &[])).unwrap();
    assert_eq!((log.xml.len(), log.t0_us), (0, -5));
}

#[test]
fn writes_the_layout_exactly() {
    let bytes = file(XML, T0, &records());
    let log = Log::from_bytes(&bytes).unwrap();
    assert_eq!(log.to_bytes().unwrap(), bytes);
}

#[test]
fn writes_the_xml_verbatim() {
    // Not UTF-8 and not XML: written as given all the same.
    let xml = b"\xff\x00<not xml";
    let log = Log {
        xml: xml.to_vec(),
        t0_us: 1,
        transitions: Vec::new(),
    };
    assert_eq!(log.to_bytes().unwrap(), file(xml, 1, &[]));
}

#[test]
fn offsets_are_cut_to_48_bits() {
    let log = Log {
        xml: XML.to_vec(),
        t0_us: T0,
        transitions: vec![Transition {
            offset_us: (0xABCD << 48) | 5,
            uid: 1,
            status: Status::Running,
        }],
    };
    assert_eq!(log.to_bytes().unwrap(), file(XML, T0, &[(5, 1, 1)]));
}

#[test]
fn write_then_read() {
    let log = Log::from_bytes(&file(XML, T0, &records())).unwrap();
    let again = Log::from_bytes(&log.to_bytes().unwrap()).unwrap();
    assert_eq!(again, log);
}

#[test]
fn an_empty_file_is_truncated() {
    let err = Log::from_bytes(b"").unwrap_err();
    assert_eq!(
        err,
        ReadError::Truncated {
            offset: 0,
            needed: 18,
            found: 0
        }
    );
}

#[test]
fn a_wrong_magic_names_the_bytes_found() {
    let mut bytes = file(XML, T0, &[]);
    bytes[5] = b'5';
    let err = Log::from_bytes(&bytes).unwrap_err();
    assert_eq!(
        err,
        ReadError::Magic {
            found: b"BTCPP5-FileLogger2".to_vec()
        }
    );
    assert_eq!(err.offset(), 0);
    // A short file that is not a log is a wrong magic, not a short log.
    assert_eq!(
        Log::from_bytes(b"BTX").unwrap_err(),
        ReadError::Magic {
            found: b"BTX".to_vec()
        }
    );
    // A short file that could be a log is truncated.
    assert_eq!(
        Log::from_bytes(&MAGIC[..5]).unwrap_err(),
        ReadError::Truncated {
            offset: 0,
            needed: 18,
            found: 5
        }
    );
}

#[test]
fn a_wrong_version_is_refused() {
    let mut bytes = file(XML, T0, &[]);
    bytes[18] = 2;
    let err = Log::from_bytes(&bytes).unwrap_err();
    assert_eq!(err, ReadError::Version { found: 2 });
    assert_eq!(err.offset(), 18);
}

#[test]
fn a_negative_xml_length_is_refused() {
    let mut bytes = file(XML, T0, &[]);
    bytes[19..23].copy_from_slice(&(-1i32).to_le_bytes());
    let err = Log::from_bytes(&bytes).unwrap_err();
    assert_eq!(err, ReadError::XmlLength { found: -1 });
    assert_eq!(err.offset(), 19);
}

#[test]
fn every_short_header_is_truncated_at_its_field() {
    let bytes = file(XML, T0, &[]);
    let t0_at = 23 + XML.len();
    for len in 18..bytes.len() {
        let (offset, needed) = match len {
            18 => (18, 1),
            19..=22 => (19, 4),
            _ if len < t0_at => (23, XML.len()),
            _ => (t0_at, 8),
        };
        assert_eq!(
            Log::from_bytes(&bytes[..len]).unwrap_err(),
            ReadError::Truncated {
                offset,
                needed,
                found: len - offset
            },
            "length {len}"
        );
    }
}

#[test]
fn a_status_outside_the_enum_is_refused_with_what_came_before() {
    for bad in [5, 10, 12, 255] {
        let mut recs = records();
        recs[2].2 = bad;
        let err = Log::from_bytes(&file(XML, T0, &recs)).unwrap_err();
        let ReadError::Status {
            offset,
            found,
            partial,
        } = &err
        else {
            panic!("{err:?}");
        };
        assert_eq!(*offset, first_record() + 2 * 9 + 8);
        assert_eq!(*found, bad);
        assert_eq!(partial.transitions.len(), 2);
        assert_eq!(partial.xml, XML);
        assert_eq!(err.partial().unwrap().transitions.len(), 2);
    }
}

#[test]
fn trailing_bytes_are_an_error_that_keeps_every_record() {
    for extra in 1..9 {
        let mut bytes = file(XML, T0, &records());
        let end = bytes.len();
        bytes.extend(std::iter::repeat_n(0xEE, extra));
        let err = Log::from_bytes(&bytes).unwrap_err();
        assert_eq!(err.offset(), end);
        let ReadError::Trailing { found, partial, .. } = &err else {
            panic!("{err:?}");
        };
        assert_eq!(found.len(), extra);
        assert_eq!(partial.transitions.len(), records().len());
        assert_eq!(
            **partial,
            Log::from_bytes(&file(XML, T0, &records())).unwrap()
        );
    }
}

#[test]
fn unknown_uids_are_diagnostics_once_each() {
    let log = Log::from_bytes(&file(
        XML,
        T0,
        &[(0, 1, 1), (1, 9, 1), (2, 2, 2), (3, 9, 2), (4, 300, 1)],
    ))
    .unwrap();
    let got: Vec<_> = log
        .diagnostics()
        .into_iter()
        .map(|d| (d.offset, d.kind))
        .collect();
    assert_eq!(
        got,
        [
            (first_record() + 9, DiagnosticKind::UnknownUid { uid: 9 }),
            (
                first_record() + 4 * 9,
                DiagnosticKind::UnknownUid { uid: 300 }
            ),
        ]
    );
}

#[test]
fn known_uids_have_no_diagnostics() {
    let log = Log::from_bytes(&file(XML, T0, &[(0, 1, 1), (1, 2, 1)])).unwrap();
    assert!(log.diagnostics().is_empty());
}

#[test]
fn an_unreadable_xml_is_one_diagnostic() {
    let log = Log::from_bytes(&file(b"<root><a></root>", T0, &[(0, 7, 1)])).unwrap();
    let d = log.diagnostics();
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].offset, 23);
    assert!(matches!(d[0].kind, DiagnosticKind::XmlUnreadable(_)));
}

#[test]
fn the_previous_status_is_tracked_per_uid() {
    let log = Log::from_bytes(&file(
        XML,
        T0,
        &[
            (0, 1, 1),
            (1, 2, 1),
            (2, 2, 2),
            (3, 1, 2),
            (4, 2, 0),
            (5, 1, 0),
        ],
    ))
    .unwrap();
    let got: Vec<_> = log
        .with_previous()
        .map(|(t, prev)| (t.uid, prev, t.status))
        .collect();
    use Status::*;
    assert_eq!(
        got,
        [
            (1, Idle, Running),
            (2, Idle, Running),
            (2, Running, Success),
            (1, Running, Success),
            (2, Success, Idle),
            (1, Success, Idle),
        ]
    );
}

#[test]
fn absolute_time_is_t0_plus_offset() {
    let log = Log::from_bytes(&file(XML, T0, &[(OFFSET_MASK, 1, 1)])).unwrap();
    assert_eq!(
        log.absolute_us(&log.transitions[0]),
        Some(T0 + OFFSET_MASK as i64)
    );
    let near_max = Log {
        t0_us: i64::MAX,
        ..log.clone()
    };
    assert_eq!(near_max.absolute_us(&log.transitions[0]), None);
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn entries(dir: &PathBuf) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn write_file_leaves_only_the_complete_file() {
    let dir = scratch("btlog-write");
    let path = dir.join("run.btlog");
    let bytes = file(XML, T0, &records());
    let log = Log::from_bytes(&bytes).unwrap();
    log.write_file(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(entries(&dir), ["run.btlog"]);

    // An existing file is replaced whole.
    let shorter = Log {
        transitions: Vec::new(),
        ..log
    };
    shorter.write_file(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), file(XML, T0, &[]));
    assert_eq!(entries(&dir), ["run.btlog"]);
}

#[test]
fn a_failed_write_leaves_nothing() {
    let dir = scratch("btlog-write-fail");
    let log = Log::from_bytes(&file(XML, T0, &[])).unwrap();
    let err = log.write_file(&dir.join("missing").join("run.btlog"));
    assert!(matches!(err, Err(WriteError::Io(_))), "{err:?}");
    assert!(entries(&dir).is_empty());

    // The target is a directory: the rename fails after the temporary file
    // was written, and the temporary file is removed.
    fs::create_dir(dir.join("taken.btlog")).unwrap();
    let err = log.write_file(&dir.join("taken.btlog"));
    assert!(matches!(err, Err(WriteError::Io(_))), "{err:?}");
    assert_eq!(entries(&dir), ["taken.btlog"]);
    assert!(
        fs::read_dir(dir.join("taken.btlog"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn a_path_without_a_file_name_is_refused() {
    let log = Log::from_bytes(&file(XML, T0, &[])).unwrap();
    assert!(matches!(
        log.write_file(std::path::Path::new("/")),
        Err(WriteError::Io(_))
    ));
}
