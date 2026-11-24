"""Validate one original #248 UART file. Usage: python tools/deadline_supply_replay.py PROFILE UART"""
import argparse
import json
from pathlib import Path


def require(condition, message):
    if not condition:
        raise ValueError(message)


def product(seed, blocks):
    # Compose affine maps over Z/(2^64); each reported block has 64 rounds.
    mask = (1 << 64) - 1
    a, b = 33, 1
    n = 64 * blocks
    while n:
        if n & 1:
            seed = (a * seed + b) & mask
        a, b = a * a & mask, (a * b + b) & mask
        n >>= 1
    return seed


def replay(profile, path, plain=False):
    text = path.read_text()
    records = {}
    events = []
    for line in text.splitlines():
        fields = line.split()
        if not fields or fields[0] not in {'SUPPLY', 'CONFIG', 'JOB', 'D', 'J', 'RESULT'}:
            continue
        tag, values = fields[0], list(map(int, fields[1:]))
        records.setdefault(tag, []).append(values)
        if tag == 'D':
            events.append((values, []))
        elif tag == 'J':
            events[-1][1].append(values)
    (mask, full, tight, top, stack, pc), = records['SUPPLY']
    (hz, epoch, q, llf, overload, refused, horizon, frame), = records['CONFIG']
    require((mask, full, tight, llf, overload, horizon, q*10) ==
            (2, 1, int(profile.endswith('-refuse')), int('llf' in profile), 0, 16, hz), 'configuration')
    require('ORDINARY DEADLINE FAIL' not in text, 'guest failure')
    if tight:
        require(refused == (9 if llf else 8) and 'ORDINARY DEADLINE REFUSED BEFORE LAUNCH' in text
                and not any(t in records for t in ('D', 'J', 'JOB', 'RESULT'))
                and 'ORDINARY DEADLINE FINISHED' not in text, 'early refusal')
        return {'refusal': refused, 'full_supply_admitted': True}
    require(not refused and 'ORDINARY DEADLINE FINISHED' in text, 'normal termination')
    jobs = records['JOB']
    require([j[:5] for j in jobs] == [[0,0,5,2,250],[1,1,7,2,30]], 'linked table')
    if plain:
        require(not events, 'expected plain image')
        (checksum,sp,a,b),=records['RESULT']
        require(top-stack<sp<=top and checksum==89440, 'plain service output')
        return {'plain_finished':True,'service_sum':checksum,'worker_products':[a,b]}
    require(len(events) >= 5, 'trace required')
    prior = None
    spent = [0,0,0]
    counts = [0,0,0]
    completions = []
    grid = 0
    for e, js in events:
        kind,k,g,target,program,iar,restore,entry,handler,out,owner,fp,elr,dst,de,queue,releases,exhaust,miss = e
        require(len(js)==3 and [x[0] for x in js]==[0,1,2], 'job rows')
        require(k==(entry-epoch)//q and g==grid and target==epoch+g*q and handler>=entry>=restore, 'absolute grid')
        if prior:
            require(out==prior[10] and restore>=prior[8], 'owner continuity')
        if out < 3:
            require(jobs[out][6]-stack<=fp<=jobs[out][6]-frame, 'worker stack')
            spent[out] += k-prior[1]
            counts[out] += entry-restore
        require([x[3] for x in js]==spent and [x[4] for x in js]==counts, 'actual worker accounting')
        if kind==1:
            require(k==grid and target==program and iar&1023==30, 'timer')
            grid += 1
        else:
            require(kind==2 and k<grid and out<2 and entry<=epoch+jobs[out][2]*q and not miss, 'completion deadline')
            completions.append(out)
        require(releases == (1 if prior is None else 2 if kind==1 and k==1 else 0), 'release order')
        require(owner == (4 if k==1 else 0 if 0 not in completions else 1 if 1 not in completions else 3), 'dispatch')
        prior=e
    require(completions==[0,1], 'finite completion')
    (cut,cj),(resume,rj) = events[1:3]
    require((cut[0],cut[1],cut[9],cut[10],resume[0],resume[1],resume[9],resume[10])==(1,1,0,4,1,2,4,0), 'service interval')
    require((cut[11],cut[12])==(resume[13],resume[14]) and cut[14]==pc
            and cut[13]==top-frame and top-stack<=resume[11]<=top-frame, 'service entry and worker restore')
    require(cj[0][1]==rj[0][1]==1 and cj[0][5]==rj[0][5]>0 and cj[1][1]==rj[1][1]==2, 'withheld remaining and progress')
    (checksum,sp,a,b),=records['RESULT']
    last=events[-1][1]
    require(top-stack<sp<=top and checksum==89440 and last[0][5]>rj[0][5] and last[1][5]>0, 'service product and worker progress')
    require([a,b]==[product(i+1,last[i][5]) for i in range(2)], 'worker products')
    return {'service_counts':resume[7]-resume[6], 'grid_counts':q,'worker_counts':counts[:2],
            'worker_blocks':[j[5] for j in last[:2]], 'worker_products':[a,b],'service_sum':checksum}


if __name__ == '__main__':
    p=argparse.ArgumentParser()
    p.add_argument('profile',choices=['ordinary-'+p+'-supply'+s for p in ('edf','llf') for s in ('','-refuse')])
    p.add_argument('uart',type=Path)
    p.add_argument('--plain', action='store_true')
    args=p.parse_args()
    print(json.dumps(replay(args.profile,args.uart,args.plain),indent=2))
