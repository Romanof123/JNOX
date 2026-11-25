# Final integrated clock reader / 최종 통합 시계 판독기

Approve the d01e7f30fc35e4a50e2cc81285bf5802387c6e9e reader for the finite single-vCPU diagnostic deployment. All six #354 source sessions produced observations-met. The authentic first-run public classification remains five met and one failed. The original C/Rust compatibility session recorded a real return-gate handoff in row 909, then the high app's exit restored the original low process in row 1515. The original four timed crossings had exact raw GICC_IAR 0x1e and HI1/g0 ownership. Cold sessions, loaded programs and timings were reused unchanged. The integrated revision changed the reader after the previous hold; no guest launch or build was needed.

단일 가상 CPU 진단 구성의 수정 판독기를 승인한다. 실제 여섯 세션이 모두 관측 조건을 충족했다. 원래 공개 리더의 최초 5/6 판정과 실제 복귀 게이트 선점 기록은 유지된다. 과거 실행을 새 측정으로 바꾸지 않고 원시 자료를 다시 판독했다.

|Offline cohort / 판독 집합|Observed result / 실제 결과|
|---|---|
|#354 original six|6 met, including disabled fallback and C/Rust control|
|Prior 29 altered RAM receipts|29 rejected, including all four 0x41e fake PPI acknowledgements|
|Prior four altered external traces|4 rejected, including the two authentic IRQ destinations replaced with synchronous vectors|
|Additional six altered destinations|6 rejected. Each of idle EL1 IRQ, EL0 IRQ and EL0 SVC was tested once with the destination removed and once with the wrong linked slot|
|Explicit CPU annotation controls|CPU 0 accepted, CPU 1 rejected on the otherwise identical one-CPU trace|
|#348 original twelve on final shared CLI|10 met, two incomplete|
|#348 twenty-two prior falsifications plus four additional RAM controls|26 rejected|

The six new controls use an actual position-2 QEMU trace and its sealed kernel’s native_el0_vectors address 0x40202800 (size 0x800). GIC/EL1h idle IRQ maps to +0x280, EL0 synchronous entry to +0x400 and EL0 IRQ to +0x480. Rehashed changed artifacts and sessions reached the public validator. The same CLI accepted the original bytes. The original 93488785 reader accepted six invalid inputs; compare the archived prior hold with the explicit outcomes in clock-results.json. One repository unittest executed its three exception categories and wrong/missing subcases. No unexpected rejection or false acceptance occurred in the specified matrix. The raw session files remain obtainable from the identified public archives; this compact supplement retains the exact commands, manifest identities, independent scripts and actual outputs.

실제 ELF 벡터 표와 원본 QEMU 기록을 기준으로 세 진입 유형의 벡터 삭제·교체를 각각 재현했다. 변조 후 해당 자료와 세션의 해시를 갱신해 공개 진입점까지 검사했다. 이전 보류의 여섯 허위 통과가 모두 해소됐으며 검사한 집합에서 남은 오판은 없었다.

For #348 the first 8 met/2 incomplete/2 failed remains historical. Later 10 met/2 incomplete remains the interpretation of the same records. Normal-HI traced and staggered traced still have no HI1 crossing PPI30. The two CPU-clock SVCs occurred 125 and 1,145 ticks past their compares. Each archive and session member was checked before replay; all 38 legacy reader invocations completed. raw/verified.json contains the archived helper's historical reader label; the actual reader executed here, its script digests, git revision and commands are bound separately in final-reader.txt and the corresponding command logs. Hardware jitter, WCET, FTTI and product release remain outside these virtual observations.

#348 원본 두 추적은 여전히 시계 SVC가 먼저였고 HI1 세대의 교차 PPI30은 없었다. #354의 다른 프로그램·읽기 경로에서 발생한 성공을 이 두 실패에 소급하지 않는다. 물리 보드 시간과 제품 승인은 열려 있다.
