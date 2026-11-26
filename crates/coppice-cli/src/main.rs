//! The `coppice` command-line tool (design decision D10). Each verb exposes
//! one feature of the library crates.

use std::ffi::OsString;
#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use coppice_btlog::{
    DiagnosticKind as LogDiagnosticKind, Log, OFFSET_MASK, Status, Transition, XML_OFFSET,
};
use coppice_doc::{
    Diagnostic, DiagnosticKind, Document, LoadError, Project, ReferenceOptions, TreeDocument,
    WriteMode, write_tree,
};

const USAGE: &str = "usage: coppice --version
       coppice check <file>
       coppice save-as <input> <output> [--reference [--v3] [--target <version>]]
       coppice save <tree-file> --reference [--project <project-file>] [--v3] [--target <version>]
       coppice log dump <file.btlog>
       coppice log write <tree.xml> <records> <file.btlog>";

/// How a run ended short of success.
enum Failure {
    /// Exit 1: the input has a problem, or was refused.
    Refused(String),
    /// Exit 2: the command line is wrong.
    Usage(String),
    /// Standard output was closed by its reader; stop quietly, exit 0.
    Closed,
}

type Outcome = Result<(), Failure>;

fn refused(message: impl Into<String>) -> Failure {
    Failure::Refused(message.into())
}

fn usage(message: impl Into<String>) -> Failure {
    Failure::Usage(message.into())
}

/// Writes a line to standard error. A failure to write there has nowhere
/// to be reported, so it is ignored rather than aborting the run.
fn warn(line: &str) {
    let _ = writeln!(io::stderr().lock(), "{line}");
}

/// Writes a line to standard output. A reader that stopped reading ends the
/// run quietly; any other write failure is reported.
fn say(out: &mut impl Write, line: &str) -> Outcome {
    writeln!(out, "{line}").map_err(|e| {
        if e.kind() == io::ErrorKind::BrokenPipe {
            Failure::Closed
        } else {
            refused(format!("cannot write the output: {e}"))
        }
    })
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let outcome = match args
        .iter()
        .map(|a| a.to_str())
        .collect::<Option<Vec<&str>>>()
    {
        None => Err(usage("arguments must be valid UTF-8")),
        Some(args) => run(&args),
    };
    match outcome {
        Ok(()) | Err(Failure::Closed) => ExitCode::SUCCESS,
        Err(Failure::Refused(message)) => {
            if !message.is_empty() {
                warn(&format!("coppice: {message}"));
            }
            ExitCode::from(1)
        }
        Err(Failure::Usage(message)) => {
            if !message.is_empty() {
                warn(&format!("coppice: {message}"));
            }
            warn(USAGE);
            ExitCode::from(2)
        }
    }
}

fn run(args: &[&str]) -> Outcome {
    // An operand that looks like an option is a mistake far more often than
    // a file name: `save-as in --reference` has lost its output. A file
    // whose name starts with `-` can be named as `./-name`.
    let operands = match args {
        ["check" | "save", file, ..] => vec![*file],
        ["save-as", input, output, ..] => vec![*input, *output],
        ["log", "dump", file] => vec![*file],
        ["log", "write", xml, records, output] => vec![*xml, *records, *output],
        _ => Vec::new(),
    };
    if let Some(bad) = operands.iter().find(|a| a.starts_with('-')) {
        return Err(usage(format!(
            "{bad} is where a file name belongs (options follow the file names); \
             write ./{bad} for a file named so"
        )));
    }
    match args {
        ["--version" | "-V"] => say(
            &mut io::stdout().lock(),
            &format!("coppice {}", env!("CARGO_PKG_VERSION")),
        ),
        ["check", file] => check(Path::new(file)),
        ["save-as", input, output, rest @ ..] => save_as(Path::new(input), Path::new(output), rest),
        ["save", file, rest @ ..] => save(Path::new(file), rest),
        ["log", "dump", file] => log_dump(Path::new(file)),
        ["log", "write", xml, records, output] => {
            log_write(Path::new(xml), Path::new(records), Path::new(output))
        }
        _ => Err(usage("")),
    }
}

/// The 1-based line and column of byte `offset` in `source`, the column
/// counted in characters. A byte-order mark at the start is not counted.
fn position(source: &str, offset: usize) -> (usize, usize) {
    let mut end = offset.min(source.len());
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    let before = &source[..end];
    let line = before.matches('\n').count() + 1;
    let last = before.rsplit('\n').next().unwrap_or("");
    let last = if line == 1 {
        last.strip_prefix('\u{feff}').unwrap_or(last)
    } else {
        last
    };
    (line, last.chars().count() + 1)
}

/// `<path>:<line>:<column>: <kind>: <message>`.
fn report(path: &Path, source: &str, d: &Diagnostic) {
    let (line, column) = position(source, d.offset);
    warn(&format!(
        "{}:{line}:{column}: {:?}: {}",
        path.display(),
        d.kind,
        d.message
    ));
}

/// A load failure, with the position of a parse error when there is one.
fn load_failure(e: &LoadError) -> Failure {
    if let LoadError::Parse { path, error } = e
        && let Ok(bytes) = fs::read(path)
    {
        let source = String::from_utf8_lossy(&bytes);
        let (line, column) = position(&source, error.offset());
        return refused(format!("{}:{line}:{column}: {error}", path.display()));
    }
    refused(e.to_string())
}

/// F10: load a file, with its includes, and report every diagnostic.
fn check(path: &Path) -> Outcome {
    let project = Project::load(path).map_err(|e| load_failure(&e))?;
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

#[derive(Default)]
struct Options {
    reference: bool,
    v3: bool,
    target: Option<String>,
    project: Option<PathBuf>,
}

impl Options {
    fn writer(&self) -> ReferenceOptions {
        let mut o = ReferenceOptions::default();
        if self.v3 {
            o.mode = WriteMode::V3;
        }
        if let Some(t) = &self.target {
            o.target = t.clone();
        }
        o
    }
}

/// Parses the options after a verb's operands. Every option may appear
/// once; an option's value may not be empty or start with `-`.
fn options(rest: &[&str], project_allowed: bool) -> Result<Options, Failure> {
    let mut o = Options::default();
    let mut i = 0;
    while i < rest.len() {
        let flag = rest[i];
        let mut value = || -> Result<String, Failure> {
            i += 1;
            match rest.get(i) {
                Some(v) if !v.is_empty() && !v.starts_with('-') => Ok((*v).to_owned()),
                _ => Err(usage(format!("{flag} needs a value"))),
            }
        };
        let repeated = match flag {
            "--reference" => std::mem::replace(&mut o.reference, true),
            "--v3" => std::mem::replace(&mut o.v3, true),
            "--target" => o.target.replace(value()?).is_some(),
            "--project" if project_allowed => o.project.replace(PathBuf::from(value()?)).is_some(),
            other => return Err(usage(format!("unknown option {other}"))),
        };
        if repeated {
            return Err(usage(format!("{flag} is given twice")));
        }
        i += 1;
    }
    if (o.v3 || o.target.is_some()) && !o.reference {
        return Err(usage("--v3 and --target apply only with --reference"));
    }
    Ok(o)
}

fn read_tree(path: &Path) -> Result<TreeDocument, Failure> {
    let bytes = fs::read(path).map_err(|e| refused(format!("{}: {e}", path.display())))?;
    let doc = Document::parse(&bytes).map_err(|e| {
        let source = String::from_utf8_lossy(&bytes);
        let (line, column) = position(&source, e.offset());
        refused(format!("{}:{line}:{column}: {e}", path.display()))
    })?;
    TreeDocument::new(doc).map_err(|e| refused(format!("{}: {e}", path.display())))
}

/// The standalone reference form of `tree` (F9), its notes reported.
fn reference_text(path: &Path, tree: &TreeDocument, o: &Options) -> Result<String, Failure> {
    let models = tree.models();
    match write_tree(tree, &models, &o.writer()) {
        Ok(written) => {
            for note in &written.notes {
                report(path, tree.document().source(), note);
            }
            Ok(written.text)
        }
        Err(diagnostics) => {
            for d in &diagnostics {
                report(path, tree.document().source(), d);
            }
            Err(refused(format!(
                "{}: not saved; the reference writer refused it",
                path.display()
            )))
        }
    }
}

/// F5 (lossless) or F9 (reference) save of a standalone file to a new path.
fn save_as(input: &Path, output: &Path, rest: &[&str]) -> Outcome {
    let o = options(rest, false)?;
    let tree = read_tree(input)?;
    let text = if o.reference {
        reference_text(input, &tree, &o)?
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
    let existing = replaceable(file)?;
    let Some(project_path) = &o.project else {
        let tree = read_tree(file)?;
        let text = reference_text(file, &tree, &o)?;
        return replace(file, &existing, text.as_bytes());
    };
    let project = Project::load(project_path).map_err(|e| load_failure(&e))?;
    let wanted = fs::canonicalize(file).map_err(|e| refused(format!("{}: {e}", file.display())))?;
    let documents = project.documents();
    let Some(index) = documents.iter().position(|d| d.canonical_path == wanted) else {
        return Err(refused(format!(
            "{} is not part of the project {}",
            file.display(),
            project_path.display()
        )));
    };
    match project.write_reference(index, &o.writer()) {
        Ok(written) => {
            // A declaration that loses to an earlier one does not block the
            // save, but the user may have meant it to win: say which. The
            // saved file's own is in the notes, as the save replaces it.
            for pd in project.diagnostics() {
                if pd.diagnostic.kind == DiagnosticKind::DuplicateModel && pd.document != index {
                    let d = &documents[pd.document];
                    report(&d.path, d.tree.document().source(), &pd.diagnostic);
                }
            }
            let d = &documents[index];
            for note in &written.notes {
                report(&d.path, d.tree.document().source(), note);
            }
            replace(file, &existing, written.text.as_bytes())
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

/// Checks that `path` can be replaced by a new file without losing
/// anything the user would expect kept, and returns its metadata. Refused:
/// a symbolic link (replacing it would cut the link, not save its target),
/// a file this user may not write, and, on Unix, a file with more than one
/// hard link (the other names would keep the old content).
fn replaceable(path: &Path) -> Result<fs::Metadata, Failure> {
    let meta =
        fs::symlink_metadata(path).map_err(|e| refused(format!("{}: {e}", path.display())))?;
    let why = if meta.file_type().is_symlink() {
        Some("is a symbolic link; save its target instead")
    } else if !meta.is_file() {
        Some("is not a regular file")
    } else if meta.permissions().readonly() || !writable(path) {
        Some("is read-only")
    } else if hard_links(&meta) > 1 {
        Some("has more than one hard link")
    } else {
        None
    };
    match why {
        Some(why) => Err(refused(format!("{}: {why}; not saved", path.display()))),
        None => Ok(meta),
    }
}

/// Whether this user may write `path`: opening it for writing, without
/// truncating or writing, answers that as the system would. The open is
/// visible to file watchers as an open and a close, and waits for any lease
/// on the file to be released; it is done once per save.
fn writable(path: &Path) -> bool {
    OpenOptions::new().write(true).open(path).is_ok()
}

/// The identity and state of a file, to notice it being swapped, linked or
/// rewritten while a save runs: device, inode, link count, size and
/// modification time.
#[cfg(unix)]
fn identity(meta: &fs::Metadata) -> (u64, u64, u64, u64, i64, i64) {
    use std::os::unix::fs::MetadataExt;
    (
        meta.dev(),
        meta.ino(),
        meta.nlink(),
        meta.len(),
        meta.mtime(),
        meta.mtime_nsec(),
    )
}

#[cfg(not(unix))]
fn identity(meta: &fs::Metadata) -> (u64, u64, u64, u64, i64, i64) {
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as i64);
    (0, 0, 0, meta.len(), modified, 0)
}

/// Gives the new file the old one's owner and group, where the platform has
/// them; a user who cannot (the file is someone else's) is refused.
#[cfg(unix)]
fn keep_owner(temp: &Path, existing: &fs::Metadata) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let new = fs::metadata(temp)?;
    if (new.uid(), new.gid()) == (existing.uid(), existing.gid()) {
        return Ok(());
    }
    std::os::unix::fs::chown(temp, Some(existing.uid()), Some(existing.gid()))
}

#[cfg(not(unix))]
fn keep_owner(_: &Path, _: &fs::Metadata) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn hard_links(meta: &fs::Metadata) -> u64 {
    std::os::unix::fs::MetadataExt::nlink(meta)
}

#[cfg(not(unix))]
fn hard_links(_: &fs::Metadata) -> u64 {
    1
}

fn directory_of(path: &Path) -> &Path {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

/// Flushes a directory's entries to disk, where the platform allows it, so
/// a rename or link inside it survives a crash. Best effort.
fn sync_directory(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// A new temporary file next to `path`, holding `bytes`, flushed to disk.
fn temporary(path: &Path, bytes: &[u8]) -> Result<PathBuf, Failure> {
    let fail = |e: io::Error| refused(format!("{}: {e}", path.display()));
    let name = path
        .file_name()
        .ok_or_else(|| refused(format!("{}: names no file", path.display())))?;
    let dir = directory_of(path);
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
    })?;
    sync_directory(directory_of(path));
    Ok(())
}

/// Replaces `path` whole, atomically, keeping its permissions, owner and
/// group. Just before the rename, the file is checked again, and a save is
/// refused if it was swapped, linked or rewritten since `existing` was
/// taken. Between that check and the rename a short window remains,
/// which the standard library cannot close.
fn replace(path: &Path, existing: &fs::Metadata, bytes: &[u8]) -> Outcome {
    let temp = temporary(path, bytes)?;
    let prepared = keep_owner(&temp, existing)
        .map_err(|e| {
            refused(format!(
                "{}: cannot keep its owner and group: {e}",
                path.display()
            ))
        })
        .and_then(|()| {
            fs::set_permissions(&temp, existing.permissions())
                .map_err(|e| refused(format!("{}: {e}", path.display())))
        })
        .and_then(|()| {
            let now = fs::symlink_metadata(path)
                .map_err(|e| refused(format!("{}: {e}", path.display())))?;
            if identity(&now) == identity(existing) {
                Ok(())
            } else {
                Err(refused(format!(
                    "{}: changed while it was being saved; not saved",
                    path.display()
                )))
            }
        })
        .and_then(|()| {
            fs::rename(&temp, path).map_err(|e| refused(format!("{}: {e}", path.display())))
        });
    if let Err(e) = prepared {
        let _ = fs::remove_file(&temp);
        return Err(e);
    }
    sync_directory(directory_of(path));
    Ok(())
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

fn print_log(log: &Log) -> Outcome {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    say(&mut out, &format!("xml_bytes\t{}", log.xml.len()))?;
    say(&mut out, &format!("t0_us\t{}", log.t0_us))?;
    say(&mut out, &format!("records\t{}", log.transitions.len()))?;
    say(&mut out, "# offset_us\tabsolute_us\tuid\tprevious\tstatus")?;
    for (t, previous) in log.with_previous() {
        let absolute = log
            .absolute_us(t)
            .map_or_else(|| "-".to_owned(), |a| a.to_string());
        say(
            &mut out,
            &format!(
                "{}\t{absolute}\t{}\t{}\t{}",
                t.offset_us,
                t.uid,
                status_name(previous),
                status_name(t.status)
            ),
        )?;
    }
    out.flush().map_err(|e| {
        if e.kind() == io::ErrorKind::BrokenPipe {
            Failure::Closed
        } else {
            refused(format!("cannot write the output: {e}"))
        }
    })
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
    for d in log.diagnostics() {
        let text = match &d.kind {
            LogDiagnosticKind::UnknownUid { uid } => {
                format!("byte {}: no node in the XML has uid {uid}", d.offset)
            }
            LogDiagnosticKind::XmlUnreadable(e) => format!(
                "byte {}: the XML does not parse ({e}, counting from the XML's start); uids are not checked",
                XML_OFFSET + e.offset()
            ),
        };
        warn(&format!("{}: {text}", path.display()));
    }
    print_log(&log)?;
    match error {
        None => Ok(()),
        Some(e) => Err(refused(format!(
            "{}: {e}; the records above were read before it",
            path.display()
        ))),
    }
}

/// The status a record's name stands for, as `log dump` prints it.
fn status_from_name(name: &str) -> Option<Status> {
    [
        Status::Idle,
        Status::Running,
        Status::Success,
        Status::Failure,
        Status::Skipped,
    ]
    .into_iter()
    .find(|s| status_name(*s) == name)
}

/// Reads records in the form `log dump` prints: `t0_us`, then one record
/// per line. Blank lines and lines starting with `#` are skipped. A record
/// is `offset_us uid status`, or the five columns `log dump` prints, whose
/// `absolute_us` and `previous` are ignored. `records`, if given, must
/// count the records; `xml_bytes`, if given, must be `xml_len`.
fn parse_records(text: &str, xml_len: usize) -> Result<(i64, Vec<Transition>), String> {
    let mut t0 = None;
    let mut count = None;
    let mut transitions = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let at = |what: String| format!("line {}: {what}", n + 1);
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields.as_slice() {
            [] => {}
            [first, ..] if first.starts_with('#') => {}
            [key @ ("t0_us" | "records" | "xml_bytes"), value] => {
                let duplicate = match *key {
                    "t0_us" => t0
                        .replace(
                            value
                                .parse::<i64>()
                                .map_err(|_| at(format!("t0_us {value} is not a whole number")))?,
                        )
                        .is_some(),
                    "records" => count
                        .replace(
                            value
                                .parse::<usize>()
                                .map_err(|_| at(format!("records {value} is not a count")))?,
                        )
                        .is_some(),
                    _ => {
                        let bytes = value
                            .parse::<usize>()
                            .map_err(|_| at(format!("xml_bytes {value} is not a count")))?;
                        if bytes != xml_len {
                            return Err(at(format!(
                                "xml_bytes is {bytes}, but the XML file has {xml_len} bytes"
                            )));
                        }
                        false
                    }
                };
                if duplicate {
                    return Err(at(format!("{key} is given twice")));
                }
            }
            [offset, uid, status] | [offset, _, uid, _, status] => {
                let offset_us = offset
                    .parse::<u64>()
                    .ok()
                    .filter(|o| *o <= OFFSET_MASK)
                    .ok_or_else(|| {
                        at(format!(
                            "offset {offset} is not a whole number from 0 to {OFFSET_MASK}"
                        ))
                    })?;
                let uid = uid
                    .parse::<u16>()
                    .map_err(|_| at(format!("uid {uid} is not a whole number from 0 to 65535")))?;
                let status = status_from_name(status).ok_or_else(|| {
                    at(format!(
                        "{status} is not a status (IDLE, RUNNING, SUCCESS, FAILURE or SKIPPED)"
                    ))
                })?;
                transitions.push(Transition {
                    offset_us,
                    uid,
                    status,
                });
            }
            _ => {
                return Err(at(
                    "expected t0_us, records, xml_bytes or a record of 3 or 5 columns".to_owned(),
                ));
            }
        }
    }
    let t0 = t0.ok_or("no t0_us line")?;
    if let Some(count) = count
        && count != transitions.len()
    {
        return Err(format!(
            "records says {count}, but {} records follow",
            transitions.len()
        ));
    }
    Ok((t0, transitions))
}

/// F12: write a recording from a tree's XML and records in the form
/// `log dump` prints, to a file that does not exist yet.
fn log_write(xml: &Path, records: &Path, output: &Path) -> Outcome {
    let xml_bytes = fs::read(xml).map_err(|e| refused(format!("{}: {e}", xml.display())))?;
    let text = fs::read(records).map_err(|e| refused(format!("{}: {e}", records.display())))?;
    let text = String::from_utf8(text)
        .map_err(|_| refused(format!("{}: not UTF-8 text", records.display())))?;
    let (t0_us, transitions) = parse_records(&text, xml_bytes.len())
        .map_err(|e| refused(format!("{}: {e}", records.display())))?;
    let log = Log {
        xml: xml_bytes,
        t0_us,
        transitions,
    };
    for d in log.diagnostics() {
        let text = match &d.kind {
            LogDiagnosticKind::UnknownUid { uid } => {
                format!("no node in the XML has uid {uid}")
            }
            LogDiagnosticKind::XmlUnreadable(e) => {
                format!("the XML does not parse ({e}); uids are not checked")
            }
        };
        warn(&format!("{}: {text}", output.display()));
    }
    let bytes = log
        .to_bytes()
        .map_err(|e| refused(format!("{}: {e}", output.display())))?;
    write_new(output, &bytes)
}
