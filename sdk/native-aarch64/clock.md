# Native monotonic clock and weighted monitor

CHG-088 extends the native v1 integer-call table for a single awake CPU in one boot-scoped monotonic clock domain. See the [system time design](../../docs/system-time-plan.en.md) and [native ABI](../../docs/native-c-abi.en.md).

| Native SVC #0x4a01 | Arguments | C contract |
|---|---|---|
| x8=6, clock_gettime | x0=clockid, x1=result VA | Success returns 0. Errors return -1 and set errno. |
| x8=7, clock_nanosleep | x0=clockid, x1=flags, x2=request VA, x3=remaining VA | Success returns 0. Failures return a positive error number directly and preserve errno. For absolute waits the wrapper sets x3=0 and leaves remaining untouched. |

Clock ID 1 is CLOCK_MONOTONIC. The sleep profile accepts TIMER_ABSTIME=1 only. Unimplemented choices must return an error. A timespec consists of signed LP64 seconds and signed LP64 nanoseconds. Reads round elapsed time down; absolute arms round up to a representable tick. Equal consecutive reads are valid. Validate 0 <= tv_nsec < 10^9 and reject overflow before altering a deadline. Kernel copyout for call 6 requires a writable user span of 16 bytes; copyin for call 7 requires a readable user span of 16 bytes. Copy the request once before blocking. An expired valid target completes immediately. Signal handling, relative sleeps, realtime, RTC, suspend and physical timing qualification have separate requirements.

Build native_weighted_monitor.c with build.sh and load the same image in two private address spaces. Process arguments are A or B and share or wake. The launcher maps a read-only completion word at 0x01200000 in both processes. It starts at 0 and becomes 1 after 36 charged timer closures in the share case or the third wake in the wake case. A and B have configured weights 1 and 2.

Each process scans the following ring in 256-reading chunks. A starts at offset 0 and B at 5. For sample i within a chunk let phase=i mod 16. Add ring[(phase+offset) mod 16] to sum and (phase+1) times the same value to weighted_sum. The busy path reads the completion word at chunk boundaries, performs no time syscalls, and fails upon exhausting its 2^28-chunk bound. The workload measures both sample throughput and useful arithmetic; compare actual CPU service independently.

    3 1 4 1 5 9 2 6 5 3 5 8 9 7 9 3

| Role | Samples/chunk | Sum/chunk | Weighted sum/chunk | Exit |
|---|---:|---:|---:|---:|
| A | 256 | 1280 | 12576 | 41 |
| B | 256 | 1280 | 9760 | 42 |

In wake mode B computes one chunk, reads monotonic time, requests an absolute sleep for that time plus 40 ms, then reads the clock in its first post-sleep call. It verifies the timestamp and performs a full subsequent chunk before publishing the round. After three rounds it requires the kernel completion flag. Its final totals must be 6 chunks, 1536 samples, sum 7680 and weighted sum 58560. Each absolute deadline derives from that round's actual pre-sleep read. The resumed time includes return and clock-call overhead. A continues working without yielding.

The private ELF symbol native_weighted_result contains 28 little-endian 64-bit words. Words 0–8 are version=1, role (A=1,B=2), arm (share=1,wake=2), sequence, chunks, samples, sum, weighted_sum and completed wake rounds. Each of the next three six-word records contains before, target and resumed times as signed seconds/nanoseconds. Word 27 is the main return status published last. A stable final sequence equals twice the completed chunks. A captured odd sequence denotes a partial chunk publication. Process-exit observation is required to claim a sealed final product. On error 90 means invalid arguments, 91 work cap, 92 initial clock read, 93 target overflow, 94 sleep, 95 resumed clock read, 96 early return, 97 invalid completion state.
