# PM inventory of open engineering work / PM 미종결 작업 추적 기록

At 2026-11-24 18:06:40 UTC (Nov 25 03:06:40 KST), the public GitHub issues endpoint returned one complete page containing 31 open issues and 15 open pull requests. The collector followed REST next-page links to exhaustion. Main was `7db361161f6943f5b330ba1d2b8de495295d538d`. Each item had one recognized team. Each PR and its primary tracking issue cited one another in their bodies. The PM checked primary ownership separately from the automated reciprocal-mention check. A team label identifies the next owner. The product authority records release approval separately.

2026년 11월 25일 03시 06분 40초(KST)에 공개 GitHub의 열린 이슈 31건과 PR 15건을 전부 수집했다. 모든 항목에 다음 담당 팀이 하나씩 있으며, 모든 PR에서 주 이슈와 양방향 본문 링크를 확인했다. 아래 표는 해당 시각의 기록이다. 제품 안전 승인은 제품 결정권자의 별도 기록에 따른다.

| Pull request | Primary tracking issue / 주 이슈 | Next PR owner / 다음 PR 담당 |
| --- | --- | --- |
| [#126](https://github.com/Romanof123/JNOX/pull/126) | [#123](https://github.com/Romanof123/JNOX/issues/123) | team:integration |
| [#133](https://github.com/Romanof123/JNOX/pull/133) | [#99](https://github.com/Romanof123/JNOX/issues/99) | team:integration |
| [#135](https://github.com/Romanof123/JNOX/pull/135) | [#99](https://github.com/Romanof123/JNOX/issues/99) | team:systems |
| [#138](https://github.com/Romanof123/JNOX/pull/138) | [#123](https://github.com/Romanof123/JNOX/issues/123) | team:integration |
| [#153](https://github.com/Romanof123/JNOX/pull/153) | [#123](https://github.com/Romanof123/JNOX/issues/123) | team:integration |
| [#172](https://github.com/Romanof123/JNOX/pull/172) | [#123](https://github.com/Romanof123/JNOX/issues/123) | team:integration |
| [#178](https://github.com/Romanof123/JNOX/pull/178) | [#177](https://github.com/Romanof123/JNOX/issues/177) | team:development |
| [#192](https://github.com/Romanof123/JNOX/pull/192) | [#123](https://github.com/Romanof123/JNOX/issues/123) | team:development |
| [#201](https://github.com/Romanof123/JNOX/pull/201) | [#123](https://github.com/Romanof123/JNOX/issues/123) | team:development |
| [#204](https://github.com/Romanof123/JNOX/pull/204) | [#47](https://github.com/Romanof123/JNOX/issues/47) | team:verification |
| [#207](https://github.com/Romanof123/JNOX/pull/207) | [#49](https://github.com/Romanof123/JNOX/issues/49) | team:development |
| [#213](https://github.com/Romanof123/JNOX/pull/213) | [#94](https://github.com/Romanof123/JNOX/issues/94) | team:development |
| [#218](https://github.com/Romanof123/JNOX/pull/218) | [#217](https://github.com/Romanof123/JNOX/issues/217) | team:verification |
| [#219](https://github.com/Romanof123/JNOX/pull/219) | [#216](https://github.com/Romanof123/JNOX/issues/216) | team:verification |
| [#222](https://github.com/Romanof123/JNOX/pull/222) | [#220](https://github.com/Romanof123/JNOX/issues/220) | team:verification |

[PM lifecycle and handoffs](../lifecycle.en.md) · [한국어 변경 및 인계 절차](../lifecycle.md) · [PM issue #6](https://github.com/Romanof123/JNOX/issues/6) · [Scheduling #21](https://github.com/Romanof123/JNOX/issues/21) · [IPC sequencing #24](https://github.com/Romanof123/JNOX/issues/24).

Extract the archive and run `python -B tools/open_tracking.py open-items.json` from this source tree. The original response, checker output and PM mapping are included. 원자료, 도구 판정 및 PM 확인 매핑을 첨부한다.

[Original inventory and audit / 원본 목록과 검사 결과](pm-open-tracking-20261124T180640Z.zip) · ZIP SHA-256 `8b5dc5dd5853cfb7d7e2f7375561f10a5b3c34f53c57e8600424cd0186704d55`. Raw JSON SHA-256 `ea6a3b21c561c1f718e6a37d16cc956332266a0a4ee077f3d820b797b3778468`.
