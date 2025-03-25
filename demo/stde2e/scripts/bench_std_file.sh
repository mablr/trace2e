#!/bin/bash

get_timings() {
    local enroll_p_sent=$(echo "$1" | grep "\[P2M\] P-> local_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local enroll_m_begin=$(echo "$1" | grep "\[P2M\] ->M local_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local enroll_m_done=$(echo "$1" | grep "\[P2M\] <-M local_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local enroll_p_recv=$(echo "$1" | grep "\[P2M\] P<- local_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local io_request_p_sent=$(echo "$1" | grep "\[P2M\] P-> io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_m_begin=$(echo "$1" | grep "\[P2M\] ->M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_m_done=$(echo "$1" | grep "\[P2M\] <-M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_p_recv=$(echo "$1" | grep "\[P2M\] P<- io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local io_report_p_sent=$(echo "$1" | grep "\[P2M\] P-> io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_m_begin=$(echo "$1" | grep "\[P2M\] ->M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_m_done=$(echo "$1" | grep "\[P2M\] <-M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_p_recv=$(echo "$1" | grep "\[P2M\] P<- io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local enroll_req_time=$(echo "$enroll_m_begin - $enroll_p_sent" | bc)
    local enroll_processing_time=$(echo "$enroll_m_done - $enroll_m_begin" | bc)
    local enroll_ack_time=$(echo "$enroll_p_recv - $enroll_m_done" | bc)

    local io_request_req_time=$(echo "$io_request_m_begin - $io_request_p_sent" | bc)
    local io_request_processing_time=$(echo "$io_request_m_done - $io_request_m_begin" | bc)
    local io_request_ack_time=$(echo "$io_request_p_recv - $io_request_m_done" | bc)

    local io_time=$(echo "$io_report_p_sent - $io_request_p_recv" | bc)

    local io_report_req_time=$(echo "$io_report_m_begin - $io_report_p_sent" | bc)
    local io_report_processing_time=$(echo "$io_report_m_done - $io_report_m_begin" | bc)
    local io_report_ack_time=$(echo "$io_report_p_recv - $io_report_m_done" | bc)

    echo "$enroll_req_time;$enroll_processing_time;$enroll_ack_time;$io_request_req_time;$io_request_processing_time;$io_request_ack_time;$io_time;$io_report_req_time;$io_report_processing_time;$io_report_ack_time"
}


NB_ITER=1000

echo "read_enroll_req_time;read_enroll_processing_time;read_enroll_ack_time;read_io_request_req_time;read_io_request_processing_time;read_io_request_ack_time;read_io_time;read_io_report_req_time;read_io_report_processing_time;read_io_report_ack_time;write_enroll_req_time;write_enroll_processing_time;write_enroll_ack_time;write_io_request_req_time;write_io_request_processing_time;write_io_request_ack_time;write_io_time;write_io_report_req_time;write_io_report_processing_time;write_io_report_ack_time"

i=0
while [ $i -lt $NB_ITER ];do

    LOG=$(echo 'RUST_LOG="trace2e_middleware::p2m_service=info" /trace2e_middleware & middleware=$! && RUST_LOG=info /file_forwarder_e2e -i /dev/null -o /dev/zero; kill "$middleware"' | docker exec -i stde2e_client bash | ansi2txt)

    # if same amount of local_enroll, io_request, io_report in P_LOG and M_LOG
    if [ $(echo "$LOG" | grep "local_enroll" | wc -l) -ne "8"\
        -o $(echo "$LOG" | grep "io_request" | wc -l) -ne "8"\
        -o $(echo "$LOG" | grep "io_report" | wc -l) -ne "8"\
        -o $(echo "$LOG" | grep "\[P2M\]" | wc -l) -ne "24"\
        -o $(echo "$LOG" | grep "root@.*#" | wc -l) -ne "0" ];
    then
        echo "Error: incorrect logs, retrying..." >&2
        continue
    fi

    LOG_READ=$(echo "$LOG" | head -n 12)
    LOG_WRITE=$(echo "$LOG"| tail -n 12)

    echo "$(get_timings "$LOG_READ");$(get_timings "$LOG_WRITE")"

    i=$((i+1))
done

