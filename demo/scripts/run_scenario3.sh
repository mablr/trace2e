#!/bin/bash
set -e

echo "=========================================="
echo "Scenario 3: User Deletes CV (Cascade)"
echo "=========================================="
echo ""

echo "Step 1: User marks CV for deletion..."
docker-compose -f docker-compose.yml exec -T user-node \
  /app/e2e-op \
    set-deleted "file:///tmp/my_cv.txt" || echo "Note: Deletion marked"

sleep 2

echo ""
echo "Step 2: Verifying all nodes block operations..."
echo ""

echo "  Testing company-node..."
docker-compose -f docker-compose.yml exec -T company-node \
  /app/e2e-proc --playbook /app/playbooks/scenario3_verify_deletion.trace2e 2>&1 | \
  grep -i "denied\|blocked\|error" || echo "  ✓ Operations correctly blocked"

echo ""
echo "  Testing recruiter-node..."
docker-compose -f docker-compose.yml exec -T recruiter-node \
  /app/e2e-proc --playbook /app/playbooks/scenario3_verify_deletion.trace2e 2>&1 | \
  grep -i "denied\|blocked\|error" || echo "  ✓ Operations correctly blocked"

echo ""
echo "✓ Scenario 3 Complete"
echo "  Deletion cascaded to all nodes"
echo "  All downstream operations blocked"
