# Native IPC scheduling and cancellation cases

Build the C application once from the repository root. Each package loads its own copy of that ELF in two or three private address spaces. The argument selects the role. Slot 0 is the primary ELF, slots 1 and 2 follow the order of the peer ELFs.

```bash
out=output/ipc-cases
mkdir -p "$out"
app="$out/operational.elf"
sh sdk/native-aarch64/build.sh examples/native-sync-ipc/cases/operational.c "$app"
pack=(python3 tools/build_native_el0.py --policy fifo --priority 7 --elf "$app")

"${pack[@]}" --out "$out/donation" --arg high \
  --peer-elf "$app" --peer-priority 4 --peer-arg medium \
  --peer-elf "$app" --peer-priority 1 --peer2-arg low --ipc-route 0:2
"${pack[@]}" --out "$out/timeout" --arg timeout \
  --peer-elf "$app" --peer-priority 1 --peer-arg late --ipc-route 0:1
"${pack[@]}" --out "$out/death" --arg orphan \
  --peer-elf "$app" --peer-priority 1 --peer-arg depart --ipc-route 0:1
"${pack[@]}" --out "$out/nested" --arg root \
  --peer-elf "$app" --peer-priority 4 --peer-arg relay \
  --peer-elf "$app" --peer-priority 1 --peer2-arg leaf \
  --ipc-route 0:1 --ipc-route 1:2 --ipc-route 2:0

# Select donation, timeout, death or nested. Each boot ends in the kernel park loop.
case_name=donation
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
  -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio \
  -kernel "$out/$case_name/kernel.elf"
```

| Package | Expected exit by slot (0, 1, 2) | What to check |
| --- | --- | --- |
| donation | 51, 52, 53 | HIGH calls LOW with MED ready. The first switch is 0→2. LOW's successful reply returns service to HIGH. After HIGH exits, MED executes a million volatile sample reads, verifies sum 133693440 and exits before LOW resumes. The owner sequence, ignoring consecutive repeats, is 0, 2, 0, 1, 2. |
| timeout | 54, 55 | The client sets D to its guest start time + 100 ms, receives −110 at or after D and submits another request. The server sleeps until D + 50 ms, receives −116 on its first token and answers the new request with a distinct token. Check that the timeout woke the client while the server slept. |
| death | 56, 57 | The server receives the request and exits holding its token. Both the blocked call and a new call to the dead endpoint return −32. |
| nested | 58, 59, 60 | In the live 0→1→2 chain, the relay sends its outer token as data. The leaf gets −1 when trying to use that foreign token, then −35 when the authorized call 2→0 would close the cycle. Both original calls complete with their checked replies. |

The count request is 0xf000001190000007. Its two unsigned 32-bit counts sum to 0x180000018. A successful outer client checks every reply bit. A failure exits 81–107 at the corresponding branch in the source. At each slot's `ipc_case` symbol inspect the signed operation statuses, received words, reply words, tokens and monotonic times. MED records its checksum. In the donation run, match actual owner changes and service counters to the application intervals. The summary exit lines are printed in slot order.

These cases leave simultaneous deadline/reply or deadline/death, multiple queued callers' FIFO order and cancellation before receive to separate fixtures. The nested case exposes the chain and its completion. An effective-priority trace or a policy test is needed to check both inherited ranks along that chain.
