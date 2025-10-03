#!/bin/bash

# Set up environment
export FORGE_DB_PATH=/tmp/forge-test-db
export FORGE_REPOS_PATH=/tmp/forge-repos
export FORGE_EXTENSIONS_DIR=extensions_dir

# Create directories
mkdir -p "$FORGE_DB_PATH" "$FORGE_REPOS_PATH"

# Start server in background
echo "Starting server..."
cargo run > /tmp/server.log 2>&1 &
SERVER_PID=$!

# Wait for server to start
echo "Waiting for server to start..."
for i in {1..10}; do
    if curl -s http://localhost:8000 > /dev/null 2>&1; then
        echo "Server started successfully"
        break
    fi
    sleep 1
done

# Test GraphQL query
echo "Testing GraphQL query for getAllIssues..."
curl -s http://localhost:8000/graphql \
    -X POST \
    -H "Content-Type: application/json" \
    -d '{"query": "{ getAllIssues }"}' | jq .

# Clean up
echo "Stopping server..."
kill $SERVER_PID 2>/dev/null

echo "Check /tmp/server.log for server output"