#!/bin/bash
set -e

DEMO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DEMO_DIR/.."

usage() {
  cat <<EOF
Usage: ./demo/run_demo.sh [COMMAND]

Commands:
  start       - Start all containers
  stop        - Stop all containers
  restart     - Restart all containers
  logs        - Show logs from all containers
  scenario1   - Run Scenario 1 (User sends CV)
  scenario2   - Run Scenario 2 (Company forwards, needs consent)
  scenario3   - Run Scenario 3 (User deletes CV, cascade)
  all         - Run all scenarios in sequence
  interactive - Start interactive shell on user-node
  clean       - Stop and remove all containers and volumes

Examples:
  ./demo/run_demo.sh start
  ./demo/run_demo.sh scenario1
  ./demo/run_demo.sh all
EOF
}

case "${1:-help}" in
  start)
    echo "Starting trace2e demo infrastructure..."
    docker-compose -f docker-compose.yml up -d
    echo "Waiting for services to be healthy..."
    sleep 5
    echo "✓ All services ready"
    ;;

  stop)
    echo "Stopping trace2e demo..."
    docker-compose -f docker-compose.yml stop
    ;;

  restart)
    echo "Restarting trace2e demo..."
    docker-compose -f docker-compose.yml restart
    ;;

  logs)
    docker-compose -f docker-compose.yml logs -f
    ;;

  scenario1)
    ./demo/scripts/run_scenario1.sh
    ;;

  scenario2)
    ./demo/scripts/run_scenario2.sh
    ;;

  scenario3)
    ./demo/scripts/run_scenario3.sh
    ;;

  all)
    echo "Running all scenarios..."
    echo ""
    ./demo/scripts/run_scenario1.sh
    sleep 2
    ./demo/scripts/run_scenario2.sh
    sleep 2
    ./demo/scripts/run_scenario3.sh
    echo ""
    echo "=========================================="
    echo "All scenarios complete!"
    echo "=========================================="
    ;;

  interactive)
    echo "Starting interactive shell on user-node..."
    docker-compose -f docker-compose.yml exec user-node /bin/bash
    ;;

  clean)
    echo "Cleaning up demo environment..."
    docker-compose -f docker-compose.yml down -v
    echo "✓ Cleanup complete"
    ;;

  help|*)
    usage
    ;;
esac
