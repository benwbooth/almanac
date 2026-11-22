import QtQuick
import Qt.labs.platform as P

// The app's menus inside the window as a NATIVE menu (Qt.labs.platform: the
// desktop style's own QMenu, with icons and right-aligned shortcuts), from
// the same model GlobalMenuBar exports to the desktop's global menu bar.
// For apps whose ☰ was a native menu before the shared model (almanac,
// tessera); AppMenu.qml is the Qt Quick version. Rebuilt each time it
// opens so texts and states are current.
//     NativeAppMenu { id: appMenu; model: root.menuModel; onTriggered: id => root.menuAction(id) }
//     onClicked: appMenu.openBelow(button)
//
// Menu entries may carry `icon` (theme name) and `iconSource` (URL), like
// items. An item with `inWindow: false` is left out here (it stays in the
// global menu bar). `appMenuAtEnd`: the first menu (the app-name menu)
// isn't a submenu but its items go flat at the bottom, as some apps' ☰
// had them (Preferences…, Quit).
// Shortcuts are shown after a tab (QMenu draws them right-aligned) rather
// than set as the action's shortcut, so they never fire twice: the app's
// own Shortcut objects do the work.
//
// Shared by Ben's Qt Quick apps (keep the copies the same).
P.Menu {
    id: root
    property var model: []
    // Flat: every menu's items inline, a separator between menus.
    property bool flat: false
    property bool appMenuAtEnd: false
    signal triggered(string id)

    function openBelow(item) {
        rebuild()
        open(item)
    }
    function openAt(item, x, y) {
        rebuild()
        open(item)
    }

    property var _objects: []
    function rebuild() {
        clear()
        for (var i = 0; i < _objects.length; i++) _objects[i].destroy()
        _objects = []
        var menus = model || []
        var tail = null
        if (appMenuAtEnd && menus.length > 1) { tail = menus[0]; menus = menus.slice(1) }
        for (var m = 0; m < menus.length; m++) {
            if (flat) {
                if (m > 0) addItem(make(separatorComponent, {}))
                fill(root, menus[m].items || [])
                continue
            }
            var sub = make(submenuComponent, { title: menus[m].title })
            setIcon(sub, menus[m])
            fill(sub, menus[m].items || [])
            addMenu(sub)
        }
        if (tail) {
            addItem(make(separatorComponent, {}))
            fill(root, tail.items || [])
        }
    }
    function make(component, props) {
        var obj = component.createObject(root, props)
        _objects.push(obj)
        return obj
    }
    function setIcon(obj, it) {
        if (it.icon) obj.icon.name = it.icon
        if (it.iconSource) obj.icon.source = it.iconSource
    }
    function fill(menu, items) {
        // No separator first, last or twice in a row (hidden items can leave one).
        items = items.filter(function(it) { return it.inWindow !== false })
        items = items.filter(function(it, i) {
            return !it.separator || (i > 0 && i < items.length - 1 && !items[i - 1].separator)
        })
        for (var i = 0; i < items.length; i++) {
            var it = items[i]
            if (it.separator) {
                menu.addItem(make(separatorComponent, {}))
            } else if (it.items) {
                var sub = make(submenuComponent, { title: it.text, enabled: it.enabled !== false })
                setIcon(sub, it)
                fill(sub, it.items)
                menu.addMenu(sub)
            } else {
                var obj = make(itemComponent, {
                    text: it.text + (it.shortcut ? "\t" + it.shortcut : ""),
                    enabled: it.enabled !== false,
                    checkable: !!it.checkable, checked: !!it.checkable && !!it.checked })
                setIcon(obj, it)
                const id = it.id || ""
                obj.triggered.connect(function() { root.triggered(id) })
                menu.addItem(obj)
            }
        }
    }

    Component { id: separatorComponent; P.MenuSeparator {} }
    Component { id: submenuComponent; P.Menu {} }
    Component { id: itemComponent; P.MenuItem {} }
}
