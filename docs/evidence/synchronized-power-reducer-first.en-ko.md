# JNOX #244 / #243 independent offline verification · 독립 오프라인 검증

Signed / 서명: Codex independent verifier power244_verifier, 2026-11-25 KST. Release lane / 판정 범위: host-only arithmetic and fixture provenance. 호스트 산술과 fixture 바이트 식별에 한정한다. All acquisitions are invented fixtures. Physical acquisition, guest execution, calibration, product limits and safety release remain outside this review. 실물 취득·게스트 실행·교정·제품 허용값·안전 출시는 검증 대상 밖이다.

Frozen source 27199e9d28c4938d3370ccb0d97b3fa6a6d147df merged cleanly in a detached checkout with main a3e68327e31e8a70befd766b3d374bf86521d46b. Local merge 3c6be6ee3559ba2cd8e4f7c2eb0b1cbdc34b9e93, tree 78d7b1a6063e6ed529229355f41460908803e74b. Relative to main only both M3 docs and the two reducer Python files differ. Both prospective docs are byte-identical to f5efc974dab57fdcf509994944d14a96ce0a2d3a. 고정 main과 실제 병합했고 선행 한영 기준서의 동일성을 확인했다.

Synthetic edge at meter 17 ns / counter 100, frequency 3 Hz and recorded uncertainty 7 ns. Relative to that edge, idle occupies [0,1/3] s, run [2/3,5/3] s. Sample spacing is 1 s, equal to the prospective bound. Linear endpoint power P=1+t gives idle 7/18 J, baseline 7/6 W, gross 13/6 J and net +1 J. P=4−t gives 23/18 J, 23/6 W, 17/6 J and −1 J. µV × µA × ns scales by 10⁻²¹ J. Positive, negative and zero margins gave +1/3, −1/3 and 0 seconds. Only the negative case recorded a miss. Wake delay was 1/3 s and response 1 s. Truncating the same synchronized series preserved 1/3 s run coverage out of 1 s, fully covered idle, produced exit 1 and withheld all energy outputs. Accepted reports reproduced the original context, producer, class and hashes. Digest agreement identifies bytes; a physical custody chain requires independent acquisition records.

3 Hz의 분수 틱과 17 ns 공통 오프셋에서 두 구간의 경계 보간, 서로 다른 구간 길이로 계산한 유휴 기준선과 보정 에너지 부호를 확인했다. 기한과 완료 시각이 일치할 때 위반은 없었고 한 틱 늦으면 위반이었다. 같은 fixture의 마지막 샘플을 빼면 실행 1초 중 1/3초만 확보되어 종료 코드 1과 에너지 값 생략을 확인했다. 보고서의 파일 해시와 문맥도 원본 바이트와 대조했다.

Independent CLI 4/4 as expected. Repository reducer 5/5 (including hash mismatch, interior gap, duplicate time, documented falling baseline and partial window), graph suite 6/6, graph checker exit 0, Rust host power tests 9/9. No arithmetic discrepancy observed. 독립 CLI 네 건, 기존 리듀서 다섯 건, 그래프 여섯 건과 검사기, Rust 전력 호스트 아홉 건에서 기대 결과를 얻었다.

## Replay · 재현

From the merged checkout with the tree above, set EVIDENCE to the extracted ZIP directory. The script uses Python 3 and no external modules. 아래 명령은 위 병합 트리에서 실행하며 EVIDENCE는 압축을 푼 경로다.

```powershell
python "$env:EVIDENCE/replay.py" .
python -m unittest discover -s tools -p test_power_reduce.py -v
python -m unittest discover -s tools -p test_requirement_graph.py -v
python tools/requirement_graph.py
rustc +1.95.0 --edition 2024 --test tools/scheduling_tests.rs -o "$env:TEMP/jnox-independent-244.exe"
& "$env:TEMP/jnox-independent-244.exe" power --test-threads=1
```

[Independent known-answer measurements and replay logs](synchronized-power-reducer-first.zip) · SHA-256 0cd4078b8d5bee630e848444c3b681d9ae3ac4eebd1acffcd18076948c1dba51.
