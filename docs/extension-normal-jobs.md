# Ordinary bounded extension jobs (#221 / #21, POL-018)

Use the existing one-core nine-slot counter schedule, five VM fuel units, one-slot grants and one independent reservation every third slot. Link a finite Rust sensor producer, a consuming accumulator and an independent periodic checksum. A single IfReady program nominates the producer when its one-element mailbox is empty. Otherwise Yield lets the trusted fallback service the consumer. Each service records its result once per epoch. Replay the three mailbox handoffs and the three independent samples from the linked inputs. Compare fresh entry and subsequent saved dispatch PCs, timer budgets, fuel and reservation timing against the existing extension contract. Keep legacy image selection and capture layout. Verify normal UART on a plain run, then compare a fresh capture from the same ELF. These are prospective QEMU checks.

# 일반 확장 작업 계획

기존 단일 코어의 아홉 슬롯, 연료 다섯 단위, 한 슬롯 승인, 세 슬롯마다 독립 예약 한 번을 유지한다. 연결된 Rust 생산자가 빈 우편함에 센서 표본을 넣으면 같은 IfReady 프로그램이 다음 차례에 Yield를 실행하여 소비자가 누적값을 계산한다. 독립 예약 작업은 별도 입력의 검사합을 갱신한다. 슬롯별 결과와 세 번의 전달, 예약 시각, 실제 저장 PC에서의 재개, 연료와 승인량을 독립 재생으로 확인한다. 기존 이미지 선택과 캡처 형식을 유지하며 같은 ELF의 일반 실행과 캡처 실행을 비교할 예정이다.
