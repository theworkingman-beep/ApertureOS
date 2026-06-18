#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

cargo run -p bootloader

UEFI_IMAGE="target/aperture-uefi.img"
BIOS_IMAGE="target/aperture-bios.img"

if command -v qemu-system-x86_64 >/dev/null 2>&1; then
    QEMU="qemu-system-x86_64"
elif command -v qemu >/dev/null 2>&1; then
    QEMU="qemu"
else
    echo "qemu-system-x86_64 not found; cannot run OS."
    exit 1
fi

if [[ -f "$UEFI_IMAGE" ]]; then
    echo "Running UEFI image: $UEFI_IMAGE"
    $QEMU -drive "format=raw,file=$UEFI_IMAGE" -serial stdio -m 256M
elif [[ -f "$BIOS_IMAGE" ]]; then
    echo "Running BIOS image: $BIOS_IMAGE"
    $QEMU -drive "format=raw,file=$BIOS_IMAGE" -serial stdio -m 256M
else
    echo "No bootable disk image found."
    echo "Looked for:"
    echo "  $UEFI_IMAGE"
    echo "  $BIOS_IMAGE"
    exit 1
fi
