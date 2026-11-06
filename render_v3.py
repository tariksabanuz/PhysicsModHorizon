"""Renders the vertex soups of the ignored `dump_v3` test (soft puffs, alpha blended, drawn in
order) over a plain ground and sky, for a look at the particles without the game."""
import json, sys, numpy as np
from PIL import Image

W, H, F = 720, 540, 560.0

def background(cam_y, roof, ground=(0.36, 0.55, 0.25), snow=False):
    ys, xs = np.mgrid[0:H, 0:W].astype(np.float32)
    dx, dy, dz = (xs - W / 2) / F, -(ys - H / 2) / F, np.ones_like(xs)
    img = np.zeros((H, W, 3), np.float32)
    sky = np.clip(dy * 1.5 + 0.3, 0, 1)[..., None]
    img[:] = (1 - sky) * np.array([0.78, 0.86, 0.98]) + sky * np.array([0.45, 0.63, 0.95])
    with np.errstate(divide='ignore', invalid='ignore'):
        t = ((64.125 if snow else 64.0) - cam_y) / dy
        hit = (t > 0) & (dy < 0)
        gx, gz = dx * t, dz * t
        check = ((np.floor(gx) + np.floor(gz)) % 2 == 0)[..., None]
        g = np.array(ground) * np.where(check, 1.0, 0.92)
        fog = np.clip(t / 60.0, 0, 1)[..., None]
        img = np.where(hit[..., None], g * (1 - fog) + img * fog, img)
        if roof:
            t2 = (68.0 - cam_y) / dy
            rx, rz = 0.5 + dx * t2, dz * t2 - 7.0
            r = (t2 > 0) & (dy > 0) & (rx > -3) & (rx < 4) & (rz > -3) & (rz < 4)
            img = np.where(r[..., None], np.array([0.42, 0.3, 0.18]) * 0.6, img)
    return img

def draw(img, soup):
    v = np.array([[a[0], a[1], a[2]] for a in soup], np.float32)
    c = np.array([a[3] for a in soup], np.uint32)
    rgba = np.stack([(c & 255), (c >> 8) & 255, (c >> 16) & 255, (c >> 24) & 255], 1).astype(np.float32) / 255
    sx = W / 2 + v[:, 0] / v[:, 2] * F
    sy = H / 2 - v[:, 1] / v[:, 2] * F
    for q in range(0, len(v), 4):
        if (v[q:q + 4, 2] <= 0.05).any():
            continue
        for tri in ((q, q + 1, q + 2), (q, q + 2, q + 3)):
            xs, ys = sx[list(tri)], sy[list(tri)]
            x0, x1 = int(max(0, np.floor(xs.min()))), int(min(W - 1, np.ceil(xs.max())))
            y0, y1 = int(max(0, np.floor(ys.min()))), int(min(H - 1, np.ceil(ys.max())))
            if x1 < x0 or y1 < y0:
                continue
            area = (xs[1] - xs[0]) * (ys[2] - ys[0]) - (xs[2] - xs[0]) * (ys[1] - ys[0])
            if abs(area) < 1e-6:
                continue
            py, px = np.mgrid[y0:y1 + 1, x0:x1 + 1].astype(np.float32) + 0.5
            w0 = ((xs[1] - px) * (ys[2] - py) - (xs[2] - px) * (ys[1] - py)) / area
            w1 = ((xs[2] - px) * (ys[0] - py) - (xs[0] - px) * (ys[2] - py)) / area
            w2 = 1 - w0 - w1
            inside = (w0 >= 0) & (w1 >= 0) & (w2 >= 0)
            if not inside.any():
                continue
            col = w0[..., None] * rgba[tri[0]] + w1[..., None] * rgba[tri[1]] + w2[..., None] * rgba[tri[2]]
            a = col[..., 3:4]
            keep = (inside & (col[..., 3] >= 0.1))[..., None]
            region = img[y0:y1 + 1, x0:x1 + 1]
            img[y0:y1 + 1, x0:x1 + 1] = np.where(keep, region * (1 - a) + col[..., :3] * a, region)
    return img

if __name__ == '__main__':
    out = sys.argv[1]
    scenes = [('roof', 65.7, True), ('blast_a', 66.0, False), ('blast_b', 66.0, False), ('blast_c', 66.0, False), ('splash', 66.3, False), ('snow', 65.4, False)]
    for name, cy, roof in scenes:
        soup = json.load(open(f'/tmp/hp3_{name}.json'))
        img = draw(background(cy, roof, {'splash': (0.2, 0.35, 0.6), 'snow': (0.93, 0.95, 0.98)}.get(name, (0.36, 0.55, 0.25)), name == 'snow'), soup)
        Image.fromarray((np.clip(img, 0, 1) * 255).astype(np.uint8)).save(f'{out}/{name}.png')
        print(name, len(soup))
