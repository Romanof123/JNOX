# AArch64 libX11 and Xtrans guest results

Revision `96af1678682d700629fd2ea4fdcfd571c4b3c2e4` built libX11
1.8.10 from the pinned upstream archive with the JNOX Unix-only,
single-threaded graphics configuration. The build used the host C compiler
for the original `makekeys` generator and the target compiler for the
library. Configure detected `sys/ioctl.h`. The two local changes to Xlib
preserve the XCB error assertion and make the header available to the
single-threaded caller. [Procedure](README.md) · [Server acceptance #448](https://github.com/Romanof123/JNOX/issues/448)

| Check | Observed result |
| --- | --- |
| Rust AF_UNIX broker controls | 7 passed, 0 failed |
| Pinned Xtrans byte-count and vectored stream guest | Caller exit 0, broker exit 0 |
| Complete libX11 build | `make -j1` returned 0 |
| Archive member headers | 408 of 408 were AArch64 |
| Static archive SHA-256 | `c4573dcfa02d048e26134854672ff6313a8bbf6cfcaa85ee840624975a424119` |
| App linked to the archive | `XParseGeometry` checked sizes, signed offsets, omitted dimensions and invalid input. Guest exit 0 |

The library test ran before opening a display. Xwayland startup, successful
X11 display setup and rendered Mosaic output remain separate acceptance
criteria. The X11 cookie tests have their own five-process results.
[Authorization guest results](https://github.com/Romanof123/JNOX/issues/465#issuecomment-7211285104)

## Public candidate replay

The published candidate at `28b0ebb085ffe8a288e1a85b6d1dfce07220a111`
has the same Git tree as the separately rebuilt revision `f822231`.
Its AF_UNIX broker passed all seven host tests. The two-process Xtrans
guest and the separately linked Xlib app both passed. The strengthened
gate checked each ELF member's architecture and relocatable type, and
compared the complete sorted member list against the original 408-object
inventory. The two runs produced different archive hashes and the same
inventory. Byte-for-byte archive reproducibility remains unverified.
[Archive report](evidence/pr-candidate-archive.json) · [Selected terminal records](evidence/pr-candidate-exits.txt)

## 한국어

같은 리비전에서 브로커 시험 7개와 원본 Xtrans의 AArch64 게스트 시험을
통과했다. `libX11.a` 전체 빌드가 성공했고 오브젝트 408개의 ELF
아키텍처가 모두 AArch64였다. 이 파일을 정적 링크한 앱이 크기, 음수 좌표,
크기 생략, 잘못된 문자열에 대한 `XParseGeometry` 결과를 확인하고 0으로
종료했다. 디스플레이 연결과 창 렌더링은 전체 서버 시험에서 추적한다.

공개 후보의 동일 소스 트리도 별도로 재빌드해 두 게스트 시험을 통과했다.
408개 오브젝트의 이름 목록과 각 ELF의 아키텍처·유형을 확인했다. 두 빌드의
아카이브 해시는 달라서 바이너리 단위 재현성은 별도 검증이 필요하다.
