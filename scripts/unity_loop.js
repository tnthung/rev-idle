

class ClickSteps {
  constructor() {
    this.steps = [];
  }

  click(x, y) {
    this.steps.push({ type: 'click', x, y });
    return this;
  }

  drag(x1, y1, x2, y2) {
    this.steps.push({ type: 'drag', x1, y1, x2, y2 });
    return this;
  }

  chain(step) {
    this.steps.push(...step.steps);
    return this;
  }

  wait(gap = 50) {
    this.steps.push({ type: 'wait', wait: gap })
    return this;
  }

  async execute(gap = 50) {
    for (const step of this.steps) {
      switch (step.type) {
        case 'click':
          rev.click(step.x, step.y);
          await rev.sleep(gap);
          break;
        case 'drag':
          rev.drag(step.x1, step.y1, step.x2, step.y2);
          await rev.sleep(gap);
          break;
        case 'wait':
          await rev.sleep(step.wait);
          break;
      }
    }
  }

  clone() {
    const clone = new ClickSteps();
    clone.steps = [...this.steps];
    return clone;
  }

  static Dismiss = new ClickSteps().wait(100).click(987, 583);

  static Revolution = new ClickSteps().click(1188, 87);
  static ClaimIP = ClickSteps.Revolution.clone().click(550, 520);
  static ClaimEP = ClickSteps.Revolution.clone().click(550, 500);

  static Eternal = new ClickSteps().click(1182, 164);

  static EC   = ClickSteps.Eternal.clone().click(401, 83);
  static EC1  = ClickSteps.EC.clone().click(112, 254);
  static EC2  = ClickSteps.EC.clone().click(300, 268);
  static EC3  = ClickSteps.EC.clone().click(542, 265);
  static EC4  = ClickSteps.EC.clone().click(756, 268);
  static EC5  = ClickSteps.EC.clone().click(118, 362);
  static EC6  = ClickSteps.EC.clone().click(329, 388);
  static EC7  = ClickSteps.EC.clone().click(548, 392);
  static EC8  = ClickSteps.EC.clone().click(742, 384);
  static EC9  = ClickSteps.EC.clone().click(161, 496);
  static EC10 = ClickSteps.EC.clone().click(324, 502);

  static StartEC = ClickSteps.EC.clone().click(995, 566);

  static Dilation = ClickSteps.Eternal.clone().click(855, 85);

  static toggleDilation = ClickSteps.Dilation.clone().click(201, 148);

  static DilationTree = ClickSteps.Eternal.clone().click(1023, 79);

  static Unity = new ClickSteps().click(1155, 205);

  static Zodiac = ClickSteps.Unity.clone().click(63, 84);
  static ZodiacShop = ClickSteps.Zodiac.clone().click(550, 444);

  static UC = ClickSteps.Unity.clone().click(214, 82);
  static ResetUnity = ClickSteps.UC.clone().click(1087, 134);
}


function passThrough(v) {
  console.log(v);
  return v;
}

function mantissa(v) { return Number(v.split('e')[0]); }
function exponent(v) { return BigInt(v.split('e')[1]); }

async function wait_for(conditionFn, interval = 500, timeout = 5000) {
  do {
    await rev.sleep(interval)
    if ((timeout -= interval) <= 0) return false;
  } while (!await conditionFn());
  return true;
}

async function wait_for_exponent(key, target, interval = 500) {
  await wait_for(async () => exponent(await rev.state(key)) >= target, interval);
}

async function print_state(key) {
  console.log(JSON.stringify(await rev.state(key), null, 2));
}


class DT {
  static applicationSteps = ClickSteps.DilationTree.clone()
    .click(1014, 547)
    .click(766, 329)
    .chain(ClickSteps.Dismiss)
    .click(766, 306)
    .click(773, 332)
    .chain(ClickSteps.Dismiss);

  static exportationSteps = ClickSteps.DilationTree.clone()
    .click(1014, 547)
    .click(766, 356)
    .chain(ClickSteps.Dismiss);


  static IncC  = ClickSteps.DilationTree.clone().click(106, 202).click(106, 202);
  static IncT1 = ClickSteps.DilationTree.clone().click(277, 209).click(277, 209);
  static IncT2 = ClickSteps.DilationTree.clone().click(448, 203).click(448, 203);
  static IncT3 = ClickSteps.DilationTree.clone().click(613, 205).click(613, 205);
  static IncT4 = ClickSteps.DilationTree.clone().click(797, 206).click(797, 206);
  static IncM1 = ClickSteps.DilationTree.clone().click(271, 293).click(271, 293);
  static IncM2 = ClickSteps.DilationTree.clone().click(446, 381).click(446, 381);
  static IncM3 = ClickSteps.DilationTree.clone().click(629, 466).click(629, 466);
  static IncM4 = ClickSteps.DilationTree.clone().click(805, 540).click(805, 540);
  static IncB1 = ClickSteps.DilationTree.clone().click(95, 297).click(95, 297);
  static IncB2 = ClickSteps.DilationTree.clone().click(102, 372).click(102, 372);
  static IncB3 = ClickSteps.DilationTree.clone().click(100, 458).click(100, 458);
  static IncB4 = ClickSteps.DilationTree.clone().click(95, 554).click(95, 554);

  constructor() {
    this.c  = 0;
    this.t1 = 0;
    this.t2 = 0;
    this.t3 = 0;
    this.t4 = 0;
    this.m1 = 0;
    this.m2 = 0;
    this.m3 = 0;
    this.m4 = 0;
    this.b1 = 0;
    this.b2 = 0;
    this.b3 = 0;
    this.b4 = 0;
  }

  static async current() {
    const current = await rev.state("gameData.eternity.dilationTree");
    return new DT().ctr(current.bot[0].prev.level)
      .top(...current.top.map(raw => raw.level))
      .mid(...current.mid.map(raw => raw.level))
      .bot(...current.bot.map(raw => raw.level));
  }

  static validatePoint(p) {
    if (p < 0 || p > 5) throw new Error('Invalid point value');
  }

  validateChain(type, p1, p2, p3, p4) {
    if (type !== 't' && type !== 'm' && type !== 'b')
      throw new Error('Invalid chain type');

    DT.validatePoint(p1);
    DT.validatePoint(p2);
    DT.validatePoint(p3);
    DT.validatePoint(p4);

    if ((this.c == 0) && (p1 + p2 + p3 + p4) > 0)
      throw new Error('Cannot set any value when c is 0');

    if (this[`${type}1`] == 0 && p1 == 0 && (p2 + p3 + p4) > 0)
      throw new Error(`Cannot set ${type}1 to 0 when following values are non-zero`);

    if (this[`${type}2`] == 0 && p2 == 0 && (p3 + p4) > 0)
      throw new Error(`Cannot set ${type}2 to 0 when following values are non-zero`);

    if (this[`${type}3`] == 0 && p3 == 0 && p4 > 0)
      throw new Error(`Cannot set ${type}3 to 0 when following values are non-zero`);
  }

  ctr(c) {
    DT.validatePoint(c);
    this.c = c;
    return this;
  }

  top(t1, t2, t3, t4) {
    this.validateChain('t', t1, t2, t3, t4);
    this.t1 = t1;
    this.t2 = t2;
    this.t3 = t3;
    this.t4 = t4;
    return this;
  }

  mid(m1, m2, m3, m4) {
    this.validateChain('m', m1, m2, m3, m4);
    this.m1 = m1;
    this.m2 = m2;
    this.m3 = m3;
    this.m4 = m4;
    return this;
  }

  bot(b1, b2, b3, b4) {
    this.validateChain('b', b1, b2, b3, b4);
    this.b1 = b1;
    this.b2 = b2;
    this.b3 = b3;
    this.b4 = b4;
    return this;
  }

  get total() {
    return (this.c +
      this.t1 + this.t2 + this.t3 + this.t4 +
      this.m1 + this.m2 + this.m3 + this.m4 +
      this.b1 + this.b2 + this.b3 + this.b4);
  }

  get string() {
    // Ex: C5;T0,0,0,0;M1,1,5,5;B0,0,0,0
    return [
      `C${this.c}`,
      `T${this.t1},${this.t2},${this.t3},${this.t4}`,
      `M${this.m1},${this.m2},${this.m3},${this.m4}`,
      `B${this.b1},${this.b2},${this.b3},${this.b4}`,
    ].join(';');
  }

  clone() {
    return new DT().ctr(this.c)
      .top(this.t1, this.t2, this.t3, this.t4)
      .mid(this.m1, this.m2, this.m3, this.m4)
      .bot(this.b1, this.b2, this.b3, this.b4);
  }

  async match() {
    return (await DT.current()).string === this.string;
  }

  async apply() {
    if (await this.match()) {
      await ClickSteps.Dismiss.execute();
      return;
    }

    const old = rev.read_clipboard();
    rev.write_clipboard(this.string);
    console.log(`Applied DT steps ${this.total} with string: ${this.string}`);
    await DT.applicationSteps.execute();
    rev.write_clipboard(old);
  }

  static DTP1  = new DT().ctr(1);
  static DTP2  = new DT().ctr(1).top(1, 0, 0, 0);
  static DTP3  = new DT().ctr(1).top(1, 1, 0, 0);
  static DTP4  = new DT().ctr(1).top(1, 1, 1, 0);
  static SN5   = new DT().ctr(1).top(1, 1, 2, 0);  // SN 80
  static DTP5  = new DT().ctr(1).bot(1, 1, 2, 0);
  static DTP6  = DT.DTP5.clone().top(1, 0, 0, 0);
  static DTP7  = DT.DTP6.clone().top(1, 1, 0, 0);
  static SN8   = new DT().ctr(1).top(1, 1, 5, 0);  // SN 105
  static DTP8  = DT.DTP7.clone().bot(1, 1, 3, 0);
  static DTP9  = DT.DTP8.clone().bot(1, 1, 4, 0);
  static DTP10 = DT.DTP9.clone().bot(1, 1, 5, 0);
  static DTP11 = DT.DTP10.clone().mid(1, 0, 0, 0);
  static DTP12 = DT.DTP11.clone().mid(2, 0, 0, 0);
  static ETN13 = new DT().ctr(1).mid(1, 5, 1, 5); // ETN 1e8~1e10
  static DTP13 = new DT().ctr(5).top(1, 1, 0, 0).bot(1, 1, 4, 0);
  static DTP14 = new DT().ctr(5).top(1, 1, 1, 1).mid(1, 0, 0, 0).bot(1, 1, 1, 1);
  static DTP15 = new DT().ctr(5).top(1, 1, 1, 1).mid(1, 0, 0, 0).bot(1, 1, 2, 1);
  static SN16  = new DT().ctr(1).top(1, 1, 5, 0).bot(1, 1, 1, 5); // SN 120
  static DTP16 = new DT().ctr(4).mid(1, 1, 5, 5);
  static DTP17 = DT.DTP16.clone().ctr(5);
  static SN18  = new DT().ctr(1).top(1, 1, 5, 2).bot(1, 1, 1, 5); // SN 128
  static DTP18 = new DT().ctr(5).mid(2, 1, 5, 5);
  static DTP19 = DT.DTP18.clone().mid(3, 1, 5, 5);
  static DTP20 = DT.DTP19.clone().mid(4, 1, 5, 5);
  static DTP21 = DT.DTP20.clone().mid(5, 1, 5, 5);
  static SN22  = new DT().ctr(1).top(1, 1, 5, 5).bot(1, 2, 1, 5); // SN 149
  static DTP22 = new DT().ctr(4).top(1, 1, 0, 0).mid(5, 1, 5, 5);
  static DTP23 = DT.DTP22.clone().ctr(5);
  static DTP24 = DT.DTP23;
  static DTP25 = new DT().ctr(5).mid(5, 1, 5, 5).bot(1, 1, 2, 0);
  static DTP26 = DT.DTP25.clone().bot(1, 1, 3, 0);
  static DTP27 = DT.DTP26.clone().bot(1, 1, 4, 0);
  static DTP28 = DT.DTP27.clone().bot(1, 1, 5, 0);
  static DTP29 = new DT().ctr(5).top(1, 1, 0, 0).mid(5, 1, 5, 5).bot(1, 1, 4, 0);
  static DTP30 = DT.DTP29.clone().bot(1, 1, 5, 0);
  static DTP31 = new DT().ctr(5).top(1, 1, 1, 1).mid(5, 1, 5, 5).bot(1, 1, 4, 0);
  static DTP32 = DT.DTP31.clone().bot(1, 1, 5, 0);
  static DTP33 = new DT().ctr(5).top(1, 4, 0, 0).mid(5, 1, 5, 5).bot(1, 1, 5, 0);
  static DTP34 = DT.DTP33.clone().top(1, 5, 0, 0);
  static SN35  = new DT().ctr(1).top(1, 1, 5, 5).mid(1, 1, 5, 3).bot(1, 5, 1, 5); // SN 152
  static DTP35 = new DT().ctr(5).top(1, 1, 1, 4).mid(5, 1, 5, 5).bot(1, 1, 5, 0);
  static DTP36 = DT.DTP35.clone().top(1, 1, 1, 5);
  static DTP37 = DT.DTP36.clone().bot(1, 1, 5, 1);
  static DTP38 = DT.DTP36.clone().bot(1, 1, 5, 2);
  static DTP39 = new DT().ctr(5).top(1, 1, 1, 5).mid(5, 1, 5, 5).bot(1, 1, 5, 3);
  static SN40  = new DT().ctr(1).top(1, 1, 5, 5).mid(1, 1, 5, 5).bot(4, 5, 1, 5); // SN 154
  static DTP40 = new DT().ctr(5).top(1, 2, 1, 5).mid(5, 1, 5, 5).bot(1, 1, 3, 5);
  static DTP41 = new DT().ctr(1).top(1, 1, 1, 5).mid(1, 5, 5, 5).bot(5, 5, 1, 5);
}


const DT_STAGES = [
  { dtp: 5,  state: "supernovaLv", target: 80,  loadout: DT.SN5 },
  { dtp: 8,  state: "supernovaLv", target: 105, loadout: DT.SN8 },
  { dtp: 13, state: "eters",       target: 1e8, loadout: DT.ETN13 },
  { dtp: 16, state: "supernovaLv", target: 120, loadout: DT.SN16 },
  { dtp: 18, state: "supernovaLv", target: 128, loadout: DT.SN18 },
  { dtp: 22, state: "supernovaLv", target: 149, loadout: DT.SN22 },
  { dtp: 35, state: "supernovaLv", target: 152, loadout: DT.SN35 },
  { dtp: 40, state: "supernovaLv", target: 154, loadout: DT.SN40 },
];


const ZODIAC_POS = [
  [795, 345],
  [843, 349],
  [885, 349],
  [923, 349],
  [974, 347],
  [1010, 347],
  [1065, 350],
  [793, 399],
  [849, 403],
  [882, 392],
  [930, 395],
  [970, 393],
  [1014, 390],
  [1059, 394],
  [798, 443],
  [847, 441],
  [879, 440],
  [931, 440],
  [974, 440],
  [1015, 439],
];


let initialized = false;

(async () => {
  try {
    if (!initialized) {
      rev.resize(1270, 600);
      // await ClickSteps.ResetUnity.execute();
      initialized = true;
    }

    const unityInventory = await rev.state("gameData.unity.inventory");
    if (Object.values(unityInventory).length >= 4) {
      await ClickSteps.ZodiacShop.execute();
      for (const pos of Object.keys(unityInventory)) {
        const [x, y] = ZODIAC_POS[Number(pos)];
        await rev.drag(x, y, 613, 525);
        await rev.sleep(500);
        rev.click(686, 525);
      }
    }

    // bootstrap the unity
    if (Number(await rev.state("EP")) === 0) {
      if (rev.global.runStart !== undefined)
        console.log(`Previous run elapsed time: ${(Date.now() - rev.global.runStart) / 1000}s`);

      console.log(`Bootstrapping the unity`);
      rev.global.runStart = Date.now();

      for (let i=0; i<2; i++) {
        await wait_for_exponent("nextIP", 300n);
        await ClickSteps.ClaimIP.execute();
      }

      await rev.sleep(1000);
      await ClickSteps.ClaimEP.execute();

      for (let i=1; i<4; i++) {
        await wait_for_exponent("nextEP", 10n * BigInt(i));
        await ClickSteps.ClaimEP.execute();
      }

      return;
    }

    // eternal challenge 1-9
    for (let cid=0; cid<9; cid++) {
      const challenge = await rev.state(`gameData.eternity.challenges.${cid}`);
      if (challenge.completeDiff === 5) continue;

      for (let offset = 0; offset < 3; offset++) {
        const offsetCid = cid + offset;
        if (offsetCid >= 9) break;

        for (let attempt = 0; attempt < 2; attempt++) {
          await rev.sleep(100);
          if (await rev.state(`gameData.eternity.challenges.${offsetCid}.completeDiff`) === 5)
            break;

          await ClickSteps[`EC${offsetCid+1}`].execute();
          await ClickSteps.StartEC.execute();

          if (await wait_for(async () => !await rev.state(`gameData.eternity.challenges.${offsetCid}.inChallenge`), 100, 3000))
            await ClickSteps.Dismiss.execute();
          else
            await ClickSteps.StartEC.execute();
        }
      }

      await rev.sleep(2500);
      await ClickSteps.ClaimEP.execute();
      return;
    }

    // eternal challenge 10
    const EC10 = await rev.state(`gameData.eternity.challenges.9`);
    if (EC10.completeDiff !== 5) {
      // enter dilation if not already in it
      if (!await rev.state("gameData.eternity.inDilation")) {
        await ClickSteps.toggleDilation.execute();
        await rev.sleep(EC10.completeDiff === 0 ? 5000 : 500);
      }

      // exit dilation
      await ClickSteps.toggleDilation.execute();

      // start EC10
      await ClickSteps.EC10.execute();
      await ClickSteps.StartEC.execute();

      if (await wait_for(async () => !await rev.state(`gameData.eternity.challenges.9.inChallenge`), 50, 3000))
        await ClickSteps.Dismiss.execute();
      else
        await ClickSteps.StartEC.execute();
      return;
    }

    // bootstrap the dilation
    if (await rev.state("gameData.eternity.dtpBought") === 0) {
      await ClickSteps.toggleDilation.execute();
      await rev.sleep(500);
      await ClickSteps.toggleDilation.execute();
      return;
    }

    // get at least 5 DTPs
    const DTP = await rev.state("gameData.eternity.dtpBought");
    if (DTP < 5) {
      await DT[`DTP${DTP}`].apply();

      for (let i = DTP; i < 5; i++) {
        await ClickSteps.toggleDilation.execute();
        await rev.sleep(1000);
        await ClickSteps.toggleDilation.execute();
      }

      return;
    }

    // meet non-DTP guide milestones before buying more DTPs
    for (const stage of DT_STAGES) {
      const key = `gameData.eternity.${stage.state}`
      if (DTP < stage.dtp || Number(await rev.state(key)) >= stage.target) continue;
      await stage.loadout.apply();
      await wait_for(async () => Number(await rev.state(key)) >= stage.target, 500, 3000);
      await rev.sleep(4000);
      return;
    }

    if (DTP > 40) {
      const start = Date.now();
      while ((Date.now() - start) < 60000) {
        const currentDT = await DT.current();
        const unspent = await rev.state("dtpFree");

        for (let i = 0; i < unspent; i++) {
          if (currentDT.b3 < 5) {
            await DT.IncB3.execute();
          } else if (currentDT.t2 < 5) {
            await DT.IncT2.execute();
          } else if (currentDT.b1 < 5) {
            await DT.IncB1.execute();
          } else if (currentDT.t3 < 5) {
            await DT.IncT3.execute();
          }
        }

        await rev.sleep(1000);
      }

      await ClickSteps.ClaimEP.execute();
      return;
    }

    await DT[`DTP${DTP}`].apply();
    await ClickSteps.toggleDilation.execute();
    await rev.sleep(1000);
    await ClickSteps.toggleDilation.execute();
    await rev.sleep(4000);
  }

  catch (error) {
    console.error(error);
  }
})
