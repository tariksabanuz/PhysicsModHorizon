# Renders the 0.2.0 preview scenes (/tmp/hp_dump2.json from the ignored `dump_v2` test) as GIFs:
# flat-shaded polygons, painter's order, a checkered floor.
import json, math, sys
from PIL import Image, ImageDraw
W, H = 520, 340
data = json.load(open('/tmp/hp_dump2.json'))
def cam(eye, target):
    f = [t - e for t, e in zip(target, eye)]; n = math.sqrt(sum(x * x for x in f)); f = [x / n for x in f]
    up = (0, 1, 0)
    r = [f[1] * up[2] - f[2] * up[1], f[2] * up[0] - f[0] * up[2], f[0] * up[1] - f[1] * up[0]]
    n = math.sqrt(sum(x * x for x in r)); r = [x / n for x in r]
    u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]]
    return eye, f, r, u
def proj(c, p):
    eye, f, r, u = c; d = [a - b for a, b in zip(p, eye)]
    z = sum(a * b for a, b in zip(d, f)); x = sum(a * b for a, b in zip(d, r)); y = sum(a * b for a, b in zip(d, u))
    s = 1.25 / max(z, 0.05)
    return (W / 2 + x * s * H / 2, H / 2 - y * s * H / 2), z
LIGHT = (0.35, 0.85, -0.4)
def render(frames, c, out, grid):
    imgs = []
    for polys in frames:
        im = Image.new('RGB', (W, H), (150, 190, 235)); d = ImageDraw.Draw(im)
        items = []
        for gx in range(grid[0], grid[1]):
            for gz in range(grid[2], grid[3]):
                q = [(gx, 64, gz), (gx + 1, 64, gz), (gx + 1, 64, gz + 1), (gx, 64, gz + 1)]
                pts = [proj(c, p) for p in q]
                if all(z > 0.1 for _, z in pts):
                    items.append((1e9, [p for p, _ in pts], (95, 150, 70) if (gx + gz) % 2 else (105, 160, 78)))
        for pts3, col in polys:
            if len(pts3) < 3: continue
            a, b, cc = pts3[0], pts3[1], pts3[2]
            e1 = [b[k] - a[k] for k in range(3)]; e2 = [cc[k] - a[k] for k in range(3)]
            n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]]
            nl = math.sqrt(sum(x * x for x in n)) or 1; n = [x / nl for x in n]
            shade = 0.6 + 0.4 * abs(sum(n[k] * LIGHT[k] for k in range(3)))
            pts = [proj(c, p) for p in pts3]
            if any(z <= 0.1 for _, z in pts): continue
            items.append((sum(z for _, z in pts) / len(pts), [p for p, _ in pts], tuple(min(255, int(v * shade)) for v in col)))
        items.sort(key=lambda t: -t[0])
        for _, pts, col in items:
            d.polygon(pts, fill=col)
        imgs.append(im)
    imgs[0].save(out, save_all=True, append_images=imgs[1:], duration=33, loop=0, optimize=True)
    w = len(imgs) // 4
    sheet = Image.new('RGB', (W * 2, H * 2))
    for i in range(4):
        sheet.paste(imgs[min(len(imgs) - 1, 8 + i * w)], ((i % 2) * W, (i // 2) * H))
    sheet.save(out.replace('.gif', '.png').replace('prev_', 'sheet_'))
render(data[0], cam((-8.5, 70.5, -5.0), (0.5, 64.8, 4.5)), 'prev_explosion.gif', (-8, 10, -6, 14))
render(data[1], cam((-0.5, 66.3, -0.8), (1.6, 65.0, 3.6)), 'prev_cape_lead.gif', (-6, 8, -2, 10))
