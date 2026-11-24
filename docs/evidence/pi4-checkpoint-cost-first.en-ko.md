# #283 independent paired TLS checkpoint evidence / TLS 체크포인트 독립 검증

Integration parent 9207337f95edfd7365a3cabf39b03d58607181f6, candidate 2545d64ed05e5945b9258d8368920f2cad2d4d3e, public predeclared plan b79d8e4ba70c7ac9c90ddba2b725c4db514fb522. Both trees use MG3. The original #279 release used ce8aebc872612cb036c05928fc8764d947d76145. This pair excludes #281. Twelve builds from exact Git archives used identical paths/toolchain, separate clean targets for each side, and the same release options. See pairs.sh, pairs.log, isolated.patch, source-tracking files, and per-image freeze.json for hashes, stock-QEMU identity, observer and ELF/raw inputs. input-contract.sha256 predates candidate inspection. scored-freeze.sha256 predates all 12 scored boots.

## Linked path / 링크 경로

Each active M prologue was executed from its linked entry to the first acquire. At that boundary the ordinary third-block state was supplied: one stable lease, count two, two sparse rows retained, independent prefix and digest through block two. Unicorn 2.1.4 executed every instruction through the return to the same acquire. The third prefix, digest and count matched independent arithmetic. Branch operands/NZCV and ELF/raw checked opcodes are in per-image spans.json and executed.tsv.gz. memory.tsv.gz records effective stack and TLS accesses including aliases. linked.json records symbol addresses/sizes, image by image; public.py separately passed all six inputs through the candidate's path verifier. It validates the submitted path, direct edges and SP-literal accesses. It requires independent predicate, alias and data-flow evidence.

|Workload (all three policies)|Instructions before → after|Calls/returns|Conditional executions|Direct SP loads/stores before → after|.text bytes|Raw bytes|
|---|---:|---:|---:|---:|---:|---:|
|Short|250 → 245|4/4|29|12/8 → 11/8|20,468 → 20,468|53,448 → 53,448|
|Stream|16,490 → 16,485|4/4|1,035|12/8 → 11/8|20,468 → 20,468|53,448 → 53,448|

All six baseline helpers save the indirect destination X8 into X19, then STP X0,X8,[X19] and STR X1,[X19,#16] materialize three 64-bit words; the caller reloads the word pointer from its stack. All six candidate helpers retain the activation lookup and both live MRS reads with CMP/CCMP, return the validated pointer in X0, and the caller performs LDR [X0], rotate/XOR/add, STR [X0]. The ordinary candidate path has no extra spills or conditional executions and no alias stores into an indirect result. The helper saves/restores X30 alone instead of X30/X19 together. Stack reads/writes through the arithmetic X21 cursor remain present as in the baseline. checked_registers is inlined, and update_digest is fused into checkpoint. Symbol sizes of retained helpers and terminal paths are in linked.json. The whole-image ELF/raw section and instruction bytes matched independently. All 78 linked helper probes (13 per candidate image) reached correct valid returns or failure 0x34/0x33 before any TLS-word access.

동일한 제3 블록의 acquire에서 다음 acquire 전까지 실행한 결과 여섯 쌍 모두 명령 5개와 SP 직접 읽기 1개가 줄었다. 세 워드 간접 반환 저장과 호출자의 포인터 재읽기가 사라졌다. 두 TLS 레지스터의 실제 읽기·검증, 활성화 조회, 다이제스트 갱신은 유지됐다. 링크된 기계어에서 측정한 수치이며 실제 경과 시간과 실물 보드 성능은 측정하지 않았다.

## Original positions / 최초 순번

|Workload/policy|#|CPU/lease from → to|IRQ = ERET|Strict bounds [lo,hi)|Frame / SP / X21|Resumed index, product|Strict|
|---|---:|---|---|---|---|---|---|
|short-global|1|0/1 → 1/4|0x80190|0x82318, 0x8233c|0x90cd0 / 0x91010 / 0x60f723e8|57040, 1626809320|miss|
|short-global|2|0/1 → 1/4|0x82038|0x82318, 0x8233c|0x90cc0 / 0x91000 / 0x5e886e00|56320, 1585999360|miss|
|short-partitioned|1|0/1 → 0/5|0x8231c|0x8231c, 0x82340|0x90d70 / 0x910b0 / 0x910c0|60704, 1842518160|yes|
|short-partitioned|2|0/1 → 0/5|0x8203c|0x8231c, 0x82340|0x90cc0 / 0x91000 / 0x6ac69968|59856, 1791400296|miss|
|short-clustered|1|0/1 → 0/5|0x823b4|0x823c8, 0x823ec|0x90d80 / 0x910c0 / 0x0|55968, 1566236496|miss|
|short-clustered|2|0/1 → 0/5|0x823c8|0x823c8, 0x823ec|0x90d70 / 0x910b0 / 0x910c0|56416, 1591410736|yes|
|stream-global|1|0/1 → 1/4|0x80d0c|0x80d0c, 0x80d4c|0x94540 / 0x94880 / 0x94890|92160, 1578889643|yes|
|stream-global|2|0/1 → 1/4|0x80d0c|0x80d0c, 0x80d4c|0x94540 / 0x94880 / 0x94890|74752, 2120520456|yes|
|stream-partitioned|1|0/1 → 0/5|0x80d0c|0x80d0c, 0x80d4c|0x94540 / 0x94880 / 0x94890|119808, 799373509|yes|
|stream-partitioned|2|0/1 → 0/5|0x80d0c|0x80d0c, 0x80d4c|0x94540 / 0x94880 / 0x94890|142336, 1109154100|yes|
|stream-clustered|1|0/1 → 0/5|0x80d0c|0x80d0c, 0x80d4c|0x94540 / 0x94880 / 0x94890|103424, 249037660|yes|
|stream-clustered|2|0/1 → 0/5|0x80d0c|0x80d0c, 0x80d4c|0x94540 / 0x94880 / 0x94890|118784, 3690937743|yes|

Short 2/6, stream 6/6. All twelve reached their terminal products. All saved IRQ PCs matched external ERET and frame/SP/X21 continuation. Four short misses had out-of-loop saved PCs. No early completion. Historical #279 short 3/6 and stream 6/6. The new short sample has one fewer qualifying position. Global resumed on CPU1, partitioned and clustered on CPU0. positions.json records actual saved/live TLS, first resumed digest and terminal count/digest/index/product per original. For short the terminal tuple is 4096 / 18080812676020141577 / 65536 / 2147516416. For stream it is 4096 / 8765611335505774534 / 4194304 / 2027039441 (CRC32 0x78d22ad1). Independent zlib/sum/prefix and all four activation outcomes are checked by independent.py.

짧은 작업 2/6, 스트림 6/6. 첫 순번을 교체하지 않았다. 짧은 작업의 네 번은 IRQ와 복귀가 실제로 관측됐으나 엄격한 산술 구간 밖에 있었다. 조기 종료는 없었다. #279의 3/6과 비교해 짧은 작업의 적격 순번은 하나 줄었다. #47 상위 이주 판정 및 실물 보드 결과는 이 자료의 범위 밖이다.

## Controls / 대조 검증

The integrated reader replayed all 30 archived positions (MG1 1/6, MG2 0/6, original CRC 6/6, MG3 short 3/6 and stream 6/6). Historical early completions retain their original dispositions. Four resealed semantic corruption cases were rejected. Ten MG3 mutations rejected with the positive witness accepted. Nineteen independent host vectors passed. Fresh generic SMP and SW1 stock-model controls passed; ownership listings are included. The initial bounded SW1 offline replay lacked pyelftools; reopening the identical sealed session with the dependency yielded observed software and matched CPU-tagged PCs. All-feature and invalid configuration build logs are retained. No physical-board or wall-clock speed claim.

과거 원본 30개와 네 계열의 손상 대조, MG3 증인 변조 10개 및 호스트 테스트 19개를 확인했다. 일반 SMP와 SW1도 새로 실행하고 링크된 레지스터 사용을 대조했다. SW1의 최초 판독 환경에서 누락된 라이브러리를 복구한 다음 동일한 원본을 재판독했다.

[Paired linked checkpoint paths and twelve sealed boot positions / 링크 경로 대조와 최초 부팅 원본](pi4-checkpoint-cost-first.zip) · SHA-256 3be34eeddd14bf3ec649fc92e974eb523acc629e69aaf2db5a79b37fd17e1187.
