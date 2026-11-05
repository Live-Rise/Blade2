import os, sys
from PIL import Image

base = r"E:\Syncthing\DshWinUI\tmp-cap"
out = r"E:\Syncthing\DshWinUI\tmp-cap\R4-crops"
os.makedirs(out, exist_ok=True)

def crop(side, name, box, tag, scale=2):
    p = os.path.join(base, side, name + ".png")
    im = Image.open(p).convert("RGB")
    W, H = im.size
    x0, y0, x1, y1 = box
    # normalize: allow fractional coords relative to 2000x1250 reference
    x0 = int(x0 / 2000 * W); x1 = int(x1 / 2000 * W)
    y0 = int(y0 / 1250 * H); y1 = int(y1 / 1250 * H)
    c = im.crop((x0, y0, x1, y1))
    if scale and scale != 1:
        c = c.resize((c.width * scale, c.height * scale), Image.LANCZOS)
    fp = os.path.join(out, f"{tag}.png")
    c.save(fp)
    print(fp, c.size)

M = "parity-main-R4"
R = "parity-rust-R4"

# sizes probe
for side in (M, R):
    for name in ("S2-session", "P-usage", "P-pet", "P-memory", "P-personalization"):
        im = Image.open(os.path.join(base, side, name + ".png"))
        print(side, name, im.size)
print("---crops---")

# S2 tool row: chevron + leading area before title
crop(M, "S2-session", (440, 715, 1250, 775), "S2-main-toolrow")
crop(R, "S2-session", (325, 150, 1000, 200), "S2-rust-toolrow")
# S2 context rows chevron area
crop(M, "S2-session", (440, 355, 1000, 545), "S2-main-ctxrows")
crop(R, "S2-session", (325, 375, 800, 445), "S2-rust-ctxrows")
# S2 stats line
crop(M, "S2-session", (900, 1050, 1500, 1100), "S2-main-stats")
crop(R, "S2-session", (650, 810, 1150, 850), "S2-rust-stats")

# P-usage tabs + axis + legend
crop(M, "P-usage", (1500, 480, 1950, 560), "U-main-tabs", 2)
crop(R, "P-usage", (1500, 420, 1950, 500), "U-rust-tabs", 2)
crop(M, "P-usage", (760, 740, 1560, 870), "U-main-axis")
crop(R, "P-usage", (760, 660, 1560, 790), "U-rust-axis")
crop(M, "P-usage", (1550, 800, 1950, 880), "U-main-legend", 2)
crop(R, "P-usage", (1550, 700, 1950, 800), "U-rust-legend", 2)
crop(M, "P-usage", (1550, 890, 1950, 990), "U-main-range", 2)
crop(R, "P-usage", (1550, 790, 1950, 890), "U-rust-range", 2)

# P-pet toggle + list boundaries
crop(M, "P-pet", (1600, 390, 1960, 480), "Pet-main-toggle", 2)
crop(R, "P-pet", (1600, 370, 1960, 460), "Pet-rust-toggle", 2)
crop(M, "P-pet", (470, 600, 1400, 850), "Pet-main-listtop", 1)
crop(R, "P-pet", (470, 570, 1400, 830), "Pet-rust-listtop", 1)

# P-memory toggle + storage card
crop(M, "P-memory", (1600, 290, 1960, 400), "Mem-main-toggle", 2)
crop(R, "P-memory", (1600, 290, 1960, 400), "Mem-rust-toggle", 2)
crop(M, "P-memory", (470, 420, 1100, 560), "Mem-main-storage")
crop(R, "P-memory", (470, 420, 1100, 560), "Mem-rust-storage")
crop(M, "P-memory", (470, 1120, 1100, 1200), "Mem-main-statusline", 2)
crop(R, "P-memory", (470, 1100, 1100, 1180), "Mem-rust-statusline", 2)

# P-personalization status line
crop(M, "P-personalization", (470, 590, 1100, 680), "Per-main-statusline", 2)
crop(R, "P-personalization", (470, 580, 1100, 680), "Per-rust-statusline", 2)
