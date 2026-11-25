# Independent native hard-CBS verification / 네이티브 하드 CBS 독립 검증

[Public source c84539f](https://github.com/Romanof123/JNOX/tree/c84539f07c7023b1ac1f6740d7f6c238f20f2ad1) · [Prospective design 8647b53](https://github.com/Romanof123/JNOX/blob/8647b534f3ad9a9cb5ef9da7491f32f7dd930a15/docs/native-cbs-applications.en-ko.md)

Fourteen first positions ran serially on one emulated Cortex-A72, QEMU 6.2.0 virt/GICv2, TCG, 128 MiB and the default clock. The timer frequency was 62,500,000 Hz, each nominal slot 3,125,000 counts and the guard 62,500 counts. A grant's first pre-ERET sample bounds its EL0 return from below. Its first observed EL0-origin exception bounds the return from above. The three completed event streams each had one first-return bracket spanning the 1 ms limit. Deployment timing qualification remains partial.

동일한 단일 가상 CPU에서 일곱 구성의 일반 실행과 외부 추적 실행을 차례로 관측했다. 슬롯은 50 ms, 허용 경계는 1 ms였다. 복귀 직전 카운터와 최초 EL0 예외의 카운터 사이에 실제 복귀가 있다. 이벤트 스트림이 끝까지 기록된 세 실행 모두 첫 복귀 구간이 1 ms 경계를 걸쳐 전체 타이밍 판정은 부분 검증으로 남는다.

| Configuration / 구성 | First plain / 최초 일반 실행 | First traced / 최초 추적 실행 |
|---|---|---|
| Saturated / 포화 | First eight 0,1,0,1,0,1,0,1. CRC 14,567 blocks and histogram 21,401 verified. Both exit events, 15/16 launch upper bounds within guard. Terminal status line truncated. | Capture transport failed during boot, before a CBS event. |
| Isolated / 격리 | Slots 0,1,4,5 went to 0,1,0,1. No grants in slots 2,3,6,7. CRC 6,575 and histogram 11,023 blocks verified, statuses 17/29. 9/10 launch upper bounds within guard. | Opening inhibited at +187,814 counts. |
| Reversed / ELF 순서 반전 | Pre-return inhibited at +83,508. | Opening inhibited at +122,859. |
| Over capacity / 용량 초과 | 9/8 refused, category 2. All private block counters zero. | Same refusal and zero progress. |
| Three servers / 세 서버 | Slot 2 selected, pre-return inhibited at +68,294. | Opening inhibited at +113,878. |
| Block/yield with short peer / 짧은 동료 | Statuses 59/53 and 3/1 blocks verified. Wake at slot 4 retained (q,d)=(1,8) on equality. Yield then resumed in slot 8. 3/4 launch upper bounds within guard. | Opening inhibited at +129,245. |
| Block/yield with histogram / 긴 동료 | Pre-return inhibited at +69,915. | Opening inhibited at +143,527. |

All offsets are physical counter counts after nominal opening. Saturated, isolated and short plain first brackets were [60,555, 69,615], [55,415, 64,152] and [58,817, 70,429]. All three streams' recorded outgoing stop upper bounds, individual grant and per-allocation owner-wall bounds satisfied the normal caps. Maximum observed long cutoff lateness was 12,270 counts in saturated plain and 18,093 in isolated plain. Saved and restored private PC/SP matched on 14/14, 8/8 and 2/2 respective reentries. Final results were checked against zlib CRC-32, all 16 arithmetic histogram bins, byte counts and short-worker sums. Ordered host QMP progress samples establish productive work, with no synchronized per-instruction elapsed EL0 measurement.

표의 지연값은 명목 슬롯 시작 이후의 물리 카운터 수다. 포화·격리·짧은 동료 일반 실행에서 관측한 정지 및 소유 시간은 슬롯별·할당별 상한을 충족했다. 긴 실행의 컷오프 지연 최댓값은 각각 12,270 및 18,093카운트였다. 실제 작업량은 각 프로세스의 메모리에서 CRC와 16개 히스토그램 구간 또는 작업 합계를 독립 복원해 확인했다. QMP의 진행 샘플은 호스트 시계로 측정했으므로 개별 명령의 EL0 경과 시간을 정밀하게 결정할 수 없다.

Eight host policy tests passed, including three-server order, admission arithmetic overflow and wake arithmetic. CLI rejected zero, Q>P and malformed tuples. Raw Cargo validation and native arithmetic refusal were not established in this independent cohort. Native zero-budget wake, excess density, expiry, lower density and already-due sleep remained unobserved. No natural outgoing cutoff through nominal end +2 ms was observed. Historical linked gates and physical-device execution require their own evidence.

호스트 정책 시험 8개를 통과했으며 CLI는 0, Q>P와 형식 오류를 거부했다. 이 코호트의 네이티브 관측으로 확인한 깨움 밀도 분기는 등호였다. 이미 만료된 수면 요청, 나머지 밀도 분기, 산술 오버플로에 대한 게스트 거부 및 raw Cargo 검증은 이 독립 결과에서 미확인이다. 컷오프가 슬롯 끝을 2 ms 이상 지난 실제 사례도 관측되지 않았다.

The saturated plain capture stopped while printing the terminal PID status, after its final zero-drop marker. Saturated traced encountered WSL shared-drive errno 61 while reading the live UART file. The capture controller killed QEMU (status -9), leaving its original zero-byte exceptions log, partial UART and no final RAM sample. Neither first position was rerun. The continuation retained the remaining original positions and tolerated transient read errors. The excess traced exceptions file is also an original zero-byte file.

포화 일반 실행은 UART 종료 상태 출력 도중 수집이 끝났다. 포화 추적 실행은 WSL 공유 드라이브의 UART 읽기 오류 61로 종료되어 첫 원본을 부분 관측으로 보존했다. 두 위치 모두 재실행하지 않았다. 추적 원본에는 길이가 0인 두 파일도 포함한다.

The compact packet contains first-opening counters, UART, sampled progress, available terminal RAM, launch commands and host results. Its SHA-256 is 46521f5c31892af039ea520fcf52631d8af935b302ac484566420cf85be783ed. The raw supplement contains all seven original external exception logs, five standalone application ELFs, seven deployed kernel ELFs, original manifests and build logs, source archive, producer scripts and per-member SHA-256 checksums. Rebuild from the exact repository commit using the compiler and single-job settings in toolchain.txt and build.sh, adapting the machine-specific output paths. The capture scripts preserve the original and continuation behaviors.

기본 패킷에는 카운터, UART, 진행 샘플, 수집한 종료 메모리와 명령이 있다. 원시 보충 패킷에는 외부 추적 로그 전체, 배포한 커널과 앱 ELF, 매니페스트, 빌드 로그, 원본 소스 묶음, 생성 스크립트 및 파일별 SHA-256을 담았다.
