# Generates the app icon: one meander period from ui/src/components/Mosaic.tsx, set in tesserae on a macOS squircle.
# Run: python3 scripts/icon.py src-tauri/icons/icon.svg && cargo tauri icon src-tauri/icons/icon.svg && rm -rf src-tauri/icons/{android,ios}
import sys
KEY = ['#######.', '#.....#.', '#.###.#.', '#.#.#.#.', '#.#...#.', '#.#####.', '#.......', '########']  # keep in sync with Mosaic.tsx
GRID = ['.' * 10] + ['.' + row + '.' for row in KEY] + ['.' * 10]  # one ground course around the key
KEY_FILL = ['#d98a68', '#df9970', '#c97a5a', '#d98a68']  # dark-theme --tess-key and its v1/v2 mixes from styles.css
GROUND_FILL = ['#3a2c24', '#33271f', '#3a2c24', '#41322a']  # dark umber ground so the key stands out
BODY, X0 = 824, 100  # Apple's 1024 icon grid: 824px body, 100px margin
C = BODY / len(GRID)
GAP = C * 0.2  # same grout ratio as the sidebar frieze (0.6 of 3px)
out = ['<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">',
       f'<clipPath id="sq"><rect x="{X0}" y="{X0}" width="{BODY}" height="{BODY}" rx="185"/></clipPath>',
       f'<g clip-path="url(#sq)"><rect x="{X0}" y="{X0}" width="{BODY}" height="{BODY}" fill="#1f1713"/>']
for y, row in enumerate(GRID):
    for x, cell in enumerate(row):
        v = (x * 7 + y * 13 + x * y) % 4  # same per-tessera variation as Mosaic.tsx
        fill = (KEY_FILL if cell == '#' else GROUND_FILL)[v]
        op = 0.78 + ((x * 7 + y * 13) % 5) * 0.055
        out.append(f'<rect x="{X0 + x * C + GAP / 2:.1f}" y="{X0 + y * C + GAP / 2:.1f}" width="{C - GAP:.1f}" height="{C - GAP:.1f}" '
                   f'rx="{C * 0.06:.1f}" fill="{fill}" fill-opacity="{op:.3f}"/>')
out.append('</g></svg>')
open(sys.argv[1], 'w').write('\n'.join(out) + '\n')
