# QEMU host service intervals / QEMU 호스트 서비스 구간

24 serial default-clock cold boots, frozen c5ab397e guest, observer protocol 971c6467. Within each policy/case the order was stock, empty, observe, observe, empty, stock. Every kernel and user executable matched the archived default capture. QEMU 6.2.0 Ubuntu revision 2ubuntu6.31, GCC 11.4.0. Official v6.2.0 header and its peeled commit 44f28df24767cf9dca1ddc9b23157737c4cbb645 returned the same SHA-256 82c233fc6ab1a9649a26b1a4eae69e94ba2452d280bd82076372ea6bd1a0753f. QEMU SHA-256 0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8. Plugin SHA-256 9ff1cf7999df9ea5f2451dcf02a16041e2876e67ab15af1020539d5011abd27a.

## Guest endpoints / 게스트 종료점

Full controller release-to-finish bounds in 16 ns guest ticks. Outer host ms includes capture and termination. A0 and B0 are the first two phases. A4 is the designed overrun position. The phase limit is 62,500 ticks.

|Position|Host ms|A0 ticks|B0 ticks|A4 ticks|ENABLE 18/37 ms|FIXED dropped|
|---|---:|---:|---:|---:|---|---:|
|01-fifo-healthy-stock|275.77|116707–116770|21951–22009|15350–15919|1/1|278|
|02-fifo-healthy-empty|275.95|111540–111632|100070–100132|23087–23144|1/1|278|
|03-fifo-healthy-observe|275.54|140659–140727|27676–27722|15277–15330|1/1|278|
|04-fifo-healthy-observe|272.95|137564–137648|33355–33420|15968–16029|0/1|278|
|05-fifo-healthy-empty|271.09|89539–89617|91325–91391|12846–12917|1/1|278|
|06-fifo-healthy-stock|269.71|116629–116708|20186–20250|15443–15509|1/1|278|
|07-fifo-overrun-stock|270.25|134386–134480|38500–38563|86057–86494|1/1|324|
|08-fifo-overrun-empty|270.74|117106–117176|19837–19897|88427–88482|1/1|356|
|09-fifo-overrun-observe|273.4|137300–137359|27675–27735|85986–86040|1/1|327|
|10-fifo-overrun-observe|270.81|123735–123809|46596–46663|85681–85719|1/1|323|
|11-fifo-overrun-empty|276.19|119628–119691|31868–31940|89891–89950|1/1|342|
|12-fifo-overrun-stock|274.71|117401–117466|28803–28864|88147–88203|1/1|330|
|13-rr-healthy-stock|278.64|124045–124129|25588–25658|17580–18025|1/1|278|
|14-rr-healthy-empty|271.15|90747–90824|95677–95736|13506–13965|1/1|278|
|15-rr-healthy-observe|278.0|145135–145224|36749–36871|15345–15440|1/1|278|
|16-rr-healthy-observe|272.71|119291–119371|46030–46096|17071–17691|1/1|278|
|17-rr-healthy-empty|271.52|85517–85583|82531–82594|16203–17054|1/1|278|
|18-rr-healthy-stock|284.5|117280–117364|19134–19199|15301–15360|1/1|278|
|19-rr-overrun-stock|274.04|117830–117974|21040–21096|84681–84737|0/1|331|
|20-rr-overrun-empty|271.36|125505–125608|33026–33088|84939–84994|1/1|344|
|21-rr-overrun-observe|275.8|124932–125012|43969–44038|90928–90985|1/1|314|
|22-rr-overrun-observe|271.75|118893–118966|45717–45782|90249–90714|0/1|322|
|23-rr-overrun-empty|273.05|134601–134683|38009–38075|86838–86902|1/1|347|
|24-rr-overrun-stock|272.9|125905–126024|31923–31984|88145–88201|1/1|355|

## Observed service intervals / 관측한 서비스 구간

Across eight observed boots. Wall is the smallest lower bound through the largest upper bound in µs. CPU and RAW show the range of same-thread paired point differences. Switch counts are per interval. The loan service encloses the nested receive. early-services.csv preserves individual intervals, ordinals, thread IDs, and per-function translation overlap.

|Phase|Service|Wall µs|CPU µs|RAW µs|Translations|Vol./invol. switches|
|---|---|---:|---:|---:|---:|---|
|A0|CHECK|368.93–859.14|371.48–856.02|371.22–856.06|23|0–0 / 0|
|A0|loan35 empty|234.01–265.46|235.40–264.07|235.20–263.72|35|0–0 / 0|
|A0|nested empty|51.80–56.12|52.88–55.04|52.84–54.98|7|0–0 / 0|
|A0|COMPLETE|53.30–96.16|54.65–94.09|54.55–94.52|7|0–0 / 0|
|B0|CHECK|1.84–29.94|3.24–10.09|2.98–28.25|0|0–1 / 0|
|B0|loan35 empty|3.09–10.31|4.25–8.53|4.03–8.06|0|0–0 / 0|
|B0|nested empty|0.33–3.66|1.11–2.25|1.10–2.26|0|0–0 / 0|
|B0|COMPLETE|1.28–47.95|2.40–9.78|2.24–28.18|0|0–1 / 0|

Every A0 deadline failed. All boots retained all three expected exits, 32 COMPLETEs and eight successful publications. At 18 ms ENABLE was low at positions 04, 19 and 22, including one stock control. All reached ENABLE at 37 ms. The plugin series cannot establish a causal effect size. The first CHECK consumed 371–856 µs of vCPU-thread CPU time with zero recorded switches. Later CHECK had zero translation callbacks and one observed voluntary switch in some intervals. Cost includes emulator helpers, translated guest work and probe execution. Translation counts do not measure TCG compilation time. Per-hit median sampling span across observed boots was 1.142–1.183 µs. The true instrumentation cost also includes callback registration and scanning, outside these samples.

All 2,086 host intervals closed. Zero mmap drops, QUAL and SAFETY drops. FIXED dropped its suffix (278–356 entries per boot). Ordinals beyond the retained prefix have no independently verified operation labels. Other generic calls inside the prefix may also lack a CHECK/COMPLETE label. pairs.json retains all intervals, including unlabeled ones. Host bounds never change the guest score.

24회 모두 첫 A 위상이 1 ms를 넘었고, 18 ms 시점의 ENABLE은 04·19·22번에서 0이었다. 19번은 원래 QEMU 대조군이었다. 37 ms에는 전부 1이었다. 첫 CHECK에서는 호스트 문맥 전환이 기록되지 않았고 vCPU 스레드 CPU 시간은 371–856 µs였다. 다음 CHECK의 번역 콜백 수는 0이었다. 번역 시간과 관측기 비용 전체를 분리할 수 없다. FIXED 로그의 유실 구간은 명령을 특정하지 않았다.

## Offline replay / 오프라인 검증

From the extracted directory run sha256sum -c SHA256SUMS, python3 analyze.py . and python3 reduce.py . . with GNU AArch64 binutils installed, followed by python3 report.py . . The first two reducers verify session file hashes, match frozen executables, pair mmap records and decode guest RAM/UART. The archived argv.json is the actual QEMU command line for each trial.

[Raw captures and offline replay](native-permit-host-services.tar.gz) (SHA-256 `63d021bac38068045d58a6cb5a7f9f4356c8f3ffb4c91e9fb4c0ed1afce7bd25`).
