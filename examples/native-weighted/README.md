# Native weighted application inputs

From the repository root, build each C file separately using
`sh sdk/native-aarch64/build.sh examples/native-weighted/checksum.c checksum.elf`
(substitute the chosen C name). Build the histogram with
`sh sdk/native-aarch64/build-rust.sh examples/native-weighted-rust/Cargo.toml histogram.elf`.

| Application | Fixed input and complete expected private_result | Final private_progress | Success status |
| --- | --- | ---: | ---: |
| checksum | CRC-32/ISO-HDLC over 65,536 copies of bytes 0 through 255, 0x2a223dad | 16777216 bytes | 52 |
| histogram | 262,144 blocks of 256 bytes, each shifted by its block index modulo 256. Bit-count bins 0 through 8 equal [262144, 2097152, 7340032, 14680064, 18350080, 14680064, 7340032, 2097152, 262144] | 67108864 bytes | 197 |
| continuous_reporter | Sum of squared signed samples -128 through 127 for 524,288 frames, 733030121472 | 134217728 samples | 242 |
| reporter | Three fresh absolute 40 ms waits, one successful deadline check per bit, mask 7 | 3 reports | 7 |
| boundary | Common absolute grid wake, second wait and final yield, result 17 | 3 | 17 |
| boundary_peer | Common absolute grid wake and exit, result 19 | 2 | 19 |

The CRC can be independently computed with Python zlib.crc32 over the specified byte sequence. Histogram bin k is 262144 times the binomial coefficient C(8,k). The histogram status is the sum of bin[k] times (k+1) squared, modulo 251. The checksum and energy statuses are the full result modulo 251. The energy is 524288 times the sum of i squared from -128 through 127. In the timed reporter the three requested absolute deadlines and subsequent user clock observations are uint64 nanosecond arrays report_deadline_ns and report_observed_ns. Each boundary client records its own grid target in boundary_deadline_ns.

Use checksum/histogram for the pair and swapped-weight control, checksum/reporter for the timed pair, checksum/histogram/reporter for mixed 1:2:3 and checksum/histogram/continuous_reporter for all-ready 1:2:3. Run boundary alone for solo yield and with boundary_peer for the idle and exit control. Match installed ELF symbols checksum_stream, histogram_stream and measure_energy to preemption PCs. Check per-slot private RAM against the table. Apply the predeclared first-36 and first-60 rules to the recorded observations.
