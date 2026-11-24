# 고정 우선순위 복귀 계약

CHG-029 / IMP-027 / 예정 EVD-027 · POL-002 · #63, 상위 #21 · REQ-002/005/023, DES-003/012, VER-002/005/023.

단일 AArch64 virt CPU에서 세 회차를 별도 스택으로 실행한다. A가 시작하고 CNTP PPI 30이 A 서비스의 1/4 퀀텀 뒤 동순위 B를, 다시 1/4 퀀텀 뒤 HIGH를 준비시킨다. 두 FIFO 방식 모두 B 도착 때 A를 유지한다. 선점형은 HIGH 도착 때 A를 B 앞으로 되돌린다. HIGH 종료 뒤 A는 중단 지점에서 재개해 양보하고 B 종료 뒤 양보 직후 명령에서 재개한다. 비선점형은 두 도착 모두 A로 복귀한다. A가 양보하면 HIGH와 B가 차례로 종료하고 A가 재개한다. 선점형 RR에서는 HIGH 종료 뒤 A의 잔여 서비스만 실행하고 만료로 B에게 넘긴다. B 종료 뒤 A도 종료한다. 매 회차 종료 시 대기열은 비어야 한다.

퀀텀은 물리 카운터 단위 floor(CNTFRQ/10)이다. 프레임 복원 전 마지막 카운터 표본부터 다음 예외 콜백의 첫 표본까지 벡터 비용을 포함해 부과하고 스케줄러 결정 시간은 제외한다. 도착 때마다 실측 서비스를 빼며 HIGH 실행 중 A의 잔여량을 보존하고 재개 때 그 값으로 타이머를 설정한다. 만료 IRQ의 허용 지연은 1/8 퀀텀 이하다. 두 FIFO 회차에서 HIGH 도착 지연 설정은 동일하다. 진행량, 스택 표식, 레지스터 및 저장·선택된 ELR을 기록한다. 독립 첫 부팅에서 대기열·서비스·예외 복귀를 재구성하고 기존 FIFO/RR 프로필을 재실행한다. 실보드 시간 및 제품 승인은 #21에서 추적한다.

```sh
chmod +x tools/priority_resume_capture.py
PRIORITY_RESUME_CAPTURE_DIR="$capture_dir" python3 tools/qemu_boot.py --profile priority-resume --qemu ./tools/priority_resume_capture.py --kernel "$kernel_elf"
```
