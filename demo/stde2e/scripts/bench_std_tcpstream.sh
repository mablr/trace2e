#!/bin/bash

get_timings_read() {
    local remote_enroll_p_sent=$(echo "$1" | grep "\[P2M\] P-> remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local remote_enroll_m_begin=$(echo "$2" | grep "\[P2M\] ->M remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local remote_enroll_m_done=$(echo "$2" | grep "\[P2M\] <-M remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local remote_enroll_p_recv=$(echo "$1" | grep "\[P2M\] P<- remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local io_request_p_sent=$(echo "$1" | grep "\[P2M\] P-> io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_m_begin=$(echo "$2" | grep "\[P2M\] ->M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_m_done=$(echo "$2" | grep "\[P2M\] <-M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_p_recv=$(echo "$1" | grep "\[P2M\] P<- io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local io_report_p_sent=$(echo "$1" | grep "\[P2M\] P-> io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_m_begin=$(echo "$2" | grep "\[P2M\] ->M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_m_done=$(echo "$2" | grep "\[P2M\] <-M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_p_recv=$(echo "$1" | grep "\[P2M\] P<- io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local remote_enroll_communication_time=$(echo "$remote_enroll_m_begin - $remote_enroll_p_sent + $remote_enroll_p_recv - $remote_enroll_m_done" | bc)
    local io_request_communication_time=$(echo "$io_request_m_begin - $io_request_p_sent + $io_request_p_recv - $io_request_m_done" | bc)
    local io_report_communication_time=$(echo "$io_report_m_begin - $io_report_p_sent + $io_report_p_recv - $io_report_m_done" | bc)
    
    local remote_enroll_time=$(echo "$remote_enroll_m_done - $remote_enroll_m_begin" | bc)
    local io_request_time=$(echo "$io_request_m_done - $io_request_m_begin" | bc)
    local io_report_time=$(echo "$io_report_m_done - $io_report_m_begin" | bc)

    local communication_time=$(echo "$remote_enroll_communication_time + $io_request_communication_time + $io_report_communication_time" | bc)
    local middleware_time=$(echo "$remote_enroll_time + $io_request_time + $io_report_time" | bc)
    local std_io_time=$(echo "$io_report_p_sent - $io_request_p_recv" | bc)

    echo "$std_io_time" "$middleware_time" "$communication_time"
}

get_timings_write() {
    local remote_enroll_p_sent=$(echo "$1" | grep "\[P2M\] P-> remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local remote_enroll_m_begin=$(echo "$2" | grep "\[P2M\] ->M remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local remote_enroll_m_done=$(echo "$2" | grep "\[P2M\] <-M remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local remote_enroll_p_recv=$(echo "$1" | grep "\[P2M\] P<- remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    
    local io_request_p_sent=$(echo "$1" | grep "\[P2M\] P-> io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_m_begin=$(echo "$2" | grep "\[P2M\] ->M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_m_done=$(echo "$2" | grep "\[P2M\] <-M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_p_recv=$(echo "$1" | grep "\[P2M\] P<- io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local io_report_p_sent=$(echo "$1" | grep "\[P2M\] P-> io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_m_begin=$(echo "$2" | grep "\[P2M\] ->M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_m_done=$(echo "$2" | grep "\[P2M\] <-M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_p_recv=$(echo "$1" | grep "\[P2M\] P<- io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local reserve_l_sent=$(echo "$2" | grep "\[M2M\] LM-> reserve" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local reserve_r_begin=$(echo "$2" | grep "\[M2M\] ->RM reserve" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local reserve_r_done=$(echo "$2" | grep "\[M2M\] <-RM reserve" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local reserve_l_recv=$(echo "$2" | grep "\[M2M\] LM<- reserve" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local sync_prov_l_sent=$(echo "$2" | grep "\[M2M\] LM-> sync_prov" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local sync_prov_r_begin=$(echo "$2" | grep "\[M2M\] ->RM sync_prov" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local sync_prov_r_done=$(echo "$2" | grep "\[M2M\] <-RM sync_prov" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local sync_prov_l_recv=$(echo "$2" | grep "\[M2M\] LM<- sync_prov" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    
    local remote_enroll_communication_time=$(echo "$remote_enroll_m_begin - $remote_enroll_p_sent + $remote_enroll_p_recv - $remote_enroll_m_done" | bc)
    local io_request_communication_time=$(echo "$io_request_m_begin - $io_request_p_sent + $io_request_p_recv - $io_request_m_done" | bc)
    local io_report_communication_time=$(echo "$io_report_m_begin - $io_report_p_sent + $io_report_p_recv - $io_report_m_done" | bc)
    local reserve_communication_time=$(echo "$reserve_r_begin - $reserve_l_sent + $reserve_l_recv - $reserve_r_done" | bc)
    local sync_prov_communication_time=$(echo "$sync_prov_r_begin - $sync_prov_l_sent + $sync_prov_l_recv - $sync_prov_r_done" | bc)

    local remote_enroll_time=$(echo "$remote_enroll_m_done - $remote_enroll_m_begin" | bc)
    local io_request_time=$(echo "$io_request_m_done - $io_request_m_begin" | bc)
    local io_report_time=$(echo "$io_report_m_done - $io_report_m_begin" | bc)

    local communication_p2m_time=$(echo "$remote_enroll_communication_time + $io_request_communication_time + $io_report_communication_time" | bc)
    local communication_m2m_time=$(echo "$reserve_communication_time + $sync_prov_communication_time" | bc)
    local middleware_time=$(echo "$remote_enroll_time + $io_request_time + $io_report_time" | bc)
    local std_io_time=$(echo "$io_report_p_sent - $io_request_p_recv" | bc)

    echo "$std_io_time" "$middleware_time" "$communication_p2m_time" "$communication_m2m_time"
}

NB_ITER=100

echo "std_io_time;middleware_time;communication_p2m_time;communication_m2m_time"

i=0
while [ $i -lt $NB_ITER ];do

    sudo truncate -s 0 $(docker inspect --format='{{.LogPath}}' stde2e)

    P_LOG=$(echo "RUST_LOG=info /tcp_server_e2e & RUST_LOG=info /tcp_client_e2e" | docker exec -i stde2e bash 2>/dev/null | sed -r "s/\x1B\[([0-9]{1,3}(;[0-9]{1,3})*)?[mGK]//g")
    M_LOG=$(docker logs stde2e | sed "s/^root@.*# //" | sed -r "s/\x1B\[([0-9]{1,3}(;[0-9]{1,3})*)?[mGK]//g")

    echo -n "$P_LOG" > p_log.txt
    echo -n "$M_LOG" > m_log.txt

    # P_LOG=$(cat p_log.txt)
    # M_LOG=$(cat m_log.txt)

    if [ $(echo "$P_LOG" | grep "local_enroll" | wc -l) -ne $(echo "$M_LOG" | grep "local_enroll" | wc -l)\
        -o $(echo "$P_LOG" | grep "io_request" | wc -l) -ne $(echo "$M_LOG" | grep "io_request" | wc -l)\
        -o $(echo "$P_LOG" | grep "io_report" | wc -l) -ne $(echo "$M_LOG" | grep "io_report" | wc -l)\
        -o $(echo "$P_LOG" | grep "\[P2M\]" | wc -l) -ne "12"\
        -o $(echo "$M_LOG" | grep "\[P2M\]" | wc -l) -ne "12"\
        -o $(echo "$M_LOG" | grep "\[M2M\]" | wc -l) -ne "8" ];
    then
        echo "Error: incomplete logs, retrying..." >&2
        continue
    fi

    FD_TO_READ=$(echo "$P_LOG" | grep "P-> remote_enroll" | grep "8888])$" | awk -F', ' '{print $2}')

    P_LOG_READ=$(echo "$P_LOG" | grep -v "\[DEMO\]" | grep "$FD_TO_READ")
    M_LOG_READ=$(echo "$M_LOG" | grep "$FD_TO_READ")
    P_LOG_WRITE=$(echo "$P_LOG" | grep -v "\[DEMO\]" | grep -v "$FD_TO_READ")
    M_LOG_WRITE=$(echo "$M_LOG"| grep -v "$FD_TO_READ")

    read READ_STD_IO_TIME READ_MIDDLEWARE_TIME READ_COMMUNICATION_P2M_TIME < <(get_timings_read "$P_LOG_READ" "$M_LOG_READ")
    read WRITE_STD_IO_TIME WRITE_MIDDLEWARE_TIME WRITE_COMMUNICATION_P2M_TIME COMMUNICATION_M2M_TIME < <(get_timings_write "$P_LOG_WRITE" "$M_LOG_WRITE")

    # echo "$READ_STD_IO_TIME;$READ_MIDDLEWARE_TIME;$READ_COMMUNICATION_P2M_TIME"
    # echo "$WRITE_STD_IO_TIME;$WRITE_MIDDLEWARE_TIME;$WRITE_COMMUNICATION_P2M_TIME;$COMMUNICATION_M2M_TIME"
    STD_IO_TIME=$(echo "$READ_STD_IO_TIME + $WRITE_STD_IO_TIME" | bc)
    MIDDLEWARE_TIME=$(echo "$READ_MIDDLEWARE_TIME + $WRITE_MIDDLEWARE_TIME" | bc)
    COMMUNICATION_P2M_TIME=$(echo "$READ_COMMUNICATION_P2M_TIME + $WRITE_COMMUNICATION_P2M_TIME" | bc)
    
    # A discrepancy in the measured communication times persists (P2M too high, M2M too low).
    echo "$STD_IO_TIME;$MIDDLEWARE_TIME;$COMMUNICATION_P2M_TIME;$COMMUNICATION_M2M_TIME"

    i=$((i+1))
done

