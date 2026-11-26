# Independent first positions for IPC-010 #389 / 독립 최초 실행 판정

Frozen source fbbb5ceacaa39df47bac418ca9ef22390bc51b6b, tree 1ea80734209e52de371fad9961b0f7f628e605bc. Bench only, one QEMU virt Cortex-A72, QEMU 6.2.0 TCG and 62,500 ticks/ms. All 34 boots have three expected exits. Every qualification boot exceeded the inclusive 62,500-tick full phase limit. RR/live additionally failed to block for job 0 phase B. Refuse both tested packages. No independent physical device observer, analytical WCET, comprehensive blocking bound or universal IRQ envelope was established.

FIFO·RR 각 10개 기본 시나리오와 네 독립 타이머 부하 실행은 모두 1 ms 위상 한계를 초과했다. RR/live의 첫 작업 B에서는 실제 절대 대기가 누락됐다. 두 구성의 수용을 거부한다. 실측 가상 출력에 한정하며 제품·실기기 안전, 독립 출력 계측 및 최악시간 상한은 미확정이다.

## Closed full-response brackets in physical counter ticks / 전체 응답 구간

Effective blocking is bracketed by scheduler pre/post transition counter reads, including work after stage 3. A sleeping segment must have a real block, ready mask edge, dispatch and EL0 return/first marker. The selected dispatch bracket begins at the pre-transition sample and ends at the assembly return sample, ahead of the resumed user marker. RAM was read by walking three descriptors from each PID's own SPACES root using symbols in its own ELF. All rank selections match highest runnable priority, all boots initially reached idle. All 32 COMPLETEs per boot were retained. Dedicated QUAL and SAFETY lost zero rows. Legacy FIXED overflowed, so that stream alone cannot reconstruct any entire run. In RR/live the one open response gives only a lower bound. The ‘closed max’ is the maximum among closed phase intervals. Sender and job columns are maximum upper endpoints of closed intervals. The gap column bounds adjacent COMPLETE bracket differences including B to next A. Grid IRQ includes actual sampled physical IRQ returns for exact grid compares only.

|Boot|Phase max lower|Closed max upper|Open|Job upper|Sender upper|Check gap upper|Grid IRQ upper|ENABLE 18/37 ms|
|---|---:|---:|---:|---:|---:|---:|---:|---|
|fifo-boundary-witness|123241|123340|0|204353|216311|216786|23993|1/1|
|fifo-collision|134200|134297|0|203715|219810|207758|29235|1/1|
|fifo-delayed|125701|125792|0|217669|131685|199625|29714|0/0|
|fifo-healthy|116507|116640|0|191474|213592|218452|28251|1/1|
|fifo-healthy-irq|67713|67833|0|224083|134703|182181|14065|1/1|
|fifo-invalid|119331|119437|0|188204|215178|218779|25111|1/1|
|fifo-live|120641|120730|0|195629|225396|211415|22222|0/0|
|fifo-lower-work|131062|131172|0|193886|224272|198300|24691|1/1|
|fifo-lower-work-irq|66389|66438|0|222688|150316|174430|11528|1/1|
|fifo-overrun|116655|116746|0|190430|207407|221738|22027|1/1|
|fifo-saturation|132252|132348|0|198476|229187|208921|34558|1/1|
|fifo-silence|89389|89471|0|245721|253950|171187|23499|1/1|
|rr-boundary-witness|120034|120123|0|202843|213351|219487|25429|1/1|
|rr-collision|120602|120685|0|202960|215203|223367|23956|1/1|
|rr-delayed|127611|127708|0|222699|133988|203508|29138|0/0|
|rr-healthy|116868|116964|0|190946|208081|219764|23023|1/1|
|rr-healthy-irq|68094|68141|0|224391|131615|181228|11260|1/1|
|rr-invalid|96365|96455|0|239192|246075|169855|27051|1/1|
|rr-live|159915|99733|1|192199|270097|233022|36904|0/0|
|rr-lower-work|119526|119627|0|194697|211936|227365|28588|1/1|
|rr-lower-work-irq|78572|78627|0|234877|139024|178710|12772|1/1|
|rr-overrun|114385|114470|0|195545|207963|223656|28135|1/1|
|rr-saturation|127830|127922|0|202538|232500|203378|33724|1/1|
|rr-silence|121858|121952|0|188526|213637|220537|28032|1/1|

Qualification maxima are observations of these first runs. The 5-ms consecutive check criterion and 10-ms sender deadline held over all closed intervals. The grid IRQ upper endpoints above 31,250 ticks warrant separate inspection of individual target, entry and return rows. All successful EVENT 7 snapshots matched 44 original sender bytes and handles, all EVENT 11 publication rows matched sender attempts. The saturation FIFO/RR runs reached queue occupancy two, logged failed publications and retained retries with matching prior EAGAIN handle and identical bytes. Both live first-pair spacings missed 625,000 ticks (FIFO 525,029 and RR 459,823). No live ENABLE. Synthetic first pairs in healthy recovered. See each issue tick alongside the creation tick in observations.json. Collision/boundary wire issues are synthetic. Delayed seq 1 was originally issued at E and delivered at 30 ms; the new valid pair and later rearm are separate in RAM. Both private boundary clones recorded the D−1 fresh, the next 5-ms check latched, and the simultaneous valid input rejected. Ordinary observer rows below use the live counter.

## Virtual fault observations / 가상 출력 오류 응답

Times are conservative upper endpoints from injector EVENT 4, expiration operand EVENT 3 and device write/barrier/readback EVENT 2. Virtual register brackets are recorded by the kernel; these are not independent external transition times. Reported bounds apply to the observed injection. Inspection of last valid issue separately is essential. Raw timestamps are in observations.json.

|Boot|fault→detect|detect→safe|fault→safe|
|---|---:|---:|---:|
|fifo-boundary-witness|1045082|1588|1046670|
|fifo-collision|1043396|2379|1045775|
|fifo-invalid|1087430|2319|1089749|
|fifo-saturation|1228017|1571|1229588|
|fifo-silence|1222923|2277|1225200|
|rr-boundary-witness|1049118|2038|1051156|
|rr-collision|1048719|2097|1050816|
|rr-invalid|1227174|2116|1229290|
|rr-saturation|1225725|1881|1227606|
|rr-silence|1226536|2168|1228704|

## Original single-check observer and controls / 기존 단일 검사 경계

Offsets are signed physical-counter ticks from D = last valid wire issue + 1,250,000. Observer collision had no enabled state before expiry in either policy. In phase, the retained actual decision operands fail the requested exact two-check witness. The first accepted issue and signed near-boundary operands, expiry, switch and publication brackets are retained in the JSON. Engineering invocation used one CPU, tcg,thread=single and shift=0,align=off,sleep=off. Default controls use original role ELFs and ordinary TCG. The four engineering boots are separate from the six original normal-clock regression controls.

|Boot|Enabled before D|Detect offset|Next publication bracket offsets|
|---|---:|---:|---|
|default-fifo-collision|0|20930|[[12470, 12838]]|
|default-fifo-healthy|0|70560|[]|
|default-fifo-phase|1|6823|[]|
|default-rr-collision|0|21660|[[11627, 11946]]|
|default-rr-healthy|0|71767|[]|
|default-rr-phase|1|5762|[]|
|observer-fifo-collision|0|1133|[[554, 687]]|
|observer-fifo-phase|1|162|[]|
|observer-rr-collision|0|1169|[[567, 700]]|
|observer-rr-phase|1|170|[]|

The optional IRQ boots delivered 26, 27, 26 and 26 of their respective 27, 28, 27 and 27 arms, in FIFO healthy/lower-work and RR healthy/lower-work order. No MASK overlap record was retained. This supplies no residual-blocking estimate. All IRQ records in those streams had IAR 27 and completed EOI brackets. Independent IRQ domain and per-return timing evidence is in the UART. The lower-work supervisor requests a separate 19.9-ms calendar point. Three simultaneous scheduling inputs and bounded observed activity do not establish policy equivalence.

## Reproduction / 재현

The outer archive contains a compressed, individually hashable per-boot tar for every capture, both sets of role ELFs, command scripts, reducer, observations and SHA256SUMS. The boot tars retain untouched RAM, UART, ELF, launch and session identities. Extract each boot tar into a separate directory named after its tar basename under a working root, put the supplied role ELFs at that root, then run `python3 reduce.py WORKING_ROOT OUTPUT_DIR` on Linux with aarch64-linux-gnu-nm available. For a fresh capture use separate clean paths in the shell scripts, check out the exact revision, source ~/.cargo/env and run build.sh, run.sh, original.sh sequentially. original.sh obtains fixed single-thread icount through sitecustomize.py, which is supplied beside it. Compare launch.json actual hypervisor command line and ELF hashes. Wall-clock results can vary.

각 실행 압축 파일은 RAM·UART·실행 ELF·명령·해시를 포함한다. 각 tar를 같은 이름의 디렉터리에 풀고 ELF를 상위 경로에 둔 뒤 reduce.py로 재판정할 수 있다. 실시간 카운터의 반복 결과는 달라질 수 있다. 첫 도구 준비 중 PowerShell 변수 전달, Bash 줄 끝, Cargo PATH 오류가 발생했고 게스트 시작 전 해결했다. preflight-cargo.log는 최초 Cargo 오류 원본이다.

[Raw captures and offline replay](native-safety-timing-first.tar.gz) (SHA-256 `1cf0453e19df78da3a7b2b5e3b7736143b4b3a36052c24f0249d7f81a501ba06`).
