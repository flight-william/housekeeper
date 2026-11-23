//! `house review [--from YYYY-MM-DD] [--to YYYY-MM-DD] [--repo <path or name>] [--json]`:
//! the run records in the record folder, read back into one report.

use super::{Cmd, USAGE, today};
use crate::excuses::Date;
use crate::review::{self, Filter, Repo, ReviewError};
use crate::{files, record};
use std::path::Path;

/// The arguments after `review`. A date must be a real `YYYY-MM-DD`, and `--from`
/// may not be after `--to`.
pub(super) fn parse_review(rest: &[&str]) -> Result<Cmd, String> {
    let mut from = None;
    let mut to = None;
    let mut repo = None;
    let mut json = false;
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match *arg {
            "--json" => json = true,
            "--from" => from = Some(date(it.next())?),
            "--to" => to = Some(date(it.next())?),
            "--repo" => match it.next() {
                Some(r) if !r.is_empty() && !r.starts_with('-') => repo = Some(r.to_string()),
                _ => return Err(USAGE.into()),
            },
            _ => return Err(USAGE.into()),
        }
    }
    if let (Some(from), Some(to)) = (from, to)
        && from > to
    {
        return Err(USAGE.into());
    }
    Ok(Cmd::Review {
        from,
        to,
        repo,
        json,
    })
}

fn date(arg: Option<&&str>) -> Result<Date, String> {
    arg.and_then(|d| Date::parse(d)).ok_or_else(|| USAGE.into())
}

/// Print the review of the records for the period; `--to` defaults to today. Exit 0,
/// or 2 when the record folder cannot be read.
pub(super) fn run_review(
    cwd: &Path,
    from: Option<Date>,
    to: Option<Date>,
    repo: Option<&str>,
    json: bool,
) -> i32 {
    let Some(dir) = record::log_dir() else {
        println!(
            "house review: cannot read the records: neither HOUSE_LOG_DIR nor HOME is set. fix: set HOUSE_LOG_DIR to the record folder"
        );
        return 2;
    };
    let filter = Filter {
        from,
        to: Some(to.unwrap_or_else(today)),
        repo: repo.map(|r| repo_key(cwd, r)),
    };
    let review = match review::review_folder(&dir, &filter) {
        Ok(review) => review,
        Err(ReviewError::Read(e)) => {
            println!(
                "house review: cannot read the records in {}: {e}. fix: set HOUSE_LOG_DIR to a folder you can read",
                dir.display()
            );
            return 2;
        }
        Err(ReviewError::Limit(what)) => {
            println!(
                "house review: cannot review: {what}. fix: review a shorter period or one repo"
            );
            return 2;
        }
    };
    if json {
        println!("{}", review.json());
    } else {
        print!("{}", review.text());
    }
    0
}

/// A path (it holds a `/` or starts with `.`) becomes its repo's top folder, as the
/// records name it, and its repository's root commit; anything else is a name.
fn repo_key(cwd: &Path, repo: &str) -> Repo {
    if !repo.contains('/') && !repo.starts_with('.') {
        return Repo::Name(repo.to_string());
    }
    let path = cwd.join(repo);
    let folder = files::repo_root(&path)
        .or_else(|_| path.canonicalize().map_err(|_| ()))
        .map_or(repo.trim_end_matches('/').to_string(), |p| {
            p.display().to_string()
        });
    let is_dir = path.is_dir();
    let name = is_dir
        .then(|| files::main_checkout(&path))
        .flatten()
        .and_then(|main| main.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| last_part(&folder).to_string());
    Repo::Path {
        name,
        root_commit: is_dir.then(|| files::root_commit(&path)).flatten(),
        folder,
    }
}

fn last_part(folder: &str) -> &str {
    folder.rsplit('/').find(|s| !s.is_empty()).unwrap_or(folder)
}
