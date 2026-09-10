import { States } from "./lib/states";


export default async function() {
  rev.write_file("__zodiac.json", JSON.stringify({
    planetInventory: await States.planetZodiacInventory(),
    unityInventory: await States.unityZodiacInventory(),
    sacrificeState: await States.sacrificeState(),
    attackRelics: await States.attackRelics(),
  }, null, 2));
}
