# Open issues, pull requests and release gates / 이슈·PR 및 출시 판정

At 2026-11-25 04:47:50 KST (19:47:50 UTC on November 24), PM collected the open GitHub issues endpoint through its final page. The saved 1-page inventory contains 32 issues and 17 PRs. Every item has one next-action team. Every PR and its primary issue cite each other in body prose. The role is the next owner. Product authorities record release decisions in their own issues.

수집 당시 열린 이슈 32건과 PR 17건의 다음 담당과 주 이슈 양방향 링크를 모두 확인했다. 이 표의 기준 소스는 [공개 main](https://github.com/Romanof123/JNOX/commit/be9b1cd61a1815422f9c0febdb767a6932e06bf1)이다. 후속 변경은 [PM 이슈](https://github.com/Romanof123/JNOX/issues/6)에서 추적한다.

| Gate / 판정 | At the capture time / 수집 시점 |
| --- | --- |
| Typed trace / 문서 추적 | 24 requirements · 18 policies · 58 numbered scheduling records · 1,335 unique edges. The four dated paths [EVD-091–094](../finite-scheduling-index.en-ko.md) preserve their first captures. |
| Scheduling and IPC / 출시 | [#21](https://github.com/Romanof123/JNOX/issues/21) 0/18 scheduling families at product release; [#24](https://github.com/Romanof123/JNOX/issues/24) 0/10 IPC family release boxes. IPC bilingual planning is recorded. |
| Product inputs / 제품 입력 | [#1](https://github.com/Romanof123/JNOX/issues/1) item and workload, [#10](https://github.com/Romanof123/JNOX/issues/10) safe state, FTTI and safety authority, [#34](https://github.com/Romanof123/JNOX/issues/34) physical board and calibrated measurements. |
| Functional safety decisions / 기능안전 | [#79](https://github.com/Romanof123/JNOX/issues/79) and [#110](https://github.com/Romanof123/JNOX/issues/110) original failures confirmed; historical predicates and risk dispositions remain open. |
| Held engineering cases / 보류 사례 | [#236](https://github.com/Romanof123/JNOX/pull/236) new chain 2/2, combined legacy controls 2/4; [#238](https://github.com/Romanof123/JNOX/pull/238) retained ordinary 7/20 exceeded 1 ms. Zero new hardware measurements. |

## Open pull requests / 열린 PR

| PR | Primary issue / 주 이슈 | Next owner / 다음 담당 |
| --- | --- | --- |
| [#126](https://github.com/Romanof123/JNOX/pull/126) | [#123](https://github.com/Romanof123/JNOX/issues/123) | team:integration |
| [#133](https://github.com/Romanof123/JNOX/pull/133) | [#99](https://github.com/Romanof123/JNOX/issues/99) | team:integration |
| [#135](https://github.com/Romanof123/JNOX/pull/135) | [#99](https://github.com/Romanof123/JNOX/issues/99) | team:systems |
| [#138](https://github.com/Romanof123/JNOX/pull/138) | [#123](https://github.com/Romanof123/JNOX/issues/123) | team:integration |
| [#153](https://github.com/Romanof123/JNOX/pull/153) | [#123](https://github.com/Romanof123/JNOX/issues/123) | team:integration |
| [#172](https://github.com/Romanof123/JNOX/pull/172) | [#123](https://github.com/Romanof123/JNOX/issues/123) | team:integration |
| [#178](https://github.com/Romanof123/JNOX/pull/178) | [#177](https://github.com/Romanof123/JNOX/issues/177) | team:development |
| [#192](https://github.com/Romanof123/JNOX/pull/192) | [#123](https://github.com/Romanof123/JNOX/issues/123) | team:development |
| [#201](https://github.com/Romanof123/JNOX/pull/201) | [#123](https://github.com/Romanof123/JNOX/issues/123) | team:development |
| [#204](https://github.com/Romanof123/JNOX/pull/204) | [#47](https://github.com/Romanof123/JNOX/issues/47) | team:development |
| [#207](https://github.com/Romanof123/JNOX/pull/207) | [#49](https://github.com/Romanof123/JNOX/issues/49) | team:development |
| [#213](https://github.com/Romanof123/JNOX/pull/213) | [#94](https://github.com/Romanof123/JNOX/issues/94) | team:development |
| [#218](https://github.com/Romanof123/JNOX/pull/218) | [#217](https://github.com/Romanof123/JNOX/issues/217) | team:systems |
| [#219](https://github.com/Romanof123/JNOX/pull/219) | [#216](https://github.com/Romanof123/JNOX/issues/216) | team:systems |
| [#231](https://github.com/Romanof123/JNOX/pull/231) | [#230](https://github.com/Romanof123/JNOX/issues/230) | team:systems |
| [#236](https://github.com/Romanof123/JNOX/pull/236) | [#232](https://github.com/Romanof123/JNOX/issues/232) | team:systems |
| [#238](https://github.com/Romanof123/JNOX/pull/238) | [#233](https://github.com/Romanof123/JNOX/issues/233) | team:systems |

## Open issues / 열린 이슈

| Issue / 이슈 | Title at capture / 수집 당시 제목 | Next owner / 다음 담당 | Primary PRs / 담당 PR |
| --- | --- | --- | --- |
| [#1](https://github.com/Romanof123/JNOX/issues/1) | [M0] 제품 기획 기준선과 첫 안전 사용 사례 승인 / Approve the planning baseline and first safety use case | team:systems | — |
| [#2](https://github.com/Romanof123/JNOX/issues/2) | [M1] Rust 툴체인과 Arm·x86 포팅 기준선 선정 / Select the Rust toolchain and Arm/x86 port baselines | team:systems | — |
| [#3](https://github.com/Romanof123/JNOX/issues/3) | [M2] Linux 드라이버 변경 없는 이관과 장애 격리 / Integrate an unchanged Linux driver and isolate failures | team:integration | — |
| [#5](https://github.com/Romanof123/JNOX/issues/5) | [M2] J-NOX 네이티브 Rust 드라이버 SDK와 첫 드라이버 / Define the native Rust driver SDK and first driver | team:systems | — |
| [#6](https://github.com/Romanof123/JNOX/issues/6) | [M0] 추적 검사와 독립 검증 인계 체계 구축 / Set up trace checks and independent verification handoffs | team:pm | — |
| [#7](https://github.com/Romanof123/JNOX/issues/7) | [M3] 보안 부팅·업데이트와 Linux 신뢰 경계 정의 / Define secure boot, updates and the Linux trust boundary | team:systems | — |
| [#8](https://github.com/Romanof123/JNOX/issues/8) | [M4] 멀티스레드 소켓·스크립트와 POSIX 지원 범위 정의 / Define socket, scripting and POSIX support for multithreaded applications | team:systems | — |
| [#9](https://github.com/Romanof123/JNOX/issues/9) | [M3] 실기기에서 클록·감시·리셋 고장 반응 검증 / Verify clock, monitor and reset fault responses on hardware | team:verification | — |
| [#10](https://github.com/Romanof123/JNOX/issues/10) | [M0] 기능안전팀의 제품 안전 상태·FTTI 결정 / Determine the product safe state and FTTI | team:functional-safety | — |
| [#11](https://github.com/Romanof123/JNOX/issues/11) | [M1/M2] 안전 IPC 허가 채널 구현·검증 / Implement and verify the safety IPC permit channel | team:systems | — |
| [#12](https://github.com/Romanof123/JNOX/issues/12) | [M4] 네이티브 Rust·C 앱과 기존 Linux 바이너리 직접 실행 / Native Rust, native C and prebuilt Linux apps | team:integration | — |
| [#21](https://github.com/Romanof123/JNOX/issues/21) | [M1–M5] 스케줄링 정책별 실제 태스크·보드 승인 / Qualify each scheduling policy on tasks and boards | team:integration | — |
| [#24](https://github.com/Romanof123/JNOX/issues/24) | [CHG-007] 스케줄링 완료 후 IPC 기술별 개발·검증 / IPC implementation after scheduling acceptance | team:pm | — |
| [#34](https://github.com/Romanof123/JNOX/issues/34) | P1 physical bench candidate · Raspberry Pi 4 Model B 1 GB 실보드 준비 | team:integration | — |
| [#41](https://github.com/Romanof123/JNOX/issues/41) | P1 BCM2711 · Physical timer IRQ and restored task context / Pi 4 타이머 선점 후보 | team:verification | — |
| [#47](https://github.com/Romanof123/JNOX/issues/47) | POL-013 · Two-CPU dispatch, ownership and migration / 두 코어 실행과 마이그레이션 | team:development | [#204](https://github.com/Romanof123/JNOX/pull/204) |
| [#49](https://github.com/Romanof123/JNOX/issues/49) | POL-014 · Integrate mutex blocking, transitive donation and immediate ceilings | team:development | [#207](https://github.com/Romanof123/JNOX/pull/207) |
| [#79](https://github.com/Romanof123/JNOX/issues/79) | Retain the first failing proportional-scheduler boot and identify its decision boundary | team:functional-safety | — |
| [#94](https://github.com/Romanof123/JNOX/issues/94) | Schedule and observe the ADR-001 virtual control workload on ordinary RM and DM | team:development | [#213](https://github.com/Romanof123/JNOX/pull/213) |
| [#99](https://github.com/Romanof123/JNOX/issues/99) | Run ordinary Rust partition workloads across major-frame boundaries | team:development | [#133](https://github.com/Romanof123/JNOX/pull/133), [#135](https://github.com/Romanof123/JNOX/pull/135) |
| [#101](https://github.com/Romanof123/JNOX/issues/101) | Qualify bounded aperiodic service with ordinary Rust requests | team:systems | — |
| [#110](https://github.com/Romanof123/JNOX/issues/110) | Identify the first failing condition in the original static-window bench | team:functional-safety | — |
| [#123](https://github.com/Romanof123/JNOX/issues/123) | POL-008 · Admit ordinary Rust reservations and measure neighbor service on QEMU virt | team:development | [#126](https://github.com/Romanof123/JNOX/pull/126), [#138](https://github.com/Romanof123/JNOX/pull/138), [#153](https://github.com/Romanof123/JNOX/pull/153), [#172](https://github.com/Romanof123/JNOX/pull/172), [#192](https://github.com/Romanof123/JNOX/pull/192), [#201](https://github.com/Romanof123/JNOX/pull/201) |
| [#137](https://github.com/Romanof123/JNOX/issues/137) | CAP-013 · Define and qualify application, scheduling and IPC tracing | team:systems | — |
| [#162](https://github.com/Romanof123/JNOX/issues/162) | CHG-077 · Decide the first system service profile / 첫 시스템 서비스 구성 결정 | team:systems | — |
| [#177](https://github.com/Romanof123/JNOX/issues/177) | CHG-088 · Verify weighted scheduling across two native AArch64 processes / 네이티브 프로세스 두 개의 가중 스케줄링 검증 | team:development | [#178](https://github.com/Romanof123/JNOX/pull/178) |
| [#184](https://github.com/Romanof123/JNOX/issues/184) | POL-011 · Resolve default-clock service bounds for two native jobs / 기본 클록 가중 작업 서비스 상한 | team:development | — |
| [#216](https://github.com/Romanof123/JNOX/issues/216) | POL-006 · Schedule two ordinary EL0 processes in named cyclic windows / 두 사용자 프로세스의 순환 시간 배정 | team:systems | [#219](https://github.com/Romanof123/JNOX/pull/219) |
| [#217](https://github.com/Romanof123/JNOX/issues/217) | Restore the outer control guard before a missed-phase terminal SVC / 누락 위상 종료 전 외부 IRQ 가드 복원 | team:systems | [#218](https://github.com/Romanof123/JNOX/pull/218) |
| [#230](https://github.com/Romanof123/JNOX/issues/230) | POL-006 · Bound recurring result collection for native static windows / 네이티브 시간창 결과 수집 경로 한정 | team:systems | [#231](https://github.com/Romanof123/JNOX/pull/231) |
| [#232](https://github.com/Romanof123/JNOX/issues/232) | POL-014 · Run a timed transitive mutex wait through ordinary Rust tasks / 일반 Rust 작업의 연쇄 잠금 대기와 만료 | team:systems | [#236](https://github.com/Romanof123/JNOX/pull/236) |
| [#233](https://github.com/Romanof123/JNOX/issues/233) | POL-006 · Preserve full native frame evidence on the scheduler’s ordinary frame path / 기본 문맥 경로에서 전체 프레임 근거 보존 | team:systems | [#238](https://github.com/Romanof123/JNOX/pull/238) |

## Reproduce the snapshot / 수집본 재검산

Extract the [saved REST records, primary mapping, checker transcript and replay script](pm-open-tracking-20261124T194750Z.zip). From the extracted directory run `python -B replay.py PATH_TO_CHECKOUT` against `be9b1cd61a1815422f9c0febdb767a6932e06bf1`. The verifier checks archived body references, all primary pairs and the numbered graph. The collector followed pagination to the end. Later GitHub edits have their own timestamps.

보관된 공개 응답 레코드 SHA-256 `2dedd67bec6aa4ac1350f7c62dd05af5a57170ad8a465ed5bde4643d7fd5c47f` · ZIP SHA-256 `9d96de9fbd6d5d17cbaab8f3b49f18c381dfe2f18f88090b16a8aefde62783c1`. 압축을 풀고 위 리비전에서 재검산할 수 있다. 표는 수집 시점 기준이므로 이 기록을 올리는 PR은 이후 목록에 나타난다.
