#!/bin/bash
set -e


echo "=========================================="
echo "Scenario 2: User Consent for CV Forwarding"
echo "=========================================="
echo ""

echo "Step 0.1: User creates CV..."
docker compose -f docker-compose.yml exec -T user-node \
  /app/e2e-proc --playbook /app/playbooks/scenario1_user_sends_cv.trace2e &
USER_PID=$!

echo "Step 0.2: Company waits to receive CV..."
docker compose -f docker-compose.yml exec -T company-node \
  /app/e2e-proc --playbook /app/playbooks/scenario2_company_receive.trace2e &
COMPANY_PID=$!

# Wait for both to complete
wait $USER_PID $COMPANY_PID

echo "  - User created CV: /tmp/my_cv.txt"
echo "  - Company received CV: /tmp/received_cv.txt"
echo ""

# Step 1: Start consent enforcement on user-node (background process)
# This sets the consent policy flag AND creates the notification channel
# Output is piped to allow us to detect when notifications arrive
echo "Step 1: Setting up consent enforcement on CV file..."
docker compose -f docker-compose.yml exec -T user-node \
  /app/e2e-op \
    enforce-consent "file:///tmp/my_cv.txt" 2>&1 &
CONSENT_MONITOR_PID=$!


# Pre-emptively set consent decisions for known destinations
# (In a real scenario, these would be requested dynamically via notifications)
docker compose -f docker-compose.yml exec -T user-node \
  /app/e2e-op \
    set-consent-decision \
      --source "file:///tmp/my_cv.txt" \
      --destination "172.20.0.10" \
      --grant || echo "Consent decision recorded"
docker compose -f docker-compose.yml exec -T user-node \
  /app/e2e-op \
    set-consent-decision \
      --source "file:///tmp/my_cv.txt" \
      --destination "172.20.0.20" \
      --grant || echo "Consent decision recorded"

echo ""
echo "Step 2: Company attempts to forward CV..."
docker compose -f docker-compose.yml exec -T company-node \
  /app/e2e-proc --playbook /app/playbooks/scenario2_company_send.trace2e &
COMPANY_PID=$!

echo ""
echo "Step 3: Recruiter waits to receive..."
docker compose -f docker-compose.yml exec -T recruiter-node \
  /app/e2e-proc --playbook /app/playbooks/scenario2_recruiter_receive.trace2e &
RECRUITER_PID=$!

sleep 1
echo ""
echo "=========================================="
echo "CONSENT REQUEST RECEIVED!"
echo "=========================================="
echo ""
echo "The company is attempting to forward your CV to a recruiter."
echo "This requires your explicit consent."
echo ""
echo "Grant consent? [y/n]: "
read -r RESPONSE

if [[ "$RESPONSE" =~ ^[Yy]$ ]]; then
  echo "Granting consent..."
  docker compose -f docker-compose.yml exec -T user-node \
    /app/e2e-op \
      set-consent-decision \
        --source "file:///tmp/my_cv.txt" \
        --destination "172.20.0.30" \
        --grant || echo "Consent decision recorded"
else
  echo "Denying consent..."
  docker compose -f docker-compose.yml exec -T user-node \
    /app/e2e-op \
      set-consent-decision \
        --source "file:///tmp/my_cv.txt" \
        --destination "172.20.0.30" \
        --deny || echo "Consent decision recorded"
fi

# Clean up processes and temporary files
kill $CONSENT_MONITOR_PID 2>/dev/null || true

# Wait for completion
wait $COMPANY_PID $RECRUITER_PID 2>/dev/null || true

echo ""
echo "✓ Scenario 2 Complete"
