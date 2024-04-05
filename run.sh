#!/bin/bash

# ./run.sh <daemon_socket> <fixed_staking> <input_folder> <bootstrap_peer_socket> [<network_size>]

if [ "$#" -lt 4 ] then
  echo "Not enough arguments supplied"
  exit 1
fi

export DAEMON_SOCKET="$1"
export FIXED_STAKING="$2"
export INPUT_FOLDER="$3"

export BLOCK_CHAT_BOOTSTRAP_PEER_SOCKET="$4"

if [ -n "$5" ] then
    export BLOCK_CHAT_NETWORK_SIZE="$5"
fi

cargo build --release

./target/release/daemon &
./target/release/helper
