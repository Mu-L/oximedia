#!/usr/bin/env bash
# Build script for `OxiMedia` WASM
# Publishes as @cooljapan/oximedia on npm

set -e

SCOPE="cooljapan"
NPM_NAME="@${SCOPE}/oximedia"

echo "Building OxiMedia WASM (${NPM_NAME})..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "Error: wasm-pack is not installed"
    echo "Install it with: cargo install wasm-pack"
    exit 1
fi

# Build for web target
echo "Building for web target..."
wasm-pack build --scope "${SCOPE}" --target web --out-dir pkg-web --release

# Build for Node.js target
echo "Building for Node.js target..."
wasm-pack build --scope "${SCOPE}" --target nodejs --out-dir pkg-node --release

# Build for bundler target (webpack, etc.)
echo "Building for bundler target..."
wasm-pack build --scope "${SCOPE}" --target bundler --out-dir pkg-bundler --release

# Rename package to @cooljapan/oximedia in all generated package.json files
for dir in pkg-web pkg-node pkg-bundler; do
    if [ -f "${dir}/package.json" ]; then
        sed -i '' "s|\"@${SCOPE}/oximedia-wasm\"|\"${NPM_NAME}\"|g" "${dir}/package.json"
        echo "  Updated ${dir}/package.json → ${NPM_NAME}"
    fi
done

echo ""
echo "Build complete!"
echo ""
echo "Output directories:"
echo "  - pkg-web/      : For use in web browsers"
echo "  - pkg-node/     : For use in Node.js"
echo "  - pkg-bundler/  : For use with webpack/rollup/etc."
echo ""
echo "To publish (bundler target recommended for npm):"
echo "  cd pkg-bundler && npm publish --access public"
