//! Which runs a review covers: a period, and the repository `--repo` names, matched
//! by its root commit, not by one of its folders.

use super::line::Run;
use crate::excuses::Date;
use std::path::Path;

/// Which runs a review covers: those whose start day is within `from..=to` (either
/// end open when `None`), in `repo` when given.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub from: Option<Date>,
    pub to: Option<Date>,
    pub repo: Option<Repo>,
}

/// The repository `--repo` names: a repository, not one of its folders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Repo {
    /// A path, resolved to its top folder and the root commit of its repository,
    /// and named by its main checkout's last part. A record with a root commit is
    /// the repository's when the roots are the same; a record without one (before
    /// 0.3.1), or a path with none, when the folders are the same.
    Path {
        name: String,
        folder: String,
        root_commit: Option<String>,
    },
    /// A name: a record's folder is the repository's when the folder's last part or
    /// its main checkout's last part is the name.
    Name(String),
}

impl Repo {
    /// What the report calls it.
    pub fn name(&self) -> &str {
        match self {
            Repo::Path { name, .. } | Repo::Name(name) => name,
        }
    }

    fn holds(&self, run: &Run) -> bool {
        match self {
            Repo::Path {
                folder,
                root_commit,
                ..
            } => match (root_commit, &run.root_commit) {
                (Some(want), Some(root)) => want == root,
                _ => run.repo == *folder,
            },
            Repo::Name(name) => {
                same_repo(&run.repo, name)
                    || run
                        .main_checkout
                        .as_deref()
                        .is_some_and(|main| same_repo(main, name))
            }
        }
    }
}

impl Filter {
    pub(super) fn covers(&self, day: Date, run: &Run) -> bool {
        self.from.is_none_or(|from| day >= from)
            && self.to.is_none_or(|to| day <= to)
            && self.in_repo(run)
    }

    pub(super) fn in_repo(&self, run: &Run) -> bool {
        self.repo.as_ref().is_none_or(|want| want.holds(run))
    }
}

fn same_repo(repo: &str, want: &str) -> bool {
    repo == want || Path::new(repo).file_name().is_some_and(|name| name == want)
}
