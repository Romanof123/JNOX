# Independent review of the native AF_UNIX and shared-file substrate

Source `5cc4884a292fb45844b53e80d4db0766def4ebfe`. The first build failed to link fcntl. The frozen second build contains the corrected SDK wrapper. All observed runs use a single Cortex-A72 vCPU under QEMU TCG and the native fixed-priority FIFO policy.

| Frozen original run | What the independent reader found |
| --- | --- |
| Ordinary poll | The 20 ms call took 20,487,968 ns. The linked poll syscall left PID 0 at journal row 48 and returned to the same user PC and stack from an idle timer interrupt at row 56. The data wait at row 69 returned at broker reply row 87. The server retained byte 0x78. The physical decision table matched 128 UART rows. A further 22 rows were dropped, including the end of the final wait. |
| Six transfer boots | At the first stop, the broker and kernel both held delivered receive-COMMIT token 57 for PID 0, FD 65, socket object 2. Both transaction banks had an empty ring. The caller later recorded −110, FD 98 was still free and escrow token 72 marked the F in the queued bytes `prFD1z`. After installation only `D1z` remained. |
| Four memfd boots | After sender FD 96 and 97 had been reused for a four-byte decoy, the queued right and subsequently receiver FD 98 referred to original backing object 2 with size 307,200 and shrink seal 2. The new descriptor was read-only. Its protected CLOEXEC bit at the installed stop was 0 in both default boots and 1 in both flagged boots. The two ordinary shared-file control boots showed the same transfer and a 0 flag. |
| Mappings, display and cleanup | The producer had 75 writable non-executable pages and the receiver had 75 read-only non-executable aliases of those same physical pages. Neither other role had a user alias. For every boot the first and second raw QMP screendumps matched the shared XRGB backing, and all 76,800 pixels changed. At terminal there were zero live shared fds, escrow entries, mappings or user translations to the protected pool. |
| Altered-data controls | 24 challenge executions across the first boot of each variant rejected modified kernel call words, broker tokens, queued ring bytes, page permissions, physical pixels or terminal rights. The two memfd variants also rejected altered seal state, FD 98 access or CLOEXEC. Each mutation changed only the decoder's private read view. |

The first poll capture has one terminal snapshot. Its source and successful role exits support the per-FD HUP and NVAL checks. The exact HUP revents and same-FD EOF require the distinct follow-up capture. In the 128 retained FIFO rows all three quantum budgets were zero. The broker was selected ahead of a higher *base*-priority peer 56 times. The source donates a blocked caller's priority to the broker. The rows record ready order and selection, while the actual effective priorities are not present. The absolute timeout belongs to the IPC call.

FD 98’s bit was sampled with the installed right. Atomic installation follows the frozen single-CPU kernel’s authenticated INSTALL callback. The timed test cancels on an empty stream before the producer is allowed to send. The retained `prFD1z` message proves the later transfer stayed intact. These results cover native socket, shared-memory and display primitives; the four-role workload uses the project fixture.

The separate observed boot used source `f9db3c1be87db520aaf4357e733105694ecad90c` and the identical broker ELF. Each of three stopped images contained a delivered PID 0 kernel call, the identical pending token and poll word in the broker's PID 0 slot, and a saved caller PC immediately after the actual poll SVC. Both broker banks bound listener FD 64 to socket object 0 and accepted FD 65 to socket object 2.

| Wait | Token | Protected deadline | Per-FD result |
| --- | ---: | --- | --- |
| Finite read wait | 16 | 1,507,544,160 ns | Both revents were zero. The call returned at 1,507,841,392 ns. |
| Readable peer | 23 | Infinite | Only FD 65 returned POLLIN. The server received x and the later snapshot retained that consumed byte in the same accepted socket ring. |
| Peer hangup | 36 | Infinite | FD 65 requested zero events and returned POLLHUP. A one-byte recv on FD 65 returned zero with its destination still equal to '!'. After local close, FD 65 returned POLLNVAL. |

Corrupting the kernel word and broker token separately at each stopped wait rejected six further controls, for 30 executions across both revisions. The observed journal retained 128 rows and dropped 54. Complete transition claims here use the live snapshots, raw broker ring, caller results and sender receipts. A discarded scheduler tail cannot supply exact dispatch timing for the final hangup.

## 독립 검증 범위와 결과

최초 빌드에서 fcntl 링크가 실패했다. SDK 래퍼를 고친 두 번째 소스 `5cc4884`의 게스트 ELF와 정지한 물리 메모리, 직렬 출력, QMP 화면을 대조했다. 실행 환경은 QEMU TCG의 Cortex-A72 단일 가상 CPU와 고정 우선순위 FIFO였다.

* 기본 poll의 20 ms 대기는 20,487,968 ns가 걸렸다. PID 0은 스케줄러 기록 48번에서 poll 시스템 호출로 빠졌고, 유휴 상태의 타이머 인터럽트가 발생한 56번에서 같은 사용자 PC와 스택으로 돌아왔다. 데이터 대기는 브로커 응답으로 끝났으며 수신한 바이트는 x였다. 물리 메모리의 128개 결정 행이 UART와 일치했고, 끝부분 22개 행은 누락됐다. 기본 증거에서 마지막 HUP 결과는 소스와 프로세스 종료 코드로 확인된다.
* 일반 파일 대조군 2회, memfd 기본 수신 2회, CLOEXEC 수신 2회 모두 PID 0의 FD 65 수신 요청에 대해 커널과 브로커가 같은 토큰 57을 보유했다. 당시 스트림은 비어 있었다. 이후 타임아웃 −110과 미설치 상태의 FD 98, 새 토큰 72 및 전체 바이트열 `prFD1z`를 확인했다. FD 설치 후 스트림에는 `D1z`가 남았다.
* 송신자는 원래의 FD 96·97을 4바이트짜리 다른 객체에 재사용했다. memfd 시험 4회에서 커널이 보관한 권리와 수신 FD 98은 모두 크기 307,200바이트, 축소 봉인값 2인 원래 객체를 가리켰다. 수신 FD는 읽기 전용이었다. 설치 직후 CLOEXEC 비트는 기본 수신 2회 모두 0, 플래그 지정 수신 2회 모두 1이었다. 설치의 원자성은 해당 단일 CPU 커널의 INSTALL 콜백 구현으로 확인했다.
* 페이지 테이블에서 동일한 물리 페이지 75개를 송신자는 읽고 쓸 수 있었고 수신자는 읽을 수 있었다. 두 역할 모두 실행은 금지됐고 다른 두 프로세스에는 매핑이 없었다. 매 부팅에서 원시 QMP 화면은 공유 버퍼와 일치했고 두 화면 사이 76,800개 픽셀이 모두 바뀌었다. 종료 후 살아 있는 FD, 전송 대기 권리, 매핑, 보호 영역에 접근할 수 있는 EL0 주소는 모두 0개였다.
* 독립 판독기는 원본을 보존한 채 메모리 판독값을 바꾼 24회 대조 시험을 모두 거부했다. 이 결과는 JNOX 네이티브 AF_UNIX, 공유 파일, 화면 출력 기반 기능에 대한 증거다. 시험 앱은 프로젝트의 네이티브 검증용 앱이다.
* 후속 부팅 `f9db3c1`에서는 기존 브로커를 그대로 사용했다. 유한 대기, 읽기 준비 대기, 연결 종료 대기를 각각 중지해 PID 0의 커널 요청과 브로커 보관 토큰 16·23·36, poll 시스템 호출 직후 사용자 PC를 대조했다. 리스너 FD 64와 연결 FD 65는 서로 다른 소켓 객체였다. 유한 대기는 실제 마감 뒤 0을 반환했고, 이후 FD 65에서만 POLLIN과 POLLHUP이 순서대로 관측됐다. 같은 FD에서 recv는 0을 반환했고 대상 바이트 '!'는 그대로였다. 로컬 close 뒤에는 POLLNVAL이 돌아왔다. 커널 요청 단어와 브로커 토큰을 각 단계에서 바꾼 6회도 모두 거부해 두 리비전의 대조 시험은 총 30회였다. 후속 실행의 스케줄러 기록도 128개 행 이후 54개 행이 누락됐다.

## Reproduction

Extract the zip into a new directory. Run `python -B unpack_ram.py` there to reconstruct the 36 original stopped images, then run `python -B review.py build poll`, followed by `baseline`, `default`, and `cloexec` as the last argument. For the extended poll run `python -B review.py supplement observed`. Each invocation prints one report. The expected reports are in `receipts/`. The sparse reconstruction verifies the complete SHA-256 of each 128 MiB image. Full uncompressed logical extent for the 36 transfer snapshots is 4.5 GiB. On a filesystem without sparse allocation allow that much free space. The poll captures are read directly from gzip.
