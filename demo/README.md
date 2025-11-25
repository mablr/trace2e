# Trace2E Job Application Demo

Demonstrates distributed data flow tracking, consent management, and deletion cascade
across 3 containerized nodes representing a realistic job application scenario.

## Scenario

1. **User** creates CV and sends to **Company**
2. **Company** attempts to forward CV to **Recruiter** (requires user consent)
3. **User** deletes CV → deletion cascades to all nodes

## Architecture

```
┌─────────────┐         ┌─────────────┐         ┌─────────────┐
│  user-node  │────────>│company-node │────────>│recruiter-   │
│ 172.20.0.10 │         │ 172.20.0.20 │         │  node       │
│   :50051    │         │   :50051    │         │ 172.20.0.30 │
│             │         │             │         │   :50051    │
│  CV.txt     │         │ received_cv │         │ forwarded_cv│
│  (source)   │         │  (copy 1)   │         │  (copy 2)   │
└─────────────┘         └─────────────┘         └─────────────┘
```

## Quick Start

```bash
# Build and start all containers
./demo/run_demo.sh start

# Run all scenarios
./demo/run_demo.sh all

# Or run individually
./demo/run_demo.sh scenario1
./demo/run_demo.sh scenario2
./demo/run_demo.sh scenario3

# Stop demo
./demo/run_demo.sh stop

# Cleanup everything
./demo/run_demo.sh clean
```

## Scenarios

### Scenario 1: User Sends CV to Company

Demonstrates basic cross-node data flow with provenance tracking.

```bash
./demo/run_demo.sh scenario1
```

**What happens:**
- User creates CV file on user-node
- User establishes TCP connection to company-node
- CV data flows from user-node → company-node
- Company stores received CV
- Provenance: `file:///tmp/my_cv.txt@user-node` → `file:///tmp/received_cv.txt@company-node`

### Scenario 2: Company Forwards (Consent Required)

Demonstrates consent enforcement for third-party data sharing.

```bash
./demo/run_demo.sh scenario1
./demo/run_demo.sh scenario2
```

**What happens:**
- Recuiter attempts to download the CV
- System detects third-party sharing
- Consent request sent to user-node
- **User prompted to GRANT or DENY**
- If granted: CV flows to recruiter
- If denied: Flow blocked, no data transfer

### Scenario 3: User Deletes CV (Cascade)

Demonstrates deletion broadcast and enforcement across all nodes.

```bash
./demo/run_demo.sh scenario1
./demo/run_demo.sh scenario3
```

**What happens:**
- User marks original CV for deletion
- Deletion status broadcast to company-node and recruiter-node
- System marks all derived copies as DELETED
- All subsequent I/O operations blocked on all nodes
- Verification: Read/write attempts fail with permission denied

## Interactive Mode

Access any node for manual testing:

```bash
# User node
docker compose exec user-node /bin/bash
/app/e2e-proc  # Start interactive mode

# Company node
docker compose exec company-node /bin/bash

# Recruiter node
docker compose exec recruiter-node /bin/bash
```

## Manual Operations

### Create and send CV manually:
```bash
docker compose exec user-node /app/e2e-proc
> CREATE /tmp/my_cv.txt
> WRITE file:///tmp/my_cv.txt
> CONNECT company-node:8080
> WRITE stream://user-node:8080::company-node:8080
```

### Monitor consent requests:
```bash
./demo/scripts/monitor_consent.sh
```

### Grant/deny consent:
```bash
./demo/scripts/grant_consent.sh file:///tmp/my_cv.txt recruiter-node grant
./demo/scripts/grant_consent.sh file:///tmp/my_cv.txt recruiter-node deny
```

### Check provenance:
```bash
docker compose exec user-node /app/e2e-op get-references file:///tmp/my_cv.txt
```

## Troubleshooting

### View logs:
```bash
docker compose logs -f
docker compose logs user-node
docker compose logs company-node
```

### Check middleware status:
```bash
docker compose ps
docker compose exec user-node ss -tlnp | grep 50051
```

### Reset everything:
```bash
./demo/run_demo.sh restart
```

## Directory Structure

```
demo/
├── playbooks/
│   ├── scenario1_user_sends_cv.trace2e
│   ├── scenario1_company_receive.trace2e
│   ├── scenario2_company_forward.trace2e
│   ├── scenario2_recruiter_receive.trace2e
│   └── scenario3_verify_deletion.trace2e
├── scripts/
│   ├── run_scenario1.sh
│   ├── run_scenario2.sh
│   ├── run_scenario3.sh
│   └── monitor_consent.sh
├── data/
│   └── (holds demo data files)
├── run_demo.sh
└── README.md
```

## Key Concepts Demonstrated

- **Cross-node provenance tracking**: Data lineage maintained across containers
- **M2M communication**: Middlewares communicate over Docker network
- **Consent enforcement**: User controls third-party data sharing
- **Deletion cascade**: Deletion propagates to all derived copies
- **Policy enforcement**: Confidentiality levels respected across nodes

## Environment

The demo uses:
- **Docker Compose** for orchestration
- **3 containers** (user-node, company-node, recruiter-node)
- **Custom bridge network** (172.20.0.0/16 subnet)
- **Named volumes** for persistent data
- **Health checks** for service readiness

## Notes

- The demo is designed to be run locally with Docker and Docker Compose
- Each scenario is independent (can run in isolation)
- Scenarios 1 and 3 are fully automated
- Scenario 2 is interactive (requires user input for consent)
- All data is stored in container volumes for demo purposes
- Network communication uses hostnames (Docker's internal DNS)
