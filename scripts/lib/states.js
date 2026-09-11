import { Enum, BigNum } from "./utils.js";


export class States {
  /** @type {function(): Promise<BigNum>} */
  static currentIP = () => rev.state("IP")
    .then(data => new BigNum(data));

  /** @type {function(): Promise<BigNum>} */
  static currentEP = () => rev.state("EP")
    .then(data => new BigNum(data));

  /** @type {function(): Promise<BigNum>} */
  static nextIP = () => rev.state("nextIP")
    .then(data => new BigNum(data));

  /** @type {function(): Promise<BigNum>} */
  static nextEP = () => rev.state("nextEP")
    .then(data => new BigNum(data));

  /** @type {function(): Promise<number>} */
  static supernovaLevel = () => rev.state("gameData.eternity.supernovaLv");

  /** @type {function(): Promise<BigNum>} */
  static eternities = () => rev.state("gameData.eternity.eters")
    .then(data => new BigNum(data));

  /** @type {function(): Promise<BigNum>} */
  static totalAP = () => rev.state("gameData.eternity.APbought")
    .then(data => new BigNum(data));

  /** @type {function(): Promise<BigNum>} */
  static DilationMaxScore = () => rev.state("dilationMaxScoreCurrent")
    .then(data => new BigNum(data));

  /** @type {function(): Promise<boolean>} */
  static inDilation = () => rev.state("gameData.eternity.inDilation");

  /** @type {function(): Promise<number>} */
  static totalDTP = () => rev.state("DTP");

  /** @type {function(): Promise<number>} */
  static unusedDTP = () => rev.state("dtpFree");

  /** @type {function(): Promise<number>} */
  static spentDTP = () => rev.state("dtpSpent");

  /** @type {function(): Promise<Record<string, UnityZodiac>>} */
  static unityZodiacInventory = () => rev.state("gameData.unity.inventory")
    .then(data => Object.fromEntries(Object.entries(data)
      .map(([key, value]) => [key, new UnityZodiac(value)])));

  /** @type {function(): Promise<Record<string, UnityZodiac>>} */
  static planetZodiacInventory = () => rev.state("gameData.unity.planetsInventory")
    .then(data => Object.fromEntries(Object.entries(data)
      .map(([key, value]) => [key, new UnityZodiac(value)])));

  /** @type {function(): Promise<BigNum>} */
  static gold = () => rev.state("gameData.attacks.gold")
    .then(data => new BigNum(data));

  /** @type {function(): Promise<BigNum>} */
  static nextGold = () => rev.state("gameData.attacks.goldOnUnity")
    .then(data => new BigNum(data));

  /** @type {function(): Promise<AttackRelic[]>} */
  static attackRelics = () => rev.state("gameData.attacks.relics")
    .then(data => data.map(relic => new AttackRelic(relic)));

  /** @type {function(): Promise<number>} */
  static zodiacInventorySlotCount = () => rev.state("gameController.inventory.SlotZodiac.CurrentValue");

  /** @type {function(): Promise<BigNum>} */
  static currentAttackDamage = () => rev.state("gameData.attacks.totalAtkMult")
    .then(data => new BigNum(data));

  /** @type {function(number): Promise<EternalChallenge>} */
  static eternalChallenge = (n) => rev.state(`gameData.eternity.challenges.${n}`)
    .then(data => new EternalChallenge(data));

  /** @type {function(): Promise<UnityZodiac[]>} */
  static nextUnityZodiacs = () => rev.state("gameData.unity.NextZodiacs")
    .then(data => data.map(zodiac => new UnityZodiac(zodiac)));

  static sacrificeState = () => rev.state("gameData.unity.sacriStats")
    .then(data => Object.entries(data).map(([type, value]) => new ZodiacStat({ type, value })));

  /** @type {function(): Promise<AttackLevel>} */
  static attackLevel = () => rev.state("gameData.attacks.level")
    .then(data => new AttackLevel(data));

  /** @type {function(): Promise<number>} */
  static maxAttackLevelReached = () => rev.state("gameData.attacks.maxLevelReached")
    .then(Number);
}


export class EternalChallenge {
  constructor({ completeDiff, inChallenge, Unlocked, num }) {
    /** @type {number} */
    this.challengeLevel = num;
    /** @type {number} */
    this.completeDiff = completeDiff;
    /** @type {boolean} */
    this.inChallenge = inChallenge;
    /** @type {boolean} */
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
    /** @type {typeof ZodiacElement[keyof typeof ZodiacElement] & number} */
    this.Element = ZodiacElement[Element];
    /** @type {boolean} */
    this.IsEmpty = IsEmpty;
    /** @type {number} */
    this.RangeOffset = RangeOffset;
    /** @type {typeof ZodiacSeason[keyof typeof ZodiacSeason] & number} */
    this.Season = ZodiacSeason[Season];
    /** @type {boolean} */
    this.hasPlanet = hasPlanet;
    /** @type {BigNum} */
    this.level = new BigNum(level);
    /** @type {boolean} */
    this.locked = locked;
    /** @type {UnityPlanet} */
    this.planet = planet;
    /** @type {BigNum} */
    this.quality = new BigNum(quality);
    /** @type {typeof ZodiacRarity[keyof typeof ZodiacRarity] & number} */
    this.rarity = ZodiacRarity[rarity];
    /** @type {number} */
    this.rarityPlus = Number(rarityPlus);
    /** @type {BigNum} */
    this.score = new BigNum(score);
    /** @type {typeof ZodiacSign[keyof typeof ZodiacSign] & number} */
    this.sign = ZodiacSign[sign];
    /** @type {ZodiacStat[]} */
    this.stats = stats.map(stat => new ZodiacStat(stat));
  }

  get mergeKey() {
    return `${this.Element};${this.rarity};${this.rarityPlus}`;
  }
}


export class UnityPlanet {
  constructor({ bonusType, bonusValue, type, unlocked }) {
    /** @type {typeof PlanetStatType[keyof typeof PlanetStatType] & number} */
    this.bonusType = PlanetStatType[bonusType];
    /** @type {BigNum} */
    this.bonusValue = new BigNum(bonusValue);
    /** @type {typeof Planet[keyof typeof Planet] & number} */
    this.type = Planet[type];
    /** @type {boolean} */
    this.unlocked = unlocked;
  }
}


export class ZodiacStat {
  constructor({ type, value }) {
    /** @type {typeof ZodiacStatType[keyof typeof ZodiacStatType] & number} */
    this.type = ZodiacStatType[type];
    /** @type {BigNum} */
    this.value = new BigNum(value);
  }
}


export class AttackLevel {
  constructor({ currentHP, goldGain, level, maxHP, unlocked }) {
    /** @type {BigNum} */
    this.currentHP = new BigNum(currentHP);
    /** @type {BigNum} */
    this.goldGain = new BigNum(goldGain);
    /** @type {number} */
    this.level = Number(level);
    /** @type {BigNum} */
    this.maxHP = new BigNum(maxHP);
    /** @type {boolean} */
    this.unlocked = unlocked;
  }
}


export class AttackRelic {
  constructor({
    ReqLevel,
    amount,
    baseCost,
    buyAmount,
    costInc,
    effect,
    effect_next,
    num,
    regainedLevelsEst,
    sacriEffect,
    sacriLevel,
    totalCost,
    unlocked,
  }) {
    /** @type {BigNum} */
    this.ReqLevel = new BigNum(ReqLevel);
    /** @type {BigNum} */
    this.amount = new BigNum(amount);
    /** @type {BigNum} */
    this.baseCost = new BigNum(baseCost);
    /** @type {BigNum} */
    this.buyAmount = new BigNum(buyAmount);
    /** @type {BigNum} */
    this.costInc = new BigNum(costInc);
    /** @type {BigNum} */
    this.effect = new BigNum(effect);
    /** @type {BigNum} */
    this.effect_next = new BigNum(effect_next);
    /** @type {number} */
    this.num = Number(num);
    /** @type {BigNum} */
    this.regainedLevelsEst = new BigNum(regainedLevelsEst);
    /** @type {BigNum} */
    this.sacriEffect = new BigNum(sacriEffect);
    /** @type {BigNum} */
    this.sacriLevel = new BigNum(sacriLevel);
    /** @type {BigNum} */
    this.totalCost = new BigNum(totalCost);
    /** @type {boolean} */
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

export const PlanetStatType = Enum(
  "SupernovaRewards");

export const Planet = Enum(
  "Sun",
  "Mercury",
  "Venus",
  "Moon",
  "Mars",
  "Jupiter",
  "Saturn",
  "Uranus",
  "Neptune",
  "Pluto",
  "Chiron",
  "Fortune");

export const PlanetUpper = Enum(
  "SUN",
  "MERCURY",
  "VENUS",
  "MOON",
  "MARS",
  "JUPITER",
  "SATURN",
  "URANUS",
  "NEPTUNE",
  "PLUTO",
  "CHIRON",
  "FORTUNE");
