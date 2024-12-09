# Hyper.rs demo for Trace2e middleware
## Overview

This folder provides a demo environment to test the Trace2e middleware using Docker. The environment consists of two services:

- hyper_server: Runs the middleware and provides a web service through multiple applications.
- hyper_client: Allows interactive execution of scripts for demo scenarios using `grpcurl` tool to interact with middleware through its client gRPC interface.

The infrastructure uses Docker Compose to manage the services and a shared scripts folder for client-side demo scripts.
Prerequisites

## Setup and Usage
### 1. Working directory

Have the Trace2e repository cloned on your machine, and this folder as working directory:
```
cd demo/hyper
```

### 2. Start the Environment
```
docker-compose up -d
```
This command starts the `hyper_server` and `hyper_client` services.
The hyper_client service is interactive and allows you to run demo scripts.

### 3. Access the Client container

Open an interactive shell inside the `hyper_client` container:
```
docker exec -it hyper_client bash
```

### 4. Visualize logs

If you want to see the middleware logs, run the following in second terminal, with the same working directory:
```
docker compose logs -f
```

## Running the Demo
### Step 1: Retrieve the File

Inside the `hyper_client` container, run the `get_file.sh` script:
```
./scripts/get_file.sh
```
This fetches the HTML file hosted by `hyper_server` and displays it.

### Step 2: Enable Confidentiality

Run the enable_confidentiality.sh script to enable local confidentiality for the file:
```
./scripts/enable_confidentiality.sh
```
This uses grpcurl to send a gRPC request to the server.

### Step 3: Attempt to Retrieve the File Again

Re-run the get_file.sh script:

./scripts/get_file.sh

The response will now be an incomplete message, demonstrating the confidentiality feature.

## Stopping the Environment

To stop and clean up all running containers:
```
docker-compose down
```

## Folder Structure

- `scripts/`: Contains the demo scripts to run from `hyper_client`:
    - `get_file.sh`: Fetches the file from the server.
    - `enable_confidentiality.sh`: Enables confidentiality for the file on the server.
- `*.Dockerfile`: Configuration files for building server and client containers.
- `compose.yml`: Orchestrates the multi-container setup.

## Additional Notes

- **Volumes**: The `scripts/` folder is shared with the `hyper_client` container, allowing you to modify and run scripts dynamically.
- **Ports**: The `hyper_server` exposes services internally within the Docker network, ensuring isolated communication between containers.
