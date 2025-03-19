#!/bin/bash

get_timings_write() {


    echo "$enroll_req_time" "$enroll_processing_time" "$enroll_ack_time" "$io_request_req_time" "$io_request_processing_time" "$io_request_ack_time" "$io_time" "$io_report_req_time" "$io_report_processing_time" "$io_report_ack_time" "$reserve_req_time" "$reserve_processing_time" "$reserve_ack_time" "$sync_prov_req_time" "$sync_prov_processing_time" "$sync_prov_ack_time"
}

NB_ITER=100

echo "write_enroll_req_time;write_enroll_processing_time;write_enroll_ack_time;write_io_request_req_time;write_io_request_processing_time;write_io_request_ack_time;write_io_time;write_io_report_req_time;write_io_report_processing_time;write_io_report_ack_time;write_reserve_req_time;write_reserve_processing_time;write_reserve_ack_time;write_sync_prov_req_time;write_sync_prov_processing_time;write_sync_prov_ack_time;read_enroll_req_time;read_enroll_processing_time;read_enroll_ack_time;read_io_request_req_time;read_io_request_processing_time;read_io_request_ack_time;read_io_time;read_io_report_req_time;read_io_report_processing_time;read_io_report_ack_time"

i=0
while [ $i -lt $NB_ITER ];do

    sudo truncate -s 0 $(docker inspect --format='{{.LogPath}}' stde2e) $(docker inspect --format='{{.LogPath}}' stde2e_server)

    P_LOG=$(echo "RUST_LOG=info /tcp_server_e2e" | docker exec -i stde2e_server bash 2>/dev/null | ansi2txt & echo "RUST_LOG=info /tcp_client_e2e stde2e_server:8888" | docker exec -i stde2e bash  2>/dev/null | ansi2txt)
    M_LOG=$(docker compose logs | ansi2txt | sed "s/root.*20/20/;s/.*| //")

    echo -n "$P_LOG" > p_log.txt
    echo -n "$M_LOG" > m_log.txt

    # P_LOG=$(cat p_log.txt)
    # M_LOG=$(cat m_log.txt)

    if [ $(echo "$P_LOG" | grep "remote_enroll" | wc -l) -ne $(echo "$M_LOG" | grep "remote_enroll" | wc -l)\
        -o $(echo "$P_LOG" | grep "io_request" | wc -l) -ne $(echo "$M_LOG" | grep "io_request" | wc -l)\
        -o $(echo "$P_LOG" | grep "io_report" | wc -l) -ne $(echo "$M_LOG" | grep "io_report" | wc -l)\
        -o $(echo "$P_LOG" | grep "\[P2M\]" | wc -l) -ne "12"\
        -o $(echo "$M_LOG" | grep "\[P2M\]" | wc -l) -ne "12"\
        -o $(echo "$M_LOG" | grep "\[M2M\]" | wc -l) -ne "8"\
        -o $(echo "$M_LOG" | grep "root@.*#" | wc -l) -ne "0" ];
    then
        echo "Error: incorrect logs, retrying..." >&2
        continue
    fi

    FD_TO_READ=$(echo "$P_LOG" | grep "P-> remote_enroll" | grep "8888])$" | awk -F', ' '{print $2}')

    P_LOG_READ=$(echo "$P_LOG" | grep -v "\[DEMO\]" | grep "$FD_TO_READ")
    M_LOG_READ=$(echo "$M_LOG" | grep "$FD_TO_READ")
    P_LOG_WRITE=$(echo "$P_LOG" | grep -v "\[DEMO\]" | grep -v "$FD_TO_READ")
    M_LOG_WRITE=$(echo "$M_LOG"| grep -v "$FD_TO_READ")


    read_enroll_p_sent=$(echo "$P_LOG_READ" | grep "\[P2M\] P-> remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    read_enroll_m_begin=$(echo "$M_LOG_READ" | grep "\[P2M\] ->M remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    read_enroll_m_done=$(echo "$M_LOG_READ" | grep "\[P2M\] <-M remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    read_enroll_p_recv=$(echo "$P_LOG_READ" | grep "\[P2M\] P<- remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    read_io_request_p_sent=$(echo "$P_LOG_READ" | grep "\[P2M\] P-> io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    read_io_request_m_begin=$(echo "$M_LOG_READ" | grep "\[P2M\] ->M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    read_io_request_m_done=$(echo "$M_LOG_READ" | grep "\[P2M\] <-M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    read_io_request_p_recv=$(echo "$P_LOG_READ" | grep "\[P2M\] P<- io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    read_io_report_p_sent=$(echo "$P_LOG_READ" | grep "\[P2M\] P-> io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    read_io_report_m_begin=$(echo "$M_LOG_READ" | grep "\[P2M\] ->M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    read_io_report_m_done=$(echo "$M_LOG_READ" | grep "\[P2M\] <-M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    read_io_report_p_recv=$(echo "$P_LOG_READ" | grep "\[P2M\] P<- io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)


    read_enroll_req_time=$(echo "$read_enroll_m_begin - $read_enroll_p_sent" | bc)
    read_enroll_processing_time=$(echo "$read_enroll_m_done - $read_enroll_m_begin" | bc)
    read_enroll_ack_time=$(echo "$read_enroll_p_recv - $read_enroll_m_done" | bc)

    read_io_request_req_time=$(echo "$read_io_request_m_begin - $read_io_request_p_sent" | bc)
    read_io_request_processing_time=$(echo "$read_io_request_m_done - $read_io_request_m_begin" | bc)
    read_io_request_ack_time=$(echo "$read_io_request_p_recv - $read_io_request_m_done" | bc)

    read_io_time=$(echo "$read_io_report_p_sent - $read_io_request_p_recv" | bc)

    read_io_report_req_time=$(echo "$read_io_report_m_begin - $read_io_report_p_sent" | bc)
    read_io_report_processing_time=$(echo "$read_io_report_m_done - $read_io_report_m_begin" | bc)
    read_io_report_ack_time=$(echo "$read_io_report_p_recv - $read_io_report_m_done" | bc)


    write_enroll_p_sent=$(echo "$P_LOG_WRITE" | grep "\[P2M\] P-> remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_enroll_m_begin=$(echo "$M_LOG_WRITE" | grep "\[P2M\] ->M remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_enroll_m_done=$(echo "$M_LOG_WRITE" | grep "\[P2M\] <-M remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_enroll_p_recv=$(echo "$P_LOG_WRITE" | grep "\[P2M\] P<- remote_enroll" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    
    write_io_request_p_sent=$(echo "$P_LOG_WRITE" | grep "\[P2M\] P-> io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_io_request_m_begin=$(echo "$M_LOG_WRITE" | grep "\[P2M\] ->M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_io_request_m_done=$(echo "$M_LOG_WRITE" | grep "\[P2M\] <-M io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_io_request_p_recv=$(echo "$P_LOG_WRITE" | grep "\[P2M\] P<- io_request" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    write_io_report_p_sent=$(echo "$P_LOG_WRITE" | grep "\[P2M\] P-> io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_io_report_m_begin=$(echo "$M_LOG_WRITE" | grep "\[P2M\] ->M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_io_report_m_done=$(echo "$M_LOG_WRITE" | grep "\[P2M\] <-M io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_io_report_p_recv=$(echo "$P_LOG_WRITE" | grep "\[P2M\] P<- io_report" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    write_reserve_l_sent=$(echo "$M_LOG_WRITE" | grep "\[M2M\] LM-> reserve" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_reserve_r_begin=$(echo "$M_LOG_WRITE" | grep "\[M2M\] ->RM reserve" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_reserve_r_done=$(echo "$M_LOG_WRITE" | grep "\[M2M\] <-RM reserve" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_reserve_l_recv=$(echo "$M_LOG_WRITE" | grep "\[M2M\] LM<- reserve" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)

    write_sync_prov_l_sent=$(echo "$M_LOG_WRITE" | grep "\[M2M\] LM-> sync_prov" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_sync_prov_r_begin=$(echo "$M_LOG_WRITE" | grep "\[M2M\] ->RM sync_prov" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_sync_prov_r_done=$(echo "$M_LOG_WRITE" | grep "\[M2M\] <-RM sync_prov" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    write_sync_prov_l_recv=$(echo "$M_LOG_WRITE" | grep "\[M2M\] LM<- sync_prov" | awk '{print $1}' | xargs -I {} date -d "{}" +%s%N | cut -c 1-16)
    
    write_enroll_req_time=$(echo "$write_enroll_m_begin - $write_enroll_p_sent" | bc)
    write_enroll_processing_time=$(echo "$write_enroll_m_done - $write_enroll_m_begin" | bc)
    write_enroll_ack_time=$(echo "$write_enroll_p_recv - $write_enroll_m_done" | bc)

    write_io_request_req_time=$(echo "$write_io_request_m_begin - $write_io_request_p_sent" | bc)
    write_io_request_processing_time=$(echo "$write_io_request_m_done - $write_io_request_m_begin" | bc) # includes reserve time
    write_io_request_ack_time=$(echo "$write_io_request_p_recv - $write_io_request_m_done" | bc)

    write_io_time=$(echo "$write_io_report_p_sent - $write_io_request_p_recv" | bc)

    write_io_report_req_time=$(echo "$write_io_report_m_begin - $write_io_report_p_sent" | bc)
    write_io_report_processing_time=$(echo "$write_io_report_m_done - $write_io_report_m_begin" | bc) # includes sync_prov time
    write_io_report_ack_time=$(echo "$write_io_report_p_recv - $write_io_report_m_done" | bc)

    write_reserve_req_time=$(echo "$write_reserve_r_begin - $write_reserve_l_sent" | bc)
    write_reserve_processing_time=$(echo "$write_reserve_r_done - $write_reserve_r_begin" | bc)
    write_reserve_ack_time=$(echo "$write_reserve_l_recv - $write_reserve_r_done" | bc)

    write_sync_prov_req_time=$(echo "$write_sync_prov_r_begin - $write_sync_prov_l_sent" | bc)
    write_sync_prov_processing_time=$(echo "$write_sync_prov_r_done - $write_sync_prov_r_begin" | bc)
    write_sync_prov_ack_time=$(echo "$write_sync_prov_l_recv - $write_sync_prov_r_done" | bc)

    echo "$write_enroll_req_time;$write_enroll_processing_time;$write_enroll_ack_time;$write_io_request_req_time;$write_io_request_processing_time;$write_io_request_ack_time;$write_io_time;$write_io_report_req_time;$write_io_report_processing_time;$write_io_report_ack_time;$write_reserve_req_time;$write_reserve_processing_time;$write_reserve_ack_time;$write_sync_prov_req_time;$write_sync_prov_processing_time;$write_sync_prov_ack_time;$read_enroll_req_time;$read_enroll_processing_time;$read_enroll_ack_time;$read_io_request_req_time;$read_io_request_processing_time;$read_io_request_ack_time;$read_io_time;$read_io_report_req_time;$read_io_report_processing_time;$read_io_report_ack_time"
    i=$((i+1))
done

