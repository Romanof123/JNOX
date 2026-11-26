# Permit runtime decision / 허가 런타임 판정

Revision 7c5b0c2d20f1404f433cc0bf972d790f6a3faa45. Refuse admission under FIFO and RR. Both policies exceed the unchanged inclusive 62,500-tick phase bound in every one of their twelve sampled positions. All 24 provide complete waits and 32 COMPLETEs. Identical workload commands and ELFs can be compared against the original separately sealed series fbbb5ce. Wall-clock QEMU TCG, Cortex-A72, 62,500 ticks/ms. Cold means job 0 here. Finite observations give no analytical hard-time bound or hardware evidence.

FIFO·RR 각각 12개 실행 모두 1 ms 위상 제한을 넘었다. 이번 표본은 모든 대기와 32개 완료를 기록했으며, 첫 작업과 후속 작업을 구분했다. 실제 장치의 응답시간 상한은 미확정이다.

|Position|Old phase upper|New lower–upper|Job 0 A/B upper|Later max upper|Old/new gap upper|Old/new missing|
|---|---:|---:|---:|---:|---:|---:|
|fifo-boundary-witness|123340|119644–119713|119713/25430|89420|216786/220449|0/0|
|fifo-collision|134297|142414–142525|142525/33567|77073|207758/201534|0/0|
|fifo-delayed|125792|117206–117276|117276/20484|58623|199625/198884|0/0|
|fifo-healthy|116640|130187–130275|130275/35520|78324|218452/200305|0/0|
|fifo-healthy-irq|67833|67309–67349|55851/67349|31116|182181/176464|0/0|
|fifo-invalid|119437|125979–126071|126071/38662|88116|218779/204127|0/0|
|fifo-live|120730|119400–119472|119472/21530|87448|211415/220923|0/0|
|fifo-lower-work|131172|102831–102891|102891/90532|31867|198300/172215|0/0|
|fifo-lower-work-irq|66438|67205–67240|50541/67240|33322|174430/179026|0/0|
|fifo-overrun|116746|125750–125817|125817/30513|91510|221738/219557|0/0|
|fifo-saturation|132348|135627–135696|135696/26205|92103|208921/224533|0/0|
|fifo-silence|89471|125809–125870|125870/32935|94304|171187/218385|0/0|
|rr-boundary-witness|120123|124207–124275|124275/32519|91278|219487/218686|0/0|
|rr-collision|120685|121079–121175|121175/27792|78419|223367/207906|0/0|
|rr-delayed|127708|99381–99456|99456/20026|78371|203508/216133|0/0|
|rr-healthy|116964|93281–93341|92971/93341|32048|219764/169858|0/0|
|rr-healthy-irq|68141|69653–69687|59409/69687|34054|181228/178327|0/0|
|rr-invalid|96455|105740–105812|105812/99676|34946|169855/174387|0/0|
|rr-live|99733|135636–135712|135712/31309|92574|233022/216503|1/0|
|rr-lower-work|119627|114232–114296|114296/101665|33790|227365/168837|0/0|
|rr-lower-work-irq|78627|78630–78684|58034/78684|33036|178710/182123|0/0|
|rr-overrun|114470|136490–136558|136558/22398|88147|223656/220400|0/0|
|rr-saturation|127922|139288–139368|139368/24679|92263|203378/224354|0/0|
|rr-silence|121952|142085–142165|142165/20023|94208|220537/229613|0/0|

## Live wire / 실제 발행 프레임

fifo-live encoded issue deltas in ticks, first series [525029, 621089, 624041, 626776, 623035, 621128, 629595], corrected [522853, 623909, 619088, 628081, 620096, 627417, 625123]. Required first pair 625000. ENABLE 18/37 ms 0/0.
rr-live encoded issue deltas in ticks, first series [459823, 625452, 620237, 622068, 625562, 631625, 621607], corrected [509381, 621510, 624601, 621065, 625395, 628214, 625464]. Required first pair 625000. ENABLE 18/37 ms 0/0.

## Authentic single-check observations / 단일 검사

|Boot|Exit values|Enabled immediately before expiry|Detect ticks after expiry|Next publication relative to expiry|
|---|---|---|---:|---|
|default-fifo-collision|[[0, 71], [1, 72], [2, 95]]|0|19493|[[9684, 9979]]|
|default-fifo-healthy|[[0, 71], [1, 72], [2, 73]]|0|69366|[]|
|default-fifo-phase|[[0, 71], [1, 72], [2, 73]]|1|6988|[]|
|default-rr-collision|[[0, 71], [1, 72], [2, 95]]|0|18151|[[9794, 10068]]|
|default-rr-healthy|[[0, 71], [1, 72], [2, 73]]|0|70360|[]|
|default-rr-phase|[[0, 71], [1, 72], [2, 73]]|1|8617|[]|
|observer-fifo-collision|[[0, 71], [1, 72], [2, 95]]|0|1043|[[516, 649]]|
|observer-fifo-phase|[[0, 71], [1, 72], [2, 73]]|1|135|[]|
|observer-rr-collision|[[0, 71], [1, 72], [2, 95]]|0|1080|[[529, 661]]|
|observer-rr-phase|[[0, 71], [1, 72], [2, 73]]|1|140|[]|

## Runtime regressions / 런타임 회귀

The safety-enabled timeout case returns exits 54/55/70. Its server rejects the expired token and answers the second indefinite call. The safety-enabled donation case returns 51/52/53 with the lower priority server dispatched before the ready middle-priority task. The frame case returns 69/70/70. After both peers exit, PID 0 twice sleeps from its own active root, resumes with the saved PC/SP shown by FIXED, and checks the x19/SP and x20 sentinel in EL0. The same capture first switches across distinct private roots. Real 44-byte healthy traffic includes NATIVE LOAN MAP 1/2/3/4, with writable producer PTE, revoked producer PTE, read-only receiver PTE and revoked receiver PTE. The ordinary variable-length loan regression returns 73/74/70. Its deliberately retained first run with the permit profile returns 82/90/70 (incompatible variable-length data); inspect reg-loan separately.

유한 타임아웃·기부 및 무기한 요청/응답, 동일 루트 유휴 복귀와 서로 다른 프로세스 전환, 실제 허가 페이지 전달·회수의 관측 자료를 포함한다. 일반 가변 길이 대여를 허가 프로파일에서 시도한 실패도 보존했다.

## Reproduction / 재현

Extract each qualification/default/observer tar into a directory named for its stem; place the supplied role ELFs at their common parent. Run reduce.py WORK OUTPUT with Linux aarch64-linux-gnu-nm and then compare.py OUTPUT FIRST_EVIDENCE. The original first package is an external comparison input. Regression sources and commands are supplied. Startup shell quoting failed before checkout, the first frame fixture used an unsupported plain asm spelling (failed C source retained), and trailing CR in two host output paths was corrected after capture. No guest position was removed or replaced. Both output copies contain the same sealed archive.

[Raw captures and offline replay](native-safety-timing-corrected.tar.gz) (SHA-256 `5aef9adee569dfe1ea1b259f491157ec636eef82710c0b8b560b2a1291349bf8`).
