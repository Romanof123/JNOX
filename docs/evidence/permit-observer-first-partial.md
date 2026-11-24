# OFF 관찰 비용 · 중단된 최초 집단

[#144](https://github.com/Romanof123/JNOX/pull/144) · [CHG-061](../permit-observer-cost-plan.md) · [CLI](https://github.com/Romanof123/JNOX/blob/dc9f25897e661dae84be09d91c92dd713743d3a0/tools/permit_observer_cost.py) · [JSON](permit-observer-first-partial.json) · [ZIP](permit-observer-first-partial.zip)

고정 순서 16건 중 1건만 시작했고 나머지 15건은 실행되지 않았다. 8쌍 모두 R−F 차이를 산출할 수 없다.

01-rm-off-1-F는 UART 종료 표식에 도달했다. 수집기는 69바이트 QEMU SIGTERM 진단을 예상 밖 stderr로 분류해 미완료로 기록했다. QEMU 프로세스 상태는 0이었다. 당시 수집기는 이 판정에서 중지해 미실행 목록을 cohort.json에 쓰고 미완료 집단에 1을 반환한다. 원본 메타데이터에는 첫 관측·종료 표식·경보·정지의 호스트 시각과 값이 없는 여덟 쌍이 들어 있다.

원본과 해시가 일치하는 UART에서 첫 IRQ 지연 13,171틱, 첫 위상 지연 55,167틱, 첫 서비스 20,707틱을 계산했다. 사유 103과 쓰기 두 건의 전후 0 표본을 기록했다. 호스트 벡터 6건이 통과했고 별도 합성 자식 제어는 켜진 비트를 감지해 자식을 정지시켰다. 이 표본으로 게스트 출력의 전체 이력을 확정할 수 없다.

75,321바이트 메커니즘 스트림에서 첫 25행과 마지막 10행만 남았다. 전체 해시는 당시 집단 메타데이터에 기록된 값이고 2,302바이트 발췌본은 별도 해시를 가진다. 고정 입력 해시는 출력됐으나 입력 파일은 남아 있지 않다. 모델 이벤트 전체, 게스트 틱 대응, 관찰 비용 및 실물 동작은 미확인이다.

| File | Bytes | SHA-256 |
|---|---:|---|
| UART | 884 | ee5c5eef236d452dc545c37465affe91f372d23da5fc2be5f965a6af2b3706eb |
| stderr | 69 | bd17ff5d7d1a2061850aea97cbd32fd1ad7ae1747858792cc9fb7a7029c0530d |
| mechanism (original) | 75321 | 23ecdf8590a9a0a137e59fa9b1b705ef44a7b139334411d4f7d02bd26f9823db |
| mechanism (head 25 + tail 10) | 2302 | 528be28e0c7fd657987fb8e5fa37a2bb4acb98f4a0078b42cced11c6e9c6b097 |

보존 개선안은 고정 입력과 전체 스트림을 영구 저장하고 감독자가 요청한 종료 진단과 기타 stderr를 구별하는 것이다. 후속 집단은 별도로 집계한다. [#144](https://github.com/Romanof123/JNOX/pull/144)

```sh
unzip permit-observer-first-partial.zip -d first-partial
(cd first-partial && sha256sum -c SHA256SUMS)
```
