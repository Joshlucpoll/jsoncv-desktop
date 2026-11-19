# Generates the app icon: a window on the running meander from ui/src/components/Mosaic.tsx, set in tesserae on a macOS squircle.
# Run: python3 scripts/icon.py src-tauri/icons/icon.svg && cargo tauri icon src-tauri/icons/icon.svg && rm -rf src-tauri/icons/{android,ios}
import math, random, sys
KEY = ['#########.', '#.......#.', '#.#####.#.', '#.#...#.#.', '#.#.#.#.#.', '#.#.#...#.', '#.#.#####.', '#.#.......', '#.########']  # keep in sync with Mosaic.tsx
# A 13-tessera window on the running frieze: one whole key in the middle, its neighbours cut off at both edges.
GRID = ['.' * 13] * 2 + [(row * 3)[8:21] for row in KEY] + ['.' * 13] * 2
# Hand-cut stone: each tessera is a slightly skewed quad in its own shade, so the grid never reads as pixels.
# Only the key is set in stone; the ground is smooth limestone plaster, which keeps the icon calm.
KEY_FILL = ['#a8502f', '#9c4529', '#b35a35', '#8f4329', '#b8693f', '#a04b2c']  # terracotta around --tess-key, as in the sidebar frieze
BODY, X0 = 824, 100  # Apple's 1024 icon grid: 824px body, 100px margin
C = BODY / len(GRID)
r = random.Random(42)  # fixed seed: the same stones every run
G = '0.33 0.33 0.33 0 0 0.33 0.33 0.33 0 0 0.33 0.33 0.33 0 0 0 0 0 0 1'  # noise to grey
# Shading overlays: each stone gets one, a faint fall-off from one side to the other, at a random angle.
SHADES = ''.join(
    f'<linearGradient id="g{i}" gradientTransform="rotate({i * 45} 0.5 0.5)">'
    '<stop offset="0" stop-color="#fff" stop-opacity="0.1"/><stop offset="0.5" stop-color="#fff" stop-opacity="0"/>'
    '<stop offset="1" stop-color="#000" stop-opacity="0.16"/></linearGradient>' for i in range(8))
out = ['<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">',
       f'<clipPath id="sq"><rect x="{X0}" y="{X0}" width="{BODY}" height="{BODY}" rx="185"/></clipPath>', SHADES,
       '<radialGradient id="vig"><stop offset="0.55" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.2"/></radialGradient>',
       # Stone: smooth worn edges, broad mottling inside each stone, faint grain, and a shallow shadow into the grout.
       '<filter id="stone" x="0" y="0" width="100%" height="100%">'
       '<feGaussianBlur in="SourceGraphic" stdDeviation="0.9" result="soft"/>'
       '<feTurbulence type="fractalNoise" baseFrequency="0.05" numOctaves="3" seed="4" result="m"/>'
       f'<feColorMatrix in="m" type="matrix" values="{G}" result="mg"/>'
       '<feComposite in="soft" in2="mg" operator="arithmetic" k2="1" k3="0.34" k4="-0.17" result="mot"/>'
       '<feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" seed="5" result="n"/>'
       f'<feColorMatrix in="n" type="matrix" values="{G}" result="ng"/>'
       '<feComposite in="mot" in2="ng" operator="arithmetic" k2="1" k3="0.16" k4="-0.08"/>'
       '<feComposite in2="soft" operator="in"/>'
       '<feDropShadow dx="1" dy="1.5" stdDeviation="2.5" flood-color="#3d2f1d" flood-opacity="0.35"/></filter>'
       # Whole surface: soft blotchy wear, and a little grit over stone and grout alike.
       '<filter id="age" x="0" y="0" width="100%" height="100%">'
       '<feTurbulence type="fractalNoise" baseFrequency="0.012" numOctaves="4" seed="7" result="lo"/>'
       f'<feColorMatrix in="lo" type="matrix" values="{G}" result="log"/>'
       '<feComposite in="SourceGraphic" in2="log" operator="arithmetic" k2="1" k3="0.3" k4="-0.15" result="worn"/>'
       '<feTurbulence type="fractalNoise" baseFrequency="1.4" numOctaves="2" seed="11" result="hi"/>'
       f'<feColorMatrix in="hi" type="matrix" values="{G}" result="hig"/>'
       '<feComposite in="worn" in2="hig" operator="arithmetic" k2="1" k3="0.12" k4="-0.06"/>'
       '<feComposite in2="SourceGraphic" operator="in"/></filter>'
       '<filter id="blur"><feGaussianBlur stdDeviation="5"/></filter>',
       f'<g clip-path="url(#sq)"><g filter="url(#age)"><rect x="{X0}" y="{X0}" width="{BODY}" height="{BODY}" fill="#d9cfbd"/><g filter="url(#stone)">']
for y, row in enumerate(GRID):
    for x, cell in enumerate(row):
        if cell != '#':
            continue
        cx, cy = X0 + (x + 0.5) * C + r.uniform(-1.5, 1.5), X0 + (y + 0.5) * C + r.uniform(-1.5, 1.5)
        h = C * r.uniform(0.38, 0.43)  # half-width: grout of 14-24% of the pitch, uneven like hand-set work
        # Four corners, each nudged on its own, then the whole stone turned a few degrees.
        a = math.radians(r.uniform(-5, 5))
        pts = []
        for dx, dy in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
            px, py = dx * h + r.uniform(-3, 3), dy * h + r.uniform(-3, 3)
            pts.append(f'{cx + px * math.cos(a) - py * math.sin(a):.1f},{cy + px * math.sin(a) + py * math.cos(a):.1f}')
        pts = " ".join(pts)
        fill = r.choice(KEY_FILL)
        out.append(f'<polygon points="{pts}" fill="{fill}" stroke="{fill}" stroke-width="5" stroke-linejoin="round"/>'
                   f'<polygon points="{pts}" fill="url(#g{r.randrange(8)})"/>')
# An engraved groove just inside the edge (shadow below, highlight offset under it) gives the eye a frame to stop at.
out.append('</g><rect x="130" y="131" width="768" height="768" rx="160" fill="none" stroke="#fff" stroke-opacity="0.35" stroke-width="5" filter="url(#blur)"/>'
           '<rect x="128" y="128" width="768" height="768" rx="160" fill="none" stroke="#3d2f1d" stroke-opacity="0.4" stroke-width="5" filter="url(#blur)"/>')
out.append(f'</g><rect x="{X0}" y="{X0}" width="{BODY}" height="{BODY}" fill="url(#vig)"/></g></svg>')
open(sys.argv[1], 'w').write('\n'.join(out) + '\n')
