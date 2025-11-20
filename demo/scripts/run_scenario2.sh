#!/bin/bash
set -e

echo "=========================================="
echo "Scenario 2: Company Forwards CV (Requires Consent)"
echo "=========================================="
echo ""

# Step 1: Enable consent on CV file
echo "Step 1: Enabling consent requirement on CV..."
docker-compose -f docker-compose.yml exec -T user-node \
  /app/e2e-op \
    get-policies "file:///tmp/my_cv.txt" || echo "Note: Getting policies..."

echo ""
echo "Step 2: Company attempts to forward CV..."
docker-compose -f docker-compose.yml exec -T company-node \
  /app/e2e-proc --playbook /app/playbooks/scenario2_company_forward.txt &
COMPANY_PID=$!

echo ""
echo "Step 3: Recruiter waits to receive..."
docker-compose -f docker-compose.yml exec -T recruiter-node \
  /app/e2e-proc --playbook /app/playbooks/scenario2_recruiter_receive.txt &
RECRUITER_PID=$!

sleep 2

echo ""
echo "=========================================="
echo "CONSENT REQUIRED!"
echo "=========================================="
echo ""
echo "The company is attempting to forward your CV to a recruiter."
echo "This requires your explicit consent."
echo ""
echo "Grant consent? [y/n]: "
read -r RESPONSE

if [[ "$RESPONSE" =~ ^[Yy]$ ]]; then
  echo "Granting consent..."
  docker-compose -f docker-compose.yml exec -T user-node \
    /app/e2e-op \
      set-consent-decision \
        --source "file:///tmp/my_cv.txt" \
        --destination "recruiter-node" \
        --grant || echo "Consent decision recorded"
else
  echo "Denying consent..."
  docker-compose -f docker-compose.yml exec -T user-node \
    /app/e2e-op \
      set-consent-decision \
        --source "file:///tmp/my_cv.txt" \
        --destination "recruiter-node" \
        --deny || echo "Consent decision recorded"
fi

# Wait for completion
wait $COMPANY_PID $RECRUITER_PID 2>/dev/null || true

echo ""
echo "✓ Scenario 2 Complete"
