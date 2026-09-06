

export class Action {
  constructor() {
    this.steps = [];
  }

  click(x, y, delayMs=10) {
    this.steps.push({ type: 'click', x, y, delayMs });
    return this;
  }

  scroll(x, y, length, axis='vertical', delayMs=100) {
    this.steps.push({ type: 'scroll', x, y, length, axis, delayMs });
    return this;
  }

  drag(x1, y1, x2, y2, delayMs=10) {
    this.steps.push({ type: 'drag', x1, y1, x2, y2, delayMs });
    return this;
  }

  wait(ms) {
    this.steps.push({ type: 'wait', ms });
    return this;
  }

  clone() {
    const newSteps = new Action();
    newSteps.steps = [...this.steps];
    return newSteps;
  }

  chain(step) {
    this.steps.push(...step.steps);
    return this;
  }

  async execute() {
    for (let i = 0; i < this.steps.length; i++) {
      const step = this.steps[i];

      switch (step.type) {
        case 'click':
          rev.click(step.x, step.y);
          break;
        case 'scroll':
          rev.scroll(step.x, step.y, step.length, step.axis);
          break;
        case 'drag':
          rev.drag(step.x1, step.y1, step.x2, step.y2);
          break;
        case 'wait':
          await rev.sleep(step.ms);
          continue;
      }

      const nextStep = this.steps[i + 1];
      if (nextStep && nextStep.type !== 'wait')
        await rev.sleep(nextStep.delayMs);
    }
  }


  static Dismiss = new Action().wait(100).click(987, 583);


  static Revolution = new Action().click(1188, 87);
  static ClaimIP    = this.Revolution.clone().click(550, 520);
  static ClaimEP    = this.Revolution.clone().click(550, 500);


  static Infinity = new Action().click(1188, 125);


  static Eternity = new Action().click(1188, 164);

  static EternityChallenge       = this.Eternity.clone().click(401, 83);
  static EternityChallenge1      = this.EternityChallenge.clone().click(112, 254);
  static EternityChallenge2      = this.EternityChallenge.clone().click(300, 268);
  static EternityChallenge3      = this.EternityChallenge.clone().click(542, 265);
  static EternityChallenge4      = this.EternityChallenge.clone().click(756, 268);
  static EternityChallenge5      = this.EternityChallenge.clone().click(118, 362);
  static EternityChallenge6      = this.EternityChallenge.clone().click(329, 388);
  static EternityChallenge7      = this.EternityChallenge.clone().click(548, 392);
  static EternityChallenge8      = this.EternityChallenge.clone().click(742, 384);
  static EternityChallenge9      = this.EternityChallenge.clone().click(161, 496);
  static EternityChallenge10     = this.EternityChallenge.clone().click(324, 502);
  static toggleEternityChallenge = this.EternityChallenge.clone().click(995, 566);

  static Dilation       = this.Eternity.clone().click(855, 85);
  static toggleDilation = this.Dilation.clone().click(201, 148);

  static DilationTree = this.Eternity.clone().click(1023, 79);


  static Unity = new Action().click(1155, 201);

  static Zodiac     = this.Unity.clone().click(63, 84);
  static ZodiacShop = this.Zodiac.clone().click(550, 444);

  static async SellZodiac(n) {
    n = Number(n);
    const row = n / 8;
    const col = n % 8;

    const x = 795 + col * 40;
    const y = 345 + row * 40;

    await ClickSteps.ZodiacShop.execute();
    await rev.drag(x, y, 613, 525);
    await rev.sleep(500);
    rev.click(686, 525);
  }

  static UnityTrial = this.Unity.clone().click(214, 148);
  static ResetUnity = this.Unity.clone().click(1087, 134);


  static Automation = new Action().click(1155, 239);


  static TimeFlow = new Action().click(1155, 277);


  static Settings = new Action().click(1155, 353);
}
