# #336 host boundary observations / 호스트 경계 관측

Candidate 9aef21d5e4e60f3282c309864f559e30f65dce57. First public rustc build and test invocation on private WSL ext4 both exited 0. All nine tests passed: two new decision-point probes, one revised RR/PI case, three retained boundary cases, and three imported ready-queue tests. Rustc 1.95.0 emitted 13 unused-code warnings. Runtime stderr was empty. A preliminary lookup for a previous temporary binary found it absent and ended before this candidate's compiler ran. The previously sealed receipt c2c5626f remains separate.

| Host sample | Directly observed |
|---|---|
| D=150, immediately after advance and before owner unlock or dispatch | HIGH returned ETIMEDOUT (110) once while OWNER still owned the lock and HIGH had left its queue. After unlock the lock had no owner and the completion read was empty. |
| D−1=149, before and immediately after unlock | HIGH had no pending completion before unlock. Before dispatch, it received direct ownership by incarnation and completion 0. After dispatch and advance to D, HIGH still owned the lock and no second completion appeared. |
| RR donation and withdrawal | Owner rank rose 2→4 with 70 ticks left, fell to 2 with 50 left. HIGH's timeout was consumed once and read empty a second time before dispatch. Returning at 270, OWNER retained at 319 and rotated at 320. Its service totaled 30+20+49+1=100. |
| Retained non-drained D−1, D, D+1 | Outcomes of 0, 110, 110 respectively survived unlock/dispatch and subsequent advance until first read. Each second read was empty. All three checked stale-token refusal after same-address reinitialization. D+1's direct completion read remains deferred in this retained case. |

최초 컴파일과 실행이 모두 성공했고 총 9건이 통과했다. 새 검사 2건은 D에서 기존 소유자가 잠금을 풀기 전에 만료값 110을, D−1에서는 인계 직후 디스패치 전에 성공값 0을 직접 읽었다. 후속 해제 또는 시간 진행 뒤 중복 완료는 없었다. 수정한 RR 사례에서는 기부·철회 후 70틱과 50틱이 유지됐고 철회 시 디스패치 전에 완료가 한 번만 전달됐다. 기존 비소모 대조 3건은 이후까지 결과가 보존됨을 확인했다. 이 결과는 고정 카운터로 실행한 호스트 정책 검사이며, 기존 게스트 기록에 새 IRQ·타이밍 관측을 추가하지 않는다.
