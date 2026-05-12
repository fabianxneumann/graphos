#!/usr/bin/env bash
set -euo pipefail

PVE_HOST="root@pve"
VMID=107
PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
EFI_FILE="$PROJECT_DIR/target/x86_64-unknown-uefi/release/graphos-boot.efi"

echo "[1/4] Building GraphOS (release)..."
export PATH="$HOME/.cargo/bin:$PATH"
cd "$PROJECT_DIR"
cargo build -p graphos-boot --target x86_64-unknown-uefi --release 2>&1 | tail -5

if [ ! -f "$EFI_FILE" ]; then
    echo "[ERROR] Build failed — EFI binary not found at $EFI_FILE"
    exit 1
fi

EFI_SIZE=$(stat -f%z "$EFI_FILE" 2>/dev/null || stat -c%s "$EFI_FILE")
echo "[1/4] Done. EFI binary: $(( EFI_SIZE / 1024 )) KiB"

echo "[2/4] Uploading EFI to Proxmox..."
scp -q "$EFI_FILE" "$PVE_HOST:/tmp/graphos-boot.efi"

echo "[3/4] Flashing boot disk and restarting VM $VMID..."
ssh "$PVE_HOST" bash <<'REMOTE'
set -e
VMID=107
LV_PATH="/dev/pve/vm-107-disk-2"

# Create fresh 64MB FAT32 image with EFI binary
IMG="/tmp/graphos-disk.img"
dd if=/dev/zero of="$IMG" bs=1M count=64 status=none
mkfs.fat -F32 "$IMG" >/dev/null 2>&1

# Mount and copy EFI
mkdir -p /tmp/efi_mount
mount -o loop "$IMG" /tmp/efi_mount
mkdir -p /tmp/efi_mount/EFI/BOOT
cp /tmp/graphos-boot.efi /tmp/efi_mount/EFI/BOOT/BOOTX64.EFI
umount /tmp/efi_mount

# Flash directly into LVM thin volume
dd if="$IMG" of="$LV_PATH" bs=1M status=none
rm -f "$IMG"

# Restart VM
qm stop $VMID 2>/dev/null || true
sleep 2
qm start $VMID
echo "VM $VMID restarted."
REMOTE

echo "[4/4] Done! GraphOS deployed to VM $VMID."
echo ""
echo "Connect with:"
echo "  ssh root@pve"
echo "  qm terminal $VMID"
echo ""
echo "Press Enter to exit, or Ctrl+C."
