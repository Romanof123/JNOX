# Boot-provisioned local X11 authority / 로컬 X11 인증 파일 공급

On the AArch64 virt five-active display profile, Xwayland (PID 1) and Mosaic (PID 4) each have an immutable process-local view of `/run/Xauthority`. The compositor, the second native client and the socket broker cannot read either record. This interface supplies data consumed by the X11 authorization layer. The server and client must still check the configured node name, display number, protocol and cookie. [Five-role deployment](../verification/xwayland/five-active.md) · [Acceptance #465](https://github.com/Romanof123/JNOX/issues/465) · [Node identity #464](https://github.com/Romanof123/JNOX/issues/464)

Start QEMU with two private files in an access-controlled working directory. Set `server_record` and `client_record` to their paths. For an authorized client the same private file can supply both inputs. For the wrong-cookie control use two separately generated records for the same node and display. The QEMU command takes file paths. Package arguments, environment variables and the public verification record contain no cookie values.

```sh
-fw_cfg "name=opt/jnox/xauth-server,file=$server_record" \
-fw_cfg "name=opt/jnox/xauth-client,file=$client_record"
```

EL1 reads the firmware directory once after installing the privileged device mapping and before entering the first process. It accepts 1–256 opaque bytes per role, with at most 256 directory entries, a valid selector and a unique resource name. The SVC dispatcher uses the caller's current slot. The caller cannot select its peer's record. Copies validate the complete private writable image or stack range before writing. Shared mappings cannot be destinations. An authorized process is responsible for its own subsequent use of the credentials. The host and firmware remain trusted for this QEMU experiment.

| SVC 199 operation | Result |
| --- | --- |
| `x0=0, x1=0, x2=0` | Record size, in bytes |
| `x0=1, x1=destination, x2=capacity` | Full record length on successful copy |
| Caller outside slots 1 and 4 | `-EACCES` |
| Missing resource, invalid length, malformed directory respectively | `-ENOENT`, `-EINVAL`, `-EIO` |
| Unsupported operation or nonzero reserved argument | `-EINVAL` |
| Short buffer or invalid/private-unwritable destination | `-ERANGE` or `-EFAULT`, without a partial copy |

The C [SDK header](../sdk/native-aarch64/include/jnox/boot_resource.h) exposes size and copy. Filesystem mounting, persistent writes, product credential provisioning and key rotation have separate requirements. Verify boot-resource isolation independently of the later test in which the original Xwayland listener accepts or rejects an X11 connection. [Filesystem plan](https://github.com/Romanof123/JNOX/issues/459)

## 한국어

다섯 프로세스 구성에서 서버 1번과 Mosaic 4번은 각각 자기 슬롯에 지정된 인증 파일만 읽는다. 커널은 첫 프로세스를 시작하기 전에 EL1 전용 장치 페이지를 통해 자원을 복사한다. 파일당 최대 256바이트이며, API는 현재 슬롯의 사설 쓰기 가능 메모리로만 전체 레코드를 반환한다. 0·2·3번은 권한 오류를 받는다. 없는 파일, 부족한 버퍼, 잘못된 주소는 각각 다른 오류로 반환된다.

정상 시험에서는 두 슬롯에 동일한 비공개 인증 파일을 제공하고, 인증 거절 시험에서는 같은 노드·디스플레이에 서로 다른 쿠키를 제공한다. 실제 X11 요청 허용·거부는 원본 서버와 Mosaic를 함께 실행해 검증한다. 이 가상 프로필의 부팅 자원 조회와 제품용 영속 파일시스템·키 관리는 별도로 추적한다.

[Nine guest boots and broker checks · 게스트 부팅 아홉 건 및 브로커 검사](../verification/xwayland/authority/resource-gate.en-ko.md)

CHG-346 / IMP-346 adds the bounded per-role read grant under REQ-011 and DES-001. EVD-346 supplies partial VER-011 evidence for this resource. EVD-346은 해당 리소스 접근 권한과 역할 분리에 대한 증거이다.
