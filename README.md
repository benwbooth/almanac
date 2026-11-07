# Almanac

Mail, Calendar and Contacts for Linux, in one app.

Almanac talks to your accounts directly: IMAP for mail, SMTP for sending,
CalDAV for calendars and CardDAV for contacts, with OAuth sign-in for
Google, Microsoft, Yahoo and AOL. Passwords and refresh tokens are kept in
the system keychain.

## Stack

- **Rust** for the app logic (mail sync, CalDAV/CardDAV, OAuth, storage)
- **[cxx-qt](https://github.com/KDAB/cxx-qt)** to bridge Rust and Qt
- **Qt 6 / Qt Quick (QML)** for the interface, with Qt WebEngine for message bodies

## Building

### With Nix

```sh
nix develop      # shell with Rust and Qt 6
cargo run

nix build        # or build the package; the binary lands in ./result/bin
```

`flake.nix` follows `nixos-unstable`, pinned in `flake.lock`; run
`nix flake update` to move the pin.

### Development loop

```sh
scripts/dev.sh              # rebuild and restart Almanac on every change
scripts/dev.sh --scratch    # same, with a throwaway data folder instead of your mail
scripts/dev.sh --release -- --some-almanac-flag
```

It watches `src/`, `qml/`, `icons/`, `include/`, `build.rs` and `Cargo.toml`
with watchexec, and only replaces the running Almanac once a build succeeds.
App output goes to `target/dev-almanac.log`. It enters `nix develop` by itself
when watchexec or Qt aren't on the PATH.

### Without Nix

Install Rust and Qt 6 (qtbase, qtdeclarative, qtsvg, qtwebengine,
qtwebchannel, qtpositioning), then either set `QMAKE` to your Qt 6 `qmake`
or make `.qt-env` a symlink to your Qt install folder:

```sh
QMAKE=/path/to/qt6/bin/qmake cargo run --release
```

### OAuth clients

Sign-in with Google, Yahoo and AOL needs an OAuth client of your own. You can
create one from the Sign-in Setup screen inside the app, or bake one in at
build time with `ALMANAC_GOOGLE_CLIENT_ID` / `ALMANAC_GOOGLE_CLIENT_SECRET`
(and the `YAHOO` / `AOL` equivalents).
