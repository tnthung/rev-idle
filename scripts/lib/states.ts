// cspell:ignore eters Mult Mults sacri
import { BigNum } from "./utils.ts";


export class States {
  static async currentIP() {
    return new BigNum(await rev.state<string | number>("IP"));
  }

  static async currentEP() {
    return new BigNum(await rev.state<string | number>("EP"));
  }

  static async nextIP() {
    return new BigNum(await rev.state<string | number>("nextIP"));
  }

  static async nextEP() {
    return new BigNum(await rev.state<string | number>("nextEP"));
  }

  static async supernovaLevel() {
    return await rev.state<number>("gameData.eternity.supernovaLv");
  }

  static async eternities() {
    return new BigNum(await rev.state<string | number>("gameData.eternity.eters"));
  }

  static async totalAP() {
    return new BigNum(await rev.state<string | number>("gameData.eternity.APbought"));
  }

  static async DilationMaxScore() {
    return new BigNum(await rev.state<string | number>("dilationMaxScore"));
  }

  static async inDilation() {
    return await rev.state<boolean>("gameData.eternity.inDilation");
  }

  static async totalDTP() {
    return await rev.state<number>("DTP");
  }

  static async unusedDTP() {
    return await rev.state<number>("dtpFree");
  }

  static async spentDTP() {
    return await rev.state<number>("dtpSpent");
  }

  static async unityLevel() {
    return await rev.state<number>("unityLevel");
  }

  static async unities() {
    return new BigNum(await rev.state<string | number>("gameData.unity.unities"));
  }

  static async unityZodiacInventory(): Promise<Record<string, UnityZodiac>> {
    return Object.map(await rev.state<Record<string, UnityZodiacData>>("gameData.unity.inventory"),
      (key, value) => value && [key, new UnityZodiac(value)]);
  }

  static async planetZodiacInventory(): Promise<Record<keyof typeof Planet, UnityZodiac>> {
    return Object.map(await rev.state<Record<keyof typeof Planet, UnityZodiacData>>("gameData.unity.planetsInventory"),
      (key, value) => value && [key, new UnityZodiac(value)]);
  }

  static async gold() {
    return new BigNum(await rev.state<string | number>("gameData.attacks.gold"));
  }

  static async nextGold() {
    return new BigNum(await rev.state<string | number>("gameData.attacks.goldOnUnity"));
  }

  static async attackRelics() {
    return (await rev.state<AttackRelicData[]>("gameData.attacks.relics"))
      .map(relic => new AttackRelic(relic));
  }

  static async attackRelic(n: number) {
    return new AttackRelic((await rev.state<AttackRelicData>(`gameData.attacks.relics.${n}`)));
  }

  static async zodiacInventorySlotCount() {
    return await rev.state<number>("gameController.inventory.SlotZodiac.CurrentValue");
  }

  static async currentAttackDamage() {
    return new BigNum(await rev.state<string | number>("gameData.attacks.totalAtkMult"));
  }

  static async eternalChallenge(n: number) {
    return new EternalChallenge(await rev.state<EternalChallengeData>(`gameData.eternity.challenges.${n}`));
  }

  static async nextUnityZodiacs(): Promise<[UnityZodiac, UnityZodiac, UnityZodiac, UnityZodiac]> {
    return (await rev.state<UnityZodiacData[]>("gameData.unity.NextZodiacs")).map(zodiac => new UnityZodiac(zodiac)) as any;
  }

  static async sacrificeState() {
    return Object.entries(await rev.state<Partial<Record<keyof typeof ZodiacStatType, { value: string | number }>>>("gameData.unity.sacriStats"))
      .map(([type, {value}]) => new ZodiacStat({ type: type as keyof typeof ZodiacStatType, value }));
  }

  static async attackLevel() {
    return new AttackLevel(await rev.state<AttackLevelData>("gameData.attacks.level"));
  }

  static async attackRevolutionMults() {
    return (await rev.state<{ IsActive: boolean; mult: string | number }[]>("gameData.attacks.revolutions"))
      .map(ring => ring.IsActive ? new BigNum(ring.mult) : null);
  }

  static async maxAttackLevelReached() {
    return Number(await rev.state<string | number>("gameData.attacks.maxLevelReached"));
  }
}


interface EternalChallengeData {
  completeDiff: number;
  inChallenge: boolean;
  Unlocked: boolean;
  num: number;
}


export class EternalChallenge {
  challengeLevel: number;
  completeDiff: number;
  inChallenge: boolean;
  Unlocked: boolean;

  constructor({ completeDiff, inChallenge, Unlocked, num }: EternalChallengeData) {
    this.challengeLevel = num;
    this.completeDiff = completeDiff;
    this.inChallenge = inChallenge;
    this.Unlocked = Unlocked;
  }
}


interface UnityZodiacData {
  Element: keyof typeof ZodiacElement;
  IsEmpty: boolean;
  RangeOffset: number;
  Season: keyof typeof ZodiacSeason;
  hasPlanet: boolean;
  level: string | number;
  locked: boolean;
  planet: UnityPlanetData | null;
  quality: string | number;
  rarity: keyof typeof ZodiacRarity;
  rarityPlus: string | number;
  score: string | number;
  sign: keyof typeof ZodiacSign;
  stats: ZodiacStatData[];
}


export class UnityZodiac {
  Element: ZodiacElement;
  IsEmpty: boolean;
  RangeOffset: number;
  Season: ZodiacSeason;
  hasPlanet: boolean;
  level: BigNum;
  locked: boolean;
  planet: UnityPlanetData | null;
  quality: BigNum;
  rarity: ZodiacRarity;
  rarityPlus: number;
  score: BigNum;
  sign: ZodiacSign;
  stats: ZodiacStat[];
  statMap: Partial<Record<ZodiacStatType, BigNum>>;

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
  }: UnityZodiacData) {
    this.Element = ZodiacElement[Element];
    this.IsEmpty = IsEmpty;
    this.RangeOffset = RangeOffset;
    this.Season = ZodiacSeason[Season];
    this.hasPlanet = hasPlanet;
    this.level = new BigNum(level);
    this.locked = locked;
    this.planet = planet;
    this.quality = new BigNum(quality);
    this.rarity = ZodiacRarity[rarity];
    this.rarityPlus = Number(rarityPlus);
    this.score = new BigNum(score);
    this.sign = ZodiacSign[sign];
    this.stats = stats.map(stat => new ZodiacStat(stat));
    this.statMap = Object.fromEntries(this.stats.map(stat => [stat.type, stat.value]));
  }

  get mergeKey() {
    return `${this.Element};${this.rarity};${this.rarityPlus}`;
  }

  hasStat(statType: ZodiacStatType) {
    return this.statMap[statType] !== undefined;
  }
}


interface UnityPlanetData {
  bonusType: keyof typeof PlanetStatType;
  bonusValue: string | number;
  type: keyof typeof Planet;
  unlocked: boolean;
}


export class UnityPlanet {
  bonusType: PlanetStatType;
  bonusValue: BigNum;
  type: Planet;
  unlocked: boolean;

  constructor({ bonusType, bonusValue, type, unlocked }: UnityPlanetData) {
    this.bonusType = PlanetStatType[bonusType];
    if (this.bonusType == null) console.log(`planet stat type ${bonusType} is missing from the enum`);
    this.bonusValue = new BigNum(bonusValue);
    this.type = Planet[type];
    this.unlocked = unlocked;
  }
}


interface ZodiacStatData {
  type: keyof typeof ZodiacStatType;
  value: string | number;
}


export class ZodiacStat {
  type: ZodiacStatType;
  value: BigNum;

  constructor({ type, value }: ZodiacStatData) {
    this.type = ZodiacStatType[type];
    if (type == null) console.error(`zodiac stat type ${type} is missing from the enum`);
    this.value = new BigNum(value);
  }
}


interface AttackLevelData {
  currentHP: string | number;
  goldGain: string | number;
  level: string | number;
  maxHP: string | number;
  unlocked: boolean;
}


export class AttackLevel {
  currentHP: BigNum;
  goldGain: BigNum;
  level: number;
  maxHP: BigNum;
  unlocked: boolean;

  constructor({ currentHP, goldGain, level, maxHP, unlocked }: AttackLevelData) {
    this.currentHP = new BigNum(currentHP);
    this.goldGain = new BigNum(goldGain);
    this.level = Number(level);
    this.maxHP = new BigNum(maxHP);
    this.unlocked = unlocked;
  }
}


interface AttackRelicData {
  ReqLevel: string | number;
  amount: string | number;
  baseCost: string | number;
  buyAmount: string | number;
  costInc: string | number;
  effect: string | number;
  effect_next: string | number;
  num: string | number;
  regainedLevelsEst: string | number;
  sacriEffect: string | number;
  sacriLevel: string | number;
  totalCost: string | number;
  unlocked: boolean;
}


export class AttackRelic {
  ReqLevel: BigNum;
  amount: BigNum;
  baseCost: BigNum;
  buyAmount: BigNum;
  costInc: BigNum;
  effect: BigNum;
  effect_next: BigNum;
  num: number;
  regainedLevelsEst: BigNum;
  sacriEffect: BigNum;
  sacriLevel: BigNum;
  totalCost: BigNum;
  unlocked: boolean;

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
  }: AttackRelicData) {
    this.ReqLevel = new BigNum(ReqLevel);
    this.amount = new BigNum(amount);
    this.baseCost = new BigNum(baseCost);
    this.buyAmount = new BigNum(buyAmount);
    this.costInc = new BigNum(costInc);
    this.effect = new BigNum(effect);
    this.effect_next = new BigNum(effect_next);
    this.num = Number(num);
    this.regainedLevelsEst = new BigNum(regainedLevelsEst);
    this.sacriEffect = new BigNum(sacriEffect);
    this.sacriLevel = new BigNum(sacriLevel);
    this.totalCost = new BigNum(totalCost);
    this.unlocked = unlocked;
  }
}


export enum ZodiacElement {
  Fire,
  Water,
  Earth,
  Wind,
}


export enum ZodiacSeason {
  Spring,
  Summer,
  Autumn,
  Winter,
}


export enum ZodiacSign {
  Aries,
  Taurus,
  Gemini,
  Cancer,
  Leo,
  Virgo,
  Libra,
  Scorpio,
  Sagittarius,
  Capricorn,
  Aquarius,
  Pisces,
}


export enum ZodiacRarity {
  Garbage,
  Common,
  Uncommon,
  Rare,
  Epic,
  Legendary,
  Mythic,
  Godly,
  Divine,
  Immortal,
}


export enum ZodiacStatType {
  MultsGain,
  CommonExponent,
  AscensionPower,
  PromPower,
  LapsSpeed,
  SlowdownPower,

  IPGain,
  GenExponent,
  MultPerBoughtGen,
  InfinityGain,
  StarBase,
  StardustExponent,

  LabMultPower,
  SupernovaReq,
  EPGain,
  EternityGain,
  DPGain,
  FreeLabLevels,

  GameSpeed,
  LuckAdd,
  Ach29Reward,
  DTPCost,
  CenterDTUEff,
  ZodiacQualityMult,
}


export enum PlanetStatType {
  Undefined = -1,
  ZodiacQualityMult,
  LuckMult,
  UnityRewards,
  EternityRewards,
  EternityGain,
  InfinityGain,
  DPMult,
  StarCost,
  GameSpeed,
  LapSpeed,
  SupernovaRewards ,
  DilationUpgradesPower,
}


export enum Planet {
  Sun,
  Mercury,
  Venus,
  Moon,
  Mars,
  Jupiter,
  Saturn,
  Uranus,
  Neptune,
  Pluto,
  Chiron,
  Fortune,
}

export enum PlanetUpper {
  SUN,
  MERCURY,
  VENUS,
  MOON,
  MARS,
  JUPITER,
  SATURN,
  URANUS,
  NEPTUNE,
  PLUTO,
  CHIRON,
  FORTUNE,
}
