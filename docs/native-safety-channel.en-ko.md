# Protected virtual permit / 보호된 가상 허가 채널

**Prospective CPU-only bench contract · CHG-311 · IPC-010.** Scope is one Cortex-A72 on QEMU virt and a simulation-only ENABLE register. REQ-019 → DES-010 → VER-019 and REQ-024 → VER-024 apply. ADR-001 supplies all timing and recovery values. Earlier symbolic scheduling-admission rejection stays recorded; this design neither admits that workload nor supplies missing WCET inputs. The declared native tasks must pass the applicable admission procedure on their real configuration before any timing qualification. [ADR-001](safety-parameters.en.md) · [DES-010](architecture.en.md#des-010-safety-ipc-channel) · [Channel #11](https://github.com/Romanof123/JNOX/issues/11) · [Product parameters #10](https://github.com/Romanof123/JNOX/issues/10)

단일 Cortex-A72 QEMU virt와 시뮬레이션 전용 ENABLE 레지스터를 대상으로 하는 설계다. 시간·복구 값은 ADR-001을 따른다. 이전 기호 입력의 스케줄링 승인 거부를 유지한다. 실제 구성에 적용할 승인 절차와 WCET 근거를 별도로 충족해야 한다.

## Endpoints and transport / 종단과 전송

Slot 0 is the synthetic producer, slot 1 the receiver/controller and slot 2 the supervisor/injector. Use the existing CPU-only shared-buffer loan route 0→1: three 4096-byte backing pages, a two-entry SPSC FIFO, one complete protected frame per published loan. On full, publish returns EAGAIN immediately and preserves queued entries and the outstanding writer loan. A retry must retain the original issue time or discard the uncommitted loan; retry cannot refresh data. Only the kernel-authorized producer writes its current loan; the consumer receives its read-only mapping. Bind route, authenticated endpoints, channel 1, data ID 0x0101 and permit type 1 at both ends. The loan handle is transport metadata, never wire authority. [Loan contract](native-loan-ipc.en-ko.md)

Use a separate kernel-authenticated supervisor command path. Only slot 2 may open a generation or request rearm for that generation. Encode OPEN and REARM as distinct commands carrying a nonzero u64 generation; take issuer from kernel dispatch, never a caller-supplied PID. OPEN accepts exactly the next generation, starting at 1; it inhibits, invalidates the prior receive context and starts recovery without enabling. Reject duplicate/replayed/wrong-generation commands. REARM is one-shot and is discarded if premature. Only the control path may update the kernel-owned virtual device; only the injector may atomically close the permit delivery gate. Record gate closure and invalidate queued undelivered frames at that boundary, preserve the last previously delivered issue time and reject further transmissions through the closed gate. Disconnect/restart invalidates old transport handles and queued inputs; reopen requires a fresh generation. Mere availability of #374 loans does not implement these control hooks.

슬롯 0은 합성 송신자, 1은 수신·제어기, 2는 감독·결함 주입자다. 0→1 대여 경로의 4 KiB 페이지 3개와 SPSC 큐 2칸을 사용한다. 게시 한 건에 프레임 하나를 넣는다. 포화 시 즉시 EAGAIN이며 기존 큐와 미게시 쓰기 대여는 보존한다. 재시도 시 원래 발행 시각을 유지한다. 세대 개방과 재무장은 별도 명령이며 커널이 확인한 슬롯 2만 요청할 수 있다. OPEN은 다음 세대만 열고 출력을 차단한다. 이른 REARM은 보관하지 않는다. 차단 게이트 폐쇄 시 아직 전달되지 않은 프레임을 원자적으로 무효화하고 이전에 전달된 허가의 발행 시각을 보존한다. 재시작·단절 후 이전 핸들과 큐를 무효화하고 새 세대에서 복구한다.

## Wire format / 프레임 형식

Exactly 44 bytes; unsigned multibyte fields are little-endian. No native structure padding. Offsets and sizes are bytes. / 총 44바이트이며 다중 바이트 부호 없는 정수는 리틀엔디언이다. 구조체 패딩 없이 다음 오프셋을 사용한다.

| Offset | Size | Meaning / 의미 |
|---:|---:|---|
| 0 | 1 | Version = 1 / 버전 |
| 1 | 1 | Permit type = 1 / 허가 유형 |
| 2 | 2 | Total length = 44 / 전체 길이 |
| 4 | 2 | Channel = 1 / 채널 |
| 6 | 2 | Data ID = 0x0101 / 데이터 ID |
| 8 | 8 | Generation, initially 1 / 최초 세대 1 |
| 16 | 8 | Sequence, initially 1 per generation / 세대별 최초 순번 1 |
| 24 | 8 | Issue time in common guest monotonic counter ticks / 공통 게스트 단조 카운터 발행 틱 |
| 32 | 4 | Validity interval in milliseconds, must equal 20 / 유효 기간은 반드시 20 ms |
| 36 | 1 | Permit 0 or 1 / 허가 비트 |
| 37 | 3 | Reserved zero, included in CRC / 예약 영역 0, CRC 포함 |
| 40 | 4 | CRC over bytes 0–39 / 앞 40바이트의 CRC |

CRC-32/ISO-HDLC parameters are width 32, polynomial 0x04C11DB7, init 0xFFFFFFFF, RefIn/RefOut true, xorout 0xFFFFFFFF. Reflected update polynomial 0xEDB88320; no extra zero bytes. ASCII 123456789 gives 0xCBF43926. The complete frame for generation 1, sequence 1, issue tick 0 and permit 1 follows. This is a serialization vector; runtime freshness depends on the observed clock.

~~~text
01 01 2c 00 01 00 01 01  01 00 00 00 00 00 00 00
01 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00
14 00 00 00 01 00 00 00  70 6e 56 e3
~~~

An independent reference expression is `zlib.crc32(frame[:40]).to_bytes(4, "little") == frame[40:44]`; the CRC is 0xE3566E70. CRC checks accidental corruption. Sender identity comes from endpoint authority. A malicious source, plausible bad input or a keyed gateway authentication protocol requires separate analysis. / 위 벡터의 CRC는 0xE3566E70이다. CRC는 우발적 손상을 검사하며 송신자 신원은 커널 종단 권한으로 확인한다. 악의적 송신자나 그럴듯한 오입력의 잔여 위험은 별도로 검토한다.

## Receiver and timing / 수신·시간 판정

Start inhibited, ENABLE=0. On every nominal 5 ms control release perform a freshness decision even with no message or a permanently full queue. First evaluate the last accepted permit at the current tick. Age ≥20 ms latches the fault; equality wins over an arriving valid frame or REARM. Handle at most two nonblocking frame attempts and one supervisor command per activation. No drain-until-empty loop or wait on the sender. Check freshness again against actual commit time before changing valid state or enabling. Measure release-to-completion ≤5 ms **and** actual time between completed freshness checks ≤5 ms including jitter. This work bound needs measurement; it is not a timing result.

Check endpoint and transport-slot bounds and require published length 44 before copying exactly 44 bytes into private receiver memory. Release the loan and check only the snapshot: CRC, version/type, lengths, IDs, reserved bytes, permit range, current generation, next consecutive sequence, monotonically ordered issue time, no future issue and age strictly below 20 ms. Compare ticks using the recorded frequency and checked wide arithmetic; reject wrap, ambiguous time or exhausted generation/sequence instead of reusing identities. Commit the last valid value, issue time, sequence and recovery count together only after every check. Transport faults and rejected data leave all four unchanged; an independent expiry may still latch inhibition. Valid permit 0 withdraws permission and inhibits. Startup/recovery requires a fresh OPEN, **two consecutive set permits** from that generation issued at the nominal 10 ms spacing, and a separate REARM while both are individually fresh. No automatic or saved rearm.

부팅 시 ENABLE=0이다. 5 ms마다 큐의 빈 상태·포화 여부와 관계없이 먼저 마지막 유효 허가의 신선도를 판정한다. 나이 ≥20 ms이면 고장을 래치하며 같은 틱의 도착·재무장보다 만료가 우선한다. 한 활성화에 비차단 프레임 최대 2개와 감독 명령 최대 1개만 처리한다. 유효 상태 반영이나 출력 허용 직전 실제 시각으로 신선도를 다시 검사한다. 릴리스부터 완료까지 ≤5 ms, 실제 신선도 검사 완료 간격도 지터 포함 ≤5 ms인지 각각 측정한다. 종단·슬롯·길이를 검사해 사설 메모리에 44바이트만 복사한 뒤 CRC·형식·ID·세대·정확히 다음 순번·발행 시각·유효 기간·허가 값을 확인한다. 거부 시 마지막 유효 값·발행 시각·순번·복구 횟수는 변하지 않는다. 허가 0은 출력을 차단한다. 복구에는 새 세대에서 발행 시각 간격 10 ms인 연속 허가 1 두 건과 둘 다 신선할 때의 별도 REARM이 필요하다.

## Independent evidence / 독립 검증

Establish ENABLE=1 via the full startup sequence. Record t_fault at atomic injection/gate closure, t_detect at the controller fault latch, and t_safe at the first independently observed virtual-device ENABLE transition 1→0. Preserve source images, raw frame/queue and register evidence, monotonic frequency, resolution, event order and measurement uncertainty. Use a device transition observer independent of the controller's request; polling or a pre-write timestamp alone cannot identify an exact transition. Exclude ambiguous boundaries or missing events from passing evidence. Enforce t_fault ≤ t_detect ≤ t_safe, detection ≤25 ms, inhibition after detection ≤5 ms and total ≤30 ms separately. Score a healthy run for premature inhibition. Use independent initial states for silence, continuous invalid input and queue saturation; verify loss immediately after delivery, no late queued permit, a check one tick before expiry followed 5 ms later, and expiry concurrent with the next valid sequence at 20 ms after skipping the 10 ms issue. For invalid-traffic and saturation, timestamp the defined injector transition and independently show the last valid issue; account for the producer’s failed publications and queue occupancy. Preserve outputs until a legitimate rearm.

Check corruption, wrong route/data ID, future or expired frames, gaps, duplicates, reordered or prior-generation frames, restart and overflow. Compare receiver valid state before and after each rejection. Verify each omitted recovery condition keeps ENABLE=0. With set permits issued at 0 and 10 ms, REARM at 15 ms enables; in an independent run a first REARM at 20 ms fails. A command before permit two requires another command. Verdict controls must pass at each allowed time limit and fail one tick beyond. Separate the collision with the delivery gate open from the loss injection with the gate closed. Optional Linux relay evidence would require the same protected bytes forwarded unchanged and the same independent receiver timer through relay silence/failure; native results alone cannot qualify a gateway.

독립 관측은 주입·게이트 폐쇄 시각, 제어기 고장 래치 시각, 가상 장치에서 실제 ENABLE 1→0이 된 시각을 공통 단조 틱으로 따로 보존한다. 불확실해 경계를 구분할 수 없는 결과는 통과로 처리하지 않는다. 검출 ≤25 ms, 이후 차단 ≤5 ms, 전체 ≤30 ms와 사건 순서를 각각 검사한다. 정상 운전 중 조기 차단 여부, 무송신·연속 손상·포화는 독립 실행으로 측정한다. 만료 한 틱 전 검사와 5 ms 뒤 검사, 20 ms 동시 도착에서 만료 우선, 손상·오배달·중복·세대·재시작 거부를 확인한다. 0·10 ms 허가 후 15 ms 재무장은 허용하고 별도 실행에서 최초 20 ms 재무장은 거부한다. 조건을 하나씩 제외하고 각 한계값은 통과, 한 틱 초과는 실패하는지 확인한다. Linux 중계는 별도 검증이 필요하다. 물리 장치 반응, 제품 FTTI·ASIL, 고장난 시간원 및 실제 액추에이터의 격리는 이 가상 판정 범위 밖이다.

The bench provides no physical actuator, physical FTTI, ASIL or item-level approval. The Functional Safety Team and accountable product integrator retain the separate item/HARA, board/HIL, independence, residual-error and approval work under #10.

## First native verification / 첫 네이티브 검증

**IMP-311 · EVD-311 · HOLD.** A Rust 2024 identifier prevented the first protected image from building. After correction the independent verifier booted all 32 FIFO/RR cases. Each of the 544 release-to-completion measurements fit within 5 ms. Consecutive completed checks exceeded 5 ms in every boot. Both collision supervisors observed ENABLE low at 18 ms and 37 ms and exited 95. Ten marked fault runs met the measured 25/5/30 ms limits. The regular workload and exact boundary experiments are tracked in #389. [Original](evidence/native-safety-channel-first.en-ko.md) · [Corrected runs](evidence/native-safety-channel-corrected.en-ko.md) · [Native timing plan](native-safety-admission.en-ko.md)

최초 보호 이미지 빌드 실패와 수정본의 32회 실행을 보존했다. 단일 작업의 완료 시간은 544회 모두 기준을 충족했지만 연속 검사 사이의 간격은 모든 부팅에서 초과했다. 두 만료 충돌 사례에서도 기대한 출력 전환을 관측하지 못해 안전 채널을 승인 보류했다. [원본과 측정값](evidence/native-safety-channel-corrected.en-ko.md) · [후속 검증](https://github.com/Romanof123/JNOX/issues/389)

## Closed-gate fault inputs / 송신 차단 이후 결함 입력

VER-019 · IMP-311 · EVD-311. Historical invalid-traffic and saturation cases left the normal producer publishing after MARK. The silence cases used CLOSE. The two new virtual fault configurations use CLOSE to revoke grants, invalidate queued permits and deny all subsequent normal producer attempts. [Historical fault audit](evidence/native-safety-fault-audit.en-ko.md) · [Acceptance issue](https://github.com/Romanof123/JNOX/issues/11).

For `gate-invalid` and `gate-saturation` the normal package tool accepts `--safety-downstream-injector` with `--safety-qualification` on the protected three-role virt FIFO/RR profile. All three role arguments must name the same case. The controller can additionally select `fused-phase`. Only slot 2 can request the immutable 44-byte frame for the current closed generation through SVC 59, command 5. Its checksum differs by one bit. The call accepts no address, length or payload. A free, unmapped page and its zero tail are initialized before publication on the existing two-entry FIFO. The controller follows its usual mapping, private copy, release and validation path. Refusal at capacity preserves queued data. Each generation allows 64 eligible requests including full-queue refusals. Event 14 records entry tick, issuer, requested generation, signed status, returned handle or zero, depths before and after, and the closed-gate bit. The row timestamp bounds completion. The associated command record identifies the current generation. [Frozen fixture](https://github.com/Romanof123/JNOX/commit/2aec98ad1296497819a74f4a70bf502c53311564) · [Calendar and RAM records](../examples/native-safety-admission/README.md).

정상 송신자는 CLOSE 이후 EPIPE를 받는다. 지정한 두 가상 시험 패키지에서 감독 슬롯 2가 같은 세대의 닫힌 게이트 뒤에 고정 CRC 오류 프레임을 넣는다. 수신기는 기존 큐와 읽기 전용 매핑으로 비공개 복사본을 검사한다. 포화 거부도 세대별 64회에 포함한다. 독립 검증팀은 실제 입력·큐 깊이·마지막 승인 상태·가상 출력 차단을 각각 확인한다. 중복·채널 ID 등 개별 거부 사유는 [#397](https://github.com/Romanof123/JNOX/issues/397)에서 추적한다.

### Independent result / 독립 검증 결과

On the single-core Cortex-A72 virtual bench the three independently reset fault cases first established output ON and actually closed normal input under both FIFO and RR. Seven invalid fixtures per policy reached the receiver in the invalid-stream case. Each saturation run delivered 14 fixtures and recorded seven full-queue refusals in 21 requests. Across these six boots conservative upper observations were 19.923 ms from CLOSE to latch, 0.031 ms from latch to virtual output inhibition and 19.927 ms end to end. The largest conservative interval between completed checks was 3.524 ms. Every scoped limit was checked independently. The ten timing boots each missed two full 1 ms phases. Source, private RAM, register transition bounds, false-claim controls and independent replay are retained. [Archived measurements and replay](evidence/native-permit-closed-gate.en-ko.md).

가상 단일 Cortex-A72의 FIFO·RR에서 무송신, 연속 불량, 큐 포화를 각각 독립 부팅으로 시험했다. 실제 CLOSE 후 정상 송신이 거부됐다. 각 정책에서 연속 불량 프레임 7개를 수신했고 포화 시도 21건 중 14개 전달·7개 EAGAIN을 확인했다. 여섯 부팅에서 차단 후 검출은 최대 19.923 ms, 검출 후 가상 출력 차단은 0.031 ms, 전체는 19.927 ms였다. 연속 완료 간격의 상한은 최대 3.524 ms였다. 1 ms 위상 기준과 실물·제품 승인은 계속 열려 있다. [개별 입력 거부 검증](https://github.com/Romanof123/JNOX/issues/397).
