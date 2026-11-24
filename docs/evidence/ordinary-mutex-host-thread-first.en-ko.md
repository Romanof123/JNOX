# JNOX #247 / #246 independent first capture · 독립 최초 검증

Four frozen first boots completed with reason 0, 256 chunks and guest replay qualification. Each parked guest exited through timeout 124. IRQ interrupted LOW with its work flag active in all four. PI observed had entered the loop before completing its first chunk. 최초 네 실행 모두 적격이며 각 256청크를 완료했다. 네 IRQ 모두 LOW 작업 활성 구간에 발생했다. PI 계측은 첫 청크 완료 전이었다.

| Position / 최초 순서 | IRQ completed chunks / IRQ 청크 | IRQ saved PC | LOW return ordinal / total (boot = 1) | LOW frame → destination |
|---|---:|---|---|---|
|00 PI trace|34|40202bb0|2/8|40209ed0 → 40202480|
|01 PI trace + observer|0|40202bc8|2/8|40209ed0 → 40202480|
|02 ceiling trace|65|40202cd8|3/13|40209ec0 → 40202554|
|03 ceiling trace + observer|1|40202cd4|3/13|40209ec0 → 40202554|

Each interval below starts at the pre-ERET callback. All six callbacks in the target epoch occurred in the frozen order, single vCPU 0, TID 699 (PI) or 913 (ceiling). Raw clock brackets and every executed epoch are retained. Signed residuals include zero throughout. 각 행은 ERET 실행 전 콜백에서 해당 지점까지의 나노초 범위다. 동일 가상 CPU·동일 호스트 스레드의 벽시계 및 CPU 시계 명목 해상도는 각각 1 ns였다. 비계측 두 건의 호스트 시각은 관측되지 않았다.

| Workload | Endpoint / 종료 지점 | Wall ns | Thread CPU ns | Wall minus CPU ns |
|---|---|---:|---:|---:|
|PI|2 return destination / 복귀 목적지|7493–8619|8053–8057|−564…566|
|PI|3 publication store / 활성 플래그 저장 직전|35829–37106|36446–36450|−621…660|
|PI|4 store successor / 저장 다음 명령|36801–37787|37288–37292|−491…499|
|PI|5 IRQ vector / IRQ 벡터|69416–70622|69969–69973|−557…653|
|Ceiling|2 return destination|3345–4371|3835–3839|−494…536|
|Ceiling|3 publication store|33885–35111|34482–34486|−601…629|
|Ceiling|4 store successor|34847–35793|35314–35318|−471…479|
|Ceiling|5 IRQ vector|79384–80571|79957–79961|−577…614|

Public PR head bc770814d7736f11815445c261196fdead244f22, frozen plan e139384b8310a26819461d9b93b4bb9c98a6a092. Original #236 archive from 317d7365ad966441453eabb38aeab1c78e751f7b, SHA-256 fa89510c77757edbcbd70c300c69663f5e7220d31a1303f84015c6207db362fd. Exact ELF SHA-256 PI fc5be57112e78bb9a4d2de6bb4fc17078331e69c7b26eac10660c9eeea6075ce, ceiling 172a88efd01ebda34e6cd05bc32ab4318f814cdec53f40c95d98dc9fb852727c. Capture verified six linked words per ELF before launch. Both clocks and all site PCs/words are described in frozen.md. WSL Ubuntu uid 1000, stock package QEMU 6.2.0 (dpkg verification clean), one Cortex-A72 TCG, GICv2, 62.5 MHz, comparator +6250 ticks, INTID 30. The #178 plugin source at 663b598e5976d8d8578153b4716c677d727f80b8 registers pre-instruction NO_REGS callbacks and writes host samples to a file. No guest changes, elevated launches or replacement boots. Public plan/capture/report merged cleanly onto main 2be75be76f363b90a60bdee8dd1b958fd80158b5, resulting tree c8580d34a1e22cb431606c352df79e86fa43476c. All three files match; combined report byte-identical on the original raw files.

원본 ELF 두 개와 이미지별 여섯 실행 명령어를 검사했다. 단일 vCPU에서 일반 사용자로 기본 배포 QEMU와 원래 외부 예외 로그를 사용했다. 계측 때만 #178의 동일 플러그인을 사용했다. 모든 UART 복귀·예외를 외부 기록에 대조했고 해당 LOW 획득과 다음 IRQ 사이의 복귀 프레임·목적지 및 계측 순서를 확인했다. 공개 캡처·게스트 재생·호스트 보고 명령은 각각 종료 코드 0이었다. 결합 트리에서도 원본을 재생해 같은 호스트 보고를 얻었다.

Host wall readings bound the thread-CPU sample at each callback; bounds widen by twice the corresponding nominal resolution. Subtraction uses opposing ends. Site 3 precedes the store; site 4 establishes passage beyond it. Vector callback follows exception delivery. Clock accuracy, translation, observer and exception logging overhead remain unresolved. Historical #236 chain 2/2, controls 2/4, both traced controls reason 16 with zero final chunks. These later samples cannot identify historical descheduling, attribute cost causally, establish physical/product timing, or satisfy the original six-boot criterion.

콜백은 명령 실행 전이며 벡터 콜백은 예외 전달 후다. 벽시계 양끝과 스레드 CPU 구간에 각 해상도 두 배를 반영하고 반대쪽 끝을 빼 잔차를 얻었다. 저장 다음 명령 도달은 저장 통과를 보이며 계산 진도는 게스트 청크 수로 확인한다. #236 최초 체인 2/2, 대조군 2/4이며 당시 추적 대조군 둘은 사유 16·최종 0청크였다. 이번 네 건으로 과거 스케줄 이탈, 계측 비용의 원인, 실물·제품 지연 또는 원래 6회 기준 충족을 판단할 수 없다.

## Offline replay / 오프라인 재생

From the extracted archive / 압축 해제 디렉터리에서 실행한다.

~~~sh
(cd first && sha256sum -c SHA256SUMS)
python3 first/control_replay.py first
python3 tools/ordinary_mutex_host_report.py first
python3 independent.py first
~~~

[Four original captures, ELF identities and independent clock interval replay](ordinary-mutex-host-thread-first.zip) · SHA-256 2bcc73d39677e9b5460dcda5f01026cec1df6856ccba5bbfb5721b3b85e2de9e.
