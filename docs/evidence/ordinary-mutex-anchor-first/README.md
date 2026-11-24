# CHG-072 first optional cohorts / 선택형 이미지 최초 실행

Source da8005510e3d240660fc1f8f445161826852a291. Native WSL2 QEMU virt/GICv2, single Cortex-A72, TCG, 128 MiB, default clock, 15 s allowance. Builds used Windows MSVC Rust 1.95.0 for aarch64-unknown-none. First launches only, one boot per position. Complete footer in all 12, raw argv/PID/time/status in capture archive. All compare intervals exactly 6,250 ticks at 62.5 MHz. Raw samples and ELF/disassembly are included per position. A traced ERET record gives the destination PC, its cycle remains unobserved.

| Mode | Position | Guest reason | IRQ ELR | chunks | arm | CVAL | R | pre-ERET | vector |
|---|---|---:|---|---:|---:|---:|---:|---:|---:|
| rust-prepared | 01-ordinary-pi-plain | 0 | 0x40202b5c | 17 | 138256 | 144506 | 142239 | — | 161668 |
| rust-prepared | 02-ordinary-pi-traced | 16 | 0x402024ac | 0 | 132654 | 138904 | 141013 | — | 146050 |
| rust-prepared | 03-ordinary-ceiling-plain | 0 | 0x40202c84 | 66 | 113892 | 120142 | 117574 | — | 127455 |
| rust-prepared | 04-ordinary-ceiling-traced | 0 | 0x40202c84 | 102 | 138904 | 145154 | 142159 | — | 153201 |
| rust-stages | 01-ordinary-pi-plain | 0 | 0x40202b98 | 1 | 112574 | 118824 | 117564 | — | 128967 |
| rust-stages | 02-ordinary-pi-traced | 16 | 0x402024ec | 0 | 116670 | 122920 | 127753 | — | 132670 |
| rust-stages | 03-ordinary-ceiling-plain | 0 | 0x40202cc4 | 14 | 117356 | 123606 | 122102 | — | 130558 |
| rust-stages | 04-ordinary-ceiling-traced | 0 | 0x40202cc0 | 1 | 124767 | 131017 | 129755 | — | 139107 |
| selected-return | 01-ordinary-pi-plain | 0 | 0x40202d0c | 18 | 143768 | 150018 | 138299 | 147151 | 157111 |
| selected-return | 02-ordinary-pi-traced | 0 | 0x40202d0c | 36 | 115708 | 121958 | 112055 | 119231 | 131285 |
| selected-return | 03-ordinary-ceiling-plain | 0 | 0x40202e34 | 44 | 133046 | 139296 | 129276 | 136589 | 147260 |
| selected-return | 04-ordinary-ceiling-traced | 0 | 0x40202e4c | 0 | 132214 | 138464 | 128426 | 137127 | 146927 |

Rust-prepared and rust-stages each qualified 3/4 (PI 1/2, ceiling 2/2). Both traced PI failures ended with reason 16, IN_WORK zero and no completed chunks. Selected-return qualified 4/4 (PI 2/2, ceiling 2/2). The selected-return ceiling trace interrupted the instruction immediately after the input loop before publishing the first chunk. All qualifying boots completed the 256 chunk records, final count 65,536, sum 557,056 and MEDIUM sum 32,896. The independent replay checks ready membership, priorities, owners/waiters, handoffs, frames and registers, all restores and available external exception matches. Per-position interpretation contains actual interval and input-PC evidence.

Each optional linked image contains exactly one CVAL writer. In selected-return both ELFs have CLREX at 0x40201aa0, arm sample at 0x40201b98, CVAL/CTL writes at 0x40201ba4/0x40201bac and readbacks at 0x40201bb4/0x40201bb8. Final sample at 0x40201bf4 precedes one store, NZCV restoration, a bounded 832-byte frame revisit for x9–x14, SP recovery, branch and ERET at 0x40201c58. The added static block covers 109 instructions, including fail paths, adds no register spills and uses a 160-byte witness. Each rust-stages capture includes original-anchor write-end/readback-end/restore-entry values in P.

Default PI and ceiling both have equal executable text and seven bytes differing in six diagnostic source line numbers. Legacy mutex-bench has equal PT_LOAD segments and text, different full ELF. Separate hashes and reference paths are archived. All six invalid selector combinations exited 101. Offline host suites passed five decoder tests and two policy tests. Original RAW 1 remains a separate cohort. Results describe this virtual machine and observer.

선택형 세 코호트는 각각 독립적으로 집계했다. Rust 준비형과 단계 계측형은 각각 3/4였고 두 경우 모두 PI 추적 실행이 보호 작업에 진입하기 전에 IRQ를 받아 사유 16으로 종료됐다. 선택 프레임 반환형은 4/4였다. 각 첫 실행의 ELF, UART, 명령행과 추적 로그를 보존했다. 물리 보드나 최악 실행 시간은 측정하지 않았다.
