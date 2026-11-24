# Pi 4 RR generation-bound mailbox operations / Pi 4 RR 세대별 메일박스 명령 관측

On QEMU 11.1.3 raspi4b, the first cold sequence O1/E1/E2/O2 returned 0/0/0/0 under each image’s own contract. Both engineering boots showed A releasing a positive payload in generation 5 and B acquiring that same payload in generation 6. Both preserved the ambiguous v2-style outer bracket. Replaying the **original two v2 boots** with both the parent and candidate decoders reproduced RC2 for each. The independent loop-progress criterion also remained incomplete in both new boots.

QEMU 11.1.3 raspi4b 최초 콜드 부팅 O1/E1/E2/O2는 각 이미지 기준에서 0/0/0/0이었다. 두 공학용 부팅 모두 세대 5에서 A가 공개한 양수를 세대 6에서 B가 읽었다. 두 실행에서 기존 외부 시간 구간은 여전히 모호했다. **기존 v2 최초 부팅 2건**은 부모·후보 디코더 모두에서 각각 RC2를 재현했다. 새 부팅에서도 두 작업의 후속 산술 진행을 모두 요구하는 기준은 미충족이다.

| First engineering boot / 공학용 최초 부팅 | Matching payload / 전달값 | Wide A/B ticks / 외부 구간 길이 | A immediately after HIGH / HIGH 복귀 후 A 진행 | Later arithmetic A/B / 교대 후 산술 표본 |
|---|---:|---|---|---|
| E1 | 1,867,983 | 936 / 24,325 | 15,594,811 → 15,594,812 | A observed, B unobserved / A 관측, B 미관측 |
| E2 | 1,800,445 | 954 / 24,977 | 15,578,708 → 15,578,709 | Both unobserved / 둘 다 미관측 |

The independent ERET/IRQ/SVC ledger joined each captured exception to its selected frame. In each engineering boot, the peer IRQ saved B at 0x816e0 and ERET later resumed at that address, before the three instructions bracketing B’s successful mailbox load. The two inner GEN reads resolved to 0x85b58 via the linked NOP/ADR relaxation. A’s STLR at 0x817b0 and B’s LDAR at 0x816fc used mailbox 0x89ea8, separately corroborated by the address-register copy. The outer A interval stayed in generation 5 while the outer B interval spanned 4–6. Inner observations supply dispatch generations only. The operation SP was 0 bytes below A’s stack top and 192 bytes below B’s in both boots.

독립 예외 장부에서 B의 IRQ 저장 PC와 후속 ERET 복귀 PC가 두 실행 모두 0x816e0으로 일치했다. 이는 B의 메일박스 판독을 감싼 세 명령보다 앞이다. 세대 판독 주소는 0x85b58이고 실제 두 메일박스 명령은 0x89ea8을 사용했다. 외부 시간 구간은 A에서 세대 5, B에서 세대 4–6이었다. 내부 관측은 명령 시각 대신 디스패치 세대를 제공한다.

The engineering image added 480 text bytes, 10 read-only data bytes and 40 BSS bytes over v2. Data and reserved stack sizes matched. Ordinary RR, v1 FIFO/RR and v2 RR raw images matched their parent counterparts byte for byte. Default and directed-wake SMP ELF files also matched. Eight malformed records returned 1, the valid ambiguous control returned 2, and seven invalid build configurations were rejected. Historical v1 replay returned FIFO/RR 0/0 in the original cohort and 0/2 in the integrated cohort. Direct kernel loading and packaged firmware checks establish model coverage only. Physical Pi boot, stack high-water and uninstrumented timing remain open.

v2 대비 공학용 이미지의 코드가 480바이트, 읽기 전용 데이터가 10바이트, BSS가 40바이트 증가했다. 일반 RR·v1 FIFO/RR·v2 RR 원시 이미지가 각각 부모와 일치했다. 손상 대조 8건은 코드 1, 모호성 대조는 코드 2였고 잘못된 빌드 구성 7건이 거부됐다. 실물 Pi 부팅·최대 스택 사용량·비계측 실행 시간은 미확인이다.

[Machine-readable measurements / 측정값](pi4-rr-inner-first.json) · [Raw captures and replay / 원본과 재생](pi4-rr-inner-first.zip) · [Original v2 cohort / v2 최초 기록](pi4-rr-handoff-integration.en.md) · [Frozen board plan / 고정 설계](../boards/pi4-rr-handoff.en.md) · [POL-002](../scheduling-release-cases.en.md)

Signed by role / 서명 역할 — independent execution and verification reviewer / 독립 실행·검증 담당.
