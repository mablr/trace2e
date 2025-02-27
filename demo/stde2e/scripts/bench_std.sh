#!/bin/bash

stde2e_result=$(/file_forwarder_e2e -i /dev/null -o /dev/null)
std_result=$(/file_forwarder -i /dev/null -o /dev/null)

# Measure trace2e time
echo "{ $std_result $stde2e_result }"

 