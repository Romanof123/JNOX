"""Emit QEMU 6.2 plugin site arguments from the actual packaged AArch64 ELF."""
import re
import subprocess
import sys

names = [
    'jnox_kernel::native_el0::safety::service',
    'jnox_kernel::native_el0::loans::service',
    'jnox_kernel::native_el0::loans::channel::LoanIpc::receive',
]
elf = sys.argv[1]
symbols = subprocess.check_output(['aarch64-linux-gnu-nm','-nCS',elf],text=True)
assembly = subprocess.check_output(['aarch64-linux-gnu-objdump','-dC',elf],text=True)
regions = []
for name in names:
    symbol, = re.findall(r'^([0-9a-f]+) ([0-9a-f]+) [tT] ' + re.escape(name) + '$',
                         symbols, re.M)
    start, size = (int(s,16) for s in symbol)
    call, = re.findall(r'^\s*([0-9a-f]+):\s+[0-9a-f]+\s+bl\s+0*'
                       + f'{start:x}' + ' <' + re.escape(name) + '>$', assembly, re.M)
    regions.append(f'region={call}:{start:x}:{start+size:x}')
print(','.join(regions))
