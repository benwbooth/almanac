#pragma once
// Paragraphs in the message editor (QML can't reach a text block's format).
//
// Enter starts a new paragraph with a gap under the one before, like a
// word processor; the gap is the block's bottom margin, so it shows while
// writing and goes out in the HTML (Qt writes it as margin-bottom).
// Shift+Enter stays Qt's own line break inside the paragraph.

#include <QtCore/QObject>
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

private:
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
