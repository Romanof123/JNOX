# First slot-zero frame-window diagnostic / 슬롯 0 프레임 검증 최초 진단

The frozen four positions at `f20232585815e052cf3c5c861e8e851e6558b5b6` produced two observed frame brackets. At 62.5 MHz they occupied 6,555 ticks (104.880 µs) with external tracing and 7,098 ticks (113.568 µs) without it. The traced baseline stopped before publication. The plain baseline completed eight grants in ten slots. Both probes stopped after A's first worker marker exceeded the unchanged 62,500-tick opening bound. All four retained their terminal journal. These observations do not establish how much of the uninstrumented baseline interval was spent validating frames.

네 개의 최초 실행에서 두 계측 구간이 관측됐다. 프레임 검증과 계측 자체를 포함한 구간은 외부 추적 시 6,555틱, 일반 실행 시 7,098틱이었다. 추적 기준 이미지는 게시 전에 중단했고, 일반 기준 이미지는 10슬롯에서 8개 할당을 완료했다. 두 계측 이미지는 A의 최초 작업 표본이 기존 62,500틱 한계를 넘긴 뒤 중단했다. 진단 결과는 원래 8건의 합격 판정과 별개다.

| First position / 최초 위치 | Source end → precommit ticks / 전체 | Source → before + bracket + after → precommit / 세 구간 | Headroom at source, before, after, precommit / 남은 틱 | First worker offset / 첫 표본 | Pre-ERET → worker | Actual grants |
|---|---:|---|---|---:|---:|---:|
| 1 B trace | 31478 | — | 30741, —, —, −737 | — | — | 0 |
| 2 P trace | 19391 | 9937 + 6555 + 2899 | 36826, 26889, 20334, 17435 | 62735 | 3007 | 1 |
| 3 P plain | 23852 | 13745 + 7098 + 3009 | 36819, 23074, 15976, 12967 | 64219 | 2173 | 1 |
| 4 B plain | 14611 | — | 41929, —, —, 27318 | 51750 | 2688 | 8 |

P minus B for the whole interval was −12,087 ticks with tracing and +9,241 without tracing. Position 1 recorded source and precommit (mask 3), then branch (2,5) with operands 3,231,936 and 3,168,699. Positions 2 and 3 reached selection (mask 31) and a real worker return; branch (2,2) compared 3,228,278 with 3,165,543 and 3,223,544 with 3,159,325. A's single charged intervals were 3,012,478 and 3,007,399 ticks respectively. Position 4 reached its horizon with mask 31. The archive gives every original charge, allocation deadline and actual stop. The independent ledger replay preserved ownership, work recurrence, first-return and closing checks after removing only the two RAW5 diagnostic rows. Traced positions also matched their same-process exceptions and returns.

기준 대비 계측 전체 구간의 차이는 추적에서 −12,087틱, 일반 실행에서 +9,241틱이었다. 1번은 커밋 직전 737틱 초과로 중단했다. 2·3번은 실제 복귀한 작업자에게 각각 3,012,478틱과 3,007,399틱을 부과했다. 실행마다 도달 단계와 시간 편차가 달랐다. 보존된 원장에서 진단 두 행만 제거한 뒤 소유권, 산술, 복귀, 종료 및 할당 기한별 부과량을 대조했다.

**Preflight.** A Linux-host build had 179 different RX bytes against the archived Windows-built #172 baseline and never launched. Rebuilding both arms with the original Windows Rust 1.95.0 toolchain, one job, reproduced B's entry, all PT_LOAD headers and bytes. Its full ELF hash differed. P's linked IRQ initializes the frame-loop index to zero at 0x40201814. ISB/MRS/ADR sample before the loop at 0x4020182c. Success advances the index by eight until 16; failing checks go to unexpected. The successful exit samples again at 0x4020194c before recording the pair. The destination-bank load follows at 0x402019a0 and the precommit sample at 0x40201a18. P added 292 text bytes (73 instruction words), 48 BSS bytes, and zero bytes to data or rodata. All three reader-control groups passed, including ten malformed-pair mutations and five artifact/position mutations. The original manifest SHA-256 is `4266753ef7131bd7a65e0c01fe0d205b3bc723f3147b7548571d32efa2eeb3f4`. QEMU ran one fresh process per position on Linux ext4; all four reached RSRV END before the recorder's 15-second stop.

리눅스 호스트 빌드는 원래 Windows 빌드와 RX 바이트가 달라 기동하지 않았다. 원래 도구 체인으로 두 이미지를 다시 빌드해 기준 이미지의 모든 적재 헤더와 바이트를 검증했다. 계측은 두 프레임 검증의 정상 경로를 감싸고 대상 뱅크 접근 전에 쌍을 기록했다. 코드 292바이트와 BSS 48바이트가 추가됐다. 세 판독기 대조군이 모두 통과했다. [Archive and replay instructions / 원본·재생 절차](reservation-frame-window-first.zip) · [Frozen plan / 고정 계획](../reservation-frame-window.en-ko.md)
