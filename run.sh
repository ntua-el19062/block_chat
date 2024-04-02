#!/bin/bash

if [ -z "$1" ] then
  echo "Usage: <bootstrap_peer_socket> [<network_size>]"
  exit 1
fi

export BLOCK_CHAT_BOOTSTRAP_PEER_SOCKET="$1"

if [ -n "$2" ]; then
  export BLOCK_CHAT_NETWORK_SIZE="$2"
fi

export BLOCK_CHAT_DAEMON_SOCKET="127.0.0.1:27737"

cargo build --release

# start 2 processes, daemon and helper without waiting for any of then to finish
./target/release/daemon &
./target/release/helper
