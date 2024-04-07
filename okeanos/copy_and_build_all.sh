#!/bin/bash

max_id=4

for i in $(seq 0 $max_id); do
    echo "node$i: removing old files"
    ssh node$i "rm -rf ~/block_chat && mkdir ~/block_chat" &
done

wait
echo ""

for i in $(seq 0 $max_id); do
    echo "node$i: copying source files"
    scp -r ../src ../Cargo.* node$i:~/block_chat &
done

wait
echo ""

for i in $(seq 0 $max_id); do
    echo "node$i: building binaries"
    ssh node$i "source ~/.cargo/env && cd ~/block_chat && cargo build --release" &
done

wait
echo ""

for i in $(seq 0 $max_id); do
    echo "node$i: copying input files"
    scp -r ../inputs node$i:~/block_chat &
done

wait

if [ "$1" != "--partial" ]; then
    echo ""

    for i in $(seq 0 $max_id); do
        echo "node$i: copying service file and moving it to /etc/systemd/system/"
        scp ./block_chat.service node$i:~/block_chat
        ssh node$i "sudo cp ~/block_chat/block_chat.service /etc/systemd/system" &
    done

    wait
    echo ""

    for i in $(seq 0 $max_id); do
        echo "node$i: copying override file and moving it to /etc/systemd/system/block_chat.service.d/"
        scp ./override.conf node$i:~/block_chat
        ssh node$i "sudo mkdir -p /etc/systemd/system/block_chat.service.d && sudo cp ~/block_chat/override.conf /etc/systemd/system/block_chat.service.d" &
    done
fi

wait