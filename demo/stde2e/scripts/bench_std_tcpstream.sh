#!/bin/bash

get_timings() {
    local enroll_p_sent=$(echo "$1" | grep "\[P2M\] P-> remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local enroll_m_begin=$(echo "$1" | grep "\[P2M\] ->M remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local enroll_m_done=$(echo "$1" | grep "\[P2M\] <-M remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local enroll_p_recv=$(echo "$1" | grep "\[P2M\] P<- remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local io_request_p_sent=$(echo "$1" | grep "\[P2M\] P-> io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_m_begin=$(echo "$1" | grep "\[P2M\] ->M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_m_done=$(echo "$1" | grep "\[P2M\] <-M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_p_recv=$(echo "$1" | grep "\[P2M\] P<- io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local io_report_p_sent=$(echo "$1" | grep "\[P2M\] P-> io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_m_begin=$(echo "$1" | grep "\[P2M\] ->M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_m_done=$(echo "$1" | grep "\[P2M\] <-M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_p_recv=$(echo "$1" | grep "\[P2M\] P<- io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local enroll_req_time=$(((enroll_m_begin - enroll_p_sent) / 2))
    local enroll_processing_time=$((enroll_m_done - enroll_m_begin))
    local enroll_ack_time=$(((enroll_p_recv - enroll_m_done) / 2))

    local io_request_req_time=$(((io_request_m_begin - io_request_p_sent) / 2))
    local io_request_processing_time=$((io_request_m_done - io_request_m_begin))
    local io_request_ack_time=$(((io_request_p_recv - io_request_m_done) / 2))

    local io_time=$((io_report_p_sent - io_request_p_recv))

    local io_report_req_time=$(((io_report_m_begin - io_report_p_sent) / 2))
    local io_report_processing_time=$((io_report_m_done - io_report_m_begin))
    local io_report_ack_time=$(((io_report_p_recv - io_report_m_done) / 2))

    echo "$enroll_req_time;$enroll_processing_time;$enroll_ack_time;$io_request_req_time;$io_request_processing_time;$io_request_ack_time;$io_time;$io_report_req_time;$io_report_processing_time;$io_report_ack_time"
}

get_timings_m2m() {
    local write_reserve_l_sent=$(echo "$1" | grep "\[M2M\] LM-> reserve" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local write_reserve_r_begin=$(echo "$1" | grep "\[M2M\] ->RM reserve" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local write_reserve_r_done=$(echo "$1" | grep "\[M2M\] <-RM reserve" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local write_reserve_l_recv=$(echo "$1" | grep "\[M2M\] LM<- reserve" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local write_sync_prov_l_sent=$(echo "$1" | grep "\[M2M\] LM-> sync_prov" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local write_sync_prov_r_begin=$(echo "$1" | grep "\[M2M\] ->RM sync_prov" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local write_sync_prov_r_done=$(echo "$1" | grep "\[M2M\] <-RM sync_prov" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local write_sync_prov_l_recv=$(echo "$1" | grep "\[M2M\] LM<- sync_prov" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local write_reserve_req_time=$((write_reserve_r_begin - write_reserve_l_sent))
    local write_reserve_processing_time=$((write_reserve_r_done - write_reserve_r_begin))
    local write_reserve_ack_time=$((write_reserve_l_recv - write_reserve_r_done))

    local write_sync_prov_req_time=$((write_sync_prov_r_begin - write_sync_prov_l_sent))
    local write_sync_prov_processing_time=$((write_sync_prov_r_done - write_sync_prov_r_begin))
    local write_sync_prov_ack_time=$((write_sync_prov_l_recv - write_sync_prov_r_done))

    echo "$write_reserve_req_time;$write_reserve_processing_time;$write_reserve_ack_time;$write_sync_prov_req_time;$write_sync_prov_processing_time;$write_sync_prov_ack_time"
}

NB_ITER=1000

echo "read_enroll_req_time;read_enroll_processing_time;read_enroll_ack_time;read_io_request_req_time;read_io_request_processing_time;read_io_request_ack_time;read_io_time;read_io_report_req_time;read_io_report_processing_time;read_io_report_ack_time;write_enroll_req_time;write_enroll_processing_time;write_enroll_ack_time;write_io_request_req_time;write_io_request_processing_time;write_io_request_ack_time;write_io_time;write_io_report_req_time;write_io_report_processing_time;write_io_report_ack_time;write_reserve_req_time;write_reserve_processing_time;write_reserve_ack_time;write_sync_prov_req_time;write_sync_prov_processing_time;write_sync_prov_ack_time"

i=0
while [ $i -lt $NB_ITER ];do

    LOG=$(echo 'RUST_LOG="trace2e_middleware::p2m_service=info" /trace2e_middleware & middleware=$! && RUST_LOG=trace2e_client=info /tcp_server_e2e; kill "$middleware"' | docker exec -i stde2e_server bash 2>/dev/null | ansi2txt & echo 'RUST_LOG="trace2e_middleware::p2m_service=info" /trace2e_middleware & middleware=$! && sleep 0.1 && RUST_LOG=trace2e_client=info /tcp_client_e2e stde2e_server:8888; kill "$middleware"' | docker exec -i stde2e_client bash  2>/dev/null | ansi2txt)

    LOG_M2M=$(echo 'RUST_LOG="trace2e_middleware::m2m_service=info,trace2e_middleware::traceability=info" /trace2e_middleware & middleware=$! && /tcp_server_e2e; kill "$middleware"' | docker exec -i stde2e_server bash 2>/dev/null | ansi2txt & echo 'RUST_LOG="trace2e_middleware::m2m_service=info,trace2e_middleware::traceability=info" /trace2e_middleware & middleware=$! && sleep 0.1 && /tcp_client_e2e stde2e_server:8888; kill "$middleware"' | docker exec -i stde2e_client bash  2>/dev/null | ansi2txt)
    if [ $(echo "$LOG" | grep "remote_enroll" | wc -l) -ne "8"\
        -o $(echo "$LOG" | grep "io_request" | wc -l) -ne "8"\
        -o $(echo "$LOG" | grep "io_report" | wc -l) -ne "8"\
        -o $(echo "$LOG" | grep "root@.*#" | wc -l) -ne "0"\
        -o $(echo "$LOG_M2M" | grep "reserve" | wc -l) -ne "4"\
        -o $(echo "$LOG_M2M" | grep "sync_prov" | wc -l) -ne "4"\
        -o $(echo "$LOG_M2M" | grep "root@.*#" | wc -l) -ne "0" ];
    then
        echo "Error: incorrect logs, retrying..." >&2
        continue
    fi

    FD_TO_READ=$(echo "$LOG" | grep "P-> remote_enroll" | grep "8888])$" | awk -F', ' '{print $2}')

    LOG_READ=$(echo "$LOG" | grep "$FD_TO_READ")
    LOG_WRITE=$(echo "$LOG" | grep -v "$FD_TO_READ")

    echo "$(get_timings "$LOG_READ");$(get_timings "$LOG_WRITE");$(get_timings_m2m "$LOG_M2M")"
    i=$((i+1))
done

