//! Sync: the notebooks folder is a Git repository, and syncing is commit, fetch, merge, push.
//!
//! The remote is any Git server that speaks HTTPS with a username and a token: GitHub (the
//! token is a personal access token) or a server of your own. Nothing is sent anywhere unless
//! the owner has set a remote and turned sync on, and the token is only ever given to Git's
//! credential callback, never put in the URL, a log line or a message.
//!
//! A change made on two devices to the same file never loses either: the local file stays as it
//! is and the other device's version is kept beside it as `name.conflict-XXXXXXX.ext`.

use git2::{
    Cred, ErrorCode, FetchOptions, IndexAddOption, IndexEntry, IndexTime, MergeOptions, PushOptions, RemoteCallbacks, Repository,
    RepositoryInitOptions, Signature,
};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

/// Where to sync to and who as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// An HTTPS address (`https://github.com/me/notes.git`), or a path or `file://` address.
    pub url: String,
    pub username: String,
    pub token: String,
    pub branch: String,
}

impl Config {
    pub fn new(url: &str, username: &str, token: &str) -> Config {
        Config { url: url.trim().to_string(), username: username.trim().to_string(), token: token.trim().to_string(), branch: "main".into() }
    }

    /// Enough to try a sync.
    pub fn is_complete(&self) -> bool {
        !self.url.is_empty() && (is_local(&self.url) || (!self.username.is_empty() && !self.token.is_empty()))
    }
}

/// What a sync did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Local changes were saved as a commit.
    pub committed: bool,
    /// Changes from the remote were brought in.
    pub pulled: bool,
    /// Local commits were sent to the remote.
    pub pushed: bool,
    /// Files changed on both sides; each has a `.conflict-` copy of the other version.
    pub conflicts: usize,
}

impl Report {
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.committed {
            parts.push("saved local changes");
        }
        if self.pulled {
            parts.push("received changes");
        }
        if self.pushed {
            parts.push("sent changes");
        }
        let mut text = if parts.is_empty() { "already up to date".to_string() } else { parts.join(", ") };
        if self.conflicts > 0 {
            text.push_str(&format!("; {} file(s) changed on both sides, the other version is kept beside each", self.conflicts));
        }
        text
    }
}

/// Whether a token may be sent to this address: HTTPS, a local path, or plain HTTP to this very
/// computer (a development server, or a tunnel to one). Never plain HTTP across a network.
pub fn address_allowed(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() || url.starts_with("https://") || is_local(url) {
        return true;
    }
    let loopback = ["http://127.0.0.1", "http://localhost", "http://[::1]"];
    loopback.iter().any(|prefix| url.strip_prefix(prefix).is_some_and(|rest| rest.is_empty() || rest.starts_with(':') || rest.starts_with('/')))
}

fn is_local(url: &str) -> bool {
    url.starts_with("file://") || !url.contains("://")
}

/// Whether the remote's server can be reached at all. A local remote always can.
pub fn reachable(url: &str) -> bool {
    if is_local(url) {
        return true;
    }
    let Some(rest) = url.split("://").nth(1) else { return false };
    let authority = rest.split('/').next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let target = if authority.contains(':') { authority.to_string() } else { format!("{authority}:443") };
    target.to_socket_addrs().ok().and_then(|mut a| a.next()).is_some_and(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(4)).is_ok())
}

fn callbacks(config: &Config) -> RemoteCallbacks<'_> {
    let mut callbacks = RemoteCallbacks::new();
    let mut tries = 0;
    callbacks.credentials(move |_url, _user, allowed| {
        // A wrong token would be asked for again and again; one try, then the error.
        tries += 1;
        if tries > 1 || !allowed.is_user_pass_plaintext() {
            return Err(git2::Error::from_str("the server did not accept the username and token"));
        }
        Cred::userpass_plaintext(&config.username, &config.token)
    });
    callbacks
}

fn err(what: &str, e: git2::Error) -> String {
    format!("{what}: {}", e.message())
}

/// Opens the folder's repository, making one the first time.
fn open(root: &Path, branch: &str) -> Result<Repository, String> {
    match Repository::open(root) {
        Ok(repo) => Ok(repo),
        Err(_) => {
            let mut options = RepositoryInitOptions::new();
            options.initial_head(branch);
            let repo = Repository::init_opts(root, &options).map_err(|e| err("cannot start the sync repository", e))?;
            let ignore = root.join(".gitignore");
            if !ignore.exists() {
                // Half-written files of the app's own work folders do not belong in the history.
                let _ = std::fs::write(&ignore, ".officeimo-staging-*/\n.demo-copy-*/\n*.tmp\n");
            }
            Ok(repo)
        }
    }
}

fn signature() -> Result<Signature<'static>, String> {
    Signature::now("NoteCraft", "notecraft@localhost").map_err(|e| err("cannot sign the commit", e))
}

/// Saves what changed in the folder as one commit. True when there was something to save.
fn commit_local(repo: &Repository, branch: &str) -> Result<bool, String> {
    let mut index = repo.index().map_err(|e| err("cannot read the index", e))?;
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None).map_err(|e| err("cannot stage the notes", e))?;
    index.update_all(["*"].iter(), None).map_err(|e| err("cannot stage the deletions", e))?;
    index.write().map_err(|e| err("cannot write the index", e))?;
    let tree = repo.find_tree(index.write_tree().map_err(|e| err("cannot write the tree", e))?).map_err(|e| err("cannot read the tree", e))?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    if let Some(parent) = &parent
        && parent.tree_id() == tree.id()
    {
        return Ok(false);
    }
    let when = format!("NoteCraft sync {}", notecraft_time());
    let sig = signature()?;
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    repo.commit(Some(&format!("refs/heads/{branch}")), &sig, &sig, &when, &tree, &parents).map_err(|e| err("cannot save the commit", e))?;
    Ok(true)
}

fn notecraft_time() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{secs}")
}

/// Brings the remote branch in. Returns (anything changed, conflicts).
fn merge_remote(repo: &Repository, branch: &str) -> Result<(bool, usize), String> {
    let remote_ref = format!("refs/remotes/origin/{branch}");
    let Ok(theirs_ref) = repo.find_reference(&remote_ref) else { return Ok((false, 0)) };
    let theirs = theirs_ref.peel_to_commit().map_err(|e| err("cannot read the remote branch", e))?;
    let Some(ours) = repo.head().ok().and_then(|h| h.peel_to_commit().ok()) else { return Ok((false, 0)) };
    if ours.id() == theirs.id() || repo.graph_descendant_of(ours.id(), theirs.id()).unwrap_or(false) {
        return Ok((false, 0));
    }
    let head_name = format!("refs/heads/{branch}");
    if repo.graph_descendant_of(theirs.id(), ours.id()).unwrap_or(false) {
        repo.reference(&head_name, theirs.id(), true, "NoteCraft sync: fast-forward").map_err(|e| err("cannot move the branch", e))?;
        checkout(repo)?;
        return Ok((true, 0));
    }
    let mut index = repo.merge_commits(&ours, &theirs, Some(&MergeOptions::new())).map_err(|e| err("cannot merge", e))?;
    let mut conflicts = 0;
    if index.has_conflicts() {
        let all: Vec<_> = index.conflicts().map_err(|e| err("cannot list conflicts", e))?.filter_map(Result::ok).collect();
        for conflict in all {
            let path = conflict.our.as_ref().or(conflict.their.as_ref()).map(|e| e.path.clone()).unwrap_or_default();
            let name = String::from_utf8_lossy(&path).into_owned();
            index.conflict_remove(Path::new(&name)).map_err(|e| err("cannot settle a conflict", e))?;
            match (conflict.our, conflict.their) {
                // Changed on both sides: ours stays, theirs is kept beside it.
                (Some(our), Some(their)) => {
                    index.add(&plain(&our, our.path.clone())).map_err(|e| err("cannot keep the local file", e))?;
                    let copy = conflict_name(&name, &their.id.to_string());
                    let entry = plain(&their, copy.into_bytes());
                    index.add(&entry).map_err(|e| err("cannot keep the other version", e))?;
                    conflicts += 1;
                }
                // Deleted here, changed there: the changed file comes back rather than being lost.
                (None, Some(their)) => index.add(&plain(&their, their.path.clone())).map_err(|e| err("cannot keep the other version", e))?,
                // Changed here, deleted there: the local file stays.
                (Some(our), None) => index.add(&plain(&our, our.path.clone())).map_err(|e| err("cannot keep the local file", e))?,
                (None, None) => {}
            }
        }
    }
    let tree =
        repo.find_tree(index.write_tree_to(repo).map_err(|e| err("cannot write the merge", e))?).map_err(|e| err("cannot read the merge", e))?;
    let sig = signature()?;
    repo.commit(Some(&head_name), &sig, &sig, "NoteCraft sync: merge", &tree, &[&ours, &theirs]).map_err(|e| err("cannot save the merge", e))?;
    checkout(repo)?;
    Ok((true, conflicts))
}

/// The same file, as an ordinary entry (not one side of a conflict) at `path`.
fn plain(entry: &IndexEntry, path: Vec<u8>) -> IndexEntry {
    IndexEntry {
        ctime: IndexTime::new(entry.ctime.seconds(), entry.ctime.nanoseconds()),
        mtime: IndexTime::new(entry.mtime.seconds(), entry.mtime.nanoseconds()),
        dev: entry.dev,
        ino: entry.ino,
        mode: entry.mode,
        uid: entry.uid,
        gid: entry.gid,
        file_size: entry.file_size,
        id: entry.id,
        flags: 0,
        flags_extended: 0,
        path,
    }
}

/// `dir/page.md` becomes `dir/page.conflict-1a2b3c4.md`.
fn conflict_name(path: &str, id: &str) -> String {
    let short = &id[..id.len().min(7)];
    let (dir, file) = path.rsplit_once('/').map(|(d, f)| (format!("{d}/"), f)).unwrap_or((String::new(), path));
    match file.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{dir}{stem}.conflict-{short}.{ext}"),
        _ => format!("{dir}{file}.conflict-{short}"),
    }
}

fn checkout(repo: &Repository) -> Result<(), String> {
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force())).map_err(|e| err("cannot update the files", e))
}

/// Commits what changed here, takes in what changed there, and sends the result.
pub fn sync(root: &Path, config: &Config) -> Result<Report, String> {
    if !config.is_complete() {
        return Err("the sync address, username and token are not all set".into());
    }
    let repo = open(root, &config.branch)?;
    let mut report = Report { committed: commit_local(&repo, &config.branch)?, ..Report::default() };
    let mut remote = match repo.find_remote("origin") {
        Ok(remote) => {
            if remote.url().ok() != Some(config.url.as_str()) {
                drop(remote);
                repo.remote_set_url("origin", &config.url).map_err(|e| err("cannot set the remote", e))?;
            }
            repo.find_remote("origin").map_err(|e| err("cannot open the remote", e))?
        }
        Err(_) => repo.remote("origin", &config.url).map_err(|e| err("cannot set the remote", e))?,
    };
    let refspec = format!("+refs/heads/{0}:refs/remotes/origin/{0}", config.branch);
    let mut fetch = FetchOptions::new();
    fetch.remote_callbacks(callbacks(config));
    match remote.fetch(&[refspec.as_str()], Some(&mut fetch), None) {
        Ok(()) => {}
        // A brand new remote has no branch yet: there is nothing to take in, only to send.
        Err(e) if e.code() == ErrorCode::NotFound || e.message().contains("couldn't find remote ref") => {}
        Err(e) => return Err(err("cannot reach the remote", e)),
    }
    let (pulled, conflicts) = merge_remote(&repo, &config.branch)?;
    report.pulled = pulled;
    report.conflicts = conflicts;

    let head = repo.head().ok().and_then(|h| h.target());
    let known = repo.find_reference(&format!("refs/remotes/origin/{}", config.branch)).ok().and_then(|r| r.target());
    if head.is_some() && head != known {
        let mut rejected: Option<String> = None;
        {
            let mut callbacks = callbacks(config);
            callbacks.push_update_reference(|_name, status| {
                if let Some(status) = status {
                    rejected = Some(status.to_string());
                }
                Ok(())
            });
            let mut push = PushOptions::new();
            push.remote_callbacks(callbacks);
            let spec = format!("refs/heads/{0}:refs/heads/{0}", config.branch);
            remote.push(&[spec.as_str()], Some(&mut push)).map_err(|e| err("cannot send the changes", e))?;
        }
        if let Some(why) = rejected {
            return Err(format!("the remote refused the changes: {why}"));
        }
        report.pushed = true;
    }
    Ok(report)
}

#[cfg(test)]
mod tests;
