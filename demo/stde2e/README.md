# StdE2E demo for TracE2E middleware
## Overview

This demo showcases the performance comparison of I/O operations with Rust standard library and our modified version that supports end-to-end traceability with TracE2E middleware.

## Prerequisites

- Docker and Docker Compose
- Python with pandas, numpy, and matplotlib (for analysis)

## Running the Demo

1. **Start the Docker container**:

   ```bash
   cd demo/stde2e
   docker compose up -d
   ```

2. **Generate performance data**:

   Run the benchmarking scripts to compare standard I/O library performance with our modified version:

   ```bash
   # Benchmark File operations
   ./scripts/bench_stde2e_breakdown_file.sh > analysis/data/stde2e_breakdown_file.csv

   # Benchmark TCP stream operations
   ./scripts/bench_stde2e_breakdown_tcpstream.sh > analysis/data/stde2e_breakdown_tcpstream.csv
   ```

3. **Analyze the results**:

   Open and run the Jupyter notebook to visualize the performance comparison:

   ```bash
   cd analysis
   jupyter notebook stde2e_performance.ipynb
   ```

## Understanding the Results

The analysis compares:

- **Standard execution time**: Baseline performance of standard Rust I/O operations
- **StdE2E execution time**: Performance of the same operations with E2E traceability, this includes the overhead of the TracE2E communication protocols.

The benchmark covers two resource types:
- File operations (read/write)
- TCP stream operations (client/server communication)

## Stopping the Demo

To stop the demo, run:

```bash
docker compose down
```
