# CHG-312 independent first receipt / 독립 1차 검증 기록

Frozen source c3015b4f9316589e045feb73f0fb1d7aa90a78b5, tree 64718e7107df9dd66b38d0afd047b851bb8ef4eb. Public contract https://github.com/Romanof123/JNOX/issues/392. Independent verifier used QEMU virt 6.2.0, one CPU and 62,500,000 counter ticks/s. Frozen worker SHA-256 be0d42e614cb76aa21733f0726a63a4aedab1d23fa4fcfe971b8d54e9f3aba07. All twelve ext4 captures report 71/72/73 and zero private error. Each healthy capture enabled until teardown. Healthy target-to-post-COMPLETE maxima 70,524 FIFO and 85,164 RR ticks exceed the 62,500-tick budget. Timing admission and physical safety qualification remain open.

동일 소스·실행 파일로 FIFO/RR 각각 여섯 사례를 실행했다. 정상 사례는 종료 전까지 출력을 유지했다. 두 정책 모두 정상 사례에서 1 ms 단계 예산을 넘겼다. 실물 안전성 및 응답시간 인수는 미결이다.

| Case / 사례 | FIFO | RR |
|---|---|---|
| Sleep / 절전 | timer decision D+3743 | D+3804 |
| Compute / 무호출 계산 | D=5355137, IRQ entry=5358795, decision=5359102, off-write/readback 5361619–5361628 | D=5324032, IRQ entry=5327429, decision=5331548, off-write/readback 5332586–5332591 |
| Controller exit / 제어기 종료 | Guard reached, enabled→off on death | Same |
| Scheduled recovery / 지정 시각 복구 | issues 6267182, 6892182 | 6214073, 6839073 |
| Live recovery / 실측 시각 복구 | issues 6235027, 6859111 | 6218776, 6841402 |

Both compute IRQs interrupted the loop body at 0x402e84. PPI30 handling observed the elapsed deadline before late COMPLETE returned -110. The scheduled recovery pair passed exact 625000-tick spacing. Both rows remained disabled at sequence 2, followed by separate supervisor generation-2 REARM, pending and enabled observations. Fresh live pairs missed exact spacing and REARM returned -11. The death workload rejected later REARM and OPEN. The deadline in each observed prime/refresh equals L+312501. The suite does not deliver a controller call precisely at the inclusive boundary, or exercise every invalid/competing command or every previous workload. These six cases alone cannot close those claims.

두 정책에서 계산 중 IRQ의 중단 PC가 실제 무호출 루프 본문과 일치했다. 마감 이후 하드웨어 타이머 경로가 차단을 결정했고, 늦은 완료는 -110을 반환했다. 지정 시각 복구에서는 두 번째 입력 승인 직후에도 출력이 꺼져 있었고 감독자의 별도 재무장 뒤에만 다시 켜졌다. 실측 입력은 두 실행 모두 신선했으나 정확한 10 ms 간격을 충족하지 않아 -11로 거부되었다. 정확한 경계 틱에서의 게스트 명령, 전체 경쟁 명령 및 기존 작업 부하 전수 회귀는 이 기록의 미검증 항목이다.

## Firsts / 최초 시도

setup contains the successful worker build and all first packaging failures (git archive lacked .git for git rev-parse HEAD). drvfs contains the next cohort. FIFO controller-exit and FIFO recover produced complete captures. Ten captures ended with host Errno 61 while reading active UART. Their boot prefixes, stderr and manifests are retained. No guest outcome is inferred from an incomplete capture. ext4 contains twelve fresh launches of the same built binaries. Original local raw files were preserved.

setup에는 최초 패키징 오류를 보존했다. drvfs 단계에서 FIFO 종료와 지정 시각 복구만 완전하게 기록됐다. 다른 열 건은 호스트 UART 읽기 오류이며 게스트 장애 판정에 쓰지 않았다. ext4에는 같은 바이너리의 재실행 원본을 담았다.

## Offline replay / 오프라인 재판독

Extract on Linux. Run from the extracted root with Python 3 and llvm-nm on PATH. sha256sum -c SHA256SUMS validates the distributed inputs. Run python3 decode.py ext4 and python3 decode.py drvfs to regenerate first-observations.json and the summary rows from captured UART and RAM. The decoder resolves each kernel SPACES symbol/stride, walks three independent page-table roots and reads worker symbols at frozen ELF addresses. Confirm addresses with llvm-nm -n ext4/fifo-compute-late/user.elf. llvm-objdump -d --disassemble-symbols=wd_compute_loop on that ELF shows the loop. Arithmetic uses integer counter ticks. Kind 13 stores operation, generation, old L/D, decision time, source and new L/D. Qualifier type 3 supplies IAR, CVAL and interrupted PC. Kind 2 records readback transition brackets. Inspect private supervisor rows for commanded generation/status and later independently observed flags. session.json hashes authenticate original capture files. The decoder prints observations and does not turn a successful primary exit into a passing scenario.

압축 해제 뒤 SHA256SUMS와 각 session.json의 해시를 확인한다. 위 Python 명령은 가상 주소가 같아도 세 프로세스의 개별 페이지 테이블로 RAM을 읽는다. IRQ의 IAR·CVAL·중단 PC, 레지스터 전환 구간 및 감독자 사후 관측값을 대조해야 한다. 전체 실행 명령은 scripts 및 packages/run.log에 있다. 호스트 전용 절대 경로는 최초 환경을 기록한 값이다.

[Raw captures and offline replay](native-permit-watchdog-first.tar.gz) (SHA-256 `3a142c08ac384289d0c0f6fcdb573f8cd7cf7bee47b5a079d71703c79a365258`).
