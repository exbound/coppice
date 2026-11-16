//! F11 and F12 against BehaviorTree.CPP 4.9.0's own `FileLogger2` (spec §8).
//!
//! The files come from `tests/harness/`, built and run by `just oracle`,
//! which points `COPPICE_BTLOG_ORACLE` at its output. These tests are
//! ignored by a plain `cargo test`, and fail rather than pass when run
//! without that directory.

use std::fs;
use std::path::PathBuf;

use coppice_btlog::{Log, Status, Transition};

/// Every scenario the harness writes. The directory must hold exactly these.
const SCENARIOS: [&str; 9] = [
    "quiet", "sequence", "fallback", "halted", "running", "skipped", "subtree", "repeat", "wide",
];

struct Case {
    name: &'static str,
    btlog: Vec<u8>,
    xml: Vec<u8>,
    /// Absolute nanoseconds, uid and status, from the harness's own logger.
    rows: Vec<(i64, u16, u8)>,
}

fn oracle_dir() -> PathBuf {
    let dir = std::env::var_os("COPPICE_BTLOG_ORACLE")
        .expect("COPPICE_BTLOG_ORACLE is not set: run `just oracle`");
    PathBuf::from(dir)
}

fn cases() -> Vec<Case> {
    let dir = oracle_dir();
    let mut found: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    let mut expected: Vec<String> = SCENARIOS
        .iter()
        .flat_map(|n| ["btlog", "transitions", "xml"].map(|ext| format!("{n}.{ext}")))
        .collect();
    expected.sort();
    assert_eq!(found, expected, "the harness output in {}", dir.display());
    SCENARIOS
        .iter()
        .map(|&name| {
            let read = |ext: &str| fs::read(dir.join(format!("{name}.{ext}"))).unwrap();
            let rows = String::from_utf8(read("transitions"))
                .unwrap()
                .lines()
                .map(|line| {
                    let f: Vec<&str> = line.split(' ').collect();
                    assert_eq!(f.len(), 3, "{name}: {line:?}");
                    (
                        f[0].parse().unwrap(),
                        f[1].parse().unwrap(),
                        f[2].parse().unwrap(),
                    )
                })
                .collect();
            Case {
                name,
                btlog: read("btlog"),
                xml: read("xml"),
                rows,
            }
        })
        .collect()
}

/// The fields of the file, sliced at the offsets spec §4 gives, without the
/// reader under test: T0, and each record's offset.
fn slice(case: &Case) -> (i64, Vec<u64>) {
    let b = &case.btlog;
    let n = i32::from_le_bytes(b[19..23].try_into().unwrap()) as usize;
    assert_eq!(&b[23..23 + n], case.xml, "{}: the embedded XML", case.name);
    let t0 = i64::from_le_bytes(b[23 + n..31 + n].try_into().unwrap());
    let body = &b[31 + n..];
    assert_eq!(body.len() % 9, 0, "{}", case.name);
    assert_eq!(body.len() / 9, case.rows.len(), "{}: records", case.name);
    let offsets = body
        .chunks(9)
        .map(|r| {
            let mut o = [0; 8];
            o[..6].copy_from_slice(&r[..6]);
            u64::from_le_bytes(o)
        })
        .collect();
    (t0, offsets)
}

/// The logger stores `T0 = floor(t0_ns / 1000)` and each offset as
/// `floor((t_ns - t0_ns) / 1000)` (BT 4.9.0
/// `src/loggers/bt_file_logger_v2.cpp:20-23,78-81,107`), so an offset is not
/// always `floor(t_ns / 1000) - T0`. With `t0_ns = 1000·T0 + r`, every
/// record must agree on one remainder `r` in 0..=999.
fn check_offsets(case: &Case, t0: i64, offsets: &[u64]) {
    let (mut lo, mut hi) = (0i64, 999i64);
    for (&offset, &(ns, _, _)) in offsets.iter().zip(&case.rows) {
        let d = ns - t0 * 1000;
        let o = offset as i64;
        lo = lo.max(d - o * 1000 - 999);
        hi = hi.min(d - o * 1000);
    }
    assert!(
        lo <= hi,
        "{}: no single sub-microsecond T0 remainder fits every offset",
        case.name
    );
}

fn hex_diff(name: &str, got: &[u8], want: &[u8]) {
    if got == want {
        return;
    }
    let at = got
        .iter()
        .zip(want)
        .position(|(a, b)| a != b)
        .unwrap_or(got.len().min(want.len()));
    let window = |b: &[u8]| {
        let start = at.saturating_sub(8);
        b[start..(at + 8).min(b.len())]
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    panic!(
        "{name}: first difference at byte {at} (lengths {} and {})\n got:  {}\n want: {}",
        got.len(),
        want.len(),
        window(got),
        window(want)
    );
}

#[test]
#[ignore = "needs the BehaviorTree.CPP harness output: run `just oracle`"]
fn f11_reads_what_the_library_wrote() {
    for case in cases() {
        let log = Log::from_bytes(&case.btlog).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        assert_eq!(log.xml, case.xml, "{}", case.name);
        let (t0, offsets) = slice(&case);
        assert_eq!(log.t0_us, t0, "{}", case.name);
        let got: Vec<_> = log
            .transitions
            .iter()
            .map(|t| (t.uid, t.status.byte()))
            .collect();
        let want: Vec<_> = case.rows.iter().map(|&(_, u, s)| (u, s)).collect();
        assert_eq!(got, want, "{}", case.name);
        let read: Vec<_> = log.transitions.iter().map(|t| t.offset_us).collect();
        assert_eq!(read, offsets, "{}", case.name);
        check_offsets(&case, log.t0_us, &read);
        assert!(
            log.diagnostics().is_empty(),
            "{}: {:?}",
            case.name,
            log.diagnostics()
        );
    }
}

#[test]
#[ignore = "needs the BehaviorTree.CPP harness output: run `just oracle`"]
fn f12_writes_what_the_library_wrote() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("btlog-oracle-write");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for case in cases() {
        // From scratch: the harness's XML, uids and statuses, with T0 and
        // the offsets sliced from the library's file, not decoded by the
        // reader under test.
        let (t0, offsets) = slice(&case);
        check_offsets(&case, t0, &offsets);
        let log = Log {
            xml: case.xml.clone(),
            t0_us: t0,
            transitions: case
                .rows
                .iter()
                .zip(&offsets)
                .map(|(&(_, uid, status), &offset_us)| Transition {
                    offset_us,
                    uid,
                    status: Status::from_byte(status).unwrap(),
                })
                .collect(),
        };
        hex_diff(case.name, &log.to_bytes().unwrap(), &case.btlog);
        let path = dir.join(format!("{}.btlog", case.name));
        log.write_file(&path).unwrap();
        hex_diff(case.name, &fs::read(&path).unwrap(), &case.btlog);
        assert_eq!(Log::from_bytes(&case.btlog).unwrap(), log, "{}", case.name);
    }
}

/// Confirms the scenarios cover what spec §8 asks of them.
#[test]
#[ignore = "needs the BehaviorTree.CPP harness output: run `just oracle`"]
fn the_scenarios_cover_the_format() {
    let cases = cases();
    let all: Vec<(u16, u8)> = cases
        .iter()
        .flat_map(|c| c.rows.iter().map(|&(_, u, s)| (u, s)))
        .collect();
    for status in 0..=3 {
        assert!(
            all.iter().any(|&(_, s)| s == status),
            "status {status} never logged"
        );
    }
    // A skipped node keeps its IDLE status and logs nothing (BT 4.9.0
    // `src/tree_node.cpp:151-155`), so SKIPPED never reaches a log from
    // ticking. The byte is covered by the synthetic tests instead.
    assert!(all.iter().all(|&(_, s)| s != 4), "SKIPPED was logged");
    assert!(all.iter().any(|&(u, _)| u > 255), "no uid above 255");
    assert!(cases.iter().any(|c| c.rows.is_empty()), "no empty log");
    let log = |name: &str| {
        let c = cases.iter().find(|c| c.name == name).unwrap();
        Log::from_bytes(&c.btlog).unwrap()
    };
    // RUNNING halted to IDLE, and IDLE after SUCCESS and FAILURE.
    let after = |name: &str, before: Status| {
        log(name)
            .with_previous()
            .any(|(t, prev)| t.status == Status::Idle && prev == before)
    };
    assert!(after("halted", Status::Running));
    assert!(after("sequence", Status::Success));
    assert!(after("fallback", Status::Failure));
    // The skipped node (uid 2 in `skipped`) never appears.
    let skipped = cases.iter().find(|c| c.name == "skipped").unwrap();
    assert!(!skipped.rows.is_empty());
    assert!(skipped.rows.iter().all(|&(_, u, _)| u != 2));
}
