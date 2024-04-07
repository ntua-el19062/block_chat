#!/bin/bash

# parameters
DS="127.0.0.1:27737"
FS="10"
IF="inputs/10nodes"

DS_2="127.0.0.1:27739"
FS_2="10"
IF_2="inputs/10nodes"

max_id=4

start_helper="cd ~/block_chat &&\
 export DAEMON_SOCKET=$DS &&\
 export FIXED_STAKING=$FS &&\
 export INPUT_FOLDER=$IF &&\
 ./target/release/helper"

start_helper_2="cd ~/block_chat &&\
 export DAEMON_SOCKET=$DS_2 &&\
 export FIXED_STAKING=$FS_2 &&\
 export INPUT_FOLDER=$IF_2 &&\
 ./target/release/helper"

for i in $(seq 0 $max_id); do
    echo "node$i: stopping (possibly running) service(s)"
    ssh node$i "sudo systemctl stop block_chat" &
    ssh node$i "sudo systemctl stop block_chat_2" &
done

wait
echo ""

for i in $(seq 0 $max_id); do
    echo "node$i: performing daemon reload"
    ssh node$i "sudo systemctl daemon-reload" &
done

wait
echo ""

for i in $(seq 0 $max_id); do
    echo "node$i: starting helper(s)"
    ssh node$i "$start_helper" &
    ssh node$i "$start_helper_2" &
done

# don't wait
echo ""

for i in $(seq 0 $max_id); do
    echo "node$i: starting daemon(s)"
    ssh node$i "sudo systemctl start block_chat" &
    ssh node$i "sudo systemctl start block_chat_2" &
done

wait
