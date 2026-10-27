import {describe, expect, it} from 'vitest';
import {BACKING_FADE_EM, BACKING_HEIGHT_EM, backingAspect, backingAtlas, backingIcon} from '../src/backing.js';

/** The soft backing behind a name: opaque over the text, fading to nothing past it, as wide as the name. */
describe('the label backing', () => {
  const atlas = backingAtlas();
  const alpha = (key: string, x: number, y: number) => {
    const m = atlas.mapping[key]!;
    return atlas.data[((m.y + y) * atlas.width + m.x + x) * 4 + 3]!;
  };

  it('is opaque where the text sits and fades to nothing at its edge, without a step', () => {
    const key = backingIcon(backingAspect(120, 14));
    const m = atlas.mapping[key]!;
    const mid = Math.floor(m.height / 2);
    expect(alpha(key, Math.floor(m.width / 2), mid)).toBe(255);
    expect(alpha(key, Math.floor(m.width / 2), 0)).toBeLessThan(8);
    expect(alpha(key, 0, mid)).toBeLessThan(8);
    // From the centre outward along the middle row, the alpha never rises and moves in small steps.
    const row = Array.from({length: Math.floor(m.width / 2)}, (_, i) => alpha(key, Math.floor(m.width / 2) + i, mid));
    for (let i = 1; i < row.length; i++) {
      expect(row[i]!).toBeLessThanOrEqual(row[i - 1]!);
      expect(row[i - 1]! - row[i]!).toBeLessThan(80);
    }
  });

  it('is as wide as the name and its fade at both ends, within the atlas step', () => {
    for (const [width, size] of [[40, 13], [120, 14], [300, 16]] as const) {
      const m = atlas.mapping[backingIcon(backingAspect(width, size))]!;
      const drawn = (m.width / m.height) * BACKING_HEIGHT_EM * size;
      expect(Math.abs(drawn - (width + 2 * BACKING_FADE_EM * size)) / drawn).toBeLessThan(0.06);
    }
  });
});
