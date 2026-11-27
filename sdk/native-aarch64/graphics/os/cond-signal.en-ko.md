# Process-private XCB condition notifications

The graphics EL0 profile runs one execution context in each address space.
An uncontended XCB signal with zero waiters succeeds and leaves no credit for
a future wait. Processes own separate condition objects. A native guest cannot
demonstrate concurrent pthread waiters with this profile.

Each wait registers a stack node while holding the condition guard and releases
its mutex before another notifier can inspect that registration. Signal marks
one waiting node, broadcast marks all, and repeated signals skip marked nodes.
The monotonic one-millisecond sleep polls the flag. Its expiry and interruption
resume polling. A sleep error initiates mutex reacquisition and cleanup. The
caller must check the predicate again after success. This interface has no
timed-wait entry point or process-shared condition attributes.

From this directory, build the deterministic fixture with
`cc -std=c11 sync.c cond-fixture.c -o cond-fixture && ./cond-fixture`.
Its replacement for the sleep primitive enters a nested wait and distinguishes
one notification from broadcast. It checks empty signals, destruction while
busy, error cleanup, and mutex reacquisition. The native socket caller in
`caller.c` separately exercises an ordinary empty signal with the real adapter.

## 한국어

그래픽 EL0 프로필은 주소 공간마다 실행 문맥 하나를 사용한다. 대기자가 없는
신호는 성공하며 이후 대기에 사용할 신호를 저장하지 않는다. 서로 다른
프로세스는 조건 변수 상태를 공유하지 않는다.

대기자는 조건 변수 잠금 안에서 등록하고 호출자의 뮤텍스를 해제한다. signal은
아직 통지를 받지 않은 대기자 하나를 표시하고 broadcast는 모두 표시한다.
1밀리초 단조 시계 대기가 끝나거나 중단되면 다시 상태를 확인한다. 오류 시에도
뮤텍스를 재획득한 후 등록을 정리한다. 실제 게스트의 단일 문맥 검사는
대기자가 없는 호출을 검증한다. 중첩 대기와 오류 처리는 별도 C 픽스처에서
결정적으로 재현한다. 시간 제한 대기와 프로세스 간 공유는 지원 범위 밖이다.
