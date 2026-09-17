import { stringify } from "./lib/utils.ts";


export default async function() {
  const graph = JSON.parse(rev.read_file("D:\\Project\\rev-idle\\plugin\\STATE_GRAPH.json"));
  const properties = new Map();
  for (const edge of graph.edges) {
    if (!properties.has(edge.from)) properties.set(edge.from, new Map());
    properties.get(edge.from).set(edge.property, edge.valueType);
  }
  const dump = { startedAt: new Date().toISOString(), state: {}, missingScalars: [], errors: {} };

  async function read(path, type, value, depth = 0, identity) {
    try {
      if (depth > 64) throw new Error("Dump traversal exceeded the maximum depth.");
      const list = /^(?:List|Il2CppStructArray|Il2CppReferenceArray)<(.+)>$/.exec(type);
      const dictionary = /^Dictionary<[^,]+, (.+)>$/.exec(type);
      // A null collection slot can mean an already-visited object, not an empty slot.
      if (value === undefined || value === null && (properties.has(type) || list || dictionary)) {
        value = await rev.state(path);
      }
      if (value === null || typeof value !== "object") return value;
      if (list || dictionary) {
        const result = list ? [] : {};
        for (const [key, item] of Object.entries(value)) {
          result[key] = await read(`${path}.${key}`, (list || dictionary)[1], item, depth + 1, identity);
        }
        return result;
      }
      if (!properties.has(type)) return value;

      const result = {};
      for (const [key, childType] of properties.get(type)) {
        if (identity && !identity.includes(key)) continue;
        // System references belong at their canonical top-level paths.
        if (["GameData", "AttacksData", "AutomationData", "ElementsData", "EternityData", "InfinityData",
          "InventoryData", "MineralsData", "PlagueData", "SingularityData", "TarotData", "UnityData"].includes(childType)) continue;
        if (["KeyDesc", "KeyName", "KeyReward", "KeyGoal", "KeyPenalty",
          "KeyEffect1", "KeyEffect1Challenge", "KeyEffect1New", "KeyEffect1Passive", "KeyEffect1Useless",
          "KeyEffect2", "ImagePath", "BackgroundColor", "RomanNumber"].includes(key)) continue;
        // Keep canonical collections and selection IDs instead of their object aliases.
        if (type === "UnityData" && ["AllTrials", "_allTrials", "BonusTrials", "EasyTrials", "MediumTrials", "HardTrials", "InsaneTrials"].includes(key)) continue;
        if (type === "PlagueData" && ["CurrentStage", "GlobalStage"].includes(key)) continue;
        if (type === "PlagueGlobalStage" && key === "Selected") continue;
        if (type === "TarotChallengeSuit" && key === "SelectedChallenge") continue;
        if (type === "MacroData" && ["Blocks", "Slot"].includes(key)) continue;
        // Tree edges already have lowercase next/prev ID arrays.
        if (["ElementNode", "RefineNode", "SingularityTreeNode"].includes(type) && ["Next", "Prev"].includes(key)) continue;
        // Cycles cannot omit scalars; record schema/runtime differences without retrying them.
        if (value[key] === undefined && !properties.has(childType)
          && !/^(?:List|Il2CppStructArray|Il2CppReferenceArray|Dictionary)</.test(childType)) {
          dump.missingScalars.push(`${path}.${key}`);
          continue;
        }
        result[key] = await read(`${path}.${key}`, childType, value[key], depth + 1,
          type === "DilationTreeUpgrade" && key === "prev" ? ["axis", "num"] :
          type === "Block" && key === "Scope" ? ["Index"] :
          type === "TarotChallenges" && key === "PendingChallenge" ? ["type", "id"] : undefined);
      }
      return result;
    } catch (error) {
      dump.errors[path] = String(error);
      return { $error: String(error) };
    }
  }

  for (const path of [
    "gameData.score", "gameData.income", "gameData.gameSpeedBonus",
    "gameData.scoreInfinity", "gameData.scoreEternity", "gameData.scoreUnity",
    "gameData.scorePromotion", "gameData.scoreEquality",
    "gameData.bestIPs", "gameData.bestEPs",
    "gameData.fastestInf", "gameData.fastestEter", "gameData.fastestUnity",
    "gameData.timeInf", "gameData.timeEtr", "gameData.timeUnity",
    "gameData.timeEquality", "gameData.timeTotal", "gameData.timeTotalUnscaled",
    "gameData.PrestigeUnlocked", "gameData.PromotionUnlocked", "gameData.InfinityUnlocked",
    "gameData.EternityUnlocked", "gameData.UnityUnlocked", "gameData.AutomationUnlocked",
    "gameData.AttacksUnlocked", "gameData.MineralsUnlocked", "gameData.MacroUnlocked",
    "gameData.SlowdownUnlocked", "gameData.ShopUnlocked",
    "gameData.infBroken", "gameData.eterBroken", "gameData.unityBroken",
    "gameData.unlockedAch", "gameData.achArtifact", "gameData.checkpoints",
    "gameData.AchievementBonus", "gameData.AchievementBonus2", "gameData.AchievementBonus3",
    "gameData.pMult", "gameData.expon", "gameData.prestigeMult", "gameData.prestigeExp",
    "gameData.revolutions", "gameData.promotions", "gameData.promotionsInf",
    "gameData.infinity", "gameData.eternity", "gameData.unity",
    "gameData.attacks", "gameData.minerals", "gameData.elements",
    "gameData.singularity", "gameData.plague", "gameData.tarot",
    "gameData.automation", "gameData.macro",
    "gameData.timeFlux", "gameData.tfCapacityLevel", "gameData.tfGainLevel",
    "gameData.tfCustomSpeed", "gameData.tfConvertedPercent", "gameData.TfMax", "gameData.TfGainPerHour",
    "gameData.offlineFlux", "gameData.ofCapacityLevel", "gameData.ofState", "gameData.ofAuto",
    "gameData.OfMax", "gameData.OfGainPerHour", "gameData.slowdownPower",
    "gameController.gameSpeed", "gameController.baseMult", "gameController.inventory",
    "infinityController.IPGain", "infinityController.brokenIPGain",
    "eternityController.EPGain", "eternityController.brokenEPGain",
  ]) {
    const [root, property] = path.split(".");
    dump.state[path] = await read(path, properties.get(graph.roots.find(item => item.key === root).node).get(property));
  }

  dump.finishedAt = new Date().toISOString();
  rev.write_file("D:\\Project\\rev-idle\\__dump.json", stringify(dump, 2));
  rev.stop();
}
