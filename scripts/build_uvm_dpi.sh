#!/usr/bin/env bash
# Thin wrapper around the Makefile in this directory. Builds the UVM DPI
# shared objects that xezim loads at runtime.
#
# All real logic — fetching the Accellera tarballs, extracting the
# standards source trees, and compiling each variant — lives in the
# adjacent Makefile. This script just forwards arguments.
#
# Usage:
#   ./scripts/build_uvm_dpi.sh                  # make (default target)
#   ./scripts/build_uvm_dpi.sh clean            # make clean
#   ./scripts/build_uvm_dpi.sh fetch            # make fetch
#   ./scripts/build_uvm_dpi.sh -j4 distclean    # parallel + extra target
#
# Run `make help` in this directory for the full target list.

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)

if ! command -v make >/dev/null; then
    echo "error: 'make' not found in PATH" >&2
    exit 127
fi

exec make -C "$SCRIPT_DIR" "$@"
