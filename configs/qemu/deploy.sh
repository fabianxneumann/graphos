#!/usr/bin/env bash
# GraphOS Deploy Pipeline
# Builds the UEFI binary locally, deploys to Proxmox VM 107, boots and verifies.
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
EFI_TARGET="x86_64-unknown-uefi"
EFI_BINARY="$PROJECT_ROOT/target/$EFI_TARGET/release/graphos-boot.efi"
PVE_HOST="root@pve"
VMID=107
PVE_DEPLOY_DIR="/tmp/graphos-deploy"

# Boot timeout in seconds (TCG emulation without KVM is slow ~50s)
BOOT_TIMEOUT=60

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[DEPLOY]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
err() { echo -e "${RED}[ERROR]${NC} $1" >&2; exit 1; }

# --- Phase 1: Build ---
log "Phase 1: Building graphos-boot for $EFI_TARGET..."
source "$HOME/.cargo/env"
cd "$PROJECT_ROOT"
cargo build -p graphos-boot --target "$EFI_TARGET" --release 2>&1 | tail -3

[ -f "$EFI_BINARY" ] || err "Build failed: $EFI_BINARY not found"
log "Binary ready: $(ls -lh "$EFI_BINARY" | awk '{print $5}') -- $EFI_BINARY"

# --- Phase 2: Transfer to PVE ---
log "Phase 2: Transferring .efi to Proxmox..."
ssh "$PVE_HOST" "mkdir -p $PVE_DEPLOY_DIR" 2>/dev/null
scp -q "$EFI_BINARY" "$PVE_HOST:$PVE_DEPLOY_DIR/graphos-boot.efi"
log "Transfer complete."

# --- Phase 3: Create GPT+ESP boot disk on PVE ---
log "Phase 3: Creating GPT disk with EFI System Partition on PVE..."
ssh "$PVE_HOST" bash <<'REMOTE_SCRIPT'
set -euo pipefail
DEPLOY_DIR="/tmp/graphos-deploy"
IMG="$DEPLOY_DIR/boot.img"
VMID=107
LV_PATH="/dev/pve/vm-${VMID}-disk-1"

# Create a 64MB image with GPT + EFI System Partition
dd if=/dev/zero of="$IMG" bs=1M count=64 status=none
sgdisk --clear "$IMG" >/dev/null 2>&1
sgdisk --new=1:2048:131038 --typecode=1:EF00 --change-name=1:"EFI" "$IMG" >/dev/null 2>&1

# Create FAT32 filesystem for the ESP partition
PART_SECTORS=$((131038 - 2048 + 1))
dd if=/dev/zero of="$DEPLOY_DIR/esp.img" bs=512 count=$PART_SECTORS status=none
mkfs.vfat -F 32 "$DEPLOY_DIR/esp.img" >/dev/null

# Mount and place the EFI binary at the standard UEFI boot path
mkdir -p "$DEPLOY_DIR/mnt"
mount -o loop "$DEPLOY_DIR/esp.img" "$DEPLOY_DIR/mnt"
mkdir -p "$DEPLOY_DIR/mnt/EFI/BOOT"
cp "$DEPLOY_DIR/graphos-boot.efi" "$DEPLOY_DIR/mnt/EFI/BOOT/BOOTX64.EFI"
sync
umount "$DEPLOY_DIR/mnt"
rmdir "$DEPLOY_DIR/mnt"

# Inject the ESP into the GPT image at partition offset
dd if="$DEPLOY_DIR/esp.img" of="$IMG" bs=512 seek=2048 conv=notrunc status=none
rm -f "$DEPLOY_DIR/esp.img"

# Write the complete disk image to the LV
dd if="$IMG" of="$LV_PATH" bs=1M status=none
rm -f "$IMG"

echo "GPT+ESP boot image written to $LV_PATH"
REMOTE_SCRIPT

log "Boot disk prepared on PVE."

# --- Phase 4: Stop VM, reset EFI vars, configure and start ---
log "Phase 4: Configuring VM and starting..."
ssh "$PVE_HOST" bash <<REMOTE_START
set -euo pipefail
VMID=$VMID

# Stop VM if running
qm status \$VMID 2>/dev/null | grep -q running && qm stop \$VMID --timeout 5 2>/dev/null || true
sleep 2

# Ensure correct VM settings
qm set \$VMID --boot order=scsi0 2>/dev/null || true
qm set \$VMID --vga serial0 2>/dev/null || true
qm set \$VMID --kvm 0 2>/dev/null || true

# Reset EFI variables so OVMF re-discovers boot devices
dd if=/usr/share/pve-edk2-firmware/OVMF_VARS_4M.fd of=/dev/pve/vm-\${VMID}-disk-0 bs=1M status=none

# Start VM and immediately begin capturing serial output
qm start \$VMID
echo "VM \$VMID started."
REMOTE_START

log "VM $VMID started. Waiting for boot (TCG mode, ~${BOOT_TIMEOUT}s)..."

# --- Phase 5: Capture serial output and verify ---
log "Phase 5: Capturing serial output..."
SERIAL_LOG="/tmp/graphos-serial-$$.log"

ssh "$PVE_HOST" "timeout $BOOT_TIMEOUT socat -u UNIX-CONNECT:/var/run/qemu-server/${VMID}.serial0 STDOUT 2>/dev/null" > "$SERIAL_LOG" &
CAPTURE_PID=$!

# Wait for boot to complete (check periodically)
for i in $(seq 1 $BOOT_TIMEOUT); do
    sleep 1
    if grep -q "GraphOS" "$SERIAL_LOG" 2>/dev/null; then
        # Give it 2 more seconds to capture the full output
        sleep 2
        kill $CAPTURE_PID 2>/dev/null || true
        break
    fi
done

# Ensure capture process is stopped
kill $CAPTURE_PID 2>/dev/null || true
wait $CAPTURE_PID 2>/dev/null || true

echo ""
echo "--- Serial Output ---"
cat "$SERIAL_LOG"
echo "--- End Output ---"
echo ""

if grep -q "GraphOS" "$SERIAL_LOG"; then
    log "SUCCESS: GraphOS banner detected! Deploy complete."
    rm -f "$SERIAL_LOG"
    exit 0
else
    warn "GraphOS banner not found in serial output after ${BOOT_TIMEOUT}s."
    warn "The VM is running. Debug with: ssh root@pve 'qm terminal $VMID'"
    rm -f "$SERIAL_LOG"
    exit 1
fi
