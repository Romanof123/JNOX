# Scheduling evidence repair / 스케줄링 근거 추적 복구

[#205](https://github.com/Romanof123/JNOX/issues/205) records four findings on main `2a24bf993a4f326437ad93b1d84bc780410c1417`. VER-009 has a dangling EVD-090 target. EVD-058 lacks a tested implementation edge and both canonical language files. The general requirements graph passed its separate structural check.

기준 소스에서 VER-009의 EVD-090 대상 한 건과 EVD-058의 시험 구현 연결 및 한국어·영어 문서 세 건이 누락되었다. 일반 요구사항 그래프의 구조 검사는 당시 통과했다.

| Record / 기록 | Upstream subject and evidence / 원래 시험 대상과 근거 | Planned mapping / 연결 계획 |
| --- | --- | --- |
| EVD-058 | CHG-076 ordinary Rust AMC, single Cortex-A72 QEMU virt. Continuous source `c4a63bebc3b773550c630522ba3f62cd9be1a526` reached 1/3. Finite source `420eaaa2f7d85507b716cfb862fb2c7c137dfabf` and integrated source `6a336318e265785b5c0d8df3b5a8649632520919` each reached 3/3. The integration commit introduced the EVD-058 edges and the independent receipt names that exact source. [Original](evidence/ordinary-amc-first.en-ko.md) · [Finite and integrated](https://github.com/Romanof123/JNOX/blob/89eca30c3928b791fec7208bb51de71833a54b70/docs/evidence/ordinary-amc-finite-first.en-ko.md) | Assign IMP-058 here to the measured ordinary AMC runtime. Link DES-003/012, POL-015 and the existing partial VER-002/005/023 paths. 단일 코어 일반 AMC 구현에 추적 ID를 부여하고 세 집단의 원본과 제품 한계를 두 언어로 연결한다. |
| EVD-090 | VER-009 static ELF extent measurement. Original tool source `34454f955a30b0d3be0a4dfc73435a344f2dbff9` misclassified two layout controls. Corrected tool source `812cfd80c62b8936bfd53e5ff4ffe185195375a3` was checked against the original ELFs and linked controls. [First record](evidence/evd-090-virt-static-first.en.md) · [Corrected and blind](https://github.com/Romanof123/JNOX/blob/142907384f7de433801925276a9ceac984c2a640/docs/evidence/evd-090-virt-static-corrected.en-ko.md) | Add two canonical records for the existing VER-009 partial edge. 정적 ELF 도구 측정과 익명 대조를 연결하고 실행 시간·실기 근거는 미결로 둔다. |

Independent verification under [#6](https://github.com/Romanof123/JNOX/issues/6) should retain the original four findings, compare both checkers at the original and candidate revisions and use disposable missing-file and missing-edge controls. [#21](https://github.com/Romanof123/JNOX/issues/21) retains the product and physical release decisions. 독립 검증에서는 두 판정기의 수정 전후 결과와 문서·연결 제거 음성 대조를 보존한다. 제품 및 실기 출시 판정은 별도 추적한다.
