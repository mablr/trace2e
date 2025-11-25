#!/bin/bash
set -e

echo "=========================================="
echo "Scenario 1: User Sends CV to Company"
echo "=========================================="
echo ""

echo "Step 1: User creates CV..."
docker-compose -f docker-compose.yml exec -T user-node \
  /app/e2e-proc --playbook /app/playbooks/scenario1_user_sends_cv.trace2e &
USER_PID=$!

echo "Step 2: Company waits to receive CV..."
docker-compose -f docker-compose.yml exec -T company-node \
  /app/e2e-proc --playbook /app/playbooks/scenario1_company_receive.trace2e &
COMPANY_PID=$!

# Wait for both to complete
wait $USER_PID $COMPANY_PID

echo ""
echo "✓ Scenario 1 Complete"
echo "  - User created CV: /tmp/my_cv.txt"
echo "  - Company received CV: /tmp/received_cv.txt"
echo ""
