import { Enum } from "./utils.js";


export class States {
  static currentIP             = () => rev.state("IP");
  static currentEP             = () => rev.state("EP");
  static nextIP                = () => rev.state("nextIP");
  static nextEP                = () => rev.state("nextEP");
  static supernovaLevel        = () => rev.state("gameData.eternity.supernovaLv");
  static eternities            = () => rev.state("gameData.eternity.eters");
  static totalAP               = () => rev.state("gameData.eternity.APbought");
  static DilationMaxScore      = () => rev.state("dilationMaxScoreCurrent");
  static inDilation            = () => rev.state("gameData.eternity.inDilation");
  static totalDTP              = () => rev.state("DTP");
  static unusedDTP             = () => rev.state("dtpFree");
  static spentDTP              = () => rev.state("dtpSpent");
  static unityZodiacInventory  = () => rev.state("gameData.unity.inventory");
  static planetZodiacInventory = () => rev.state("gameData.unity.planetsInventory");
  static gold                  = () => rev.state("gameData.attacks.gold");
  static nextGold              = () => rev.state("gameData.attacks.goldOnUnity");

  /** @type {function(): Promise<string>} */
  static currentAttackDamage = () => rev.state("gameData.attacks.totalAtkMult");

  /** @type {function(number): Promise<EternalChallenge>} */
  static eternalChallenge = (n) => rev.state(`gameData.eternity.challenges.${n}`)
    .then(data => new EternalChallenge(data));

  /** @type {function(): Promise<UnityZodiac[]>} */
  static nextUnityZodiacs = () => rev.state("gameData.unity.NextZodiacs")
    .then(data => data.map(zodiac => new UnityZodiac(zodiac)));

  /** @type {function(): Promise<AttackLevel>} */
  static attackLevel = () => rev.state("gameData.attacks.level")
    .then(data => new AttackLevel(data));

  /** @type {function(): Promise<number>} */
  static maxAttackLevelReached = () => rev.state("gameData.attacks.maxLevelReached")
    .then(Number);
}


export class EternalChallenge {
  constructor({ completeDiff, inChallenge, Unlocked, num }) {
    this.challengeLevel = num;
    this.completeDiff = completeDiff;
    this.inChallenge = inChallenge;
    this.Unlocked = Unlocked;
  }
}


export class UnityZodiac {
  constructor({
    Element,
    IsEmpty,
    RangeOffset,
    Season,
    hasPlanet,
    level,
    locked,
    planet,
    quality,
    rarity,
    rarityPlus,
    score,
    sign,
    stats,
  }) {
    this.Element = ZodiacElement[Element];
    this.IsEmpty = IsEmpty;
    this.RangeOffset = RangeOffset;
    this.Season = ZodiacSeason[Season];
    this.hasPlanet = hasPlanet;
    this.level = Number(level);
    this.locked = locked;
    this.planet = planet;
    this.quality = quality;
    this.rarity = ZodiacRarity[rarity];
    this.rarityPlus = Number(rarityPlus);
    this.score = score;
    this.sign = ZodiacSign[sign];
    this.stats = stats.map(stat => new ZodiacStat(stat));
  }

  get mergeKey() {
    return `${this.Element};${this.rarity};${this.rarityPlus}`;
  }
}


export class ZodiacStat {
  constructor({ type, value }) {
    this.type = type;
    this.value = value;
  }
}


export class AttackLevel {
  constructor({ currentHP, goldGain, level, maxHP, unlocked }) {
    this.currentHP = currentHP;
    this.goldGain = goldGain;
    this.level = Number(level);
    this.maxHP = maxHP;
    this.unlocked = unlocked;
  }
}


export const ZodiacElement = Enum(
  "Fire",
  "Water",
  "Earth",
  "Wind");

export const ZodiacSeason = Enum(
  "Spring",
  "Summer",
  "Autumn",
  "Winter");

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

export const ZodiacStatType = Enum(
  "MultsGain",
  "CommonExponent",
  "AscensionPower",
  "PromPower",
  "LapsSpeed",
  "SlowdownPower",

  "IPGain",
  "GenExponent",
  "MultPerBoughtGen",
  "InfinityGain",
  "StarBase",
  "StardustExponent",

  "LabMultPower",
  "SupernovaReq",
  "EPGain",
  "EternityGain",
  "DPGain",
  "FreeLabLevels",

  "GameSpeed",
  "LuckAdd",
  "Ach29Reward",
  "DTPCost",
  "CenterDTUEffect",
  "ZodiacQualityMult");
