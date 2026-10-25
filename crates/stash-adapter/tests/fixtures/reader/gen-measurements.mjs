// Generates measurements.json, the parity set for spec 005 SC-010.
//
// The block between the markers is copied verbatim from the semantic-tagging plugin,
// src/ui/matrix.js at commit 2c04141, lines 218-302 (parseMeasurements and its constants) and
// 304-356 (DERIVED_PERFORMER_FIELDS). It is the reference the engine's Rust derivation
// (crates/stash-adapter/src/reader/measurements.rs) must match; it is not engine code.
//
// usage: node gen-measurements.mjs   (reads measurements-input.json, writes measurements.json)
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// ---- begin verbatim copy (matrix.js @ 2c04141)
  // Cup ordinal scale, from a second user-supplied chart with half-cup
  // resolution (AA, A, A½, B, B½, C, C½, D, D½, E, E½, F, G, H...) —
  // replaces the previous chart's coarse 6-tier (XS–2XL) lookup with a
  // smoother, more continuous progression. E/DD, F/DDD, and G/DDDD are
  // synonyms in the source chart (each always shown as "E (DD)" etc.),
  // normalized to the plain letter here via CUP_LETTER_SYNONYMS before
  // lookup. No half-steps exist past E½ in the source chart (F/G/H+ are
  // whole-letter only), matching common convention.
  const CUP_LETTER_SYNONYMS = { DD: 'E', DDD: 'F', DDDD: 'G' };
  const CUP_ORDINAL_FINE = {
    AA: 0,
    A: 1, 'A½': 2,
    B: 3, 'B½': 4,
    C: 5, 'C½': 6,
    D: 7, 'D½': 8,
    E: 9, 'E½': 10,
    F: 11,
    G: 12,
    H: 13, I: 14, J: 15, K: 16, L: 17,
  };
  // Roughly preserves the previous chart-based system's per-letter
  // weight (was ~5 points per full letter across F+; each full letter is
  // now 2 ordinal steps, so ~2.5 per step lands in the same range) — a
  // starting point for this new scale, not a settled value.
  const CUP_ORDINAL_MULTIPLIER = 2.5;

  // A tighter waist makes the same band+cup read as a noticeably fuller
  // bust by contrast — real, not just personal perception, per how
  // similarly-sized performers get described/categorized differently
  // depending on waist. Every inch of waist below this threshold adds
  // WAIST_BONUS_PER_INCH; a 24in waist (4 under 28) adds 4*2=8.
  const WAIST_BONUS_THRESHOLD = 28;
  const WAIST_BONUS_PER_INCH = 2;

  function parseMeasurements(measurementsText) {
    if (!measurementsText || typeof measurementsText !== 'string') return null;
    // Cup size letters (A, B, C, DD, DDD, ...) are optional and variable-
    // length after the band number, optionally followed by a half-cup
    // marker (½, .5, or 1/2 — all normalized to ½ below) — e.g. "34D½"
    // or "34DD.5". Without capturing the half marker explicitly, it
    // would fall outside the letter-only capture group and break the
    // waist-hip match that follows it, silently losing waist/hip for any
    // half-cup-labeled measurement even though the band+cup itself would
    // still parse.
    //
    // The waist-hip suffix is OPTIONAL (wrapped in its own non-capturing
    // group) — some performer profiles only have a bra size on record
    // (e.g. measurements = "36DD" with nothing after it), and requiring
    // the full triplet meant those performers silently failed to parse
    // at all, vanishing from every Bust Size filter regardless of
    // threshold rather than being scored correctly. Waist/Hip/WHR still
    // correctly come back null for these — there's genuinely no data for
    // them — only Bust Size is recoverable from a bust-only string.
    const match = measurementsText.trim().match(/^(\d+)\s*([A-Za-z]*)\s*(½|\.5|1\/2)?\s*(?:-\s*(\d+)\s*-\s*(\d+))?/);
    if (!match) return null;
    const band = Number(match[1]);
    if (!band) return null;
    const rawCupLetters = (match[2] || '').toUpperCase();
    const hasHalf = !!match[3];
    const waist = match[4] ? Number(match[4]) : null;
    const hip = match[5] ? Number(match[5]) : null;

    const normalizedCupBase = CUP_LETTER_SYNONYMS[rawCupLetters] || rawCupLetters;
    const cupText = normalizedCupBase + (hasHalf ? '½' : '');
    const cupOrdinal = CUP_ORDINAL_FINE[cupText];

    // band is the base, cup ordinal adds a smooth per-half-step bonus on
    // top (unrecognized cup text — a brand-specific or non-US letter run
    // — falls back to band alone, same as prior versions of this).
    let bustScore = band;
    if (cupOrdinal !== undefined) {
      bustScore += cupOrdinal * CUP_ORDINAL_MULTIPLIER;
    }
    // Same cup+band can read as very different bust sizes depending on
    // waist — a tighter waist makes the same cup look fuller by contrast.
    // Below WAIST_BONUS_THRESHOLD inches, every inch under it adds
    // WAIST_BONUS_PER_INCH, only when waist data actually exists (no
    // phantom bonus for bust-only entries).
    if (waist !== null && waist < WAIST_BONUS_THRESHOLD) {
      bustScore += (WAIST_BONUS_THRESHOLD - waist) * WAIST_BONUS_PER_INCH;
    }
    const whr = waist && hip ? waist / hip : null;

    return { band, cupText, cupOrdinal: cupOrdinal ?? null, bustScore, waist, hip, whr };
  }

  const DERIVED_PERFORMER_FIELDS = [
    {
      name: '__derived_bust',
      label: 'Bust Size (from Measurements)',
      isDerived: true,
      valueKind: 'SCALAR',
      valueTypeName: 'Float', // decimals are expected now (e.g. 39.5) — the old ordinal model was integer-only, this one isn't
      valueIsList: false,
      hasValue2: true, // supports BETWEEN the same way a real Float criterion does
      valueEnumOptions: null,
      // band + fine-grained cup ordinal (from CUP_ORDINAL_FINE, half-cup
      // resolution) * CUP_ORDINAL_MULTIPLIER + a waist-tightness bonus
      // (see parseMeasurements' bustScore) — not literal band inches,
      // named "Bust Size" rather than "Bust" so it doesn't read as a raw
      // measurement. Grounded in a real sister-sizing chart's letter
      // progression rather than hand-tuned coefficients; still open to
      // further tuning.
      compute: (m) => m?.bustScore ?? null,
    },
    {
      name: '__derived_waist',
      label: 'Waist (from Measurements)',
      isDerived: true,
      valueKind: 'SCALAR',
      valueTypeName: 'Int',
      valueIsList: false,
      hasValue2: true,
      valueEnumOptions: null,
      compute: (m) => m?.waist ?? null,
    },
    {
      name: '__derived_hip',
      label: 'Hip (from Measurements)',
      isDerived: true,
      valueKind: 'SCALAR',
      valueTypeName: 'Int',
      valueIsList: false,
      hasValue2: true,
      valueEnumOptions: null,
      compute: (m) => m?.hip ?? null,
    },
    {
      name: '__derived_whr',
      label: 'Waist-to-Hip Ratio (from Measurements)',
      isDerived: true,
      valueKind: 'SCALAR',
      valueTypeName: 'Float',
      valueIsList: false,
      hasValue2: true,
      valueEnumOptions: null,
      compute: (m) => (m && m.whr !== null ? Math.round(m.whr * 100) / 100 : null),
    },
  ];
// ---- end verbatim copy

const here = dirname(fileURLToPath(import.meta.url));
const inputs = JSON.parse(readFileSync(join(here, 'measurements-input.json'), 'utf8'));
const field = (name) => DERIVED_PERFORMER_FIELDS.find((f) => f.name === name).compute;
const out = inputs.map((measurements) => {
  const m = parseMeasurements(measurements);
  return {
    measurements,
    bust: field('__derived_bust')(m),
    waist: field('__derived_waist')(m),
    hip: field('__derived_hip')(m),
    whr: field('__derived_whr')(m),
  };
});
writeFileSync(join(here, 'measurements.json'), JSON.stringify(out, null, 1) + '\n');
console.log(`${out.length} cases written`);
