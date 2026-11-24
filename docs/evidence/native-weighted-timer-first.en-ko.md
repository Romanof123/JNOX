# #184 timer observation, first cohort / 타이머 관측 최초 두 실행

Both cold share/default positions reached the UART terminal and the original digest-pinned workload decoder exited 0 for each. The first directories were lost before the durable copy. **Both positions are unqualified for independent workload scoring and timing attribution.** The release ledger must retain these two positions and their limitation.

기본 클록의 지분 작업 두 건 모두 UART 종료 표식을 출력했고, 각 세션 해시로 실행한 원래 판독기의 종료 코드는 0이었다. 보관용 복사 전에 첫 실행 디렉터리가 사라졌다. 두 건 모두 부하 기준의 독립 판정과 지연 원인 분석은 미확인이다.

| Frozen position / 고정 순서 | UART | Replay exit / 재생 종료 코드 | Printed session SHA-256 / 출력된 세션 해시 |
|---|---|---:|---|
| 1 · plain / 기본 | END | 0 | `776fa8f727d2ddc280aa5b70d87838c2b5e93ec50510ae2d2d553c3f40bfbe3a` |
| 2 · instrumented / 계측 | END | 0 | `d9c0963139dc53882a3af4bacd008afced9ff50a7c44899819ceabbfaa1c1576` |

The capture used the #183 binary revision `af952e34cea8fbe0aef5c7b4cb1f945a2884fff5`. The observer and both replays used `58e9350f3d8b63f04786a181dbfe4132dd2031a5`. Both used Ubuntu QEMU 6.2.0, package `1:6.2+dfsg-2ubuntu6.31`, sha256 `0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8`. Preflight matched the two ELF files and C probe bytes from the published ZIP against its SHA256SUMS and original build manifest. The captured console lists all three digests. The preflight encountered two preparation errors before any guest launched. The first run script then launched each position exactly once, in order, and ran both original replays with their separately printed hashes.

WSL Ubuntu ran as uid 1000. The host command completed at 00:13:02 KST on 2026-11-25. The next copy targeted the workspace from `/tmp/jnox186-first-58e9350f/record` and reported a missing source. The system journal records volatile-file setup stopping at 00:13:20 and starting again at 00:13:21. The installed configuration clears `/tmp` at startup. Subsequent checks found neither the source nor a durable copy. The journal contains service events; the available records do not identify the individual unlink operation.

원래의 커널·사용자 ELF와 C 프로브는 공개 #183 ZIP 및 원본 빌드 목록의 해시와 일치했다. QEMU와 도구 버전, 세 입력 해시는 실행 콘솔에 남아 있다. 첫 두 사전 점검 오류는 게스트 실행 전에 발생했다. 2026년 11월 25일 00:13:02 KST에 두 실행과 판독이 끝났다. 복사 시점에 WSL의 원본 경로가 사라졌고, 00:13:20~21에 임시 파일 처리 서비스가 재시작됐다. 특정 파일이 삭제된 개별 기록은 없다.

The transcript contains no RAM, mixed QEMU trace, per-lease rows, exit-tail rows, host scheduling samples or numeric timing verdicts. The original CHG-088 36 closures, prefix and terminal bounds still apply. The two finite SDK exit leases are outside that horizon. All individual guest intervals, host intervals, outliers, queue delays and causal attribution remain unobserved in this surviving receipt. The original #183 plain miss remains in its own historical cohort.

원본 RAM·혼합 로그·구간별 수치·종료 꼬리·호스트 표본이 남지 않아 36개 폐쇄 구간과 누적·최종 지분을 다시 계산할 수 없다. 두 SDK 종료 구간은 지분 계산에서 제외한다. 모든 구간별 지연과 원인 분석은 미관측이다. 기존 #183의 초과 결과는 별도 이력으로 유지한다.

Extract the ZIP and run `python verify.py`. To verify the input bytes again, pass the path to `docs/evidence/native-weighted-irq-second.zip` as its argument. The check authenticates the surviving transcript, public inputs and observed replay exits. The archive also contains the exact first-run scripts and a copy-failure receipt. Re-executing the capture scripts would create a later experiment.

압축을 푼 뒤 `python verify.py`를 실행한다. 공개 #183 ZIP 경로를 인수로 주면 입력 바이트까지 대조한다. 이 검사는 남은 실행 기록을 검증하며, 첫 RAM과 로그의 독립 재생은 불가능하다.
