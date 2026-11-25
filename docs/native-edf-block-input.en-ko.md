# Operator-selected block count for the native EDF batch / 네이티브 EDF 배치의 블록 수 지정

[#298](https://github.com/Romanof123/JNOX/issues/298) follow-up · CHG-124 / IMP-124 / EVD-124. Prospective on main `dea5772db92b8e9139cbe83a2bd5d37d1a600cc3`, once [#310](https://github.com/Romanof123/JNOX/issues/310) per-slot arguments are integrated. A sensor operator can run exactly N complete 4,096-byte blocks using one compiled batch ELF. Only the C application's input handling changes; the existing EDF scheduler and native ABI already supply the required execution path. [Batch source](../examples/native-edf/batch.c)

센서 운영자가 같은 배치 ELF로 4,096바이트 블록을 지정한 개수만큼 처리한다. #310의 슬롯별 인수를 사용하며 EDF 스케줄러와 네이티브 ABI는 기존 경로를 따른다.

## Application contract / 앱 입력과 결과

With argc=1 preserve the original minimum-160 ms mode: take the initial monotonic sample before loading input, check time after each complete block, retain the 65,536-block cap, success 17 and existing failures 101–104. With exactly two extras, require the literal --blocks and a nonempty ASCII decimal N in 1…65,536 (leading zeros permitted). Reject signs, whitespace, non-ASCII digits, trailing bytes, extra/missing fields, zero, and values above the limit without integer wrap. For invalid input return 105 before loading samples or updating either private output. This is an application result after a successful package and boot.

In fixed mode initialize the same volatile sensor ramp once. Execute exactly N calls to the existing batch_reduce, with no clock read or other syscall from entry into the reduction loop through its last iteration. After each full block add its sum to volatile u64 private_result, then publish the completed count in volatile u32 private_progress. Return 17 when the final sum equals 522,240×N; return 103 on a product mismatch. The normal SDK exit follows the computation.

추가 인수가 없으면 기존 160 ms 시계 기반 모드와 상한·종료값을 보존한다. 정확히 두 인수 --blocks N일 때 ASCII 십진수 1~65,536만 허용한다. 선행 0은 허용하고 부호·공백·잘못된 개수·범위 초과는 작업 시작 전 105로 종료한다. 고정 모드는 블록 사이에도 시계를 읽지 않는다. 전체 합을 먼저 저장한 뒤 완료 블록 수를 게시하며 성공은 17이다.

## Frozen inputs and observations / 고정 입력과 판정

~~~sh
python3 tools/build_native_el0.py --out edf-fixed --policy edf \
  --elf batch.elf --arg=--blocks --arg 32768 --job 0,8,2 \
  --peer-elf histogram.elf --peer-job 0,12,8 \
  --peer-elf urgent.elf --peer-job 1,4,1
~~~

Freeze the three ELF hashes and disassemblies, package arguments, source revision, private symbol addresses, QEMU version/command, capture capacity and timeout before launch. Run two independent cold boots in order, plain then externally exception-traced, on one Cortex-A72, virt/GICv2, TCG, 128 MiB guest RAM and default clock. A reads 128 MiB cumulatively by reusing its 4 KiB ramp. Its exact outputs are 32,768 and 17,112,760,320. Unchanged Rust B must publish 128 complete blocks and sixteen bins of 32,768 (exit 29); unchanged C must publish eight blocks and 2,048 exceedances (exit 43). Collect complete private results and signed exits.

Require A→C→A→B, with grid-1 PPI30 interrupting live A inside batch_reduce. Map the saved ELR to A's disassembly and record its SP and private completed count. Correlate the external exception entry and the actual return after C's exit with the same A PC/SP in its own mapping; show later advancing private progress and the full final product. Distinguish native scheduling records from externally witnessed return. At Q=100 ms compare each actual exit sample to S+8Q, S+12Q and S+4Q for A, B and C. Report observed deadline misses, a different interrupted instruction, an early finish, skipped samples or missing return evidence separately; retain the first captures without tuning input sizes or substituting retries.

Use the identical batch ELF for these controls: one block must produce (1,522240) and exit 17; --blocks 65537 and --blocks 2x must each exit 105 with both outputs zero; no extra arguments must use the original clock path and produce (n,522240×n) on success at or after its 160 ms threshold. Preserve the original #299 positions, including the externally traced clock-helper interruption, as their own evidence. Any additional qualification applies only to the named fixed-input configuration. [Original first evidence](evidence/native-edf-applications-first.en-ko.md)

A의 입력은 32,768블록(누적 읽기 128 MiB), 기대 합은 17,112,760,320이다. 일반 콜드 부팅 후 별도 외부 예외 추적 부팅을 실행한다. 격자 1에서 A의 실제 계산 루프를 선점하고 C 종료 후 같은 PC·SP로 A가 복귀하는지, 사설 진행량과 세 앱의 전체 결과를 확인한다. 실제 마감은 슬롯별로 따로 판정한다. 같은 ELF로 1블록·잘못된 인수·무인수 모드를 대조한다. 기존 #299 추적의 시계 함수 선점과 최초 결과는 그대로 보존하며, 이번 실측의 판정은 명시한 고정 입력에만 적용한다.
