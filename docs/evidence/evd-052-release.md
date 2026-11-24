# EVD-052 · 마커 릴리스 이미지 비교 및 여섯 가상 부팅

[EVD-052](evd-052.md) · [#124](https://github.com/Romanof123/JNOX/pull/124) · [3d314b0](https://github.com/Romanof123/JNOX/tree/3d314b0a188b1e93f1b2a6ad9ee35a47c30a7c9b) · [JSON](evd-052-release.json) · [ZIP](evd-052-release.zip)

이미지 13개의 직접 참조 13개는 ELF 전체가 일치했다. 비교 24건 중 18건은 ELF 전체와 적재 세그먼트가 같았고 24건 모두 .text가 같았다. #122의 RM·DM 기본/기술자/콜드 여섯 이미지와 비교하면 이미지당 적재 바이트 7~9개가 달랐다. 변경 위치는 control_runtime.rs 소스 줄 번호 메타데이터였다. 잘못된 RM cold+descriptor, descriptor+marker, marker+cold 및 marker-loss 선택자 4개가 예상대로 거절됐다.

변경된 이미지마다 새 QEMU 프로세스를 한 번씩 실행했다. 6회 모두 종료 표시에 도달했으나 운영 적격은 0/6이었다. RM·DM 기본 이미지는 위상 시간 기준, 두 기술자 이미지는 실제 허가 전달에서 실패했다. 두 콜드 진단 게스트는 시험 IRQ가 없는 고정 기준 모드를 확인했다. 일반 healthy 점수 도구는 각 UART의 네 번째 줄에서 진단 사건 48을 지원하지 않아 거절했다. 이 두 판정은 진단 시작 실패나 별도의 운영 시간 위반을 입증하지 않는다.

ab14de0의 엄격 감사는 IMP-052 근거 경로 누락으로 HOLD(종료 1)였다. 근거 추가 뒤 bf07551, 최종 소스 3d314b0에서 각각 18개 정책, 발견 0건, 종료 0을 기록했다. 원래 EVD-052 ZIP의 파일 해시 147개와 이전 HOLD 기록 해시도 일치했다. 과거 실제 마커 저널 4개에 대한 판독기 비교와 이번 게스트 6회는 서로 다른 실험이다.
