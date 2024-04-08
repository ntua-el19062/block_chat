#!/bin/bash

max_node_id=4

# delete old files
for i in $(seq 0 $max_node_id); do
    echo "node$i: removing old files"
    ssh node$i "rm -rf ~/block_chat && mkdir ~/block_chat" &
done

wait
echo ""

# copy over new files
for i in $(seq 0 $max_node_id); do
    echo "node$i: copying source files"
    scp -r ../src ../Cargo.* node$i:~/block_chat &
done

wait
echo ""

# build the necessary binaries
for i in $(seq 0 $max_node_id); do
    echo "node$i: building binaries"
    ssh node$i "source ~/.cargo/env && cd ~/block_chat && cargo build --release" &
done

wait
echo ""

# copy over the input files
for i in $(seq 0 $max_node_id); do
    echo "node$i: copying input files"
    scp -r ../inputs node$i:~/block_chat &
done

wait

# the following steps take a long time to complete
# and rarely change, so we can skip them if we want
if [ "$1" != "--partial" ]; then
    echo ""

    # copy over the files describing the block_chat service (and block_chat_2 for 10 nodes)
    for i in $(seq 0 $max_node_id); do
        echo "node$i: copying service files and moving them to /etc/systemd/system/"
        scp ./block_chat*.service node$i:~/block_chat
        ssh node$i "sudo cp ~/block_chat/block_chat.service /etc/systemd/system" &
        ssh node$i "sudo cp ~/block_chat/block_chat_2.service /etc/systemd/system" &
    done

    wait
    echo ""

    # copy over the files describing the block_chat service overrides to set the necessary environment variables
    for i in $(seq 0 $max_node_id); do
        echo "node$i: copying override files and moving them to /etc/systemd/system/block_chat_*.service.d/"
        scp ./override*.conf node$i:~/block_chat
        ssh node$i "sudo mkdir -p /etc/systemd/system/block_chat.service.d && sudo cp ~/block_chat/override.conf /etc/systemd/system/block_chat.service.d" &
        ssh node$i "sudo mkdir -p /etc/systemd/system/block_chat_2.service.d && sudo cp ~/block_chat/override_2.conf /etc/systemd/system/block_chat_2.service.d/override.conf" &
    done

    wait
    echo ""

    # reload the systemd daemon
    for i in $(seq 0 $max_node_id); do
        echo "node$i: performing daemon reload"
        ssh node$i "sudo systemctl daemon-reload" &
    done
fi

wait
