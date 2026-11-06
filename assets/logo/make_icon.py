import math, sys
from PIL import Image, ImageDraw

S = 1024

def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(len(a)))

def tile():
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    bg = Image.new("RGBA", (S, S))
    px = bg.load()
    for y in range(S):
        for x in range(S):
            t = (x * 0.4 + y * 0.6) / S
            px[x, y] = lerp((22, 18, 40, 255), (10, 10, 18, 255), t)
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle((40, 40, S - 40, S - 40), radius=220, fill=255)
    img.paste(bg, (0, 0), mask)
    return img

def stroke_path():
    pts = []
    for i in range(41):
        t = i / 40
        pts.append((0.24 + 0.24 * t, 0.2 + 0.52 * t))
    for i in range(1, 41):
        t = i / 40
        pts.append((0.48 + 0.24 * t, 0.72 - 0.52 * t))
    return pts

def tail_path():
    pts = []
    for i in range(0, 61):
        t = i / 60
        x = 0.48 + 0.34 * t
        y = 0.80 + 0.045 * math.sin(t * math.tau * 1.5) * (1 - t * 0.3)
        pts.append((x, y))
    return pts

def draw_grad(img, pts, width):
    d = ImageDraw.Draw(img)
    c0, c1 = (0, 214, 255, 255), (255, 60, 200, 255)
    n = len(pts)
    for i in range(n - 1):
        t = i / (n - 1)
        col = lerp(c0, c1, t)
        a = (pts[i][0] * S, pts[i][1] * S)
        b = (pts[i + 1][0] * S, pts[i + 1][1] * S)
        d.line([a, b], fill=col, width=width)
        r = width / 2
        d.ellipse((b[0] - r, b[1] - r, b[0] + r, b[1] + r), fill=col)
    r = width / 2
    a = (pts[0][0] * S, pts[0][1] * S)
    d.ellipse((a[0] - r, a[1] - r, a[0] + r, a[1] + r), fill=c0)

def make(out_dir):
    img = tile()
    glow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    v = stroke_path()
    draw_grad(glow, v, 150)
    from PIL import ImageFilter
    glow = glow.filter(ImageFilter.GaussianBlur(40))
    g = glow.split()[3].point(lambda a: int(a * 0.55))
    glow.putalpha(g)
    img.alpha_composite(glow)
    draw_grad(img, v, 96)
    draw_grad(img, tail_path(), 56)
    sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
    imgs = [img.resize((s, s), Image.LANCZOS) for s in sizes]
    imgs[-1].save(f"{out_dir}/vizz.ico", sizes=[(s, s) for s in sizes], append_images=imgs[:-1])
    img.resize((512, 512), Image.LANCZOS).save(f"{out_dir}/vizz_icon.png")
    img.resize((256, 256), Image.LANCZOS).save(f"{out_dir}/vizz_icon_256.png")
    sheet = Image.new("RGBA", (16 + 24 + 32 + 48 + 256 + 60, 256), (40, 40, 40, 255))
    x = 0
    for s in (16, 24, 32, 48, 256):
        sheet.alpha_composite(img.resize((s, s), Image.LANCZOS), (x, 256 - s))
        x += s + 12
    sheet.save(f"{out_dir}/icon_sizes.png")

make(sys.argv[1])
