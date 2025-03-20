#!/bin/bash

get_timings() {
    local enroll_p_sent=$(echo "$1" | grep "\[P2M\] P-> local_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local enroll_m_begin=$(echo "$2" | grep "\[P2M\] ->M local_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local enroll_m_done=$(echo "$2" | grep "\[P2M\] <-M local_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local enroll_p_recv=$(echo "$1" | grep "\[P2M\] P<- local_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local io_request_p_sent=$(echo "$1" | grep "\[P2M\] P-> io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_m_begin=$(echo "$2" | grep "\[P2M\] ->M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_m_done=$(echo "$2" | grep "\[P2M\] <-M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_p_recv=$(echo "$1" | grep "\[P2M\] P<- io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local io_report_p_sent=$(echo "$1" | grep "\[P2M\] P-> io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_m_begin=$(echo "$2" | grep "\[P2M\] ->M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_m_done=$(echo "$2" | grep "\[P2M\] <-M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_p_recv=$(echo "$1" | grep "\[P2M\] P<- io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local enroll_req_time=$(echo "$enroll_m_begin - $enroll_p_sent" | bc)
    local enroll_processing_time=$(echo "$enroll_m_done - $enroll_m_begin" | bc)
    local enroll_ack_time=$(echo "$enroll_p_recv - $enroll_m_done" | bc)

    local io_request_req_time=$(echo "$io_request_m_begin - $io_request_p_sent" | bc)
    local io_request_processing_time=$(echo "$io_request_m_done - $io_request_m_begin" | bc)
    local io_request_ack_time=$(echo "$io_request_p_recv - $io_request_m_done" | bc)

    local io_time=$(echo "$io_request_p_recv - $io_request_p_sent" | bc)

    local io_report_req_time=$(echo "$io_report_m_begin - $io_report_p_sent" | bc)
    local io_report_processing_time=$(echo "$io_report_m_done - $io_report_m_begin" | bc)
    local io_report_ack_time=$(echo "$io_report_p_recv - $io_report_m_done" | bc)

    echo "$enroll_req_time;$enroll_processing_time;$enroll_ack_time;$io_request_req_time;$io_request_processing_time;$io_request_ack_time;$io_time;$io_report_req_time;$io_report_processing_time;$io_report_ack_time"
}


NB_ITER=100

echo "read_enroll_req_time;read_enroll_processing_time;read_enroll_ack_time;read_io_request_req_time;read_io_request_processing_time;read_io_request_ack_time;read_io_time;read_io_report_req_time;read_io_report_processing_time;read_io_report_ack_time;write_enroll_req_time;write_enroll_processing_time;write_enroll_ack_time;write_io_request_req_time;write_io_request_processing_time;write_io_request_ack_time;write_io_time;write_io_report_req_time;write_io_report_processing_time;write_io_report_ack_time"

i=0
while [ $i -lt $NB_ITER ];do
    sudo truncate -s 0 $(docker inspect --format='{{.LogPath}}' stde2e)

    P_LOG=$(echo "RUST_LOG=info /file_forwarder_e2e -i /dev/null -o /dev/zero" | docker exec -i stde2e bash | ansi2txt)
    M_LOG=$(docker logs stde2e | ansi2txt)

    # echo -n "$P_LOG" > p_log.txt
    # echo -n "$M_LOG" > m_log.txt

    # P_LOG=$(cat p_log.txt)
    # M_LOG=$(cat m_log.txt)

    # if same amount of local_enroll, io_request, io_report in P_LOG and M_LOG
    if [ $(echo "$P_LOG" | grep "local_enroll" | wc -l) -ne $(echo "$M_LOG" | grep "local_enroll" | wc -l)\
        -o $(echo "$P_LOG" | grep "io_request" | wc -l) -ne $(echo "$M_LOG" | grep "io_request" | wc -l)\
        -o $(echo "$P_LOG" | grep "io_report" | wc -l) -ne $(echo "$M_LOG" | grep "io_report" | wc -l)\
        -o $(echo "$P_LOG" | grep "\[P2M\]" | wc -l) -ne "12"\
        -o $(echo "$M_LOG" | grep "\[P2M\]" | wc -l) -ne "12"\
        -o $(echo "$M_LOG" | grep "root@.*#" | wc -l) -ne "0" ];
    then
        echo "Error: incorrect logs, retrying..." >&2
        continue
    fi

    P_LOG_READ=$(echo "$P_LOG" | grep -v "\[DEMO\]" | head -n 6)
    M_LOG_READ=$(echo "$M_LOG" | head -n 6)
    P_LOG_WRITE=$(echo "$P_LOG" | grep -v "\[DEMO\]" | tail -n 6)
    M_LOG_WRITE=$(echo "$M_LOG"| tail -n 6)

    echo "$(get_timings "$P_LOG_READ" "$M_LOG_READ");$(get_timings "$P_LOG_WRITE" "$M_LOG_WRITE")"

    i=$((i+1))
done

