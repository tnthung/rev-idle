import {
  ZodiacRarity,
  UnityZodiac,
  States,
  ZodiacSign,
  ZodiacElement,
  ZodiacStatType,
} from "./lib/states";

import {
  BigNum,
} from "./lib/utils";


const MIN_LEVEL = new BigNum(100);
const MIN_RARITY = ZodiacRarity.Divine;

const PRIORITY_MULT_GAIN = new BigNum(1_000_000_000);
const PRIORITY_GAME_SPEED = new BigNum(100_000_000);
const PRIORITY_ARIES = new BigNum(10_000_000);
const PRIORITY_GAME_SPEED_STAT = new BigNum(1_000_000);
const PRIORITY_FIRE = new BigNum(100_000);


// Keep Mult Gain on these 4 planets.
// These were among your weaker GS slots, so replacing them costs less GS.
const MULT_GAIN_PLANETS = new Set([
  "SATURN",
  "URANUS",
  "NEPTUNE",
  "PLUTO",
]);


function primaryStat(zodiac) {
  return zodiac.stats[0]?.type;
}


function statOf(zodiac, type) {
  return zodiac.stats.find(stat => stat.type === type);
}


function hasStat(zodiac, type) {
  return zodiac.stats.some(stat => stat.type === type);
}


function compareByStat(a, b, type) {
  const av = statOf(a, type)?.value;
  const bv = statOf(b, type)?.value;

  if (av == null && bv == null)
    return a.score.compareTo(b.score);

  if (av == null) return -1;
  if (bv == null) return 1;

  return av.compareTo(bv) || a.score.compareTo(b.score);
}


function isGoodZodiac(zodiac) {
  return (
    zodiac.level.compareTo(MIN_LEVEL) >= 0 &&
    zodiac.rarity >= MIN_RARITY);
}


function isMultGainSacrifice(zodiac) {
  return primaryStat(zodiac) === ZodiacStatType.MultsGain;
}


function isAriesMultBuildCandidate(zodiac) {
  return (
    isGoodZodiac(zodiac) &&
    zodiac.sign === ZodiacSign.Aries &&
    hasStat(zodiac, ZodiacStatType.MultsGain)
  );
}


function isGameSpeedBuildCandidate(zodiac) {
  return (
    isGoodZodiac(zodiac) &&
    hasStat(zodiac, ZodiacStatType.GameSpeed));
}


function compareMultGainZodiac(a, b) {
  return compareByStat(a, b, ZodiacStatType.MultsGain);
}


function compareGameSpeedZodiac(a, b) {
  return compareByStat(a, b, ZodiacStatType.GameSpeed);
}


function findBest(
  /** @type {Record<string, UnityZodiac>} */
  possibleZodiacs,
  predicate,
  comparator,
) {
  let bestKey = null;
  let bestZodiac = null;

  for (const [key, zodiac] of Object.entries(possibleZodiacs)) {
    if (!zodiac)
      continue;

    if (!predicate(zodiac))
      continue;

    if (bestZodiac === null || comparator(zodiac, bestZodiac) > 0) {
      bestKey = key;
      bestZodiac = zodiac;
    }
  }

  return bestKey;
}


export default {
  unity_run_threshold_s: 200,
  attack_eta_threshold_s: new BigNum(10),

  ECWaitTime: 3000,
  attackMode: true,

  async zodiacToGetOnNextUnit() {
    const POSITION = [
      "Left",
      "Top",
      "Bottom",
      "Right",
    ];

    const [zodiacs, rawPlanetZodiacs] = await Promise.all([
      States.nextUnityZodiacs(),
      States.planetZodiacInventory(),
    ]);

    const planetZodiacs = Object.fromEntries(Object.entries(rawPlanetZodiacs)
      .map(([planet, zodiac]) => [planet.toUpperCase(), zodiac]));

    // Determine what the planet setup is currently missing.
    let missingMultGain = 0;
    let missingGameSpeed = 0;

    for (const [planet, zodiac] of Object.entries(planetZodiacs)) {
      if (MULT_GAIN_PLANETS.has(planet)) {
        if (!zodiac || !isAriesMultBuildCandidate(zodiac))
          missingMultGain++;
      }

      else if (!zodiac || !isGameSpeedBuildCandidate(zodiac))
        missingGameSpeed++;
    }


    let bestIndex = 0;
    let bestPriority = null;
    let bestScore = null;

    for (let i = 0; i < zodiacs.length; ++i) {
      const zodiac = zodiacs[i];

      let priority = BigNum.ZERO;

      // While the 4 Mult Gain slots aren't complete,
      // heavily prioritize suitable Aries.
      if (missingMultGain > 0 && isAriesMultBuildCandidate(zodiac))
        priority = priority.add(PRIORITY_MULT_GAIN);

      // While the 8 GS slots aren't complete,
      // prioritize suitable Game Speed zodiacs.
      if (missingGameSpeed > 0 && isGameSpeedBuildCandidate(zodiac))
        priority = priority.add(PRIORITY_GAME_SPEED);

      // Continue preferring Aries after the initial setup
      // for future upgrades / merges.
      if (zodiac.sign === ZodiacSign.Aries)
        priority = priority.add(PRIORITY_ARIES);

      // GS zodiacs remain useful for upgrading the 8 GS slots.
      if (hasStat(zodiac, ZodiacStatType.GameSpeed))
        priority = priority.add(PRIORITY_GAME_SPEED_STAT);

      // Fire is a reasonable fallback.
      if (zodiac.Element === ZodiacElement.Fire)
        priority = priority.add(PRIORITY_FIRE);

      priority = priority.add(new BigNum(zodiac.rarity * 1_000));
      priority = priority.add(new BigNum(zodiac.rarityPlus * 10));
      priority = priority.add(zodiac.level);

      if (
        bestPriority === null ||
        priority.compareTo(bestPriority) > 0 ||
        (
          priority.compareTo(bestPriority) === 0 &&
          (
            bestScore === null ||
            zodiac.score.compareTo(bestScore) > 0
          )
        )
      ) {
        bestPriority = priority;
        bestScore = zodiac.score;
        bestIndex = i;
      }
    }

    return POSITION[bestIndex];
  },

  shouldSellZodiac(/** @type {UnityZodiac} */ zodiac) {
    if (zodiac.locked)
      return false;

    // Never sell something that should be sacrificed.
    if (isMultGainSacrifice(zodiac))
      return false;

    // Preserve Aries for the Mult Gain portion
    // of the planet setup.
    if (isAriesMultBuildCandidate(zodiac))
      return false;

    // Preserve good GS candidates for the other
    // eight planets.
    if (isGameSpeedBuildCandidate(zodiac))
      return false;

    return (
      zodiac.level.compareTo(MIN_LEVEL) < 0 ||
      zodiac.rarity < MIN_RARITY);
  },

  shouldSacrificeZodiac(/** @type {UnityZodiac} */ zodiac) {
    if (zodiac.locked)
      return false;

    // Good Aries must survive until merge/equip.
    //
    // This is required because sacrifice happens
    // BEFORE the equip phase in your main script.
    if (isAriesMultBuildCandidate(zodiac))
      return false;

    return isMultGainSacrifice(zodiac);
  },

  // Suffix can be used to differentiate different groups.
  // Merge only zodiacs with the same suffix.
  mergeKeySuffix(/** @type {UnityZodiac} */ zodiac) {
    if (isAriesMultBuildCandidate(zodiac))
      return "mult-gain";

    if (isGameSpeedBuildCandidate(zodiac))
      return `game-speed-${zodiac.sign}`;

    return "";
  },

  // Given the planet, current equipped zodiac, and all currently
  // available zodiacs, return the key to equip.
  equipZodiacFor(
    /** @type {"SUN" | "MERCURY" | "VENUS" | "MOON" | "MARS" | "JUPITER" | "SATURN" | "URANUS" | "NEPTUNE" | "PLUTO" | "CHIRON" | "FORTUNE"} */ planet,
    /** @type {UnityZodiac | null} */ currentZodiac,
    /** @type {Record<string, UnityZodiac>} */ possibleZodiacs
  ) {
    // 4 × Aries / Mult Gain
    if (MULT_GAIN_PLANETS.has(planet)) {
      const best = findBest(
        possibleZodiacs,
        isAriesMultBuildCandidate,
        compareMultGainZodiac);

      // No Aries available yet:
      // leave the current zodiac in place.
      return best ?? planet;
    }

    // 8 × Game Speed
    const best = findBest(
      possibleZodiacs,
      isGameSpeedBuildCandidate,
      compareGameSpeedZodiac);

    // No suitable GS replacement:
    // don't remove what is currently equipped.
    return best ?? planet;
  },
};
