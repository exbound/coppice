//! The `coppice` command-line tool (design decision D10). Each verb exposes
//! one feature of the library crates.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use coppice_btlog::{Log, Status};
use coppice_doc::{
    Diagnostic, Document, Project, ReferenceOptions, TreeDocument, WriteMode, write_tree,
};

const USAGE: &str = "usage: coppice --version
       coppice check <file>
       coppice save-as <input> <output> [--reference [--v3] [--target <version>]]
       coppice save <tree-file> --reference [--project <project-file>] [--v3] [--target <version>]
       coppice log dump <file.btlog>";

/// How a run ended: 0 done, 1 the input has problems or was refused,
/// 2 the command line is wrong.
enum Failure {
    Refused(String),
    Usage(String),
}

type Outcome = Result<(), Failure>;

fn refused(message: impl Into<String>) -> Failure {
    Failure::Refused(message.into())
}

fn usage(message: impl Into<String>) -> Failure {
    Failure::Usage(message.into())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let outcome = match args.as_slice() {
        ["--version" | "-V"] => {
            println!("coppice {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        ["check", file] => check(Path::new(file)),
        ["save-as", input, output, rest @ ..] => save_as(Path::new(input), Path::new(output), rest),
        ["save", file, rest @ ..] => save(Path::new(file), rest),
        ["log", "dump", file] => log_dump(Path::new(file)),
        _ => Err(usage("")),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Refused(message)) => {
            if !message.is_empty() {
                eprintln!("coppice: {message}");
            }
            ExitCode::from(1)
        }
        Err(Failure::Usage(message)) => {
            if !message.is_empty() {
                eprintln!("coppice: {message}");
            }
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// `<path>:<line>:<column>: <kind>: <message>`, with a 1-based line and a
/// 1-based column counted in characters.
fn report(path: &Path, source: &str, d: &Diagnostic) {
    let before = &source[..d.offset.min(source.len())];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    eprintln!(
        "{}:{line}:{column}: {:?}: {}",
        path.display(),
        d.kind,
        d.message
    );
}

/// F10: load a file, with its includes, and report every diagnostic.
fn check(path: &Path) -> Outcome {
    let project = Project::load(path).map_err(|e| refused(e.to_string()))?;
    let diagnostics = project.diagnostics();
    let documents = project.documents();
    for pd in &diagnostics {
        let d = &documents[pd.document];
        report(&d.path, d.tree.document().source(), &pd.diagnostic);
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(refused(format!(
            "{} diagnostic{}",
            diagnostics.len(),
            if diagnostics.len() == 1 { "" } else { "s" }
        )))
    }
}

struct Options {
    reference: bool,
    project: Option<PathBuf>,
    reference_options: ReferenceOptions,
}

fn options(rest: &[&str], project_allowed: bool) -> Result<Options, Failure> {
    let mut o = Options {
        reference: false,
        project: None,
        reference_options: ReferenceOptions::default(),
    };
    let mut writer_flag = false;
    let mut i = 0;
    while i < rest.len() {
        match rest[i] {
            "--reference" => o.reference = true,
            "--v3" => {
                o.reference_options.mode = WriteMode::V3;
                writer_flag = true;
            }
            "--target" => {
                i += 1;
                let Some(v) = rest.get(i) else {
                    return Err(usage("--target needs a version"));
                };
                o.reference_options.target = (*v).to_owned();
                writer_flag = true;
            }
            "--project" if project_allowed => {
                i += 1;
                let Some(v) = rest.get(i) else {
                    return Err(usage("--project needs a project file"));
                };
                o.project = Some(PathBuf::from(v));
            }
            other => return Err(usage(format!("unknown option {other}"))),
        }
        i += 1;
    }
    if writer_flag && !o.reference {
        return Err(usage("--v3 and --target apply only with --reference"));
    }
    Ok(o)
}

fn read_tree(path: &Path) -> Result<TreeDocument, Failure> {
    let bytes = fs::read(path).map_err(|e| refused(format!("{}: {e}", path.display())))?;
    let doc = Document::parse(&bytes).map_err(|e| refused(format!("{}: {e}", path.display())))?;
    TreeDocument::new(doc).map_err(|e| refused(format!("{}: {e}", path.display())))
}

/// F5 (lossless) or F9 (reference) save of a standalone file to a new path.
fn save_as(input: &Path, output: &Path, rest: &[&str]) -> Outcome {
    let o = options(rest, false)?;
    let tree = read_tree(input)?;
    let text = if o.reference {
        let models = tree.models();
        match write_tree(&tree, &models, &o.reference_options) {
            Ok(written) => {
                for note in &written.notes {
                    report(input, tree.document().source(), note);
                }
                written.text
            }
            Err(diagnostics) => {
                for d in &diagnostics {
                    report(input, tree.document().source(), d);
                }
                return Err(refused(format!(
                    "{}: not saved; the reference writer refused it",
                    input.display()
                )));
            }
        }
    } else {
        tree.document().render()
    };
    write_new(output, text.as_bytes())
}

/// F6/F7 (reference save of a tree in a project) or F9 (standalone),
/// replacing the file.
fn save(file: &Path, rest: &[&str]) -> Outcome {
    let o = options(rest, true)?;
    if !o.reference {
        return Err(usage(
            "save needs --reference: a lossless save of an unedited file changes nothing",
        ));
    }
    let Some(project_path) = &o.project else {
        let tree = read_tree(file)?;
        let models = tree.models();
        return match write_tree(&tree, &models, &o.reference_options) {
            Ok(written) => {
                for note in &written.notes {
                    report(file, tree.document().source(), note);
                }
                replace(file, written.text.as_bytes())
            }
            Err(diagnostics) => {
                for d in &diagnostics {
                    report(file, tree.document().source(), d);
                }
                Err(refused(format!(
                    "{}: not saved; the reference writer refused it",
                    file.display()
                )))
            }
        };
    };
    let project = Project::load(project_path).map_err(|e| refused(e.to_string()))?;
    let wanted = fs::canonicalize(file).map_err(|e| refused(format!("{}: {e}", file.display())))?;
    let documents = project.documents();
    let Some(index) = documents.iter().position(|d| d.canonical_path == wanted) else {
        return Err(refused(format!(
            "{} is not part of the project {}",
            file.display(),
            project_path.display()
        )));
    };
    match project.write_reference(index, &o.reference_options) {
        Ok(written) => {
            let d = &documents[index];
            for note in &written.notes {
                report(&d.path, d.tree.document().source(), note);
            }
            replace(file, written.text.as_bytes())
        }
        Err(diagnostics) => {
            for pd in &diagnostics {
                let d = &documents[pd.document];
                report(&d.path, d.tree.document().source(), &pd.diagnostic);
            }
            Err(refused(format!(
                "{}: not saved; the reference writer refused it",
                file.display()
            )))
        }
    }
}

/// A new temporary file next to `path`, holding `bytes`, flushed to disk.
fn temporary(path: &Path, bytes: &[u8]) -> Result<PathBuf, Failure> {
    let fail = |e: io::Error| refused(format!("{}: {e}", path.display()));
    let name = path
        .file_name()
        .ok_or_else(|| refused(format!("{}: names no file", path.display())))?;
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    for n in 0u32.. {
        let temp = dir.join(format!(
            ".{}.{}-{n}.tmp",
            name.to_string_lossy(),
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(mut file) => {
                let written = file.write_all(bytes).and_then(|()| file.sync_all());
                if let Err(e) = written {
                    let _ = fs::remove_file(&temp);
                    return Err(fail(e));
                }
                return Ok(temp);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(fail(e)),
        }
    }
    Err(refused(format!(
        "{}: no free temporary name",
        path.display()
    )))
}

/// Writes `path` whole, and only if nothing is there yet.
fn write_new(path: &Path, bytes: &[u8]) -> Outcome {
    let temp = temporary(path, bytes)?;
    // A hard link fails if `path` exists, so an existing file is never
    // replaced, and `path` appears complete or not at all.
    let linked = fs::hard_link(&temp, path);
    let _ = fs::remove_file(&temp);
    linked.map_err(|e| {
        if e.kind() == io::ErrorKind::AlreadyExists {
            refused(format!("{}: exists; not overwritten", path.display()))
        } else {
            refused(format!("{}: {e}", path.display()))
        }
    })
}

/// Replaces `path` whole, atomically.
fn replace(path: &Path, bytes: &[u8]) -> Outcome {
    let temp = temporary(path, bytes)?;
    fs::rename(&temp, path).map_err(|e| {
        let _ = fs::remove_file(&temp);
        refused(format!("{}: {e}", path.display()))
    })
}

fn status_name(s: Status) -> &'static str {
    match s {
        Status::Idle => "IDLE",
        Status::Running => "RUNNING",
        Status::Success => "SUCCESS",
        Status::Failure => "FAILURE",
        Status::Skipped => "SKIPPED",
    }
}

fn print_log(log: &Log) {
    println!("xml_bytes\t{}", log.xml.len());
    println!("t0_us\t{}", log.t0_us);
    println!("records\t{}", log.transitions.len());
    println!("# offset_us\tabsolute_us\tuid\tprevious\tstatus");
    for (t, previous) in log.with_previous() {
        let absolute = log
            .absolute_us(t)
            .map_or_else(|| "-".to_owned(), |a| a.to_string());
        println!(
            "{}\t{absolute}\t{}\t{}\t{}",
            t.offset_us,
            t.uid,
            status_name(previous),
            status_name(t.status)
        );
    }
}

/// F11: print a recording's header and records, one per line.
fn log_dump(path: &Path) -> Outcome {
    let bytes = fs::read(path).map_err(|e| refused(format!("{}: {e}", path.display())))?;
    let (log, error) = match Log::from_bytes(&bytes) {
        Ok(log) => (log, None),
        Err(e) => match e.partial() {
            Some(partial) => (partial.clone(), Some(e)),
            None => return Err(refused(format!("{}: {e}", path.display()))),
        },
    };
    print_log(&log);
    for d in log.diagnostics() {
        eprintln!("{}:@{}: {:?}", path.display(), d.offset, d.kind);
    }
    match error {
        None => Ok(()),
        Some(e) => Err(refused(format!(
            "{}: {e}; the records above were read before it",
            path.display()
        ))),
    }
}
