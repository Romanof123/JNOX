# AF_UNIX poll / AF_UNIX 준비 상태 대기

## Design / 설계

Socket readiness belongs to the EL0 broker that owns the receive queues, links and listener backlog. One authenticated call registers all four local slots simultaneously and sleeps on its one-shot reply. The broker rechecks outstanding calls after accepted operations and owner death. A finite timeout uses the kernel monotonic IPC deadline; an expired call cannot publish a socket mutation. Mailbox, pulse and event waitsets observe different sources. 소켓 큐를 소유한 브로커가 네 슬롯을 한 번에 검사한다. 준비된 슬롯이 없으면 커널의 단일 응답 토큰을 보관하고 요청 처리나 종료 뒤 다시 확인한다. 시간 제한은 커널 단조 시계를 사용한다.

## Implementation / 구현

The SDK merges duplicate entries for the broker, then projects the readiness back onto each original entry. Negative descriptors are ignored. Missing local socket slots yield POLLNVAL. A pending connection gives the listener POLLIN. Queued bytes or records and stream EOF give POLLIN. POLLOUT requires an open writer, open compatible receiver and available space. Peer disappearance reports POLLHUP irrespective of requested events. Poll reads are nonconsuming. Each process can issue one socket SDK operation at a time. The supported descriptor space is the AF_UNIX socket service; other native descriptor classes require separate readiness providers. An unconnected datagram has no default write destination. Duplicate entries count separately. 소켓 상태를 조회해도 데이터를 소비하지 않는다. 다중 FD, 중복 항목, 음수 FD, 종료, 무효 슬롯 및 시간 제한을 지원한다. 일반 파일·파이프·장치의 준비 상태는 이 구현 범위 밖이다.

## Acceptance procedure / 검증 절차

From the repository root, build two application slots from the same ordinary C source and the resident Rust broker. The server binds and listens, polls the accept queue, accepts a second descriptor and checks a 20 ms timeout while the client waits for a byte. A real reply then wakes the data wait. Closing the remote endpoint produces HUP and closing the local descriptor produces NVAL. Expected PID exits are 71, 72, 0. The server stores the two monotonic timestamps and the received byte in poll_probe. 서버는 두 실제 FD를 함께 대기하고 20 ms 만료, 입력·출력 준비, 상대 종료 및 로컬 FD 종료를 검사한다. 게스트 실행 전까지 이 절차는 검증 계획이다.

~~~sh
sh sdk/native-aarch64/build.sh --local-socket examples/native-fd-poll/probe.c poll.elf
sh sdk/native-aarch64/build-rust.sh services/socket/Cargo.toml broker.elf
python3 tools/build_native_el0.py --board virt --policy fifo --out native-poll \
  --elf poll.elf --arg server --priority 7 --peer-elf poll.elf --peer-arg client --peer-priority 4 \
  --peer-elf broker.elf --peer2-arg 3 --peer-priority 1 --socket-service 2 --ipc-route 0:2 --ipc-route 1:2
~~~
