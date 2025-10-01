#!/usr/bin/env bash
set -euo pipefail

echo "🚀 Starting Forgepoint with Extensions..."
echo ""

# Build WASM extension if needed
if [ ! -f "server/extensions/issues.wasm" ] || [ "packages/extensions/issues/src/lib.rs" -nt "server/extensions/issues.wasm" ]; then
    echo "📦 Building Issues extension (WASM)..."
    cd packages/extensions/issues
    cargo build --target wasm32-wasip1 --release
    cd ../../..

    echo "📋 Copying WASM to server/extensions/..."
    mkdir -p server/extensions
    cp packages/extensions/issues/target/wasm32-wasip1/release/forgepoint_extension_issues.wasm \
       server/extensions/issues.wasm
    echo "✅ Extension built successfully!"
    echo ""
else
    echo "✅ Extension already built (issues.wasm exists)"
    echo ""
fi

# Check if we should start the server
if [ "${1:-}" = "server" ]; then
    echo "🖥️  Starting Forge Server..."
    cd server
    FORGE_IN_MEMORY_DB=true cargo run --bin server
    exit 0
fi

# Check if we should start the web app
if [ "${1:-}" = "web" ]; then
    echo "🌐 Starting Web Frontend..."
    cd apps/web
    bun install
    bun run dev
    exit 0
fi

# Default: show instructions
echo "📝 Extension is ready! Now start the server and web app in separate terminals:"
echo ""
echo "Terminal 1 (Server):"
echo "  cd server"
echo "  FORGE_IN_MEMORY_DB=true cargo run --bin server"
echo ""
echo "Terminal 2 (Web):"
echo "  cd apps/web"
echo "  bun run dev"
echo ""
echo "Or use these shortcuts:"
echo "  ./start-with-extensions.sh server  # Start server only"
echo "  ./start-with-extensions.sh web     # Start web only"
echo ""
echo "Then visit:"
echo "  - GraphQL API: http://localhost:8000/graphql"
echo "  - Web App: http://localhost:4321"
echo "  - Issues Page: http://localhost:4321/issues"
echo ""
