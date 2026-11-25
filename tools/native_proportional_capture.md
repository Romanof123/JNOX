# Capturing native proportional service

Keep the package directory, all linked user ELFs, source revision and toolchain identity with each cold run. For the three extended analytics workloads, from the repository root on the ordinary capture host:

```sh
python3 tools/native_el0_capture.py capture package/kernel.elf package/user.elf examples/native-weighted-analytics/checksum.c "$(git rev-parse HEAD)" --manifest package/manifest.json --expect-exit 237 --timeout 180 --uart-limit 33554432
python3 tools/native_proportional_replay.py package capture --session-sha256 "$SESSION_SHA256" --products crc,histogram,energy
```

Set SESSION_SHA256 to the session digest printed by capture. The exit argument identifies the primary slot. For the reporter pair use exit 53, the reporter source and `--products reporter,crc`. For a solo checksum use `--products crc`. Add `--stride-prefix 48` for the frozen three-client stride interval. Use distinct directories per launch. Capture records the ordinary physical clock and retains QEMU exception tracing by default. Add `--plain` for the corresponding untraced boot.

The reader binds the UART, RAM and kernel to the supplied session and package, checks each startup stack and reconstructs the draw or pass ledger from the observations. JSON includes raw service per process, each configured window, and requested stride prefixes with exact rational spreads and deviations. Products come from private virtual addresses translated through the captured slot roots. Omit the product option for journal-only analysis. Exit 0 means the requested replay completed, 1 indicates a mismatch, and 2 indicates missing, dropped or conditionally unscorable evidence. Retain the original files for independent disassembly, exception-stream correlation and review.
