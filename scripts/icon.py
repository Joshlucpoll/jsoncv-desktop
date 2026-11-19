# Generates the app icon: a window on the running meander from ui/src/components/Mosaic.tsx, set in tesserae on a macOS squircle.
# Run: python3 scripts/icon.py src-tauri/icons/icon.svg && cargo tauri icon src-tauri/icons/icon.svg && rm -rf src-tauri/icons/{android,ios}
import sys
KEY = ['#########.', '#.......#.', '#.#####.#.', '#.#...#.#.', '#.#.#.#.#.', '#.#.#...#.', '#.#.#####.', '#.#.......', '#.########']  # keep in sync with Mosaic.tsx
# A 13-tessera window on the running frieze: one whole key in the middle, its neighbours cut off at both edges.
GRID = ['.' * 13] * 2 + [(row * 3)[8:21] for row in KEY] + ['.' * 13] * 2
KEY_FILL = ['#a8502f', '#b8693f', '#8f4329', '#a8502f']  # --tess-key and its v1/v2 mixes from styles.css
GROUND_FILL = ['#efe3c8', '#e9d6b4', '#efe3c8', '#efe3c8']
BODY, X0 = 824, 100  # Apple's 1024 icon grid: 824px body, 100px margin
C = BODY / len(GRID)
GAP = C * 0.2  # same grout ratio as the sidebar frieze (0.6 of 3px)
out = ['<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">',
       f'<clipPath id="sq"><rect x="{X0}" y="{X0}" width="{BODY}" height="{BODY}" rx="185"/></clipPath>',
       f'<g clip-path="url(#sq)"><rect x="{X0}" y="{X0}" width="{BODY}" height="{BODY}" fill="#cbb48c"/>']
for y, row in enumerate(GRID):
    for x, cell in enumerate(row):
        v = (x * 7 + y * 13 + x * y) % 4  # same per-tessera variation as Mosaic.tsx
        fill = (KEY_FILL if cell == '#' else GROUND_FILL)[v]
        op = 0.78 + ((x * 7 + y * 13) % 5) * 0.055
        out.append(f'<rect x="{X0 + x * C + GAP / 2:.1f}" y="{X0 + y * C + GAP / 2:.1f}" width="{C - GAP:.1f}" height="{C - GAP:.1f}" '
                   f'rx="{C * 0.06:.1f}" fill="{fill}" fill-opacity="{op:.3f}"/>')
out.append('</g></svg>')
open(sys.argv[1], 'w').write('\n'.join(out) + '\n')
