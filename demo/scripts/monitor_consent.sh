#!/bin/bash
set -e

echo "=========================================="
echo "Monitoring Consent Requests on User Node"
echo "=========================================="
echo ""
echo "Listening for consent requests..."
echo "Press Ctrl+C to stop"
echo ""

# Monitor consent for the original CV file
docker compose -f docker-compose.yml exec -T user-node \
  /app/e2e-op \
    enforce-consent "file:///tmp/my_cv.txt"
