import QtQuick
import QtQuick.Shapes

// Something being worked on: an arc that turns (on the render thread, so it
// keeps turning while the window is busy) and grows and shrinks as it goes.
// Shown a moment after it starts (work done at once never flashes it), and
// faded out when done. Used everywhere instead of BusyIndicator, which some
// desktop styles drew standing still.
Item {
    id: sp
    property bool running: true
    property color color: Theme.accent
    property real thickness: Math.max(1.6, width / 9)
    // How long it waits before showing (ms).
    property int delay: 150
    implicitWidth: 24
    implicitHeight: 24

    property bool shown: false
    opacity: shown ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 180 } }
    onRunningChanged: if (running) showLater.restart(); else { showLater.stop(); shown = false }
    Component.onCompleted: if (running) { if (delay <= 0) shown = true; else showLater.restart() }
    Timer { id: showLater; interval: Math.max(1, sp.delay); onTriggered: sp.shown = sp.running }
    readonly property bool animating: sp.running && sp.visible && sp.opacity > 0

    Shape {
        id: ring
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer
        // Where the arc starts (turned on by each cycle, so it never jumps).
        property real base: 0
        property real lead: 0
        ShapePath {
            strokeColor: sp.color
            strokeWidth: sp.thickness
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            PathAngleArc {
                id: arc
                centerX: ring.width / 2
                centerY: ring.height / 2
                radiusX: (Math.min(ring.width, ring.height) - sp.thickness) / 2
                radiusY: radiusX
                startAngle: ring.base + ring.lead
                sweepAngle: 30
            }
        }
        RotationAnimator on rotation {
            from: 0; to: 360
            duration: 1100
            loops: Animation.Infinite
            running: sp.animating
        }
    }
    // The arc growing to most of the ring, then its tail catching up.
    SequentialAnimation {
        running: sp.animating
        loops: Animation.Infinite
        NumberAnimation { target: arc; property: "sweepAngle"; from: 30; to: 270; duration: 650; easing.type: Easing.InOutCubic }
        ParallelAnimation {
            NumberAnimation { target: arc; property: "sweepAngle"; from: 270; to: 30; duration: 650; easing.type: Easing.InOutCubic }
            NumberAnimation { target: ring; property: "lead"; from: 0; to: 240; duration: 650; easing.type: Easing.InOutCubic }
        }
        ScriptAction { script: { ring.base = (ring.base + 240) % 360; ring.lead = 0 } }
    }
}
