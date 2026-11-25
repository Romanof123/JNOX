# #216 host observability decision / 호스트 관측 가능성 판정

Hold the four-boot pilot. #258 environment.json records QEMU 6.2.0 (Ubuntu 1:6.2+dfsg-2ubuntu6.31) and WSL2 6.18.58.1, matching the installed version strings. The current executable SHA-256 is 0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8. No historical executable digest was checked.

The installed trace catalogue has a single translate_block(tb, pc, tb_code) event. exec_tb, exec_tb_nocache, and exec_tb_exit describe execution. These records provide no paired translation start and end for a block. The current user cannot read trace_clock or sched_switch/format. perf, trace-cmd and bpftrace were absent from PATH. No shared timestamp domain, vCPU off-CPU timeline, guest/host alignment error or tracer overhead was established. WSL scheduler visibility would in any event concern Linux tasks within the VM. The observations support neither scheduling nor compilation as the cause of an archived miss.

The retained ordinary P/A ELF hashes match packages.csv. Static anchors include lower-EL AArch64 IRQ slot 0x40200c80, dispatch symbol 0x40202fa8, eret instruction 0x40201254 and user CNTPCT_EL0 read 0x400264. These PCs do not identify the IRQ source, generation, executed TB boundaries or matching later paths by themselves. No guest ran; zero pilot or qualification samples. Raw commands/output are in host-tools.txt, definitions.txt, elf.txt and anchors.txt. A CRLF error in the final grep pipeline of host-tools.txt was corrected in definitions.txt.

4회 진단 실행은 보류한다. #258 기록과 현재 QEMU·WSL 커널 버전 문자열이 일치한다. 기본 계측은 블록 번역의 시작과 끝을 쌍으로 제공하지 않는다. 일반 계정으로 스케줄 전환 이벤트와 추적 시계를 읽을 수 없었다. 공통 시계, 게스트 카운터 정렬 오차와 관측 비용도 확인되지 않았다. 원래 P/A 실행 파일 해시는 보관 목록과 일치하며 IRQ·선택·복귀·사용자 카운터 읽기의 정적 주소를 확인했다. 정적 주소만으로 세대나 실제 후속 실행 경로는 정할 수 없다. 게스트 실행은 0회이며 과거 미달의 원인은 미정이다.

Public transcripts omit local account names and group membership. The unedited local capture is retained.

[Host capability transcripts / 호스트 관측 확인 기록](native-window-host-observability.zip) · SHA-256 744a98b5283c66ba8efd95ecdfcd08ebf1e3b850556b78a4c41b552aa816bebb.
