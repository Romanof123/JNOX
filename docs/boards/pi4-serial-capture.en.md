# Capture two Pi 4 scheduler boots

[한국어](pi4-serial-capture.md) · [Physical evidence issue #34](https://github.com/Romanof123/JNOX/issues/34)

1. Prepare the scheduler image and card using the [timer package guide](pi4-timer.en.md). Mount the card on the host and identify its boot directory, corresponding ELF, board revision and EEPROM version. Follow the [3.3 V UART wiring guide](pi4-p1.en.md). Connect only Pi TX to adapter RX and common ground. Leave power, DTR and RTS disconnected.
2. Physical capture requires pyserial in the host Python environment (`python3 -m pip install pyserial`). Run from the repository root. Substitute the actual device (e.g. `COM3` on Windows), paths, revision and EEPROM version. Choose a fresh output directory under an existing parent.

```sh
python3 tools/pi4_serial_capture.py --port /dev/ttyUSB0 --boot-dir /mnt/pi-boot --elf kernel/target/aarch64-unknown-none/release/jnox-kernel --board-revision REV --eeprom VERSION --output pi4-physical-01
```

3. The tool reads the boot files and ELF, checks packaged hashes and writes session metadata before the first prompt. Safely eject the mounted card and install it in the powered-off Pi. If you hashed a staged copy, independently check the card and record which directory you hashed.
4. For each run, physically remove board power. At the first prompt press Enter while the board is off. The tool opens the adapter at 115200 8N1 and discards stale input. At the second prompt press Enter while power is still off. Apply power when the tool prints “Armed”. For run 2, remove power again and keep the same board and card. The tool reads UART and writes host files. Power switching is manual.
5. Check both `run-*/result.json` records. A match requires the exact LF-only scheduler transcript within 30 seconds of host arming and one second without additional received bytes. Each directory contains `uart.bin` and `reads.jsonl`, with byte offsets and host read times. The unsigned `session.json` records artifact hashes and operator-supplied board details. Keep partial directories after interruption and use a fresh name for retries. Record the witness, board/card identity, both power events and artifact provenance with the evidence. UART markers alone cannot establish the interrupted PC or hardware timing.

## File replay and host tests

Replay uses two regular files and labels both results `file_replay`. Its timing describes host file reads. The fake-source tests and replay require only standard Python.

```sh
python3 tools/pi4_serial_capture.py --replay first.bin second.bin --boot-dir pi4-scheduler-boot --elf kernel/target/aarch64-unknown-none/release/jnox-kernel --board-revision replay --eeprom replay --output pi4-replay-01
python3 -m unittest discover -s tools -p "test_pi4_serial_capture.py"
```

Nine host tests passed. A file replay using the archived EVD-014 package preserved both byte streams and their independent run IDs. An additional byte and an existing output directory were detected. A physical power cycle and board identity still require independent witnessing. [Host-only review](../evidence/scheduling-tools-host-review.json)

<a id="chg-062"></a>

## CHG-062 · Planned physical capture profiles

The current host tool compares each read with a slice of the single `pi4-scheduler` transcript. Extend its port and two-file replay modes with optional `--profile pi4-scheduler|pi4-fixed-fifo|pi4-fixed-rr`. Omitting the option preserves the existing scheduler invocation, output files, exit behavior and default for direct observer callers. The fixed images are the ordinary uninstrumented Pi builds from #71. Track physical acceptance in [#34](https://github.com/Romanof123/JNOX/issues/34) and [POL-002 / #21](../scheduling-release-cases.en.md).

**Byte contract.** All streams begin with `JNOX PI4 BOOT\nJNOX PI4 READY\n`. Each subsequent event in the table is a separate line with the stated prefix, a space and one LF. Accept each complete sequence exactly as listed.

| Profile | Prefix | Ordered event text |
|---|---|---|
| pi4-scheduler | SCHED | LOW RUN → TIMER IRQ → HIGH RUN → LOW RESUMED → PASS |
| pi4-fixed-fifo | FIXED FIFO | START → A START → B YIELD → A RESUME → HIGH → A DONE → B SLEEP → B DONE |
| pi4-fixed-rr | FIXED RR | START → A START → HIGH → B START → A DONE → B DONE |
| pi4-fixed-rr, HIGH first | FIXED RR | START → HIGH → A START → B START → A DONE → B DONE |

Use one bounded byte matcher for the three Pi contracts, shared by the serial/replay collector and the Pi profiles in qemu_boot. Preserve the existing TRANSCRIPTS keys, fixed observer API and other board profiles. Fixed Pi entries are DynamicTranscript values; slicing them as bytes is invalid. The generic RR validator currently allows HIGH → B START → A START under its partial order. Reject that order for Pi fixed capture in both paths. Across arbitrary read fragments keep only viable byte prefixes, detect completion at the final LF, and reject an impossible prefix or trailing byte immediately, including in the read containing the final LF. Preserve the raw failing read and its offset. Physical and file-replay Pi inputs are LF-only. Preserve caller-configured QEMU newline and clock behavior outside that strict byte contract.

**Witness and attribution.** For each selected profile collect two independently witnessed full power removals and starts on the same identified board/card/package; FIFO and RR need separate pairs. Arm while off, then apply power. Final LF must arrive by 30 seconds inclusive from host arm, followed by a full second without another received byte. Retain failures and partial runs under their own IDs; retries use a fresh directory. Retain session/run IDs, declared profile, exact accepted variant (or no match), source mode, raw digest and host read offsets/times. A witness records both power events, board revision/EEPROM, card, adapter and whether the hashed directory was the mounted card or staged copy. Tie the declared build options and revision to the actual ELF, kernel8.img, config, pinned firmware and SHA256SUMS using recorded hashes. A transcript match or matching filename cannot establish a package's origin. Report unverified associations explicitly. Model replay uses the archived package identity, never invented field values.

**Offline acceptance.** Verify the EVD-031 archive digest and member hashes before replaying each original FIFO and RR run-1/run-2 UART against its own package/ELF references. Both archived RR traces have A START first; label a constructed HIGH-first permutation as a host control. Regress EVD-014 scheduler uart_hex with its archived packaging metadata, explicit scheduler and omitted profile. Replaying a scheduler stream twice checks independent run IDs only. Exercise bytewise and split-marker reads, both RR alternatives and mixed alternatives within one pair, wrong profile/board tag, missing final LF, CRLF, reordered/duplicated/missing markers, malformed bytes, appended bytes in the same read and during quiet, short reads, and deadline/quiet boundaries with an injected host clock. Compare chunked/whole verdicts and retained raw bytes. Close the physical UART item only after witnessed hardware runs. USB timestamps and model exception traces do not measure physical IRQ delegation, interrupted PC, board latency or WCET. [EVD-031 archive](../evidence/evd-031-raw.json) · [Package index](../evidence/evd-031.json) · [EVD-014](../evidence/evd-014.json) · [Fixed runtime guide](pi4-fixed-runtime.en.md)

## Host capture and replay for fixed workloads

Use a new evidence directory and a separate matching package for each profile. The default remains `pi4-scheduler`. Each fixed-profile invocation collects two boots. Preserve the exact mounted-card or staged-directory identity and the two externally witnessed power starts.

```sh
python3 tools/pi4_serial_capture.py --profile pi4-fixed-fifo --port /dev/ttyUSB0 --boot-dir /mnt/pi-boot --elf /evidence/fifo/kernel.elf --board-revision "$REV" --eeprom "$EEPROM" --card-id "$CARD" --adapter-id "$ADAPTER" --witness "$WITNESS" --boot-dir-source mounted-card --build-record /evidence/fifo-build.json --output fifo-physical-01

python3 tools/pi4_serial_capture.py --profile pi4-fixed-rr --replay /archive/rr/run-1/uart /archive/rr/run-2/uart --boot-dir /archive/rr/boot --elf /archive/rr/kernel.elf --boot-dir-source archive --output rr-replay-01
```

The optional build-record JSON carries `profile`, `revision`, `build_options`, `elf_sha256`, `files_sha256` (a filename-to-digest map), `package_manifest_sha256` and `firmware_commit`. The session copies those declarations, hashes the submitted record and compares it against the specific files on this host. A build or package mismatch is recorded separately from the UART verdict. Linking a witnessed boot to those candidate files still needs the card, board and power record. A replay with missing board or EEPROM information records null if the options are omitted. Use the archived values when available.

Each run records its session/run IDs before opening the source. The final result adds the selected profile, exact variant, first mismatching byte offset when applicable, host arm time and observed quiet duration. An operator acknowledgment records that power was off. Keep the separately witnessed actual application of power with the capture. Replay times describe host file reads. The EVD-031 model and EVD-014 package replays can verify the host tool; physical FIFO/RR acceptance remains tracked in [#34](https://github.com/Romanof123/JNOX/issues/34).
