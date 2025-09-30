#!/usr/bin/env bash

# Test the extension system with the existing static schema

# Set up environment for in-memory mode
export FORGE_IN_MEMORY_DB=true
export FORGE_EXTENSIONS_DIR=extensions_dir

echo "Starting server with extensions..."
./run-dev.sh &
SERVER_PID=$!

# Wait for server to start
echo "Waiting for server to start..."
sleep 3

echo "Testing if server is running..."
if ! curl -s http://localhost:8000 > /dev/null 2>&1; then
    echo "Server failed to start. Check the logs."
    kill $SERVER_PID 2>/dev/null
    exit 1
fi

echo "Testing GraphQL introspection to see extension fields..."
echo "Query: Introspecting Query type fields"
curl -s http://localhost:8000/graphql \
    -X POST \
    -H "Content-Type: application/json" \
    -d '{"query": "{ __type(name: \"Query\") { fields { name } } }"}' | jq .

echo ""
echo "Testing extension field (getAllIssues)..."
curl -s http://localhost:8000/graphql \
    -X POST \
    -H "Content-Type: application/json" \
    -d '{"query": "{ getAllIssues }"}' | jq .

echo ""
echo "Stopping server..."
kill $SERVER_PID 2>/dev/null

echo "Test complete!"