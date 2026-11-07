#!/usr/bin/env bash
#
# Change -> rebuild -> restart loop for Almanac.
#
# Any change under src/, qml/, icons/ or include/ (or to build.rs / Cargo.toml)
# rebuilds Almanac. The QML is compiled into the binary, so QML edits need a
# rebuild too. Only when the build succeeds is the running Almanac stopped and
# the new one started; a failed build leaves the old one running.
#
# Usage: scripts/dev.sh [--release] [--scratch] [-- <almanac args>]
#   --release   build with --release (slower to build, faster to run)
#   --scratch   run against a throwaway data/config folder instead of your
#               real mail (kept in target/dev-scratch between restarts)
# App output goes to target/dev-almanac.log.
#
# Almanac runs one copy per data folder: if another Almanac (not started by
# this script) is using the same folder, the new one just brings that window
# forward and exits. Quit the other one first, or use --scratch.

set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 1

# Re-enter through the dev shell when its tools aren't on PATH.
if ! command -v watchexec >/dev/null || [[ -z "${QMAKE:-}" && ! -e "$root/.qt-env" ]]; then
  exec nix develop "$root" -c "$0" "$@"
fi

profile=debug
cargo_flags=()
scratch=0
step=""
app_args=()
while (( $# )); do
  case "$1" in
    --release) profile=release; cargo_flags+=(--release) ;;
    --scratch) scratch=1 ;;
    --step) step=1 ;;
    --) shift; app_args=("$@"); break ;;
    *) echo "unknown option $1 (see the top of $0)" >&2; exit 2 ;;
  esac
  shift
done

mkdir -p "$root/target"
pidfile="$root/target/dev-almanac.pid"
log="$root/target/dev-almanac.log"
bin="$root/target/$profile/almanac"

if (( scratch )); then
  export XDG_DATA_HOME="$root/target/dev-scratch/data"
  export XDG_CONFIG_HOME="$root/target/dev-scratch/config"
  mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME"
fi

stamp() { printf '\033[2m%s\033[0m %s\n' "$(date +%T)" "$*"; }

running() { [[ -f "$pidfile" ]] && kill -0 "$(cat "$pidfile")" 2>/dev/null; }

stop_app() {
  running || return 0
  local pid; pid="$(cat "$pidfile")"
  # TERM first so Almanac can save its window and state; KILL if it hangs.
  kill "$pid" 2>/dev/null
  for _ in $(seq 50); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
  kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null
  rm -f "$pidfile"
}

rebuild_and_restart() {
  stamp "building ($profile)"
  if ! cargo build "${cargo_flags[@]}"; then
    stamp "build failed; keeping the running Almanac"
    return
  fi
  stop_app
  # Its own session, so watchexec never takes it down along with a build step.
  setsid "$bin" "${app_args[@]}" >>"$log" 2>&1 </dev/null &
  echo $! >"$pidfile"
  sleep 1
  if running; then
    stamp "Almanac running (pid $(cat "$pidfile"), log: target/dev-almanac.log)"
  else
    stamp "Almanac exited right away; last lines of the log:"
    tail -5 "$log" >&2
  fi
}

# One-shot step that the watcher below calls.
if [[ -n "$step" ]]; then
  rebuild_and_restart
  exit 0
fi

cleanup() {
  trap - EXIT INT TERM
  stop_app
  kill 0 2>/dev/null
}
trap cleanup EXIT INT TERM

step_args=(--step)
[[ $profile == release ]] && step_args+=(--release)
(( scratch )) && step_args+=(--scratch)

rebuild_and_restart

watchexec --quiet --postpone --debounce 300ms --on-busy-update=queue \
  -w src -w qml -w icons -w include -w build.rs -w Cargo.toml -w Cargo.lock \
  -- "$0" "${step_args[@]}" -- "${app_args[@]}"
