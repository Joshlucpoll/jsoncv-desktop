// Roman mosaic accents: "curriculum vitae" is Latin, so the sidebar borrows a meander frieze from opus tessellatum.

// One period of a running meander (Greek key), one tessera per cell; '#' is the key, '.' the ground.
const KEY = [
  '#######.',
  '#.....#.',
  '#.###.#.',
  '#.#.#.#.',
  '#.#...#.',
  '#.#####.',
  '#.......',
  '########',
]
const C = 3 // tessera pitch in px
const GROUT = 0.6
const PERIODS = 9 // whole periods only, so the key never ends cut in half; 9 fill the 240px sidebar

/** A meander frieze of whole key periods. */
export function Meander() {
  return (
    <svg className="meander" width={PERIODS * KEY[0].length * C} height={KEY.length * C} aria-hidden="true">
      <defs>
        <pattern id="meander" width={KEY[0].length * C} height={KEY.length * C} patternUnits="userSpaceOnUse">
          {KEY.flatMap((row, y) =>
            [...row].map((cell, x) => (
              <rect
                key={`${x},${y}`}
                // Mix in a lighter and a darker stone for some tesserae, picked the same way every render.
                className={`${cell === '#' ? 'tess-key' : 'tess-ground'} v${(x * 7 + y * 13 + x * y) % 4}`}
                x={x * C + GROUT / 2} y={y * C + GROUT / 2} width={C - GROUT} height={C - GROUT}
                // Hand-set tesserae never match exactly: vary each one's tone a little, the same way every render.
                opacity={0.78 + ((x * 7 + y * 13) % 5) * 0.055}
              />
            )),
          )}
        </pattern>
      </defs>
      <rect width="100%" height="100%" fill="url(#meander)" />
    </svg>
  )
}

