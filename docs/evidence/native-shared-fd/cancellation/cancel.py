"""Read three frozen full-RAM images for the #444 timed receive. No guest execution."""
import argparse
import hashlib
import json
import mmap
import struct
import subprocess
from pathlib import Path

PAGE = 4096
BASE = 0x40000000
MASK = (1 << 48) - PAGE

def require(ok, message):
    if not ok:
        raise ValueError(message)
def symbols(path):
    output = subprocess.check_output(["nm", "-S", "--defined-only", path], text=True)
    return [(int(a, 16), int(n, 16), name) for row in output.splitlines()
            if len(parts := row.split()) == 4 for a, n, _, name in [parts]]


def one(entries, predicate):
    values = [entry for entry in entries if predicate(entry[2])]
    require(len(values) == 1, "missing or ambiguous linked symbol")
    return values[0][:2]


class RAM:
    def __init__(self, raw):
        self.raw = raw
        require(len(raw) == 128 << 20, "unexpected physical RAM size")

    def read(self, addr, size):
        offset = addr - BASE
        require(0 <= offset <= len(self.raw) - size, "physical extent outside virt RAM")
        return self.raw[offset:offset + size]

    def walk(self, root, va):
        table, user, write, xn = root, True, True, False
        require(0 <= va < 1 << 39, "VA outside 39-bit TTBR0")
        for shift in (30, 21, 12):
            desc = int.from_bytes(self.read(table + 8*((va >> shift) & 511), 8), "little")
            if not desc & 1:
                return None
            if shift != 12 and desc & 2:
                table = desc & MASK
                user &= not bool(desc & (1 << 61))
                write &= not bool(desc & (1 << 62))
                xn |= bool(desc & (1 << 60))
                continue
            require(shift != 12 or desc & 2, "reserved L3 descriptor")
            low = (1 << shift) - 1
            return ((desc & MASK & ~low) | (va & low),
                    user and bool(desc & 64), write and not bool(desc & 128),
                    xn or bool(desc & (1 << 54)), bool(desc & (1 << 10)))

    def user(self, root, va, size):
        data = bytearray()
        while len(data) < size:
            at = va + len(data)
            mapping = self.walk(root, at)
            require(mapping and mapping[1] and mapping[4], "unreadable EL0 VA")
            length = min(size-len(data), PAGE-at%PAGE)
            data.extend(self.read(mapping[0], length))
        return bytes(data)


def snapshot(path, phase, kernel, broker, roots, witness, mutation=None):
    at = lambda name: one(kernel, lambda n: name in n)[0]
    private = lambda name: one(broker, lambda n: name in n)[0]
    with path.open("rb") as stream, mmap.mmap(stream.fileno(),0,access=mmap.ACCESS_READ) as image:
        mem = RAM(image)
        if mutation:
            # Change only a private decoder view of the physical snapshot.
            endpoint=at("9ENDPOINTS")
            client_va=private("CLIENT_STATE")
            if mutation=="broker token":
                address=mem.walk(roots[3],client_va+8)[0]
            elif mutation=="broker operation":
                address=mem.walk(roots[3],client_va+168)[0]
            elif mutation=="caller slot":
                # Relocate the active request to slot 1 and mark slot 0 absent.
                source=mem.read(endpoint,33)
                original=mem.read
                def switched(a,n):
                    data=bytearray(original(a,n))
                    for target,value in [(endpoint+32,2)]+list(enumerate(source,endpoint+88)):
                        if a<=target<a+n:
                            data[target-a]=value
                    return bytes(data)
                mem.read=switched
            else:
                raise ValueError("unknown control")
            if mutation!="caller slot":
                original=mem.read
                def flipped(a,n):
                    data=bytearray(original(a,n))
                    if a<=address<a+n:
                        data[address-a]^=1
                    return bytes(data)
                mem.read=flipped
        words = struct.unpack("<32q",mem.user(roots[0],witness,256))
        require(words[1]==phase, "phase marker")
        slot = mem.read(at("9ENDPOINTS"),88)
        server, word, deadline, token = struct.unpack_from("<4Q",slot)
        delivered = slot[32]  # Rust Option<Request> niche; 2 absent, 0 queued, 1 delivered.
        require(delivered in (0,1,2),"kernel request tag")
        client = mem.user(roots[3],private("CLIENT_STATE"),272)
        ptag, pending = struct.unpack_from("<2Q",client)
        require(ptag in (0,1),"broker pending tag")
        shared = mem.read(at("jnox_native_shared_state"),224)
        fd98 = tuple(shared[36:38])
        escrows = [(tag,obj,wr) for tag,obj,wr in struct.iter_unpack("<QBB6x",shared[96:]) if tag]
        record = dict(phase=phase, cancel_status=words[29], fd98=fd98, escrows=escrows,
                      kernel_request=None if delivered==2 else dict(server=server,word=hex(word),
                      deadline=deadline,token=token,delivered=bool(delivered)),broker_pending=pending if ptag else None)
        if phase==9:
            op,fd,arg=client[168:171]
            record["broker_request"] = dict(op=op,fd=fd,count=arg,request_niche=client[267])
            live = delivered!=2 and server==3 and word==0x13<<56 and 0<deadline<(1<<64)-1 and token!=0
            require(fd98==(0,0) and not escrows,"unexpected initial right")
            record["same_delivered_commit_pending"] = bool(live and delivered==1 and ptag and pending==token and
                    (op,arg)==(16,1) and fd<4 and client[267]!=2)
            if record["same_delivered_commit_pending"]:
                # Inspect both transactional banks. Agreement avoids inferring the live-bank selector.
                views=[]
                for i in range(2):
                    bank=mem.user(roots[3],private("BANKS")+i*4544,4544)
                    tag,index=struct.unpack_from("<2Q",bank,4160+fd*16)
                    if tag!=1 or index>=8:
                        views.append(None)
                        continue
                    offset=index*520
                    views.append(dict(endpoint=index, kind=bank[offset+515],
                                      bytes=struct.unpack_from("<Q",bank,offset+168)[0]))
                record["bank_views"]=views
                record["empty_stream_confirmed"]=all(v and v["kind"]==1 and v["bytes"]==0 for v in views)
        else:
            require(words[29]==-110, "caller did not observe ETIMEDOUT")
        return record

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("build",type=Path)
    parser.add_argument("capture",type=Path)
    args=parser.parse_args()
    pkg=args.build/"package"
    for elf,h in [(pkg/"kernel.elf","6d815ba056b79a32694353659b89f56edb8ae835ba92168ec02438adad4bce9e"),
                  (pkg/"native_peer3.elf","8e23432bd3071aa2d1505dc5000898a7b7bff79b8c98ae8d50ffa2bbc17d6d29")]:
        require(hashlib.sha256(elf.read_bytes()).hexdigest()==h,"unsupported exact ELF layout")
    kernel=symbols(pkg/"kernel.elf")
    broker=symbols(pkg/"native_peer3.elf")
    base,size=one(kernel,lambda n:"native_el0_mmu" in n and "6SPACES" in n)
    roots=[base+i*size//4 for i in range(4)]
    witness=one(symbols(pkg/"user.elf"),lambda n:n=="fd_transfer_witness")[0]
    pre,queued,installed=[snapshot(args.capture/(name+".ram"),phase,kernel,broker,roots,witness)
                          for name,phase in [("pretimeout",9),("queued",10),("installed",11)]]
    require(queued["fd98"]==(0,0) and len(queued["escrows"])==1,"later escrow without installation")
    later_token,obj,wr=queued["escrows"][0]
    require(wr==0 and installed["fd98"]==(obj,0) and not installed["escrows"],"later read-only installation")
    if pre["same_delivered_commit_pending"]:
        stale=pre["broker_pending"]
        require(later_token>stale and all(not s["kernel_request"] or s["kernel_request"]["token"]!=stale for s in [queued,installed]),"old token versus later transfer")
    controls=[]
    if pre["same_delivered_commit_pending"] and pre.get("empty_stream_confirmed"):
        for mutation in ("broker token","caller slot","broker operation"):
            changed=snapshot(args.capture/"pretimeout.ram",9,kernel,broker,roots,witness,mutation)
            require(not changed["same_delivered_commit_pending"],"accepted mutation: "+mutation)
            controls.append(mutation)
    print(json.dumps(dict(rejected_phase9_controls=controls,capture=args.capture.name, stages=[pre,queued,installed],
                          limit="Snapshots bound transitions; no continuous observation between stops."),indent=2))

if __name__=="__main__":
    main()

