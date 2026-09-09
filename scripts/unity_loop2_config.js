import { ZodiacRarity, ZodiacSign } from "./lib/states";


export default {
  // targetSigns: [ZodiacSign._Leo, ZodiacSign._Sagittarius, ZodiacSign._Aries],
  // targetSigns: [ZodiacSign._Libra, ZodiacSign._Aquarius],
  targetSigns: [ZodiacSign._Pisces, ZodiacSign._Scorpio, ZodiacSign._Cancer],
  targetMinRarity: ZodiacRarity._Mythic,
  genericMinRarity: ZodiacRarity._Mythic,
  preserveDivinePlusPerTarget: 2,
  minZodiacLevel: 60,

  ECWaitTime: 3000,

  attackMode: true,
  attackTargetLevel: 105,
};
