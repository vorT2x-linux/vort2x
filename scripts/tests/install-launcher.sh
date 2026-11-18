#!/usr/bin/env bash

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
launcher="$repo_root/auto-installer/runtime/vort2x-install"
work=$(mktemp -d "${TMPDIR:-/tmp}/vort2x-install-test.XXXXXX")
work=$(cd -- "$work" && pwd -P)
trap 'rm -rf "$work"' EXIT

target=7.1.3-200.fc44.x86_64
fake_home="$work/home"
fake_repo="$work/repository"
fake_state="$work/state"
fake_bin="$work/bin"
log="$work/commands.log"
initial_output="$work/initial-output.log"
origin_url_file="$work/origin-url"
canonical_url=https://github.com/vorT2x-linux/vort2x.git
git_prefix="-c safe.directory=$fake_repo -C $fake_repo"
fake_os_release="$work/os-release"
mkdir -p "$fake_home" "$fake_repo/.git" "$fake_state" "$fake_bin"
printf 'ID=fedora\nVERSION_ID=45\n' >"$fake_os_release"
printf '%s\n' "$canonical_url" >"$origin_url_file"
printf 'phase=reboot_pending\ntarget_kernel=%s\n' "$target" >"$fake_state/state"

cat >"$fake_bin/flock" <<'EOF'
#!/usr/bin/env bash
printf 'flock %s\n' "$*" >>"$VORT2X_TEST_LOG"
exit 0
EOF
cat >"$fake_bin/uname" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$VORT2X_TEST_TARGET"
EOF
cat >"$fake_bin/git" <<'EOF'
#!/usr/bin/env bash
prefix="-c safe.directory=$VORT2X_TEST_REPOSITORY -C $VORT2X_TEST_REPOSITORY"
case "$*" in
	"$prefix status --porcelain") exit 0 ;;
	"$prefix remote get-url origin") cat "$VORT2X_TEST_ORIGIN_URL_FILE" ;;
	"$prefix remote set-url origin "*)
		printf '%s\n' "${@: -1}" >"$VORT2X_TEST_ORIGIN_URL_FILE"
		printf 'git %s\n' "$*" >>"$VORT2X_TEST_LOG"
		;;
	"ls-remote --exit-code "*) exit 0 ;;
	# v1.2.0 supports only Fedora 46, so Fedora 45 must get v1.1.0.
	"$prefix tag --list v* --sort=-version:refname")
		[[ ${VORT2X_TEST_NO_TAGS:-0} == 1 ]] || printf 'v1.2.0\nv1.2.0-rc1\nv1.1.0\nv1.0.0\n'
		;;
	"$prefix show refs/tags/v1.2.0:scripts/fedora/supported-releases") printf '# x\n46\n' ;;
	"$prefix show refs/tags/v1.1.0:scripts/fedora/supported-releases") printf '45\n46\n' ;;
	"$prefix show refs/tags/v1.0.0:scripts/fedora/supported-releases") printf '45\n' ;;
	"$prefix show "*) exit 128 ;;
	*) printf 'git %s\n' "$*" >>"$VORT2X_TEST_LOG" ;;
esac
EOF
cat >"$fake_bin/sudo" <<'EOF'
#!/usr/bin/env bash
printf 'sudo cwd=%s command=%s\n' "$PWD" "$*" >>"$VORT2X_TEST_LOG"
[[ "${VORT2X_TEST_INSTALL_FAILS:-0}" == 1 && "$*" == "bash ./scripts/fedora/install.sh" ]] && exit 1
[[ "${VORT2X_TEST_COMPLETE_FAILS:-0}" == 1 && "$*" == */vort2x-prepare\ --complete ]] && exit 1
exit 0
EOF
chmod 0755 "$fake_bin"/*

run_launcher() {
	local script=$1
	shift
	env \
		HOME="$fake_home" \
		XDG_RUNTIME_DIR="$work" \
		PATH="$fake_bin:/usr/bin:/bin" \
		VORT2X_STATE_DIR="$fake_state" \
		VORT2X_REPOSITORY="$fake_repo" \
		VORT2X_OS_RELEASE="$fake_os_release" \
		VORT2X_TEST_TARGET="$target" \
		VORT2X_TEST_REPOSITORY="$fake_repo" \
		VORT2X_TEST_ORIGIN_URL_FILE="$origin_url_file" \
		VORT2X_TEST_LOG="$log" \
		"$@" bash "$script"
}

printf 'n\n' |
	run_launcher "$launcher" >"$initial_output"

grep -Fq "git $git_prefix checkout --quiet --detach refs/tags/v1.1.0" "$log"
grep -Fq "sudo cwd=$fake_repo command=bash ./scripts/fedora/install.sh" "$log"
grep -Fq 'command=/usr/local/bin/vort2x-prepare --complete' "$log"
grep -Fq "Repository: $fake_repo" "$initial_output"
grep -Fq 'Run vort2x-install at any time to update VorT2x.' "$initial_output"
grep -Fq 'Reboot once more to start the fully configured system.' "$initial_output"

: >"$log"
printf 'phase=complete\ntarget_kernel=%s\n' "$target" >"$fake_state/state"
printf 'n\n' |
	run_launcher "$launcher" >/dev/null
grep -Fq "git $git_prefix checkout --quiet --detach refs/tags/v1.1.0" "$log"
grep -Fq "sudo cwd=$fake_repo command=bash ./scripts/fedora/install.sh" "$log"
! grep -Fq 'vort2x-prepare --complete' "$log"

mkdir -p "$fake_repo/auto-installer/runtime"
cat >"$fake_repo/auto-installer/runtime/vort2x-install" <<'EOF'
#!/usr/bin/env bash
printf 'delegated active=%s\n' \
	"${VORT2X_INSTALL_SESSION_ACTIVE:-0}" >>"$VORT2X_TEST_LOG"
EOF
: >"$log"
run_launcher "$launcher" >/dev/null
grep -Fxq 'delegated active=0' "$log"
! grep -Fq 'flock ' "$log"

cp "$launcher" "$fake_repo/auto-installer/runtime/vort2x-install"
: >"$log"
printf 'phase=complete\ntarget_kernel=%s\n' "$target" >"$fake_state/state"
printf 'n\n' |
	run_launcher "$fake_repo/auto-installer/runtime/vort2x-install" \
		VORT2X_INSTALL_SESSION_ACTIVE=1 >/dev/null
grep -Fq "git $git_prefix checkout --quiet --detach refs/tags/v1.1.0" "$log"
grep -Fq "sudo cwd=$fake_repo command=bash ./scripts/fedora/install.sh" "$log"
! grep -Fq 'flock ' "$log"

: >"$log"
printf '%s\n' 'https://github.com' >"$origin_url_file"
printf 'phase=complete\ntarget_kernel=%s\n' "$target" >"$fake_state/state"
repair_output="$work/repair-output.log"
printf 'n\n' |
	run_launcher "$launcher" >"$repair_output"
grep -Fq 'Repairing the VorT2x origin URL (found: https://github.com).' "$repair_output"
grep -Fq "git $git_prefix remote set-url origin $canonical_url" "$log"
grep -Fxq "$canonical_url" "$origin_url_file"
grep -Fq "git $git_prefix checkout --quiet --detach refs/tags/v1.1.0" "$log"
grep -Fq "sudo cwd=$fake_repo command=bash ./scripts/fedora/install.sh" "$log"

# A step reporting errors must not also trip the generic ERR trap. That
# would bury run_project_installer's own explanation under a misleading
# "just retry" message, and still needs to fail without marking it complete.
: >"$log"
printf 'phase=reboot_pending\ntarget_kernel=%s\n' "$target" >"$fake_state/state"
failure_output="$work/failure-output.log"
if printf 'n\n' |
	run_launcher "$launcher" VORT2X_TEST_INSTALL_FAILS=1 >"$failure_output" 2>&1; then
	printf 'error: vort2x-install returned success after a reported install failure\n' >&2
	exit 1
fi
grep -Fq 'reported errors' "$failure_output"
grep -Fq 'not marked complete' "$failure_output"
! grep -Fq 'installation stopped at line' "$failure_output"
! grep -Fq 'vort2x-prepare --complete' "$log"

: >"$log"
printf 'phase=reboot_pending\ntarget_kernel=%s\n' "$target" >"$fake_state/state"
completion_output="$work/completion-failure.log"
if printf 'n\n' |
	run_launcher "$launcher" VORT2X_TEST_COMPLETE_FAILS=1 >"$completion_output" 2>&1; then
	printf 'error: vort2x-install returned success after final boot selection failed\n' >&2
	exit 1
fi
grep -Fq 'could not finalize the configured boot kernel' "$completion_output"
! grep -Fq 'completed successfully' "$completion_output"
! grep -Fq 'Reboot now?' "$completion_output"
grep -Fxq 'phase=reboot_pending' "$fake_state/state"

# VORT2X_REF picks a development branch instead of a release.
: >"$log"
printf 'phase=complete\ntarget_kernel=%s\n' "$target" >"$fake_state/state"
printf 'n\n' |
	run_launcher "$launcher" VORT2X_REF=main >/dev/null
grep -Fq "git $git_prefix fetch --quiet $canonical_url +refs/heads/main:refs/remotes/origin/main" "$log"
grep -Fq "git $git_prefix checkout --quiet --detach refs/remotes/origin/main" "$log"
! grep -Fq 'refs/tags/v' "$log"

# No release for this Fedora: stop before running the installer.
: >"$log"
printf 'phase=complete\ntarget_kernel=%s\n' "$target" >"$fake_state/state"
printf 'ID=fedora\nVERSION_ID=44\n' >"$fake_os_release"
norelease_output="$work/norelease-output.log"
if printf 'n\n' |
	run_launcher "$launcher" >"$norelease_output" 2>&1; then
	printf 'error: vort2x-install succeeded without a release for Fedora 44\n' >&2
	exit 1
fi
grep -Fq 'No VorT2x release supports Fedora 44 yet.' "$norelease_output"
! grep -Fq 'command=bash ./scripts/fedora/install.sh' "$log"
