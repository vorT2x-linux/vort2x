#!/usr/bin/env bash
# A step whose part failed must not count as applied, and the ACPI step's
# autofix failure is a warning, not the end of the installer.
set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
test_dir=$(mktemp -d /tmp/vort2x-step-status-test.XXXXXX)

export VORT2X_INSTALL_ERRORS="$test_dir/errors"
source "$repo_root/scripts/fedora/lib.sh"
trap 'rm -rf -- "$test_dir"' EXIT

# A child installer like install-dkms-modules.sh: one part fails, the
# script carries on and exits 0.
swallowing_step() {
	run_step "simulated module build" false
	return 0
}

{
	run_install_step "simulated step with a failed part" swallowing_step
	(( STEP_STATUS != 0 ))
	run_install_step "simulated clean step" true
	(( STEP_STATUS == 0 ))
} >"$test_dir/run.log" 2>&1

# install-acpi-fixes.sh with an autofix that fails: exit 0 and a warning.
mkdir -p "$test_dir/bin"
cat >"$test_dir/bin/python3" <<'STUB'
#!/bin/sh
echo "ACPI fix could not be applied: simulated failure"
exit 1
STUB
chmod +x "$test_dir/bin/python3"
: >"$VORT2X_INSTALL_ERRORS"
PATH="$test_dir/bin:$PATH" bash "$repo_root/scripts/fedora/install-acpi-fixes.sh" \
	>"$test_dir/acpi.log" 2>&1
grep -q 'ACPI autofix failed: simulated failure' "$VORT2X_INSTALL_ERRORS"

echo "Step status checks passed."
