#!/bin/bash

get_timings() {
    local local_enroll_p_sent=$(echo "$1" | grep "\[P2M\] P-> local_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local local_enroll_m_begin=$(echo "$2" | grep "\[P2M\] ->M local_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local local_enroll_m_done=$(echo "$2" | grep "\[P2M\] <-M local_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local local_enroll_p_recv=$(echo "$1" | grep "\[P2M\] P<- local_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local io_request_p_sent=$(echo "$1" | grep "\[P2M\] P-> io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_m_begin=$(echo "$2" | grep "\[P2M\] ->M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_m_done=$(echo "$2" | grep "\[P2M\] <-M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_request_p_recv=$(echo "$1" | grep "\[P2M\] P<- io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local io_report_p_sent=$(echo "$1" | grep "\[P2M\] P-> io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_m_begin=$(echo "$2" | grep "\[P2M\] ->M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_m_done=$(echo "$2" | grep "\[P2M\] <-M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    local io_report_p_recv=$(echo "$1" | grep "\[P2M\] P<- io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    local local_enroll_communication_time=$(echo "$local_enroll_m_begin - $local_enroll_p_sent + $local_enroll_p_recv - $local_enroll_m_done" | bc)
    local io_request_communication_time=$(echo "$io_request_m_begin - $io_request_p_sent + $io_request_p_recv - $io_request_m_done" | bc)
    local io_report_communication_time=$(echo "$io_report_m_begin - $io_report_p_sent + $io_report_p_recv - $io_report_m_done" | bc)
    
    local local_enroll_time=$(echo "$local_enroll_m_done - $local_enroll_m_begin" | bc)
    local io_request_time=$(echo "$io_request_m_done - $io_request_m_begin" | bc)
    local io_report_time=$(echo "$io_report_m_done - $io_report_m_begin" | bc)

    local communication_time=$(echo "$local_enroll_communication_time + $io_request_communication_time + $io_report_communication_time" | bc)
    local middleware_time=$(echo "$local_enroll_time + $io_request_time + $io_report_time" | bc)
    local std_io_time=$(echo "$io_report_p_sent - $io_request_p_recv" | bc)

    echo "$std_io_time" "$middleware_time" "$communication_time"
}


NB_ITER=100

echo "std_io_time;middleware_time;communication_time"
# echo "read_std_io_time;read_middleware_time;read_communication_time;write_std_io_time;write_middleware_time;write_communication_time"

for i in $(seq 1 $NB_ITER);do

    sudo truncate -s 0 $(docker inspect --format='{{.LogPath}}' stde2e)

    P_LOG=$(echo "RUST_LOG=info /file_forwarder_e2e -i /dev/null -o /dev/zero" | docker exec -i stde2e bash | sed -r "s/\x1B\[([0-9]{1,3}(;[0-9]{1,3})*)?[mGK]//g")
    M_LOG=$(docker logs stde2e | sed "s/root@.*# //" | sed -r "s/\x1B\[([0-9]{1,3}(;[0-9]{1,3})*)?[mGK]//g" | sed "s/root@.*# //")

    # echo -n "$P_LOG" > p_log.txt
    # echo -n "$M_LOG" > m_log.txt

    # P_LOG=$(cat p_log.txt)
    # M_LOG=$(cat m_log.txt)

    # if same amount of local_enroll, io_request, io_report in P_LOG and M_LOG
    if [ $(echo "$P_LOG" | grep "local_enroll" | wc -l) -ne $(echo "$M_LOG" | grep "local_enroll" | wc -l)\
        -o $(echo "$P_LOG" | grep "io_request" | wc -l) -ne $(echo "$M_LOG" | grep "io_request" | wc -l)\
        -o $(echo "$P_LOG" | grep "io_report" | wc -l) -ne $(echo "$M_LOG" | grep "io_report" | wc -l)\
        -o $(echo "$P_LOG" | grep "\[P2M\]" | wc -l) -ne "12"\
        -o $(echo "$M_LOG" | grep "\[P2M\]" | wc -l) -ne "12" ];
    then
        echo "Error: incomplete logs"
        continue
    fi

    P_LOG_READ=$(echo "$P_LOG" | grep -v "\[DEMO\]" | head -n 6)
    M_LOG_READ=$(echo "$M_LOG" | head -n 6)
    P_LOG_WRITE=$(echo "$P_LOG" | grep -v "\[DEMO\]" | tail -n 6)
    M_LOG_WRITE=$(echo "$M_LOG"| tail -n 6)

    read READ_STD_IO_TIME READ_MIDDLEWARE_TIME READ_COMMUNICATION_TIME < <(get_timings "$P_LOG_READ" "$M_LOG_READ")
    read WRITE_STD_IO_TIME WRITE_MIDDLEWARE_TIME WRITE_COMMUNICATION_TIME < <(get_timings "$P_LOG_WRITE" "$M_LOG_WRITE")

    STD_IO_TIME=$(echo "$READ_STD_IO_TIME + $WRITE_STD_IO_TIME" | bc)
    MIDDLEWARE_TIME=$(echo "$READ_MIDDLEWARE_TIME + $WRITE_MIDDLEWARE_TIME" | bc)
    COMMUNICATION_TIME=$(echo "$READ_COMMUNICATION_TIME + $WRITE_COMMUNICATION_TIME" | bc)

    echo "$STD_IO_TIME;$MIDDLEWARE_TIME;$COMMUNICATION_TIME"

    # echo "$READ_STD_IO_TIME;$READ_MIDDLEWARE_TIME;$READ_COMMUNICATION_TIME;$WRITE_STD_IO_TIME;$WRITE_MIDDLEWARE_TIME;$WRITE_COMMUNICATION_TIME"

done

