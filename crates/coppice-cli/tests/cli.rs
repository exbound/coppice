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
        String::from_utf8(out.stdout).unwrap(),
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
