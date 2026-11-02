use super::*;
use std::fs;
use std::path::PathBuf;

struct Dir(PathBuf);

impl Dir {
    fn new(label: &str) -> Dir {
        let path = std::env::temp_dir().join(format!("notecraft-sync-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("dir");
        Dir(path)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A bare repository standing in for the server, and its address.
fn server(dir: &Dir) -> Config {
    let path = dir.0.join("server.git");
    Repository::init_bare(&path).expect("bare");
    Config::new(path.to_str().expect("utf8"), "", "")
}

fn device(dir: &Dir, name: &str) -> PathBuf {
    let path = dir.0.join(name);
    fs::create_dir_all(&path).expect("device");
    path
}

#[test]
fn a_note_made_on_one_device_arrives_on_the_other() {
    let dir = Dir::new("roundtrip");
    let config = server(&dir);
    let (a, b) = (device(&dir, "a"), device(&dir, "b"));

    fs::create_dir_all(a.join("nb/section")).expect("dirs");
    fs::write(a.join("nb/section/page.md"), "# Hello").expect("write");
    let first = sync(&a, &config).expect("sync a");
    assert!(first.committed && first.pushed && !first.pulled, "{first:?}");

    let second = sync(&b, &config).expect("sync b");
    assert!(second.pulled, "{second:?}");
    assert_eq!(fs::read_to_string(b.join("nb/section/page.md")).expect("note"), "# Hello");

    // Nothing changed: nothing to do, on both sides.
    assert_eq!(sync(&b, &config).expect("again"), Report::default());
    let again = sync(&a, &config).expect("a again");
    assert!(!again.committed && !again.pushed, "{again:?}");
}

#[test]
fn changes_on_different_files_merge_and_deletions_travel() {
    let dir = Dir::new("merge");
    let config = server(&dir);
    let (a, b) = (device(&dir, "a"), device(&dir, "b"));
    fs::write(a.join("one.md"), "1").expect("write");
    fs::write(a.join("two.md"), "2").expect("write");
    sync(&a, &config).expect("a");
    sync(&b, &config).expect("b");

    fs::write(a.join("one.md"), "1 changed on a").expect("write");
    fs::remove_file(b.join("two.md")).expect("delete");
    fs::write(b.join("three.md"), "3").expect("write");
    sync(&a, &config).expect("a again");
    let merged = sync(&b, &config).expect("b again");
    assert!(merged.pulled && merged.pushed && merged.conflicts == 0, "{merged:?}");
    sync(&a, &config).expect("a last");

    for device in [&a, &b] {
        assert_eq!(fs::read_to_string(device.join("one.md")).expect("one"), "1 changed on a");
        assert_eq!(fs::read_to_string(device.join("three.md")).expect("three"), "3");
        assert!(!device.join("two.md").exists(), "the deletion reached {}", device.display());
    }
}

#[test]
fn a_file_changed_on_both_devices_keeps_both_versions() {
    let dir = Dir::new("conflict");
    let config = server(&dir);
    let (a, b) = (device(&dir, "a"), device(&dir, "b"));
    fs::write(a.join("page.md"), "start").expect("write");
    sync(&a, &config).expect("a");
    sync(&b, &config).expect("b");

    fs::write(a.join("page.md"), "from a").expect("write");
    fs::write(b.join("page.md"), "from b").expect("write");
    sync(&a, &config).expect("a sends");
    let report = sync(&b, &config).expect("b merges");
    assert_eq!(report.conflicts, 1, "{report:?}");

    // b keeps its own version, and a's is beside it; nothing is lost.
    assert_eq!(fs::read_to_string(b.join("page.md")).expect("page"), "from b");
    let copy = fs::read_dir(&b)
        .expect("dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|n| n.starts_with("page.conflict-") && n.ends_with(".md"))
        .expect("a conflict copy");
    assert_eq!(fs::read_to_string(b.join(copy)).expect("copy"), "from a");

    // And the other device receives both on its next sync.
    sync(&a, &config).expect("a receives");
    assert_eq!(fs::read_to_string(a.join("page.md")).expect("page"), "from b");
}

#[test]
fn two_devices_that_both_already_had_notes_come_together() {
    let dir = Dir::new("unrelated");
    let config = server(&dir);
    let (a, b) = (device(&dir, "a"), device(&dir, "b"));
    fs::write(a.join("a.md"), "a").expect("write");
    fs::write(b.join("b.md"), "b").expect("write");
    sync(&a, &config).expect("a");
    sync(&b, &config).expect("b");
    sync(&a, &config).expect("a again");
    for device in [&a, &b] {
        assert!(device.join("a.md").exists() && device.join("b.md").exists(), "{}", device.display());
    }
}

#[test]
fn an_incomplete_setup_is_refused_and_conflict_copies_are_named_beside_the_file() {
    let dir = Dir::new("setup");
    let path = device(&dir, "a");
    assert!(sync(&path, &Config::new("https://example.test/me/notes.git", "me", "")).is_err());
    assert!(!Config::new("", "", "").is_complete());
    assert!(Config::new("https://example.test/x.git", "me", "token").is_complete());
    assert_eq!(conflict_name("nb/page.md", "1234567890"), "nb/page.conflict-1234567.md");
    assert_eq!(conflict_name("README", "1234567890"), "README.conflict-1234567");
    assert!(!reachable("https://"), "no host, no connection");
}

#[test]
fn a_token_is_only_sent_over_https_or_to_this_computer() {
    assert!(address_allowed("https://github.com/me/notes.git"));
    assert!(address_allowed("http://127.0.0.1:8765/notes.git"));
    assert!(address_allowed("http://localhost/notes.git"));
    assert!(address_allowed("/srv/notes.git"));
    assert!(!address_allowed("http://example.test/notes.git"));
    assert!(!address_allowed("http://127.0.0.1.example.test/notes.git"));
}

/// Against a real server: `NOTECRAFT_SYNC_URL`, `NOTECRAFT_SYNC_USER` and `NOTECRAFT_SYNC_TOKEN`
/// name an empty repository. Run with `cargo test -p notecraft-sync -- --ignored`.
#[test]
#[ignore = "needs a Git server and its credentials in the environment"]
fn a_real_server_takes_the_token_and_refuses_a_wrong_one() {
    let (Ok(url), Ok(user), Ok(token)) =
        (std::env::var("NOTECRAFT_SYNC_URL"), std::env::var("NOTECRAFT_SYNC_USER"), std::env::var("NOTECRAFT_SYNC_TOKEN"))
    else {
        return;
    };
    let dir = Dir::new("real");
    let (a, b) = (device(&dir, "a"), device(&dir, "b"));
    fs::write(a.join("page.md"), "over the wire").expect("write");
    let sent = sync(&a, &Config::new(&url, &user, &token)).expect("a syncs with the right token");
    assert!(sent.pushed, "{sent:?}");
    let got = sync(&b, &Config::new(&url, &user, &token)).expect("b syncs");
    assert!(got.pulled, "{got:?}");
    assert_eq!(fs::read_to_string(b.join("page.md")).expect("note"), "over the wire");

    let wrong = sync(&b, &Config::new(&url, &user, "not-the-token"));
    assert!(wrong.is_err(), "a wrong token must fail, not loop: {wrong:?}");
}
