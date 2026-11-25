# Native mailbox capabilities / 네이티브 메일박스 권한 핸들

CHG-310 · REQ-024 → DES-013 → IPC-009 → VER-024. This prospective local profile gives ordinary C and Rust applications transferable authority to send a word to a real mailbox. The kernel owns the bounded handle tables and transfer notices. Run the three applications as separate processes on single-core AArch64 QEMU virt under fixed-priority FIFO and RR. IPC-009 remains open. [Capability contract #386](https://github.com/Romanof123/JNOX/issues/386) · [IPC acceptance #24](https://github.com/Romanof123/JNOX/issues/24)

일반 C·Rust 앱이 실제 메일박스에 한 워드를 보낼 권한을 다른 프로세스에 전달하는 로컬 프로필이다. 커널이 유한한 핸들 표와 전달 통지를 소유한다. 단일 코어 AArch64 QEMU virt에서 세 앱을 별도 프로세스로 실행하고 고정 우선순위 FIFO·RR을 각각 검증한다. IPC-009는 승인 전이다.

## Authority and commit / 권한과 확정 시점

Each process has four handle entries and two pending transfer notices. A handle identifies an owner-local slot and generation. Provision each origin with a live target mailbox and a subset of SEND and DELEGATE. Configure allowed transfer directions separately from origin grants and direct mailbox routes. SEND authorizes the existing bounded mailbox operation. DELEGATE authorizes transfer on a permitted direction. Neither a numeric slot nor knowledge of a tag conveys either right.

프로세스당 핸들 슬롯은 4개, 미수신 전달 통지는 2개다. 핸들은 소유 프로세스 안의 슬롯과 세대를 식별한다. 최초 권한에는 사용 중인 대상 메일박스와 SEND·DELEGATE 중 허용한 권한을 명시한다. 전달 방향, 최초 권한, 직접 메일박스 경로를 각각 설정한다. SEND는 기존 유한 메일박스로의 송신을, DELEGATE는 허용 방향으로의 전달을 승인한다. 슬롯 번호나 태그를 아는 것만으로는 권한을 얻을 수 없다.

A transfer requests copy or move, a subset of the source rights and one opaque user tag. Validate the source identity, requested rights, direction, endpoint liveness, recipient handle space, notice space and generation capacity before committing. In one atomic operation install the recipient handle and enqueue its notice; a move also invalidates the source. A copy preserves the source. On refusal, keep the original ownership, rights, generations, references and notice queue, including existing tags. Consume each notice exactly once. Its recipient can send with SEND and can delegate further only with DELEGATE.

전달 요청에는 복사 또는 이동, 원본 권한의 부분집합과 불투명한 사용자 태그 하나를 담는다. 확정 전에 원본 핸들, 요청 권한·방향, 종단 생존, 수신 핸들 슬롯·통지 공간·세대 여유를 검사한다. 수신 핸들 설치와 통지 추가를 원자적으로 처리하며, 이동이면 같은 시점에 원본을 무효화한다. 복사는 원본을 보존한다. 실패하면 소유권·권한·세대·참조·기존 통지와 태그를 유지한다. 통지는 한 번만 꺼낼 수 있다. 수신자가 다시 전달하려면 DELEGATE가 남아 있어야 한다.

Closing a handle or exiting releases the applicable references. Target mailbox teardown revokes the ability to send through outstanding handles. A reused slot has a new generation; an old handle must fail even if the slot number and target match. Refuse generation exhaustion before wraparound. Preserve the direct mailbox ABI for callers with explicit routes. Expose C and Rust calls with distinguishable outcomes for stale identity, denied rights or direction, insufficient recipient resources and endpoint death. Freeze the numeric ABI and error mapping with the implementation.

핸들 닫기와 프로세스 종료는 해당 참조를 회수한다. 대상 메일박스를 폐기하면 기존 핸들로도 송신할 수 없어야 한다. 재사용된 슬롯은 새 세대를 사용하며 번호와 대상이 같아도 과거 핸들은 거부한다. 세대는 순환 전에 고갈로 거부한다. 직접 경로에 대한 기존 메일박스 ABI를 유지한다. C·Rust 호출은 오래된 핸들, 권한·방향 거부, 수신 자원 부족, 종단 종료 결과를 구별하며 구체적인 ABI 번호와 오류값은 구현 계약에 기록한다.

## Independent acceptance / 독립 검증

* **Authority in use.** Give an origin a capability, allow transfer to a second process, and give that holder no direct route to the third process's mailbox. Record the holder's direct-send refusal. Take the tagged notice once and use the installed handle to send a distinct payload; check the target's actual received word and sender identity. After copying verify that the origin still sends. After moving verify that its old handle fails while the recipient succeeds. Attempt to add a missing right, delegate a SEND-only handle and transfer in a forbidden direction. Check that refused operations cause no delivery.
* **Capacity and lifetime.** Independently fill the recipient's four handle entries and its two pending notices, leaving the other resource available in each case. Attempt a move. Check the source is still usable, record the unchanged references and tags, free one entry and retry. Require exactly one committed transfer and one notice on success. Exercise receiver exit before and after commit, close/reuse, stale generation and target death. A precommit exit must leave the source usable; a postcommit exit reclaims the receiver's reference without resurrecting the moved source.
* **Evidence.** Retain package grants, actual process ELF identities, payloads, owners, slot/generation pairs, rights, tags, operation results and exits. Reconcile peak handles and pending notices with final references and queue contents. Record scheduler order, timing and observation gaps. Use deterministic host controls for exact death/commit permutations and generation exhaustion, identifying which orderings actually occurred in guests. No physical-board safety, WCET or product qualification is claimed by these virtual cases.

* **권한 사용.** 최초 소유자에서 두 번째 프로세스로의 전달을 허용하되 두 번째 프로세스에서 세 번째 프로세스의 메일박스로 보내는 직접 경로는 부여하지 않는다. 직접 송신 거부를 확인한 뒤 태그가 있는 통지를 한 번 꺼내 핸들로 고유한 데이터를 보내고, 대상의 실제 수신 워드와 송신자 신원을 대조한다. 복사 후 원본 송신 성공, 이동 후 원본 거부와 수신자 성공을 확인한다. 권한 추가, SEND 전용 핸들의 재전달 및 금지 방향 전달을 각각 거부하고 유령 전달이 없음을 확인한다.
* **포화와 수명.** 다른 자원은 남겨 둔 채 수신 핸들 4개와 대기 통지 2개를 각각 가득 채워 이동을 시도한다. 실패 후 원본 사용 가능 여부와 보존된 참조·태그를 확인하고 한 칸을 비워 재시도한다. 성공 시 확정과 통지는 각각 한 건이어야 한다. 수신자 종료가 확정 전이면 원본을 보존하고 확정 후이면 이동된 원본을 되살리지 않고 수신 참조를 회수한다. 닫기·재사용, 과거 세대 거부, 대상 종료에 따른 권한 소멸도 확인한다.
* **증거.** 패키지 권한, 실제 ELF 식별값, 데이터, 소유자, 슬롯·세대, 권한 비트, 태그, 연산 결과와 종료값을 보존한다. 최대 핸들·통지 점유량과 최종 참조·큐 내용을 대조한다. 스케줄 순서·시간과 관측 공백을 기록한다. 종료와 확정의 경계 순서 및 세대 고갈은 결정론적 호스트 시험과 실제 게스트 관측을 구별한다. 가상 사례는 실물 보드 안전·WCET·제품 적격성 근거를 제공하지 않는다.
