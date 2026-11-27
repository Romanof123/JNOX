# CHG-339 · Compositor recovery and control continuity / 합성기 복구와 제어 연속성

REQ-015/024 · DES-009/013 · VER-015/024. The five-role AArch64 QEMU image assigns a separate address space to each of the primary compositor, surviving client, standby compositor, socket broker and headless controller. The ordinary four-role image remains a regression target. [Recovery acceptance #449](https://github.com/Romanof123/JNOX/issues/449) · [Parent compositor #446](https://github.com/Romanof123/JNOX/issues/446)

Only the active compositor receives display and input authority. A deliberate synchronous exception in the test profile retires the primary. Close its sockets, revoke its mappings and device grants, discard old input and admit the spare once. The spare binds the same socket path. The surviving client must negotiate a fresh connection, transfer its buffer and produce a new observed frame. Clean primary exit dismisses the spare. A second compositor fault exhausts the replacement policy.

The independent controller has no display, buffer or socket grants. Record 4,096 completions against absolute monotonic deadlines spaced 10 ms apart. The predeclared maximum interval between consecutive completions is 20 ms in this virtual experiment. Check the sequence across the primary fault, resource cleanup, replacement and the first reconstructed frame. Report fault-to-frame latency separately. Collect external pixels, guest address-space and grant state, standard wire records and raw controller timestamps. Repeat clean exit and spare failure with the same kernel, then run the ordinary GUI and a separate headless image. Physical timing and the product safety case require their own evidence.

단일 CPU 가상 머신에서 주 합성기, 재접속할 앱, 예비 합성기, 소켓 브로커, 화면 없는 제어 앱을 독립 프로세스로 실행한다. 주 합성기 고장 시 기존 권한과 연결을 회수하고 예비 합성기를 한 번 기동한다. 기존 앱이 다시 연결해 그린 새 화면을 확인한다. 정상 종료와 두 번째 고장도 각각 시험한다.

제어 앱은 10 ms 절대 주기의 완료 시각 4,096개를 기록한다. 가상 시험의 연속 완료 간격 목표는 최대 20 ms이며, 고장부터 새 화면까지의 시간은 별도로 측정한다. 실제 장치와 제품별 안전 시간 한도는 별도 검증 대상이다.
