# Native P/Q first IRQ / P·Q 최초 IRQ

Source dd090eee0012393b70eb1c954e0f550aa6429248, prospective plan 63290f3d00c37071ad0925c4ec6ae38f1eb93da6. The initial 20 package-wrapper pairs all failed diagnostic symbol identity because build_native_el0.py removes JNOX_* before spawning the kernel build. The first archive preserves those 40 original captures, logs and ordering. SHA-256 f00faa4f797705c4e873ddf9cc0fae83dde158a3e426e63f24f4fd5f9a7fbf0a. They supply zero phase observations. Subsequent explicit build_kernel.py builds used the original ordinary user ELF and verified the nonzero linked symbol before launch. These first actual diagnostic pairs are a separate cohort. Both cohorts used five serial pairs per table and clock, plain-first in rounds 1, 3, 5. WSL ext4, one Cargo job, one vCPU Cortex-A72, QEMU 6.2 TCG. Only ordinary boots.

All 40 symbol-qualified captures passed ordinary functional replay with the prior 01521256 decoder, including embedded load image, physical address spaces, stopped private state and ordinary generation checks. User ELF 4969269f4f9a6cf5d75e35678586f02bbccbf219129851fc73b15744aeb04e0c matches the prior P/Q packages byte-for-byte. Decoder/journal ordinary sources unchanged from the prospective plan. Explicit kernel selectors rejected an unsupported value, weighted-share, and non-native mode. All 20 matched comparisons completed. Independent read of the separate symbol, raw RAM and ordinary traces checked 210 actual old/new generation and PID joins, incl. Q same-PID, ordered stamps, both margins and interval sums. Absolute stamps in pair JSONs and individual old/new intervals, four paired deltas and both margins in transitions.tsv. First means generation 1→2. Generation 1 has no prior IRQ. Drain transitions lack phase rows.

Original limit 62,500 ticks, 62.5 MHz. On default, plain/phase misses occurred on 3/5 and 5/5 P boots, 1/5 and 4/5 Q boots, all generation 2. Both icount3 tables passed 5/5 each mode. Stock plain therefore still misses 1 ms. The diagnostics altered observed timings. The largest first-stage interval in the first default pair on each table was the private-result read (P 10,819, Q 18,088 ticks). Multiple other stages also took thousands. On icount3 the audit interval was largest. No causal host attribution or constant observer-cost correction is supported. Sampled IRQ entry follows timer expiry and the first EL0 read follows ERET. Neither exact instruction crossing nor a host translation component was measured. No hardware evidence.

한국어. 첫 패키징 도구 실행 20쌍에서는 환경변수가 제거되어 진단 심볼이 없었다. 원본 40개와 실패를 보관했다. 별도 커널 빌드에서 심볼을 확인한 뒤 같은 사용자 ELF로 얻은 최초 유효한 20쌍은 별도로 집계했다. 기본 시계의 일반 실행은 P 5회 중 3회, Q 5회 중 1회에서 1 ms를 초과했다. 계측 실행은 각각 5회와 4회 초과했다. 모두 세대 1→2였다. icount3에서는 모두 통과했다. 두 표 모두 기본 시계 첫 쌍에서 결과 복사가 가장 긴 구간이었다. 첫 IRQ 처리, 선택 후 복귀, 사용자 관측까지 계측의 영향이 있으며 호스트 원인이나 단일 보정 비용으로 분리할 수 없다. 일반 부팅 수용 조건은 충족하지 못했다.

Numeric summary below. Stage order in ticks is IRQ entry→observe begin, observe, private result copy, audit, gap including first record write, schedule, MMU, generation publication, publication end→selection. Delta order is IRQ→selection, selection→return, return→first EL0, opening→first EL0. Each median uses five first transitions or all individual later transitions for that table/clock. Minima are the smallest margins among those plain and phase transitions. First-round absolute stamps and all later transitions appear in the attached evidence.
p default miss plain/phase [([], [2]), ([2], [2]), ([], [2]), ([2], [2]), ([2], [2])]
first delta median [3793, 1714, -126, 4051] margins minima [-13602, -14780]
later delta median [62.5, 5.5, 9.0, 578.5] margins minima [33199, 35015]
round1 stages first [7575, 8722, 10819, 7168, 1734, 227, 882, 35, 3]
p icount3 miss plain/phase [([], []), ([], []), ([], []), ([], []), ([], [])]
first delta median [30, 17, 0, 47] margins minima [61076, 61029]
later delta median [30.0, 17.0, -0.5, 46.5] margins minima [61068, 61023]
round1 stages first [33, 283, 175, 406, 27, 37, 47, 3, 2]
q default miss plain/phase [([], [2]), ([], []), ([], [2]), ([], [2]), ([2], [2])]
first delta median [6366, 1660, 145, 10026] margins minima [-5907, -9979]
later delta median [132, 23, 67, 1072] margins minima [32066, 26526]
round1 stages first [8903, 10417, 18088, 8519, 2168, 1035, 403, 4, 7]
q icount3 miss plain/phase [([], []), ([], []), ([], []), ([], []), ([], [])]
first delta median [30, 17, 0, 47] margins minima [61071, 61024]
later delta median [30, 17, 1, 48] margins minima [61061, 61014]
round1 stages first [33, 290, 183, 405, 27, 38, 54, 3, 2]

[Both original packaging and measured stage series / 최초 패키징과 실제 단계 계측 원본](native-partition-irq-phases-first.zip) · SHA-256 f20dc5423a7a496c3d5e0e6b071434cfb32baa5c548e9f94294c4674b4febc8d.
