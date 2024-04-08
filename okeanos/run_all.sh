#!/bin/bash

# parameters
bc1_daemon_socket="127.0.0.1:27737"
bc2_daemon_socket="127.0.0.1:27739" # 10 nodes only
fixed_staking="10"
greater_staking="100"
input_folder="inputs/5nodes"

# change input folder for 10 nodes
if [ "$1" == "--10" ]; then
    input_folder="inputs/10nodes"
fi

max_node_id=4

start_helper="cd ~/block_chat &&\
 export DAEMON_SOCKET=$bc1_daemon_socket &&\
 export FIXED_STAKING=$fixed_staking &&\
 export INPUT_FOLDER=$input_folder &&\
 ./target/release/helper"

start_helper_2="cd ~/block_chat &&\
 export DAEMON_SOCKET=$bc2_daemon_socket &&\
 export FIXED_STAKING=$fixed_staking &&\
 export INPUT_FOLDER=$input_folder &&\
 ./target/release/helper"

start_helper_greater_staking="cd ~/block_chat &&\
 export DAEMON_SOCKET=$bc1_daemon_socket &&\
 export FIXED_STAKING=$greater_staking &&\
 export INPUT_FOLDER=$input_folder &&\
 ./target/release/helper"

for i in $(seq 0 $max_node_id); do
    echo "node$i: stopping (possibly running) service(s)"
    ssh node$i "sudo systemctl stop block_chat" &
    ssh node$i "sudo systemctl stop block_chat_2" &
done

wait
echo ""

# start the helpers first
# they do nothing until the daemons reply to their requests
for i in $(seq 0 $max_node_id); do
    echo "node$i: starting helper(s)"

    if [ "$1" == "--unfair" ] && [ $i -eq 0 ]; then
        ssh node$i "$start_helper_greater_staking" &
    else
        ssh node$i "$start_helper" &
    fi

    if [ "$1" == "--10" ]; then
        ssh node$i "$start_helper_2" &
    fi
done

# don't wait
echo ""

# start the daemons
for i in $(seq 0 $max_node_id); do
    echo "node$i: starting daemon(s)"
    ssh node$i "sudo systemctl start block_chat" &

    # start the second daemon on each node when --10 is passed
    if [ "$1" == "--10" ]; then
        ssh node$i "sudo systemctl start block_chat_2" &
    fi
done

wait
