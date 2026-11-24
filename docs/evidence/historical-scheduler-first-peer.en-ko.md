# Independent review of JNOX #235 · 독립 검증
Head 173260958cba2d36b3db2ecbc9ce3a9708163ee3. Published evidence ZIP SHA-256 865128a0e911908503f74fa5f328455e70f9e64b8c70582504c79f38232c95f9.

Both original UART and exception streams match the pinned EVD-032 and EVD-047 records byte for byte. Both ELFs match their original launch-path files. #79 interrupted B/A/B/A and failed before another return. Two frame/status branches merge at 0x40202300. #110 recorded A FIRST and interrupted A at 0x40201278 before PART FAIL. Three checks merge at 0x40201964. The archives preserve neither the taken predecessor nor its operands. The linked listings contain 10 and 8 direct fail calls respectively. The #79 regenerated listing matches after ignoring the file path heading and line endings.

#79 baseline d19fa8f and integration tree a75da97 share a raw image digest. Their ELF hashes differ. #110 comparison commit 6fccd8a and frozen a57a0cb share the whole ELF. Git identifies e8f212e as the immediate parent of a57a0cb. POL-006 and POL-010 gained historical evidence in both catalogs. Extracted verification, graph, matrix and 16 scoped tests passed. Historical root cause and product risk acceptance remain open. Review used stored captures and static checks only.

두 최초 실행의 UART·IRQ 로그가 당시 공개 파일과 바이트 단위로 일치했고 두 ELF도 실행 경로 원본과 일치했다. #79는 B/A/B/A 순서에서 마지막 A가 중단된 뒤, #110은 A FIRST 이후 첫 IRQ에서 실패했다. 각 바이너리에서 서로 다른 검사가 같은 실패 호출로 모인다. 당시 선택한 분기와 피연산자는 복원할 수 없다. #79 비교는 원시 이미지와 통합 트리 기준이고 #110 비교 커밋은 실제 직전 부모와 다르다. 양 언어의 POL-006·010 변경, 추출 검증기, 그래프·행렬 검사와 범위 테스트 16개를 확인했다. 두 원인과 제품 잔여 위험 승인은 미결이다.

[Independent replay records / 검증 기록](historical-scheduler-first-peer.zip) · SHA-256 9d66dfe57fc4196a6e9cbda6b4c6faa4cd3001c35ca50bb52bfc36599adde3fc.
