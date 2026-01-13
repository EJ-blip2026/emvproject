#!/bin/bash

# Test script for admin endpoints
# Assumes server is running on http://localhost:3000
# Set ADMIN_TOKEN to match server config (default: "admintoken")

ADMIN_TOKEN="${ADMIN_TOKEN:-admintoken}"
BASE_URL="${BASE_URL:-http://localhost:3000}"

echo "==== Testing Admin Endpoints ===="
echo "Base URL: $BASE_URL"
echo "Admin Token: $ADMIN_TOKEN"
echo ""

# Test 1: List all API keys
echo "1. GET /admin/list-keys"
curl -s -X GET "$BASE_URL/admin/list-keys" \
  -H "x-admin-token: $ADMIN_TOKEN" \
  -H "Content-Type: application/json" | jq '.' || echo "Failed"
echo ""

# Test 2: List all subscriptions
echo "2. GET /admin/list-subs"
curl -s -X GET "$BASE_URL/admin/list-subs" \
  -H "x-admin-token: $ADMIN_TOKEN" \
  -H "Content-Type: application/json" | jq '.' || echo "Failed"
echo ""

# Test 3: Add a new API key (manual admin operation)
echo "3. POST /admin/keys (add manual key)"
curl -s -X POST "$BASE_URL/admin/keys" \
  -H "x-admin-token: $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"key":"manual-test-key-001"}' | jq '.' || echo "Failed"
echo ""

# Test 4: Rotate key (requires an existing key; example uses hardcoded key)
echo "4. POST /admin/rotate-key (rotate by old key)"
SAMPLE_KEY="testkey"  # Change this to an actual key from list-keys
curl -s -X POST "$BASE_URL/admin/rotate-key" \
  -H "x-admin-token: $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":\"$SAMPLE_KEY\"}" | jq '.' || echo "Failed"
echo ""

# Test 5: Get usage stats
echo "5. GET /admin/usage"
curl -s -X GET "$BASE_URL/admin/usage" \
  -H "x-admin-token: $ADMIN_TOKEN" \
  -H "Content-Type: application/json" | jq '.' || echo "Failed"
echo ""

# Test 6: Health check
echo "6. GET /health"
curl -s -X GET "$BASE_URL/health" | jq '.' || echo "Failed"
echo ""

echo "==== Tests Complete ===="
