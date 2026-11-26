# Protected native permit · corrected first cohort / 수정본 최초 검증

b741028b2b81cb188b5bc9ebe2ab2707b4488aed · tree 9bb22a88791e38264c574b749189f5990ab1e851. QEMU virt Cortex-A72, one CPU, 62,500,000 Hz. 32 independent boots, 17 COMPLETE brackets per boot. 30 exit triplets 71/72/73; both collision supervisors exited 95, all terminal virtual registers zero. No safety event drops. The fixed scheduler trace overflowed; see per-run count. Native qualification HOLD. The original 5 ms inter-completion limit was exceeded in every case at the lower bound. Release to COMPLETE fits 5 ms in these samples. WCET and analytical FIFO/RR admission remain unknown. No physical outputs were used.

32회 독립 부팅, 각 17회 COMPLETE. 모든 시나리오에서 완료 간격 5 ms 초과를 확인했다. 측정된 릴리스→COMPLETE는 5 ms 이내였으며 분석적 수용 여부는 미정이다. 가상 장치만 검증했다.

## Original criteria / 원래 판정 기준

Maxima below are lower..upper elapsed ticks from kernel event 10 brackets. Worst-gap source and destination brackets are retained explicitly. R is release→COMPLETE, G is consecutive COMPLETE spacing. Supervisor mismatch count tests the expected observation at each requested time. Scheduler drop count is separate from the lossless safety ring.
|Case|R ticks|G ticks|Worst gap pair (two COMPLETE intervals)|Supervisor mismatches|Scheduler drops|
|---|---:|---:|---|---:|---:|
|fifo-bad-spacing|[208623, 211518]|[350705, 350880]|[4371391, 4371437] → [4722142, 4722271]|0|190|
|fifo-boundary|[154914, 157399]|[321850, 321932]|[4189169, 4189209] → [4511059, 4511101]|0|191|
|fifo-collision|[215270, 217830]|[341937, 342019]|[4127529, 4127568] → [4469505, 4469548]|2|229|
|fifo-delayed|[98590, 98634]|[397680, 397769]|[5070414, 5070459] → [5468139, 5468183]|0|244|
|fifo-early|[189459, 191966]|[324335, 324420]|[4054447, 4054487] → [4378822, 4378867]|0|208|
|fifo-healthy|[165780, 168808]|[325541, 325628]|[7324143, 7324187] → [7649728, 7649771]|0|302|
|fifo-invalid|[163695, 166170]|[321893, 321989]|[7868372, 7868413] → [8190306, 8190361]|0|365|
|fifo-live-startup|[163892, 166397]|[324855, 325434]|[5432896, 5433422] → [5758277, 5758330]|0|312|
|fifo-no-rearm|[163387, 166310]|[318464, 319602]|[4163630, 4164271] → [4482735, 4483232]|0|182|
|fifo-observed|[167814, 170373]|[322502, 322626]|[8228780, 8228824] → [8551326, 8551406]|0|302|
|fifo-one|[176420, 178786]|[321413, 321542]|[4798908, 4798972] → [5120385, 5120450]|0|177|
|fifo-phase|[198698, 201163]|[321549, 322215]|[5344573, 5345187] → [5666736, 5666788]|0|234|
|fifo-queued|[177869, 180398]|[333530, 333617]|[4412763, 4412804] → [4746334, 4746380]|0|265|
|fifo-restart|[156850, 159373]|[325495, 325595]|[6380257, 6380312] → [6705807, 6705852]|0|373|
|fifo-saturation|[166422, 168883]|[327674, 328150]|[4204934, 4204974] → [4532648, 4533084]|0|367|
|fifo-silence|[171256, 174298]|[330823, 330925]|[4209559, 4209607] → [4540430, 4540484]|0|236|
|rr-bad-spacing|[153850, 156300]|[337918, 338010]|[4183202, 4183253] → [4521171, 4521212]|0|190|
|rr-boundary|[164215, 167196]|[326477, 326574]|[4184855, 4184910] → [4511387, 4511429]|0|191|
|rr-collision|[220745, 223189]|[354400, 354518]|[4061146, 4061215] → [4415615, 4415664]|2|229|
|rr-delayed|[92137, 92183]|[392688, 392772]|[5110880, 5110918] → [5503606, 5503652]|0|248|
|rr-early|[181903, 184768]|[322429, 322513]|[4172986, 4173032] → [4495461, 4495499]|0|208|
|rr-healthy|[179224, 181718]|[324807, 324906]|[6692172, 6692221] → [7017028, 7017078]|0|302|
|rr-invalid|[169342, 171778]|[322758, 322854]|[4192140, 4192195] → [4514953, 4514994]|0|365|
|rr-live-startup|[168492, 171044]|[319886, 319988]|[6676338, 6676397] → [6996283, 6996326]|0|312|
|rr-no-rearm|[152871, 155333]|[323365, 323458]|[4228490, 4228540] → [4551905, 4551948]|0|182|
|rr-observed|[168875, 171373]|[333771, 333877]|[4187802, 4187842] → [4521613, 4521679]|0|302|
|rr-one|[183758, 186144]|[323366, 323488]|[5711661, 5711701] → [6035067, 6035149]|0|177|
|rr-phase|[147367, 150241]|[326137, 326231]|[7243445, 7243488] → [7569625, 7569676]|0|233|
|rr-queued|[163925, 166972]|[319830, 319923]|[4159399, 4159451] → [4479281, 4479322]|0|265|
|rr-restart|[180170, 182717]|[322197, 322313]|[4152907, 4152973] → [4475170, 4475220]|0|373|
|rr-saturation|[160604, 163265]|[325432, 325526]|[5093636, 5093676] → [5419108, 5419162]|0|367|
|rr-silence|[175296, 178965]|[330603, 331149]|[4219883, 4220328] → [4550931, 4551032]|0|236|
## Gate, detection, virtual output / 게이트·감지·출력
Cmd 3 is atomic CLOSE. Cmd 4 is MARK during continued delivery. Bounds are latest detect−earliest injection, latest safe−earliest detect, latest safe−earliest injection. 25/5/30 ms limit ticks are 1562500/312500/1875000. Every listed triple fits. The latch bracket spans the pre-latch sampled counter and subsequent log stamp. Output bracket spans the actual swap/barrier/readback. These on-device code brackets and terminal RAM support finite bench observations. A separately qualified independent observer is still outstanding. Restart CLOSE follows intentional withdrawal and provides no enabled→safe fault response.
|Case|Cmd|Fault interval|Detect interval|Safe interval|Three upper bounds|Ordered|
|---|---:|---|---|---|---|---|
|fifo-invalid|4|[5182532, 5183404]|[6305472, 6307158]|[6307555, 6307559]|[1124626, 2087, 1125027]|True|
|fifo-phase|3|[5132262, 5133502]|[6274272, 6275927]|[6276313, 6276318]|[1143665, 2046, 1144056]|True|
|fifo-queued|3|[5210531, 5213718]|[6270413, 6272154]|[6272596, 6272601]|[1061623, 2188, 1062070]|True|
|fifo-saturation|4|[5265382, 5266499]|[6386569, 6388097]|[6388418, 6388428]|[1122715, 1859, 1123046]|True|
|fifo-silence|3|[5162060, 5163293]|[6391195, 6393016]|[6393404, 6393411]|[1230956, 2216, 1231351]|True|
|rr-invalid|4|[5255165, 5256100]|[6371267, 6372615]|[6372922, 6372926]|[1117450, 1659, 1117761]|True|
|rr-phase|3|[5152210, 5153406]|[6304412, 6306113]|[6306490, 6306495]|[1153903, 2083, 1154285]|True|
|rr-queued|3|[5276559, 5279193]|[6337559, 6339259]|[6339637, 6339642]|[1062700, 2083, 1063083]|True|
|rr-saturation|4|[5210605, 5211527]|[6332482, 6334071]|[6334444, 6334454]|[1123466, 1972, 1123849]|True|
|rr-silence|3|[5180666, 5182545]|[6396767, 6399833]|[6400459, 6400481]|[1219167, 3714, 1219815]|True|
## Frame and recovery observations / 프레임 및 복구 관찰
Output sequence lists successive observed transition destinations. Sender/received counts include staged or retried writer operations and represent different sets. Rejected private decision rows compare sequence/issue/permit/recovery before and after; the fault latch is separate. Header errors are sender unexpected returns / receiver sequence or issue disagreement / supervisor observation mismatches.
|Case|Output sequence|Sender/received|Unchanged/rejected|Header errors|
|---|---|---|---|---|
|fifo-bad-spacing|none|2/2|0/0|0/0/0|
|fifo-boundary|none|2/2|0/0|0/0/0|
|fifo-collision|none|4/4|1/1|0/0/2|
|fifo-delayed|1,0|5/5|1/1|0/0/0|
|fifo-early|1,0|2/2|0/0|0/0/0|
|fifo-healthy|1,0|7/7|0/0|0/0/0|
|fifo-invalid|1,0|13/13|10/10|0/0/0|
|fifo-live-startup|none|7/7|0/0|0/0/0|
|fifo-no-rearm|none|2/2|0/0|0/0/0|
|fifo-observed|1,0|7/7|0/0|0/0/0|
|fifo-one|none|1/1|0/0|0/0/0|
|fifo-phase|1,0|4/3|0/0|0/0/0|
|fifo-queued|1,0|7/3|0/0|0/0/0|
|fifo-restart|1,0,1,0|11/9|1/1|0/0/0|
|fifo-saturation|1,0|14/13|10/10|0/0/0|
|fifo-silence|1,0|4/3|0/0|0/0/0|
|rr-bad-spacing|none|2/2|0/0|0/0/0|
|rr-boundary|none|2/2|0/0|0/0/0|
|rr-collision|none|4/4|1/1|0/0/2|
|rr-delayed|1,0|5/5|1/1|0/0/0|
|rr-early|1,0|2/2|0/0|0/0/0|
|rr-healthy|1,0|7/7|0/0|0/0/0|
|rr-invalid|1,0|13/13|10/10|0/0/0|
|rr-live-startup|none|7/7|0/0|0/0/0|
|rr-no-rearm|none|2/2|0/0|0/0/0|
|rr-observed|1,0|7/7|0/0|0/0/0|
|rr-one|none|1/1|0/0|0/0/0|
|rr-phase|1,0|4/3|0/0|0/0/0|
|rr-queued|1,0|7/3|0/0|0/0/0|
|rr-restart|1,0,1,0|11/9|1/1|0/0/0|
|rr-saturation|1,0|14/13|10/10|0/0/0|
|rr-silence|1,0|4/3|0/0|0/0/0|
### FIFO
Collision expected enabled at 18 and 37 ms. Actual mismatch rows (planned, before, after, flags, seq) [[4926710, 4934362, 4935293, 9, 2], [6114210, 6120989, 6121933, 1, 3]]. No enabled precondition; no private clock control in this collision run. Seq 4 wire issue 6301710, actual publish interval [6311364, 6311670], detection [6321610]. Exact live coincidence HOLD.
Phase private exact-input clone [1, 5017302, 6267302, 0, 1, 17, 3, 0, 1, 6579801, 1, 0]. Real last issue+20 ms 6267302; wake and post-CHECK for the nominal expiry−1 activation [6267301, 6273352, 6277542, 6283319]. Live one-tick-before condition missed.
Live-startup original first/second issue, pre-rearm, eligible fields [3809080, 4428292, 4738672, 0, 0]. Observed-mode seq 1–2 synthetic; seq 3 onward measured. Actual wire issue ticks [3776186, 4401186, 5031110, 5656836, 6283147, 6906656, 7541204]. Healthy wire issue ticks are synthetic [3813385, 4438385, 5063385, 5688385, 6313385, 6938385, 7563385]. All other cases inject scheduled timestamps by design.
Early and boundary independent private controls [1, 4603512, 0, 1, 4916012, 12, 0] and [1, 4737247, 0, 1, 5049747, 12, 0]. One/no-rearm/bad-spacing had no enable transition. Restart had two enable→withdraw cycles. Saturation staged retry (mode, issue, handle, publish) [[0, 5315130, 26, 18446744073709551605], [2, 5315130, 26, 0]]. Queued staged/closed retry [[0, 5074996, 26, 18446744073709551605], [2, 5074996, 26, 18446744073709551584]]. Status fields are raw two’s-complement u64.
### RR
Collision expected enabled at 18 and 37 ms. Actual mismatch rows (planned, before, after, flags, seq) [[4861943, 4871805, 4873075, 9, 2], [6049443, 6057120, 6058077, 1, 3]]. No enabled precondition; no private clock control in this collision run. Seq 4 wire issue 6236943, actual publish interval [6246804, 6247124], detection [6256421]. Exact live coincidence HOLD.
Phase private exact-input clone [1, 5044650, 6294650, 0, 1, 17, 3, 0, 1, 6607149, 1, 0]. Real last issue+20 ms 6294650; wake and post-CHECK for the nominal expiry−1 activation [6294649, 6302704, 6307697, 6315823]. Live one-tick-before condition missed.
Live-startup original first/second issue, pre-rearm, eligible fields [3806347, 4421655, 4733535, 0, 0]. Observed-mode seq 1–2 synthetic; seq 3 onward measured. Actual wire issue ticks [3799274, 4424274, 5053487, 5681624, 6305670, 6930189, 7554749]. Healthy wire issue ticks are synthetic [3801954, 4426954, 5051954, 5676954, 6301954, 6926954, 7551954]. All other cases inject scheduled timestamps by design.
Early and boundary independent private controls [1, 4720157, 0, 1, 5032657, 12, 0] and [1, 4729618, 0, 1, 5042118, 12, 0]. One/no-rearm/bad-spacing had no enable transition. Restart had two enable→withdraw cycles. Saturation staged retry (mode, issue, handle, publish) [[0, 5263498, 26, 18446744073709551605], [2, 5263498, 26, 0]]. Queued staged/closed retry [[0, 5141252, 26, 18446744073709551605], [2, 5141252, 26, 18446744073709551584]]. Status fields are raw two’s-complement u64.
## Evidence boundary / 증거 범위
Every RAM dump was decoded against that boot’s kernel ELF and each application’s own ELF and TTBR0. Per-boot decoded JSON includes sender and received protected bytes, application rows, kernel event rows, drops and flags. Raw launch manifest binds processor, emulator digest, kernel/user hashes and exact argv. Host 5/5 and independent 5/25/30 ms equality/one-tick-over controls passed. Default startup-probe exited 37. Qualification remains open for original completed-check timing, live exact expiry collision, live worst-phase, exact 10 ms live issue spacing, independent observation uncertainty and applicable scheduling admission.
최대 완료 간격, 실제 만료·도착 동시성, 만료 1틱 전 실제 검사, 실제 발행 주기, 독립 관측 불확도 및 분석적 수용 조건은 여전히 미충족이다. 호스트 경계 검증과 가상 장치 실측은 각각 별도 증거로 보관했다.

[Raw captures and offline replay](native-safety-channel-corrected.tar.gz) (SHA-256 `ac1ccebba5a87122971cfa2c6c601765abddbd09d8193c1e28a659e4b4fb6587`).
