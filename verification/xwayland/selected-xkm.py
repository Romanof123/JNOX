"""Prepare the explicitly selected JNOX keyboard profile in an extracted upstream tree."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
PACK = 'b25970e444fadf4717e5624bc2a2eecef785d09d731b571da8d43f5297b43b12'
WIRE = '60f9e7f24f4dff1904fd1331b9c21c39c20dc0b5c7f6f7c8802f0b57c1d398e8'
def sha(data): return hashlib.sha256(data).hexdigest()
def replace(src, before, after):
    if src.count(before) != 1: raise ValueError('unexpected upstream anchor: '+before[:64])
    return src.replace(before,after)
def array(name, data):
    rows = [', '.join(f'0x{b:02x}' for b in data[i:i+12]) for i in range(0,len(data),12)]
    return 'static const unsigned char '+name+'[] = {\n    '+',\n    '.join(rows)+'\n};\n'

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--server',type=Path,required=True,help='disposable extracted Xwayland 24.1.6')
p.add_argument('--input-rs',type=Path,required=True,help='actual compositor services/wayland/src/input.rs')
p.add_argument('--host-deb',type=Path,required=True)
p.add_argument('--host-xkbfile-deb',type=Path,help='pinned Debian amd64 libxkbfile1 if missing on the host')
p.add_argument('--out',type=Path,required=True,help='fresh provenance and generated output')
a = p.parse_args()
a.out=a.out.resolve(); a.out.mkdir(parents=True,exist_ok=False)
raw = a.input_rs.read_text(encoding='utf-8')
match = re.search(r'const KEYMAP: &\[u8\] = concat!\(\s*r#"(.*?)"#,\s*"\\0"\s*\)\.as_bytes\(\);',raw,re.S)
if not match: raise ValueError('compositor KEYMAP encoding changed')
source=match[1].encode(); wire=source+b'\0'
if sha(wire)!=WIRE: raise ValueError('unrecognized compositor map')
if sha(a.host_deb.read_bytes())!=PACK: raise ValueError('wrong xkbcomp host package')
subprocess.run(['dpkg-deb','-x',a.host_deb.resolve(),a.out/'host-tool'],check=True)
compiler=a.out/'host-tool/usr/bin/xkbcomp'
(a.out/'compositor.xkb').write_bytes(source)
(a.out/'compositor.wire').write_bytes(wire)
env=dict(os.environ,LC_ALL='C')
for k in ('LD_PRELOAD','LD_LIBRARY_PATH','XKB_CONFIG_ROOT'): env.pop(k,None)
if a.host_xkbfile_deb:
    if sha(a.host_xkbfile_deb.read_bytes())!='7c58d9986f918b71568ad83dbb6f4ab22c185f243461d41acee920cc5e13d347':
        raise ValueError('wrong host libxkbfile package')
    subprocess.run(['dpkg-deb','-x',a.host_xkbfile_deb.resolve(),a.out/'host-deps'],check=True)
    env['LD_LIBRARY_PATH']=str(a.out/'host-deps/usr/lib/x86_64-linux-gnu')
cmd=[str(compiler),'-I','-w','10','-xkm','compositor.xkb','selected.xkm']
with (a.out/'compiler.log').open('wb') as log:
    subprocess.run(cmd,cwd=a.out,env=env,check=True,stdout=log,stderr=subprocess.STDOUT)
xkm=(a.out/'selected.xkm').read_bytes()
if len(xkm) != 2224 or sha(xkm) != '7b584e4dd0c352b8c20357a0c8de5c9f852a0caa1b366d5bd420d9e2ecc779b7':
    raise ValueError('compiled XKM differs from the verified native guest input')
# Retain the ELF dependency identities of the pinned native host tool.
ldd=subprocess.check_output(['ldd',compiler],env=env,text=True)
deps={Path(s).name:sha(Path(s).read_bytes()) for s in re.findall(r'(/[^\s()]+)\s+\(',ldd)}
version=subprocess.check_output([compiler,'-version'],env=env,text=True).strip()
report=dict(host_package_sha256=PACK,xkbcomp_sha256=sha(compiler.read_bytes()),xkbcomp_version=version,
            compiler_dependencies=deps,argv=cmd[1:],input_rs_sha256=sha(a.input_rs.read_bytes()),
            text_sha256=sha(source),wire_sha256=sha(wire),xkm_sha256=sha(xkm),wire_bytes=len(wire),xkm_bytes=len(xkm))
(a.out/'inputs.json').write_text(json.dumps(report,indent=2)+'\n')
root=a.server
ddx=root/'xkb/ddxLoad.c'; wl=root/'hw/xwayland/xwayland-input.c'
meson=root/'meson.build'; opts=root/'meson_options.txt'
if sha(ddx.read_bytes())!='bd9f7eaf1249df1f77601fd407a9ef410dda0a97d070a7df6e504fcc09451583':
    raise ValueError('unexpected upstream ddxLoad.c')
ddx_text=replace(ddx.read_text(),'#include <xkbsrv.h>','#include <xkbsrv.h>\n#ifdef JNOX_SELECTED_XKM\n#include "jnox-selected-xkm.inc"\n#else')+'\n#endif\n'
wl_text=replace(wl.read_text(),'static void\nkeyboard_handle_keymap(','''#ifdef JNOX_SELECTED_XKM
#include "jnox-selected-xkm.h"
#endif
static void
keyboard_handle_keymap(''')
start=wl_text.index('    if (xwl_screen->nokeymap)',wl_text.index('static void\nkeyboard_handle_keymap('))
end=wl_text.index('    XkbUpdateDescActions(',start)
original=wl_text[start:end]
wl_text=wl_text[:start]+'''#ifdef JNOX_SELECTED_XKM
    if (xwl_screen->nokeymap)
        goto out;
    if (format != WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1 || size != jnox_selected_xkb_size()) {
        LogMessage(X_ERROR, "JNOX: unsupported Wayland keymap format or extent\\n");
        goto out;
    }
    /* JNOX MAP_SHARED checks this exact extent against the backing object.
     * Its read-only view lives only while identifying the pinned wire map. */
    void *incoming = mmap(NULL, size, PROT_READ, MAP_SHARED, fd, 0);
    if (incoming == MAP_FAILED)
        goto out;
    const char *selected = jnox_selected_xkb_identify(incoming, size);
    if (munmap(incoming, size) != 0)
        FatalError("JNOX: cannot release keymap mapping\\n");
    if (!selected) {
        LogMessage(X_ERROR, "JNOX: unsupported Wayland keymap bytes\\n");
        goto out;
    }
    xkb = XkbCompileKeymapFromString(xwl_seat->keyboard, selected, size - 1);
    if (!xkb)
        goto out;
    xwl_seat->keymap = selected;
    xwl_seat->keymap_size = size;
#else
'''+original+'#endif\n'+wl_text[end:]
seat=root/'hw/xwayland/xwayland-input.h'
seat_text=replace(seat.read_text(),'    char *keymap;',"""#ifdef JNOX_SELECTED_XKM
    const char *keymap; /* Borrows the immutable server image until seat teardown. */
#else
    char *keymap;
#endif""")
anchor="cc = meson.get_compiler('c')"
meson_text=replace(meson.read_text(),anchor,anchor+'''
if get_option('jnox_selected_xkm')
    if host_machine.system() != 'jnox' or get_option('xkb_default_rules') != 'jnox' or get_option('xkb_default_model') != 'jnox' or get_option('xkb_default_layout') != 'jnox'
        error('selected XKM requires JNOX and explicit jnox/ jnox/ jnox defaults')
    endif
    add_project_arguments('-DJNOX_SELECTED_XKM', language: 'c')
endif''')
opts_text=opts.read_text()
if 'jnox_selected_xkm' in opts_text: raise ValueError('profile already applied')
for path in (root/'include/jnox-selected-xkm.h',root/'xkb/jnox-selected-xkm-data.h',root/'xkb/jnox-selected-xkm.inc'):
    if path.exists(): raise FileExistsError(path)
(root/'xkb/jnox-selected-xkm-data.h').write_text(array('jnox_xkb_wire',wire)+array('jnox_xkm_bytes',xkm))
shutil.copyfile(HERE/'selected-xkm.inc',root/'xkb/jnox-selected-xkm.inc')
shutil.copyfile(HERE/'selected-xkm.h',root/'include/jnox-selected-xkm.h')
ddx.write_text(ddx_text); wl.write_text(wl_text); seat.write_text(seat_text); meson.write_text(meson_text)
opts.write_text(opts_text+"\noption('jnox_selected_xkm', type: 'boolean', value: false)\n")
