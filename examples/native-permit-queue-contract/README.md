# Queued permit expiry regression / 만료 시 대기 중인 허가 검증

This AArch64 regression links the Rust kernel's safety and loan services and their ring into one native test process. Its three role IDs, 62.5 MHz counter inputs and page mapping callbacks belong to the fixture. The host JNOX kernel schedules the test process normally. The exercise uses the service CHECK, receive, release and COMMIT paths, event 17, the atomic virtual output and the original ring metadata. [Acceptance #403](https://github.com/Romanof123/JNOX/issues/403).

| App argument | Operand supplied to CHECK | Expected result |
| --- | --- | --- |
| `before` | D−1 tick | A fresh sequence 4 advances the channel while enabled. |
| `exact` | D | Sequence 3 expires, output falls, subsequent COMMIT refuses sequence 4. |
| `after` | D+1 tick | The fault stays latched across receive, release and attempted COMMIT. |

D is the prior accepted wire issue plus 20 ms. All three traces start with verified generation 1, sequence 3 and an enabled output. A producer stages and publishes a valid 44-byte frame about 1 ms before D. Each case uses a fresh image. Exit 71 signals passage of the in-process assertions. The independent verifier inspects terminal RAM, image identity, the kernel-service journals and ring state.

```sh
app=examples/native-permit-queue-contract
sh sdk/native-aarch64/build-rust.sh "$app/Cargo.toml" worker.elf
python3 tools/build_native_el0.py --board virt --policy fifo --priority 7 \
  --elf worker.elf --arg exact --out fifo-exact
```

RR uses `--policy rr --quantum-ticks 625000`. Capture the image through `tools/native_el0_capture.py`. The separate three-process arrival images use their hardware counter, actual process identities and MMU-backed loans. [Three-process fixtures](../native-safety-channel/README.md) · [Release case #401](https://github.com/Romanof123/JNOX/issues/401).

실제 커널의 허가·대여 서비스 소스를 단일 네이티브 시험 프로세스에 링크한다. 역할 ID, 시간 입력, 페이지 매핑 동작은 시험용이다. 유효한 네 번째 프레임을 이미 큐에 넣은 뒤 만료 한 틱 전·정각·한 틱 후를 각각 검사한다. 검증팀은 이미지 해시, 커널 서비스 저널, 큐 선두와 출력 레지스터를 종료 RAM에서 대조한다. 실제 세 프로세스와 하드웨어 카운터의 만료 정각 실측은 #401에서 따로 추적한다.
