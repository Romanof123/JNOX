# Pi 4 고정 우선순위 증빙 · 최초 모델 거절

[#146](https://github.com/Romanof123/JNOX/pull/146) · [#143](https://github.com/Romanof123/JNOX/issues/143) · [JSON](pi4-fixed-witness-first.json) · [ZIP](pi4-fixed-witness-first.zip) · [CHG-063](../boards/pi4-fixed-runtime.md#chg-063)

raspi4b 모델의 첫 엔지니어링 부팅 두 건은 종료 자료를 출력했으나 배포 디코더는 모두 종료 코드 1(terminal idle mismatch)로 거절했다.

FIFO·RR의 일반 및 엔지니어링 부팅 총 4건을 보존했다. 정책별 작업 UART 접두부가 서로 일치했고 코어 0, EL1, 가상 카운터 62.5 MHz였다. 같은 부팅의 독립 IRQ/SVC/벡터/ERET 기록을 FIFO 10세대와 RR 7세대 모두에 대응시켰다. FIFO에서 HIGH가 세대 4에 A를 선점하고 세대 5에 A가 재개돼 진행량이 15,257,564에서 15,257,565가 됐다. RR에서는 세대 2에 HIGH, 세대 3에 A가 재개돼 18,722,743에서 18,722,744가 됐다. 세대 4·5에서 동료 IRQ 선택, 6에서 B의 후속 작업을 관측했다.

종료 검사는 프레임 주소와 유휴 진입 주소를 비교했다. 실제 선택 ELR(인덱스 10)은 진입 주소와 일치했다. 후속 재개 검사도 잘못된 세 필드를 사용했다. 선택 프레임·ELR·SP는 인덱스 9–11이다. 배포 제어 스크립트 두 건은 변조 전에 중단됐다. 별도 시험에서 정책별 변조 16건 모두가 거절됐고, 관측 누락으로 분류할 수 있는 유효 사례도 거절됐다. 양성 디코더 제어는 확보하지 못했다. 잘못된 Cargo 선택자 12개는 모두 거절됐다.

작업 표본 카운터(필드 6)에서 디스패치 기록 카운터(필드 24)를 뺀 구간은 FIFO 2,029–23,009틱, RR 236–20,113틱이었다. 표본 스택 깊이는 각각 144–176, 160–192바이트였다. 일반 부팅에 같은 서비스 구간의 양 끝점이 없어 계측 오버헤드나 최악 실행 시간·최대 스택 사용량을 판정할 수 없다. 엔지니어링/일반 raw는 20,768/16,672바이트, BSS는 101,560/81,992바이트, 링크 예약 스택은 모두 16,384바이트였다.

동일 경로에서 빌드한 직전 부모와 비교해 두 일반 이미지 모두 ELF와 raw 해시가 달랐다. 각 raw에서 다른 두 바이트는 fixed_runtime.rs 소스 행 195→210, 181→189에 해당한다. 과거 EVD-031 모델의 참조 해시는 별도로 기록했다. 열 해석 수정과 실제 복귀 이후 구간은 원본 바이트에 대한 별도 평가가 필요하다. 실물 및 릴리스 승인은 미확인이다. 아래 명령은 원본을 복원하고 당시 FIFO 거절을 재현한다.

```sh
unzip pi4-fixed-witness-first.zip -d witness
python3 witness/restore.py
cd witness/restored
python3 tools/pi4_fixed_witness.py fifo-eng/first/uart.bin --profile pi4-fixed-fifo-witness --manifest fifo-eng/first/manifest.json --elf fifo-eng/first/kernel.elf --image fifo-eng/first/boot/kernel8.img
```

보존된 FIFO와 RR UART를 원래 디코더는 모두 코드 1로 거절했고, 수정 디코더는 같은 바이트에서 모두 코드 0을 반환했다. 새 게스트 부팅은 없었다. [수정된 Pi 4 증빙 디코더 · 호스트 재생](pi4-fixed-witness-replay.md)
