#!/bin/bash

export RUST_LOG="info"
NB_ITER=100

echo "std_total_time;stde2e_total_time;p2m_m2m_overhead"

for i in $(seq 1 $NB_ITER);do
    STD_RESULT=$(/tcp_server & /tcp_client)
    STDE2E_RESULT=$(/tcp_server_e2e & /tcp_client_e2e)
    STD_TOTAL_TIME=$(echo "$STD_RESULT" | awk '{print $6}')
    STDE2E_P2M_M2M_OVERHEAD=$(echo "$STDE2E_RESULT" | grep "\[P2M\]" | awk '{print $6}' | paste -sd+ | bc)
    STDE2E_TOTAL_TIME=$(echo "$STDE2E_RESULT" | grep "\[DEMO\]" | awk '{print $6}')
    echo "$STD_TOTAL_TIME;$STDE2E_TOTAL_TIME;$STDE2E_P2M_M2M_OVERHEAD"
done

