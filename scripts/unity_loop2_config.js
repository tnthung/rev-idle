import {
  ZodiacRarity,
  UnityZodiac,
  States,
  ZodiacSign,
  ZodiacElement,
  ZodiacStatType,
} from "./lib/states";


export default {
  unity_run_threshold_s: 200,
  attack_eta_threshold_s: 10,

  ECWaitTime: 3000,
  attackMode: true,

  async zodiacToGetOnNextUnit() {
    const POSITION = ["Left", "Top", "Bottom", "Right"];

    let maxRarity = null;

    for (const [pos, zodiac] of Object.entries((await States.nextUnityZodiacs()))) {
      if (zodiac.sign === ZodiacSign.Pisces)
        return POSITION[pos];

      if (zodiac.Element === ZodiacElement.Water)
        return POSITION[pos];

      if (!maxRarity || maxRarity[1] < zodiac.rarity)
        maxRarity = [pos, zodiac.rarity];
    }

    return POSITION[maxRarity[0]];
  },

  shouldSellZodiac(/** @type {UnityZodiac} */ zodiac) {
    if (zodiac.locked)
      return false;

    if (zodiac.level < 100)
      return true;

    if (zodiac.rarity < ZodiacRarity._Godly)
      return true;
  },

  shouldSacrificeZodiac(/** @type {UnityZodiac} */ zodiac) {
    if (zodiac.locked)
      return false;

    return isMultGainSacrifice(zodiac);
  },

  // Suffix can be used to differentiate different group. Merge only zodiacs
  // with the same suffix (i.e. in the same group)
  mergeKeySuffix(/** @type {UnityZodiac} */ zodiac) {
    return "";
  },

  // Given the planet, the current zodiac equipped there, and the possible zodiacs to choose from.
  // Equip the returned key of that zodiac in the possibleZodiacs record. Return null to take it off.
  // The chosen zodiac for previous planet will not be included for subsequent selections
  // (i.e. it will be removed from possibleZodiacs).
  equipZodiacFor(
    /** @type {"SUN" | "MERCURY" | "VENUS" | "MOON" | "MARS" | "JUPITER" | "SATURN" | "URANUS" | "NEPTUNE" | "PLUTO" | "CHIRON" | "FORTUNE"} */ planet,
    /** @type {UnityZodiac | null} */ currentZodiac,
    /** @type {Record<string, UnityZodiac>} */ possibleZodiacs
  ) {
    return planet;
  },
};
