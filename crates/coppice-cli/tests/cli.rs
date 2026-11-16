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
    // The saved tree now repeats the project's model in its own block; the
    // next save in the same project still goes through (#12).
    let out = run(&[
        Path::new("save"),
        &dir.join("unused.xml"),
        Path::new("--reference"),
        Path::new("--project"),
        &dir.join("proj.btproj"),
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        fs::read(dir.join("unused.xml")).unwrap(),
        fs::read(fixture("reference/o-answers-2/out-o14/unused.xml")).unwrap()
    );
    let out = run(&[Path::new("check"), &dir.join("proj.btproj")]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
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
    assert!(
        err.contains(": byte 118: no node in the XML has uid 7"),
        "{err}"
    );
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

#[test]
fn a_reference_save_reports_what_it_changed() {
    let dir = scratch("cli-save-note");
    let input = fixture("reference/o-answers-2/in-o16-stamps/stamped.xml");
    let out = run(&[
        Path::new("save-as"),
        &input,
        &dir.join("out.xml"),
        Path::new("--reference"),
    ]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        stderr(&out),
        format!(
            "{}:2:1: StampRaised: BTCPP_version 4.9 is written as the target 4.10\n",
            input.display()
        )
    );
}

#[test]
fn columns_count_characters() {
    let dir = scratch("cli-column");
    let file = dir.join("t.xml");
    // "é" is two bytes; the attribute starts at character 18 of line 2,
    // byte 19.
    fs::write(
        &file,
        "<root BTCPP_format=\"4\"><BehaviorTree ID=\"T\">\n\u{e9} <AlwaysSuccess bogus=\"1\"/></BehaviorTree></root>",
    )
    .unwrap();
    let out = run(&[Path::new("check"), &file]);
    let err = stderr(&out);
    assert!(
        err.contains(&format!("{}:2:18: UnknownPort: ", file.display())),
        "{err}"
    );
}

fn copy_dir(from: &str, to: &Path) {
    for entry in fs::read_dir(fixture(from)).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
    }
}

/// Copies a fixture project, saves one tree in place with `flags`, and
/// compares it with the reference's output.
fn project_row(name: &str, from: &str, project: &str, tree: &str, want: &str, flags: &[&str]) {
    let dir = scratch(name);
    copy_dir(from, &dir);
    let mut args = vec![
        "save".to_owned(),
        dir.join(tree).to_str().unwrap().to_owned(),
        "--reference".to_owned(),
        "--project".to_owned(),
        dir.join(project).to_str().unwrap().to_owned(),
    ];
    args.extend(flags.iter().map(|f| (*f).to_owned()));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = coppice(&args);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        fs::read(dir.join(tree)).unwrap(),
        fs::read(fixture(want)).unwrap(),
        "{name}"
    );
}

#[test]
fn v3_mode_save_writes_what_the_reference_wrote() {
    project_row(
        "cli-v3",
        "reference/o-answers-2/in-o12-v3",
        "proj.btproj",
        "tree.xml",
        "reference/o-answers-2/out-o12/tree.xml",
        &["--v3"],
    );
}

#[test]
fn a_switched_target_writes_what_the_reference_wrote() {
    project_row(
        "cli-target",
        "reference/o-answers-2/in-o16-stamps",
        "proj-411.btproj",
        "t411.xml",
        "reference/o-answers-2/out-o16a/t411.xml",
        &["--target", "4.11"],
    );
}

#[test]
#[cfg(unix)]
fn a_standalone_save_replaces_the_file_atomically_and_keeps_its_mode() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let dir = scratch("cli-save-standalone");
    let file = dir.join("in-o5-single.xml");
    fs::copy(fixture("reference/o-answers/in-o5-single.xml"), &file).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    let before = fs::metadata(&file).unwrap().ino();
    let out = run(&[Path::new("save"), &file, Path::new("--reference")]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        fs::read(&file).unwrap(),
        fs::read(fixture("reference/o-answers/out-o5/in-o5-single.xml")).unwrap()
    );
    let meta = fs::metadata(&file).unwrap();
    // A new file was renamed into place, not the old one rewritten.
    assert_ne!(meta.ino(), before);
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
}

#[test]
#[cfg(unix)]
fn save_refuses_what_it_cannot_replace_safely() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = scratch("cli-save-unsafe");
    let real = dir.join("real.xml");
    let input = fs::read(fixture("reference/o-answers/in-o5-single.xml")).unwrap();
    fs::write(&real, &input).unwrap();

    let link = dir.join("link.xml");
    symlink(&real, &link).unwrap();
    let out = run(&[Path::new("save"), &link, Path::new("--reference")]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("is a symbolic link"),
        "{}",
        stderr(&out)
    );
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );

    let hard = dir.join("hard.xml");
    fs::hard_link(&real, &hard).unwrap();
    let out = run(&[Path::new("save"), &real, Path::new("--reference")]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("more than one hard link"),
        "{}",
        stderr(&out)
    );
    fs::remove_file(&hard).unwrap();

    fs::set_permissions(&real, fs::Permissions::from_mode(0o444)).unwrap();
    let out = run(&[Path::new("save"), &real, Path::new("--reference")]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("is read-only"), "{}", stderr(&out));

    assert_eq!(fs::read(&real).unwrap(), input);
    let mut names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["link.xml", "real.xml"]);
}

#[test]
fn malformed_command_lines_exit_2() {
    let file = fixture("reference/o-answers/in-o5-single.xml");
    let f = file.to_str().unwrap();
    for args in [
        vec!["save-as", f, "/x", "--reference", "--target", "--v3"],
        vec!["save-as", f, "/x", "--reference", "--target", ""],
        vec!["save-as", f, "/x", "--reference", "--reference"],
        vec!["save-as", f, "/x", "--reference", "--v3", "--v3"],
        vec![
            "save-as",
            f,
            "/x",
            "--reference",
            "--target",
            "4.9",
            "--target",
            "4.10",
        ],
        vec!["save", f, "--reference", "--project", "--v3"],
    ] {
        let out = coppice(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
    }
}

#[test]
#[cfg(unix)]
fn a_non_utf8_argument_is_a_usage_error() {
    use std::os::unix::ffi::OsStrExt;
    let out = Command::new(env!("CARGO_BIN_EXE_coppice"))
        .arg("check")
        .arg(std::ffi::OsStr::from_bytes(b"\xff.xml"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("valid UTF-8"));
}

#[test]
fn a_closed_output_ends_log_dump_quietly() {
    use std::process::Stdio;
    let dir = scratch("cli-log-pipe");
    let file = dir.join("big.btlog");
    let records: Vec<(u64, u16, u8)> = (0..50_000).map(|i| (i, 1, 1)).collect();
    fs::write(&file, btlog(&records, b"")).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_coppice"))
        .args(["log", "dump", file.to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(!stderr(&out).contains("panicked"));
}

#[test]
fn a_parse_error_has_a_line_and_column() {
    let dir = scratch("cli-parse");
    let file = dir.join("bad.xml");
    fs::write(&file, "<root BTCPP_format=\"4\">\n  <a></b>\n</root>").unwrap();
    let out = run(&[Path::new("check"), &file]);
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(
        err.starts_with(&format!("coppice: {}:2:6: ", file.display())),
        "{err}"
    );
}

#[test]
fn a_byte_order_mark_is_not_a_column() {
    let dir = scratch("cli-bom");
    let file = dir.join("t.xml");
    fs::write(
        &file,
        "\u{feff}<root BTCPP_format=\"4\"><BehaviorTree ID=\"T\"><AlwaysSuccess bogus=\"1\"/></BehaviorTree></root>",
    )
    .unwrap();
    let err = stderr(&run(&[Path::new("check"), &file]));
    // `bogus` is the 60th character after the mark.
    assert!(
        err.contains(&format!("{}:1:60: UnknownPort", file.display())),
        "{err}"
    );
}

#[test]
fn log_dump_diagnostics_are_plain_text() {
    let dir = scratch("cli-log-diag");
    let file = dir.join("run.btlog");
    let mut bytes = b"BTCPP4-FileLogger2\x01".to_vec();
    let xml = b"<root><a></b></root>";
    bytes.extend_from_slice(&(xml.len() as i32).to_le_bytes());
    bytes.extend_from_slice(xml);
    bytes.extend_from_slice(&0i64.to_le_bytes());
    fs::write(&file, &bytes).unwrap();
    let out = run(&[Path::new("log"), Path::new("dump"), &file]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        stderr(&out),
        format!(
            "{}: byte 32: the XML does not parse (end tag </b> at byte 9 does not close <a>, counting from the XML's start); uids are not checked\n",
            file.display()
        )
    );
}

#[test]
fn an_option_where_a_file_belongs_is_a_usage_error() {
    let dir = scratch("cli-flag-operand");
    let input = fixture("reference/o-answers/in-o5-single.xml");
    let out = Command::new(env!("CARGO_BIN_EXE_coppice"))
        .current_dir(&dir)
        .args(["save-as", input.to_str().unwrap(), "--reference"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("write ./--reference"),
        "{}",
        stderr(&out)
    );
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    for args in [
        vec!["check", "-x"],
        vec!["save", "-x", "--reference"],
        vec!["log", "dump", "--x"],
        vec!["save-as", "--reference", "a", "b"],
    ] {
        assert_eq!(coppice(&args).status.code(), Some(2), "{args:?}");
    }
    // A file whose name starts with `-` is still reachable.
    fs::copy(&input, dir.join("-in.xml")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_coppice"))
        .current_dir(&dir)
        .args(["check", "./-in.xml"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

#[test]
#[cfg(unix)]
fn a_file_its_user_cannot_write_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("cli-save-unwritable");
    let file = dir.join("t.xml");
    let input = fs::read(fixture("reference/o-answers/in-o5-single.xml")).unwrap();
    fs::write(&file, &input).unwrap();
    // Writable by the group, not by its owner.
    fs::set_permissions(&file, fs::Permissions::from_mode(0o464)).unwrap();
    let out = run(&[Path::new("save"), &file, Path::new("--reference")]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("is read-only"), "{}", stderr(&out));
    assert_eq!(fs::read(&file).unwrap(), input);
}

#[cfg(unix)]
fn race(name: &str, meddle: impl FnOnce(&Path)) {
    use std::io::Write as _;
    use std::process::Stdio;
    let dir = scratch(name);
    copy_dir("reference/o-answers-2/in-o14-models", &dir);
    let project = fs::read(dir.join("proj.btproj")).unwrap();
    fs::remove_file(dir.join("proj.btproj")).unwrap();
    // The project is read from a pipe, so the save waits there, after its
    // first check of the tree file.
    let made = Command::new("mkfifo")
        .arg(dir.join("proj.btproj"))
        .status()
        .unwrap();
    assert!(made.success());
    let tree = dir.join("tree.xml");
    let child = Command::new(env!("CARGO_BIN_EXE_coppice"))
        .args([
            "save",
            tree.to_str().unwrap(),
            "--reference",
            "--project",
            dir.join("proj.btproj").to_str().unwrap(),
        ])
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Opening the pipe for writing waits until the save opens it to read.
    let mut pipe = fs::OpenOptions::new()
        .write(true)
        .open(dir.join("proj.btproj"))
        .unwrap();
    meddle(&dir);
    let before = fs::read(&tree).unwrap();
    pipe.write_all(&project).unwrap();
    drop(pipe);
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    // The second check, just before the rename, sees the new link.
    assert!(stderr(&out).contains("not saved"), "{}", stderr(&out));
    assert_eq!(fs::read(&tree).unwrap(), before);
    assert_eq!(
        fs::read_dir(&dir)
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp"))
            .count(),
        0
    );
}

#[test]
#[cfg(unix)]
fn a_file_linked_during_a_save_is_not_replaced() {
    race("cli-save-race-link", |dir| {
        fs::hard_link(dir.join("tree.xml"), dir.join("backup.xml")).unwrap();
    });
}

#[test]
#[cfg(unix)]
fn a_file_swapped_during_a_save_is_not_replaced() {
    race("cli-save-race-swap", |dir| {
        // Another program saves the file the same way: a new file renamed
        // over it, with the same content.
        let content = fs::read(dir.join("tree.xml")).unwrap();
        fs::write(dir.join("other.xml"), content).unwrap();
        fs::rename(dir.join("other.xml"), dir.join("tree.xml")).unwrap();
    });
}

#[test]
fn reference_save_of_the_other_tree_writes_what_the_reference_wrote() {
    // From a fresh copy, as the reference saved it.
    project_row(
        "cli-save-project-unused",
        "reference/o-answers-2/in-o14-models",
        "proj.btproj",
        "unused.xml",
        "reference/o-answers-2/out-o14/unused.xml",
        &[],
    );
}

#[test]
#[cfg(unix)]
fn a_file_rewritten_during_a_save_is_not_replaced() {
    race("cli-save-race-rewrite", |dir| {
        // Another program writes into the same file: same inode, new content.
        let path = dir.join("tree.xml");
        let mut content = fs::read(&path).unwrap();
        content.extend_from_slice(b"<!-- edited -->");
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&path, content).unwrap();
    });
}
