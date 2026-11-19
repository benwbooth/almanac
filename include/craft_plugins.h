// Keeps the desktop's Qt plugins from another Qt release out of the app.
//
// The desktop's QT_PLUGIN_PATH (on NixOS: every Plasma package, plus the
// system profile's merged lib/qt-6/plugins) carries plugins built for the
// desktop's own Qt. When that Qt is a different release from Almanac's, Qt
// still takes the newest copy of each plugin it finds, so the desktop's
// Wayland platform plugin wins over Almanac's own, fails to load (it uses Qt's
// private API, which only fits its own release) and the app aborts at start.
//
// So before the application object is made, QT_PLUGIN_PATH is swapped for one
// folder of links to the same plugins, Almanac's own Qt's first, leaving out
// any file that belongs to another release of Qt itself (a Nix store package
// named qt<module>-X.Y.Z). Plugins from KDE packages (Breeze, Plasma
// integration, KIO) stay: those either load or are skipped, never fatal.
// When nothing is from another release the path is left as it was.
#pragma once

#include <QtCore/QByteArray>
#include <QtCore/QCoreApplication>
#include <QtCore/QCryptographicHash>
#include <QtCore/QDir>
#include <QtCore/QDirIterator>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QHash>
#include <QtCore/QRegularExpression>
#include <QtCore/QStandardPaths>
#include <QtCore/QStringList>
#include <QtCore/QtGlobal>

// The Qt release a file belongs to, from its Nix store package ("qtwayland-6.11.3"),
// or empty when it isn't part of a Qt package.
inline QString craftQtReleaseOf(const QString& file)
{
    static const QRegularExpression re(
        QStringLiteral("^/nix/store/[0-9a-z]{32}-qt[0-9a-z]*-(\\d+\\.\\d+\\.\\d+)(?:-[^/]*)?/"));
    const QRegularExpressionMatch m = re.match(QFileInfo(file).canonicalFilePath());
    return m.hasMatch() ? m.captured(1) : QString();
}

// Returns the folder now in QT_PLUGIN_PATH, or empty if it was left alone.
inline QString craftKeepForeignQtPluginsOut(const QString& own)
{
    const QString env = qEnvironmentVariable("QT_PLUGIN_PATH");
    if (env.isEmpty())
        return QString();
    const QString mine = QString::fromLatin1(qVersion());
    QStringList entries;
    if (!own.isEmpty())
        entries << own;
    for (const QString& e : env.split(QLatin1Char(':'), Qt::SkipEmptyParts))
        if (!entries.contains(e) && QFileInfo(e).isDir())
            entries << e;

    // Each plugin by its place under a plugin folder ("platforms/libqwayland.so"):
    // the first folder that has a usable one wins, as Qt's own search would.
    QHash<QString, QString> chosen;
    QStringList order;
    bool foreign = false;
    for (const QString& entry : entries) {
        const QDir base(entry);
        QDirIterator it(entry, QDir::Files | QDir::NoDotAndDotDot,
                        QDirIterator::Subdirectories | QDirIterator::FollowSymlinks);
        while (it.hasNext()) {
            const QString path = it.next();
            const QString rel = base.relativeFilePath(path);
            if (chosen.contains(rel))
                continue;
            const QString release = craftQtReleaseOf(path);
            if (!release.isEmpty() && release != mine) {
                foreign = true;
                continue;
            }
            chosen.insert(rel, QFileInfo(path).canonicalFilePath());
            order << rel;
        }
    }
    if (!foreign)
        return QString();

    // One folder per set of plugins, so later starts (and other copies running
    // at once) reuse it; it's made under a temporary name and moved into place.
    // Old sets are left (a running copy may still load plugins from one); they
    // live in the runtime folder, which goes at logout.
    QString runtime = QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation);
    if (runtime.isEmpty())
        runtime = QDir::tempPath();
    QCryptographicHash hash(QCryptographicHash::Sha1);
    hash.addData(mine.toUtf8());
    for (const QString& rel : std::as_const(order)) {
        hash.addData(rel.toUtf8());
        hash.addData(chosen.value(rel).toUtf8());
    }
    const QString root = runtime + QStringLiteral("/almanac-qt-plugins");
    const QString dir = root + QLatin1Char('/') + QString::fromLatin1(hash.result().toHex().left(16));
    if (!QFileInfo(dir).isDir()) {
        const QString tmp = dir + QStringLiteral(".tmp-") + QString::number(QCoreApplication::applicationPid());
        QDir(tmp).removeRecursively();
        bool ok = QDir().mkpath(tmp);
        for (const QString& rel : std::as_const(order)) {
            const QString link = tmp + QLatin1Char('/') + rel;
            ok = ok && QDir().mkpath(QFileInfo(link).path()) && QFile::link(chosen.value(rel), link);
        }
        if (!ok || !QDir().rename(tmp, dir)) {
            QDir(tmp).removeRecursively();
            if (!QFileInfo(dir).isDir())
                return QString();
        }
    }
    qputenv("QT_PLUGIN_PATH", dir.toLocal8Bit());
    return dir;
}
