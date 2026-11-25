# #316 package controls / 패키징 검증
At 9df9437b626da3bcaa4151f3db1370e7db24ae13, 11 CLI and 5 direct Cargo cases returned their expected status in WSL Ubuntu using Rust 1.95.0 and the first packet's real AArch64 stream and prime ELFs. Three CLI successes emitted checked manifests and hashed kernel images. Thread RR used one process slot, two threads and Q=625000. Default threaded packaging selected cooperative mode. Ordinary process RR accepted two prime process slots with explicit priorities. Eight CLI rejections returned 2. They covered missing/zero/above-maximum quantum, a peer ELF, an explicit priority, MLFQ with and without its own parameters, and a cooperative quantum. The incomplete MLFQ request failed its own parameter validation. The complete MLFQ request reached the thread-policy exclusion. Direct Cargo accepted RR and rejected missing/zero quantum, explicit priorities, and MLFQ with build-script diagnostics and status 101.

실제 ELF로 CLI 11건과 직접 Cargo 5건의 종료 상태를 확인했다. 정상 빌드 4건과 잘못된 조합 거절 12건이다. 게스트 실행은 원래 최초 측정 패킷에 있다. 이 보충 자료의 범위는 위 패키징 사례까지다.

Reproduce using the same clean source, original stream.elf and prime.elf, a writable output directory and the matching target toolchain.

~~~sh
python3 verify.py /path/to/source /path/to/stream.elf /path/to/prime.elf /path/to/output
cd /path/to/source
python3 tools/build_native_el0.py --elf /path/to/stream.elf --out /tmp/rr316-feedback-qualified --threads 2 --policy mlfq --allotment-ticks 10,20,30 --boost-ticks 100
~~~

The second command exited 2. Its diagnostic is in qualified-feedback.log. results.json includes commands, raw Cargo variables, statuses, output tails and accepted manifest fields for the other 15 cases. This supplement used zero new guest launches.
