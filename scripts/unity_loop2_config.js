import {
  ZodiacRarity,
  UnityZodiac,
  States,
  ZodiacSign,
  ZodiacElement,
  ZodiacStatType,
} from "./lib/states";

import { mantissa, exponent } from "./lib/utils";


const MIN_LEVEL = 100;
const MIN_RARITY = ZodiacRarity.Divine;


function zodiacOf(zodiac) {
  return zodiac instanceof UnityZodiac
    ? zodiac
    : new UnityZodiac(zodiac);
}


function primaryStat(zodiac) {
  return zodiac.stats[0]?.type;
}


function hasStat(zodiac, type) {
  return zodiac.stats.some(stat => stat.type === type);
}


function isMultGainSacrifice(zodiac) {
  return primaryStat(zodiac) === ZodiacStatType.MultsGain;
}


function isAriesMultBuildCandidate(zodiac) {
  return (
    zodiac.sign === ZodiacSign.Aries &&
    zodiac.level >= MIN_LEVEL &&
    zodiac.rarity >= MIN_RARITY &&
    hasStat(zodiac, ZodiacStatType.MultsGain)
  );
}


// For positive values in scientific notation.
// Returns < 0 if a < b, 0 if equal, > 0 if a > b.
function compareScientific(a, b) {
  const ea = exponent(a);
  const eb = exponent(b);

  if (ea !== eb)
    return ea < eb ? -1 : 1;

  const ma = mantissa(a);
  const mb = mantissa(b);

  return ma - mb;
}


function compareAttackZodiac(a, b) {
  return compareScientific(a.score, b.score);
}


export default {
  unity_run_threshold_s: 200,
  attack_eta_threshold_s: 10,

  ECWaitTime: 3000,
  attackMode: true,


  async zodiacToGetOnNextUnit() {
    const POSITION = ["Left", "Top", "Bottom", "Right"];
    const zodiacs = await States.nextUnityZodiacs();

    let bestIndex = 0;
    let bestPriority = -Infinity;

    for (let i = 0; i < zodiacs.length; ++i) {
      const zodiac = zodiacs[i];

      let priority = 0;

      // Main target: Aries.
      if (zodiac.sign === ZodiacSign.Aries)
        priority += 1_000_000;

      // Fallback to Fire.
      if (zodiac.Element === ZodiacElement.Fire)
        priority += 100_000;

      // Prefer actual Mult Gain.
      if (hasStat(zodiac, ZodiacStatType.MultsGain))
        priority += 10_000;

      priority += zodiac.rarity * 100;
      priority += zodiac.rarityPlus;

      if (priority > bestPriority) {
        bestPriority = priority;
        bestIndex = i;
      }
    }

    return POSITION[bestIndex];
  },


  shouldSellZodiac(/** @type {UnityZodiac} */ zodiac) {
    if (zodiac.locked)
      return false;

    // Don't sell something intended for Mult Gain sacrifice.
    if (isMultGainSacrifice(zodiac))
      return false;

    // Preserve good Aries for merging/equipping.
    if (isAriesMultBuildCandidate(zodiac))
      return false;

    return (
      zodiac.level < MIN_LEVEL ||
      zodiac.rarity < MIN_RARITY
    );
  },


  shouldSacrificeZodiac(/** @type {UnityZodiac} */ zodiac) {
    if (zodiac.locked)
      return false;

    // Good Aries are more useful as equipment/merge material.
    if (isAriesMultBuildCandidate(zodiac))
      return false;

    return isMultGainSacrifice(zodiac);
  },


  // Suffix can be used to differentiate different groups.
  // Merge only zodiacs with the same suffix.
  mergeKeySuffix(/** @type {UnityZodiac} */ zodiac) {
    if (isAriesMultBuildCandidate(zodiac))
      return "attack-mult";

    return "";
  },


  // Given the planet, current equipped zodiac, and all currently
  // available zodiacs, return the key to equip.
  equipZodiacFor(
    /** @type {"SUN" | "MERCURY" | "VENUS" | "MOON" | "MARS" | "JUPITER" | "SATURN" | "URANUS" | "NEPTUNE" | "PLUTO" | "CHIRON" | "FORTUNE"} */ planet,
    /** @type {UnityZodiac | null} */ currentZodiac,
    /** @type {Record<string, UnityZodiac>} */ possibleZodiacs
  ) {
    let bestKey = null;
    let bestZodiac = null;

    for (const [key, rawZodiac] of Object.entries(possibleZodiacs)) {
      const zodiac = zodiacOf(rawZodiac);

      if (!isAriesMultBuildCandidate(zodiac))
        continue;

      if (
        bestZodiac === null ||
        compareAttackZodiac(zodiac, bestZodiac) > 0
      ) {
        bestKey = key;
        bestZodiac = zodiac;
      }
    }

    // No suitable Aries available: leave this planet unchanged.
    return bestKey ?? planet;
  },
};