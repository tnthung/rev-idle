

class ClickSteps {
  constructor() {
    this.steps = [];
  }

  add(x, y) {
    this.steps.push({ x, y });
    return this;
  }

  async execute() {
    for (const step of this.steps) {
      rev.click(step.x, step.y);
      await rev.sleep(10);
    }
  }

  static claimIP = new ClickSteps()
    .add(1188, 87)
    .add(550, 520);

  static claimEP = new ClickSteps()
    .add(1188, 87)
    .add(550, 500);

  static resetUnity = new ClickSteps()
    .add(1155, 205)
    .add(214, 82)
    .add(1087, 134);
}


function passThrough(v) {
  console.log(v);
  return v;
}

function mantissa(v) { return Number(v.split('e')[0]); }
function exponent(v) { return BigInt(v.split('e')[1]); }


class DT {
  static applicationSteps = new ClickSteps()
    .add(1192, 164)
    .add(1025, 84)
    .add(1019, 556)
    .add(780, 332)
    .add(780, 305)
    .add(782, 328);

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

  async apply() {
    rev.write_clipboard(this.string);
    console.log(`Applied DT steps with string: ${this.string}`);
    await DT.applicationSteps.execute();
  }
}


const DT_AP = [

];


const DT_DP = [

];


const DT_SN = [

];


let initialized = false;

(async () => {
  if (!initialized) {
    rev.resize(1270, 600);
    await ClickSteps.resetUnity.execute();
    initialized = true;
  }

  if (await rev.state("EP") === "0e0") {
    await rev.sleep(5000);
    await ClickSteps.claimIP.execute();
    await rev.sleep(1000);
    await ClickSteps.claimIP.execute();

    await rev.sleep(3000);
    await ClickSteps.claimEP.execute();

    while (exponent(await rev.state("nextEP")) < 15n)
      await rev.sleep(500);

    await ClickSteps.claimEP.execute();
  }

  // console.log('Applying DT steps');
  // await new DT().ctr(1).top(1, 2, 3, 4).apply();
  // console.log(JSON.stringify((await rev.state("eternity.dilationTree.bot.1")), null, 2));
  rev.stop();
})
