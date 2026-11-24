# Ordinary mutex return and physical timer IRQ observation (#246)

Freeze before any launches. Use PI then ceiling, each first cold trace without the plugin followed by its first cold trace with the plugin. Keep all four attempts separately, including incomplete output. Never replace a first attempt. Use the #236 integration archive at 317d7365, original exception logging, 100 µs (6250 ticks), QEMU 6.2.0 Linux TCG single Cortex-A72/virt GICv2. The sole experimental change within each pair is #178’s unchanged six-site NO_REGS observer (663b598e5976d8d8578153b4716c677d727f80b8). Use an ordinary user. The archived ELF SHA-256 values are fc5be57112e78bb9a4d2de6bb4fc17078331e69c7b26eac10660c9eeea6075ce (PI) and 172a88efd01ebda34e6cd05bc32ab4318f814cdec53f40c95d98dc9fb852727c (ceiling).

|Site, execution callback before instruction|PI PC:word|Ceiling PC:word|
|---|---|---|
|0 final CLREX before ERET|40201aa0:d5033f5f|same|
|1 ERET|40201aa4:d69f03e0|same|
|2 checked SVC return destination|40202480:b5000040|40202554:b5000040|
|3 publish protected-work active flag (release store)|40202ba0:c89ffd6f|40202cc8:c89ffd8f|
|4 immediate successor to publication|40202ba4:d503201f|40202ccc:d503201f|
|5 physical IRQ vector branch|40201280:14000161|same|

Match all CLREX/ERET epochs in order with UART R and external exception returns (same destination). Match every traced exception to its UART E kind and saved PC, and its vector. R[0] is boot. Select the unique final LOW acquire E (1, resource 0 PI / 1 ceiling, incoming and selected task 0), verify its LOW stack frame, selected PC and following R. Require next exception is the physical timer IRQ (INTID 30, LOW interrupted), and before any intervening return. Attribute sites 2–4 only within this epoch on the same host TID and vCPU 0. A missing destination is valid with an immediate IRQ saving that destination. Report each available endpoint separately, never synthesize a timestamp. Site 3 alone witnesses an attempted instruction. Site 4 witnesses passage beyond the publication store, not an arithmetic result. Guest IRQ active/chunk counters measure guest progress. The vector callback precedes its branch, after exception delivery. None of these host reads timestamps guest instruction retirement.

For each eligible endpoint B relative to ERET callback A, wall lies between B.wall0−A.wall1 and B.wall1−A.wall0, widened by two reported wall resolutions. CPU delta uses the same thread CPU clock and ±2 CPU resolutions. Subtract interval bounds conservatively for wall-minus-CPU. Report raw endpoints and clock resolutions. Clock resolution is nominal, not a bound on accuracy or callback overhead. This includes emulation and exception logging, and the observer can change outcomes. No attribution to scheduler switches, compiler, TCG or logging. The #236 first integration chain qualified 2/2 and legacy controls 2/4, with both traced controls stopping at reason 16, zero chunks. Later boots have separate scope. Historical descheduling, six-boot criterion, product blocking bounds and physical timing remain unresolved.

최초 순서는 PI 비계측·계측, 천장 비계측·계측이다. 모두 기존 예외 추적을 켠 별도 콜드 부팅이며 최초 실패도 보존한다. 위 SHA-256의 ELF와 명령어, 원래 비교값 6250틱 및 일반 사용자 권한을 고정한다. CLREX와 ERET 콜백은 명령 실행 전이다. 부팅 복귀부터 모든 복귀를 UART R 및 예외 복귀 PC와 순서대로 대조하고 모든 예외를 E의 종류·저장 PC·벡터와 연결한다. 마지막 LOW 획득의 자원·태스크·프레임과 선택된 복귀를 확인한 뒤, 같은 vCPU 0과 호스트 스레드의 다음 물리 타이머 IRQ까지 분석한다. 도착 PC 실행 전 IRQ가 오면 해당 도착 시각은 비워 둔다. 작업 플래그 저장 직후 명령에 도달해야 저장 통과를 관찰한 것이다. 실제 계산 진도는 IRQ 시 게스트 청크 기록을 사용한다. 중복·누락 등으로 대응이 불명확하면 해당 시간을 제외한다. 단조 벽시계의 양끝 범위와 동일 스레드 CPU 시간에서 각 해상도 두 배를 반영하며 두 범위의 차도 보수적으로 산출한다. 계측 부하와 시계 정확도는 별도의 불확실성이다. #236 최초 체인 2/2, 대조군 2/4와 이번 네 건은 별개다. 과거 스케줄 이탈, TCG·로깅별 CPU 시간, 실물 시간 및 제품 차단 한계는 판단할 수 없다.

## Reproduction / 재현

From this checkout in the original Linux user environment, fetch the two pinned PR heads if the objects are absent, then run the following once with a fresh output path. The capture extracts authenticated ELF bytes, checks all six linked executable words before launch and retains commands, return codes, UART, exception logs and raw marker rows. Run the archived guest control replay as well as the host report. Preserve the guest replay’s positions and verdicts even when the process exits successfully with an unqualified position.

동일 Linux 일반 사용자 환경에서 두 PR 객체를 확보한 뒤 새 경로에 한 번 실행한다. 캡처 스크립트가 ELF와 실행 명령어 여섯 개를 확인한다. 원본과 모든 판정·누락 사유를 보존한다.

~~~sh
git fetch origin refs/pull/178/head refs/pull/236/head
python3 tools/ordinary_mutex_host_capture.py /tmp/mutex246-first
(cd /tmp/mutex246-first && sha256sum -c SHA256SUMS)
python3 /tmp/mutex246-first/control_replay.py /tmp/mutex246-first
python3 tools/ordinary_mutex_host_report.py /tmp/mutex246-first
~~~

[Independent first captures, clock bounds and replay / 독립 최초 측정과 재생](evidence/ordinary-mutex-host-thread-first.en-ko.md).
