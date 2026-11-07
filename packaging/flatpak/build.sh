#!/bin/sh
# Runs inside flatpak-builder (see io.github.benwbooth.Almanac.yml).
set -eu

# Qt itself is in the KDE SDK (/usr); Qt WebEngine, WebChannel and Pdf come
# from the BaseApp in /app. cxx-qt finds Qt through qmake, which only knows
# /usr, so give it one folder of headers and one of libraries holding both,
# and a qmake that names those folders.
sysqmake="$(command -v qmake6 || command -v qmake)"
merged="$PWD/qt-merged"
rm -rf "$merged"; mkdir -p "$merged/lib" "$merged/include" "$merged/bin"
for d in "$("$sysqmake" -query QT_INSTALL_LIBS)" /app/lib /app/lib/x86_64-linux-gnu /app/lib64; do
    [ -d "$d" ] || continue
    for f in "$d"/libQt6* "$d"/libQt6*.prl; do [ -e "$f" ] && ln -sf "$f" "$merged/lib/"; done
done
for d in "$("$sysqmake" -query QT_INSTALL_HEADERS)" /app/include /app/include/qt6; do
    [ -d "$d" ] || continue
    for f in "$d"/Qt*; do [ -d "$f" ] && ln -sfn "$f" "$merged/include/"; done
done
cat > "$merged/bin/qmake" <<SH
#!/bin/sh
if [ "\$1" = -query ]; then
    case "\${2:-}" in
        QT_INSTALL_LIBS) echo "$merged/lib"; exit 0 ;;
        QT_INSTALL_HEADERS) echo "$merged/include"; exit 0 ;;
    esac
fi
exec "$sysqmake" "\$@"
SH
chmod +x "$merged/bin/qmake"
export QMAKE="$merged/bin/qmake"
export LIBRARY_PATH="$merged/lib${LIBRARY_PATH:+:$LIBRARY_PATH}"
echo "Qt $("$QMAKE" -query QT_VERSION); Qt libraries: $(ls "$merged/lib" | grep -c prl) modules"

cargo build --release --locked

install -Dm755 target/release/almanac /app/bin/almanac
install -Dm644 packaging/linux/io.github.benwbooth.Almanac.desktop /app/share/applications/io.github.benwbooth.Almanac.desktop
install -Dm644 packaging/linux/io.github.benwbooth.Almanac.metainfo.xml /app/share/metainfo/io.github.benwbooth.Almanac.metainfo.xml
install -Dm644 icons/almanac.svg /app/share/icons/hicolor/scalable/apps/io.github.benwbooth.Almanac.svg
install -Dm644 packaging/icons/almanac-512.png /app/share/icons/hicolor/512x512/apps/io.github.benwbooth.Almanac.png
