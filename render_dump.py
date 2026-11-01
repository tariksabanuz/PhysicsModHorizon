import json, math
from PIL import Image, ImageDraw
data = json.load(open('/tmp/hp_dump.json'))
W, H = 480, 320
FACES = [(0,2,6,4),(1,3,7,5),(0,1,5,4),(2,3,7,6),(0,1,3,2),(4,5,7,6)]
def cam_setup(eye, target):
    f = [t-e for t,e in zip(target, eye)]; n = math.sqrt(sum(x*x for x in f)); f=[x/n for x in f]
    r = [f[2]*0 - 0*f[1], 0, 0]
    up=(0,1,0)
    r=[f[1]*up[2]-f[2]*up[1], f[2]*up[0]-f[0]*up[2], f[0]*up[1]-f[1]*up[0]]; n=math.sqrt(sum(x*x for x in r)); r=[x/n for x in r]
    u=[r[1]*f[2]-r[2]*f[1], r[2]*f[0]-r[0]*f[2], r[0]*f[1]-r[1]*f[0]]
    return eye,f,r,u
def proj(c, p):
    eye,f,r,u=c; d=[a-b for a,b in zip(p,eye)]
    z=sum(a*b for a,b in zip(d,f)); x=sum(a*b for a,b in zip(d,r)); y=sum(a*b for a,b in zip(d,u))
    s=1.3/max(z,0.05)
    return (W/2+x*s*H/2*1.0, H/2-y*s*H/2), z
cols=[(200,90,80),(90,160,90),(90,120,200),(210,180,90),(160,100,190),(80,180,180),(220,140,60),(150,150,150)]
def frame(boxes, c, fill_floor=True):
    im=Image.new('RGB',(W,H),(150,190,235)); d=ImageDraw.Draw(im)
    # floor grid
    polys=[]
    for gx in range(-4,6):
        for gz in range(-2,9):
            q=[(gx,64,gz),(gx+1,64,gz),(gx+1,64,gz+1),(gx,64,gz+1)]
            pts=[proj(c,p) for p in q]
            if all(z>0.1 for _,z in pts):
                polys.append((max(z for _,z in pts)+100,[p for p,_ in pts],(95,150,70) if (gx+gz)%2 else (105,160,78)))
    for bi,b in enumerate(boxes):
        ctr=[sum(p[k] for p in b)/8 for k in range(3)]
        for fi,fc in enumerate(FACES):
            pts3=[b[i] for i in fc]
            fcen=[sum(p[k] for p in pts3)/4 for k in range(3)]
            nrm=[fcen[k]-ctr[k] for k in range(3)]; nl=math.sqrt(sum(x*x for x in nrm)) or 1; nrm=[x/nl for x in nrm]
            pts=[proj(c,p) for p in pts3]
            if any(z<=0.1 for _,z in pts): continue
            shade=0.55+0.45*max(0,nrm[1]*0.8+nrm[0]*0.3-nrm[2]*0.2)
            col=tuple(int(v*shade) for v in cols[bi%len(cols)])
            polys.append((sum(z for _,z in pts)/4,[p for p,_ in pts],col))
    polys.sort(key=lambda t:-t[0])
    for _,pts,col in polys: d.polygon(pts,fill=col,outline=(30,30,30))
    return im
c=cam_setup((3.5,66.8,-1.5),(0.3,64.4,4.0))
names=['ragdoll','fall_apart','crumble']
for si,scene in enumerate(data):
    frames=[frame(f,c) for f in scene]
    frames[0].save(f'prev_{names[si]}.gif',save_all=True,append_images=frames[1:],duration=33,loop=0)
    sheet=Image.new('RGB',(W*4,H))
    for i,k in enumerate([0,12,30,89]): sheet.paste(frames[k],(i*W,0))
    sheet.save(f'sheet_{names[si]}.png')
print('ok')
