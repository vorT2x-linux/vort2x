#!/usr/bin/env bash

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
launcher="$repo_root/auto-installer/runtime/vort2x-launch-terminal"
work=$(mktemp -d "${TMPDIR:-/tmp}/vort2x-terminal-test.XXXXXX")
trap 'rm -rf "$work"' EXIT
log="$work/terminal.log"

run_case() {
	local terminal=$1 expected=$2
	local fake_bin="$work/$terminal"
	mkdir -p "$fake_bin"
	cat >"$fake_bin/$terminal" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >"$VORT2X_TEST_LOG"
EOF
	ln -s /bin/bash "$fake_bin/bash"
	chmod 0755 "$fake_bin/$terminal"
	PATH="$fake_bin" \
		VORT2X_INSTALL_COMMAND=/test/vort2x-install \
		VORT2X_TEST_LOG="$log" \
		bash "$launcher"
	grep -Fxq -- "$expected" "$log"
}

run_case ptyxis '--standalone -- /test/vort2x-install'
run_case konsole '-e /test/vort2x-install'
run_case cosmic-term '-e /test/vort2x-install'
