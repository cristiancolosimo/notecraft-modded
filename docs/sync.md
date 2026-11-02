# Sync with a Git server

File, Sync keeps the whole notebooks folder in one Git repository and syncs it with a remote:
**GitHub** (the address is `https://github.com/you/notes.git`, the username is your GitHub name,
the password is a personal access token with the `repo` scope) or **any Git server** that takes a
username and a token over HTTPS.

Nothing is sent until the address, the username and the token are set and you press Sync now or
turn on "Sync by itself". The timer runs while NoteCraft is open, every N minutes (15 by default),
and only when the server can be reached; with no internet it waits and tries again. On Android
it runs only while the app is in front.

## What a sync does

1. Saves the page being written, then commits what changed in the folder.
2. Fetches the remote branch (`main`) and merges it.
3. Pushes the result.

A file changed on two devices is never lost: this device keeps its version and the other one is
saved beside it as `name.conflict-xxxxxxx.ext`. A file deleted on one device and changed on the
other comes back. A device that already has notes can sync into a repository that has other
notes; both sets end up in both places.

## Safety

- The token is kept in `sync-token.local` beside the settings file (owner-only where the system
  can say so), never in the settings file, the repository, a log line or a message. It is
  handed to Git's credential callback only, never put in the address.
- A token is only sent over HTTPS, or over plain HTTP to this computer (a development server).
- A wrong token is tried once, not repeatedly.
- The history keeps every earlier version of every page, so a sync is also a backup. Deleting a
  page does not delete it from the server's history.

## Building

The engine is libgit2 (the `git2` crate), because Android has no `git` program. It uses the
system's TLS: on Linux that means the OpenSSL development package (`libssl-dev`, `openssl-devel`);
Windows and macOS use their own.

## Tests

`cargo test -p notecraft-sync` runs two devices and a server on disk. Against a real server:

```bash
NOTECRAFT_SYNC_URL=https://host/notes.git NOTECRAFT_SYNC_USER=me NOTECRAFT_SYNC_TOKEN=... \
  cargo test -p notecraft-sync -- --ignored
```

It needs an empty repository.
