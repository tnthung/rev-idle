import { ZodiacRarity, UnityZodiac } from "./lib/states";


export default {
  ECWaitTime: 3000,
  attackMode: true,

  shouldSellZodiac(/** @type {UnityZodiac} */ zodiac) {
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
