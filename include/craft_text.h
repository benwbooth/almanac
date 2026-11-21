#pragma once
// Paragraphs in the message editor (QML can't reach a text block's format).
//
// Enter starts a new paragraph with a gap under the one before, like a
// word processor; the gap is the block's bottom margin, so it shows while
// writing and goes out in the HTML (Qt writes it as margin-bottom).
// Shift+Enter stays Qt's own line break inside the paragraph.

#include <QtCore/QMimeData>
#include <QtCore/QObject>
#include <QtGui/QClipboard>
#include <QtGui/QFontDatabase>
#include <QtGui/QGuiApplication>
#include <QtGui/QPalette>
#include <QtGui/QTextDocumentFragment>
#include <QtGui/QTextFrame>
#include <QtGui/QTextBlock>
#include <QtGui/QTextCursor>
#include <QtGui/QTextDocument>
#include <QtGui/QTextList>
#include <QtGui/QTextTable>
#include <QtQml/QQmlEngine>
#include <QtQuick/QQuickTextDocument>

class CraftText : public QObject {
    Q_OBJECT
public:
    using QObject::QObject;

    // A new paragraph at the editor's cursor (replacing what's selected),
    // `gap` pixels under the current one. Returns where the cursor goes, or -1
    // where the paragraph isn't plain text (a list item, a heading, a table or
    // code block, a rule): Qt's own Enter does the right thing there.
    Q_INVOKABLE int newParagraph(QObject* edit, int gap)
    {
        QTextDocument* doc = document(edit);
        if (!doc)
            return -1;
        const int a = edit->property("selectionStart").toInt();
        const int b = edit->property("selectionEnd").toInt();
        QTextCursor c(doc);
        c.setPosition(a);
        c.setPosition(b, QTextCursor::KeepAnchor);
        if (!plain(c, a) || !plain(c, b))
            return -1;
        c.beginEditBlock();
        if (c.hasSelection())
            c.removeSelectedText();
        QTextBlockFormat f = c.blockFormat();
        if (f.bottomMargin() < gap) {
            f.setBottomMargin(gap);
            c.setBlockFormat(f);
        }
        c.insertBlock(f, c.charFormat());
        c.endEditBlock();
        return c.position();
    }

    // The editor's text with a blank line after each spaced paragraph (the
    // plain-text copy of the message, read where HTML isn't shown).
    Q_INVOKABLE QString plainText(QObject* edit)
    {
        QTextDocument* doc = document(edit);
        if (!doc)
            return QString();
        QString out;
        for (QTextBlock blk = doc->begin(); blk.isValid(); blk = blk.next()) {
            if (blk != doc->begin())
                out += QLatin1Char('\n');
            out += blk.text().replace(QChar::LineSeparator, QLatin1Char('\n'));
            const QTextBlock next = blk.next();
            if (next.isValid() && blk.blockFormat().bottomMargin() > 0 && !blk.textList()
                && !QTextCursor(blk).currentTable() && !blk.text().isEmpty() && !next.text().isEmpty())
                out += QLatin1Char('\n');
        }
        return out;
    }

    // Pastes what's on the clipboard when it's formatted (HTML, as copied
    // from an email or a web page) without its colours, backgrounds, fonts
    // and sizes, so it takes on the message's own look (an email's black
    // text on white would otherwise come along into a dark editor). Bold,
    // italics, underline, links, lists, quotes, headings, tables and
    // fixed-width text stay. Returns where the cursor goes, or -1 when the
    // clipboard has no HTML (the caller pastes it its own way).
    Q_INVOKABLE int pasteClean(QObject* edit)
    {
        QTextDocument* doc = document(edit);
        const QMimeData* md = QGuiApplication::clipboard()->mimeData();
        // (Copied from the editor itself: Qt's own paste keeps what was chosen there.)
        if (!doc || !md || !md->hasHtml() || md->hasFormat(QStringLiteral("application/x-qrichtext")))
            return -1;
        const QTextDocumentFragment frag = cleanFragment(md->html());
        if (frag.isEmpty())
            return -1;
        QTextCursor c(doc);
        c.setPosition(edit->property("selectionStart").toInt());
        c.setPosition(edit->property("selectionEnd").toInt(), QTextCursor::KeepAnchor);
        c.beginEditBlock();
        c.insertFragment(frag);
        c.endEditBlock();
        return c.position();
    }

    // The HTML as a fragment without its colours, fonts and sizes.
    static QTextDocumentFragment cleanFragment(const QString& html)
    {
        QTextDocument tmp;
        tmp.setHtml(html);
        const QBrush link = QGuiApplication::palette().link();
        for (QTextBlock blk = tmp.begin(); blk.isValid(); blk = blk.next()) {
            QTextCursor bc(blk);
            QTextBlockFormat bf = blk.blockFormat();
            if (bf.hasProperty(QTextFormat::BackgroundBrush) || bf.hasProperty(QTextFormat::ForegroundBrush)) {
                bf.clearBackground();
                bf.clearForeground();
                bc.setBlockFormat(bf);
            }
            // (A list item's bullet and an empty line take the block's own character format.)
            QTextCharFormat bcf = blk.charFormat();
            plainChar(bcf, link);
            bc.setBlockCharFormat(bcf);
            for (auto it = blk.begin(); !it.atEnd(); ++it) {
                const QTextFragment f = it.fragment();
                if (!f.isValid())
                    continue;
                QTextCharFormat cf = f.charFormat();
                if (cf.isImageFormat())
                    continue;
                plainChar(cf, link);
                QTextCursor fc(&tmp);
                fc.setPosition(f.position());
                fc.setPosition(f.position() + f.length(), QTextCursor::KeepAnchor);
                fc.setCharFormat(cf);
            }
        }
        // Table cells and frames (an email's coloured boxes).
        cleanFrame(tmp.rootFrame());
        QTextCursor all(&tmp);
        all.select(QTextCursor::Document);
        return QTextDocumentFragment(all);
    }

private:
    static void plainChar(QTextCharFormat& cf, const QBrush& link)
    {
        const bool mono = fixedPitch(cf);
        cf.clearBackground();
        cf.clearForeground();
        cf.clearProperty(QTextFormat::TextOutline);
        cf.clearProperty(QTextFormat::TextUnderlineColor);
        cf.clearProperty(QTextFormat::FontPointSize);
        cf.clearProperty(QTextFormat::FontPixelSize);
        cf.clearProperty(QTextFormat::FontFamilies);
        cf.clearProperty(QTextFormat::FontStyleHint);
        cf.clearProperty(QTextFormat::FontLetterSpacing);
        cf.clearProperty(QTextFormat::FontWordSpacing);
        if (mono)
            cf.setFontFamilies({QStringLiteral("monospace")});
        if (cf.isAnchor() && !cf.anchorHref().isEmpty())
            cf.setForeground(link);
    }

    static bool fixedPitch(const QTextCharFormat& cf)
    {
        if (cf.fontFixedPitch())
            return true;
        for (const QString& fam : cf.fontFamilies().toStringList()) {
            const QString f = fam.toLower();
            if (f.contains(QLatin1String("mono")) || f.contains(QLatin1String("courier"))
                || f.contains(QLatin1String("consol")) || f.contains(QLatin1String("menlo")))
                return true;
            if (QFontDatabase::hasFamily(fam) && QFontDatabase::isFixedPitch(fam))
                return true;
        }
        return false;
    }

    static void cleanFrame(QTextFrame* frame)
    {
        for (QTextFrame* child : frame->childFrames()) {
            if (auto* table = qobject_cast<QTextTable*>(child)) {
                for (int r = 0; r < table->rows(); ++r)
                    for (int col = 0; col < table->columns(); ++col) {
                        QTextTableCell cell = table->cellAt(r, col);
                        QTextCharFormat cf = cell.format();
                        if (cf.hasProperty(QTextFormat::BackgroundBrush)) {
                            cf.clearBackground();
                            cell.setFormat(cf);
                        }
                    }
            }
            QTextFrameFormat ff = child->frameFormat();
            if (ff.hasProperty(QTextFormat::BackgroundBrush) || ff.hasProperty(QTextFormat::FrameBorderBrush)) {
                ff.clearBackground();
                ff.clearProperty(QTextFormat::FrameBorderBrush);
                child->setFrameFormat(ff);
            }
            cleanFrame(child);
        }
    }

    static QTextDocument* document(QObject* edit)
    {
        if (!edit)
            return nullptr;
        auto* td = edit->property("textDocument").value<QQuickTextDocument*>();
        return td ? td->textDocument() : nullptr;
    }

    static bool plain(QTextCursor c, int pos)
    {
        c.setPosition(pos);
        const QTextBlockFormat f = c.blockFormat();
        return !c.currentList() && !c.currentTable() && f.headingLevel() == 0
            && !f.hasProperty(QTextFormat::BlockTrailingHorizontalRulerWidth);
    }
};

inline void craftRegisterText()
{
    qmlRegisterSingletonType<CraftText>("org.craft.text", 1, 0, "CraftText",
        [](QQmlEngine*, QJSEngine*) -> QObject* { return new CraftText; });
}
