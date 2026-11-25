import QtQuick

// almanac's right-click and drop-down menus: the shared macOS 26 menu
// (MacMenu, from hearth/shared/app-menus) in almanac's theme. Rows are
// MacMenuItem and MacMenuSeparator; nested AMenus are submenus.
// (They were Qt.labs.platform menus, drawn by the desktop style.)
MacMenu {
    dark: Theme.dark
    accent: Theme.accentFill
}
