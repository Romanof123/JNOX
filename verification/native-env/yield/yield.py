"""Additional two-process same-priority yield controls on each real startup ABI."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys

src,out,runtime = map(Path,sys.argv[1:])
out.mkdir()
here = Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('native_env_acceptance',src/'verification/native-env/run.py')
producer=importlib.util.module_from_spec(spec)
spec.loader.exec_module(producer)
producer.HERE=here
run=producer.run
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()

def check(uart):
    rows=re.findall(rb'(?m)^ENV YIELD ([AB]) ([01])\r?$',uart)
    if rows not in ([(b'A',b'0'),(b'B',b'0'),(b'A',b'1'),(b'B',b'1')],
                    [(b'B',b'0'),(b'A',b'0'),(b'B',b'1'),(b'A',b'1')]):
        raise ValueError('missing real interleave across yield')
    exits=sorted(producer.observed_exits(uart))
    if exits != [(0,91),(1,92)]: raise ValueError('wrong slot validation result')
    return [[a.decode(),b.decode()] for a,b in rows],exits

plain=out/'ordinary.elf'
run('sh',producer.SDK/'build.sh',here/'probe.c',plain)
pico=producer.pico(runtime,out)
for tag,elf in [('ordinary',plain),('picolibc',pico)]:
    case=out/tag
    run(sys.executable,src/'tools/build_native_el0.py','--board','virt','--policy','fifo','--out',case,
        '--elf',elf,'--arg','A','--priority','4','--peer-elf',elf,'--peer-arg','B','--peer-priority','4',
        '--console-write','0','--console-write','1','--env','0:WAYLAND_DISPLAY=/wayland-0',
        '--env','1:WAYLAND_DISPLAY=/wayland-1','--env','0:A_ONLY=one','--env','1:B_ONLY=two')
    manifest=json.loads((case/'manifest.json').read_bytes())
    assert [s['environment'] for s in manifest['slots']] == [
        {'WAYLAND_DISPLAY':'/wayland-0','A_ONLY':'one'},{'WAYLAND_DISPLAY':'/wayland-1','B_ONLY':'two'}]
    assert all(s['elf_sha256']==sha(elf) for s in manifest['slots'])
    capture=src/'examples/native-pixman/capture.py'; uart=case/'uart.log'
    run(sys.executable,capture,case/'kernel.elf',uart,'2','--symbol','main','--symbol','native_yield')
    raw=uart.read_bytes()
    order,exits=check(raw)
    controls={}
    for name,altered in [('dropped_after_yield',raw.replace(b'ENV YIELD A 1',b'ENV YIELD A 0',1)),
                         ('peer_return_identity',raw.replace(b'EXIT PID 1 92',b'EXIT PID 1 91',1))]:
        assert altered!=raw
        try: check(altered)
        except ValueError as e: controls[name]=str(e)
        else: raise ValueError('accepted corrupted receipt: '+name)
    report={'runtime':tag,'app_sha256':sha(elf),'kernel_sha256':sha(case/'kernel.elf'),
            'uart_sha256':sha(uart),'exits':exits,'order':order,'mutations':controls}
    (case/'receipt.json').write_text(json.dumps(report,indent=2))
    print(report,flush=True)
