// Saves a picture of the app's window (including open panels and menus).
#pragma once

#include <QtCore/QString>
#include <QtCore/QDateTime>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtGui/QImage>
#include <QtQuick/QQuickWindow>

// Lays out an HTML page on A4 paper and saves it as a PDF (the person then
// prints it from their PDF viewer, which works the same on every system).
#include <QtGui/QPageLayout>
#include <QtGui/QPageSize>
#include <QtGui/QPdfWriter>
#include <QtGui/QTextDocument>

inline bool craftPrintPdf(const QString& html, const QString& path)
{
    QPdfWriter writer(path);
    writer.setPageSize(QPageSize(QPageSize::A4));
    writer.setPageMargins(QMarginsF(15, 15, 15, 15), QPageLayout::Millimeter);
    writer.setResolution(96);
    QTextDocument doc;
    doc.setHtml(html);
    doc.print(&writer);
    return true;
}

#include <QtWidgets/QApplication>
#include <QtPrintSupport/QPrintDialog>
#include <QtPrintSupport/QPrinter>

// Wayland desktops place windows themselves; with the session-restore
// extension (xx-session-management) they put each one back where it was.
#if __has_include(<QtGui/qpa/qplatformwindow_p.h>)
#include <QtGui/qpa/qplatformwindow_p.h>
#if QT_CONFIG(wayland)
#define CRAFT_WAYLAND_RESTORE 1
#endif
#endif

// A window hidden by the desktop (on another virtual desktop, fully covered,
// in the overview) stops being drawn; shown again without getting the focus,
// nothing told the page, and the browser engine's message pane stayed black
// until clicked. This notices a window coming back into view and tells it
// (its QML `reexposed()` signal), so the pane draws again.
class CraftExposeWatch : public QObject {
public:
    using QObject::QObject;
    bool eventFilter(QObject* o, QEvent* e) override
    {
        if (e->type() == QEvent::Expose) {
            if (auto* w = qobject_cast<QQuickWindow*>(o)) {
                // Keep each window's graphics and scene while it's hidden: released
                // (Qt's default allows it), the message pane's picture from the
                // browser engine could be lost and come back black.
                if (!w->property("_craftPersistent").toBool()) {
                    w->setProperty("_craftPersistent", true);
                    w->setPersistentGraphics(true);
                    w->setPersistentSceneGraph(true);
                }
                const bool now = w->isExposed();
                const QVariant was = w->property("_craftExposed");
                w->setProperty("_craftExposed", now);
                if (now && was.isValid() && !was.toBool()) {
                    // Every time it's back, however briefly it was hidden (a blip can
                    // leave the pane black too); a burst of them counts once. The
                    // pane's redraw changes no sizes, so this doesn't flicker.
                    const qint64 now_ms = QDateTime::currentMSecsSinceEpoch();
                    const qint64 hiddenFor = now_ms - w->property("_craftHiddenAt").toLongLong();
                    qWarning("almanac: window shown again after %lld ms hidden by the desktop", hiddenFor);
                    if (now_ms - w->property("_craftToldAt").toLongLong() >= 300 && w->metaObject()->indexOfSignal("reexposed()") >= 0) {
                        w->setProperty("_craftToldAt", now_ms);
                        QMetaObject::invokeMethod(w, "reexposed", Qt::QueuedConnection);
                    }
                } else if (!now && (!was.isValid() || was.toBool())) {
                    w->setProperty("_craftHiddenAt", QDateTime::currentMSecsSinceEpoch());
                    qWarning("almanac: window hidden by the desktop");
                }
            }
        }
        return false;
    }
};

// The application object (a widgets one, so print dialogs work).
inline void craftCreateApp(const QString& plugins, const QString& session)
{
    static int argc = 1;
    static char name[] = "almanac";
    static char flag[] = "-session";
    static QByteArray id;
    static char* argv[] = { name, nullptr, nullptr, nullptr };
    // The last run's session, so the desktop can restore its windows.
    if (!session.isEmpty()) {
        id = session.toLatin1();
        argv[1] = flag;
        argv[2] = id.data();
        argc = 3;
    }
    new QApplication(argc, argv);
    qApp->installEventFilter(new CraftExposeWatch(qApp));
    QCoreApplication::setApplicationName(QStringLiteral("Almanac"));
    QCoreApplication::setOrganizationName(QStringLiteral("Craft Anything"));
    // Inside a Flatpak the desktop file is named after the app ID.
    QGuiApplication::setDesktopFileName(qEnvironmentVariableIsSet("FLATPAK_ID")
        ? qEnvironmentVariable("FLATPAK_ID") : QStringLiteral("almanac"));
    if (!plugins.isEmpty())
        QCoreApplication::addLibraryPath(plugins);
}

inline int craftExec() { return QApplication::exec(); }

// The app's icon (taskbar, window switcher, title bars).
inline void craftSetAppIcon(const QString& path) { QGuiApplication::setWindowIcon(QIcon(path)); }

// This run's session (given by the desktop), to pass back next time.
inline QString craftSessionId() { return qApp ? qApp->sessionId() : QString(); }

// Shows the windows QML left hidden (property showAtStart), each named first
// so the desktop can match it with the window it saw last time.
inline void craftShowWindows()
{
    for (QWindow* w : QGuiApplication::topLevelWindows()) {
        if (!w->property("showAtStart").toBool() || w->isVisible())
            continue;
        w->create();
#ifdef CRAFT_WAYLAND_RESTORE
        if (auto* wl = w->nativeInterface<QNativeInterface::Private::QWaylandWindow>())
            wl->setSessionRestoreId(w->objectName());
#endif
        w->setVisible(true);
    }
}

// The system's print dialog, then prints the page. False when cancelled.
inline bool craftPrintDialog(const QString& html, const QString& title)
{
    QPrinter printer(QPrinter::HighResolution);
    printer.setDocName(title);
    QPrintDialog dialog(&printer);
    dialog.setWindowTitle(QStringLiteral("Print ") + title);
    if (dialog.exec() != QDialog::Accepted)
        return false;
    QTextDocument doc;
    doc.setHtml(html);
    doc.print(&printer);
    return true;
}

// The system's print dialog, then prints a PDF file (an attachment). Where
// the printing system is CUPS (Linux, BSD), the file goes to it as it is, with
// the choices made in the dialog (CUPS prints PDFs itself, quickly and sharply);
// elsewhere, or for "print to file", its pages are drawn here. False when
// cancelled or when the file can't be read.
#include <QtPdf/QPdfDocument>
#include <QtCore/QDir>
#include <QtGui/QPainter>
#include <QtGui/QImage>
#include <QtGui/QPainter>
#include <QtCore/QElapsedTimer>
#include <QtCore/QFile>
#include <QtCore/QProcess>
#include <QtCore/QStandardPaths>
#include <cstdio>
inline void craftPrintMark(QElapsedTimer& t, const char* what)
{
    if (qEnvironmentVariableIsSet("ALMANAC_TIMING"))
        std::fprintf(stderr, "almanac print: %6lld ms  %s\n", static_cast<long long>(t.elapsed()), what);
}
inline bool craftPrintPdfFile(const QString& path, const QString& title)
{
    QElapsedTimer t;
    t.start();
    if (!QFile::exists(path))
        return false;
    QPrinter printer(QPrinter::HighResolution);
    printer.setDocName(title);
    QPrintDialog dialog(&printer);
    dialog.setWindowTitle(QStringLiteral("Print ") + title);
    dialog.setMinMax(1, 9999);
    craftPrintMark(t, "dialog made");
    if (dialog.exec() != QDialog::Accepted)
        return false;
    craftPrintMark(t, "dialog closed");
    // Print to a PDF file: it already is one.
    if (printer.outputFormat() == QPrinter::PdfFormat && !printer.outputFileName().isEmpty()) {
        QFile::remove(printer.outputFileName());
        return QFile::copy(path, printer.outputFileName());
    }
    const QString lp = QStandardPaths::findExecutable(QStringLiteral("lp"));
    if (!lp.isEmpty() && !printer.printerName().isEmpty()) {
        QStringList args = { QStringLiteral("-d"), printer.printerName(), QStringLiteral("-t"), title,
                             QStringLiteral("-n"), QString::number(qMax(1, printer.copyCount())) };
        if (printer.printRange() == QPrinter::PageRange && printer.fromPage() > 0)
            args << QStringLiteral("-P") << QStringLiteral("%1-%2").arg(printer.fromPage()).arg(qMax(printer.fromPage(), printer.toPage()));
        if (printer.duplex() == QPrinter::DuplexLongSide)
            args << QStringLiteral("-o") << QStringLiteral("sides=two-sided-long-edge");
        else if (printer.duplex() == QPrinter::DuplexShortSide)
            args << QStringLiteral("-o") << QStringLiteral("sides=two-sided-short-edge");
        if (printer.colorMode() == QPrinter::GrayScale)
            args << QStringLiteral("-o") << QStringLiteral("print-color-mode=monochrome");
        args << path;
        return QProcess::startDetached(lp, args);
    }
    // No CUPS: each page drawn and printed here.
    QPdfDocument doc;
    if (doc.load(path) != QPdfDocument::Error::None || doc.pageCount() < 1)
        return false;
    const int from = printer.fromPage() > 0 ? printer.fromPage() - 1 : 0;
    const int to = printer.toPage() > 0 ? qMin(printer.toPage(), doc.pageCount()) - 1 : doc.pageCount() - 1;
    QPainter painter;
    if (!painter.begin(&printer))
        return false;
    for (int i = from; i <= to; ++i) {
        if (i > from)
            printer.newPage();
        // Each page as big as the paper allows, drawn at 300 dpi.
        const QRect area = painter.viewport();
        const QSizeF points = doc.pagePointSize(i);
        const QSize fit = points.scaled(QSizeF(area.size()), Qt::KeepAspectRatio).toSize();
        const QSize pixels = (points * (300.0 / 72.0)).toSize();
        const QImage image = doc.render(i, pixels);
        painter.drawImage(QRect(area.x() + (area.width() - fit.width()) / 2, area.y(), fit.width(), fit.height()), image);
    }
    painter.end();
    return true;
}

// One page of a PDF as a picture (`out`, PNG), `width` pixels wide, for a PDF
// shown in a message. Returns how many pages the PDF has (0: not readable).
// (Qt's PDF reader works on the main thread only: called a page at a time so
// the window keeps going.)
inline int craftPdfPage(const QString& path, int index, const QString& out, int width)
{
    QPdfDocument doc;
    if (doc.load(path) != QPdfDocument::Error::None || doc.pageCount() < 1)
        return 0;
    if (index < doc.pageCount()) {
        const QSizeF points = doc.pagePointSize(index);
        if (points.width() > 0) {
            const QSize pixels(width, qMax(1, int(width * points.height() / points.width())));
            const QImage page = doc.render(index, pixels);
            // On white (pages with no background of their own would be see-through).
            QImage white(page.size(), QImage::Format_RGB32);
            white.fill(Qt::white);
            QPainter painter(&white);
            painter.drawImage(0, 0, page);
            painter.end();
            white.save(out, "PNG");
        }
    }
    return doc.pageCount();
}

inline bool craftSaveWindow(const QString& path)
{
    const auto windows = QGuiApplication::topLevelWindows();
    for (QWindow* w : windows) {
        auto* quick = qobject_cast<QQuickWindow*>(w);
        if (quick && quick->isVisible() && quick->width() > 400) {
            return quick->grabWindow().save(path, "PNG");
        }
    }
    return false;
}

