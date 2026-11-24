# EEVDF 통합 이미지 및 원본 최초 실행

통합 소스 0145c2342833fefd6d7cffea24b1785c6ba664f2의 텔레메트리 진입점 및 모든 PT_LOAD 헤더·바이트는 e203caa997eea8a561c89e59fb41994b64547214와 일치했다. 전체 ELF 해시는 달랐다. 통합 판독기로 최초 세션(68cca2efd813872eb02c6868fccbbe4e4e2ae2c22f76a602e4413c755bf0cf16)을 재생한 결과 0이었다. 84행·깨움 8건·서비스 창 5개를 충족했다. 원본 기록 재생으로 추가 게스트는 0회였다. 기존 EEVDF 벤치 3종도 e203caa 대비 진입점과 적재 이미지가 같았다. main 12a98a9178903798b6b90cced17bd785d219eedc 대비 기본 virt는 전체 ELF가 같았고 고정 RR·AMC exact-lo·mutex ordinary-pi는 전체 해시가 달라도 진입점·적재 바이트가 같았다. Linux Rust 1.95.0에서 잠금 파일을 사용해 release 단일 작업으로 빌드했다. 보드 타이밍과 지속 부하 수용성은 미검증이다.

[JSON](ordinary-eevdf-integrated.json) · [ZIP](ordinary-eevdf-integrated.zip) · [First capture](ordinary-eevdf-first.md)
