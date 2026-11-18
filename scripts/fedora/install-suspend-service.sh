#!/usr/bin/env bash

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)/lib.sh"

require_root
require_repo_root
require_fedora
require_command install systemctl readlink timeout

SCRIPT_SRC="$REPO_ROOT/scripts/fedora/vort2x-suspend.sh"

[[ -f "$SCRIPT_SRC" ]] || fail "missing $SCRIPT_SRC"

info "installing Vort2x suspend helper"
install -d -o root -g root -m 0755 /usr/local/libexec/vort2x
install -o root -g root -m 0755 "$SCRIPT_SRC" /usr/local/libexec/vort2x/vort2x-suspend.sh

tee /etc/systemd/system/vort2x-suspend.service >/dev/null <<'EOF'
[Unit]
Description=Vort2x T2 suspend and resume hardware fixes
Before=sleep.target
After=NetworkManager.service t2-services-suspend.service
StopWhenUnneeded=yes

[Service]
Type=oneshot
RemainAfterExit=yes
TimeoutStopSec=150
ExecStart=/usr/local/libexec/vort2x/vort2x-suspend.sh pre
ExecStop=/usr/local/libexec/vort2x/vort2x-suspend.sh post

[Install]
WantedBy=sleep.target
EOF
chmod 0644 /etc/systemd/system/vort2x-suspend.service

systemctl daemon-reload
systemctl enable vort2x-suspend.service

info "Vort2x suspend helper installed"
