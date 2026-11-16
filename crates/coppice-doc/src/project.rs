//! A project: a starting document and every document its includes reach
//! (spec §2.3, §2.3.1; F3), with the checks that need all of them at once.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

use crate::reference::{ReferenceOptions, Written, write_parts};
use crate::tree::{
    Diagnostic, DiagnosticKind, NodeModel, NotATreeDocument, TreeDocument, model_lookup,
};
use crate::{Document, Element, Node, ParseError};

/// Where a project's files come from.
///
/// [`Disk`] reads the file system. Any other source (an in-memory set of
/// files, an archive) implements this to load a project from there.
pub trait Files {
    /// Open the file at `path`: return its canonical identity (two paths
    /// naming the same file must give the same identity) and its bytes.
    fn open(&mut self, path: &Path) -> io::Result<(PathBuf, Vec<u8>)>;
}

/// The file system: identity is [`std::fs::canonicalize`], so a symbolic
/// link and its target are the same file.
#[derive(Debug, Clone, Copy, Default)]
pub struct Disk;

impl Files for Disk {
    fn open(&mut self, path: &Path) -> io::Result<(PathBuf, Vec<u8>)> {
        let canonical = std::fs::canonicalize(path)?;
        let bytes = std::fs::read(&canonical)?;
        Ok((canonical, bytes))
    }
}

/// One loaded document of a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDocument {
    /// The path the document was reached by: the starting path made
    /// absolute, or an include's path joined to its including document's
    /// directory. Nested relative includes resolve against its directory.
    pub path: PathBuf,
    /// The document's canonical identity, from [`Files::open`].
    pub canonical_path: PathBuf,
    /// The document.
    pub tree: TreeDocument,
}

/// A diagnostic in one of a project's documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDiagnostic {
    /// Index into [`Project::documents`]; the offset is into its source.
    pub document: usize,
    /// The diagnostic.
    pub diagnostic: Diagnostic,
}

/// A registered tree: the `<BehaviorTree>` a name resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectTree<'a> {
    /// The tree's `ID`.
    pub id: &'a str,
    /// Index into [`Project::documents`] of the document holding it.
    pub document: usize,
    /// The `<BehaviorTree>` element.
    pub element: &'a Element,
}

/// The starting document could not be loaded, so there is no project.
#[derive(Debug)]
pub enum LoadError {
    /// It could not be read (or the path could not be made absolute).
    Read {
        /// The path as given.
        path: PathBuf,
        /// Why.
        error: io::Error,
    },
    /// It is not well-formed XML.
    Parse {
        /// The path as given.
        path: PathBuf,
        /// Why.
        error: ParseError,
    },
    /// Its top-level element is not `<root>`.
    NotATree {
        /// The path as given.
        path: PathBuf,
        /// Why.
        error: NotATreeDocument,
    },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Read { path, error } => write!(f, "{}: {error}", path.display()),
            LoadError::Parse { path, error } => write!(f, "{}: {error}", path.display()),
            LoadError::NotATree { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::Read { error, .. } => Some(error),
            LoadError::Parse { error, .. } => Some(error),
            LoadError::NotATree { error, .. } => Some(error),
        }
    }
}

/// A loaded project (spec §2.3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// In load order: the starting document first, then each include
    /// depth-first, in source order.
    documents: Vec<ProjectDocument>,
    /// Document indices in the order the library registers their trees:
    /// a document after everything it includes.
    registration: Vec<usize>,
    /// Each registered tree `ID`, resolved once at load: the document and
    /// the tree's index in its `<root>` element's `children`.
    trees: BTreeMap<String, (usize, usize)>,
    /// What loading found wrong with includes.
    include_diagnostics: Vec<ProjectDiagnostic>,
}

/// One document being loaded, on the chain of includes.
struct Frame {
    document: usize,
    includes: Vec<crate::tree::Include>,
    next: usize,
    base: PathBuf,
}

impl Project {
    /// Load the project starting at `path` from the file system.
    pub fn load(path: &Path) -> Result<Project, LoadError> {
        Project::load_from(path, &mut Disk)
    }

    /// Load the project starting at `path` from `files`.
    ///
    /// Fails only when the starting document cannot be loaded; anything
    /// wrong with an include is a diagnostic, and that include is skipped.
    /// The walk is iterative, so a long chain of includes cannot exhaust
    /// the stack.
    pub fn load_from(path: &Path, files: &mut impl Files) -> Result<Project, LoadError> {
        let read_error = |error| LoadError::Read {
            path: path.to_owned(),
            error,
        };
        let reached = std::path::absolute(path).map_err(read_error)?;
        let (canonical, bytes) = files.open(&reached).map_err(read_error)?;
        let doc = Document::parse(&bytes).map_err(|error| LoadError::Parse {
            path: path.to_owned(),
            error,
        })?;
        let tree = TreeDocument::new(doc).map_err(|error| LoadError::NotATree {
            path: path.to_owned(),
            error,
        })?;

        let mut project = Project {
            documents: Vec::new(),
            registration: Vec::new(),
            trees: BTreeMap::new(),
            include_diagnostics: Vec::new(),
        };
        // Every file loaded so far, and those on the current chain of
        // includes (a subset), by canonical identity.
        let mut loaded: HashSet<PathBuf> = HashSet::new();
        let mut chain: HashSet<PathBuf> = HashSet::new();
        let mut stack: Vec<Frame> = Vec::new();
        project.push(
            &mut stack,
            &mut loaded,
            &mut chain,
            reached,
            canonical,
            tree,
        );

        while let Some(frame) = stack.last_mut() {
            let Some(include) = frame.includes.get(frame.next).cloned() else {
                let document = frame.document;
                project.registration.push(document);
                chain.remove(&project.documents[document].canonical_path);
                stack.pop();
                continue;
            };
            frame.next += 1;
            let from = frame.document;
            let base = frame.base.clone();
            let mut report = |kind, message: String| {
                project.include_diagnostics.push(ProjectDiagnostic {
                    document: from,
                    diagnostic: Diagnostic {
                        kind,
                        offset: include.offset,
                        message,
                    },
                });
            };

            if include.path.is_empty() {
                report(
                    DiagnosticKind::IncompleteInclude,
                    "an <include> without a path, or with an empty one".to_owned(),
                );
                continue;
            }
            let written = PathBuf::from(&include.path);
            if let Some(pkg) = &include.ros_pkg
                && !written.is_absolute()
            {
                report(
                    DiagnosticKind::RosPackageInclude,
                    format!(
                        "include {} is relative to ROS package {pkg}, which cannot be resolved here",
                        include.path
                    ),
                );
                continue;
            }
            let full = if written.is_absolute() {
                written
            } else {
                base.join(written)
            };
            let (canonical, bytes) = match files.open(&full) {
                Ok(opened) => opened,
                Err(e) => {
                    report(
                        DiagnosticKind::UnreadableInclude,
                        format!("cannot read included file {}: {e}", full.display()),
                    );
                    continue;
                }
            };
            if chain.contains(&canonical) {
                report(
                    DiagnosticKind::IncludeCycle,
                    format!(
                        "include {} leads back to a file that includes it; not followed",
                        full.display()
                    ),
                );
                continue;
            }
            if loaded.contains(&canonical) {
                continue;
            }
            let tree = match Document::parse(&bytes) {
                Err(e) => Err(e.to_string()),
                Ok(doc) => TreeDocument::new(doc).map_err(|e| e.to_string()),
            };
            match tree {
                Ok(tree) => {
                    project.push(&mut stack, &mut loaded, &mut chain, full, canonical, tree);
                }
                Err(e) => report(
                    DiagnosticKind::MalformedInclude,
                    format!(
                        "included file {} is not a tree document: {e}",
                        full.display()
                    ),
                ),
            }
        }
        for &document in &project.registration {
            let root = project.documents[document].tree.document().root_element();
            for (index, node) in root.children.iter().enumerate() {
                if let Node::Element(e) = node
                    && e.name == "BehaviorTree"
                    && let Some(id) = e.attribute("ID")
                {
                    project.trees.insert(id.to_owned(), (document, index));
                }
            }
        }
        Ok(project)
    }

    fn push(
        &mut self,
        stack: &mut Vec<Frame>,
        loaded: &mut HashSet<PathBuf>,
        chain: &mut HashSet<PathBuf>,
        path: PathBuf,
        canonical_path: PathBuf,
        tree: TreeDocument,
    ) {
        let document = self.documents.len();
        let base = path.parent().map_or_else(|| path.clone(), Path::to_owned);
        stack.push(Frame {
            document,
            includes: tree.includes(),
            next: 0,
            base,
        });
        loaded.insert(canonical_path.clone());
        chain.insert(canonical_path.clone());
        self.documents.push(ProjectDocument {
            path,
            canonical_path,
            tree,
        });
    }

    /// The loaded documents, in load order: the starting document first.
    pub fn documents(&self) -> &[ProjectDocument] {
        &self.documents
    }

    /// The `<TreeNodesModel>` entries of every document, merged in load
    /// order; for an `ID` declared more than once, the first declaration.
    pub fn models(&self) -> Vec<NodeModel> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut out: Vec<NodeModel> = Vec::new();
        for d in &self.documents {
            for m in d.tree.models() {
                if seen.insert(m.id.clone()) {
                    out.push(m);
                }
            }
        }
        out
    }

    /// The registered trees, by `ID` in byte order of the name (as the
    /// library lists them), each resolved to the registration that wins.
    /// A `<BehaviorTree>` without an `ID` is not listed (the library names
    /// it `BehaviorTree_<n>`; coppice does not).
    pub fn trees(&self) -> Vec<ProjectTree<'_>> {
        self.trees
            .iter()
            .filter_map(|(id, &at)| self.resolve(id, at))
            .collect()
    }

    /// The tree `id` resolves to.
    pub fn tree(&self, id: &str) -> Option<ProjectTree<'_>> {
        let (id, &at) = self.trees.get_key_value(id)?;
        self.resolve(id, at)
    }

    /// The starting document's `main_tree_to_execute`, if any.
    pub fn main_tree(&self) -> Option<&str> {
        self.documents.first().and_then(|d| {
            d.tree
                .document()
                .root_element()
                .attribute("main_tree_to_execute")
        })
    }

    fn resolve<'a>(
        &'a self,
        id: &'a str,
        (document, index): (usize, usize),
    ) -> Option<ProjectTree<'a>> {
        let root = self.documents.get(document)?.tree.document().root_element();
        match root.children.get(index)? {
            Node::Element(element) => Some(ProjectTree {
                id,
                document,
                element,
            }),
            _ => None,
        }
    }

    /// Write document `document` as the reference writes a tree file
    /// saved through this project (spec §3.2), with the project's merged
    /// models.
    ///
    /// Refuses, besides what [`crate::write_tree`] refuses (spec §3.7):
    /// - any diagnostic of this project in that document, project-level
    ///   ones included (a subtree that names no tree);
    /// - any model diagnostic in any document, since the merged models
    ///   fill the model block;
    /// - a model block that is not empty while an include is unresolved,
    ///   as a model it holds (or would hold) may be declared there.
    ///
    /// A refusal about a declared model points into the document that
    /// declared it.
    ///
    /// # Panics
    ///
    /// If `document` is not an index into [`Project::documents`].
    pub fn write_reference(
        &self,
        document: usize,
        options: &ReferenceOptions,
    ) -> Result<Written, Vec<ProjectDiagnostic>> {
        let d = &self.documents[document];
        let all = self.diagnostics();
        let blocking: Vec<ProjectDiagnostic> = all
            .iter()
            .filter(|p| {
                p.document == document
                    || matches!(
                        p.diagnostic.kind,
                        DiagnosticKind::UnknownModelKind
                            | DiagnosticKind::IncompleteModel
                            | DiagnosticKind::DuplicateModelsBlock
                            | DiagnosticKind::DuplicateModel
                            | DiagnosticKind::ModelShadowsBuiltin
                    )
            })
            .cloned()
            .collect();
        if !blocking.is_empty() {
            return Err(blocking);
        }
        // Which document each merged model came from: the first to declare
        // its ID, as `models` merges.
        let mut origin: HashMap<String, usize> = HashMap::new();
        for (i, doc) in self.documents.iter().enumerate() {
            for m in doc.tree.models() {
                origin.entry(m.id).or_insert(i);
            }
        }
        let written = write_parts(&d.tree, &self.models(), options).map_err(|r| {
            let mut out: Vec<ProjectDiagnostic> = r
                .tree
                .into_iter()
                .map(|diagnostic| ProjectDiagnostic {
                    document,
                    diagnostic,
                })
                .collect();
            out.extend(
                r.models
                    .into_iter()
                    .map(|(id, diagnostic)| ProjectDiagnostic {
                        document: origin.get(&id).copied().unwrap_or(document),
                        diagnostic,
                    }),
            );
            out.sort_by_key(|p| (p.document, p.diagnostic.offset));
            out
        })?;
        let unresolved: Vec<ProjectDiagnostic> = all
            .into_iter()
            .filter(|p| {
                matches!(
                    p.diagnostic.kind,
                    DiagnosticKind::UnreadableInclude
                        | DiagnosticKind::MalformedInclude
                        | DiagnosticKind::RosPackageInclude
                        | DiagnosticKind::IncompleteInclude
                )
            })
            .collect();
        // An unreadable file may declare a model the block holds (a
        // duplicate the project would then refuse, or the declaration that
        // would win), or the one the V3 importer would otherwise invent.
        if !written.models.is_empty() && !unresolved.is_empty() {
            let mut out = unresolved;
            out.push(ProjectDiagnostic {
                document,
                diagnostic: Diagnostic {
                    kind: DiagnosticKind::NotInRecord,
                    offset: d.tree.document().root_element().span.start,
                    message: format!(
                        "the model block holds [{}], but an include is unresolved, and a model may be declared there",
                        written.models.join(", ")
                    ),
                },
            });
            out.sort_by_key(|p| (p.document, p.diagnostic.offset));
            return Err(out);
        }
        Ok(written)
    }

    /// Everything worth reporting across the project, sorted by document
    /// and then offset: each document's own diagnostics with its nodes
    /// checked against the merged [`Project::models`], what loading found
    /// wrong with includes, and the checks that need every document.
    pub fn diagnostics(&self) -> Vec<ProjectDiagnostic> {
        let models = self.models();
        let lookup = model_lookup(&models);
        let mut out = self.include_diagnostics.clone();
        let mut push = |document, kind, offset, message| {
            out.push(ProjectDiagnostic {
                document,
                diagnostic: Diagnostic {
                    kind,
                    offset,
                    message,
                },
            });
        };

        for (document, d) in self.documents.iter().enumerate() {
            for diagnostic in d.tree.diagnostics_with_lookup(&lookup) {
                push(
                    document,
                    diagnostic.kind,
                    diagnostic.offset,
                    diagnostic.message,
                );
            }
        }

        // A model declared again in a later document, reported at its first
        // declaration in that document. Repeats within one document are
        // already reported by that document.
        let mut first_in: HashMap<String, usize> = HashMap::new();
        for (document, d) in self.documents.iter().enumerate() {
            let mut here: HashSet<String> = HashSet::new();
            for m in d.tree.models() {
                if !here.insert(m.id.clone()) {
                    continue;
                }
                match first_in.get(&m.id) {
                    Some(&first) => push(
                        document,
                        DiagnosticKind::DuplicateModel,
                        m.offset,
                        format!(
                            "model {} is already declared in {}; that one is used",
                            m.id,
                            self.documents[first].path.display()
                        ),
                    ),
                    None => {
                        first_in.insert(m.id, document);
                    }
                }
            }
        }

        // Trees replaced by a later registration of the same ID.
        let mut registered: BTreeMap<&str, usize> = BTreeMap::new();
        for &document in &self.registration {
            for element in self.documents[document].tree.behavior_trees() {
                let Some(id) = element.attribute("ID") else {
                    continue;
                };
                if let Some(earlier) = registered.insert(id, document) {
                    push(
                        document,
                        DiagnosticKind::DuplicateTree,
                        element.span.start,
                        format!(
                            "tree {id} replaces the tree of the same ID in {}",
                            self.documents[earlier].path.display()
                        ),
                    );
                }
            }
        }

        if let Some(root) = self
            .documents
            .first()
            .map(|d| d.tree.document().root_element())
            && let Some(a) = root
                .attributes
                .iter()
                .find(|a| a.name == "main_tree_to_execute")
            && !registered.contains_key(a.value.as_str())
        {
            push(
                0,
                DiagnosticKind::UnknownMainTree,
                a.name_span.start,
                format!("main_tree_to_execute names {}, which no tree has", a.value),
            );
        }

        for (document, d) in self.documents.iter().enumerate() {
            for tree in d.tree.behavior_trees() {
                let mut stack: Vec<&Element> = tree.child_elements().collect();
                while let Some(e) = stack.pop() {
                    stack.extend(e.child_elements());
                    if (e.name == "SubTree" || e.name == "SubTreePlus")
                        && let Some(id) = e.attribute("ID")
                        && !registered.contains_key(id)
                    {
                        push(
                            document,
                            DiagnosticKind::UnknownSubTree,
                            e.span.start,
                            format!(
                                "<{}> names tree {id}, which the project has no tree of",
                                e.name
                            ),
                        );
                    }
                }
            }
        }

        out.sort_by_key(|p| (p.document, p.diagnostic.offset));
        out
    }
}
