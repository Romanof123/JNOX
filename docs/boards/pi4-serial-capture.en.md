# Capture two Pi 4 scheduler boots

[한국어](pi4-serial-capture.md) · [Physical evidence issue #34](https://github.com/Romanof123/JNOX/issues/34)

1. Prepare the scheduler image and card using the [timer package guide](pi4-timer.en.md). Mount the card on the host and identify its boot directory, corresponding ELF, board revision and EEPROM version. Follow the [3.3 V UART wiring guide](pi4-p1.en.md). Connect only Pi TX to adapter RX and common ground. Leave power, DTR and RTS disconnected.
2. Physical capture requires pyserial in the host Python environment (`python3 -m pip install pyserial`). Run from the repository root. Substitute the actual device (e.g. `COM3` on Windows), paths, revision and EEPROM version. Choose a fresh output directory under an existing parent.

```sh
python3 tools/pi4_serial_capture.py --port /dev/ttyUSB0 --boot-dir /mnt/pi-boot --elf kernel/target/aarch64-unknown-none/release/jnox-kernel --board-revision REV --eeprom VERSION --output pi4-physical-01
```

3. The tool reads the boot files and ELF, checks packaged hashes and writes session metadata before the first prompt. Safely eject the mounted card and install it in the powered-off Pi. If you hashed a staged copy, independently check the card and record which directory you hashed.
4. For each run, physically remove board power. At the first prompt press Enter while the board is off. The tool opens the adapter at 115200 8N1 and discards stale input. At the second prompt press Enter and immediately apply power. For run 2, remove power again and keep the same board and card. The tool reads UART and writes host files. Power switching is manual.
5. Check both `run-*/result.json` records. A match requires the exact LF-only scheduler transcript within 30 seconds of host arming and one second without additional received bytes. Each directory contains `uart.bin` and `reads.jsonl`, with byte offsets and host read times. The unsigned `session.json` records artifact hashes and operator-supplied board details. Keep partial directories after interruption and use a fresh name for retries. Record the witness, board/card identity, both power events and artifact provenance with the evidence. UART markers alone cannot establish the interrupted PC or hardware timing.

## File replay and host tests

Replay uses two regular files and labels both results `file_replay`. Its timing describes host file reads. The fake-source tests and replay require only standard Python.

```sh
python3 tools/pi4_serial_capture.py --replay first.bin second.bin --boot-dir pi4-scheduler-boot --elf kernel/target/aarch64-unknown-none/release/jnox-kernel --board-revision replay --eeprom replay --output pi4-replay-01
python3 -m unittest discover -s tools -p "test_pi4_serial_capture.py"
```
