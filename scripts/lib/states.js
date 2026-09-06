

export class States {
  static currentIP        = ()  => rev.state("IP");
  static currentEP        = ()  => rev.state("EP");
  static nextIP           = ()  => rev.state("nextIP");
  static nextEP           = ()  => rev.state("nextEP");
  static supernovaLevel   = ()  => rev.state("gameData.eternity.supernovaLv");
  static eternities       = ()  => rev.state("gameData.eternity.eters");
  static totalAP          = ()  => rev.state("gameData.eternity.APbought");
  static EC               = async (n) => new EternalChallenge(await rev.state(`gameData.eternity.challenges.${n}`));
  static DilationMaxScore = ()  => rev.state("dilationMaxScoreCurrent");
  static inDilation       = ()  => rev.state("gameData.eternity.inDilation");
  static totalDTP         = ()  => rev.state("DTP");
  static unusedDTP        = ()  => rev.state("dtpFree");
  static unityInventory   = ()  => rev.state("gameData.unity.inventory");
}


class EternalChallenge {
  constructor({ completeDiff, inChallenge, Unlocked, num }) {
    this.challengeLevel = num;
    this.completeDiff = completeDiff;
    this.inChallenge = inChallenge;
    this.Unlocked = Unlocked;
  }
}
