# Generates the seamless sidebar tile of irregular tesserae (opus tessellatum). Run: python3 scripts/tesserae.py ui/src/assets
import random, sys
def tile(palette, alpha, seed=7, n=10, c=6.0):
    r = random.Random(seed)
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{n*c:g}" height="{n*c:g}">']
    for y in range(n):
        for x in range(n):
            s = r.uniform(4.3, 5.1)
            cx, cy = x*c + c/2 + r.uniform(-0.3, 0.3), y*c + c/2 + r.uniform(-0.3, 0.3)
            rot = r.uniform(-5, 5)
            col = r.choice(palette)
            a = r.uniform(*alpha)
            out.append(f'<rect x="{cx-s/2:.2f}" y="{cy-s/2:.2f}" width="{s:.2f}" height="{s:.2f}" rx="0.6" fill="{col}" fill-opacity="{a:.3f}" transform="rotate({rot:.1f} {cx:.2f} {cy:.2f})"/>')
    out.append('</svg>')
    return ''.join(out)
d = sys.argv[1]
open(f'{d}/tesserae-light.svg', 'w').write(tile(['#9c4a2f', '#b8893a', '#5a5248', '#c06a3c', '#7d7a4a', '#4f5d6e'], (0.025, 0.07)) + '\n')
open(f'{d}/tesserae-dark.svg', 'w').write(tile(['#e0a184', '#e3c27a', '#d8d2c8', '#eab07a', '#c8c48e', '#a9b6c8'], (0.02, 0.055)) + '\n')
