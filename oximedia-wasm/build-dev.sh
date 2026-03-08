#!/usr/bin/env bash
# Development build script for `OxiMedia` WASM
# This builds in debug mode with debug symbols

set -e

echo "Building OxiMedia WASM (debug mode)..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "Error: wasm-pack is not installed"
    echo "Install it with: cargo install wasm-pack"
    exit 1
fi

# Build for web target in debug mode
echo "Building for web target (debug)..."
wasm-pack build --dev --target web --out-dir pkg-web-dev

echo "Debug build complete!"
echo "Output: pkg-web-dev/"
