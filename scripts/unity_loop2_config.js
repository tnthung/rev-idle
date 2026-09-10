import { ZodiacRarity, UnityZodiac, States, ZodiacSign, ZodiacElement } from "./lib/states";


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

  // Suffix can be used to differentiate different group. Merge only zodiacs
  // with the same suffix (i.e. in the same group)
  mergeKeySuffix(/** @type {UnityZodiac} */ zodiac) {
    return "";
  }
};
