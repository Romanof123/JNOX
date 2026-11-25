# Native deployment policy v1 / 네이티브 배포 정책 v1

CHG-313 · REQ-025 · DES-014 · VER-025. A system integrator can package named applications and their periodic tasks together for one AArch64 QEMU virt CPU. The policy selects the fixed higher-priority scheduler and FIFO ordering among equally ranked ready tasks. Existing ordinary native packages keep their own entry points and evidence. [Change #394](https://github.com/Romanof123/JNOX/issues/394) · [Scheduling #21](https://github.com/Romanof123/JNOX/issues/21) · [Application execution #8](https://github.com/Romanof123/JNOX/issues/8) · [Runtime #12](https://github.com/Romanof123/JNOX/issues/12)

## Build contract / 빌드 계약

Run `python3 tools/build_native_el0.py --board virt --task-policy deployment.txt --out image` with a new output path. The first declaration is `version 1`. Each application declaration is `app APP ELF_PATH`; each task declaration is `task APP TASK ENTRY PERIOD_MS PRIORITY DEADLINE_MS WATCHDOG_MS|off`. For example, these two hypothetical ELFs export the named functions:

~~~text
version 1
app sensor sensor.elf
app report report.elf
task sensor sample sample_entry 10 7 4 20
task sensor filter filter_entry 20 5 8 off
task report publish publish_entry 50 3 15 100
~~~

Resolve ELF paths relative to the policy file. Use unique application names and unique task names within each application. Application/task identifiers are at most 31 ASCII letters, digits or underscores and cannot start with a digit. ENTRY names a defined executable function in that application's static AArch64 ELF. Period and enabled watchdog are positive decimal milliseconds fitting u32; priority is 1–255 and deadline is 1 through PERIOD_MS inclusive. Multiple application names may refer to identical ELF bytes. The compiler must emit every declaration, bind the complete versioned `policy.bin` and ELF identities to the kernel image, or reject the whole build with the offending declaration or resource constraint. Shape, arithmetic, entry, image permissions, guarded stack space and target RAM must fit. Silently dropping the twentieth app or substituting another entry invalidates the package.

위 명령은 정책 파일을 기준으로 ELF 경로를 해석한다. 앱 이름은 전체에서, 태스크 이름은 해당 앱 안에서 유일해야 한다. ENTRY는 그 ELF의 실행 가능한 함수 심볼이다. 주기와 활성 감시 간격은 양의 u32 밀리초, 우선순위는 1–255, 마감은 1부터 해당 주기까지다. 감시만 끄려면 소문자 `off`를 쓴다. 같은 ELF를 여러 이름으로 선언할 수 있다. 전체 선언과 ELF 식별 정보를 버전 있는 policy.bin 및 커널에 결합한다. 잘못된 형식·심볼·권한·범위 또는 RAM/스택 한도 초과 시 문제 선언이나 자원 한도를 밝혀 전체 빌드를 거부한다.

## Execution and supervision / 실행과 감시

An application owns one private address space even when another app loads identical bytes. Its tasks share application data and use distinct saved contexts and per-app local stack slots with unmapped guards. All tasks start from the declared periodic release calendar. A higher-priority ready task preempts a lower-priority one. Equal-priority tasks join the ready queue in FIFO order. Completion waits for the next applicable nominal release; elapsed periods do not create a burst of backlogged jobs.

| AArch64 service in x8 | Calling task operation |
| --- | --- |
| 80 | Complete the current job and wait. A successful call resumes at its next release. |
| 81 | Report a nonzero u32 fault code in x0. Stop the calling task immediately at that syscall; a zero code is invalid. |
| 82 | Exit the calling task with its signed status in x0. |

Use the native C or Rust task-policy SDK. An entry function must explicitly complete in a loop, report a fault, or exit. Falling out of the entry function is not a defined termination mechanism. Fault reports carry the application and task identity; sibling tasks and other applications continue according to their own policy.

A job completing exactly at its deadline is on time. A still-outstanding job records a miss at the first representable tick beyond its deadline; a miss alone does not stop the task. An enabled watchdog measures the gap from **first release**, then from **that task's last successful COMPLETE**, including time spent waiting for its next job. Equality at the watchdog limit is allowed. Process an already-expired watchdog before accepting a completion at a later tick. A late job that still meets its independent watchdog can complete and renew its own watchdog. Other tasks' progress cannot renew it. The kernel includes the earliest obligation in its timer decision while a CPU-bound task runs or the CPU idles. Retain the nominal due tick and actual observed handling time separately. Turning the watchdog off preserves deadlines, periodic scheduling and explicit fault handling.

앱마다 별도의 주소공간을 사용한다. 같은 앱의 태스크는 데이터를 공유하고 각자의 문맥과 가드가 있는 로컬 스택을 사용한다. 높은 준비 우선순위가 선점하며 동순위는 준비 큐 도착순이다. 완료는 다음 해당 주기까지 대기하며 지나간 주기를 한꺼번에 재실행하지 않는다. C/Rust SDK에서 SVC80은 현재 작업 완료·대기, 81은 0이 아닌 자체 오류 즉시 중단, 82는 명시적 태스크 종료다. 엔트리 함수 반환에 종료를 맡기지 않는다. 오류 기록에 앱과 태스크를 모두 남기고 다른 태스크는 계속 실행한다.

작업 마감과 감시 간격은 경계 틱을 포함한다. 마감만 넘으면 미스를 기록하며 감시 기한 안의 늦은 완료는 유효하다. 감시는 최초 릴리스에서 시작하고 이후 자기 태스크의 성공한 완료만 기준을 바꾼다. 다음 작업을 기다리는 중에도 감시하며 이미 지난 감시 기한은 새 완료로 되돌릴 수 없다. `off`는 완료 감시만 비활성화한다. 시스템콜 없는 계산 또는 유휴 상태에서도 타이머로 처리하고 예정 시각과 실제 처리 시각을 구분한다.

## Public acceptance / 공개 수용 기준

- Build a fitting configuration with 20 named applications, including repeated ELF identities and an application with multiple named tasks. Reconcile source declarations, manifest, embedded table and all runtime identities. Demonstrate independent writable products across applications and intentional sharing between sibling tasks; check guarded stack placement and distinct saved contexts.
- Observe different periods, higher-priority preemption, FIFO ties and per-task completions. Separate an ordinary deadline miss followed by a successful renewal from a watchdog stop. Exercise first-release silence, waiting past the watchdog, a no-SVC runaway, watchdog off, invalid zero fault, immediate nonzero fault and explicit exit. Show a surviving task's subsequent useful work in each stop case.
- Exercise equality and the first tick past each limit in the production decision logic. Preserve raw timer/return and per-task records for live observations. Reject duplicate names, missing apps/functions, incompatible ELF/permissions, unsupported versions, invalid numbers and over-capacity input. Retain original outcomes, partial evidence and the ordinary native regression.

20개 앱이 수용되는 실제 구성, 동일 ELF의 앱별 분리, 앱 내 복수 태스크 공유·스택 보호, 주기·동순위 순서·선점을 확인한다. 첫 릴리스부터 무응답, 대기 중 감시 초과, 호출 없는 계산, 감시 해제, 마감만 넘긴 완료, 자체 오류와 종료를 구분하고 남은 태스크의 실제 작업을 확인한다. 두 기한의 경계 틱·다음 틱, 잘못된 입력·용량 초과의 전체 거부와 기존 네이티브 경로를 독립 확인한다. 최초 실패·부분 기록도 보존한다.

The minimum reaction in this profile is to identify and stop the faulting task and preserve scheduling for survivors. Any actuator inhibition, cross-task escalation, physical response bound or approval must be allocated by the product integrator and demonstrated for its target. The protected virtual-output watchdog has its own receiver and recovery contract. [Safety channel #11](https://github.com/Romanof123/JNOX/issues/11) · [Protected watchdog #392](https://github.com/Romanof123/JNOX/issues/392)

이 프로필의 최소 반응은 고장 태스크 식별·중단과 나머지 태스크의 스케줄링 유지다. 장치 차단, 다른 태스크로의 확대 조치, 실물 반응 기한과 승인은 제품 통합 책임자가 별도로 배정하고 해당 대상에서 입증한다.
