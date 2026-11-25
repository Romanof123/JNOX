# Native two-core ABI firsts / 두 코어 네이티브 ABI 최초 검증

Candidate 2423f0a7f3588ce2861a2721275ade3ac3eae064. The revised SDK command assembled its SIMD leaf separately. Ordinary Pi 4 SMP packaging and prelaunch seal succeeded. Exactly one plain and one traced first boot on the same raw hash b8e60bb9638a30ae7147c6af4cdb7bebc4465a94d378109d9cb7746bb8c74b15 and the prior stock raspi4b executable. The earlier adc98fd build failure remains separate. No additional observation boot or stage-1 prime replay.

Both boots completed all 33554432 iterations per core. Scalar and SIMD sums each equal 1125899906842624 for odd (CPU0, exit 12) and 1125899940397056 for even (CPU1, exit 77). Both roles have private writable backing for their TLS records, verified distinct roots/ASIDs, and final user hardware readbacks for x19 (1111222233334444 / aaaabbbbccccdddd hex), both q16 lanes (101,102 / 201,202 hex), TPIDR_EL0 (4020a0 / 4020a8 hex), TPIDRRO_EL0=0, FPCR=400000/800000 hex, FPSR=8000000/0 hex. Exit frames show the original TLS and FP environment restored. Sampled PPI30 frames came from EL0 at 0x400d2c; all 816 bytes equal the corresponding prepared frame at each of 12 boundaries. The compact archive supplies individual raw-frame digests, counts, sentinels and hot-loop disassembly.

| Position | CPU0 completed at three PPIs | CPU1 completed at three PPIs | Acceptance |
|---|---|---|---|
| Plain | 9141831, 19141703, 29202500 | 9319516, 19307282, 29410988 | Both journals show same-owner returns, later EL0-origin entries and increasing work, ending at verified products. No unambiguous cross-core overlap under offset [-1487,+2867]. |
| Traced | 9304191, 19208471, 29156773 | 9285905, 19279587, 29422769 | Source CPU and increasing work verified. Unlabeled ERET packets at identical virtual PC could not individually establish which core returned. Cross-core overlap unresolved under [-3303,+916]. |

Both report exits 2. Plain continuation is supported by per-core subsequent EL0 entry and the independent private output; traced per-ERET CPU assignment remains unresolved. These observations establish real productive activity on both emulated cores with nonzero ABI state, leaving the narrower time-overlap and physical-board claims open.

수정 SDK로 일반 ELF 두 개를 빌드하고 실행 전에 해시와 링크된 루프를 고정했다. 같은 이미지로 일반·추적 실행을 한 번씩 수행했다. 두 실행에서 각각 홀수·짝수 합산 33554432회를 끝냈고 스칼라와 SIMD 결과가 일치했다. 각 코어의 루프 내 실제 PPI30 3건마다 비영 x19, q16, TLS 및 FP 제어 상태와 전체 준비 프레임을 확인했다. 일반 실행은 이어지는 EL0 실행과 실제 진척을 확인했다. 추적 실행에서 동일한 PC로 복귀하는 ERET 패킷은 CPU별로 유일하게 귀속시킬 수 없었다. 시간 오차를 반영한 양 코어 실행 구간 겹침은 두 최초 실행 모두 미확인이다.
