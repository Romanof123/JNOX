# 통합 Pi 4 RR 적재 이미지 비교 및 원본 캡처 재생

통합 커밋 42a1ffe678a015913fda9c8656a30e204927dafd의 두 부모는 #159 증거 커밋 39d2c2e8f54fd29b26b3cbb60f46ee9a5e43b7f4 및 #157 main 29b7f79c4c50e8c92a7abde4e74bc4fc9476d4c9이다. 같은 체크아웃·빌드 경로에서 각 구성을 새 타깃 디렉터리로 만들었다. Rust 1.95.0과 Cargo 작업 한 개를 사용했다. 이번 증거는 정적 빌드와 원본 UART 재생이며 추가 게스트 실행은 0회다.

측정한 #159 소스 f1e2a730614f9313e711b89dcc5a1a9f0b96e760을 기준으로 일반 RR, 공학용 RR 및 v1 FIFO/RR은 진입점과 PT_LOAD 헤더·내용이 일치했다. 전체 ELF 해시는 달랐다. 두 RR raw 이미지는 당시 실행 이미지와 바이트 단위로 일치했다. 기존 판정값을 입력에서 제외하고 원본 UART를 새 ELF·이미지와 함께 판독했다. 일반 O1/O2가 통과했다. 공학용 E1/E2는 구조와 HIGH 복귀가 유효했으나 결과는 각각 2였다. A는 세대 5에서 1,958,498과 1,744,682를 공개했다. B에서 각각 같은 값을 읽은 구간은 세대 4–6에 걸쳤다. 단일 세대 귀속과 후속 산술 진척은 미확인이다.

#157 main과 비교해 일반 뮤텍스 여덟 구성(PI/ceiling별 기본·prepared·stages·selected-return), 기존 뮤텍스, virt/Pi 기본과 Pi directed-wake는 전체 ELF가 일치했다. 나머지 고정 우선순위·협력형 비교 18건은 진입점과 실행 코드가 같았고, 적재 이미지마다 소스 행 번호 2바이트만 달랐다. 기록에 각 가상 주소, 파일명, 행 및 같은 열 번호를 남겼다. Pi·virt FIFO/RR, virt empty/delayed/ties/late, 협력형 events/single, virt 고정 추적과 FIFO/RR 비용 계측을 비교했다. 앞선 뮤텍스 첫 부팅과 SMP 정적 검증의 범위는 그대로다. 물리 보드 동작은 미측정이다.

[ZIP](pi4-rr-handoff-combined.zip) · [CHG-073](pi4-rr-handoff-integration.md) · [CHG-072](ordinary-mutex-anchor-integrated/README.md)

[Finite AMC integration / 유한 AMC 통합](pi4-rr-after-amc.en-ko.md)
