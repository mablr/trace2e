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
docker compose up -d
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

## Running the Demo on Confidentiality
### Step 1: Retrieve the File

Inside the `hyper_client` container, run the `read_confidential_file.sh` script:
```
cat scripts/read_confidential_file.sh | docker exec -i hyper_client bash
```
This fetches the HTML file hosted by `hyper_server` and displays it.

### Step 2: Enable Confidentiality

Run the enable_confidentiality.sh script to enable local confidentiality for the file:
```
cat scripts/enable_confidentiality.sh | docker exec -i hyper_client bash
```
This uses grpcurl to send a gRPC request to the server.

### Step 3: Attempt to Retrieve the File Again

Re-run the read_confidential_file.sh script:
```
cat scripts/read_confidential_file.sh | docker exec -i hyper_client bash
```
The response will now be an incomplete message, demonstrating the confidentiality feature.

## Running the Demo on Integrity
### Step 1: Try to alter the template.html file

Run the alter_template.sh script:
```
cat scripts/alter_template.sh | docker exec -i hyper_client bash
```
This will attempt to alter the remote template.html file.

### Step 2: Enable Integrity

Run the enable_integrity.sh script to enable local integrity for the file:
```
cat scripts/enable_integrity.sh | docker exec -i hyper_client bash
```
This will enable local integrity for the remote template.html file.

### Step 3: Try to alter the template.html file again

Re-run the alter_template.sh script:
```
cat scripts/alter_template.sh | docker exec -i hyper_client bash
```
This will now fail, demonstrating the integrity feature.

## Stopping the Environment

To stop and clean up all running containers:
```
docker compose down
```

## Folder Structure

- `scripts/`: Contains the demo scripts to run from `hyper_client`:
    - `read_confidential_file.sh`: Fetches the confidential file from the server.
    - `enable_confidentiality.sh`: Enables confidentiality for the file on the server.
    - `alter_protected_file.sh`: Attempts to alter the template.html file.
    - `enable_integrity.sh`: Enables integrity for the template.html file.
- `compose.yml`: Orchestrates the multi-container setup.

## Additional Notes

- **Volumes**: The `scripts/` folder is shared with the `hyper_client` container, allowing you to modify and run scripts dynamically.
- **Ports**: The `hyper_server` exposes services internally within the Docker network, ensuring isolated communication between containers.
