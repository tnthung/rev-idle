import { Action } from './lib/action.js';
import { States } from './lib/states.js';


export default async function() {
  for (const item of Object.entries(await States.unityInventory())) {
    console.log(item);
  }
  rev.stop();
}
