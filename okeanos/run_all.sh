#!/bin/bash

# parameters
DS="127.0.0.1:27737"
FS="10"
IF="inputs/5nodes"

max_id=4

start_helper="cd ~/block_chat &&\
 export DAEMON_SOCKET=$DS &&\
 export FIXED_STAKING=$FS &&\
 export INPUT_FOLDER=$IF &&\
 ./target/release/helper"

for i in $(seq 0 $max_id); do
    echo "node$i: stopping (possibly running) service"
    ssh node$i "sudo systemctl stop block_chat" &
done

wait
echo ""

for i in $(seq 0 $max_id); do
    echo "node$i: reloading daemon"
    ssh node$i "sudo systemctl daemon-reload" &
done

wait
echo ""

for i in $(seq 0 $max_id); do
    echo "node$i: starting helper"
    ssh node$i "$start_helper" &
done

# don't wait
echo ""

for i in $(seq 0 $max_id); do
    echo "node$i: starting daemon"
    ssh node$i "sudo systemctl start block_chat"
done
