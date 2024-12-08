#! /bin/bash

# parse options:
# uvrun.sh <input> [-d <debug_dir> ] [-r <run_mode>] [-t] [-f]
debug_dir="temp"
run_mode="optimize"
trace_more="0"
file="0"

while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--debug)
            debug_dir="$2"
            shift 2
            ;;
        -r|--run-mode)
            run_mode="$2" 
            shift 2
            ;;
        -t|--trace)
            trace_more="1"
            shift
            ;;
        -f|--file)
            file="1"
            shift
            ;;
        *)
            input="$1"
            shift
            ;;
    esac
done

if [ -z "$input" ]; then
    echo "Error: Input file required"
    echo "Usage: uvrun.sh <input> [-d <debug_dir>] [-r <run_mode>] [-t]"
    exit 1
fi

basename=$(basename $input)
basename_wo_ext="${basename%.*}"

stderr_redirect=""
if [ $file -eq 1 ]; then
    stderr_redirect="2> $debug_dir/$basename_wo_ext.stderr.ansi"
fi

echo "mkdir -p $debug_dir"
mkdir -p $debug_dir

echo "UV_TRACE=$trace_more RUST_LOG=warn cargo run -- $input --run-mode egglog > $debug_dir/$basename_wo_ext.egg $stderr_redirect"
UV_TRACE=$trace_more RUST_LOG=warn cargo run -- $input --run-mode egglog > $debug_dir/$basename_wo_ext.egg $stderr_redirect

echo "UV_TRACE=$trace_more RUST_LOG=warn cargo run -- $input --debug-dir $debug_dir --run-mode $run_mode > $debug_dir/$basename_wo_ext.out $stderr_redirect"
UV_TRACE=$trace_more RUST_LOG=warn cargo run -- $input --debug-dir $debug_dir --run-mode $run_mode > $debug_dir/$basename_wo_ext.out $stderr_redirect
