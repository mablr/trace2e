#!/bin/bash

# Function to get time in microseconds
get_time_us() {
    date +%s%N | cut -b1-16
}

# Measure trace2e time
start_time=$(get_time_us)
/file_forwarder_e2e -i /dev/null -o /dev/null
end_time=$(get_time_us)
trace2e_time=$((end_time - start_time))

# Measure stock time
start_time=$(get_time_us)
/file_forwarder -i /dev/null -o /dev/null
end_time=$(get_time_us)
stock_time=$((end_time - start_time))

# Output in JSON format
echo "{ \"std (us)\": $stock_time, \"stde2e (us)\": $trace2e_time}"
 