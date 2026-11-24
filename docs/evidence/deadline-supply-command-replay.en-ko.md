# Independent documented command verification / 공개 명령 독립 검증

Tested b816c26b54d98526cfc0f3f97505f19abc75e19d on WSL ext4 with the published literal Bash recipe, one Cortex-A72 QEMU virt/GICv2 TCG vCPU, 128 MiB. The original aggregate was 0. Original UART and ELFs were lost when WSL cleared /tmp before the copy to D:. first-observed.en-ko.md retains observed terminal data. The second invocation used persistent ext4 TMPDIR. 원본 바이트를 보존하지 못한 첫 실행과 원본 보관에 성공한 재실행을 분리했다.

Second aggregate / 재실행 종료 코드 0.

|Position|Mode|Selector|Build/QEMU/replay|UART terminal|Reason or A progress|
|---|---|---|---|---|---|
|1|trace|ordinary-edf-supply-refuse|0/124/0|ORDINARY DEADLINE REFUSED BEFORE LAUNCH|CONFIG 62500000 6285025 6250000 0 0 8 16 832|
|2|trace|ordinary-llf-supply-refuse|0/124/0|ORDINARY DEADLINE REFUSED BEFORE LAUNCH|CONFIG 62500000 6280016 6250000 1 0 9 16 832|
|3|trace|ordinary-edf-supply|0/124/0|ORDINARY DEADLINE FINISHED|A_PROGRESS 591220 591220 898735|
|4|trace|ordinary-llf-supply|0/124/0|ORDINARY DEADLINE FINISHED|A_PROGRESS 610608 610608 892262|
|5|plain|ordinary-edf-supply-refuse|0/124/0|ORDINARY DEADLINE REFUSED BEFORE LAUNCH|CONFIG 62500000 6291156 6250000 0 0 8 16 832|
|6|plain|ordinary-llf-supply-refuse|0/124/0|ORDINARY DEADLINE REFUSED BEFORE LAUNCH|CONFIG 62500000 6285567 6250000 1 0 9 16 832|
|7|plain|ordinary-edf-supply|0/124/0|ORDINARY DEADLINE FINISHED|A_PROGRESS 590579 590579 885520|
|8|plain|ordinary-llf-supply|0/124/0|ORDINARY DEADLINE FINISHED|A_PROGRESS 593260 593260 841777|

All sixteen archived ELF/UART hashes were independently recomputed after copying. See second/*/SHA256SUMS. 정상 및 거부 결과의 UART, ELF, 예외 로그, 재생 JSON과 종료 코드를 모두 보관했다.

dcbf2251887bcaca73a040eb7774572e659d396d changes five documentation/evidence files. Kernel, tests, trace CSV and literal command match the tested checkout. b816c26 adds only the recipe document over 608de71. Both graph tools exited 0. The diff adds 17 typed links, with three requirement chains, two design links and three implementation, test and policy evidence links. The benchmark tools/qemu_boot.py matches main 8609aab and requires its fixed deadline transcript after the boot prefix. EVD-095 historically records bacd8dc with LLF plain failure, 5/6 host checks, and the interrupted overload capture and successful retry. The corrected archive preserves eight truncated acquisitions, eighteen before-guest failures, eight complete supply captures and ten complete legacy controls. 17개 유형 연결과 원래 실패 기록, 수정 후 별도 증거를 확인했다.

Both language receipts resolve their local documents and CRC-valid ZIPs. Digests are in receipts.txt. EVD-096 at held #251 28050f99867ad86a40689127af43b041f1c2f304 reports default-clock candidate P exceeding 62,500 ticks in 3/5 boots. All 18 scheduling release decisions remain open in both languages on both branches. 두 언어에서 18개 출시 판정이 모두 열려 있다. #251의 P 게이트는 보류 상태다. Physical/product timing was outside this QEMU run.

[Independent replay of the documented EDF and LLF command](deadline-supply-command-replay.zip) · SHA-256 0abe2f4ea7599a13c6255dc43488a692ac75f6e02fd15d9f1170135b5b23cad1.
