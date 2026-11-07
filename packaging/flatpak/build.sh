#!/bin/sh
# Runs inside flatpak-builder (see io.github.benwbooth.Almanac.yml).
set -eu

# Qt itself is in the KDE SDK (/usr); Qt WebEngine, WebChannel and Pdf come
# from the BaseApp in /app. qmake only knows /usr, so point the compiler and
# linker at /app's headers and libraries as well.
export QMAKE="$(command -v qmake6 || command -v qmake)"
inc=""
for d in /app/include /app/include/qt6 /app/include/Qt*  /app/include/qt6/Qt*; do
    [ -d "$d" ] && inc="$inc:$d"
done
export CPATH="${inc#:}${CPATH:+:$CPATH}"
export LIBRARY_PATH="/app/lib${LIBRARY_PATH:+:$LIBRARY_PATH}"
export RUSTFLAGS="-L native=/app/lib"
echo "QMAKE=$QMAKE  Qt $($QMAKE -query QT_VERSION)"
echo "CPATH=$CPATH"

cargo build --release --locked

install -Dm755 target/release/almanac /app/bin/almanac
install -Dm644 packaging/linux/io.github.benwbooth.Almanac.desktop /app/share/applications/io.github.benwbooth.Almanac.desktop
install -Dm644 packaging/linux/io.github.benwbooth.Almanac.metainfo.xml /app/share/metainfo/io.github.benwbooth.Almanac.metainfo.xml
install -Dm644 icons/almanac.svg /app/share/icons/hicolor/scalable/apps/io.github.benwbooth.Almanac.svg
