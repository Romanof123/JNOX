import pathlib,re,collections
import argparse,hashlib
parser=argparse.ArgumentParser(description="Replay the four original captures from the extracted archive")
parser.add_argument("directory",type=pathlib.Path)
H=parser.parse_args().directory
for line in (H/'SHA256SUMS').read_text().splitlines():
 digest,name=line.split('  ',1)
 if hashlib.sha256((H/name).read_bytes()).hexdigest()!=digest:raise ValueError(name)
assert len(list(H.glob('0*/uart')))==4
N=2**64-1
for p in sorted(H.glob('0*')):
 t=collections.defaultdict(list)
 for line in (p/'uart').read_text().splitlines()[3:-1]:
  key,*v=line.split();t[key].append([int(x,16) for x in v])
 e,r=t['E'],t['R']; ce,reason,ne,nr,nc=t['H'][0]; F,start,cv,rb,ctl,iar,irqctl,icv=t['T'][0]
 assert F==62500000 and cv==rb==icv==start+6250 and ctl==1 and iar==30 and irqctl==5
 assert ne==len(e) and nr==len(r) and nc==len(t['C'])
 ready={0,3};own=[N,N];wait=[[],[]];dead=0;frame={};previous=0
 for i,x in enumerate(e):
  entry,dec,op,j,a,b=x[:6];assert previous<=entry<=dec;previous=dec
  assert a in ready and (i==0 or a==e[i-1][5])
  if op==5:ready|={1,2};assert entry>=cv and x[8]&128==0
  if op==1:
   if own[j]==N:own[j]=a;assert x[19]==0
   else:wait[j].append(a);ready.remove(a);assert x[19]==N
  if op==2:
   assert own[j]==a
   own[j]=wait[j].pop(0) if wait[j] else N
   if own[j]!=N:ready.add(own[j])
  if op==4:ready.remove(a);dead|=1<<a
  priority=[2,5,9,0]
  for z,o in enumerate(own):
   if o!=N:priority[o]=max([priority[o]]+([5+4*z] if ce else [priority[w] for w in wait[z]]))
  mask=lambda ss:sum(1<<v for v in ss)
  assert x[13:19]==[mask(ready),*own,*[mask(w) for w in wait],sum(v<<(8*k) for k,v in enumerate(priority))]
  assert x[22]==dead and b in ready and priority[b]==max(priority[k] for k in ready)
  assert x[23:25]==[a+1,b+1]
  frame[a]=x[6],x[7],x[9]
  if b in frame:assert frame[b]==(x[10],x[11],x[12])
  for who,addr in [(a,x[6]),(b,x[10])]:
   tid,base,lo,hi,size=t['S'][who];assert tid==who and base==[2,5,9,0][who] and lo<=addr<=hi-size
  if i+1<len(r):assert r[i+1][:2]==x[10:12] and r[i+1][2]>=dec
  if op==2 and b==2:assert x[25]==0
 if (p/'external.log').exists():
  s=(p/'external.log').read_text()
  assert [int(v,16) for v in re.findall(r'with ELR 0x([0-9a-f]+)',s)]==[x[7] for x in e]
  assert [int(v,16) for v in re.findall(r'Exception return .*?PC 0x([0-9a-f]+)',s)]==[x[1] for x in r]
  assert re.findall(r'Taking exception \d+ \[(.*?)\]',s)==['IRQ' if x[2]==5 else 'SVC' for x in e]
 if reason:print(p.name,'UNQUALIFIED',reason);continue
 assert t['C']==[[256*k,2176*k] for k in range(1,257)]
 assert t['B']==t['HIGH']==[[1,65536,557056]] and t['A']==[[ce,ce*256]] and t['M']==[[ce,ce*256,32896]] and dead==7
 K=t['K'][0];irq=next(x for x in e if x[2]==5);blk=next(x for x in e if x[19]==N)
 assert irq[21]==1 and (not ce or irq[20]<128)
 transfer=next(x for x in e if x[2]==2 and x[4:6]==[0,2])
 assert K[0]<=irq[0]<=K[3]<=blk[0]<=blk[1]<K[1]<=transfer[0]<=K[8]<=transfer[1]<=K[4]<=K[5] and (not ce or K[2]<K[6]<K[7])
 # event windows use exact sampled endpoints, never bounds on deployed performance
 print(p.name,'IRQchunk',irq[20],'ticks IRQ/pre-attempt',K[3]-irq[0],'IRQ/SVCentry',blk[0]-irq[0],'blockDecision/transferMarker',K[8]-blk[1],'transferMarker/returnedMarker',K[4]-K[8],'LOW final-lock post-return/transfer',K[8]-K[0],'LOW final-lock post-return/commit',K[1]-K[0])
 for x in e:
  if x[2]==2:
   idx=e.index(x);acq=next(y for y in reversed(e[:idx]) if y[3]==x[3] and ((y[2]==1 and y[4]==x[4] and y[19]==0) or (y[2]==2 and y[5]==x[4])))
   print('resource,owner,residency tick bracket',x[3],x[4],x[0]-acq[1],x[1]-acq[0])
