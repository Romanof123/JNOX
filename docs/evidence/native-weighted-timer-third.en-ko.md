# Third prospective timer cohort / 세 번째 사전 등록 타이머 관측

One Ubuntu WSL invocation ran plain and instrumented share/default exactly once at ccff112c1cc1084de63fd117bdd5aa009ad3a2d6 against the unchanged #183 inputs. The wrapper, both captures and both official replays exited 0. Preflight imported pyelftools 0.32 under /usr/bin/python3 (3.10.12) in the wrapper and its child. The Linux reader and native Windows PowerShell 5.1.26100.9818 reader verified the fixed D: fixture and both D: archives. Independent Windows Python rehashed each member (19 plain, 20 instrumented), and offline Linux replay of the extracted D: raw sessions reproduced both official JSON reports. An initial offline shell-quoting attempt failed before reading any session. No guest was repeated.

Ubuntu WSL 단일 호출에서 고정된 #183 입력으로 일반·계측 위치를 각각 한 번 실행했다. 래퍼·양쪽 캡처·공식 재생이 모두 0으로 종료됐다. 사전점검은 실제 부모·자식 Python의 ELF 의존성과 D: 고정 검체의 Linux·네이티브 Windows 검증을 통과했다. 독립 Windows Python으로 각 ZIP 멤버를 재해싱하고 D:에서 다시 연 원본을 Linux에서 재생해 공식 JSON과 양쪽 모두 일치했다. 오프라인 재생 명령의 첫 인용 처리 오류는 세션 열기 전에 발생했다. 게스트 재실행은 없었다.

| Position / 위치 | IRQ service min–max (ticks) | Max gap (38 leases) | Prefix 2A−B min–max | A / B at IRQ 36 | T | A/T |
|---|---:|---:|---:|---:|---:|---:|
| Plain / 일반 | 629320–672793 | 50562 | −633217…1269398 | 7578148 / 15208827 | 22786975 | 0.332565 |
| Instrumented / 계측 | 628388–633457 | 43625 | −628762…1264713 | 7570206 / 15127465 | 22697671 | 0.333523 |

Both positions met all 36 R/C/I service bounds, all gaps, all prefixes and both terminal tests under the frozen limits. R is lease start, C=R+625000 is programmed compare, and I is the recorded IRQ stop in the guest counter domain (62.5 MHz). Per-lease values are in independent.json. Plain provided 36 guest triplets and no host trace by design. Instrumentation provided 36 guest triplets and six unambiguous host points per IRQ (CVAL, GIC PPI, CPU IRQ line, active recalculation, handler IAR and disable). Host wall-clock envelopes in microseconds were 10027–10076 CVAL→PPI, 6–52 PPI→CPU line, 27–294 PPI→IAR and 30–301 PPI→disable. Host timestamps include trace overhead, with wall-clock continuity assumed. Callback entry, exact architectural IRQ acceptance and per-lease off-CPU episodes remain unavailable. Aggregate vCPU schedstat across 74 samples recorded 368878287 ns running and 6622 ns queued. Neither position missed a bound. No interval can causally explain older misses across different runs or align the separate host and guest clocks.

양쪽 모두 36개 게스트 R/C/I 서비스 구간, 전체 공백, 모든 누적 편차 및 마지막 두 기준을 충족했다. R은 시작, C는 비교값, I는 IRQ 종료 시점이며 모두 62.5 MHz 게스트 카운터 단위다. 일반 위치는 호스트 추적을 수집하지 않았다. 계측 위치는 각 IRQ에 호스트 지점 6개를 모호성 없이 연결했다. 호스트 벽시계 구간에는 추적 비용이 포함된다. 콜백 진입·정확한 CPU 수락 시점·구간별 비실행 시간은 관측되지 않았다. 두 시간 영역은 별개이며 이번 쌍에는 기준 초과가 없어 이전 실행의 원인을 특정할 수 없다.

#190 demonstrated retention for both first positions in this cohort. #184 has one qualified paired observation under these inputs and environment. The earlier two cohorts remain unqualified and this pair does not establish timing repeatability or a general bound.

#190은 이 집단의 두 최초 위치에서 보존 성공을 확인했다. #184의 이번 쌍은 지정 환경과 입력에서 통과했다. 기존 두 집단은 그대로 미충족이며 반복성이나 일반적인 시간 상한은 확정되지 않았다.

[Original capture, receipts and independent analysis / 원본·보관 영수증·독립 분석](native-weighted-timer-third.zip) · SHA-256 c2a74b931d49eed59d5b22f9b08de3548dbf9559898169e4cd1e0e9498cf01d3.
