#!/bin/bash
set -e

echo "=========================================="
echo "Scenario 1: User Sends CV to Company"
echo "=========================================="
echo ""

echo "Step 1: User creates and sends CV..."
docker compose -f docker-compose.yml exec -T user-node \
  /app/e2e-proc --playbook /app/playbooks/scenario1_user_sends_cv.trace2e &
USER_PID=$!

echo "Step 2: Company receives and forwards CV..."
docker compose -f docker-compose.yml exec -T company-node \
  /app/e2e-proc --playbook /app/playbooks/scenario1_company_forward.trace2e &
COMPANY_PID=$!

wait $USER_PID

echo "Step 3: Recruiter receives CV..."
docker compose -f docker-compose.yml exec -T recruiter-node \
  /app/e2e-proc --playbook /app/playbooks/scenario1_recruiter_receive.trace2e &
RECRUITER_PID=$!

# Wait for all to complete
wait $COMPANY_PID $RECRUITER_PID

echo "Step 4: Validating scenario completion..."
output=$(docker compose -f docker-compose.yml exec -T recruiter-node /app/e2e-op get-references file:///tmp/forwarded_cv.txt)

if echo "$output" | grep -q "file:///tmp/my_cv.txt@172.20.0.10"; then
  echo ""
  echo "✓ Scenario 1 Complete"
  echo "  - User created and sent CV: /tmp/my_cv.txt"
  echo "  - Company received and forwarded CV: /tmp/received_cv.txt"
  echo "  - Recruiter received CV: /tmp/forwarded_cv.txt"
  echo ""
else
  echo ""
  echo "✗ Scenario 1 Failed"
  echo "Expected reference not found in forwarded CV"
  echo "$output"
  echo ""
  exit 1
fi