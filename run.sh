#!/bin/bash

if [ -n "$1" ]; then
  export BLOCK_CHAT_NETWORK_SIZE="$1"
fi

export BLOCK_CHAT_BOOTSTRAP_PEER_SOCKET="192.168.0.3:27736"
export BLOCK_CHAT_DAEMON_SOCKET="127.0.0.1:27737"

cargo build --release

# start 2 processes, daemon and helper without waiting for any of then to finish
./target/release/daemon &
./target/release/helper
