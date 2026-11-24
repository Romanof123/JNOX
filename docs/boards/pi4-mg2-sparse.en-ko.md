# Clustered MG2 sparse simulator pair / 클러스터 MG2 희소 관측

Before either first attempt, freeze SHA-256 of QEMU and GNU build ID, plugin and source, configure/build record, header, both complete argv arrays and the existing 30 s / 1 s quiet capture contract. Use the same plugin-capable 11.1.3 executable, raspi4b four CPUs / MTTCG and archived clustered inputs from pi4-migration-mg2-first.zip. First position has the observer off, second on. Preserve QMP RAM/journals and per-thread exception logs in both. Enable plugin log routing in both. Retain every failure. No replacement guest or replay under the first-attempt label.

| Archived input | SHA-256 |
|---|---|
| ELF | 8660010cca701e54f23a6af3c1782dd24bfd8da66d345147b5b4daba95f0002f |
| raw | cc3dbb8ececa7f00c01164b716310596d73bdad1c8eed49753f9f3a5ea60ce79 |
| linker.map | 28eef4b2abd972fd79f9ffb69267881aa659301b5f9288f94a209074296d9666 |
| disassembly | aaa48104cf61ea6d1de88b2441929cd2ff0528fa7d2b13f9c42a7ae5f24e683a |

The linked pi4_migration_log::event CPU0 STLR at 0x81a00 publishes the completed row count to LOG (0xa9178) + 6912. Require the exact original instruction word 0xc89ffd28, eight-byte RAM store, matching VA/PA and count 1..96. Read back count, committed row and prior dispatch at fixed bounded RAM addresses on the same masked vCPU. Require kind 5, task 0, positive equal before/after lease, epoch-zero latest kind-11 dispatch matching task/lease, index 65536 and sum 2147516416. The sparse checkpoint logged PC is 0x801f0. Require successful independent terminal RAM reconciliation before credit. A failed read is incomplete. Returning from the logger alone cannot prove its successful path.

The linked pi4_smp_bsp::signal STR at 0x83954 (0xb9000100) on CPU1 must issue a little-endian four-byte MMIO write at VA/PA 0xff841f00 with observed value 0x18001. The pinned tcg/tcg-op-ldst.c emits the memory callback after the guest store operation. Synchronous arm_gic distributor handling has returned by this point. This does not timestamp the internal pending-bit mutation or receiver exception. No device readback. Confirm exact linked words at translation. Sample the other actor's release flag with acquire immediately before each candidate instruction. Publish a release flag only after that actor's transaction and identity checks. A positive pre-sample on the independently validated later transaction proves one order. Two zero samples leave overlap or absence unresolved. Reject duplicates and inconsistent evidence. Host order of log lines is immaterial. No arithmetic-loop hook, timings or qualification credit.

두 순번을 시작하기 전에 동일 실행 파일의 해시·빌드 ID, 플러그인 및 소스, 빌드 기록, 전체 인자와 원본 네 입력을 고정한다. 첫 번째는 플러그인을 끄고 두 번째는 켠다. 같은 MTTCG 설정과 시간 한도를 쓰며 실패 및 RAM·예외 원본을 보존한다. CPU0의 실제 커밋 쓰기와 읽어 확인한 행에서 태스크·임대·인덱스·합·배정 세대를 검사한다. CPU1의 실제 장치 쓰기에서 주소·폭·데이터를 확인한다. 각 쓰기 직전 상대 완료 플래그를 acquire로 읽고 해당 쓰기가 검증된 뒤에만 release로 공개한다. 양의 표식과 독립 RAM 대조가 모두 있을 때만 순서를 인정한다. 양쪽 모두 0이면 미결이며 내부 pending 시각, 실물 지연 또는 이주 성공을 의미하지 않는다.
