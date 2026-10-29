#!/bin/sh
# First-boot diagnostic for Tide OS.
#
# This service records a diagnostic report once, then marks first boot complete.
# It does not silently apply configuration, install software, or make network calls.

set -eu

MARKER=/var/lib/tide/firstboot-done
LOG=/var/log/tide/firstboot-doctor.json

install -d -m 0755 /var/lib/tide /var/log/tide

if [ -e "$MARKER" ]; then
    exit 0
fi

status=0
if command -v tidectl >/dev/null 2>&1; then
    if ! /usr/bin/tidectl doctor --json >"$LOG" 2>/var/log/tide/firstboot-doctor.err; then
        status=$?
    fi
else
    printf '%s\n' '{"error":"tidectl is not installed"}' >"$LOG"
    status=127
fi

chmod 0640 "$LOG" 2>/dev/null || true
printf 'completed=%s\ntidectl_exit=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$status" >"$MARKER"
chmod 0644 "$MARKER"

exit 0
