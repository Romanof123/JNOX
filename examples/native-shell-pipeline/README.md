# Native C shell pipeline / 네이티브 C 셸 파이프라인

The shell takes one command string. It splits two commands at a single pipe and their arguments at spaces or tabs. It starts both registered executables with the requested pipe descriptors, closes its own copies, waits for both PIDs and returns the right-hand command's status. The cat and wc executables use ordinary native C open, read, write and close operations.

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
