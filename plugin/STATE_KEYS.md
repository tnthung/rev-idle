# Complete state path reference

Generated deterministically from `BepInEx/interop/Assembly-CSharp.dll` by `generate-state-reference.ps1`.
Reachable gameplay types: **132**. Properties: **1617**.

## Path grammar and JSON behavior

Paths are case-sensitive public property names separated by `.`, starting at `GameData`. Numeric segments index arrays/lists; dictionary segments resolve string, integer, or enum keys. Collections are documented below each property. A no-argument request returns the complete nested graph; selected requests return a flat object keyed by the requested path.

JSON follows the serializer policy: BigDouble and large integers are strings; safe integers, finite floating-point values, booleans, strings, enums, dates, arrays/lists, dictionaries, and gameplay objects use their native JSON forms. Non-finite floating-point values are `"NaN"`, `"Infinity"`, or `"-Infinity"`; repeated collection objects serialize as `null` to preserve indexes.

## Compatibility aliases

| Alias | Canonical target |
| --- | --- |
| `score` | `score` |
| `income` | `income` |
| `IP` | `infinity.IP` |
| `infinities` | `infinity.infs` |
| `stars` | `infinity.stars` |
| `stardust` | `infinity.stardust` |
| `EP` | `eternity.EP` |
| `eternities` | `eternity.eters` |
| `DP` | `eternity.DP` |
| `AP` | `eternity.AP` |
| `RP` | `eternity.curRP` |
| `RPMax` | `eternity.maximumRP` |
| `RPSpent` | `eternity.spendRP` |
| `unities` | `unity.unities` |
| `passiveUnities` | `unity.passiveUnities` |
| `astrodust` | `unity.astrodust` |
| `singularities` | `singularity.singularity` |
| `atoms` | `singularity.atoms` |
| `PlP` | `plague.PlP` |
| `PlPperPlG` | `plague.PlPperPlG` |
| `PlG` | `plague.PlG` |
| `VE` | `plague.VE` |
| `ViP` | `plague.ViP` |
| `tarotSwords` | `tarot.swords` |
| `tarotWands` | `tarot.wands` |
| `tarotPentacles` | `tarot.pentacles` |
| `tarotCups` | `tarot.cups` |
| `goldTarotSwords` | `tarot.goldSwords` |
| `goldTarotWands` | `tarot.goldWands` |
| `goldTarotPentacles` | `tarot.goldPentacles` |
| `goldTarotCups` | `tarot.goldCups` |
| `tarotDraws` | `tarot.draws` |
| `timeSinceStart` | `timeSinceStart` |
| `timeInfinity` | `timeInf` |
| `timeEternity` | `timeEtr` |
| `timeUnity` | `timeUnity` |
| `timeTotal` | `timeTotal` |
| `DT` | `eternity.dilationTree` |
| `DTP` | `eternity.dtpMax` |

## Reachable gameplay types

### `AnalyticsMetaData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `buyRevo` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `exitCount` | `System.Int32` |   |
| `prestige` | `System.Boolean` |   |
| `showHelp` | `System.Boolean` |   |

### `AnimationOption`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `AstroElementType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `AstroPlanetType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `AstroSeasonType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `AstroSignType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `AttacksData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `EternityData` | `EternityData` |   |
| `HPtoGoldDivider` | `BigDouble` |   |
| `HPtoGoldPower` | `BigDouble` |   |
| `InfinityData` | `InfinityData` |   |
| `PrestigeUnlocked` | `System.Boolean` |   |
| `RelicsCount` | `System.Int32` |   |
| `RelicsUnlocked` | `System.Boolean` |   |
| `UnityData` | `UnityData` |   |
| `ascendPower` | `BigDouble` |   |
| `atkMultsMult` | `BigDouble` |   |
| `atkOtherMult` | `BigDouble` |   |
| `attacksMultsPower` | `BigDouble` |   |
| `baseLevelHP` | `BigDouble` |   |
| `buyAmmoBuyable` | `System.Int32` |   |
| `buyAmmoRelics` | `System.Int32` |   |
| `currentFrame` | `System.Int64` |   |
| `gold` | `BigDouble` |   |
| `goldOnUnity` | `BigDouble` |   |
| `level` | `AttacksLevel` |   |
| `levelsRegainedSMFactor` | `BigDouble` |   |
| `levelsRegainedSacri` | `BigDouble` |   |
| `maxLevelReached` | `BigDouble` |   |
| `maxRelicUnlocked` | `System.Int32` |   |
| `multGainMult` | `BigDouble` |   |
| `prestigeExponNext` | `BigDouble` |   |
| `prestigeExponNow` | `BigDouble` |   |
| `prestigeMultsGainNext` | `BigDouble` |   |
| `prestigeMultsGainNow` | `BigDouble` |   |
| `relicLevelsSum` | `BigDouble` |   |
| `relics` | `List`1<Relic>` | list of Relic append `.<numeric-index>` |
| `revolutions` | `List`1<AttacksRevolution>` | list of AttacksRevolution append `.<numeric-index>` |
| `scoreToAtkBase` | `BigDouble` |   |
| `scoreToAtkDivider` | `BigDouble` |   |
| `scoreToAtkLogBase` | `BigDouble` |   |
| `scoreToAtkPower` | `BigDouble` |   |
| `scoreToAtkTotalDivider` | `BigDouble` |   |
| `totalAtkMult` | `BigDouble` |   |
| `totalPrestigeMultCurrent` | `BigDouble` |   |
| `totalPrestigeMultNext` | `BigDouble` |   |

### `AttacksLevel`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `currentHP` | `BigDouble` |   |
| `goldGain` | `BigDouble` |   |
| `level` | `BigDouble` |   |
| `maxHP` | `BigDouble` |   |
| `unlocked` | `System.Boolean` |   |

### `AttacksRevolution`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Attacks` | `AttacksData` |   |
| `CanAscend` | `System.Boolean` |   |
| `CanPurchase` | `System.Boolean` |   |
| `IsActive` | `System.Boolean` |   |
| `IsUnlocked` | `System.Boolean` |   |
| `Plague` | `PlagueData` |   |
| `Singularity` | `SingularityData` |   |
| `Tarot` | `TarotData` |   |
| `amount` | `System.Int64` |   |
| `ascCooldown` | `System.Boolean` |   |
| `ascendPower` | `BigDouble` |   |
| `ascension` | `System.Int64` |   |
| `baseSpeed` | `System.Double` |   |
| `damage` | `BigDouble` |   |
| `dmgBaseMult` | `BigDouble` |   |
| `dmgBaseMults` | `Il2CppStructArray`1<BigDouble>` |   |
| `dmgInitMult` | `BigDouble` |   |
| `got` | `System.Double` |   |
| `mult` | `BigDouble` |   |
| `multGain` | `BigDouble` |   |
| `num` | `System.Int32` |   |
| `progress` | `BigDouble` |   |
| `speed` | `System.Double` |   |
| `speedNext` | `System.Double` |   |
| `thisBuyable` | `AttacksRevolutionBuyable` |   |

### `AttacksRevolutionBuyable`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `allCost` | `BigDouble` |   |
| `amount` | `System.Int32` |   |
| `baseCost` | `BigDouble` |   |
| `baseCosts` | `Il2CppStructArray`1<BigDouble>` |   |
| `buyAmount` | `System.Double` |   |
| `costInc` | `BigDouble` |   |
| `maxAmount` | `System.Int32` |   |
| `num` | `System.Int32` |   |
| `spendable` | `System.Boolean` |   |
| `totalCost` | `BigDouble` |   |

### `AutoDisolveERObject`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `enabled` | `System.Boolean` |   |
| `maxLevel` | `Nullable`1<BigDouble>` |   |
| `minLevel` | `BigDouble` |   |
| `rarities` | `List`1<PlagueEndoRarity>` | list of PlagueEndoRarity append `.<numeric-index>` |
| `types` | `List`1<PlagueEndoType>` | list of PlagueEndoType append `.<numeric-index>` |

### `AutomationData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `ActivePromotion` | `PromoteObject` |   |
| `HasAutoAP` | `System.Boolean` |   |
| `HasAutoAnimals` | `System.Boolean` |   |
| `HasAutoAscend` | `System.Boolean` |   |
| `HasAutoAscendAttacks` | `System.Boolean` |   |
| `HasAutoBuy` | `System.Boolean` |   |
| `HasAutoBuyArtifacts` | `System.Boolean` |   |
| `HasAutoBuyAttacks` | `System.Boolean` |   |
| `HasAutoBuyDTP` | `System.Boolean` |   |
| `HasAutoBuyPolishEnhance` | `System.Boolean` |   |
| `HasAutoBuyRelics` | `System.Boolean` |   |
| `HasAutoBuyRunes` | `System.Boolean` |   |
| `HasAutoBuySMP` | `System.Boolean` |   |
| `HasAutoCraftER` | `System.Boolean` |   |
| `HasAutoDeleteMin` | `System.Boolean` |   |
| `HasAutoDilationUpgrade` | `System.Boolean` |   |
| `HasAutoDisolveER` | `System.Boolean` |   |
| `HasAutoEternity` | `System.Boolean` |   |
| `HasAutoFlushArtifacts` | `System.Boolean` |   |
| `HasAutoGen` | `System.Boolean` |   |
| `HasAutoInfTree` | `System.Boolean` |   |
| `HasAutoInfinity` | `System.Boolean` |   |
| `HasAutoInfinityIP` | `System.Boolean` |   |
| `HasAutoLab` | `System.Boolean` |   |
| `HasAutoMergeSpecialMin` | `System.Boolean` |   |
| `HasAutoMinMagnetUpgrade` | `System.Boolean` |   |
| `HasAutoMinMerge` | `System.Boolean` |   |
| `HasAutoMinPolishUpgrade` | `System.Boolean` |   |
| `HasAutoMinUpgrade` | `System.Boolean` |   |
| `HasAutoPrestige` | `System.Boolean` |   |
| `HasAutoPromote` | `System.Boolean` |   |
| `HasAutoRP` | `System.Boolean` |   |
| `HasAutoSellSacrificeZodiac` | `System.Boolean` |   |
| `HasAutoSetMineralLvl` | `System.Boolean` |   |
| `HasAutoSingularity` | `System.Boolean` |   |
| `HasAutoSlowdown` | `System.Boolean` |   |
| `HasAutoSpawnSpecialMin` | `System.Boolean` |   |
| `HasAutoStar` | `System.Boolean` |   |
| `HasAutoSupernova` | `System.Boolean` |   |
| `HasAutoTarotDraw` | `System.Boolean` |   |
| `HasAutoTarotUpgrades` | `System.Boolean` |   |
| `HasAutoUnity` | `System.Boolean` |   |
| `activePromotion` | `System.Int32` |   |
| `animals` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `animalsOrder` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `anlWait` | `System.Boolean` |   |
| `artifacts` | `Dictionary`2<TarotSuitType, ValueTuple`2<System.Boolean, System.Single>>` | dictionary keyed by TarotSuitType, values ValueTuple`2<System.Boolean, System.Single> append `.<string|integer|enum-key>` |
| `ascends` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `ascendsAttacks` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `autoAll` | `System.Boolean` |   |
| `autoAnimals` | `System.Boolean` |   |
| `autoBuyArtifacts` | `System.Boolean` |   |
| `autoBuyDTP` | `System.Boolean` |   |
| `autoBuyRelics` | `System.Boolean` |   |
| `autoBuyRunes` | `System.Boolean` |   |
| `autoBuyTarotUpgrades` | `System.Boolean` |   |
| `autoCraftER` | `System.Boolean` |   |
| `autoDelMin` | `System.Boolean` |   |
| `autoDisolveER` | `System.Boolean` |   |
| `autoEternity` | `System.Boolean` |   |
| `autoFlushArtifacts` | `System.Boolean` |   |
| `autoInfTree` | `System.Boolean` |   |
| `autoInfinity` | `System.Boolean` |   |
| `autoInfinityIP` | `System.Boolean` |   |
| `autoMinMerge` | `System.Boolean` |   |
| `autoPrestige` | `System.Boolean` |   |
| `autoPromote` | `System.Boolean` |   |
| `autoRP` | `System.Boolean` |   |
| `autoSacrificeZodiac` | `System.Boolean` |   |
| `autoSellZodiac` | `System.Boolean` |   |
| `autoSetMineralLvl` | `System.Boolean` |   |
| `autoSingularity` | `System.Boolean` |   |
| `autoSlowdown` | `System.Boolean` |   |
| `autoSpawnSpeMin` | `System.Boolean` |   |
| `autoStar` | `System.Boolean` |   |
| `autoSupernova` | `System.Boolean` |   |
| `autoTarotDraw` | `System.Boolean` |   |
| `autoUnity` | `System.Boolean` |   |
| `buyAP` | `List`1<System.Boolean>` | list of System.Boolean append `.<numeric-index>` |
| `buyArtifacts` | `Dictionary`2<TarotSuitType, List`1<System.Int32>>` | dictionary keyed by TarotSuitType, values List`1<System.Int32> append `.<string|integer|enum-key>` |
| `buyDTPLimit` | `Nullable`1<BigDouble>` |   |
| `buySMP` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `buyables` | `List`1<System.Double>` | list of System.Double append `.<numeric-index>` |
| `buyablesAttacks` | `List`1<System.Double>` | list of System.Double append `.<numeric-index>` |
| `craftERFlushed` | `System.Single` |   |
| `craftERMaxLvl` | `Nullable`1<BigDouble>` |   |
| `craftERMinPlP` | `Nullable`1<BigDouble>` |   |
| `delMinFromCurMaxLvl` | `Nullable`1<BigDouble>` |   |
| `delMinMaxLvl` | `Nullable`1<BigDouble>` |   |
| `dilUp` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `disolveER` | `List`1<AutoDisolveERObject>` | list of AutoDisolveERObject append `.<numeric-index>` |
| `etrEpGain` | `BigDouble` |   |
| `etrTimeWait` | `System.Double` |   |
| `generators` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `infIpGain` | `BigDouble` |   |
| `infTimeWait` | `System.Double` |   |
| `ipMultGain` | `BigDouble` |   |
| `labUp` | `List`1<System.Boolean>` | list of System.Boolean append `.<numeric-index>` |
| `lockNewZodiac` | `System.Boolean` |   |
| `minPolish` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `minUpgrades` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `moonRunes` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `polishEnhance` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `prestigeMinExpGain` | `System.Double` |   |
| `prestigeMinMultGain` | `BigDouble` |   |
| `prestigeMinTime` | `System.Double` |   |
| `promotions` | `List`1<PromoteObject>` | list of PromoteObject append `.<numeric-index>` |
| `relics` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `relicsOrder` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `rpDistr` | `List`1<System.Single>` | list of System.Single append `.<numeric-index>` |
| `sellZodiacImmortalMax` | `Nullable`1<BigDouble>` |   |
| `sellZodiacImmortalMin` | `Nullable`1<BigDouble>` |   |
| `sellZodiacLevelMax` | `Nullable`1<BigDouble>` |   |
| `sellZodiacLevelMin` | `Nullable`1<BigDouble>` |   |
| `sellZodiacQualityMax` | `Nullable`1<BigDouble>` |   |
| `sellZodiacQualityMin` | `Nullable`1<BigDouble>` |   |
| `sellZodiacRarity` | `List`1<ZodiacRarityType>` | list of ZodiacRarityType append `.<numeric-index>` |
| `sellZodiacSign` | `List`1<AstroSignType>` | list of AstroSignType append `.<numeric-index>` |
| `singMinAtoms` | `Nullable`1<BigDouble>` |   |
| `singMinMultGain` | `Nullable`1<BigDouble>` |   |
| `singMinTime` | `Nullable`1<BigDouble>` |   |
| `singWaitUntilReady` | `System.Boolean` |   |
| `slowdownMult` | `System.Double` |   |
| `spawnSmCostMax` | `Nullable`1<BigDouble>` |   |
| `spawnSmCostMaxBaseCost` | `Nullable`1<BigDouble>` |   |
| `spawnSmCostMaxPlusBase` | `Nullable`1<BigDouble>` |   |
| `spawnSmCostSpawnedFactor` | `Nullable`1<BigDouble>` |   |
| `speMinMerge` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `stars` | `List`1<System.Boolean>` | list of System.Boolean append `.<numeric-index>` |
| `sunRunes` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `tarotDrawMinDraw` | `Nullable`1<BigDouble>` |   |
| `tarotDrawMinTime` | `Nullable`1<BigDouble>` |   |
| `tarotDrawPercent` | `System.Single` |   |
| `tarotUpgrades` | `Dictionary`2<TarotSuitType, List`1<System.Int32>>` | dictionary keyed by TarotSuitType, values List`1<System.Int32> append `.<string|integer|enum-key>` |
| `uniAttackLevel` | `Nullable`1<BigDouble>` |   |
| `uniEP` | `Nullable`1<BigDouble>` |   |
| `uniElements` | `List`1<AstroElementType>` | list of AstroElementType append `.<numeric-index>` |
| `uniGoldAfter` | `Nullable`1<BigDouble>` |   |
| `uniPriority` | `List`1<Nullable`1<System.Int32>>` | list of Nullable`1<System.Int32> append `.<numeric-index>` |
| `uniSeasons` | `List`1<AstroSeasonType>` | list of AstroSeasonType append `.<numeric-index>` |
| `uniStats` | `List`1<ZodiacStats>` | list of ZodiacStats append `.<numeric-index>` |
| `uniZodiacLvl` | `Nullable`1<BigDouble>` |   |
| `uniZodiacSign` | `List`1<AstroSignType>` | list of AstroSignType append `.<numeric-index>` |

### `BigDouble`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `DoubleExpMax` | `System.Int64` |   |
| `DoubleExpMin` | `System.Int64` |   |
| `ExpLimit` | `System.Double` |   |
| `Exponent` | `System.Double` |   |
| `Mantissa` | `System.Double` |   |
| `MaxSignificantDigits` | `System.Int32` |   |
| `MaxValue` | `BigDouble` |   |
| `MinValue` | `BigDouble` |   |
| `NaN` | `BigDouble` |   |
| `NegativeInfinity` | `BigDouble` |   |
| `One` | `BigDouble` |   |
| `PositiveInfinity` | `BigDouble` |   |
| `Tolerance` | `System.Double` |   |
| `Zero` | `BigDouble` |   |

### `Block`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `BgBody` | `UnityEngine.UI.Image` |   |
| `BgHeader` | `UnityEngine.UI.Image` |   |
| `BodyColor` | `UnityEngine.Color` |   |
| `Container` | `UnityEngine.RectTransform` |   |
| `ContainerElif` | `UnityEngine.RectTransform` |   |
| `Depth` | `System.Int32` |   |
| `HeaderColor` | `UnityEngine.Color` |   |
| `Index` | `System.Int32` |   |
| `Initialized` | `System.Boolean` |   |
| `Instance` | `UnityEngine.RectTransform` |   |
| `IsFirst` | `System.Boolean` |   |
| `IsLast` | `System.Boolean` |   |
| `LabelId` | `TMPro.TMP_Text` |   |
| `Progress` | `System.Single` |   |
| `Scope` | `List`1<Block>` | list of Block append `.<numeric-index>` |
| `State` | `BlockStateType` |   |
| `bodyColor` | `UnityEngine.Color` |   |
| `fields` | `List`1<BlockField>` | list of BlockField append `.<numeric-index>` |
| `headerColor` | `UnityEngine.Color` |   |
| `progress` | `System.Single` |   |
| `state` | `BlockStateType` |   |
| `type` | `BlockType` |   |

### `BlockField`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `BigRange` | `Nullable`1<ValueTuple`2<BigDouble, BigDouble>>` |   |
| `EnumProvider` | `Func`1<List`1<Il2CppSystem.Enum>>` |   |
| `Instance` | `UnityEngine.RectTransform` |   |
| `KeyDrpPlaceholder` | `System.String` |   |
| `KeyDrpSelect` | `System.String` |   |
| `KeyName` | `System.String` |   |
| `Name` | `System.String` |   |
| `OptionProvider` | `Func`1<List`1<System.String>>` |   |
| `Options` | `List`1<System.String>` | list of System.String append `.<numeric-index>` |
| `Range` | `Nullable`1<ValueTuple`2<System.Single, System.Single>>` |   |
| `RefreshFields` | `System.Boolean` |   |
| `StartRange` | `System.Int32` |   |
| `TypeMap` | `Dictionary`2<System.String, Il2CppSystem.Type>` | dictionary keyed by System.String, values Il2CppSystem.Type append `.<string|integer|enum-key>` |
| `type` | `BlockFieldType` |   |
| `val` | `Il2CppSystem.Object` |   |
| `valType` | `System.String` |   |
| `value` | `Il2CppSystem.Object` |   |

### `BlockFieldType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `BlockMenuType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `BlockStateType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `BlockType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `BuffMathType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `BuffPenalty`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `GetBPText` | `System.String` |   |
| `atomsNeeded` | `BigDouble` |   |
| `effect` | `BigDouble` |   |
| `id` | `System.Int32` |   |
| `mathType` | `BuffMathType` |   |
| `penType` | `StatsBuffPenType` |   |
| `type` | `BuffPenaltyType` |   |

### `BuffPenaltyType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `Buyable`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `allCost` | `BigDouble` |   |
| `amount` | `System.Double` |   |
| `baseCost` | `BigDouble` |   |
| `buyAmount` | `System.Double` |   |
| `costInc` | `BigDouble` |   |
| `maxAmount` | `System.Double` |   |
| `spendable` | `System.Boolean` |   |
| `totalCost` | `BigDouble` |   |

### `CommonMineral`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Attacks` | `AttacksData` |   |
| `Desc` | `System.String` |   |
| `Id` | `System.Int32` |   |
| `KeyDesc` | `System.String` |   |
| `KeyName` | `System.String` |   |
| `Minerals` | `MineralsData` |   |
| `Name` | `System.String` |   |
| `Tarot` | `TarotData` |   |
| `income` | `BigDouble` |   |
| `level` | `BigDouble` |   |

### `DTPScaling`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `cur` | `BigDouble` |   |
| `exp` | `System.Double` |   |
| `inc` | `BigDouble` |   |
| `start` | `System.Int32` |   |

### `DevilDebuff`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `DialogConfirmationType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `DilUpgradeCost`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `cur` | `BigDouble` |   |
| `inc` | `BigDouble` |   |

### `DilationTree`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `TotalDTP` | `System.Int32` |   |
| `bot` | `List`1<DilationTreeUpgrade>` | list of DilationTreeUpgrade append `.<numeric-index>` |
| `center` | `DilationTreeUpgrade` |   |
| `mid` | `List`1<DilationTreeUpgrade>` | list of DilationTreeUpgrade append `.<numeric-index>` |
| `top` | `List`1<DilationTreeUpgrade>` | list of DilationTreeUpgrade append `.<numeric-index>` |

### `DilationTreeLoadout`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `name` | `System.String` |   |
| `value` | `System.String` |   |

### `DilationTreeUpgrade`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `CanBuy` | `System.Boolean` |   |
| `KeyDesc` | `System.String` |   |
| `KeyName` | `System.String` |   |
| `Maxed` | `System.Boolean` |   |
| `Unlocked` | `System.Boolean` |   |
| `axis` | `EtrDilTreeAxis` |   |
| `effect` | `BigDouble` |   |
| `effectPerOne` | `BigDouble` |   |
| `effectPerOneBase` | `BigDouble` |   |
| `effectStep` | `BigDouble` |   |
| `level` | `System.Int32` |   |
| `maxLevel` | `System.Int32` |   |
| `num` | `System.Int32` |   |
| `prev` | `DilationTreeUpgrade` |   |

### `DilationUpgrade`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `buyAmmo` | `System.Int32` |   |
| `buyAmount` | `BigDouble` |   |
| `cost` | `BigDouble` |   |
| `costBase` | `BigDouble` |   |
| `costInc` | `BigDouble` |   |
| `costTotal` | `BigDouble` |   |
| `effect` | `BigDouble` |   |
| `effectPerOne` | `BigDouble` |   |
| `isSoftcap` | `System.Boolean` |   |
| `level` | `BigDouble` |   |
| `num` | `System.Int32` |   |

### `DoubleClickOption`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `Element`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Attacks` | `AttacksData` |   |
| `BindedLoadout` | `PlanetLoadoutSlot` |   |
| `Elements` | `ElementsData` |   |
| `Minerals` | `MineralsData` |   |
| `Plague` | `PlagueData` |   |
| `Singularity` | `SingularityData` |   |
| `Tarot` | `TarotData` |   |
| `Unity` | `UnityData` |   |
| `amount` | `BigDouble` |   |
| `bindedLoadoutId` | `System.Int32` |   |
| `factor1` | `ElementFactor` |   |
| `factor2` | `ElementFactor` |   |
| `factor3` | `ElementFactor` |   |
| `generation` | `BigDouble` |   |
| `type` | `AstroElementType` |   |

### `ElementFactor`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `id` | `System.Int32` |   |
| `type` | `ElementFactorType` |   |
| `unlocked` | `System.Boolean` |   |
| `value` | `BigDouble` |   |

### `ElementFactorType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `ElementNode`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Attacks` | `AttacksData` |   |
| `CanBuy` | `System.Boolean` |   |
| `Elements` | `ElementsData` |   |
| `Minerals` | `MineralsData` |   |
| `Next` | `List`1<ElementNode>` | list of ElementNode append `.<numeric-index>` |
| `Prev` | `List`1<ElementNode>` | list of ElementNode append `.<numeric-index>` |
| `Singularity` | `SingularityData` |   |
| `Tarot` | `TarotData` |   |
| `bought` | `System.Boolean` |   |
| `cost` | `BigDouble` |   |
| `effect` | `BigDouble` |   |
| `id` | `System.Int32` |   |
| `next` | `Il2CppStructArray`1<System.Int32>` |   |
| `prev` | `Il2CppStructArray`1<System.Int32>` |   |
| `type` | `AstroElementType` |   |
| `unlocked` | `System.Boolean` |   |

### `ElementsData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `BindLoadoutUnlocked` | `System.Boolean` |   |
| `ElementTreeUnlocked` | `System.Boolean` |   |
| `Minerals` | `MineralsData` |   |
| `Unlocked` | `System.Boolean` |   |
| `curGeneratingElement` | `AstroElementType` |   |
| `earth` | `Element` |   |
| `elemNodesEarth` | `Dictionary`2<System.Int32, ElementNode>` | dictionary keyed by System.Int32, values ElementNode append `.<string|integer|enum-key>` |
| `elemNodesFire` | `Dictionary`2<System.Int32, ElementNode>` | dictionary keyed by System.Int32, values ElementNode append `.<string|integer|enum-key>` |
| `elemNodesWater` | `Dictionary`2<System.Int32, ElementNode>` | dictionary keyed by System.Int32, values ElementNode append `.<string|integer|enum-key>` |
| `elemNodesWind` | `Dictionary`2<System.Int32, ElementNode>` | dictionary keyed by System.Int32, values ElementNode append `.<string|integer|enum-key>` |
| `fire` | `Element` |   |
| `globalGenMult` | `BigDouble` |   |
| `localSpeed` | `BigDouble` |   |
| `localSpeedBonus` | `BigDouble` |   |
| `localSpeedPow` | `BigDouble` |   |
| `water` | `Element` |   |
| `wind` | `Element` |   |

### `EndoplasmicReticulum`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Plague` | `PlagueData` |   |
| `baseInfectivity` | `BigDouble` |   |
| `baseSpreadPower` | `BigDouble` |   |
| `baseStealth` | `BigDouble` |   |
| `essence` | `BigDouble` |   |
| `infectivity` | `BigDouble` |   |
| `level` | `BigDouble` |   |
| `rarity` | `PlagueEndoRarity` |   |
| `spreadPower` | `BigDouble` |   |
| `stealth` | `BigDouble` |   |
| `type` | `PlagueEndoType` |   |

### `EndoplasmicReticulumUpgrade`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `CanBuy` | `System.Boolean` |   |
| `KeyDesc` | `System.String` |   |
| `KeyName` | `System.String` |   |
| `Maxed` | `System.Boolean` |   |
| `Plague` | `PlagueData` |   |
| `cost` | `BigDouble` |   |
| `effect` | `BigDouble` |   |
| `effectNext` | `BigDouble` |   |
| `id` | `System.Int32` |   |
| `level` | `BigDouble` |   |
| `maxLevel` | `BigDouble` |   |
| `unlocked` | `System.Boolean` |   |

### `EternityChallenge`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `KeyName` | `System.String` |   |
| `Unlocked` | `System.Boolean` |   |
| `completeDiff` | `System.Int32` |   |
| `curDiff` | `System.Int32` |   |
| `goal` | `BigDouble` |   |
| `inChallenge` | `System.Boolean` |   |
| `num` | `System.Int32` |   |
| `penalty` | `BigDouble` |   |
| `reward` | `BigDouble` |   |
| `rewardPenaltyIT4` | `BigDouble` |   |
| `startFrom` | `StartFromEnum` |   |

### `EternityData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `AP` | `BigDouble` |   |
| `APAmmoEP` | `System.Int32` |   |
| `APAmmoIP` | `System.Int32` |   |
| `APAmmoSC` | `System.Int32` |   |
| `APCostEP` | `BigDouble` |   |
| `APCostEPtotal` | `BigDouble` |   |
| `APCostIP` | `BigDouble` |   |
| `APCostIPtotal` | `BigDouble` |   |
| `APCostSC` | `BigDouble` |   |
| `APCostSCtotal` | `BigDouble` |   |
| `APFromAll` | `BigDouble` |   |
| `APFromEP` | `BigDouble` |   |
| `APFromEPAmount` | `BigDouble` |   |
| `APFromIP` | `BigDouble` |   |
| `APFromIPAmount` | `BigDouble` |   |
| `APFromSC` | `BigDouble` |   |
| `APFromSCAmount` | `BigDouble` |   |
| `APbought` | `BigDouble` |   |
| `APfromC` | `System.Int64` |   |
| `APspent` | `BigDouble` |   |
| `APtotal` | `BigDouble` |   |
| `AnimalsUnlocked` | `System.Boolean` |   |
| `CanBreak` | `System.Boolean` |   |
| `ChallengesCompleted` | `System.Boolean` |   |
| `ChallengesCompletedCount` | `System.Int32` |   |
| `ChallengesUnlocked` | `System.Boolean` |   |
| `DP` | `BigDouble` |   |
| `DPIncome` | `BigDouble` |   |
| `DTPScalings` | `List`1<DTPScaling>` | list of DTPScaling append `.<numeric-index>` |
| `DilUpgradeCosts` | `List`1<DilUpgradeCost>` | list of DilUpgradeCost append `.<numeric-index>` |
| `DilationTreeUnlocked` | `System.Boolean` |   |
| `DilationTreeUnlockedPerm` | `System.Boolean` |   |
| `DilationUnlocked` | `System.Boolean` |   |
| `DilationUnlockedPerm` | `System.Boolean` |   |
| `EP` | `BigDouble` |   |
| `LabPermUnlocked` | `System.Boolean` |   |
| `LabUnlocked` | `System.Boolean` |   |
| `OwnedAnimalsCount` | `System.Int32` |   |
| `RsPUpgrades` | `List`1<RPUpgrade>` | list of RPUpgrade append `.<numeric-index>` |
| `SupernovaUnlocked` | `System.Boolean` |   |
| `animalMilestones` | `List`1<System.Boolean>` | list of System.Boolean append `.<numeric-index>` |
| `animalMilestonesAmo` | `System.Int32` |   |
| `bonus` | `List`1<BigDouble>` | list of BigDouble append `.<numeric-index>` |
| `buyAmoAPEP` | `System.Int32` |   |
| `buyAmoAPIP` | `System.Int32` |   |
| `buyAmoAPSC` | `System.Int32` |   |
| `buyAmoDilation` | `System.Int32` |   |
| `buyAmoDtp` | `System.Int32` |   |
| `buyAmoLab` | `System.Int32` |   |
| `challengeMilestones` | `List`1<System.Boolean>` | list of System.Boolean append `.<numeric-index>` |
| `challengeMilestonesAmo` | `System.Int32` |   |
| `challenges` | `List`1<EternityChallenge>` | list of EternityChallenge append `.<numeric-index>` |
| `compChallenges` | `System.Int64` |   |
| `curRP` | `BigDouble` |   |
| `currentAnimalOrder` | `List`1<System.String>` | list of System.String append `.<numeric-index>` |
| `dilationAscendPenalty` | `System.Double` |   |
| `dilationGenExpPenalty` | `System.Double` |   |
| `dilationIPGainPenalty` | `System.Double` |   |
| `dilationLapSpeedPenalty1` | `System.Double` |   |
| `dilationLapSpeedPenalty2` | `System.Double` |   |
| `dilationMaxScore` | `BigDouble` |   |
| `dilationMaxScoreCurrent` | `BigDouble` |   |
| `dilationPowerPenalty` | `System.Double` |   |
| `dilationStarExpPenalty` | `System.Double` |   |
| `dilationTree` | `DilationTree` |   |
| `dilationUpgrades` | `List`1<DilationUpgrade>` | list of DilationUpgrade append `.<numeric-index>` |
| `dtpBought` | `System.Int32` |   |
| `dtpFree` | `System.Int32` |   |
| `dtpMax` | `System.Int32` |   |
| `dtpSpent` | `System.Int32` |   |
| `dtuLoadouts` | `List`1<DilationTreeLoadout>` | list of DilationTreeLoadout append `.<numeric-index>` |
| `eternityMilestones` | `List`1<System.Boolean>` | list of System.Boolean append `.<numeric-index>` |
| `eternityMilestonesAmo` | `System.Int32` |   |
| `eters` | `BigDouble` |   |
| `inDilation` | `System.Boolean` |   |
| `labPtsIncome` | `BigDouble` |   |
| `labPtsNext` | `BigDouble` |   |
| `labPtsNow` | `BigDouble` |   |
| `labPtsScalings` | `List`1<LabPtsScaling>` | list of LabPtsScaling append `.<numeric-index>` |
| `laboratoryUpgrades` | `List`1<LabUpgrade>` | list of LabUpgrade append `.<numeric-index>` |
| `lastRPAmount` | `System.Double` |   |
| `maxChalDiff` | `System.Int32` |   |
| `maxDP` | `BigDouble` |   |
| `maxOwnedAnimalsCount` | `System.Int32` |   |
| `maximumRP` | `BigDouble` |   |
| `nextDtpCost` | `BigDouble` |   |
| `ownedAnimals` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `passiveEterProg` | `BigDouble` |   |
| `rp6softcap` | `System.Boolean` |   |
| `rpFreeLevels` | `List`1<BigDouble>` | list of BigDouble append `.<numeric-index>` |
| `slowdown` | `System.Int32` |   |
| `spendRP` | `BigDouble` |   |
| `stateEternateSafety` | `System.Boolean` |   |
| `stats` | `List`1<EternityStat>` | list of EternityStat append `.<numeric-index>` |
| `supernovaBonuses` | `Il2CppStructArray`1<BigDouble>` |   |
| `supernovaBonusesNext` | `Il2CppStructArray`1<BigDouble>` |   |
| `supernovaLv` | `System.Int32` |   |
| `supernovaReq` | `BigDouble` |   |

### `EternityStat`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `ep` | `BigDouble` |   |
| `ip` | `BigDouble` |   |
| `realTime` | `System.Double` |   |
| `time` | `BigDouble` |   |

### `EtrDilTreeAxis`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `ExpFactorType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `GameBananaData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `click` | `BigDouble` |   |

### `GameData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `AchievementBonus` | `BigDouble` |   |
| `AchievementBonus2` | `BigDouble` |   |
| `AchievementBonus3` | `BigDouble` |   |
| `AttacksUnlocked` | `System.Boolean` |   |
| `AutomationUnlocked` | `System.Boolean` |   |
| `Cheater` | `System.Boolean` |   |
| `CountUnlockedAch` | `System.Int32` |   |
| `CountUnlockedAchSecret` | `System.Int32` |   |
| `DateOFFull` | `Nullable`1<Il2CppSystem.DateTime>` |   |
| `DateTFFull` | `Nullable`1<Il2CppSystem.DateTime>` |   |
| `EternityUnlocked` | `System.Boolean` |   |
| `InfinityUnlocked` | `System.Boolean` |   |
| `LeaderboardUnlocked` | `System.Boolean` |   |
| `MacroUnlocked` | `System.Boolean` |   |
| `MaxTfCanConvert` | `System.Double` |   |
| `MineralsUnlocked` | `System.Boolean` |   |
| `OFUpCapacityCost` | `System.Double` |   |
| `OfGainPerHour` | `System.Double` |   |
| `OfMax` | `System.Double` |   |
| `PrestigeUnlocked` | `System.Boolean` |   |
| `PromotionUnlocked` | `System.Boolean` |   |
| `ShopUnlocked` | `System.Boolean` |   |
| `SlowdownUnlocked` | `System.Boolean` |   |
| `TFUpCapacityCost` | `System.Double` |   |
| `TFUpGainCost` | `System.Double` |   |
| `TfFull` | `System.Boolean` |   |
| `TfGainPerHour` | `System.Double` |   |
| `TfGainPerHourGross` | `System.Double` |   |
| `TfMax` | `System.Double` |   |
| `TotalHoursUnscaled` | `System.Double` |   |
| `TotalMinutesUnscaled` | `System.Double` |   |
| `UnityUnlocked` | `System.Boolean` |   |
| `achArtifact` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `achByte` | `Il2CppStructArray`1<System.Byte>` |   |
| `achDate` | `Dictionary`2<System.Int32, System.Double>` | dictionary keyed by System.Int32, values System.Double append `.<string|integer|enum-key>` |
| `adsLastTime` | `System.Double` |   |
| `adsTime` | `List`1<System.Double>` | list of System.Double append `.<numeric-index>` |
| `analytics` | `AnalyticsMetaData` |   |
| `attacks` | `AttacksData` |   |
| `automation` | `AutomationData` |   |
| `banReason` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredString` |   |
| `bestEPs` | `BigDouble` |   |
| `bestIPs` | `BigDouble` |   |
| `buyAmmoBuyable` | `System.Int32` |   |
| `checkpoints` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `creatorCode` | `System.String` |   |
| `elements` | `ElementsData` |   |
| `eterBroken` | `System.Boolean` |   |
| `eternity` | `EternityData` |   |
| `expon` | `System.Double` |   |
| `fastestEter` | `BigDouble` |   |
| `fastestInf` | `BigDouble` |   |
| `fastestUnity` | `BigDouble` |   |
| `gameBanana` | `GameBananaData` |   |
| `gameSpeedBonus` | `BigDouble` |   |
| `hideAchUnlocked` | `System.Boolean` |   |
| `hideBanMsg` | `System.Boolean` |   |
| `ignoreSavePatchMinerals` | `System.Boolean` |   |
| `income` | `BigDouble` |   |
| `incomePrev` | `BigDouble` |   |
| `infBroken` | `System.Boolean` |   |
| `infinity` | `InfinityData` |   |
| `lastRealtimeEternity` | `System.Double` |   |
| `lastRealtimeInfinity` | `System.Double` |   |
| `lastRealtimeUnity` | `System.Double` |   |
| `lastServerDate` | `Nullable`1<Il2CppSystem.DateTime>` |   |
| `lastSession` | `Il2CppSystem.DateTime` |   |
| `lastStreakDate` | `Il2CppSystem.DateTime` |   |
| `lastTimeEternity` | `BigDouble` |   |
| `lastTimeInfinity` | `BigDouble` |   |
| `lastTimePrestige` | `BigDouble` |   |
| `lastTimePromoted` | `BigDouble` |   |
| `lastTimeUnity` | `BigDouble` |   |
| `lastUseTF` | `Il2CppSystem.DateTime` |   |
| `leaderboard` | `LeaderboardData` |   |
| `macro` | `MacroData` |   |
| `minTrashed` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `minerals` | `MineralsData` |   |
| `ofAuto` | `System.Boolean` |   |
| `ofCapacityLevel` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `ofState` | `System.Boolean` |   |
| `offlineFlux` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredDouble` |   |
| `pMult` | `BigDouble` |   |
| `plague` | `PlagueData` |   |
| `playerId` | `System.String` |   |
| `prestigeExp` | `Il2CppStructArray`1<System.Double>` |   |
| `prestigeMult` | `Il2CppStructArray`1<BigDouble>` |   |
| `profile` | `ProfileData` |   |
| `promotions` | `Il2CppReferenceArray`1<Promotion>` |   |
| `promotionsInf` | `System.Int32` |   |
| `realTimeEquality` | `System.Double` |   |
| `realTimeEtr` | `System.Double` |   |
| `realTimeInf` | `System.Double` |   |
| `realTimeUnity` | `System.Double` |   |
| `revolutions` | `List`1<Revolution>` | list of Revolution append `.<numeric-index>` |
| `saveId` | `System.Int32` |   |
| `score` | `BigDouble` |   |
| `scoreEquality` | `BigDouble` |   |
| `scoreEternity` | `BigDouble` |   |
| `scoreInfinity` | `BigDouble` |   |
| `scorePromotion` | `BigDouble` |   |
| `scoreUnity` | `BigDouble` |   |
| `singularity` | `SingularityData` |   |
| `skin` | `RevolutionSkin` |   |
| `slowdownPower` | `BigDouble` |   |
| `tarot` | `TarotData` |   |
| `tfCapacityLevel` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `tfConvertedPercent` | `System.Single` |   |
| `tfCustomSpeed` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `tfGainLevel` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `theme` | `Nullable`1<ThemeType>` |   |
| `timeEquality` | `BigDouble` |   |
| `timeEqualityUnscaled` | `System.Double` |   |
| `timeEtr` | `BigDouble` |   |
| `timeEtrUnscaled` | `System.Double` |   |
| `timeFlux` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredDouble` |   |
| `timeInf` | `BigDouble` |   |
| `timeInfUnscaled` | `System.Double` |   |
| `timeSavePatchMinerals` | `System.Double` |   |
| `timeSinceStart` | `BigDouble` |   |
| `timeSinceStartUnscaled` | `System.Double` |   |
| `timeTotal` | `BigDouble` |   |
| `timeTotalUnscaled` | `System.Double` |   |
| `timeUnity` | `BigDouble` |   |
| `timeUnityUnscaled` | `System.Double` |   |
| `unity` | `UnityData` |   |
| `unityBroken` | `System.Boolean` |   |
| `unlockedAch` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `version` | `System.Int32` |   |

### `Generator`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Infinity` | `InfinityData` |   |
| `Unity` | `UnityData` |   |
| `allCost` | `BigDouble` |   |
| `amount` | `System.Double` |   |
| `baseCost` | `BigDouble` |   |
| `buyAmount` | `System.Double` |   |
| `costInc` | `BigDouble` |   |
| `fAmount` | `BigDouble` |   |
| `genMult` | `BigDouble` |   |
| `income` | `BigDouble` |   |
| `maxAmount` | `System.Double` |   |
| `totalCost` | `BigDouble` |   |
| `type` | `System.Int32` |   |

### `InfinityChallenge`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `KeyDesc` | `System.String` |   |
| `KeyName` | `System.String` |   |
| `KeyReward` | `System.String` |   |
| `complete` | `System.Boolean` |   |
| `fastest` | `BigDouble` |   |
| `inChallenge` | `System.Boolean` |   |
| `num` | `System.Int32` |   |
| `startFrom` | `StartFromEnum` |   |

### `InfinityData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `CanBreak` | `System.Boolean` |   |
| `ChallengeTotalTime` | `System.Double` |   |
| `ChallengeUnlocked` | `System.Boolean` |   |
| `ChallengeUnlockedPerm` | `System.Boolean` |   |
| `ChallengesCompleted` | `System.Boolean` |   |
| `IP` | `BigDouble` |   |
| `StarUnlocked` | `System.Boolean` |   |
| `StarUnlockedPerm` | `System.Boolean` |   |
| `TotalChallengesCompleted` | `System.Int32` |   |
| `TotalChallengesMult` | `BigDouble` |   |
| `baseStarBase` | `BigDouble` |   |
| `buyAmmoGen` | `System.Int32` |   |
| `challenges` | `Il2CppReferenceArray`1<InfinityChallenge>` |   |
| `dustPerSec` | `BigDouble` |   |
| `genExp` | `System.Double` |   |
| `genMult` | `BigDouble` |   |
| `genPower` | `BigDouble` |   |
| `generators` | `List`1<Generator>` | list of Generator append `.<numeric-index>` |
| `infs` | `BigDouble` |   |
| `maxIPUnity` | `BigDouble` |   |
| `maxInfsUnity` | `BigDouble` |   |
| `ownedInfUpgrades` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `starCost` | `BigDouble` |   |
| `starExp` | `System.Double` |   |
| `starUpgrades` | `List`1<StarUpgrade>` | list of StarUpgrade append `.<numeric-index>` |
| `stardust` | `BigDouble` |   |
| `stardustEffect` | `BigDouble` |   |
| `stardustUpgrades` | `List`1<StardustUpgrade>` | list of StardustUpgrade append `.<numeric-index>` |
| `stars` | `BigDouble` |   |
| `stateInfiniteSafety` | `System.Boolean` |   |
| `stats` | `List`1<InfinityStat>` | list of InfinityStat append `.<numeric-index>` |
| `totalStarBase` | `BigDouble` |   |
| `totalStarBaseNext` | `BigDouble` |   |

### `InfinityStat`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `ip` | `BigDouble` |   |
| `time` | `BigDouble` |   |

### `InventoryData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `BoostArtifactLocalSpeed` | `System.Double` |   |
| `BoostAscPower` | `System.Double` |   |
| `BoostAttacksLaps` | `System.Double` |   |
| `BoostBlackGemEff` | `System.Double` |   |
| `BoostDPGain` | `System.Double` |   |
| `BoostEPGain` | `System.Double` |   |
| `BoostElementsMult` | `System.Double` |   |
| `BoostEndoCraftingMastery` | `System.Double` |   |
| `BoostEtrGain` | `System.Double` |   |
| `BoostFallSpeed` | `System.Double` |   |
| `BoostGenMult` | `System.Double` |   |
| `BoostIPGain` | `System.Double` |   |
| `BoostIncome` | `System.Double` |   |
| `BoostInfGain` | `System.Double` |   |
| `BoostLabPointsGain` | `System.Double` |   |
| `BoostLaps` | `System.Double` |   |
| `BoostLuckTotalPower` | `System.Double` |   |
| `BoostMagnetChance` | `System.Double` |   |
| `BoostMergeExp` | `System.Double` |   |
| `BoostMinLocalGS` | `System.Double` |   |
| `BoostMoonRuneSpeed` | `System.Double` |   |
| `BoostMult` | `System.Double` |   |
| `BoostPExp` | `System.Double` |   |
| `BoostPMult` | `System.Double` |   |
| `BoostPPGain` | `System.Double` |   |
| `BoostPlPPerPlG` | `System.Double` |   |
| `BoostPlagueLocalSpeed` | `System.Double` |   |
| `BoostSingLocalSpeed` | `System.Double` |   |
| `BoostSingMultGain` | `System.Double` |   |
| `BoostSmSacriDustGain` | `System.Double` |   |
| `BoostStardust` | `System.Double` |   |
| `BoostSunRuneSpeed` | `System.Double` |   |
| `BoostTFGain` | `System.Double` |   |
| `BoostTarotGoldenResourcesMult` | `System.Double` |   |
| `BoostTarotLocalSpeed` | `System.Double` |   |
| `BoostTarotResourcesMult` | `System.Double` |   |
| `BoostVEGain` | `System.Double` |   |
| `BoostVPGain` | `System.Double` |   |
| `BoostZodiacLuck` | `System.Double` |   |
| `BoostZodiacQuality` | `System.Double` |   |
| `BoostZodiacSellCost` | `System.Double` |   |
| `CountPendingDailyReward` | `System.Int32` |   |
| `HasBoost` | `System.Boolean` |   |
| `HasPendingDailyReward` | `System.Boolean` |   |
| `RemainingTimeDailyReward` | `Il2CppSystem.TimeSpan` |   |
| `SlotBlock` | `ShopSlotData` |   |
| `SlotMacro` | `ShopSlotData` |   |
| `SlotPlagueInventory` | `ShopSlotData` |   |
| `SlotPlanetLoadout` | `ShopSlotData` |   |
| `SlotSingZodiac` | `ShopSlotData` |   |
| `SlotZodiac` | `ShopSlotData` |   |
| `StreakBonus` | `System.Single` |   |
| `StreakDays` | `System.Int32` |   |
| `StreakPercent` | `System.Single` |   |
| `VEGain` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `adsClicked` | `System.Int32` |   |
| `adsRewarded` | `System.Int32` |   |
| `amountRewarded` | `Dictionary`2<System.Int32, System.Int32>` | dictionary keyed by System.Int32, values System.Int32 append `.<string|integer|enum-key>` |
| `artifactLocalSpeed` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `attacksLaps` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `blackGemEff` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `blocks` | `List`1<BlockType>` | list of BlockType append `.<numeric-index>` |
| `boostAscPower` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostFallSpeed` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostMagnetChance` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostMergeExp` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostMinLocalGS` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostPPGain` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepDPGain` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepEPGain` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepEtrGain` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepGenMult` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepIPGain` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepIncome` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepInfGain` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepLaps` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepMult` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepPExp` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepPMult` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepStardust` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostStepTFGain` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostVPGain` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostZodiacLuck` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostZodiacQuality` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boostZodiacSellCost` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `boughtSoul` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `dailyRewardDay` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `dailyRewarded` | `List`1<System.Int32>` | list of System.Int32 append `.<numeric-index>` |
| `dtuSlotsCount` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `endoCraftingMastery` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `globalElemMult` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `labPointsGain` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `lastRewardDate` | `Nullable`1<Il2CppSystem.DateTime>` |   |
| `lastStreakDate` | `Il2CppSystem.DateTime` |   |
| `luckTotalPower` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `moonRuneSpeed` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `nextZodiacs` | `List`1<UnityZodiac>` | list of UnityZodiac append `.<numeric-index>` |
| `noAds` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredBool` |   |
| `plagueLocalSpeed` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `playerId` | `System.String` |   |
| `plpPerPlg` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `saveId` | `System.Int32` |   |
| `singLocalSpeed` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `singMultGain` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `skins` | `List`1<RevolutionSkin>` | list of RevolutionSkin append `.<numeric-index>` |
| `slotBlock` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `slotMacro` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `slotPlagueInventory` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `slotPlanetLoadout` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `slotSingZodiacInventory` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `slotZodiac` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `smSacriDustGain` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `soul` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `specialPack` | `List`1<SpecialPackType>` | list of SpecialPackType append `.<numeric-index>` |
| `spendedSoul` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `startPack1` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredBool` |   |
| `sunRuneSpeed` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `tarotGoldenResourcesMult` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `tarotLocalSpeed` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `tarotResourcesGenMult` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `unityRerolls` | `CodeStage.AntiCheat.ObscuredTypes.ObscuredInt` |   |
| `usedPromotions` | `List`1<System.String>` | list of System.String append `.<numeric-index>` |
| `version` | `System.Int32` |   |

### `LabPtsScaling`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `power` | `BigDouble` |   |
| `start` | `System.Int32` |   |

### `LabUpgrade`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `KeyName` | `System.String` |   |
| `buyAmmo` | `System.Int32` |   |
| `buyAmount` | `BigDouble` |   |
| `cost` | `BigDouble` |   |
| `costTotal` | `BigDouble` |   |
| `level` | `BigDouble` |   |
| `num` | `System.Int32` |   |
| `value` | `BigDouble` |   |
| `valuePerOne` | `BigDouble` |   |

### `LeaderboardData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `leaderboards` | `Dictionary`2<LeaderboardType, NakamaLeaderboard>` | dictionary keyed by LeaderboardType, values NakamaLeaderboard append `.<string|integer|enum-key>` |
| `maxAnimals` | `BigDouble` |   |
| `maxAtoms` | `BigDouble` |   |
| `maxAttackLevel` | `BigDouble` |   |
| `maxDP` | `BigDouble` |   |
| `maxDTP` | `BigDouble` |   |
| `maxEP` | `BigDouble` |   |
| `maxERLevel` | `BigDouble` |   |
| `maxEternities` | `BigDouble` |   |
| `maxExpon` | `BigDouble` |   |
| `maxIP` | `BigDouble` |   |
| `maxInfinities` | `BigDouble` |   |
| `maxInfinityChallenges` | `BigDouble` |   |
| `maxLabLevel` | `BigDouble` |   |
| `maxScore` | `BigDouble` |   |
| `maxSingularities` | `BigDouble` |   |
| `maxStars` | `BigDouble` |   |
| `maxSupernova` | `BigDouble` |   |
| `maxTrialCount` | `BigDouble` |   |
| `maxUnities` | `BigDouble` |   |
| `maxZodiacLevel` | `BigDouble` |   |
| `rankScore` | `BigDouble` |   |

### `LeaderboardType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `MacroData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Blocks` | `List`1<Block>` | list of Block append `.<numeric-index>` |
| `Slot` | `MacroSlot` |   |
| `selectedId` | `System.Int32` |   |
| `settings` | `MacroSettingData` |   |
| `slots` | `List`1<MacroSlot>` | list of MacroSlot append `.<numeric-index>` |

### `MacroLoopMode`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `MacroSettingData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Mute` | `System.Boolean` |   |
| `Volume` | `System.Single` |   |
| `logsAll` | `System.Boolean` |   |
| `notif` | `System.Boolean` |   |
| `scrollOnPlay` | `System.Boolean` |   |
| `scrollSpeed` | `System.Single` |   |
| `startId` | `System.Int32` |   |
| `volume` | `System.Single` |   |

### `MacroSlot`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `blocks` | `List`1<Block>` | list of Block append `.<numeric-index>` |
| `logs` | `System.Boolean` |   |
| `loopMode` | `MacroLoopMode` |   |
| `name` | `System.String` |   |
| `startup` | `System.Boolean` |   |

### `MineralUpgradeType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `MineralsData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `AllSpecialsSacrificed` | `System.Boolean` |   |
| `Attacks` | `AttacksData` |   |
| `AttacksData` | `AttacksData` |   |
| `AutomationData` | `AutomationData` |   |
| `CanSpawnCommon` | `System.Boolean` |   |
| `CanSpawnSpecial` | `System.Boolean` |   |
| `EternityData` | `EternityData` |   |
| `InfinityData` | `InfinityData` |   |
| `Minerals` | `MineralsData` |   |
| `PolishEnchanceUnlocked` | `System.Boolean` |   |
| `PolishUnlocked` | `System.Boolean` |   |
| `RefineTreeUnlocked` | `System.Boolean` |   |
| `RunesUpgradesUnlocked` | `System.Boolean` |   |
| `SMPboost` | `BigDouble` |   |
| `SacrificeSMUnlocked` | `System.Boolean` |   |
| `Singularity` | `SingularityData` |   |
| `SpecialGridUnlocked` | `System.Boolean` |   |
| `SpecialMineralsProgressionUnlocked` | `System.Boolean` |   |
| `SpecialMineralsSacrificeUnlocked` | `System.Boolean` |   |
| `Tarot` | `TarotData` |   |
| `UnityData` | `UnityData` |   |
| `Unlocked` | `System.Boolean` |   |
| `VPIncome` | `BigDouble` |   |
| `VPRewardAscPower` | `BigDouble` |   |
| `VPRewardMultGain` | `BigDouble` |   |
| `autoSpawn` | `System.Boolean` |   |
| `autoSpawnSpecials` | `System.Boolean` |   |
| `avgSMPLevel` | `BigDouble` |   |
| `bestMineralLevelThisRun` | `BigDouble` |   |
| `buyAmmoMinPolish` | `System.Int32` |   |
| `buyAmmoMinPolishEnch` | `System.Int32` |   |
| `buyAmmoMinUpgrades` | `System.Int32` |   |
| `buyAmmoRefineNode` | `System.Int32` |   |
| `buyAmmoRunes` | `System.Int32` |   |
| `buyAmmoSMP` | `System.Int32` |   |
| `commonMinerals` | `Dictionary`2<System.Int32, CommonMineral>` | dictionary keyed by System.Int32, values CommonMineral append `.<string|integer|enum-key>` |
| `curMineralCost` | `BigDouble` |   |
| `curMineralLevel` | `BigDouble` |   |
| `curSpecialMineralCost` | `BigDouble` |   |
| `curSpecialMineralCostUnrounded` | `BigDouble` |   |
| `flushPercentType` | `System.Int32` |   |
| `freeSlots` | `System.Int32` |   |
| `freeSlotsSpecials` | `System.Int32` |   |
| `gridCommonHeight` | `System.Int32` |   |
| `gridCommonWidth` | `System.Int32` |   |
| `gridSize` | `System.Int32` |   |
| `gridSpecialsHeight` | `System.Int32` |   |
| `gridSpecialsSize` | `System.Int32` |   |
| `gridSpecialsWidth` | `System.Int32` |   |
| `localSpeed` | `BigDouble` |   |
| `localSpeedBonus` | `BigDouble` |   |
| `localSpeedPow` | `System.Double` |   |
| `magnetChance` | `BigDouble` |   |
| `magnetGain` | `BigDouble` |   |
| `magnets` | `BigDouble` |   |
| `maxMineralLevel` | `BigDouble` |   |
| `maxRefineUpgradeBougth` | `System.Int32` |   |
| `mergeCurExp` | `BigDouble` |   |
| `mergeLevel` | `BigDouble` |   |
| `mergeLevelBonus` | `BigDouble` |   |
| `mergeLevelRateBonus` | `BigDouble` |   |
| `mergeLevelUnlocked` | `System.Boolean` |   |
| `mergeNextExp` | `BigDouble` |   |
| `mergeProgress` | `BigDouble` |   |
| `mergeProgressSpecial` | `BigDouble` |   |
| `minMineralCost` | `BigDouble` |   |
| `minSpecialMineralCost` | `BigDouble` |   |
| `mineralBase` | `BigDouble` |   |
| `mineralCostDecrease` | `BigDouble` |   |
| `mineralCostIncrement` | `BigDouble` |   |
| `mineralMult` | `BigDouble` |   |
| `moonRuneProgress` | `BigDouble` |   |
| `moonRunes` | `BigDouble` |   |
| `moonRunesUpgrades` | `Dictionary`2<RuneUpgradeType, RuneUpgrade>` | dictionary keyed by RuneUpgradeType, values RuneUpgrade append `.<string|integer|enum-key>` |
| `onCommonMineralChanged` | `UnityEngine.Events.UnityEvent` |   |
| `onSpecialMineralChanged` | `UnityEngine.Events.UnityEvent` |   |
| `polishEnchanceUpgrades` | `Dictionary`2<PolishUpgradeType, PolishEnchanceUpgrade>` | dictionary keyed by PolishUpgradeType, values PolishEnchanceUpgrade append `.<string|integer|enum-key>` |
| `polishPoints` | `BigDouble` |   |
| `polishPointsNext` | `BigDouble` |   |
| `polishUpgrades` | `Dictionary`2<PolishUpgradeType, PolishUpgrade>` | dictionary keyed by PolishUpgradeType, values PolishUpgrade append `.<string|integer|enum-key>` |
| `refineNodes` | `Dictionary`2<System.Int32, RefineNode>` | dictionary keyed by System.Int32, values RefineNode append `.<string|integer|enum-key>` |
| `refinePoints` | `BigDouble` |   |
| `refinePointsNext` | `BigDouble` |   |
| `sacriDust` | `BigDouble` |   |
| `sacriDustBaseGainMult` | `BigDouble` |   |
| `sacriDustGainMult` | `BigDouble` |   |
| `selectedPolishUpgrade` | `PolishUpgradeType` |   |
| `smmf` | `BigDouble` |   |
| `smpSacri` | `Dictionary`2<SpecialMineralType, SpecialMineralSacrifice>` | dictionary keyed by SpecialMineralType, values SpecialMineralSacrifice append `.<string|integer|enum-key>` |
| `smpUpgrades` | `Dictionary`2<SpecialMineralType, SpecialMineralProgression>` | dictionary keyed by SpecialMineralType, values SpecialMineralProgression append `.<string|integer|enum-key>` |
| `smsChariotReduction` | `BigDouble` |   |
| `specialMineralCostDecrease` | `BigDouble` |   |
| `specialMineralCostIncrement` | `BigDouble` |   |
| `specialMineralEffects` | `Dictionary`2<SpecialMineralType, BigDouble>` | dictionary keyed by SpecialMineralType, values BigDouble append `.<string|integer|enum-key>` |
| `specialMinerals` | `Dictionary`2<System.Int32, SpecialMineral>` | dictionary keyed by System.Int32, values SpecialMineral append `.<string|integer|enum-key>` |
| `specialMineralsSpawned` | `BigDouble` |   |
| `specialsSacrificed` | `System.Int32` |   |
| `sunRuneProgress` | `BigDouble` |   |
| `sunRunes` | `BigDouble` |   |
| `sunRunesUpgrades` | `Dictionary`2<RuneUpgradeType, RuneUpgrade>` | dictionary keyed by RuneUpgradeType, values RuneUpgrade append `.<string|integer|enum-key>` |
| `totalSacriExp` | `BigDouble` |   |
| `trashSpecialConfirm` | `System.Boolean` |   |
| `upgrades` | `Dictionary`2<MineralUpgradeType, MineralsUpgrade>` | dictionary keyed by MineralUpgradeType, values MineralsUpgrade append `.<string|integer|enum-key>` |
| `valuePoints` | `BigDouble` |   |
| `valuePointsMaxPolish` | `BigDouble` |   |
| `valuePointsMaxRefine` | `BigDouble` |   |
| `valuePointsMaxTotal` | `BigDouble` |   |
| `windCostDivider` | `BigDouble` |   |

### `NotationEnum`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `NotificationType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `OneClickOption`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `PlagueData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Attacks` | `AttacksData` |   |
| `CureNerf` | `BigDouble` |   |
| `CureProgress` | `BigDouble` |   |
| `CureReady` | `System.Boolean` |   |
| `CurrentStage` | `PlagueStage` |   |
| `EndoplasmicUnlocked` | `System.Boolean` |   |
| `GlobalStage` | `PlagueGlobalStage` |   |
| `IsCrafting` | `System.Boolean` |   |
| `Minerals` | `MineralsData` |   |
| `PlG` | `BigDouble` |   |
| `PlP` | `BigDouble` |   |
| `PlPperPlG` | `BigDouble` |   |
| `Singularity` | `SingularityData` |   |
| `SlotsBought` | `System.Int32` |   |
| `Tarot` | `TarotData` |   |
| `Unlocked` | `System.Boolean` |   |
| `VE` | `BigDouble` |   |
| `ViP` | `BigDouble` |   |
| `abandonTimes` | `BigDouble` |   |
| `autoHighestStage` | `System.Boolean` |   |
| `autoRepeat` | `System.Boolean` |   |
| `craftCooldown` | `BigDouble` |   |
| `craftERP` | `BigDouble` |   |
| `craftERPNeed` | `BigDouble` |   |
| `craftERPPerS` | `BigDouble` |   |
| `craftEST` | `BigDouble` |   |
| `craftInfectivity` | `BigDouble` |   |
| `craftLvl` | `BigDouble` |   |
| `craftMastery` | `Dictionary`2<PlagueEndoRarity, BigDouble>` | dictionary keyed by PlagueEndoRarity, values BigDouble append `.<string|integer|enum-key>` |
| `craftMaxLvl` | `BigDouble` |   |
| `craftPlPFlushed` | `BigDouble` |   |
| `craftSpreadPower` | `BigDouble` |   |
| `craftStealth` | `BigDouble` |   |
| `cureProgMax` | `BigDouble` |   |
| `cureProgValue` | `BigDouble` |   |
| `cureWasReady` | `System.Boolean` |   |
| `endoInventory` | `List`1<EndoplasmicReticulum>` | list of EndoplasmicReticulum append `.<numeric-index>` |
| `endoUpgrades` | `List`1<EndoplasmicReticulumUpgrade>` | list of EndoplasmicReticulumUpgrade append `.<numeric-index>` |
| `flushCraftAmo` | `System.Int32` |   |
| `globalStages` | `Dictionary`2<PlagueStageType, PlagueGlobalStage>` | dictionary keyed by PlagueStageType, values PlagueGlobalStage append `.<string|integer|enum-key>` |
| `infection` | `System.Boolean` |   |
| `localSpeed` | `BigDouble` |   |
| `localSpeedBonus` | `BigDouble` |   |
| `localSpeedPow` | `BigDouble` |   |
| `maxStage` | `System.Int32` |   |
| `plagueStats` | `Dictionary`2<PlagueStatType, PlagueStat>` | dictionary keyed by PlagueStatType, values PlagueStat append `.<string|integer|enum-key>` |
| `queuedId` | `System.Int32` |   |
| `selectedStageType` | `PlagueStageType` |   |
| `spreadSpeed` | `BigDouble` |   |
| `startFrom` | `StartFromEnum` |   |
| `virus` | `PlagueVirus` |   |

### `PlagueEndoRarity`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `PlagueEndoType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `PlagueGlobalStage`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Selected` | `PlagueStage` |   |
| `selectedId` | `System.Int32` |   |
| `stages` | `List`1<PlagueStage>` | list of PlagueStage append `.<numeric-index>` |
| `type` | `PlagueStageType` |   |

### `PlagueStage`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `KeyName` | `System.String` |   |
| `Minerals` | `MineralsData` |   |
| `Plague` | `PlagueData` |   |
| `Singularity` | `SingularityData` |   |
| `Sprite` | `UnityEngine.Sprite` |   |
| `basePopulation` | `BigDouble` |   |
| `beatTimes` | `BigDouble` |   |
| `healthy` | `BigDouble` |   |
| `id` | `System.Int32` |   |
| `immunity` | `BigDouble` |   |
| `infected` | `BigDouble` |   |
| `infectedPeak` | `BigDouble` |   |
| `progressThis` | `BigDouble` |   |
| `reward` | `BigDouble` |   |
| `type` | `PlagueStageType` |   |

### `PlagueStageType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `PlagueStatType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `PlagueVirus`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Endo` | `EndoplasmicReticulum` |   |
| `Plague` | `PlagueData` |   |
| `equipedId` | `System.Int32` |   |
| `name` | `System.String` |   |
| `statInfectivity` | `BigDouble` |   |
| `statSpreadPower` | `BigDouble` |   |
| `statStealth` | `BigDouble` |   |

### `PlanetLoadoutSlot`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `folded` | `System.Boolean` |   |
| `name` | `System.String` |   |
| `planets` | `Dictionary`2<AstroPlanetType, PlanetLoadoutSlotElement>` | dictionary keyed by AstroPlanetType, values PlanetLoadoutSlotElement append `.<string|integer|enum-key>` |

### `PlanetShopDonutType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `PlanetShopSpaceshipType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `PlanetStatType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `PolishUpgradeType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `PrestigeType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `ProfileData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `notifications` | `Dictionary`2<System.String, System.Int32>` | dictionary keyed by System.String, values System.Int32 append `.<string|integer|enum-key>` |
| `reviewAskCount` | `System.Int32` |   |
| `reviewed` | `System.Boolean` |   |
| `startPackAskCount` | `System.Int32` |   |

### `PromoteObject`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `id` | `System.Int32` |   |
| `levelGain` | `System.Int32` |   |
| `time` | `System.Int32` |   |

### `Promotion`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Available` | `System.Boolean` |   |
| `Locked` | `System.Boolean` |   |
| `effect` | `BigDouble` |   |
| `effect_next` | `BigDouble` |   |
| `level` | `BigDouble` |   |
| `level_next` | `BigDouble` |   |
| `type` | `System.Int32` |   |
| `xpGain` | `BigDouble` |   |
| `xpNext` | `BigDouble` |   |
| `xpNow` | `BigDouble` |   |
| `xpPrev` | `BigDouble` |   |

### `RPUpgrade`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `KeyDesc` | `System.String` |   |
| `KeyName` | `System.String` |   |
| `effect` | `BigDouble` |   |
| `effectPerOne` | `BigDouble` |   |
| `level` | `BigDouble` |   |
| `num` | `System.Int32` |   |

### `RefineNode`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Attacks` | `AttacksData` |   |
| `Bought` | `System.Boolean` |   |
| `CanBuy` | `System.Boolean` |   |
| `Elements` | `ElementsData` |   |
| `Eternity` | `EternityData` |   |
| `Maxed` | `System.Boolean` |   |
| `Minerals` | `MineralsData` |   |
| `Next` | `List`1<RefineNode>` | list of RefineNode append `.<numeric-index>` |
| `Plague` | `PlagueData` |   |
| `Singularity` | `SingularityData` |   |
| `Tarot` | `TarotData` |   |
| `buyAmount` | `BigDouble` |   |
| `cost` | `BigDouble` |   |
| `effect` | `BigDouble` |   |
| `effectModifiers` | `List`1<ValueTuple`3<System.Int32, Func`1<System.Boolean>, Func`2<RefineNode, BigDouble>>>` | list of ValueTuple`3<System.Int32, Func`1<System.Boolean>, Func`2<RefineNode, BigDouble>> append `.<numeric-index>` |
| `id` | `System.Int32` |   |
| `level` | `BigDouble` |   |
| `maxLevel` | `BigDouble` |   |
| `next` | `Il2CppStructArray`1<System.Int32>` |   |
| `prev` | `Il2CppStructArray`1<System.Int32>` |   |
| `unlocked` | `System.Boolean` |   |

### `Relic`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Attacks` | `AttacksData` |   |
| `Elements` | `ElementsData` |   |
| `Minerals` | `MineralsData` |   |
| `ReqLevel` | `BigDouble` |   |
| `Singularity` | `SingularityData` |   |
| `Tarot` | `TarotData` |   |
| `Unity` | `UnityData` |   |
| `amount` | `BigDouble` |   |
| `baseCost` | `BigDouble` |   |
| `buyAmount` | `BigDouble` |   |
| `costData` | `Il2CppReferenceArray`1<ValueTuple`2<BigDouble, BigDouble>>` |   |
| `costInc` | `BigDouble` |   |
| `effect` | `BigDouble` |   |
| `effect_next` | `BigDouble` |   |
| `num` | `System.Int32` |   |
| `regainedLevelsEst` | `BigDouble` |   |
| `sacriEffect` | `BigDouble` |   |
| `sacriLevel` | `BigDouble` |   |
| `totalCost` | `BigDouble` |   |
| `unlocked` | `System.Boolean` |   |

### `Revolution`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `amount` | `System.Double` |   |
| `ascension` | `System.Int64` |   |
| `bCosts` | `Il2CppStructArray`1<BigDouble>` |   |
| `got` | `BigDouble` |   |
| `id` | `System.Int32` |   |
| `maxProgress` | `BigDouble` |   |
| `mult` | `BigDouble` |   |
| `multGain` | `BigDouble` |   |
| `pps` | `BigDouble` |   |
| `ppsNext` | `BigDouble` |   |
| `progress` | `BigDouble` |   |
| `slowdown` | `System.Int64` |   |
| `speed` | `BigDouble` |   |
| `speedNext` | `BigDouble` |   |
| `thisBuyable` | `Buyable` |   |

### `RevolutionSkin`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `RuneUpgradeType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `SettingsData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `BrightText` | `System.Boolean` |   |
| `MuteMusic` | `System.Boolean` |   |
| `MuteSound` | `System.Boolean` |   |
| `VolumeMusic` | `System.Single` |   |
| `VolumeSound` | `System.Single` |   |
| `achPopup` | `System.Boolean` |   |
| `antiFlicker` | `System.Boolean` |   |
| `brightText` | `System.Boolean` |   |
| `darkMode` | `System.Boolean` |   |
| `digitsAnim` | `System.Boolean` |   |
| `dimMode` | `System.Boolean` |   |
| `disabledGameNotif` | `List`1<NotificationType>` | list of NotificationType append `.<numeric-index>` |
| `doubleClickTo` | `List`1<DoubleClickOption>` | list of DoubleClickOption append `.<numeric-index>` |
| `fps` | `System.Boolean` |   |
| `gameNotification` | `System.Boolean` |   |
| `hiddenConfirmation` | `List`1<DialogConfirmationType>` | list of DialogConfirmationType append `.<numeric-index>` |
| `hiddenTabs` | `List`1<BlockMenuType>` | list of BlockMenuType append `.<numeric-index>` |
| `hideAnim` | `List`1<AnimationOption>` | list of AnimationOption append `.<numeric-index>` |
| `hidePrestigeAnim` | `List`1<PrestigeType>` | list of PrestigeType append `.<numeric-index>` |
| `hotkeyAsc` | `UnityEngine.KeyCode` |   |
| `hotkeyBuy` | `UnityEngine.KeyCode` |   |
| `hotkeyEternate` | `UnityEngine.KeyCode` |   |
| `hotkeyInfinite` | `UnityEngine.KeyCode` |   |
| `hotkeyMacroPause` | `UnityEngine.KeyCode` |   |
| `hotkeyMacroPlay` | `UnityEngine.KeyCode` |   |
| `hotkeyMacroStop` | `UnityEngine.KeyCode` |   |
| `hotkeyPrestige` | `UnityEngine.KeyCode` |   |
| `hotkeyProm1` | `UnityEngine.KeyCode` |   |
| `hotkeyProm2` | `UnityEngine.KeyCode` |   |
| `hotkeyProm3` | `UnityEngine.KeyCode` |   |
| `hotkeyProm4` | `UnityEngine.KeyCode` |   |
| `hotkeyUnite` | `UnityEngine.KeyCode` |   |
| `hotkeys` | `System.Boolean` |   |
| `musicMinimized` | `System.Boolean` |   |
| `notation` | `NotationEnum` |   |
| `notificationDot` | `System.Boolean` |   |
| `notifications` | `System.Boolean` |   |
| `oneClickTo` | `List`1<OneClickOption>` | list of OneClickOption append `.<numeric-index>` |
| `reduceSidebar` | `System.Boolean` |   |
| `resetSpeedOnWarp` | `System.Boolean` |   |
| `safetyLock` | `List`1<PrestigeType>` | list of PrestigeType append `.<numeric-index>` |
| `screenOrientation` | `UnityEngine.ScreenOrientation` |   |
| `screenResolutionId` | `System.Int32` |   |
| `seasonalTheme` | `System.Boolean` |   |
| `showTooltip` | `System.Boolean` |   |
| `skinDisplayMode` | `SkinDisplayMode` |   |
| `soundMinimized` | `System.Boolean` |   |
| `tutorials` | `System.Boolean` |   |
| `uiScale` | `System.Single` |   |
| `volumeMusic` | `System.Single` |   |
| `volumeSound` | `System.Single` |   |

### `ShopSlotData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Cost` | `System.Int32` |   |
| `CurrentValue` | `System.Int32` |   |
| `ITEMS` | `List`1<ShopSlotData>` | list of ShopSlotData append `.<numeric-index>` |
| `Inventory` | `InventoryData` |   |
| `KeyName` | `System.String` |   |
| `NextValue` | `System.Int32` |   |
| `ReachMaxStep` | `System.Boolean` |   |
| `Step` | `System.Int32` |   |
| `baseVal` | `System.Int32` |   |
| `cost` | `System.Int32` |   |
| `costFormula` | `Func`3<System.Int32, System.Int32, System.Int32>` |   |
| `id` | `System.Int32` |   |
| `maxStep` | `System.Int32` |   |
| `saveCoroutine` | `UnityEngine.Coroutine` |   |

### `SingEffectSubtype`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `SingFactorType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `SingMilestone`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Elements` | `ElementsData` |   |
| `Minerals` | `MineralsData` |   |
| `Plague` | `PlagueData` |   |
| `Singularity` | `SingularityData` |   |
| `Tarot` | `TarotData` |   |
| `Unlocked` | `System.Boolean` |   |
| `id` | `System.Int32` |   |
| `reached` | `System.Boolean` |   |
| `type` | `SingMilestoneType` |   |
| `value` | `BigDouble` |   |

### `SingMilestoneType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `SingularZodiac`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `ActiveStats` | `List`1<SingularZodiacStatType>` | list of SingularZodiacStatType append `.<numeric-index>` |
| `Element` | `AstroElementType` |   |
| `IsEmpty` | `System.Boolean` |   |
| `Score` | `BigDouble` |   |
| `hasHouse` | `System.Boolean` |   |
| `level` | `BigDouble` |   |
| `locked` | `System.Boolean` |   |
| `rarity` | `ZodiacRarityType` |   |
| `sign` | `AstroSignType` |   |

### `SingularZodiacStatType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `SingularityData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `BuffPenaltyCount` | `System.Int32` |   |
| `EffectsUnlocked` | `System.Boolean` |   |
| `HousesUnlocked` | `System.Boolean` |   |
| `MilestonesUnlocked` | `System.Boolean` |   |
| `Minerals` | `MineralsData` |   |
| `ReadyPart` | `System.Single` |   |
| `ReadyPartNext` | `System.Single` |   |
| `SingMultGainNext` | `BigDouble` |   |
| `SingReady` | `System.Boolean` |   |
| `SingReadyNext` | `System.Boolean` |   |
| `SingStarted` | `System.Boolean` |   |
| `SingZodiacsUnlocked` | `System.Boolean` |   |
| `Singularity` | `SingularityData` |   |
| `TreeUnlocked` | `System.Boolean` |   |
| `Unlocked` | `System.Boolean` |   |
| `UsedZodiacCount` | `System.Int32` |   |
| `atoms` | `BigDouble` |   |
| `atomsGain` | `BigDouble` |   |
| `atomsThreshold` | `BigDouble` |   |
| `atomsThresholdNextSing` | `BigDouble` |   |
| `buffsAndPenalties` | `List`1<BuffPenalty>` | list of BuffPenalty append `.<numeric-index>` |
| `defaultSingZodiacStats` | `Dictionary`2<SingularZodiacStatType, BigDouble>` | dictionary keyed by SingularZodiacStatType, values BigDouble append `.<string|integer|enum-key>` |
| `houses` | `Dictionary`2<AstroSignType, SingularHouse>` | dictionary keyed by AstroSignType, values SingularHouse append `.<string|integer|enum-key>` |
| `housesInventory` | `Dictionary`2<AstroSignType, SingularZodiac>` | dictionary keyed by AstroSignType, values SingularZodiac append `.<string|integer|enum-key>` |
| `inventory` | `Dictionary`2<System.Int32, SingularZodiac>` | dictionary keyed by System.Int32, values SingularZodiac append `.<string|integer|enum-key>` |
| `localSpeed` | `BigDouble` |   |
| `localSpeedBonus` | `BigDouble` |   |
| `localSpeedPow` | `BigDouble` |   |
| `luck` | `BigDouble` |   |
| `luckConversionRate` | `BigDouble` |   |
| `maxAtoms` | `BigDouble` |   |
| `processResetSing` | `System.Boolean` |   |
| `singBonusesAtoms` | `Dictionary`2<SingEffectSubtype, SingEffect>` | dictionary keyed by SingEffectSubtype, values SingEffect append `.<string|integer|enum-key>` |
| `singBonusesAtomsActive` | `Dictionary`2<SingEffectSubtype, SingEffect>` | dictionary keyed by SingEffectSubtype, values SingEffect append `.<string|integer|enum-key>` |
| `singBonusesSing` | `Dictionary`2<SingEffectSubtype, SingEffect>` | dictionary keyed by SingEffectSubtype, values SingEffect append `.<string|integer|enum-key>` |
| `singBonusesSingActive` | `Dictionary`2<SingEffectSubtype, SingEffect>` | dictionary keyed by SingEffectSubtype, values SingEffect append `.<string|integer|enum-key>` |
| `singBonusesSingMult` | `Dictionary`2<SingEffectSubtype, SingEffect>` | dictionary keyed by SingEffectSubtype, values SingEffect append `.<string|integer|enum-key>` |
| `singBonusesSingMultActive` | `Dictionary`2<SingEffectSubtype, SingEffect>` | dictionary keyed by SingEffectSubtype, values SingEffect append `.<string|integer|enum-key>` |
| `singFactors` | `Dictionary`2<SingFactorType, SingFactor>` | dictionary keyed by SingFactorType, values SingFactor append `.<string|integer|enum-key>` |
| `singMilestonesAtoms` | `List`1<SingMilestone>` | list of SingMilestone append `.<numeric-index>` |
| `singMilestonesProg` | `List`1<SingMilestone>` | list of SingMilestone append `.<numeric-index>` |
| `singMilestonesSingularity` | `List`1<SingMilestone>` | list of SingMilestone append `.<numeric-index>` |
| `singMult` | `BigDouble` |   |
| `singMultNext` | `BigDouble` |   |
| `singZodiacLevel` | `BigDouble` |   |
| `singZodiacRarityValues` | `Dictionary`2<ZodiacRarityType, BigDouble>` | dictionary keyed by ZodiacRarityType, values BigDouble append `.<string|integer|enum-key>` |
| `singularity` | `BigDouble` |   |
| `totalNerf` | `BigDouble` |   |
| `totalSingZodiacStats` | `Dictionary`2<SingularZodiacStatType, BigDouble>` | dictionary keyed by SingularZodiacStatType, values BigDouble append `.<string|integer|enum-key>` |
| `treeNodes` | `Dictionary`2<System.Int32, SingularityTreeNode>` | dictionary keyed by System.Int32, values SingularityTreeNode append `.<string|integer|enum-key>` |
| `zodiacLevelMult` | `BigDouble` |   |
| `zodiacLevelSing` | `BigDouble` |   |

### `SingularityTreeBonusType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `SingularityTreeNode`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `CanAscend` | `System.Boolean` |   |
| `CanBuy` | `System.Boolean` |   |
| `DisplayId` | `System.String` |   |
| `Next` | `List`1<SingularityTreeNode>` | list of SingularityTreeNode append `.<numeric-index>` |
| `Prev` | `List`1<SingularityTreeNode>` | list of SingularityTreeNode append `.<numeric-index>` |
| `Purchased` | `System.Boolean` |   |
| `Singularity` | `SingularityData` |   |
| `ascension` | `System.Int32` |   |
| `baseCost` | `BigDouble` |   |
| `cost` | `BigDouble` |   |
| `effect` | `BigDouble` |   |
| `id` | `System.Int32` |   |
| `level` | `System.Int32` |   |
| `maxLevel` | `System.Int32` |   |
| `next` | `Il2CppStructArray`1<System.Int32>` |   |
| `prev` | `Il2CppStructArray`1<System.Int32>` |   |
| `type` | `SingularityTreeBonusType` |   |
| `unlocked` | `System.Boolean` |   |

### `SkinDisplayMode`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `SpecialMineral`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Inventory` | `InventoryData` |   |
| `Minerals` | `MineralsData` |   |
| `Name` | `System.String` |   |
| `Singularity` | `SingularityData` |   |
| `Tarot` | `TarotData` |   |
| `baseEffect` | `BigDouble` |   |
| `effect` | `BigDouble` |   |
| `level` | `BigDouble` |   |
| `type` | `SpecialMineralType` |   |

### `SpecialMineralType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `SpecialPackType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `StarUpgrade`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `amount` | `BigDouble` |   |
| `baseCost` | `BigDouble` |   |
| `costInc` | `BigDouble` |   |
| `totalCost` | `BigDouble` |   |
| `type` | `System.Int32` |   |

### `StardustUpgrade`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `amount` | `BigDouble` |   |
| `baseCost` | `BigDouble` |   |
| `costInc` | `BigDouble` |   |
| `maxAmount` | `BigDouble` |   |
| `totalCost` | `BigDouble` |   |
| `type` | `System.Int32` |   |

### `StartFromEnum`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `StatsBuffPenType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `TarotCard`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Attacks` | `AttacksData` |   |
| `BackgroundColor` | `UnityEngine.Color` |   |
| `Elements` | `ElementsData` |   |
| `Inventory` | `InventoryData` |   |
| `IsActive` | `System.Boolean` |   |
| `KeyEffect1` | `System.String` |   |
| `KeyEffect1Challenge` | `System.String` |   |
| `KeyEffect1New` | `System.String` |   |
| `KeyEffect1Passive` | `System.String` |   |
| `KeyEffect1Useless` | `System.String` |   |
| `KeyEffect2` | `System.String` |   |
| `KeyName` | `System.String` |   |
| `Minerals` | `MineralsData` |   |
| `Passive` | `System.Boolean` |   |
| `Plague` | `PlagueData` |   |
| `RomanNumber` | `System.String` |   |
| `Settings` | `SettingsData` |   |
| `Sprite` | `UnityEngine.Sprite` |   |
| `Tarot` | `TarotData` |   |
| `Unity` | `UnityData` |   |
| `Unlocked` | `System.Boolean` |   |
| `activeTime` | `BigDouble` |   |
| `cooldownTime` | `BigDouble` |   |
| `duration` | `BigDouble` |   |
| `effect1` | `BigDouble` |   |
| `effect2` | `BigDouble` |   |
| `id` | `System.Int32` |   |
| `level` | `BigDouble` |   |
| `type` | `TarotSuitType` |   |

### `TarotCards`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `suitArcan` | `TarotSuit` |   |
| `suitCup` | `TarotSuit` |   |
| `suitPentacle` | `TarotSuit` |   |
| `suitSword` | `TarotSuit` |   |
| `suitWand` | `TarotSuit` |   |

### `TarotChallenge`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `ComingSoon` | `System.Boolean` |   |
| `IsTheTowerLocked` | `System.Boolean` |   |
| `KeyGoal` | `System.String` |   |
| `KeyName` | `System.String` |   |
| `KeyPenalty` | `System.String` |   |
| `KeyReward` | `System.String` |   |
| `Locked` | `System.Boolean` |   |
| `Tarot` | `TarotData` |   |
| `complete` | `System.Boolean` |   |
| `effect` | `BigDouble` |   |
| `goal` | `BigDouble` |   |
| `id` | `System.Int32` |   |
| `inChallenge` | `System.Boolean` |   |
| `maxScore` | `BigDouble` |   |
| `startFrom` | `StartFromEnum` |   |
| `type` | `TarotSuitType` |   |

### `TarotChallengeSuit`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `InChallenge` | `System.Boolean` |   |
| `SelectedChallenge` | `TarotChallenge` |   |
| `Tarot` | `TarotData` |   |
| `challenges` | `List`1<TarotChallenge>` | list of TarotChallenge append `.<numeric-index>` |
| `selectedId` | `System.Int32` |   |
| `suitEffect` | `BigDouble` |   |
| `totalCompleted` | `System.Int32` |   |

### `TarotChallenges`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `PendingChallenge` | `TarotChallenge` |   |
| `SelectedType` | `TarotSuitType` |   |
| `Tarot` | `TarotData` |   |
| `bestLevel13` | `BigDouble` |   |
| `bestQLog` | `BigDouble` |   |
| `bestRarityCup4Value` | `BigDouble` |   |
| `emperorGrowth` | `BigDouble` |   |
| `hieroUses` | `BigDouble` |   |
| `magicianUses` | `BigDouble` |   |
| `suitArcan` | `TarotChallengeSuit` |   |
| `suitCup` | `TarotChallengeSuit` |   |
| `suitPentacle` | `TarotChallengeSuit` |   |
| `suitSword` | `TarotChallengeSuit` |   |
| `suitWand` | `TarotChallengeSuit` |   |
| `totalComplete` | `System.Int32` |   |
| `totalEffect` | `BigDouble` |   |
| `unitiesMade` | `BigDouble` |   |
| `wheelLoses` | `BigDouble` |   |

### `TarotData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Attacks` | `AttacksData` |   |
| `Minerals` | `MineralsData` |   |
| `Plague` | `PlagueData` |   |
| `Singularity` | `SingularityData` |   |
| `TarotArtifactsUnlocked` | `System.Boolean` |   |
| `TarotChallengesUnlocked` | `System.Boolean` |   |
| `TarotUpgradesUnlocked` | `System.Boolean` |   |
| `Unlocked` | `System.Boolean` |   |
| `artifacts` | `Dictionary`2<TarotSuitType, TarotArtifact>` | dictionary keyed by TarotSuitType, values TarotArtifact append `.<string|integer|enum-key>` |
| `cards` | `TarotCards` |   |
| `challenges` | `TarotChallenges` |   |
| `cups` | `BigDouble` |   |
| `deathRandomTiming` | `BigDouble` |   |
| `deathTargetTime` | `BigDouble` |   |
| `devilCardPower` | `BigDouble` |   |
| `devilDebuff` | `DevilDebuff` |   |
| `devilDebuffCooldown` | `BigDouble` |   |
| `draws` | `BigDouble` |   |
| `emperorExp` | `BigDouble` |   |
| `empressBuff` | `BigDouble` |   |
| `goldCups` | `BigDouble` |   |
| `goldPentacles` | `BigDouble` |   |
| `goldSwords` | `BigDouble` |   |
| `goldWands` | `BigDouble` |   |
| `hangedTriggered` | `System.Boolean` |   |
| `hermitBonus` | `BigDouble` |   |
| `inArcanChallenge` | `System.Boolean` |   |
| `localSpeed` | `BigDouble` |   |
| `localSpeedArt` | `BigDouble` |   |
| `localSpeedArtBonus` | `BigDouble` |   |
| `localSpeedArtPow` | `BigDouble` |   |
| `localSpeedBonus` | `BigDouble` |   |
| `localSpeedPow` | `BigDouble` |   |
| `logpowCup` | `BigDouble` |   |
| `logpowPentacle` | `BigDouble` |   |
| `logpowSword` | `BigDouble` |   |
| `logpowWand` | `BigDouble` |   |
| `loversMult` | `BigDouble` |   |
| `onCardDrawed` | `UnityEngine.Events.UnityEvent` |   |
| `pentacles` | `BigDouble` |   |
| `prietressBuff` | `BigDouble` |   |
| `resetResources` | `System.Boolean` |   |
| `sacriDustBonus` | `BigDouble` |   |
| `starLuckMult` | `BigDouble` |   |
| `strengthExp` | `BigDouble` |   |
| `swords` | `BigDouble` |   |
| `totalCards` | `BigDouble` |   |
| `totalUnlocked` | `BigDouble` |   |
| `upgrades` | `TarotUpgrades` |   |
| `wands` | `BigDouble` |   |
| `wheelWinsInARow` | `BigDouble` |   |

### `TarotSuit`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `LockedCardCount` | `System.Int32` |   |
| `MaxCardCount` | `System.Int32` |   |
| `Tarot` | `TarotData` |   |
| `UnlockedCardsCount` | `System.Int32` |   |
| `cards` | `List`1<TarotCard>` | list of TarotCard append `.<numeric-index>` |
| `chance` | `System.Double` |   |
| `chanceNew` | `System.Double` |   |
| `chanceOld` | `System.Double` |   |
| `type` | `TarotSuitType` |   |

### `TarotSuitType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `TarotSuitUpgrades`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `type` | `TarotSuitType` |   |
| `upgrades` | `List`1<TarotUpgrade>` | list of TarotUpgrade append `.<numeric-index>` |

### `TarotUpgrade`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Bought` | `System.Boolean` |   |
| `CanBuy` | `System.Boolean` |   |
| `MeetReq` | `System.Boolean` |   |
| `Minerals` | `MineralsData` |   |
| `Tarot` | `TarotData` |   |
| `cost` | `BigDouble` |   |
| `effect` | `BigDouble` |   |
| `effectNext` | `BigDouble` |   |
| `id` | `System.Int32` |   |
| `level` | `BigDouble` |   |
| `maxLevel` | `BigDouble` |   |
| `reqPrev` | `BigDouble` |   |
| `type` | `TarotSuitType` |   |

### `TarotUpgrades`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `suitCup` | `TarotSuitUpgrades` |   |
| `suitPentacle` | `TarotSuitUpgrades` |   |
| `suitSword` | `TarotSuitUpgrades` |   |
| `suitWand` | `TarotSuitUpgrades` |   |

### `ThemeType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `TrialType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `UnityBonuses`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `DPGain` | `BigDouble` |   |
| `EPGain` | `BigDouble` |   |
| `IPGain` | `BigDouble` |   |
| `eterGain` | `BigDouble` |   |

### `UnityData`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `AllTrials` | `IEnumerable`1<UnityTrial>` |   |
| `BonusTrials` | `List`1<UnityTrial>` | list of UnityTrial append `.<numeric-index>` |
| `CanBreak` | `System.Boolean` |   |
| `EasyTrials` | `List`1<UnityTrial>` | list of UnityTrial append `.<numeric-index>` |
| `HardTrials` | `List`1<UnityTrial>` | list of UnityTrial append `.<numeric-index>` |
| `InsaneTrials` | `List`1<UnityTrial>` | list of UnityTrial append `.<numeric-index>` |
| `MediumTrials` | `List`1<UnityTrial>` | list of UnityTrial append `.<numeric-index>` |
| `NextZodiacs` | `List`1<UnityZodiac>` | list of UnityZodiac append `.<numeric-index>` |
| `PlanetLoadoutUnlocked` | `System.Boolean` |   |
| `PlanetShopUnlocked` | `System.Boolean` |   |
| `SacrificeUnlocked` | `System.Boolean` |   |
| `TarotData` | `TarotData` |   |
| `TrialCountCompleted` | `System.Int32` |   |
| `TrialCountPending` | `System.Int32` |   |
| `TrialsUnlocked` | `System.Boolean` |   |
| `UsedSlotCount` | `System.Int32` |   |
| `_allTrials` | `IEnumerable`1<UnityTrial>` |   |
| `ach229rewds` | `Il2CppStructArray`1<BigDouble>` |   |
| `ach230rewds` | `Il2CppStructArray`1<BigDouble>` |   |
| `ach231rewds` | `Il2CppStructArray`1<BigDouble>` |   |
| `ach232rewds` | `Il2CppStructArray`1<BigDouble>` |   |
| `astrodust` | `BigDouble` |   |
| `baseQuality` | `BigDouble` |   |
| `buyAmmoDonuts` | `System.Int32` |   |
| `completeTrials` | `System.Int32` |   |
| `currentLevel` | `BigDouble` |   |
| `defaultPlanetStats` | `Dictionary`2<PlanetStatType, BigDouble>` | dictionary keyed by PlanetStatType, values BigDouble append `.<string|integer|enum-key>` |
| `defaultStats` | `Dictionary`2<ZodiacStats, BigDouble>` | dictionary keyed by ZodiacStats, values BigDouble append `.<string|integer|enum-key>` |
| `donuts` | `List`1<UnityPlanetShopDonut>` | list of UnityPlanetShopDonut append `.<numeric-index>` |
| `ecSaved` | `System.Int32` |   |
| `elementsQualityPowers` | `Dictionary`2<AstroElementType, BigDouble>` | dictionary keyed by AstroElementType, values BigDouble append `.<string|integer|enum-key>` |
| `expFactors` | `Dictionary`2<ExpFactorType, ExpFactor>` | dictionary keyed by ExpFactorType, values ExpFactor append `.<string|integer|enum-key>` |
| `expFill` | `System.Double` |   |
| `expNext` | `BigDouble` |   |
| `expNow` | `BigDouble` |   |
| `infiniteTimes` | `System.Int32` |   |
| `inventory` | `Dictionary`2<System.Int32, UnityZodiac>` | dictionary keyed by System.Int32, values UnityZodiac append `.<string|integer|enum-key>` |
| `luck` | `BigDouble` |   |
| `maxRarityAdd` | `BigDouble` |   |
| `passiveUnities` | `BigDouble` |   |
| `planetLoadouts` | `List`1<PlanetLoadoutSlot>` | list of PlanetLoadoutSlot append `.<numeric-index>` |
| `planets` | `Dictionary`2<AstroPlanetType, UnityPlanet>` | dictionary keyed by AstroPlanetType, values UnityPlanet append `.<string|integer|enum-key>` |
| `planetsInventory` | `Dictionary`2<AstroPlanetType, UnityZodiac>` | dictionary keyed by AstroPlanetType, values UnityZodiac append `.<string|integer|enum-key>` |
| `powerMultFromTrial3` | `BigDouble` |   |
| `ppBonus` | `BigDouble` |   |
| `processResetUnity` | `System.Boolean` |   |
| `rarityValues` | `Dictionary`2<ZodiacRarityType, BigDouble>` | dictionary keyed by ZodiacRarityType, values BigDouble append `.<string|integer|enum-key>` |
| `sacriStats` | `Dictionary`2<ZodiacStats, SacriStat>` | dictionary keyed by ZodiacStats, values SacriStat append `.<string|integer|enum-key>` |
| `sacrificedZodiacs` | `System.Double` |   |
| `sellMulti` | `BigDouble` |   |
| `spaceship` | `List`1<UnityPlanetShopSpaceship>` | list of UnityPlanetShopSpaceship append `.<numeric-index>` |
| `statFormulas` | `Dictionary`2<ZodiacStats, Func`2<BigDouble, BigDouble>>` | dictionary keyed by ZodiacStats, values Func`2<BigDouble, BigDouble> append `.<string|integer|enum-key>` |
| `stateUniteSafety` | `System.Boolean` |   |
| `stats` | `List`1<UnityStat>` | list of UnityStat append `.<numeric-index>` |
| `totalPlanetStats` | `Dictionary`2<PlanetStatType, BigDouble>` | dictionary keyed by PlanetStatType, values BigDouble append `.<string|integer|enum-key>` |
| `totalStats` | `Dictionary`2<ZodiacStats, BigDouble>` | dictionary keyed by ZodiacStats, values BigDouble append `.<string|integer|enum-key>` |
| `trial` | `Dictionary`2<TrialType, List`1<UnityTrial>>` | dictionary keyed by TrialType, values List`1<UnityTrial> append `.<string|integer|enum-key>` |
| `trial5PowerLimit` | `System.Boolean` |   |
| `unities` | `BigDouble` |   |
| `unityBonuses` | `UnityBonuses` |   |
| `unityCooldown` | `BigDouble` |   |
| `unityCooldownBase` | `BigDouble` |   |
| `zodiacConfig` | `ZodiacConfiguration` |   |
| `zodiacLevelsSum` | `BigDouble` |   |
| `zodiacsCalculated` | `System.Boolean` |   |

### `UnityPlanet`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `bonusType` | `PlanetStatType` |   |
| `bonusValue` | `BigDouble` |   |
| `bonuses` | `Dictionary`2<ValueTuple`2<AstroPlanetType, AstroSeasonType>, ValueTuple`2<PlanetStatType, BigDouble>>` | dictionary keyed by ValueTuple`2<AstroPlanetType, AstroSeasonType>, values ValueTuple`2<PlanetStatType, BigDouble> append `.<string|integer|enum-key>` |
| `rulers` | `Dictionary`2<AstroPlanetType, Il2CppStructArray`1<AstroSignType>>` | dictionary keyed by AstroPlanetType, values Il2CppStructArray`1<AstroSignType> append `.<string|integer|enum-key>` |
| `type` | `AstroPlanetType` |   |
| `unlocked` | `System.Boolean` |   |

### `UnityPlanetShopDonut`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Cost` | `BigDouble` |   |
| `Effect` | `BigDouble` |   |
| `buyAmount` | `BigDouble` |   |
| `cost` | `BigDouble` |   |
| `effectBase` | `BigDouble` |   |
| `effectBaseBonus` | `BigDouble` |   |
| `effectBonus` | `BigDouble` |   |
| `effectMult` | `BigDouble` |   |
| `level` | `BigDouble` |   |
| `type` | `PlanetShopDonutType` |   |

### `UnityPlanetShopSpaceship`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `cost` | `BigDouble` |   |
| `ownded` | `System.Boolean` |   |
| `type` | `PlanetShopSpaceshipType` |   |

### `UnityStat`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `ep` | `BigDouble` |   |
| `level` | `BigDouble` |   |
| `quality` | `BigDouble` |   |
| `rarity` | `ZodiacRarityType` |   |
| `rarityPlus` | `BigDouble` |   |
| `sign` | `AstroSignType` |   |
| `time` | `BigDouble` |   |

### `UnityTrial`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `GroupUnlocked` | `System.Boolean` |   |
| `Minerals` | `MineralsData` |   |
| `Unity` | `UnityData` |   |
| `Unlocked` | `System.Boolean` |   |
| `canBeAchieved` | `System.Boolean` |   |
| `completed` | `System.Boolean` |   |
| `id` | `System.Int32` |   |
| `locked` | `System.Boolean` |   |
| `type` | `TrialType` |   |

### `UnityZodiac`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Element` | `AstroElementType` |   |
| `IsEmpty` | `System.Boolean` |   |
| `RangeOffset` | `System.Int32` |   |
| `Season` | `AstroSeasonType` |   |
| `hasPlanet` | `System.Boolean` |   |
| `level` | `BigDouble` |   |
| `locked` | `System.Boolean` |   |
| `planet` | `UnityPlanet` |   |
| `quality` | `BigDouble` |   |
| `rarity` | `ZodiacRarityType` |   |
| `rarityPlus` | `BigDouble` |   |
| `score` | `BigDouble` |   |
| `sign` | `AstroSignType` |   |
| `stats` | `List`1<ZodiacStat>` | list of ZodiacStat append `.<numeric-index>` |

### `ZodiacConfiguration`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `Amount` | `System.Int32` |   |
| `amount` | `System.Int32` |   |
| `earth` | `AstroSignType` |   |
| `fire` | `AstroSignType` |   |
| `water` | `AstroSignType` |   |
| `wind` | `AstroSignType` |   |

### `ZodiacRarityType`

| Property | CLR type | Collection path extension |
| --- | --- | --- |

### `ZodiacStat`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
| `type` | `ZodiacStats` |   |
| `value` | `BigDouble` |   |

### `ZodiacStats`

| Property | CLR type | Collection path extension |
| --- | --- | --- |
