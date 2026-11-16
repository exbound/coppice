use std::process::Command;

fn coppice(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_coppice"))
        .args(args)
        .output()
        .expect("the coppice binary runs")
}

#[test]
fn version_prints_the_package_version() {
    let out = coppice(&["--version"]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout.clone()).unwrap(),
        format!("coppice {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn unknown_arguments_are_a_usage_error() {
    let out = coppice(&["no-such-verb"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8(out.stderr).unwrap().starts_with("usage:"));
}

#[test]
fn short_version_flag_matches_long() {
    let short = coppice(&["-V"]);
    assert!(short.status.success());
    assert_eq!(short.stdout, coppice(&["--version"]).stdout);
}

#[test]
fn no_arguments_and_extra_arguments_are_usage_errors() {
    for args in [&[][..], &["--version", "extra"][..]] {
        let out = coppice(args);
        assert_eq!(out.status.code(), Some(2), "args: {args:?}");
        assert!(out.stdout.is_empty(), "args: {args:?}");
        assert!(
            String::from_utf8(out.stderr).unwrap().starts_with("usage:"),
            "args: {args:?}"
        );
    }
}

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(rel)
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&Path]) -> std::process::Output {
    let args: Vec<&str> = args.iter().map(|p| p.to_str().unwrap()).collect();
    coppice(&args)
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

#[test]
fn check_is_silent_on_a_clean_file() {
    let out = run(&[
        Path::new("check"),
        &fixture("reference/o-answers/out-o5/in-o5-single.xml"),
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
}

#[test]
fn check_reports_each_diagnostic_with_its_position() {
    let path = fixture("doc/no-format.xml");
    let out = run(&[Path::new("check"), &path]);
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(
        err.starts_with(&format!("{}:1:1: MissingFormat: ", path.display())),
        "{err}"
    );
    assert!(err.ends_with("coppice: 1 diagnostic\n"), "{err}");
}

#[test]
fn check_follows_includes_and_names_the_included_file() {
    let out = run(&[Path::new("check"), &fixture("project/app/main.btproj")]);
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.contains("app/trees/a.xml:7:14: UnknownPort: "), "{err}");
}

#[test]
fn check_of_a_missing_file_fails() {
    let out = run(&[Path::new("check"), Path::new("/nonexistent/x.xml")]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).starts_with("coppice: "));
}

#[test]
fn lossless_save_as_is_byte_identical_and_never_overwrites() {
    let dir = scratch("cli-save-as");
    let input = fixture("doc/awkward-v4.xml");
    let output = dir.join("out.xml");
    let out = run(&[Path::new("save-as"), &input, &output]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(fs::read(&output).unwrap(), fs::read(&input).unwrap());

    fs::write(&output, b"keep").unwrap();
    let out = run(&[Path::new("save-as"), &input, &output]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("exists; not overwritten"));
    assert_eq!(fs::read(&output).unwrap(), b"keep");
    let names: Vec<_> = fs::read_dir(&dir).unwrap().collect();
    assert_eq!(names.len(), 1, "no temporary file is left behind");
}

#[test]
fn reference_save_as_writes_what_the_reference_wrote() {
    let dir = scratch("cli-save-as-ref");
    let output = dir.join("out.xml");
    let out = run(&[
        Path::new("save-as"),
        &fixture("reference/o-answers/in-o5-single.xml"),
        &output,
        Path::new("--reference"),
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        fs::read(&output).unwrap(),
        fs::read(fixture("reference/o-answers/out-o5/in-o5-single.xml")).unwrap()
    );
}

#[test]
fn reference_save_in_a_project_writes_what_the_reference_wrote() {
    let dir = scratch("cli-save-project");
    for name in ["proj.btproj", "tree.xml", "unused.xml"] {
        fs::copy(
            fixture(&format!("reference/o-answers-2/in-o14-models/{name}")),
            dir.join(name),
        )
        .unwrap();
    }
    let out = run(&[
        Path::new("save"),
        &dir.join("tree.xml"),
        Path::new("--reference"),
        Path::new("--project"),
        &dir.join("proj.btproj"),
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        fs::read(dir.join("tree.xml")).unwrap(),
        fs::read(fixture("reference/o-answers-2/out-o14/tree.xml")).unwrap()
    );
    // A file outside the project is refused.
    fs::copy(dir.join("unused.xml"), dir.join("other.xml")).unwrap();
    let out = run(&[
        Path::new("save"),
        &dir.join("other.xml"),
        Path::new("--reference"),
        Path::new("--project"),
        &dir.join("proj.btproj"),
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("is not part of the project"));
}

#[test]
fn a_refused_save_leaves_the_file_alone() {
    let dir = scratch("cli-save-refused");
    let file = dir.join("t.xml");
    fs::copy(fixture("doc/no-format.xml"), &file).unwrap();
    let before = fs::read(&file).unwrap();
    let out = run(&[Path::new("save"), &file, Path::new("--reference")]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("not saved"), "{}", stderr(&out));
    assert_eq!(fs::read(&file).unwrap(), before);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
}

#[test]
fn save_option_errors_are_usage_errors() {
    let file = fixture("doc/no-format.xml");
    for args in [
        vec!["save", file.to_str().unwrap()],
        vec!["save", file.to_str().unwrap(), "--v3"],
        vec!["save-as", file.to_str().unwrap(), "/x", "--v3"],
        vec![
            "save-as",
            file.to_str().unwrap(),
            "/x",
            "--reference",
            "--target",
        ],
        vec!["save-as", file.to_str().unwrap(), "/x", "--project", "p"],
        vec!["save", file.to_str().unwrap(), "--reference", "--bogus"],
    ] {
        let out = coppice(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(stderr(&out).contains("usage:"), "{args:?}");
    }
}

fn btlog(records: &[(u64, u16, u8)], trailing: &[u8]) -> Vec<u8> {
    let xml = b"<root><BehaviorTree ID=\"M\"><Sequence _uid=\"1\"/></BehaviorTree></root>";
    let mut b = b"BTCPP4-FileLogger2\x01".to_vec();
    b.extend_from_slice(&(xml.len() as i32).to_le_bytes());
    b.extend_from_slice(xml);
    b.extend_from_slice(&1000i64.to_le_bytes());
    for &(offset, uid, status) in records {
        b.extend_from_slice(&offset.to_le_bytes()[..6]);
        b.extend_from_slice(&uid.to_le_bytes());
        b.push(status);
    }
    b.extend_from_slice(trailing);
    b
}

#[test]
fn log_dump_prints_the_header_and_each_record() {
    let dir = scratch("cli-log");
    let file = dir.join("run.btlog");
    fs::write(&file, btlog(&[(5, 1, 1), (9, 1, 2), (12, 7, 0)], b"")).unwrap();
    let out = run(&[Path::new("log"), Path::new("dump"), &file]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(out.stdout.clone()).unwrap(),
        "xml_bytes\t69\nt0_us\t1000\nrecords\t3\n\
         # offset_us\tabsolute_us\tuid\tprevious\tstatus\n\
         5\t1005\t1\tIDLE\tRUNNING\n\
         9\t1009\t1\tRUNNING\tSUCCESS\n\
         12\t1012\t7\tIDLE\tIDLE\n"
    );
    // uid 7 is not in the XML.
    let err = stderr(&out);
    assert!(err.contains("UnknownUid { uid: 7 }"), "{err}");
}

#[test]
fn log_dump_prints_what_it_read_before_an_error() {
    let dir = scratch("cli-log-bad");
    let file = dir.join("run.btlog");
    fs::write(&file, btlog(&[(5, 1, 1)], b"\x01\x02")).unwrap();
    let out = run(&[Path::new("log"), Path::new("dump"), &file]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout.clone()).unwrap();
    assert!(text.contains("records\t1\n") && text.ends_with("5\t1005\t1\tIDLE\tRUNNING\n"));
    assert!(stderr(&out).contains("trailing bytes"), "{}", stderr(&out));

    fs::write(&file, b"not a log at all").unwrap();
    let out = run(&[Path::new("log"), Path::new("dump"), &file]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}
