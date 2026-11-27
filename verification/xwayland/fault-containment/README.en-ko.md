# Guest application fault containment / 게스트 앱 고장 격리

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. The five-active AArch64 display profile runs the server and its peers in independent address spaces. An attributable, ordinary synchronous fault in one EL0 role must retire that slot and release its resources through the existing exit path. Runnable peers must continue from their own saved frames. [Guest window milestone #448](https://github.com/Romanof123/JNOX/issues/448)

The classifier applies only when the AArch64 lower-EL vector, programmed VBAR, originating EL and active TTBR agree with the fixed scheduler's current owner. Eligible cases are an undefined instruction, an A64 software breakpoint, PC/SP alignment, a lower-EL translation or permission abort, a lower-EL data alignment abort, and a malformed JNOX syscall frame. Faults from a stage-1 table walk, external aborts, ECC, SError, current-EL exceptions and inconsistent ownership keep the existing fail-stop behavior. The kernel never dereferences the faulting instruction, fault address or user stack to retire the process. [Arm exception model](https://developer.arm.com/-/media/Arm%20Developer%20Community/PDF/Learn%20the%20Architecture/Exception%20model.pdf)

The bounded kernel record stores only the slot, exception class and native exit status. The fixed scheduler's terminal report emits the record after all roles exit. Normal IPC, shared descriptor/mapping and signal retirement still govern ownership. A queued transferable right held by a live recipient retains its original ownership rules. Product fault-tolerant time, system safe state, physical-board behavior and independent safety approval need separate evidence.

The guest fixture uses a normal software breakpoint in one throwaway application. The owner fills the four SHM objects, maps and modifies each, then raises BRK. The healthy peer must allocate all four again, observe zeroed initial contents and complete its normal work. The parser requires the fault record exactly once and all five terminal process statuses, including normal survivors. The guest comparison uses role IDs and application statuses.

Run from the checkout with a fresh output path and the same shared lock used by guest graphics builds.

```sh
flock "$JNOX_LAB_ROOT/run.lock" python3 verification/xwayland/fault-containment/guest.py "$NEW_OUTPUT"
```

The package runs a real native socket broker. Slot 1 keeps an open socket, four shared descriptors and four mappings at its breakpoint. Slot 2 must complete its own socket open/close and shared-object reuse. Expected terminal statuses in slot order are 0, -5, 72, 0, 0. The single terminal fault record is slot 1, EC 60 (A64 BRK), status -5. This fixture does not boot the real window server; it isolates ownership and peer continuation.

5개 그래픽 프로세스 전용 구성에서 EL0에서 발생한 일반 동기 예외의 소유자를 스케줄러·페이지 테이블과 대조한다. 해당 역할을 기존 종료 경로로 회수하고 다른 실행 가능한 역할을 재개한다. 커널 예외, 외부 하드웨어 오류, 소유권 불일치는 기존 정지 판정을 유지한다.

시험은 한 앱의 명시적 소프트웨어 중단점, 다른 앱의 공유 객체 4개 재할당과 초기화 상태, 전체 역할의 최종 상태를 요구한다. 이 시험은 AArch64 QEMU에서의 소프트웨어 격리를 평가한다. 제품별 안전 상태·허용 시간·실보드 동작·기능안전 승인은 개별 검증 항목이다.
