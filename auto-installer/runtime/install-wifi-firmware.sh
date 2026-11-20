#!/usr/bin/env bash

set -Eeuo pipefail
shopt -s nullglob

source_dir=
root=/

usage() {
	printf 'Usage: %s --source DIR [--root DIR]\n' "${0##*/}"
}

die() {
	printf 'Error: %s\n' "$*" >&2
	exit 1
}

pick_one() {
	local label=$1
	shift
	local files=("$@")

	((${#files[@]} == 1)) ||
		die "expected exactly one $label file in $source_dir, found ${#files[@]}"
	[[ -s "${files[0]}" ]] || die "$label file is empty: ${files[0]}"
	printf '%s\n' "${files[0]}"
}

while (($# > 0)); do
	case "$1" in
		--source)
			(($# >= 2)) || { usage >&2; exit 2; }
			source_dir=$2
			shift 2
			;;
		--root)
			(($# >= 2)) || { usage >&2; exit 2; }
			root=$2
			shift 2
			;;
		-h|--help)
			usage
			exit 0
			;;
		*)
			usage >&2
			exit 2
			;;
	esac
done

[[ -d "$source_dir" ]] || die "firmware directory not found: $source_dir"
[[ -d "$root" ]] || die "target root not found: $root"

trx_file=$(pick_one '.trx firmware' "$source_dir"/*.trx)
clm_file=$(pick_one '.clmb regulatory blob' "$source_dir"/*.clmb)
txcap_file=$(pick_one '.txcb TxCap blob' "$source_dir"/*.txcb)
nvram_file=$(pick_one 'P-*.txt NVRAM' "$source_dir"/P-*.txt)

clm_stem=$(basename "$clm_file" .clmb)
txcap_stem=$(basename "$txcap_file" .txcb)
nvram_name=$(basename "$nvram_file")

[[ "$clm_stem" == "$txcap_stem" ]] ||
	die '.clmb and .txcb files do not use the same board name'
if [[ "$nvram_name" =~ ^P-([^_]+)_M-([^_]+)_V-([^_]+)__m-(.+)\.txt$ ]]; then
	nvram_board=${BASH_REMATCH[1]}
	nvram_module=${BASH_REMATCH[2]}
	nvram_vendor=${BASH_REMATCH[3]}
	nvram_version=${BASH_REMATCH[4]}
else
	die "unexpected NVRAM filename: $nvram_name"
fi
[[ "$nvram_board" == "$clm_stem" ]] ||
	die 'NVRAM board name does not match .clmb/.txcb board name'

# macOS names the blobs and the NVRAM after the board plus the antenna variant
# of this Mac, for example trinidad-X3. brcmfmac asks either for
# <board>-<module>-<vendor>-<version> or for <board>-<antenna>, never for both
# parts at once, so the antenna variant must not be pasted in front of the
# module details. The .trx file carries the bare board name both forms build on.
# Every brcmfmac also asks for the bare board name last, and it is the only
# board name a stock driver asks for when ACPI reports no antenna SKU
# (iMacPro1,1, iMac19,1, iMac19,2).
board_base=$(basename "$trx_file" .trx)
[[ "$clm_stem" == "$board_base" || "$clm_stem" == "$board_base"-* ]] ||
	die "board name '$clm_stem' does not extend the .trx board name '$board_base'"

kernel_log() {
	journalctl -b -k --no-pager 2>/dev/null || :
	dmesg 2>/dev/null || :
}

detect_requested_bin() {
	local warned prefix

	# Kernels that warn about every board-specific candidate name the most
	# specific one first, so it can be taken verbatim.
	warned=$(
		kernel_log |
			sed -n 's|.*Direct firmware load for \(brcm/brcmfmac[^ ]*\.apple,[^ ]*\.bin\) failed with error -2.*|\1|p' |
			head -n 1
	) || :
	if [[ -n "$warned" ]]; then
		printf '%s\n' "$warned"
		return 0
	fi

	# Newer kernels try the board-specific candidates quietly and warn only
	# about the very last one. The driver still reports which chip prefix it
	# uses, and the board identity comes from the NVRAM file. Together they
	# name one of the candidates the driver asks for.
	prefix=$(
		kernel_log |
			sed -n 's|.*brcmf_fw_alloc_request: using \(brcm/brcmfmac[^ ]*-pcie\) for chip .*|\1|p' |
			head -n 1
	) || :
	[[ -n "$prefix" ]] || return 1
	printf '%s.apple,%s-%s-%s-%s.bin\n' "$prefix" \
		"$board_base" "$nvram_module" "$nvram_vendor" "$nvram_version"
}

if [[ -n ${VORT2X_FIRMWARE_REQUEST:-} ]]; then
	requested_bin=$VORT2X_FIRMWARE_REQUEST
else
	requested_bin=$(detect_requested_bin) || requested_bin=
fi

[[ -n "$requested_bin" ]] ||
	die 'no brcmfmac firmware request for this Mac found in the current boot log'
target_bin=${requested_bin#brcm/}
[[ "$requested_bin" != "$target_bin" ]] ||
	die "unexpected firmware path in journal: $requested_bin"
[[ "$target_bin" == brcmfmac*-pcie.apple,* ]] ||
	die "unexpected brcmfmac filename: $target_bin"

target_prefix=${target_bin%%.apple,*}

target_clm="${target_prefix}.apple,${clm_stem}.clm_blob"
target_txcap="${target_prefix}.apple,${txcap_stem}.txcap_blob"
target_nvram="${target_prefix}.apple,${board_base}-${nvram_module}-${nvram_vendor}-${nvram_version}.txt"
dest_dir="$root/usr/lib/firmware/brcm"

install -d -m 0755 "$dest_dir"
install -m 0644 "$trx_file" "$dest_dir/$target_bin"
install -m 0644 "$clm_file" "$dest_dir/$target_clm"
install -m 0644 "$txcap_file" "$dest_dir/$target_txcap"
install -m 0644 "$nvram_file" "$dest_dir/$target_nvram"

# The bare board name points at the same four files. On the Macs above the
# blobs and the NVRAM are identical for every antenna variant.
bare_links=()
link_bare_name() {
	local target=$1 extension=$2
	local link="${target_prefix}.apple,${board_base}.${extension}"

	[[ "$link" != "$target" ]] || return 0
	ln -sfn "$target" "$dest_dir/$link"
	bare_links+=("$link")
}
link_bare_name "$target_bin" bin
link_bare_name "$target_nvram" txt
link_bare_name "$target_clm" clm_blob
link_bare_name "$target_txcap" txcap_blob

printf 'VorT2x Wi-Fi firmware installed:\n'
printf '  /usr/lib/firmware/brcm/%s\n' \
	"$target_bin" "$target_clm" "$target_txcap" "$target_nvram" \
	"${bare_links[@]}"
