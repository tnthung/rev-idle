import { Enum } from "./utils.js";


export class States {
  static currentIP             = () => rev.state("IP");
  static currentEP             = () => rev.state("EP");
  static nextIP                = () => rev.state("nextIP");
  static nextEP                = () => rev.state("nextEP");
  static supernovaLevel        = () => rev.state("gameData.eternity.supernovaLv");
  static eternities            = () => rev.state("gameData.eternity.eters");
  static totalAP               = () => rev.state("gameData.eternity.APbought");
  static eternalChallenge      = async (n) => new EternalChallenge(await rev.state(`gameData.eternity.challenges.${n}`));
  static DilationMaxScore      = () => rev.state("dilationMaxScoreCurrent");
  static inDilation            = () => rev.state("gameData.eternity.inDilation");
  static totalDTP              = () => rev.state("DTP");
  static unusedDTP             = () => rev.state("dtpFree");
  static spentDTP              = () => rev.state("dtpSpent");
  static unityZodiacInventory  = () => rev.state("gameData.unity.inventory");
  static planetZodiacInventory = () => rev.state("gameData.unity.planetsInventory");
  static attackLevel           = () => rev.state("gameData.attacks.level.level");
  static gold                  = () => rev.state("gameData.attacks.gold");
  static nextGold              = () => rev.state("gameData.attacks.goldOnUnity");
}


class EternalChallenge {
  constructor({ completeDiff, inChallenge, Unlocked, num }) {
    this.challengeLevel = num;
    this.completeDiff = completeDiff;
    this.inChallenge = inChallenge;
    this.Unlocked = Unlocked;
  }
}


export const ZodiacElement = Enum(
  "Fire",
  "Water",
  "Earth",
  "Wind");

export const ZodiacSign = Enum(
  "Aries",
  "Taurus",
  "Gemini",
  "Cancer",
  "Leo",
  "Virgo",
  "Libra",
  "Scorpio",
  "Sagittarius",
  "Capricorn",
  "Aquarius",
  "Pisces");

export const ZodiacRarity = Enum(
  "Garbage",
  "Common",
  "Uncommon",
  "Rare",
  "Epic",
  "Legendary",
  "Mythic",
  "Godly",
  "Divine",
  "Immortal");
