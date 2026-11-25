# Named FIFO and anonymous pipe workloads

Run from the repository root on the integrated native pipe SDK/runtime. The named
case has two ordinary writers and one reader in separate private address spaces.
The anonymous case uses a prewired forward stream and a reverse acknowledgment.

~~~sh
dir=examples/native-pipes
sh sdk/native-aarch64/build.sh --pipes "$dir/writer.c" writer.elf
sh sdk/native-aarch64/build-rust.sh --pipes "$dir/writer-rust/Cargo.toml" rust-writer.elf
sh sdk/native-aarch64/build.sh --pipes "$dir/reader.c" reader.elf
sh sdk/native-aarch64/build.sh --pipes "$dir/anon-writer.c" anon-writer.elf
sh sdk/native-aarch64/build.sh --pipes "$dir/anon-reader.c" anon-reader.elf
package() {
    policy=$1
    shift
    python3 tools/build_native_el0.py --pipes --policy "$policy" "$@" --out "pipe-named-$policy" \
        --elf writer.elf --priority 3 --peer-elf rust-writer.elf --peer-priority 3 \
        --peer-elf reader.elf --peer-priority 3 --fifo-grant 0:4:3 \
        --pipe-link 0:3:1:3 --pipe-link 0:4:2:3 --pipe-link 1:4:0:5
    python3 tools/build_native_el0.py --pipes --policy "$policy" "$@" --out "pipe-anon-$policy" \
        --elf anon-writer.elf --priority 3 --peer-elf anon-reader.elf --peer-priority 3 \
        --pipe-link 0:1:1:0 --pipe-link 1:4:0:4
}
package fifo
package rr --quantum-ticks 625000
~~~

Boot each resulting kernel with the normal single-core AArch64 virt runner. Use a fresh package directory for each first position.

**Named case.** Slot 0 creates /samples and authorizes slot 2 to read and slots
0 and 1 to write. A gate tells the reader when to open. Its second gate stays
closed until the C writer has put four 512-byte records into the empty stream
and retained the result of a fifth atomic nonblocking write. The expected
result is −1/EAGAIN with no bytes committed. C restores blocking mode, admits
both peers and waits for Rust to acknowledge its open writer before continuing.
C unlinks the name after all of its successful writes. The reader drains its
existing description to EOF after both writers close.

Expected exits in slot order are 61, 62 and 63. Each writer commits eight
records; the private reader symbol pipe_received contains 8192 bytes in actual
arrival order. Byte 0 is origin (C=0, Rust=1); byte 1 is per-origin ordinal
0..7. For i=2..507 byte i equals (97*origin + 31*ordinal + 17*i + (i>>3)) mod
256. Bytes 508..511 hold little-endian IEEE CRC-32 over bytes 0..507 (Python
zlib.crc32). Check every byte and per-origin order without assuming any global
writer alternation. In each writer, c_writer_crc or rust_writer_crc and
c_write_count or rust_write_count retain the independent products and counts.
In slot 0 check c_full_rc and c_full_errno. In slot 2 pipe_read_sizes[0:pipe_reads]
records the positive chunks (at most 137 bytes) and final zero. Check
pipe_records=16 and pipe_eof=1. Compare actual application data, call outcomes,
the kernel's peak occupancy and final references independently.

**Anonymous case.** The writer duplicates its prewired stdout and closes the
original after writing "rain". The reader duplicates stdin, closes the original,
reads those four bytes and acknowledges them through the reverse pipe. Before
writing "fall" and closing its last forward description, the writer closes the
reverse read end. The reader consequently observes all eight bytes before EOF
and then EPIPE on a positive-length reverse write. Expected exits are 64/65.
Read anon_writer_steps=2 from slot 0 and anon_data="rainfall", anon_eof=0,
anon_epipe_rc=-1, anon_epipe_errno=EPIPE and anon_done=1 from slot 1.
This package exercises boot descriptor wiring. Shell-driven process launch and
file reads have their own acceptance case.
