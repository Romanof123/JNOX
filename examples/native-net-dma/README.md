# Native Ethernet ownership fixture

Build from the repository root using the freestanding C SDK.

~~~sh
app=examples/native-net-dma
sh sdk/native-aarch64/build.sh "$app/primary.c" dma-primary.elf
sh sdk/native-aarch64/build.sh "$app/peer.c" dma-peer.elf
sh sdk/native-aarch64/build.sh "$app/spare.c" dma-spare.elf
~~~

Package the ELFs in that order. Assign initial network ownership to slot 0 and standby authority to slot 2. Configure synchronous IPC route 0:2 and notification route 2:1. Each sleeping thread uses the absolute monotonic clock. Slot 2 first checks that claiming the active primary's NIC returns -11, then waits for a retirement message. Slot 0 sends that message after its first exchange and returns after receiving the reply. Slot 2 sleeps for 15 guest seconds after replying, then claims its epoch.

Both owners exchange 60-byte Ethernet frames. The host uses source MAC 02:00:00:00:00:01 and broadcast destination. Bytes 12–13 are 88 b5. Bytes 14–17 contain ASCII JNDM. Byte 18 is epoch 1 or 2. Byte 19 is opcode 1 for challenge, 2 for reply or 3 for retirement. The host supplies a fresh 16-byte nonce at offsets 20–35. The reply preserves all payload bytes except the opcode and uses guest source MAC 52:54:00:12:34:56 and the host destination MAC. Send retirement only after verifying the matching echo.

The expected owner UART markers in order are DMA PRIMARY READY, DMA PRIMARY RETIRE, DMA SPARE WAIT, DMA SPARE READY and DMA SPARE RETIRE. DMA SPARE WAIT follows the IPC reply. Inspect the device state to determine when retirement finished. Slot 1 prints DMA PEER TICK after each successful round of denied RX, TX and claim calls. It continues for five more ticks after receiving the spare's completion event. Each guest exits with status zero on success.

Read the private ELF symbols dma_echo_count and dma_ignored separately for each owner. The spare records dma_spare_preclaim and dma_spare_claim. The peer records dma_peer_rx, dma_peer_tx and dma_peer_claim as signed 64-bit results, dma_peer_ticks as a count, dma_peer_ns as the latest monotonic timestamp in nanoseconds and dma_peer_finished as the tick at completion notification.
