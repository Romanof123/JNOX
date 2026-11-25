# Native synchronous IPC sample

From the repository root, build the C client and Rust server, then package them with one directed call right.

~~~sh
sdk/native-aarch64/build.sh examples/native-sync-ipc/client.c client.elf
sdk/native-aarch64/build-rust.sh examples/native-sync-ipc/server/Cargo.toml server.elf
python3 tools/build_native_el0.py --out ipc-pair --policy fifo --priority 7 --peer-priority 1 --elf client.elf --peer-elf server.elf --ipc-route 0:1
~~~

Slot 0 sends four unsigned 16-bit sensor readings (300, 512, 257, 1025), least-significant field first. Slot 1 computes the count, maximum, minimum, and sum (4, 1025, 257, 2094). The expected reply is 0x000404010101082e. Client exit 41 verifies the whole reply. Server exit 42 requires the three reply attempts to return -116 (unknown token), 0 (valid reply), and -116 (reused token).

The client exports ipc_request_word, ipc_call_status, and ipc_reply_word in its private ELF. The server exports ipc_server_words (token, request, response) and ipc_reply_statuses (three signed syscall results). Read the terminal values through each process's own mapping. The invalid-token attempt uses the bitwise complement of the delivered token in this single-outstanding-call fixture. A foreign live token requires another process and is a separate control.
