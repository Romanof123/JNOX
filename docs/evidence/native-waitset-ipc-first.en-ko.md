# IPC-004 · First native waitset verification / 네이티브 대기 집합 최초 검증

**Result / 결과.** All 16 cold AArch64 QEMU boots returned the expected pair of exit codes. The original FIFO and RR applications each blocked twice until the peer sent matching event bits. A third wait expired after an idle timer interrupt. The three candidate model tests and the separate host controls passed. / AArch64 QEMU 콜드 부팅 16회 모두 두 프로세스가 예상 코드로 종료했다. 최초 FIFO·RR 앱에서 수신자는 두 번 실제로 차단되었고 상대 앱이 일치 비트를 설정할 때마다 재개됐다. 세 번째 대기는 유휴 상태의 타이머 인터럽트로 만료됐다. 후보 자체 모델 테스트 세 건과 별도 호스트 대조도 통과했다.

The frozen contract is issue #373 and `docs/native-waitset-ipc.en-ko.md` at `16e989077dd27e34f37f6377978c04245b778570`. The clean executable source is `d33b49ee0d3daac51af70d90b4c4a2e4b5dc573c`, tree `13778a42bb5d9f11072bcef5e9ec2d2336d17e25`. The packet records both sources. Its Git bundle reconstructs the exact executable source from public prerequisite `04d699c8a445fc837a423c96ba35825810550c4b`. / 규약과 실행 소스는 서로 다른 위 커밋에서 고정했다. 패킷에는 원래 이슈·규약, 정확한 후보를 복원하는 Git 번들, 실행 입력 파일별 해시가 있다.

## Original two-ELF workload / 최초 C·Rust 작업

Slot 0 ran the unmodified Rust receiver at priority 7. Slot 1 ran the unmodified C sender at priority 1. Both policies used the same two ELF inputs, mailbox and notification route 1→0, and `--waitset`. RR quantum was 625,000 counter ticks. The emulator was WSL Ubuntu's stock QEMU 6.2.0, Virt with GICv2, one Cortex-A72, TCG, 128 MiB. / 두 정책 모두 같은 Rust 수신 ELF와 C 송신 ELF를 사용했다. 우선순위는 각각 7·1이고 우편함·알림 경로는 1→0이다. RR 퀀텀은 625,000틱이다. WSL 우분투의 기본 QEMU로 가상 단일 코어를 실행했다.

| Policy / 정책 | Exits 0/1 / 종료 | First block → wake / 첫 차단·깨움 | Replacement block → wake / 재등록 후 차단·깨움 | Timed block → compare → IRQ / 기한 대기·비교값·IRQ |
| --- | --- | --- | --- | --- |
| FIFO | 71 / 72 | 576029 → 613374 | 640718 → 641514 | 664468 → 1901113 → 1908857 |
| RR | 71 / 72 | 586833 → 626771 | 656623 → 657433 | 683248 → 1919386 → 1927610 |

The ticks come from the raw fixed-scheduler journal. The first wait handed control to the sender. Its mailbox send, pulse send and event bit 4 kept it running. The set of bit 1 restored the receiver's interrupted PC and SP. Later the set of bit 8 restored the second wait. At the finite wait the sender exited, the CPU went idle, the programmed compare remained in effect, and the next captured timer IRQ restored the third wait. The three resume PCs were 0x40009c, 0x4003d0 and 0x4005ac under both policies. Both fixed journals reported zero dropped records. / 실제 저널에서 우편함·펄스 송신과 마스크 밖 비트 4 설정 후에도 송신자가 계속 실행됐다. 비트 1과 8 설정은 각각 해당 수신 대기의 저장된 PC·SP를 복원했다. 유한 대기 중 송신자가 종료한 뒤 CPU는 유휴 상태가 되었고 비교 시각 이후의 타이머 인터럽트에서 재개됐다. 양쪽 저널 유실은 0건이다.

| Private process observation / 프로세스별 메모리 관찰 | FIFO and RR / 두 정책 공통 |
| --- | --- |
| Current handles in acquisition order / 취득 순서 핸들 | 16, 36, 56, 76, 92 |
| (handle, ready) / (핸들, 준비값) | (16,1), (36,1), (36,1), (76,1), (92,8) |
| Receiver errors / 수신 오류 | −13 foreign owner, −116 stale handle, −11 after reuse, −11 final poll, −110 finite timeout |
| Sender checks / 송신 검사 | Foreign attach −13, all five transmissions 0 |
| App data / 실제 소비 데이터 | Mailbox 0x000000010000012c, pulse code 17 and word 0x0000000200000200, sum 812 |

The message was queued before registration. Two polls returned handle 36 before the single underlying receive. The app checked the immediate post-consume poll. All four slots were occupied before detaching the ready pulse. Handle 92 used the only free slot while that pulse and bit 4 still existed. The stale handle was refused, and the first replacement poll was empty. The app then consumed matching bit 8, separately retrieved bit 4 and drained the original pulse. / 등록 전에 큐에 들어간 메시지는 두 번의 준비 조회 뒤 한 번 수신됐다. 네 슬롯을 채운 뒤 준비된 펄스만 등록 해제했으므로 새 이벤트는 그 슬롯을 재사용했다. 펄스와 비트 4가 남은 상태에서 옛 핸들은 거부됐고 첫 조회는 비어 있었다. 이후 비트 8과 비트 4, 원래 펄스를 각각 실제로 소비했다.

## Additional guest controls / 추가 게스트 대조

| Inputs and policies / 입력·정책 | Boots / 횟수 | Paired exits / 종료 | Observation / 관찰 |
| --- | ---: | --- | --- |
| Supplemental peer probe, FIFO and RR / 추가 송수신 | 2 | 73/74 | Owner passes live handle to peer by mailbox. Peer detach and foreign attach each return −13. Two pulse reads precede consume; registered pulse then reports empty. Event 0x30 triggers mask 0x1c with value 0x10 twice; consuming the match leaves 0x20. / 전달받은 실제 타인 핸들 해제와 등록은 거부됐다. 펄스·마스크 이벤트를 두 번 조회하고 소비 후 다시 비었음을 확인했다. |
| Waitset + mailbox alone, waitset + notifications alone, both families with waitset disabled, FIFO / 구성 조합 | 3 | 73/74 | Unavailable sources −95, disabled SVCs −38, enabled empty endpoints −11. Disabled waitset attach/detach/wait each −38. / 미지원 소스·비활성 서비스·활성 빈 큐를 구분했다. |
| Original synchronous call apps, standalone FIFO/RR and all families + waitset FIFO / 동기 호출 | 3 | 41/42 | Four readings → 0x000404010101082e; invalid/valid/reused reply statuses −116, 0, −116. / 원본 요청·응답과 토큰 상태를 확인했다. |
| Original mailbox apps, same three configurations / 우편함 | 3 | 51/52 | Send statuses 0,0,0,0,−11,−110,0; five words in order, sum 2182, empty −11. / 순서·타임아웃·합계를 확인했다. |
| Original notification apps, same three configurations / 알림 | 3 | 61/62 | Pulse channel sums 812/1282; event results 1,−11,12,−11. / 채널별 합계와 선택적 이벤트 소비를 확인했다. |

Each regression used the C and Rust applications documented for that family. The all-families configurations used the same compiled inputs as their standalone equivalents. Two deliberately invalid packages (missing endpoint family, repeated `--waitset`) both returned parser status 2 with the specific expected diagnostics. / 각 회귀 검사는 해당 기능의 문서에 지정된 원래 앱 조합을 사용했고, 혼합 구성도 같은 ELF를 재사용했다. 엔드포인트 누락과 옵션 중복 입력은 각각 지정된 진단으로 거부됐다.

## Host scope / 호스트 검증 범위

The candidate tests exercised four registrations, overflow refusal, rotating ready sources, malformed masks, generation exhaustion, live-handle ownership, stale generations, an equal deadline and cleanup after an undelivered result. The independent host program also killed the owner while blocked and after completion before taking the result. Both times the deadline, registrations and pending result cleared, the dead owner refused service, the restarted owner rejected the old handle, and the other process retained its registration. Two orderings of readiness versus expiry at the same timestamp yielded −110 with empty output words. / 모델에서는 4개 한도와 순환, 잘못된 마스크, 세대 고갈과 소유권을 검증했다. 새 대조는 차단 중·결과 인출 전 두 경우 모두 소유자 종료·재시작과 상대 프로세스 상태 보존을 검사했다. 같은 시각의 준비·만료 호출 순서를 바꾸어도 모두 −110과 빈 결과를 반환했다. 정확한 동시각과 종료 후 슬롯 재시작은 호스트 모델에서 관찰했다.

## Portable replay / 패킷 재검증

From the extracted directory, `sha256sum -c SHA256SUMS` checks its contents. `python3 verify.py . > replay.json` uses GNU `aarch64-linux-gnu-nm` to locate the two independent process roots, walks each L1/L2/L3 page table in captured raw RAM, and checks both slots against the ELF symbols, the original capture hash inventory, and UART. `cmp replay.json results.json` compares the decoded values and per-symbol addresses. Each original capture includes raw full linked RAM, UART, launch, session, stderr and kernel ELF. `pkg-*/` holds manifests, build logs and both application ELFs. The capture utility called its copy of the slot-0 source `probe.c` even for the Rust first application; its recorded digest matches `examples/native-waitset/receiver/src/main.rs`. / 압축 해제 후 위 명령으로 모든 원본 해시·양쪽 사설 주소 공간·저널을 다시 확인할 수 있다. 원본 RAM에서 각 L1/L2/L3 테이블을 따라가므로 같은 가상 주소를 쓰는 프로세스의 값을 혼동하지 않는다. 각 캡처의 `probe.c`는 도구가 부여한 이름이며 최초 작업의 내용은 Rust 수신 소스다.

In a checkout containing the public prerequisite, run `git fetch /absolute/path/to/candidate.bundle HEAD`, then `git rev-parse FETCH_HEAD 'FETCH_HEAD^{tree}'`. The outputs must match the candidate and tree above. `run.sh` and `extra.sh` preserve the actual build/capture invocations and `commands.sh` preserves their expanded arguments. Their original checkout, evidence, and Python dependency paths were local to this WSL machine. The packet includes `source-SHA256SUMS` for tracked build inputs. / 공개 선행 커밋이 있는 저장소에서는 번들의 HEAD를 가져와 후보 커밋·트리와 비교한다. 재실행 스크립트의 경로는 검증용 WSL 환경 기준이며, 확장된 인자와 빌드 입력 해시를 함께 보존했다.

This record covers single-core virtual FIFO/RR execution at the specified priorities, the listed package combinations, and the scoped host model. The two-priority RR run exercises that policy's ordinary syscall paths. Equal-priority quantum rotation, physical hardware and new protection fault injection fall outside these measurements. / 이 기록은 명시된 우선순위의 단일 가상 코어 FIFO·RR 실행과 구성 조합, 해당 호스트 모델에 한정한다. 동일 우선순위 RR 회전, 실기기 동작, 새로운 메모리 보호 폴트 주입은 측정 범위 밖이다.
