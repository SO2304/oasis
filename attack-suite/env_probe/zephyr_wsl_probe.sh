#!/usr/bin/env bash
# Timeboxed (15 min, enforced by the caller with `timeout 900`) probe: can a Zephyr
# Bluetooth Mesh BabbleSim test be built in this WSL? Every step is timestamped.
# Outcome decides whether attack-suite Part D runs or is deferred.
set -u
ts() { date -u +%FT%TZ; }
step() { echo; echo "[$(ts)] === $* ==="; }
step "host";            uname -a; free -m; df -h "$HOME" | tail -1; nproc
step "tools present";   for t in python3 pip3 cmake ninja gcc git dtc; do printf '%-8s ' "$t"; command -v "$t" >/dev/null && "$t" --version 2>&1 | head -1 || echo MISSING; done
step "sudo (apt needs it for dtc / python3-venv)"; if sudo -n true 2>/dev/null; then echo "passwordless sudo: yes"; sudo -n apt-get install -y -qq device-tree-compiler python3-venv 2>&1 | tail -2; else echo "passwordless sudo: NO (apt packages cannot be installed unattended)"; fi
step "west in a venv (PEP 668)"; python3 -m venv "$HOME/zvenv" 2>&1 | tail -2 || true; [ -x "$HOME/zvenv/bin/python" ] || { echo "RESULT: python3-venv missing and not installable without sudo"; exit 4; }; . "$HOME/zvenv/bin/activate"; pip install --quiet west 2>&1 | tail -3; west --version || { echo "RESULT: west unavailable"; exit 2; }
W="$HOME/zephyr-probe"
step "west init (zephyr v3.7.0, pinned)"
[ -d "$W/.west" ] || west init -m https://github.com/zephyrproject-rtos/zephyr --mr v3.7.0 "$W" 2>&1 | tail -5
cd "$W" || { echo "RESULT: init failed"; exit 3; }
step "west update (zephyr + babblesim group only, shallow)"
west config manifest.group-filter -- +babblesim
west config manifest.project-filter -- -.*,+zephyr,+babblesim_base,+babblesim_ext_2G4_libPhyComv1,+babblesim_ext_2G4_phy_v1,+babblesim_ext_2G4_channel_NtNcable,+babblesim_ext_2G4_modem_magic,+hal_nordic,+nrf_hw_models,+cmsis,+mbedtls,+tinycrypt
west update --narrow -o=--depth=1 2>&1 | tail -8
step "python requirements"
pip install --quiet -r zephyr/scripts/requirements-base.txt 2>&1 | tail -3
step "build babblesim"
export BSIM_COMPONENTS_PATH="$W/tools/bsim/components" BSIM_OUT_PATH="$W/tools/bsim"
( cd tools/bsim && make everything -j"$(nproc)" 2>&1 | tail -5 )
step "build one BT Mesh bsim test"
export ZEPHYR_BASE="$W/zephyr"
( cd zephyr && tests/bsim/bluetooth/mesh/compile.sh 2>&1 | tail -10 )
step "done"; echo "RESULT: reached the end (see above for per-step success)"
