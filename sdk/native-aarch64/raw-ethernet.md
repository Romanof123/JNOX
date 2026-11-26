# Native raw Ethernet

A native AArch64 Rust application enables the optional `raw-ethernet` feature on its `jnox-native` dependency. The slot selected by `--net-service` can immediately receive and transmit frames. The optional `--net-standby SLOT` selects one packaged slot that can claim frame access after the original owner exits and device reset succeeds. Socket broker routes continue to use the original broker identity.

```rust
use jnox_native::raw_ethernet::{self, MAX_FRAME};

fn poll_frame(buffer: &mut [u8; MAX_FRAME]) -> Result<Option<&[u8]>, i64> {
    match raw_ethernet::receive(buffer) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(-11) => Ok(None),
        Err(errno) => Err(errno),
    }
}
```

Pass a complete 14–1514 byte Ethernet frame to `raw_ethernet::transmit(frame)`. Success means the device accepted the whole frame. On `Err(-11)`, retain the bytes for a later attempt. Received bytes borrow the caller's buffer.

The designated standby calls `raw_ethernet::claim()` before its first transfer. Retry later on `Err(-11)`. Success activates the replacement epoch. A repeated claim returns `Err(-114)`, an unauthorized slot gets `Err(-13)`, and device failure propagates its negative errno.

C applications can include `<jnox/raw_ethernet.h>`. The receive and transmit functions return the transferred byte count or a negative errno. A successful claim returns zero.

패키지의 기존 NIC 소유자는 Rust의 `receive`와 `transmit`으로 완전한 이더넷 프레임을 주고받는다. `--net-standby SLOT`으로 지정한 프로세스는 이전 소유자가 종료된 뒤 `claim()`을 호출한다. 이전 소유자가 실행 중이면 `-11`, 다른 프로세스는 `-13`, 이미 권한을 얻은 예비 프로세스의 중복 요청은 `-114`가 반환된다. C 인터페이스는 같은 규칙을 적용한다. [재현 예제](../../examples/native-net-dma/README.md)
