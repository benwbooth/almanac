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

### Release builds

Publishing a GitHub release runs `.github/workflows/release.yml`, which
builds and attaches:

- Windows: an installer (`*-windows-x64-setup.exe`) and a portable zip
- macOS: disk images for Apple Silicon and Intel (signed ad hoc, not notarized)
- Linux: an AppImage and a `.flatpak` bundle

It also attaches a Flatpak repository, which `.github/workflows/pages.yml` serves at
`https://benwbooth.github.io/almanac/flatpak/` next to the website in `docs/`, and updates the
Homebrew cask in `benwbooth/homebrew-tap` (when the `HOMEBREW_TAP_TOKEN`
secret is set). Packaging files live in `packaging/`; after changing
`icons/almanac.svg`, run `scripts/make-icons.py` to remake the `.ico` and
`.icns`. Run the workflow by hand from the Actions tab to try the builds
without publishing anything.

### OAuth clients

Sign-in with Google, Yahoo and AOL needs an OAuth client of your own. You can
create one from the Sign-in Setup screen inside the app, or bake one in at
build time with `ALMANAC_GOOGLE_CLIENT_ID` / `ALMANAC_GOOGLE_CLIENT_SECRET`
(and the `YAHOO` / `AOL` equivalents).
