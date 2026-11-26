# Native C shell pipeline / 네이티브 C 셸 파이프라인

The shell accepts command strings as separate arguments or reads commands from the serial console with `-i`. For a pipeline, it splits two commands at a single pipe and their arguments at spaces or tabs. It starts both registered executables with the requested pipe descriptors, closes its own copies, waits for both PIDs and returns the right-hand command's status. The cat and wc executables use ordinary native C open, read, write and close operations.

셸은 명령 문자열을 해석해서 별도 ELF로 등록된 두 프로그램을 실행한다. 앞 명령의 표준 출력과 뒤 명령의 표준 입력이 한 파이프를 공유한다. 셸의 `shell_result`에는 실제 두 PID와 대기 결과·종료값을 남긴다.

Build each source with the shell SDK. Register cat and wc as initially dormant children in slots 1 and 2, respectively. Pass the shell command as a **single** `--arg`. Each ROM file has an independently selected host source. The same binaries can run against either input.

~~~sh
d=examples/native-shell-pipeline
for app in shell cat wc; do
    sh sdk/native-aarch64/build.sh --pipes --native-shell "$d/$app.c" "$app.elf"
done
printf 'zero\none\nlast' > input-a.txt
printf 'one\n\nthree\nfour\n' > input-b.txt
python3 tools/build_native_el0.py --out shell-a-fifo --policy fifo --pipes \
    --elf shell.elf --priority 7 --arg 'cat sample.txt | wc -l' \
    --peer-elf cat.elf --peer-priority 4 --exec-child 1:cat \
    --peer-elf wc.elf --peer-priority 4 --exec-child 2:wc \
    --rom-file sample.txt=input-a.txt
~~~

For RR use `--policy rr --quantum-ticks 625000`, a fresh output directory and the same ELFs. Repeat both positions with input B. Compare the wc process's captured stdout to `cat input-a.txt | wc -l` or the corresponding B input. A produces `2\n`, B produces `4\n`. Confirm EOF in both utilities, both child statuses and shell exit 0. With `cat missing.txt | wc -l`, expect cat status 1, wc stdout `0\n`, wc status 0 and shell status 0. With a missing second executable the shell closes both ends, waits for the first child and exits 127. A malformed pipeline such as `cat sample.txt |` exits 2 before creating a pipe or launching a child.

각 입력은 FIFO·RR 모두에서 비교한다. 끝에 개행이 없는 첫 파일은 2줄, 빈 줄이 있는 두 번째 파일은 4줄이다. 파일 누락·실행 파일 누락·잘못된 명령도 별도 사례다. [Acceptance issue / 인수 기준](https://github.com/Romanof123/JNOX/issues/379)

## Kernel messages and live serial shell / 커널 메시지와 시리얼 셸

At the `jnox$ ` prompt, type `dmesg` and press Enter. The command prints up to four kernel events from this boot. A fresh shell shows native boot and shell ready. Run `cat sample.txt | wc -l` and type `dmesg` again to see both child launches and their signed exit statuses. Each child slot can launch once per boot. Type `exit` to finish the session. In scripted mode, each `--arg` supplies one command and the process returns the last command's status.

시리얼 모드는 패키징 시 `--arg=-i`로 선택한다. `dmesg`의 일련번호는 부팅마다 1부터 시작하고, `ticks`는 CPU 물리 카운터 값이다. 최근 네 건을 넘으면 오래된 기록이 밀려나며 `dropped`가 그 건수를 표시한다. 조회는 기록을 소비하지 않는다.

The kernel appends a record on entry into native startup, after the fixed scheduler selects the first shell, after each successful child launch, and when the kernel handles the child's exit. Records use one-based insertion order and physical counter ticks. Every additional event after the fourth replaces the oldest. The header reports both the retained count and the cumulative overwrite count. Repeated reads between these events return identical bytes. The journal starts empty on boot and contains only the four event kinds listed here.

The native catalog selects the diagnostic interface for the whole image. Only shell slot 0 may call `jnox_dmesg(buffer, length)` or `jnox_shell_getchar()`. For native SVC `0x4a01`, x8=70 takes a writable buffer in x0 and its length in x1, and returns the complete text length or a negative errno in x0. A short buffer returns ENOSPC, an invalid writable range returns EFAULT, and a caller in slot 1 or 2 gets EACCES before any buffer access. Errors copy zero bytes. A 512-byte buffer fits every supported snapshot. The SDK wrappers return −1 and set errno on errors. Service 71 reads one PL011 byte, returning EAGAIN when empty or EIO for a line error. Interactive mode sleeps for 10 ms of guest time between empty polls. Both services require the native shell catalog. The kernel continues to emit its existing UART boot, pipe, scheduler and exit traces.

From the repository root on Linux or WSL, build three packages into a fresh directory and capture their original UART with the commands below. The denial case deliberately launches a second copy of the shell ELF in slot 1. Its attempt to run `dmesg` must print `permission denied` and exit 1. The interactive capture sends two commands and `exit` over QEMU serial after the successive guest prompts.

~~~sh
B=$(mktemp -d)
d=examples/native-shell-pipeline
for a in shell cat wc; do
    sh sdk/native-aarch64/build.sh --native-shell "$d/$a.c" "$B/$a.elf"
done
printf 'one\ntwo\n' > "$B/sample.txt"
package() {
    case_name=$1
    child_name=$2
    child_elf=$3
    shift 3
    python3 tools/build_native_el0.py --board virt --out "$B/$case_name" \
        --policy fifo --pipes --elf "$B/shell.elf" --priority 7 "$@" \
        --peer-elf "$B/$child_elf.elf" --peer-priority 4 --exec-child "1:$child_name" \
        --peer-elf "$B/wc.elf" --peer-priority 4 --exec-child 2:wc \
        --rom-file "sample.txt=$B/sample.txt"
}
package interactive cat cat --arg=-i
package scripted cat cat --arg dmesg --arg 'cat sample.txt | wc -l' --arg dmesg --arg dmesg
package denied probe shell --arg dmesg --arg 'probe dmesg | wc -l' --arg dmesg --arg dmesg
for a in interactive scripted denied; do
    python3 tools/verify_native_dmesg.py "$a" "$B/$a/kernel.elf" "$B/$a-capture"
done
~~~

For a manual terminal session, launch the interactive image directly.

~~~sh
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 \
    -accel tcg -m 128M -nodefaults -display none -monitor none \
    -serial stdio -kernel "$B/interactive/kernel.elf"
~~~

The verifier checks the guest shell output and terminal exit records against the requested commands. It saves `uart.bin`, the QEMU invocation and the kernel digest for each run, plus `input.bin` for the interactive session. This operator interface covers the single-core QEMU virt native shell catalog. The issue tracks REQ-012 and REQ-013. [Operator diagnostics issue / 진단 구현 범위](https://github.com/Romanof123/JNOX/issues/408)
