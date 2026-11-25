# Revised native Pi 4 two-core firsts / 수정 후보 최초 검증

Clean detached head 5d43712ae146ecc1ea3c4f1bb141e455b2c4f4b5. Five positions and 120-second cold limit kept in the original order. The C/Rust prime ELFs were taken from #322. Their bytes, the rebuilt calendar ELF and the current legacy raw boot image each match the preceding 0297172 cohort. Same stock QEMU 11.1.3 raspi4b, disposition backed by per-command raw logs. No retries.

| Position | Result |
|---|---|
| 1/2 pair, plain/traced | First operator package reached Rust kernel compilation and exited 1. No bootable raw or manifest for either guest. |
| 3/4 remote calendar, plain/traced | First package with explicit 0,0,1 cells reached kernel compilation and failed identically. |
| 5 swapped | First package failed at the same compilation stage. |
| 6 legacy FIFO plain | One cold boot and authenticated replay, both exit 0. Separate PA 33775616 and 38055936 for VA 0x402000 yielded 78498/70435, exits 12/77. Zero reported dropped events. Secondary park PCs were not sampled. |

All three build logs report the assembler diagnostic “symbol '.Lcheck_el1' is already defined”. Definitions occur in arch/aarch64/start.S:56 and kernel/src/pi4_native_smp.S:48. Valid slot processing advanced past the previous Python exception. The former firsts remain in their own cohort. Zero mask, outside mask, extra ELF and wrong board were all rejected preboot (status 2). Without an SMP kernel image, none of the five SMP runtime positions, full-frame/TLS isolation, productive concurrency, or remote SGI chronology can be scored.

이전 후보와 같은 입력·순서로 일반 패키지 생성을 수행했다. 릴리스 셀 처리는 통과했지만 페어·원격·마스크 교환 3개 패키지 모두 커널 어셈블리에서 중복 심볼 .Lcheck_el1 오류로 끝났다. SMP 게스트 5개를 시작할 수 없어 두 코어의 작업 진행이나 원격 선점을 판정할 근거가 없다. 최초 실패와 음성 대조군 4개를 보존했다. 기존 Pi FIFO는 실제 모델 부팅과 사설 메모리 판독을 완료했다.
