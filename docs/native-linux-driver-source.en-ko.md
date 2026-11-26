# CHG-325 · Native services built from Linux driver source

[Driver-source issue #424](https://github.com/Romanof123/JNOX/issues/424) · REQ-004 · REQ-011 · DES-002 · VER-004 · VER-011

Rebuild unchanged, hash-pinned upstream driver source with a bounded adapter as an unprivileged native J-NOX ELF. Reject unsupported APIs at build time. The J-NOX Rust kernel owns grants for device memory, IRQs and DMA. Native Rust drivers continue to use the native driver path. Historical Linux-module and gateway trials remain reference comparisons. Native services require neither Linux nor Bao and assume no compatibility with existing .ko binaries.

IMP-325 runs the Linux PL031 driver on JNOX. EVD-325 covers RTC reads and writes, alarm delivery, resource reclaim, peer denial and source/object identity in two cold boots. [Measured results](evidence/native-linux-source-rtc/README.md)

CHG-326 takes the next step with the upstream `virtnet_validate` callback. It defines a native feature-validation milestone and the remaining packet, DMA, IRQ and service-restart criteria. [virtio-net change record](native-linux-source-net.en-ko.md)

## CHG-325 · Linux 드라이버 소스로 빌드하는 네이티브 서비스

해시로 고정한 원본 Linux 드라이버 소스를 수정 없이 제한된 어댑터와 함께 비특권 J-NOX 네이티브 ELF로 빌드한다. 미지원 API는 빌드에서 거부한다. J-NOX Rust 커널이 장치 메모리·IRQ·DMA 권한의 부여를 담당한다. 네이티브 Rust 드라이버는 자체 경로를 계속 사용한다. 기존 Linux 모듈 및 게이트웨이 시험은 참조 비교로 보존한다. 네이티브 서비스에는 Linux 커널이나 Bao가 필요하지 않으며 기존 .ko 바이너리 호환성을 가정하지 않는다.

IMP-325는 원본 PL031 드라이버를 JNOX에서 실행한다. EVD-325에 실제 시계 읽기·설정, 알람 IRQ, 자원 회수, 비인가 앱 접근 거부와 소스·ELF 식별값을 기록했다. [실측 결과](evidence/native-linux-source-rtc/README.md)

다음 단계인 CHG-326은 업스트림 `virtnet_validate` 콜백의 네이티브 기능 검증과 이후 패킷·DMA·IRQ·서비스 재시작 인수 기준을 정의한다. [virtio-net 변경 기록](native-linux-source-net.en-ko.md)
