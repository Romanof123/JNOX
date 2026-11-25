# Independent idle delivery cold pair / 독립 유휴 타이머 전달 최초 관측

Candidate 31a418508925e5b4695673796c770ec7ecc0a8ed. First ordinary archived wake/trace control, then first matched observation, each once on ext4 WSL2. Both completed all three exits and HEALTH END. The original threshold is 1,953,125 ticks at 62.5 MHz, with observation spacing bounded by 125,000,000 ticks. Both firsts passed the measured guest checks. Neither recorded a new natural overrun. The supplied offline CLI failed on the authentic symbol table.

원본 대조와 관측을 각 한 번 실행했다. 두 실행 모두 실제 재충전·앱 계산·예외 경로·스케줄러 원장을 검증했다. 새 시간 초과는 없었고, 후보 CLI는 실제 심볼 이름을 처리하지 못했다. 원본 출력과 실패를 함께 보존했다.

|Measure / 측정|Control / 대조|Observer / 관측|
|---|---:|---:|
|Rows / 전체 행|89|91|
|Scored idle refills / 판정 재충전|29|30|
|Late, invalid, dropped / 지연, 무효, 유실|0,0,0|0,0,0|
|Maximum refill lag / 최대 지연 (ticks)|22,179|16,244|
|Maximum refill lag / 최대 지연 (ms)|0.354864|0.259904|
|Maximum decision spacing / 최대 관측 간격 (ticks)|11,733,018|11,722,631|
|QMP rounds / 메모리 표본 회차|80|82|
|External exceptions / 외부 예외|88|90|
|External user ERETs / EL0 복귀|59|60|

The archive.check receipt verifies the upstream manifest. freeze.log records local byte matches for the kernel and all three ELFs, exact QEMU hash, ABI exports and environment. run.sh and resume.sh preserve the original arguments and terminal criteria. Marker construction required the already installed local pyelftools dependency after the stock system interpreter failed to import it. Both captures used the same archived default virtual clock, A72, GICv2, one CPU, exception trace and four running pmemsave requests per round, followed by 90 ms sleep. The observation added the bounded marker and a single setup query plus host sampling. The raw outcome, UART, exceptions, registers, final RAM and samples exist for each first. No extra boots or injected faults.

양쪽 모두 동일한 보관 커널, 세 ELF, QEMU, 기본 클록, 예외 추적, 매 회 네 번의 읽기 및 90 ms 대기를 사용했다. 시스템 파이썬에 없던 ELF 파서를 기존 로컬 경로에서 불러온 최초 준비 오류도 남겼다. 게스트 재실행이나 장애 주입은 없었다.

The original offline result.json exited 2 with “ambiguous/missing 6SPACES”. nm contains exactly one matching Rust symbol at 0x40234000 with its hash and LLVM suffix. replay.py supplies the exact full observed symbol to the unchanged offline analysis on these same files. The separate archived audit.py walks the RAM page tables and replays each schedule/trace decision. Both yielded exits 7/70/179, reporter progress/result 3/7, the nine expected histogram bins, and energy 5,864,240,971,776. Sleep rows 2/6/13 were followed by wakes 3/8/14, with peer dispatch in both runs. All IRQ expiry, decision spacing and per-grant limits passed. The raw ledger and report preserve numeric details. This supplemental offline result does not confer a stock CLI pass.

원본 리더가 해시 접미사가 붙은 실제 Rust 심볼을 찾지 못해 종료 코드 2를 반환했다. 동일한 RAM·UART를 정확한 전체 심볼로 읽고, 별도로 원본 페이지 테이블·스케줄러·예외 경로를 재생했다. 결과값과 수면·기상 행, 할당·시간 판정은 양쪽 모두 일치했다.

The calibration qualified on the same boot, UID and allowed CPUs, using two disposable children on CPU 0. Their runnable trial waits were 74,295,378 and 74,195,991 ns. These are child measurements. The observer recorded main TID 745, vCPU TID 749, 1,530 host samples, zero skipped periods or drops, and 553,138,223 ns sampler CPU. One setup query lasted 377,803 ns. The 120 marker callbacks formed 30 complete four-site joins. Marker pair widths were 1,803–23,966 ns and preflight empty brackets 541–5,561 ns. Resolution was 1 ns. No emitter-timestamped assertion was available in this acquisition. No late guest event was available for post-deadline host attribution, and this pair supplies no observer-cost bound. The historical 4,201,556, 6,487,068 and 9,989,268 tick firsts retain their original failing timing verdict and unresolved causes.

같은 부팅·사용자·허용 CPU에서 자식 프로세스 대기 대조군이 검교정을 통과했다. 관측 실행의 120개 콜백이 30개 재충전에 대응했고 호스트 표본 누락은 없었다. 실행큐 대기 대조 수치는 자식 프로세스 측정값이다. 새 초과가 없어 기한 후 지연을 특정 층에 귀속할 근거가 없고, 두 실행의 차이로 관측 비용 상한을 정할 수 없다. 과거 세 초과의 원인은 미확정이다.
