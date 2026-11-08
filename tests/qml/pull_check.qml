import QtQuick
import "../../qml"

// Pull to refresh with made-up trackpad and wheel events (run by the Rust
// test `pull_to_refresh_gestures`): prints "PULL-CHECK ok <name>" or
// "PULL-CHECK FAIL <name>: why" for each, then quits (1 if any failed).
Item {
    id: root
    width: 300; height: 400
    ListView {
        id: list
        anchors.fill: parent
        model: 100
        delegate: Rectangle { width: 300; height: 40 }
        boundsBehavior: Flickable.StopAtBounds
        interactive: false
    }
    Momentum {
        id: m
        flick: list
        pullable: true
        onRefreshRequested: root.refreshes++
    }
    property int refreshes: 0
    property int failed: 0
    readonly property var pad: ({ type: PointerDevice.TouchPad })
    readonly property var mouse: ({ type: PointerDevice.Mouse })
    function ev(phase, px, dev, angle) { return { phase: phase, pixelDelta: { y: px }, angleDelta: { y: angle || 0 }, device: dev, accepted: false } }
    function check(name, ok, why) {
        console.warn("PULL-CHECK " + (ok ? "ok " + name : "FAIL " + name + ": " + why))
        if (!ok) failed++
    }
    // Each scenario: steps of [ms to wait first, function].
    property var steps: []
    property int at: 0
    function reset(y) { m.stop(); m.pull = 0; m.springBack.stop(); m.pullRelease.stop(); m.lastRefreshAt = 0; list.contentY = y; refreshes = 0 }
    function scenario(name, setup, events, gap, wait, expect) {
        steps.push([300, function() { setup() }])
        for (var i = 0; i < events.length; i++) {
            (function(e) { steps.push([gap, function() { m.handle(e) }]) })(events[i])
        }
        steps.push([wait, function() { expect(name) }])
    }
    function repeat(n, make) { var out = []; for (var i = 0; i < n; i++) out.push(make(i)); return out }

    Component.onCompleted: {
        // Scrolled up to the top, then more in the same gesture (a trackpad that
        // sends no phases): it pulls, and letting go refreshes.
        scenario("scroll up to the top, keep pulling (no phases)", function() { reset(300) },
                 repeat(40, function() { return ev(Qt.NoScrollPhase, 20, root.pad) }), 16, 600,
                 function(n) { check(n, refreshes === 1 && list.contentY <= list.originY + 0.5, "refreshes " + refreshes + " contentY " + list.contentY) })
        // The same with a trackpad that says when the gesture begins and ends.
        scenario("scroll up to the top, keep pulling (phases)", function() { reset(300) },
                 [ev(Qt.ScrollBegin, 0, root.pad)].concat(repeat(40, function() { return ev(Qt.ScrollUpdate, 20, root.pad) })).concat([ev(Qt.ScrollEnd, 0, root.pad)]), 16, 600,
                 function(n) { check(n, refreshes === 1, "refreshes " + refreshes) })
        // Begun at the top: pulls at once.
        scenario("pull from the top", function() { reset(0) },
                 [ev(Qt.ScrollBegin, 0, root.pad)].concat(repeat(15, function() { return ev(Qt.ScrollUpdate, 20, root.pad) })).concat([ev(Qt.ScrollEnd, 0, root.pad)]), 16, 600,
                 function(n) { check(n, refreshes === 1, "refreshes " + refreshes) })
        // A short pull: springs back, nothing checked.
        scenario("short pull", function() { reset(0) },
                 [ev(Qt.ScrollBegin, 0, root.pad)].concat(repeat(3, function() { return ev(Qt.ScrollUpdate, 10, root.pad) })).concat([ev(Qt.ScrollEnd, 0, root.pad)]), 16, 600,
                 function(n) { check(n, refreshes === 0 && m.pull === 0, "refreshes " + refreshes + " pull " + m.pull) })
        // A fling into the top (fingers lifted, the glide arrives): no pull.
        scenario("fling into the top", function() { reset(1500); m.addVelocity(-9000) },
                 [], 16, 1200,
                 function(n) { check(n, refreshes === 0 && m.pull === 0 && list.contentY <= list.originY + 0.5, "refreshes " + refreshes + " pull " + m.pull + " contentY " + list.contentY) })
        // A swipe that flings into the top: its last events while the glide
        // arrives don't pull.
        scenario("swipe flung into the top", function() { reset(600) },
                 repeat(6, function() { return ev(Qt.NoScrollPhase, 60, root.pad) }), 8, 900,
                 function(n) { check(n, refreshes === 0 && m.pull === 0, "refreshes " + refreshes + " pull " + m.pull) })
        // The system's momentum (fingers lifted) at the top: never a pull.
        scenario("momentum at the top", function() { reset(0) },
                 [ev(Qt.ScrollBegin, 0, root.pad), ev(Qt.ScrollEnd, 0, root.pad)].concat(repeat(30, function() { return ev(Qt.ScrollMomentum, 25, root.pad) })), 16, 600,
                 function(n) { check(n, refreshes === 0 && m.pull === 0, "refreshes " + refreshes + " pull " + m.pull) })
        // A mouse wheel: notches at the top after a moment there pull.
        scenario("wheel at the top", function() { reset(0) },
                 repeat(8, function() { return ev(Qt.NoScrollPhase, 0, root.mouse, 120) }), 40, 700,
                 function(n) { check(n, refreshes === 1, "refreshes " + refreshes) })
        steps.push([0, function() { console.warn("PULL-CHECK done " + (failed ? "failed" : "passed")); Qt.exit(failed ? 1 : 0) }])
        runner.start()
    }
    Timer {
        id: runner
        interval: 1
        onTriggered: {
            var s = root.steps[root.at++]
            s[1]()
            if (root.at < root.steps.length) { interval = Math.max(1, root.steps[root.at][0]); start() }
        }
    }
}
