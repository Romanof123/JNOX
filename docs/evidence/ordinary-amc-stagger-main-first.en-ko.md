# JNOX #237 / #234 combined-main first boots · 통합 메인 최초 부팅

Commit `9cfc563e984c7051877c708572d71757f9729955`
Tree `6e82fac4808d1a56ef9cd91b9947202f888669d0`

The published Bash block exited 0. All four first cold captures reached the guest footer. The original decoder also accepted the three earlier schemas. QEMU used TCG, one Cortex-A72 and GICv2 on virt. Durations below are capture elapsed time.

공개 절차의 네 최초 실행이 모두 통과했다. 기존 세 사례는 이전 판정기로도 통과했다. 아래 시간은 호스트 캡처 경과 시간이며 게스트 카운터 측정값은 별도로 표기했다.

| Case / 사례 | First / 최초 | Owner sequence / 실행 순서 | Elapsed / 경과 (ms) |
|---|---|---|---:|
| exact-lo | PASS | 0, 1, 0 | 190.366779 |
| exact-hi | PASS | 0, 1, 1, 1, 3, 0 | 328.711024 |
| over-hi | PASS | 0, 1, 1, 1, 3, 0 | 328.76667 |
| stagger-idle | PASS | 0/0, idle, 0/1, 3/0, 1/0, 1/0, 3/0 | 375.002446 |

## Measurements · 관측값

At 62,500,000 Hz, epoch 50,100 and quantum 3,125,000, b1 armed the idle frame. IRQ 30 arrived 3,092,430 ticks after arm and 11,382 ticks after the b2 compare. Releases at b2, b3 and b4 had absolute deadlines 6, 8 and 8. At b5 LO 0/1 was discarded after one grant at 710,669 samples and checksum 3,198,010. Trigger HI 1/0 continued to 1,411,594 samples and checksum 6,352,170. First HI work followed the b5 grid point by 23,042 ticks. Surviving HI 3/0 resumed from 8/36 to 16/72. Its b3 release to SVC took 9,398,096 ticks, leaving 6,226,904 ticks until b8. Release-to-retirement measured 3,160,257, 6,262,885 and 12,517,125 ticks for 0/0, 1/0 and 3/0 respectively.

유휴 타이머 IRQ는 b2 비교 시점보다 11,382틱 늦게 도달했다. b5에서 1회 실행한 LO를 폐기하고 두 HI의 작업 진행 상태를 보존했다. 생존 HI는 체크섬 36에서 72로 진행했고 b3 릴리스 후 9,398,096틱에 SVC에 진입했다. 이 값은 해당 QEMU 단일 실행에서 관측했다.

## Controls and reproduction · 대조군 및 재현

The old caller input JNOX_AMC_APP=stagger-idle exited 101 before launch. JNOX_APP built all four cases. Unsupported board and unknown app each exited 101. All 138 host tests passed, including all 16 idle/carry-demand combinations. The requirement graph and scheduling evidence matrix exited 0. Six graph unit tests passed. #236 remains held.

이전 선택기와 미지원 보드·앱은 각각 종료 코드 101이었다. 수정된 선택기로 네 사례 모두 부팅했다. 호스트 테스트 138개, 그래프 테스트 6개 및 스케줄링 근거 행렬 검사를 통과했다.

For a fresh run, use Bash at the repository root with the AArch64 bare-metal Rust target, QEMU, Python and pyelftools. Set JNOX_REPLAY_REVISION to the full commit above and run the Bash block in docs/amc-stagger-plan.en-ko.md with clean selectors. To verify the frozen sessions offline, extract the sibling ZIP and run the following commands in its root.

신규 실행은 위 커밋을 JNOX_REPLAY_REVISION에 설정하고 문서의 Bash 블록을 실행한다. 보존된 최초 결과는 ZIP을 풀고 다음 명령으로 검증한다.

```bash
sha256sum -c SHA256SUMS
python3 check.py
```

## SHA-256

Archive / 압축 파일 `f805c32849e1221bd7a9623f94892a1298d04d13084ab25cd4ce7ef3e87c78c1`

```text
4695151acd927e073e5f156fadd24f883fa5bd24e52d8351e072228a50e3891f  exact-lo/kernel.elf
20cdb99b07aca94bb5d4f5eb40b41ec420e9c6f9083e43b0c53193b9b319f6ef  exact-lo.map
64f129eb0fbf553b80472a8ab306a4cfdd7fe90f3ddeaff95f0d069ba87f6b03  exact-lo/freeze.json
b005d0dc8fc85945e60e10d6e28015c43e40033ed6cf10571c34f9d03572f55c  exact-lo/first/session.json
35e54413753a1f9b687bb762dbdd81c8a418c30242a50d61452275dd46a41e50  exact-lo/first/uart.bin
53dde431288b0964fe01d8288996ae5b328932aca588d020d4d224e0bc5c6a44  exact-hi/kernel.elf
0213f16cb78402864a14c0eeb6bf6f01887876269d95dbea19d339bc4b2b488b  exact-hi.map
f69120d44469eacd647e07efdd09f8f3ae599ed5a9a2fafc2643e6bb38043ac7  exact-hi/freeze.json
cd2531725eca35f291bebdc0bd0f7fd24fec6c66e46cffaa8de700c693f752b4  exact-hi/first/session.json
3464c842f4c718c24b8b08ebb42508488cf847157f61f8fc75780469ddaf60da  exact-hi/first/uart.bin
c07bbd2435bfa5434979f189b013bb9889702dff0e8c41bac08e219e0978a960  over-hi/kernel.elf
97affe6e4bad1d4f1050f2bb5c66f629d1d6e93bfa843065e8abb107db9f02d7  over-hi.map
dc1ab2abc48e8d3c30162b8c886ded326c72c62ec360fa55a4b9cfdf5b48edcb  over-hi/freeze.json
345fcd56a06f13646fc9d08c92137c1e5def14da705249dccc046ca9f86f82f3  over-hi/first/session.json
15eda3b436c0a1193cecd0827a43b2ce3099982b58350fa9426e5b918f90804e  over-hi/first/uart.bin
7e2fd23c833deecc32594ea5095642100cf507f0f24cbfe046af0966ec4d3e5f  stagger-idle/kernel.elf
e1b4f86be1930d3ddcd9b1addffa263ffc5d52376f6a508c994fa1cc769c13d0  stagger-idle.map
309aeb699948b7195c60a314db72596f9e4221ab9bd1baf7c205b905336d0b33  stagger-idle/freeze.json
ab5e1b03600b44518faae3e5e1d98062b30e72c3f6db75a8222f5397f8905165  stagger-idle/first/session.json
c604edd4e003daeb5573b27f592c9de0f1c66aa0b3d1fdf49e032a6db0ed14ef  stagger-idle/first/uart.bin
```

[Original four combined-tree first boots and corrected public workflow](ordinary-amc-stagger-main-first.zip) · SHA-256 f805c32849e1221bd7a9623f94892a1298d04d13084ab25cd4ce7ef3e87c78c1.
