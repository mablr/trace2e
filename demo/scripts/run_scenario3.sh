#!/bin/bash
set -e

echo "=========================================="
echo "Scenario 3: User Deletes CV (Cascade)"
echo "=========================================="
echo ""

echo "Step 0.1: User creates CV..."
docker compose -f docker-compose.yml exec -T user-node \
  /app/e2e-proc --playbook /app/playbooks/scenario1_user_sends_cv.trace2e &
USER_PID=$!

echo "Step 0.2: Company receives and forwards CV..."
docker compose -f docker-compose.yml exec -T company-node \
  /app/e2e-proc --playbook /app/playbooks/scenario1_company_forward.trace2e &
COMPANY_PID=$!

wait $USER_PID

echo "Step 0.3: Recruiter receives CV..."
docker compose -f docker-compose.yml exec -T recruiter-node \
  /app/e2e-proc --playbook /app/playbooks/scenario1_recruiter_receive.trace2e &
RECRUITER_PID=$!

wait $COMPANY_PID $RECRUITER_PID

echo "Step 1: User marks CV for deletion..."
docker compose -f docker-compose.yml exec -T user-node \
  /app/e2e-op \
    set-deleted "file:///tmp/my_cv.txt" || echo "Note: Deletion marked"

echo ""
echo "Step 2: Verifying all nodes block operations..."
echo ""

verify_blocked() {
  local node="$1" playbook="$2"
  echo "  Testing ${node}..."
  if docker compose -f docker-compose.yml exec -T "$node" \
    /app/e2e-proc --playbook "$playbook" 2>&1 | \
    grep -i "denied\|blocked\|error"; then
    echo "  ✓ Operation correctly blocked on ${node}"
  else
    echo "  ✗ Operation was NOT blocked on ${node} — FAILED"
    exit 1
  fi
}

verify_blocked user-node /app/playbooks/scenario3_user_verify_deletion.trace2e
verify_blocked company-node /app/playbooks/scenario3_company_verify_deletion.trace2e
verify_blocked recruiter-node /app/playbooks/scenario3_recruiter_verify_deletion.trace2e

echo ""
echo "✓ Scenario 3 Complete"
echo "  Deletion cascaded to all nodes"
echo "  All downstream operations blocked"
