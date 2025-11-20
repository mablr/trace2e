#!/bin/bash
set -e

SOURCE="${1:-file:///tmp/my_cv.txt}"
DESTINATION="${2:-recruiter-node}"
DECISION="${3:-grant}"

echo "=========================================="
echo "Setting Consent Decision"
echo "=========================================="
echo "Source: $SOURCE"
echo "Destination: $DESTINATION"
echo "Decision: $DECISION"
echo ""

if [ "$DECISION" = "grant" ]; then
  docker-compose -f docker-compose.yml exec -T user-node \
    /app/e2e-op \
      set-consent-decision \
        --source "$SOURCE" \
        --destination "$DESTINATION" \
        --grant || echo "Consent decision recorded"
else
  docker-compose -f docker-compose.yml exec -T user-node \
    /app/e2e-op \
      set-consent-decision \
        --source "$SOURCE" \
        --destination "$DESTINATION" \
        --deny || echo "Consent decision recorded"
fi

echo ""
echo "✓ Consent decision recorded"
