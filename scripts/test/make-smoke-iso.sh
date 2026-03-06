#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <workdir>" >&2
  exit 1
fi

WORKDIR="$1"
mkdir -p "$WORKDIR"

if command -v grub2-mkrescue >/dev/null 2>&1; then
  MKRESCUE=grub2-mkrescue
elif command -v grub-mkrescue >/dev/null 2>&1; then
  MKRESCUE=grub-mkrescue
else
  echo "missing grub2-mkrescue/grub-mkrescue" >&2
  exit 1
fi

if ! command -v xorriso >/dev/null 2>&1; then
  echo "missing xorriso" >&2
  exit 1
fi

TREE="$WORKDIR/tree"
OVERLAY="$WORKDIR/overlay"
ISO="$WORKDIR/source.iso"

rm -rf "$TREE" "$OVERLAY" "$ISO"
mkdir -p "$TREE/.disk" "$TREE/boot/grub" "$OVERLAY"

cat > "$TREE/.disk/info" <<'EOF'
ForgeISO Smoke 24.04 amd64
EOF

cat > "$TREE/boot/grub/grub.cfg" <<'EOF'
serial --unit=0 --speed=115200
terminal_output console serial
terminal_input console
set timeout=1
set default=0

menuentry "ForgeISO Smoke" {
  echo FORGEISO_SMOKE_START
  sleep 1
  echo FORGEISO_SMOKE_DONE
}
EOF

cat > "$OVERLAY/LOCAL-OVERLAY.txt" <<'EOF'
overlay-applied
EOF

"$MKRESCUE" -o "$ISO" "$TREE" >/dev/null 2>&1

printf 'ISO=%s\n' "$ISO"
printf 'OVERLAY=%s\n' "$OVERLAY"
