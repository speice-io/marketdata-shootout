#!/bin/bash
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

function run_shootout() {
    RUN_PREFIX="$1"
    OUTPUT_NAME="$2"

    for f in data_feeds*.pcap; do
        echo "$f" >> "$OUTPUT_NAME"
        echo $RUN_PREFIX ./target/release/md_shootout -f "$f"
        sudo $RUN_PREFIX ./target/release/md_shootout -f "$f" >> "$OUTPUT_NAME"
    done
}

PREFIXES=(
    "|shootout_normal.txt" # No CPU Pinning
    "taskset 2|shootout_taskset.txt" # Pin to CPU 1
    "taskset 2 nice -n-19|shootout_nice.txt" # Pin to CPU 2 with highest priority
    # Kinda dangerous, caused the processor to lock when running in graphical session,
    # but seemed OK in runlevel 3 until the kernel switched time sources
    #"taskset 2 chrt -f 99|shootout_chrt.txt" # Pin to CPU 3 with real-time priority
)
RUN_COUNT=10

for prefix in "${PREFIXES[@]}"; do
    (
        RUN_PREFIX="$(echo "$prefix" | cut -d'|' -f1)"
        OUTPUT_NAME="$(echo "$prefix" | cut -d'|' -f2)"

        rm "$OUTPUT_NAME"
        for i in $(seq 1 $RUN_COUNT); do
            run_shootout "$RUN_PREFIX" "$OUTPUT_NAME"
        done
    )
    wait
done
