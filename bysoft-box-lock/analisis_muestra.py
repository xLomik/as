# Script de ANALISIS (no es el conversor). Parsea el DXF de muestra y decodifica/reconstruye el .lock byte a byte.
import struct, math
U='/root/.claude/uploads/c2796c0d-7d54-560b-9b85-7ebb872be7f1/'
# --- DXF parse
L=open(U+'4e9b1609-hcmf75423_r0.dxf','rb').read()
print('CRLF' if b'\r\n' in L else 'LF', 'nonascii:', any(b>127 for b in L), 'trailing newline:', L.endswith(b'\n'))
lines=L.decode('cp1252').splitlines(); pairs=list(zip(lines[::2],lines[1::2]))
ents=[];cur=None;sec=None
for c,v in pairs:
    c=c.strip()
    if c=='0':
        if cur:ents.append(cur);cur=None
        if v=='LWPOLYLINE' and sec=='ENTITIES':cur={'v':[],'flag':0}
    elif c=='2' and v in('ENTITIES','HEADER','TABLES','BLOCKS','OBJECTS'):sec=v
    elif cur is not None:
        if c=='10':cur['v'].append([float(v),0,0])
        elif c=='20':cur['v'][-1][1]=float(v)
        elif c=='42':cur['v'][-1][2]=float(v)
        elif c=='70':cur['flag']=int(v)
        elif c=='8':cur['layer']=v
for e in ents:
    vs=e['v'];A=0;P=0;xs=[];ys=[]
    for i in range(len(vs)):
        x1,y1,b=vs[i];x2,y2,_=vs[(i+1)%len(vs)]
        A+= (x1*y2-x2*y1)/2; ch=math.hypot(x2-x1,y2-y1)
        if b:
            th=4*math.atan(b); r=ch/(2*math.sin(th/2)); A+= r*r*(th-math.sin(th))/2*(1 if b>0 else -1); P+=abs(r*th)
            cx,cy=(x1+x2)/2,(y1+y2)/2; xs+= [cx-r,cx+r]; ys+=[cy-r,cy+r]
        else:P+=ch
        xs.append(x1);ys.append(y1)
    print(e['layer'],len(vs),'closed' if e['flag']&1 else 'open','bbox',min(xs),min(ys),max(xs),max(ys),'area',round(A,4),'perim',round(P,4))
# --- LOCK parse + reproduce
d=open(U+'b36e9b0d-hcmf75423_r0.lock','rb').read()
def rs(o):
    n=0;s=0
    while True:
        b=d[o];o+=1;n|=(b&0x7f)<<s;s+=7
        if b<0x80:break
    return d[o:o+n].decode('utf-8'),o+n
o=1;tn,o=rs(o);ts=struct.unpack_from('<q',d,o)[0];o+=8;user,o=rs(o);host,o=rs(o)
print('lead',d[0],'|',tn,'|',hex(ts),'|',user,'|',host,'| end',o,len(d))
def ws(s):
    b=s.encode();n=len(b);out=b''
    while n>=0x80: out+=bytes([n&0x7f|0x80]);n>>=7
    return out+bytes([n])+b
r=b'\x01'+ws(tn)+struct.pack('<q',ts)+ws(user)+ws(host)
print('rebuild identical:', r==d)
