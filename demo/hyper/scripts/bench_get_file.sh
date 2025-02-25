#!/bin/bash

# Function to get time in microseconds
get_time_us() {
    date +%s%N | cut -b1-16
}

# Measure trace2e time
start_time=$(get_time_us)
/hypere2e_client http://hyper_server:3001 > /dev/null
end_time=$(get_time_us)
trace2e_time=$((end_time - start_time))

# Measure stock time
start_time=$(get_time_us)
/hyper_client http://hyper_server:3002 > /dev/null
end_time=$(get_time_us)
stock_time=$((end_time - start_time))

# Output in JSON format
echo "{ \"stock (us)\": $stock_time, \"trace2e (us)\": $trace2e_time}"