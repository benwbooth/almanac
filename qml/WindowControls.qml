import QtQuick

// Minimise, maximise/restore and close at the right end of the top bar (the
// top bar is the window's title bar): the shared ChromeButtons
// (hearth/shared/app-menus), drawn as arbor draws Breeze's on its title bars,
// the same in all of Ben's apps. macOS keeps its own buttons on the left.
ChromeButtons {
    fg: Theme.text
    bg: Theme.toolbar
}
