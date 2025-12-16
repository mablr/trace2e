#!/bin/bash
set -e

echo "=========================================="
echo "Benchmark 0: RTT (Round Trip Time)"
echo "=========================================="
echo ""

echo "Step 1: Starting RTT server on company-node..."
docker compose -f docker-compose.yml exec -T company-node \
  /app/e2e-proc --playbook /app/playbooks/benchmark0_rtt_server.trace2e &
SERVER_PID=$!
echo "Step 2: Running RTT client from user-node..."
docker compose -f docker-compose.yml exec -T user-node \
  /app/e2e-proc --playbook /app/playbooks/benchmark0_rtt_client.trace2e &
CLIENT_PID=$!

# Wait for both to complete
wait $SERVER_PID $CLIENT_PID
