import QtQuick
import QtQuick.Controls

// Scrolling with momentum for a Flickable or ListView (Linux trackpads and
// mouse wheels don't glide by themselves): the list follows the fingers,
// then glides on and slows down. Another swipe (or wheel turn) the same way
// while it glides keeps it going and adds to its speed.
//
// Adapted from AcceleratedWheelHandler in lunchpail (MIT, © 2026 Ben Booth):
// the glide is driven here frame by frame (not with Flickable.flick, whose
// own wheel handling fights it), and it's attached to the list itself.
WheelHandler {
    id: m
    required property Flickable flick
    parent: flick
    target: null
    // The events are this handler's alone: the list's own wheel scrolling moving
    // it at the same time made every other flick's glide stop (seen in the trace).
    blocking: true
    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad

    // Glide: speed in px/s (positive: down the list), slowed exponentially.
    property real velocity: 0
    property real friction: 3.2
    property real minimumVelocity: 30
    property real maximumVelocity: 12000
    // From the wheel (a free-spinning one sends notches for seconds): at most this.
    property real maximumWheelVelocity: 6000
    // However it's fed, a glide ends by then (ms): the list never keeps going.
    property int longestGlide: 2500
    property double glideStartedAt: 0
    // A swipe in progress, and its smoothed speed.
    property bool swiping: false
    property double lastPixelAt: 0
    property real swipeVelocity: 0
    property int swipeSamples: 0
    // Wheel notches close together accelerate.
    property double lastNotchAt: 0
    property real burst: 0
    // ---- Pull to refresh (set `pullable`): scrolling up past the top pulls the
    // list down (harder the further it goes); let go past `pullTrigger`
    // pixels, refreshRequested. As on iOS and macOS: once the list is at the
    // top and has stopped there (a moment), more scrolling up pulls, in the
    // same gesture too. A glide running into the top (ours, or the system's
    // momentum) never pulls; a mouse wheel only after a moment at the top.
    property bool pullable: false
    property real pull: 0
    property real pullTrigger: 60
    // Held open (at the spinner's height) while the mail is being checked.
    property bool refreshing: false
    signal refreshRequested()
    // When the list got to the top (1: at the top from the start; -1: not there).
    property double atTopSince: 1
    property bool gestureFromTop: false
    property double lastRefreshAt: 0
    property double lastWheelAt: 0
    function atTop() { return flick.contentY <= lowest() + 0.5 }
    function pullBy(d) {
        // (Up: d < 0. Rubber band: each pixel counts for less the further it's pulled.)
        var resist = Math.max(0.15, 1 - pull / 140)
        pull = Math.max(0, Math.min(140, pull - d * 0.6 * resist))
    }
    function releasePull() {
        pullRelease.stop()
        if (pull >= pullTrigger && !refreshing && Date.now() - lastRefreshAt > 1500) {
            lastRefreshAt = Date.now()
            refreshRequested()
        } else if (pull > 2 && !refreshing) {
            refused(pull >= pullTrigger ? "pulled again too soon" : "let go at " + Math.round(pull) + " of " + pullTrigger + " px")
        }
        springTo()
    }
    // Pulls that didn't refresh, in the log (once per gesture), to see why one
    // was missed: "ALMANAC-PULL refused: <why>".
    property string refusedWhy: ""
    function refused(why) {
        var kind = why.replace(/[0-9]+/g, "")
        if (kind === refusedWhy) return
        refusedWhy = kind
        console.warn("ALMANAC-PULL refused: " + why)
    }
    // Back to closed, or to the spinner's height while checking. (The target
    // set here: a binding on it updated too late when checking ended.)
    function springTo() { springBack.stop(); springBack.to = refreshing ? 44 : 0; springBack.start() }
    // Let go: when the gesture ends (or, from trackpads that don't say, a pause).
    property Timer pullRelease: Timer { interval: 250; onTriggered: m.releasePull() }
    property NumberAnimation springBack: NumberAnimation {
        target: m; property: "pull"; duration: 260; easing.type: Easing.OutCubic
        to: 0
    }
    onRefreshingChanged: if (!pullRelease.running) springTo()

    // Our own moves (so other moves, e.g. a scroll bar drag, stop the glide).
    property bool moving: false
    property real lastSetY: NaN

    function lowest() { return flick.originY - flick.topMargin }
    function highest() { return Math.max(lowest(), flick.originY + flick.contentHeight - flick.height + flick.bottomMargin) }
    function moveBy(d) {
        if (!isFinite(d) || d === 0) return
        moving = true
        lastSetY = Math.max(lowest(), Math.min(highest(), flick.contentY + d))
        flick.contentY = lastSetY
        moving = false
        showBar()
    }
    // Setting contentY doesn't count as the list moving, so its scroll bar
    // wouldn't show: show it while scrolling, and let it fade a moment after.
    readonly property ScrollBar bar: flick.ScrollBar.vertical
    function showBar() { if (!bar) return; bar.active = true; barTimer.restart() }
    property Timer barTimer: Timer {
        interval: 300
        onTriggered: if (m.bar && !m.bar.hovered && !m.bar.pressed) m.bar.active = false
    }
    function stop(why) { if (glide.running) note("stop " + (why || "")); glide.stop(); velocity = 0 }
    // Started with --trace-scroll: every swipe is written to the log
    // ("ALMANAC-SCROLL list …"), to see why a glide stopped.
    readonly property bool trace: Qt.application.arguments.indexOf("--trace-scroll") >= 0
    property var hist: []
    // (The last events are kept anyway: a glide that runs long writes them to the log.)
    property var recent: []
    function note(x) {
        recent.push(Date.now() % 100000 + " " + x)
        if (recent.length > 40) recent.shift()
        if (!trace) return
        hist.push(Date.now() % 100000 + " " + x); if (!traceTimer.running) traceTimer.start()
    }
    property Timer traceTimer: Timer {
        interval: 1500
        onTriggered: { if (m.hist.length) console.warn("ALMANAC-SCROLL list " + m.hist.join(" ; ")); m.hist = [] }
    }
    function addVelocity(v, cap) {
        if (!isFinite(v) || v === 0) return
        if (velocity * v < 0) velocity = 0
        var most = cap || maximumVelocity
        velocity = Math.max(-most, Math.min(most, velocity + v))
        glideStartedAt = Date.now()
        if (!glide.running) glide.start()
    }

    // ------------------------------------------------ trackpad
    // A glide going on when the fingers come down keeps going while they move
    // the same way (their movement adds to it); the other way stops it.
    function beginSwipe() {
        swiping = true
        swipeVelocity = 0; swipeSamples = 0; lastPixelAt = 0
    }
    function swipeBy(d, phased) {
        if (!swiping) beginSwipe()
        if (glide.running && velocity * d < 0) stop("other way")
        var now = Date.now(), dt = now - lastPixelAt
        if (lastPixelAt > 0 && dt <= 120) {
            var sample = d / (Math.max(8, dt) / 1000)
            swipeVelocity = swipeSamples ? swipeVelocity * 0.35 + sample * 0.65 : sample
        } else swipeVelocity = d * 60
        if (swipeVelocity * d < 0) swipeVelocity = 0
        swipeSamples++
        lastPixelAt = now
        moveBy(d)
        // Trackpads that don't say when the fingers lift: a short pause ends the swipe.
        if (phased) releaseTimer.stop(); else releaseTimer.restart()
    }
    function endSwipe() {
        releaseTimer.stop()
        if (!swiping) return
        swiping = false
        // (A quick flick of only a couple of events counts too.)
        var v = swipeSamples >= 1 && Date.now() - lastPixelAt <= 200 ? swipeVelocity : 0
        note("end v" + Math.round(v) + " n" + swipeSamples + " glide " + Math.round(velocity))
        // Added to a glide still going the same way: it picks up speed.
        if (Math.abs(v) >= minimumVelocity) addVelocity(v)
    }

    // ------------------------------------------------ mouse wheel
    function notches(steps) {
        var now = Date.now(), dir = steps < 0 ? -1 : 1
        burst = now - lastNotchAt <= 250 && dir * velocity >= 0 ? Math.min(8, burst + Math.abs(steps)) : 0
        lastNotchAt = now
        var accel = 1 + burst * 0.5
        // Part at once (it answers straight away), the rest as glide.
        moveBy(steps * 50 * Math.min(2, accel))
        addVelocity(steps * 70 * friction * accel, maximumWheelVelocity)
    }

    function advance(dt) {
        if (velocity === 0) { stop(); return }
        var age = Date.now() - glideStartedAt
        if (age > 2200 && !loggedLong) { loggedLong = true; console.warn("ALMANAC-SCROLL long glide v" + Math.round(velocity) + ": " + recent.join(" ; ")) }
        if (age > longestGlide) { stop("too long"); return }
        dt = Math.max(0.001, Math.min(0.05, dt))
        var decay = Math.exp(-friction * dt)
        var before = flick.contentY
        moveBy(velocity * (1 - decay) / friction)
        velocity *= decay
        var atEdge = Math.abs(flick.contentY - before) < 0.01
                     && ((velocity < 0 && flick.contentY <= lowest()) || (velocity > 0 && flick.contentY >= highest()))
        if (atEdge || Math.abs(velocity) < minimumVelocity) stop(atEdge ? "edge" : "slowed")
    }

    onWheel: function(ev) { handle(ev) }
    // (Also called by the checks with made-up events: {phase, pixelDelta: {y},
    // angleDelta: {y}, device: {type}}; sets ev.accepted.)
    function handle(ev) {
        var phase = ev.phase
        note("w " + ["none", "begin", "update", "end", "momentum"][phase] + " px" + ev.pixelDelta.y + " a" + ev.angleDelta.y
                        + (ev.device && ev.device.type === PointerDevice.TouchPad ? " pad" : " mouse") + (glide.running ? " g" + Math.round(velocity) : ""))
        if (phase === Qt.ScrollEnd) {
            refusedWhy = ""
            if (pull > 0) { releasePull(); gestureFromTop = false; ev.accepted = true; return }
            var was = swiping; endSwipe(); ev.accepted = was; return
        }
        if (phase === Qt.ScrollBegin) { gestureFromTop = pullable && atTop() && !glide.running; refusedWhy = "" }
        // (Wayland trackpads may start with an update, no begin: a gesture's
        // first event after a pause counts as its beginning.)
        if (phase !== Qt.ScrollMomentum && Date.now() - lastWheelAt > 250) { gestureFromTop = pullable && atTop() && !glide.running; refusedWhy = "" }
        lastWheelAt = Date.now()
        // The system's own momentum (macOS): it has its own glide.
        if (phase === Qt.ScrollMomentum) {
            stop()
            if (pull === 0) moveBy(-ev.pixelDelta.y)
            else if (pullable && ev.pixelDelta.y > 0) refused("momentum (fingers lifted)")
            ev.accepted = ev.pixelDelta.y !== 0; return
        }
        // (A list too short to scroll still pulls to refresh.)
        var canScroll = flick.flickableDirection !== Flickable.HorizontalFlick && (flick.contentHeight > flick.height || pullable)
        if (!canScroll) { ev.accepted = false; return }
        if (phase === Qt.ScrollBegin) beginSwipe()
        var touchpad = ev.device && ev.device.type === PointerDevice.TouchPad
        var pixels = ev.pixelDelta.y !== 0 && (touchpad || phase !== Qt.NoScrollPhase || ev.angleDelta.y === 0)
        var d = pixels ? -ev.pixelDelta.y : -ev.angleDelta.y
        // Fingers down without moving yet: the glide goes on until they move.
        if (d === 0) { ev.accepted = phase === Qt.ScrollBegin; return }
        // Pulled down already: scrolling back down eases it first.
        if (pull > 0 && d > 0) { springBack.stop(); pull = Math.max(refreshing ? 44 : 0, pull - d); restartRelease(phase); ev.accepted = true; return }
        // Past the top, from a list that was already there (a trackpad gesture
        // begun there, or the wheel after a moment at the top): a pull.
        if (pullable && d < 0 && atTop()) {
            // Deliberate: the gesture began at the top, or the list has been
            // stopped at the top a moment (100 ms; 400 ms for a wheel's notches).
            var still = Date.now() - atTopSince
            var settled = atTopSince > 0 && still > (pixels ? 100 : 400)
            if (glide.running && velocity < 0 && pull === 0) {
                // A glide running into the top: stopped there (it has arrived);
                // the scrolling after it pulls once it has settled.
                stop("top")
                refused("a glide ran into the top")
            } else if (gestureFromTop || settled) {
                // (The scrolling that got here is over: no glide after it.)
                if (swiping) { releaseTimer.stop(); swiping = false }
                stop()
                springBack.stop(); pullBy(pixels ? d : d / 120 * 22); restartRelease(phase); ev.accepted = true; return
            } else refused("only just at the top (" + Math.max(0, still) + " ms)")
        }
        if (pixels) swipeBy(d, phase !== Qt.NoScrollPhase)
        else notches(d / 120)
        ev.accepted = true
    }

    function restartRelease(phase) {
        // (Gestures that say when they end are let go then; a pause is a backstop.)
        pullRelease.interval = phase === Qt.NoScrollPhase ? 250 : 900
        pullRelease.restart()
    }

    property bool loggedLong: false
    property FrameAnimation glide: FrameAnimation { onTriggered: m.advance(frameTime); onRunningChanged: if (running) m.loggedLong = false }
    // A click or touch anywhere on the list stops a glide at once (passive:
    // the rows still get their clicks).
    property PointHandler press: PointHandler {
        parent: m.flick
        target: null
        acceptedButtons: Qt.AllButtons
        onActiveChanged: if (active) m.stop("pressed")
    }
    property Timer releaseTimer: Timer { interval: 60; onTriggered: m.endSwipe() }
    property Connections watch: Connections {
        target: m.flick
        // Dragged, or moved by something else (a scroll bar, keyboard, jumping
        // to a message): the glide stops.
        function onDraggingChanged() { if (m.flick.dragging) m.stop("dragged") }
        function onContentYChanged() {
            if (m.atTop()) { if (m.atTopSince < 0) m.atTopSince = Date.now() } else m.atTopSince = -1
            if (!m.moving && m.glide.running && Math.abs(m.flick.contentY - m.lastSetY) > 1) m.stop("moved by something else")
        }
    }
}
