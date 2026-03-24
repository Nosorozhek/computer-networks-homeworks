NUM_THREADS=2
DELAY=3
PORT=3000

cargo run -p "lab03" --bin server $PORT $NUM_THREADS 2>1 &> \dev\null &
SERVER_PID=$!

sleep 0.5

run_client(){
    local ID="$1"
    START=$SECONDS
    cargo run -p "lab03" --bin client -- 127.0.0.1 $PORT lab03/assets/sample.txt 2>1 &> \dev\null
    END=$SECONDS
    echo client $ID waited for $(($END - $START)) seconds
}

ITERATIONS=$(($NUM_THREADS * 2 + 1))
for ((i=0; i<ITERATIONS; i++)); do
    run_client $i &
done
sleep $(($DELAY * ($ITERATIONS / $NUM_THREADS + 1) + 1))

kill $SERVER_PID
