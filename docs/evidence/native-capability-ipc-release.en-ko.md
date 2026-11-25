# #386 / #387 integrated release verification • 통합 릴리스 검증

Commit 1d017b042e3490ea63436b4604836257397310fd, tree 1dce1ee109fcf445110ece989ffe2af4ef38e95e. All 23 fresh native AArch64 single-core QEMU virt boots reached the intended PID exits. Each manifest identifies this commit and an empty tracked diff. Five capability and three mailbox host tests passed. Sources and build dependencies are identified by the frozen Git tree and manifests.

동결 커밋에서 단일 코어 AArch64 QEMU virt로 23회 부팅했다. 모든 패키지의 프로세스 종료값이 기대값과 일치했고 호스트 시험 8개가 통과했다. 실제 보드와 최악 실행시간은 검증 범위 밖이다.

* Twelve C/Rust capability boots covered transfer, before, pending, active, target and mailbox on FIFO and RR. All private application records and both kernel state tables matched the independently replayed first packet on every one. Exit codes were 71, 72 and 73. The target received samples 1–6, 7, none, 8, none, and 9–13 respectively. The verifier walks each private page table. Receiver handles 20,21,22,23,36 carried rights 1,2,1,1,3. MOVE refusal returned -11 and -24 under distinct pressures. Successful MOVE invalidated 16 and the returned handle was 32. Final handles, notices and target references were zero.
* Two full-mailbox deadline boots returned -11, -110 and -110 for poll, past and future deadlines. For the future 100 ms wait the guest monotonic elapsed times were 100628016 ns FIFO and 100555664 ns RR. The unbounded send completed after space became available. Actual target words were exactly 101–105. One default package observed ENOSYS for each service 53–58.
* Eight regression boots ran both policies for four-process pubsub, named and anonymous pipes, and ordinary TCP/UDP. Pubsub delivered all eight publications to the active reader, preserved samples 1–4 for the stalled reader, recorded two losses, and delivered 8 after re-enrollment. Broker terminal live mask, subscriptions, queues and waits were zero. The named reader recorded 8192 bytes, all 16 records and writer CRCs matched, and the full nonblocking write returned EAGAIN. The anonymous receiver read rainfall, EOF and reverse EPIPE. IPv4 exits were 75,76,0. Both private TCP records retained ECONNRESET, the wire captures contained the reset and UDP reply order was 1,4,3,0.

FIFO·RR 각각에서 핸들 복사·이동, 큐/핸들 포화, 소유자·대상의 종료 순서와 기한 만료를 확인했다. 12개 기본 사례의 전체 사유 메모리 관측값과 시작·종료 커널 표가 최초 패킷 재생 결과와 같았다. 독립적인 기한 시험에서 수신 워드는 정확히 101–105였다. 게시/구독 4개 슬롯, 이름 있는 파이프와 익명 파이프, IPv4 TCP/UDP도 두 정책 모두 정상 종료했다. 원시 RAM, UART, ELF, IPv4 패킷과 호스트 피어 로그를 함께 보존했다.

An initial shell-quoting setup command failed before checkout. The first FIFO IPv4 decoder invocation lacked pyelftools; its traceback remains in native-inet-sockets.log. Both unchanged IPv4 captures subsequently passed the original decoder with pyelftools installed on the Python path and the archive's standalone decoder. Neither error consumed another boot. Hardware and worst-case latency were outside this run. Generation exhaustion and exact event-order boundaries were host tests.

첫 셸 설정 명령이 체크아웃 전에 실패했고, IPv4 FIFO 부팅 직후 최초 해석은 pyelftools 경로 누락으로 실패했다. 같은 캡처에서 경로를 복구하고 FIFO·RR 모두 원래 해석기와 별도 해석기로 검증했다.

## Offline replay / 오프라인 재생

From the extracted packet directory with GNU nm, sha256sum and Python 3:

    bash replay-offline.sh

Optional original comparison: extract the first delivery (SHA-256 0522dd0262c485f2a8665a748fedbb8080ac8c3e56da8c7353b86fc2f5af86ef), run its documented checksum and JSON replay, then run python3 compare_first.py FIRST/replay.json observations.json. For live builds create an empty evidence directory inside a clean frozen checkout, copy the three supplemental C inputs and replay.sh, extra.sh, regress.py into it, and run bash evidence/replay.sh, bash evidence/extra.sh and python3 evidence/regress.py with the documented toolchain and pyelftools available. The three native-*.sh files are the exact generated regression commands from this run.

[Raw captures and offline replay](native-capability-ipc-release.tar.gz) (SHA-256 `0b515784b6f4de0e24f6788d4eb7d7abbad86b01b419a347493e3213c709c8c6`).
