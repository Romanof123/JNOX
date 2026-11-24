# 스케줄링 릴리스 구성 30종의 정적 메모리 크기

[English](scheduling-footprint-review.en.md)

최소 부팅 ELF의 파일 크기는 67,200바이트, 매핑 크기는 16,667바이트였다. 구성 30종 중 매핑 크기는 SMP가 259,814바이트로 가장 컸고, 이름이 지정된 BSS는 mutex가 197,928바이트로 가장 컸다. 이 측정은 #21에서 다루는 스케줄링 구성의 링크 결과를 보완한다. [크기 측정 #66](https://github.com/Romanof123/JNOX/issues/66) · [스케줄링 검증 범위 #21](https://github.com/Romanof123/JNOX/issues/21)

| 구성 | ELF 파일(B) | PT_LOAD 매핑(B) | 최소 부팅 대비 매핑 차이(B) | BSS(B) | 링크된 스택(B) |
|---|---:|---:|---:|---:|---:|
| boot | 67,200 | 16,667 | +0 | 8 | 16,384 |
| scheduler | 85,280 | 61,030 | +44,363 | 32,792 | 16,384 |
| cbs-wake | 89,640 | 85,705 | +69,038 | 55,688 | 16,384 |
| periodic-overrun | 89,032 | 162,576 | +145,909 | 133,160 | 16,384 |
| priority-resume | 87,384 | 182,381 | +165,714 | 152,256 | 16,384 |
| mutex | 99,856 | 233,584 | +216,917 | 197,928 | 16,384 |
| smp | 125,776 | 259,814 | +243,147 | 197,792 | 16,384 |
| all | 87,728 | 79,456 | +62,789 | 49,240 | 16,384 |

PT_LOAD 매핑 크기는 가상 주소 구간의 합집합이다. BSS와 링크된 스택 영역은 이 값에 이미 포함된다. ELF 파일 길이는 디스크상의 바이트 수다. 스택 열은 링크 시 이름이 지정된 영역의 예약량이다. 실행 중 최대 스택·힙 사용량, 물리 메모리 배치와 스케줄링 지연은 측정하지 않았다.

독립 검증자는 GNU AArch64 readelf로 섹션과 세그먼트를 읽고 별도 바이트 집합 계산으로 지표 9종, 이미지 해시와 구성별 차이를 CLI 결과와 대조했다. 잘못된 입력 두 개는 종료 코드 2를 반환했다. 겹치는 매핑과 빈 매핑을 추가한 대조군은 16,672바이트를 매핑했고 이 중 16,384바이트가 영 초기화 영역이었다. 호스트 시험 4건이 통과했다. JSON에는 구성별 기능 선택, 빌드 명령, 이미지 해시와 측정값을 담았으며 내장 ZIP에는 원본 목록, 독립 검증기와 시험 출력이 있다.

Rust 1.95.0 · `aarch64-unknown-none` · `cargo build --release --locked`. [전체 측정값과 원시 기록](scheduling-footprint-review.json) · [지표 정의](../scheduling-footprint.md) · [측정 도구](https://github.com/Romanof123/JNOX/blob/6eb786ab4aa8ac10849c5d0073f279a0e8b26feb/tools/scheduling_footprint.py) · [측정한 커널 매니페스트](https://github.com/Romanof123/JNOX/blob/93c64c853c3d1bf3f6286da59d83d7dc8394f7b9/kernel/Cargo.toml)
