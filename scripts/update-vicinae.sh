#!/usr/bin/env bash
# Update Vicinae (https://vicinae.com/install) to the latest GitHub release.
#   scripts/update-vicinae.sh              update to the latest release
#   scripts/update-vicinae.sh --check      compare versions and exit
#   scripts/update-vicinae.sh --uninstall  remove Vicinae
#
# The official installer does the heavy lifting (downloads the AppImage, extracts it to
# /usr/local/lib/vicinae, re-installs desktop files, themes, the systemd unit) and prompts
# for sudo by itself. It never restarts the running daemon though, so after a version bump
# this wrapper reloads and restarts vicinae.service for the current user.
set -u

REPO=vicinaehq/vicinae
INSTALLER_URL=https://vicinae.com/install
INSTALLER_FALLBACK="https://raw.githubusercontent.com/$REPO/main/scripts/install.sh"
SERVICE=vicinae.service

usage() {
  cat <<'EOF'
Usage: update-vicinae.sh [--check | --uninstall]
  (no args)     update Vicinae to the latest release (prompts for sudo)
  --check       compare installed and latest versions, change nothing
  --uninstall   remove Vicinae
EOF
}

installed_version() {
  command -v vicinae >/dev/null 2>&1 || return 1
  vicinae version 2>/dev/null | sed -n 's/^Version v\([0-9][0-9.]*\).*/\1/p' | head -1
}

# Latest release tag, no jq: GitHub API first, releases/latest redirect as fallback
# (the API rate-limits unauthenticated callers).
latest_version() {
  local tag
  tag=$(curl -fsSL --max-time 20 "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
    | grep -o '"tag_name": *"[^"]*"' | cut -d'"' -f4)
  if [ -z "$tag" ]; then
    tag=$(curl -fsSI --max-time 20 "https://github.com/$REPO/releases/latest" 2>/dev/null \
      | awk -F/ 'tolower($1) == "location:" { print $NF }' | tr -d '\r' | head -1)
  fi
  printf '%s' "${tag#v}"
}

run_installer() {
  local tmp
  tmp=$(mktemp /tmp/vicinae-install.XXXXXX.sh)
  trap 'rm -f "$tmp"' EXIT INT TERM
  curl -fsSL --max-time 60 "$INSTALLER_URL" -o "$tmp" || \
    curl -fsSL --max-time 60 "$INSTALLER_FALLBACK" -o "$tmp" || {
      echo "could not download the installer" >&2
      exit 1
    }
  head -1 "$tmp" | grep -q '^#!' || { echo "installer is not a script: $tmp" >&2; exit 1; }
  grep -q "$REPO" "$tmp" || { echo "installer does not mention $REPO; refusing to run it" >&2; exit 1; }
  bash "$tmp" "$@"
}

case "${1:-}" in
  -h|--help)
    usage
    exit 0
    ;;
  --uninstall)
    run_installer --uninstall
    # The installer deletes the unit as root, where `systemctl --user` cannot reach this
    # session, so the running daemon is still alive: finish the job as the user.
    systemctl --user stop "$SERVICE" 2>/dev/null || true
    systemctl --user daemon-reload 2>/dev/null || true
    exit 0
    ;;
esac

installed=$(installed_version) || {
  echo "vicinae is not installed; install it with:" >&2
  echo "  curl -fsSL https://vicinae.com/install | bash" >&2
  exit 1
}
latest=$(latest_version)
[ -n "$latest" ] || { echo "could not determine the latest release (GitHub unreachable or rate-limited)" >&2; exit 1; }

echo "installed: v$installed"
echo "latest:    v$latest"

if [ "${1:-}" = "--check" ]; then
  [ "$installed" = "$latest" ] && echo "up to date" || echo "update available"
  exit 0
fi

if [ "$installed" = "$latest" ]; then
  echo "already up to date (the installer would skip the download)"
  exit 0
fi

echo "updating v$installed -> v$latest"
run_installer "$@"
rc=$?
[ "$rc" -eq 0 ] || { echo "installer exited with $rc" >&2; exit "$rc"; }

new=$(installed_version)
if [ "$new" != "$installed" ]; then
  systemctl --user daemon-reload 2>/dev/null || true
  if systemctl --user restart "$SERVICE" 2>/dev/null; then
    echo "restarted $SERVICE (now v$new)"
  else
    echo "note: could not restart the user service; run: systemctl --user restart $SERVICE" >&2
  fi
else
  echo "installer finished but the installed version still reads v$installed" >&2
  exit 1
fi
