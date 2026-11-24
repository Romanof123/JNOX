# #216 / #219 first IRQ diagnostic protocol

Parent acceptance / 상위 수용 기준. Preserve the native P/Q user image, normal journal ABI, first cold boot and original 1 ms limit. A phase trace is diagnostic only. Keep #216 open until plain boots independently satisfy ordinary timing. 기존 P/Q 사용자 이미지와 저널 ABI, 첫 콜드 부팅 및 1 ms 기준을 유지한다. 일반 부팅의 독립 검증까지 #216은 열린 상태로 둔다.

Independent first protocol / 독립 첫 실험. Before inspecting timings, allocate five cold plain/phase pairs per P/Q and default/icount3, alternating order AB, BA, AB, BA, AB. Within each pair hold source, user ELF, emulator, host, launch arguments and clock fixed. Retain every initial capture including failures. 각 조건에서 최초 실행 다섯 쌍의 순서를 교차하고 실패도 보존한다.

Validate each capture using the ordinary partition replay. Join phase rows to adjacent ordinary windows by old/new generation and actual old/new PID, including same-PID Q transitions. Require ordered absolute stamps between the previous IRQ sample and the next selection. Compare the first transition with later transitions individually. Report phase minus plain for IRQ-to-selection, selection-to-return, return-to-first-user and opening-to-first-user, plus both margins to 1 ms. No threshold adjustment or subtraction of observer cost. 정상 재생 검증 뒤 실제 세대와 PID로 연결하고 타임스탬프 순서를 확인한다. 각 구간 차이와 양쪽 여유를 보고하며 계측 비용을 차감하지 않는다.
