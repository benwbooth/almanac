#!/usr/bin/env bash
#
# Change -> rebuild -> restart loop for Almanac, via devloop
# (~/src/home-services/devloop). The same as opening Almanac from its
# launcher, but with the log shown here; Ctrl+C stops it.
#
# Every change under src/, qml/, icons/ or include/ (or to build.rs /
# Cargo.toml) rebuilds Almanac incrementally; when the build succeeds the
# running Almanac is restarted, and a failed build leaves it running. The build
# in target/debug is release-grade (see [profile.dev] in Cargo.toml) and is the
# one the launcher runs, so there's no separate install step.
#
# Usage: scripts/dev.sh [--scratch] [-- <almanac args>]
#   --scratch   run against a throwaway data/config folder instead of your
#               real mail (kept in target/dev-scratch between restarts)

set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

args=()
while (( $# )); do
  case "$1" in
    --scratch)
      export XDG_DATA_HOME="$root/target/dev-scratch/data"
      export XDG_CONFIG_HOME="$root/target/dev-scratch/config"
      mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME"
      ;;
    --) shift; args=("$@"); break ;;
    *) echo "unknown option $1 (see the top of $0)" >&2; exit 2 ;;
  esac
  shift
done

exec "$HOME/src/home-services/devloop/devloop" dev almanac "${args[@]}"
