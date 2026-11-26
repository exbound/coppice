//! The client against a scripted publisher: a REP socket on a thread that
//! answers each request with what the test gives it.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use coppice_btlog::Status;
use coppice_monitor::{Client, ClientError, NodeState, ReplyError};

const TREE: &[u8] = b"<root/>";

const TIMEOUT: Duration = Duration::from_millis(500);

/// What the scripted publisher does with one request.
enum Step {
    /// Replies with the request's header (protocol, tag and id echoed),
    /// this `tree_id`, and these payload frames.
    Reply([u8; 16], Vec<Vec<u8>>),
    /// Replies with these frames as they are.
    Raw(Vec<Vec<u8>>),
    /// Waits this long, then replies as `Reply`.
    Late(Duration, [u8; 16], Vec<Vec<u8>>),
}

/// Starts a publisher that serves `steps` in order, and returns the port
/// it listens on and a receiver of each request's frames.
fn publisher(steps: Vec<Step>) -> (u16, mpsc::Receiver<Vec<Vec<u8>>>) {
    let context = zmq::Context::new();
    let socket = context.socket(zmq::REP).unwrap();
    socket.set_linger(0).unwrap();
    socket.bind("tcp://127.0.0.1:*").unwrap();
    let endpoint = socket.get_last_endpoint().unwrap().unwrap();
    let port = endpoint.rsplit(':').next().unwrap().parse().unwrap();
    let (seen, requests) = mpsc::channel();
    thread::spawn(move || {
        let _context = context;
        for step in steps {
            let request = socket.recv_multipart(0).unwrap();
            let header = request[0].clone();
            let _ = seen.send(request);
            let echoed = |tree_id: [u8; 16], payload: Vec<Vec<u8>>| {
                let mut first = header[..6].to_vec();
                first.extend_from_slice(&tree_id);
                let mut frames = vec![first];
                frames.extend(payload);
                frames
            };
            let reply = match step {
                Step::Reply(tree_id, payload) => echoed(tree_id, payload),
                Step::Raw(frames) => frames,
                Step::Late(wait, tree_id, payload) => {
                    thread::sleep(wait);
                    echoed(tree_id, payload)
                }
            };
            socket.send_multipart(reply, 0).unwrap();
        }
    });
    (port, requests)
}

fn client(port: u16) -> Client {
    Client::connect("127.0.0.1", port, TIMEOUT).unwrap()
}

/// A FULLTREE reply from publisher `tree_id`, to start a session with.
fn tree(tree_id: [u8; 16]) -> Step {
    Step::Reply(tree_id, vec![TREE.to_vec()])
}

/// A client whose session is up: its first request, a FULLTREE, is answered.
fn started(port: u16) -> Client {
    let mut c = client(port);
    c.full_tree().unwrap();
    c
}

const A: [u8; 16] = [1; 16];
const B: [u8; 16] = [2; 16];

#[test]
fn full_tree_and_status_go_out_with_their_tags_and_ids() {
    let xml = b"<root><BehaviorTree ID=\"M\"/></root>".to_vec();
    let (port, requests) = publisher(vec![
        Step::Reply(A, vec![xml.clone()]),
        Step::Reply(A, vec![vec![1, 0, 2, 7, 0, 12]]),
    ]);
    let mut c = client(port);
    assert_eq!(c.full_tree().unwrap().xml, xml);
    let status = c.status().unwrap();
    assert_eq!(status.nodes.len(), 2);
    assert_eq!(status.nodes[0].uid, 1);
    assert_eq!(status.nodes[0].state, NodeState::Is(Status::Success));
    assert_eq!(status.nodes[1].state, NodeState::IdleAfter(Status::Success));
    // Protocol 2, the tag, and a fresh little-endian id per request.
    assert_eq!(requests.recv().unwrap(), [vec![2, b'T', 1, 0, 0, 0]]);
    assert_eq!(requests.recv().unwrap(), [vec![2, b'S', 2, 0, 0, 0]]);
}

#[test]
fn an_error_reply_is_reported_with_its_text() {
    let (port, _) = publisher(vec![Step::Raw(vec![
        b"error".to_vec(),
        b"no such hook".to_vec(),
    ])]);
    match client(port).full_tree() {
        Err(ClientError::Reply(ReplyError::Publisher { text })) => {
            assert_eq!(text, "no such hook");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_reply_to_another_request_is_refused() {
    let mut wrong = vec![2, b'T', 9, 0, 0, 0];
    wrong.extend_from_slice(&A);
    let (port, _) = publisher(vec![Step::Raw(vec![wrong, b"<root/>".to_vec()])]);
    match client(port).full_tree() {
        Err(ClientError::Reply(ReplyError::UniqueId { expected, found })) => {
            assert_eq!((expected, found), (1, 9));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_lost_reply_times_out_and_the_next_request_still_works() {
    // The first reply comes after the client gave up on it, on the socket
    // the client has since replaced; the second is answered in time.
    let (port, requests) = publisher(vec![
        Step::Late(TIMEOUT * 3, A, vec![b"late".to_vec()]),
        Step::Reply(A, vec![b"<root/>".to_vec()]),
    ]);
    let mut c = client(port);
    match c.full_tree() {
        Err(ClientError::Timeout(t)) => assert_eq!(t, TIMEOUT),
        other => panic!("{other:?}"),
    }
    // Let the late reply go out first: it must not reach the new socket.
    thread::sleep(TIMEOUT * 3);
    assert_eq!(c.full_tree().unwrap().xml, b"<root/>");
    let ids: Vec<u8> = (0..2).map(|_| requests.recv().unwrap()[0][2]).collect();
    assert_eq!(ids, [1, 2]);
}

#[test]
fn a_new_tree_id_reports_a_restart_until_the_tree_is_fetched_again() {
    let (port, _) = publisher(vec![
        Step::Reply(A, vec![b"<root/>".to_vec()]),
        Step::Reply(A, vec![vec![]]),
        Step::Reply(B, vec![vec![]]),
        Step::Reply(B, vec![vec![]]),
        Step::Reply(B, vec![b"<root/>".to_vec()]),
        Step::Reply(B, vec![vec![]]),
    ]);
    let mut c = client(port);
    c.full_tree().unwrap();
    c.status().unwrap();
    // Every status from the new publisher is refused, not just the first.
    assert!(matches!(c.status(), Err(ClientError::Restarted)));
    assert!(matches!(c.status(), Err(ClientError::Restarted)));
    c.full_tree().unwrap();
    c.status().unwrap();
}

#[test]
fn a_blackboard_reply_decodes_to_json_and_nil_to_none() {
    let dump = serde_json::json!({
        "MainTree": {"count": 3, "pose": {"x": 1.5, "tags": ["a", "b"]}},
        "ROOT": {"ok": true}
    });
    let (port, requests) = publisher(vec![
        tree(A),
        Step::Reply(A, vec![rmp_serde::to_vec(&dump).unwrap()]),
        Step::Reply(A, vec![rmp_serde::to_vec(&()).unwrap()]),
    ]);
    let mut c = started(port);
    assert_eq!(c.blackboard(&["MainTree", "ROOT"]).unwrap(), Some(dump));
    assert_eq!(c.blackboard(&["Nothing"]).unwrap(), None);
    requests.recv().unwrap();
    let first = requests.recv().unwrap();
    assert_eq!(first[0][1], b'B');
    assert_eq!(first[1], b"MainTree;ROOT");
}

#[test]
fn a_blackboard_request_that_cannot_be_sent_is_refused_before_sending() {
    let (port, requests) = publisher(vec![]);
    let mut c = client(port);
    assert!(matches!(c.blackboard(&[]), Err(ClientError::Payload(_))));
    assert!(matches!(
        c.blackboard(&["a;b"]),
        Err(ClientError::Payload(_))
    ));
    assert!(requests.try_recv().is_err());
}

#[test]
fn recording_starts_stops_and_drains() {
    let mut records = Vec::new();
    for (offset, uid, status) in [(5u64, 1u16, 1u8), (9, 1, 2)] {
        records.extend_from_slice(&offset.to_le_bytes()[..6]);
        records.extend_from_slice(&uid.to_le_bytes());
        records.push(status);
    }
    let (port, requests) = publisher(vec![
        tree(A),
        Step::Reply(A, vec![b"1700000000123456".to_vec()]),
        Step::Reply(A, vec![]),
        Step::Reply(A, vec![records]),
    ]);
    let mut c = started(port);
    requests.recv().unwrap();
    assert_eq!(c.start_recording().unwrap(), 1_700_000_000_123_456);
    c.stop_recording().unwrap();
    let drain = c.drain().unwrap();
    assert_eq!(drain.transitions.len(), 2);
    assert_eq!(drain.transitions[1].offset_us, 9);
    assert!(!drain.possible_gap);
    let sent: Vec<Vec<Vec<u8>>> = (0..3).map(|_| requests.recv().unwrap()).collect();
    assert_eq!(sent[0][1], b"start");
    assert_eq!(sent[1][1], b"stop");
    assert_eq!((sent[2][0][1], sent[2].len()), (b't', 1));
}

#[test]
fn a_reply_with_the_wrong_frame_count_is_refused() {
    let (port, _) = publisher(vec![
        Step::Reply(A, vec![]),
        tree(A),
        Step::Reply(A, vec![b"x".to_vec()]),
    ]);
    let mut c = client(port);
    assert!(matches!(
        c.full_tree(),
        Err(ClientError::Reply(ReplyError::FrameCount {
            expected: 2,
            found: 1
        }))
    ));
    c.full_tree().unwrap();
    assert!(matches!(
        c.stop_recording(),
        Err(ClientError::Reply(ReplyError::FrameCount {
            expected: 1,
            found: 2
        }))
    ));
}

#[test]
fn fetching_the_tree_accepts_a_restart_it_is_the_first_to_see() {
    let (port, _) = publisher(vec![
        Step::Reply(A, vec![b"<root/>".to_vec()]),
        Step::Reply(B, vec![b"<root/>".to_vec()]),
        Step::Reply(B, vec![vec![]]),
    ]);
    let mut c = client(port);
    assert!(!c.full_tree().unwrap().restarted);
    assert!(c.full_tree().unwrap().restarted);
    c.status().unwrap();
}

#[test]
fn a_restart_seen_first_by_any_other_request_is_reported() {
    let (port, _) = publisher(vec![
        tree(A),
        Step::Reply(B, vec![rmp_serde::to_vec(&()).unwrap()]),
        Step::Reply(B, vec![vec![]]),
        Step::Reply(B, vec![b"1".to_vec()]),
        Step::Reply(B, vec![]),
    ]);
    let mut c = started(port);
    assert!(matches!(c.blackboard(&["M"]), Err(ClientError::Restarted)));
    assert!(matches!(c.drain(), Err(ClientError::Restarted)));
    assert!(matches!(c.start_recording(), Err(ClientError::Restarted)));
    assert!(matches!(c.stop_recording(), Err(ClientError::Restarted)));
}

#[test]
fn a_blackboard_reply_with_bytes_after_its_value_is_refused() {
    let (port, _) = publisher(vec![tree(A), Step::Reply(A, vec![vec![0xc0, 0xff, 0xff]])]);
    match started(port).blackboard(&["M"]) {
        Err(ClientError::Payload(e)) => assert!(e.contains("2 bytes after"), "{e}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_blackboard_reply_with_binary_is_refused_and_nan_reads_as_null() {
    // {"a": bin8 [1, 2]}, then {"a": NaN, "b": +inf} as float64.
    let binary = vec![0x81, 0xa1, b'a', 0xc4, 0x02, 1, 2];
    let mut floats = vec![0x82, 0xa1, b'a', 0xcb];
    floats.extend_from_slice(&f64::NAN.to_be_bytes());
    floats.extend_from_slice(&[0xa1, b'b', 0xcb]);
    floats.extend_from_slice(&f64::INFINITY.to_be_bytes());
    let (port, _) = publisher(vec![
        tree(A),
        Step::Reply(A, vec![binary]),
        Step::Reply(A, vec![floats]),
    ]);
    let mut c = started(port);
    assert!(matches!(c.blackboard(&["M"]), Err(ClientError::Payload(_))));
    assert_eq!(
        c.blackboard(&["M"]).unwrap(),
        Some(serde_json::json!({"a": null, "b": null}))
    );
}

#[test]
fn a_timeout_under_a_millisecond_still_waits() {
    // One reply per try: a try that times out on a slow host leaves its
    // reply to the socket the client replaced.
    let (port, _) = publisher(
        (0..50)
            .map(|_| Step::Reply(A, vec![b"<root/>".to_vec()]))
            .collect(),
    );
    let mut c = Client::connect("127.0.0.1", port, Duration::from_micros(900)).unwrap();
    // Rounded up to 1 ms, not 0 ("do not wait").
    let got = (0..50).find_map(|_| c.full_tree().ok());
    assert_eq!(got.map(|t| t.xml).as_deref(), Some(TREE));
}

#[test]
fn dropping_a_client_with_an_unanswered_request_does_not_hang() {
    // Nothing listens on the port, so the request stays queued; with a
    // lingering socket, dropping the client would wait for it forever.
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let mut c = client(port);
        let _ = c.full_tree();
        drop(c);
        let _ = done.send(());
    });
    finished
        .recv_timeout(Duration::from_secs(10))
        .expect("dropping the client hung");
}

#[test]
fn nothing_but_a_full_tree_goes_out_before_one_succeeds() {
    let (port, requests) = publisher(vec![tree(A), Step::Reply(A, vec![vec![]])]);
    let mut c = client(port);
    assert!(matches!(c.status(), Err(ClientError::NotStarted)));
    assert!(matches!(c.drain(), Err(ClientError::NotStarted)));
    assert!(matches!(c.start_recording(), Err(ClientError::NotStarted)));
    assert!(matches!(c.blackboard(&["M"]), Err(ClientError::NotStarted)));
    assert!(requests.try_recv().is_err());
    c.full_tree().unwrap();
    c.status().unwrap();
}

#[test]
fn a_malformed_full_tree_from_a_restarted_publisher_does_not_end_the_restart() {
    let (port, _) = publisher(vec![
        tree(A),
        Step::Reply(B, vec![]),
        Step::Reply(B, vec![vec![]]),
        tree(B),
        Step::Reply(B, vec![vec![]]),
    ]);
    let mut c = started(port);
    assert!(matches!(c.full_tree(), Err(ClientError::Reply(_))));
    assert!(matches!(c.status(), Err(ClientError::Restarted)));
    assert!(c.full_tree().unwrap().restarted);
    c.status().unwrap();
}
