# Native mailbox capability telemetry

Three ordinary native executables share one profile. Slot 0 is the C origin with one SEND|DELEGATE capability targeting slot 2. Slot 1 is the Rust recipient. Slot 2 is a separate C telemetry consumer that reads the real mailbox with SVC 25. The origin uses synchronous CALLs to order phases in the other two processes. Those control routes carry command numbers and acknowledgements. The actual samples go through mailbox capability sends. Numeric rights 1, 2 and 3 mean SEND, DELEGATE and both.

These fixtures specify expected behavior for independent FIFO/RR runs. Build the three applications and package each case from the repository root. Use a fresh output directory.

```bash
set -eu
out=$(mktemp -d)
app=examples/native-capability-ipc
sh sdk/native-aarch64/build.sh "$app/origin.c" "$out/origin.elf"
sh sdk/native-aarch64/build-rust.sh "$app/receiver/Cargo.toml" "$out/receiver.elf"
sh sdk/native-aarch64/build.sh "$app/target.c" "$out/target.elf"
rev=$(git rev-parse HEAD)
for policy in fifo rr; do
  quantum=()
  if [ "$policy" = rr ]; then quantum=(--quantum-ticks 625000); fi
  for scenario in transfer before pending active target mailbox; do
    pkg="$out/$scenario-$policy"
    python3 tools/build_native_el0.py --out "$pkg" \
      --elf "$out/origin.elf" --peer-elf "$out/receiver.elf" --peer-elf "$out/target.elf" \
      --arg "$scenario" --policy "$policy" "${quantum[@]}" \
      --priority 7 --peer-priority 4 --peer-priority 2 \
      --ipc-route 0:1 --ipc-route 0:2 \
      --cap-grant 0:2:3 --cap-route 0:1 --cap-route 1:0
    python3 tools/native_el0_capture.py "$out/cap-$scenario-$policy" "$pkg/kernel.elf" \
      "$out/origin.elf" "$app/origin.c" "$rev" --plain --manifest "$pkg/manifest.json" \
      --expect-exit 71 --timeout 20
  done
done
```

The package has no direct mailbox-send routes. Both senders try ordinary SVC 24 targeting slot 2 in the transfer case and expect EACCES. The origin also tries a capability transfer to slot 2, outside its two allowed transfer directions, and expects EACCES. A successful control command echoes its number with prefix 0xace0. An exit command deliberately receives no reply. Its blocked caller gets EPIPE when the peer exits, establishing the ordering for the next operation independently of timer speed.

| Scenario | Expected observations |
| --- | --- |
| transfer | Copy two handles. MOVE with two queued notices fails with EAGAIN. The origin still sends sample 1. The receiver takes tags 0 and 1 exactly once and sends sample 2 using the SEND-only copy. It cannot delegate that copy; its separate DELEGATE-only copy cannot send or create a SEND right. Copy two more, consume both notices, and reach four live handles with zero notices. MOVE fails with EMFILE and the source sends sample 3. After the receiver closes one handle, MOVE succeeds with tag 4 and invalidates the origin. The receiver gets BOTH at the reused recipient slot under a new generation. Its old value is stale. It sends sample 4, copies SEND back under tag 5, and later sends sample 6. The origin sends sample 5 through the returned handle. All live handles are explicitly closed. |
| before | Recipient exits before MOVE. MOVE returns EPIPE. The origin's retained handle sends sample 7 to the still-live target, then closes successfully. |
| pending | MOVE succeeds. Recipient observes one installed handle and one notice and exits without taking the notice. The original handle is stale. The target has no samples. |
| active | MOVE succeeds. Recipient takes the notice, sends sample 8, observes one handle and no notices, then exits still holding its handle. The target drains that one committed word after the recipient's death. |
| target | Recipient takes one SEND copy, then a second copy is left pending. The target exits. Both previously taken sender handles report EPIPE, the second notice has been removed, and both survivors observe zero handles and zero notices. |
| mailbox | The origin fills the four successful target mailbox entries using its capability. A fifth polling send returns EAGAIN. Target reads samples 9–12, origin retries sample 13, then target reads exactly that one word. |

For every case the expected PID exits are 71, 72 and 73. The target drains at commanded boundaries and checks an additional empty receive returns −11. In `transfer` the target expects exactly samples 1–6, grouped into reads of three. Sample n equals `0xcafe000000000000 | (n << 32) | (0x301 + 11*n)`. A transfer tag with index n equals `0xe910000000000000 | n`. Failed transfer tags 90–95 must not appear in delivery records.

| Private ELF symbols | Captured values |
| --- | --- |
| Origin `cap_origin_log`, `cap_origin_nlog` | Rows contain operation ID, signed status encoded in u64 and a handle, tag, sample, PID or returned command word. IDs 30/31 are failed moves, 32 is the successful move, 33–35 are stale source checks, 36 is the returned notice, 37 is its empty retry, 38 is the rights refusal, and 39 closes the returned handle. |
| Origin `cap_origin_counts`, `cap_origin_ncounts`, `cap_origin_result` | Count rows contain phase, handles, notices, peak handles and peak notices. Initial counts are 1/0. The successful move leaves 0/0 at the origin. |
| Receiver `cap_receiver_notices`, `cap_receiver_nnotices` | Each row contains the full tag, recipient-local handle, sender, target and rights. In `transfer`, expect tag indices 0–4 with rights 1, 2, 1, 1, 3 and local raw handles 20, 21, 22, 23, 36. Origin raw 16 is stale after MOVE; the returned SEND-only copy in the origin has raw 32. |
| Receiver `cap_receiver_counts`, `cap_receiver_ncounts` | Same five-word row format. Transfer phases 1–8 have current handles/notices 2/2, 2/0, 4/2, 4/0, 4/0, 3/0, 4/1, 0/0. Peak handles/notices are 4/2. |
| Receiver `cap_receiver_log`, `cap_receiver_nlog` | Signed syscall outcomes, including the empty notice retry, direct/attenuated denials, stale pre/post-reuse operations, actual sends and closes. |
| Target `cap_target_words`, `cap_target_nwords`, `cap_target_empty`, `cap_target_nempty` | Actual words read from the bounded mailbox and subsequent empty receive results. Read only the recorded prefixes. |

Check private records through each process's actual page-table root. Correlate the mailbox and fixed scheduler journals, the target words, and the kernel capability state at boot and termination. All terminal handles, notices and target references should be zero. The separate death scenarios use naturally scheduled exits; integer generation exhaustion and exact same-tick event permutations require separate deterministic controls.
