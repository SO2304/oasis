#!/usr/bin/env bash
# OASIS Grid Demo launcher — runs v2 then v3 in sequence.
#  v2 = each layer in isolation (8 scenarios, pedagogical)
#  v3 = layers coordinated (4 chained scenarios, operational)
# Both binaries are statically linked musl + cross-compiled per OS/arch.

set -e
cd "$(dirname "$0")"

BIN_DIR="binaries"
OS="$(uname -s 2>/dev/null || echo unknown)"
ARCH="$(uname -m 2>/dev/null || echo unknown)"

echo
echo "OASIS Grid Demo launcher"
echo "  host: $OS / $ARCH"

resolve_bin() {
    local stem="$1"
    case "$OS" in
        Linux)
            case "$ARCH" in
                x86_64|amd64)  echo "$BIN_DIR/${stem}-linux-x86_64" ;;
                aarch64|arm64) echo "$BIN_DIR/${stem}-linux-aarch64" ;;
                *)
                    echo "  Unsupported Linux arch: $ARCH"
                    echo "  Contact: souhaybrharrab@gmail.com"
                    exit 3
                    ;;
            esac
            ;;
        Darwin)
            echo "  macOS not in this pack — request native build at souhaybrharrab@gmail.com"
            exit 4
            ;;
        MINGW*|MSYS*|CYGWIN*) echo "$BIN_DIR/${stem}.exe" ;;
        *) echo "  Unknown OS: $OS"; exit 5 ;;
    esac
}

run_one() {
    local label="$1"
    local stem="$2"
    local bin
    bin=$(resolve_bin "$stem")
    if [ ! -f "$bin" ]; then
        echo "  Expected binary not found: $bin"
        exit 6
    fi
    chmod +x "$bin" 2>/dev/null || true
    echo
    echo "▶ Running $label"
    echo "  binary: $bin"
    echo "─────────────────────────────────────────────────────────────────"
    ./"$bin"
    local rc=$?
    echo "─────────────────────────────────────────────────────────────────"
    echo "  $label exit: $rc"
    return $rc
}

run_one "Demo v2 (5 layers in isolation, 8 scenarios)"     oasis_grid_demo
EX2=$?

run_one "Demo v3 (chained interactions, 4 scenarios)"      oasis_grid_demo_v3
EX3=$?

run_one "Demo v4 (all 11 mechanisms, PROVEN/EXP tagged)"   oasis_grid_demo_v4
EX4=$?

echo
if [ $EX2 -eq 0 ] && [ $EX3 -eq 0 ] && [ $EX4 -eq 0 ]; then
    echo "[DONE] All 3 demos PASSED. See docs/ for level-2 evaluation."
    exit 0
else
    echo "[FAIL] v2=$EX2  v3=$EX3  v4=$EX4 — one or more unexpected."
    echo "Send this output to souhaybrharrab@gmail.com — it's on us, not you."
    exit 1
fi
