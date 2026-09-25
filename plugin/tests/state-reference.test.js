import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

for (const { type, fields, absent, count, variants, links = [] } of [
  { type: 'Relic', fields: ['amount', 'totalCost'], absent: ['Attacks', 'Elements', 'Minerals', 'Singularity', 'Tarot', 'Unity'], count: 13 },
  { type: 'DilationTree', fields: ['TotalDTP', 'bot', 'center', 'mid', 'top'], absent: ['prev'], count: 5 },
  { type: 'UnityZodiac', fields: ['Element', 'Season', 'planet', 'stats', 'rarity', 'sign'], absent: [], count: 14, links: ['AstroElementType', 'AstroSeasonType', 'ZodiacRarityType', 'AstroSignType', 'UnityPlanet', 'ZodiacStat'] },
  { type: 'UnityData', fields: ['planets'], absent: [], links: ['AstroPlanetType', 'UnityPlanet'] },
  { type: 'AstroElementType', variants: [['Undefined', '-1'], ['Fire', '0'], ['Earth', '1'], ['Wind', '2'], ['Water', '3'], ['Light', '4']] },
  { type: 'EtrDilTreeAxis', variants: [['UNKNOWN', '-1'], ['CENTER', '0'], ['MIDDLE', '1'], ['TOP', '2'], ['BOTTOM', '3']] },
]) {
  test(`${type} renders ${variants ? 'enum variants' : 'returned fields and type links'}`, () => {
    const elements = new Map();
    runInNewContext(readFileSync(new URL('../STATE_GRAPH.html', import.meta.url), 'utf8').match(/<script>([\s\S]*?)<\/script>/)[1], {
      document: {
        getElementById(id) {
          if (!elements.has(id)) elements.set(id, {
            innerHTML: '',
            textContent: '',
            value: '',
            addEventListener() {},
            querySelectorAll() { return []; },
            scrollIntoView() {},
            classList: { remove() {} },
          });
          return elements.get(id);
        },
        addEventListener() {},
      },
      location: { search: `?select=${type}` },
      history: { replaceState() {} },
      URLSearchParams,
    });
    const rendered = elements.get('detail').innerHTML;
    expect(rendered).toContain('data-active-tab="fields"');
    if (variants) {
      expect(rendered).toContain('<div class="section-label">Variants</div>');
      expect([...rendered.matchAll(/data-copy="([^"]+)"[^>]*><span class="prop-name">[^<]+.*?<span class="prop-type">([^<]+)<\/span>/g)].map(match => [match[1], match[2]])).toEqual(variants);
      return;
    }
    if (count !== undefined) expect(rendered).toContain(`<b>${count}</b> fields`);
    expect(rendered).not.toMatch(/scalar fields|references out|refs out/i);
    const fieldList = rendered.split('class="detail-col detail-col-fields"')[1].split('<div class="section-label">References in</div>')[0];
    expect(fieldList).toContain('<div class="section-label">Fields</div>');
    if (count !== undefined) expect([...fieldList.matchAll(/data-field="([^"]+)"/g)]).toHaveLength(count);
    for (const field of fields) expect(fieldList).toContain(`data-field="${field}"`);
    for (const field of absent) expect(fieldList).not.toContain(`data-field="${field}"`);
    for (const type of links) expect(fieldList).toContain(`class="prop-type node linked" data-target="${type}"`);
  });
}

test('type navigation pushes pages, Back pops them, and search starts a new history', () => {
  const elements = new Map();
  let links = [];
  let tabs = [];
  let results = [];
  let activeTab;
  function element(attributes = {}) {
    let html = '';
    return {
      events: {},
      value: '',
      disabled: false,
      get innerHTML() { return html; },
      set innerHTML(value) { html = value; activeTab = undefined; },
      addEventListener(name, handler) { this.events[name] = handler; },
      getAttribute(name) { return attributes[name]; },
      classList: { add() {}, remove() {}, toggle() {} },
      scrollIntoView() {},
      querySelectorAll(selector) {
        if (selector === '.linked') return links = [...html.matchAll(/class="[^"]*\blinked\b[^"]*" data-target="([^"]+)"/g)].map(match => element({ 'data-target': match[1] }));
        if (selector === '.detail-tab') return tabs = [...html.matchAll(/data-tab="([^"]+)"/g)].map(match => element({ 'data-tab': match[1] }));
        if (selector === '.search-result') return results = [...html.matchAll(/class="search-result"/g)].map(() => element());
        return [];
      },
      querySelector(selector) {
        if (selector !== '.detail-columns') return null;
        return {
          getAttribute() { return activeTab ?? html.match(/data-active-tab="([^"]+)"/)[1]; },
          setAttribute(name, value) { activeTab = value; },
        };
      },
    };
  }
  runInNewContext(readFileSync(new URL('../STATE_GRAPH.html', import.meta.url), 'utf8').match(/<script>([\s\S]*?)<\/script>/)[1], {
    document: {
      getElementById(id) {
        if (!elements.has(id)) elements.set(id, element());
        return elements.get(id);
      },
      addEventListener() {},
      querySelectorAll() { return []; },
    },
    location: { search: '?select=UnityData' },
    history: { replaceState() {} },
    URLSearchParams,
    CSS: { escape(value) { return value; } },
  });
  const back = elements.get('historyBack');
  const search = elements.get('searchInput');
  const detail = elements.get('detail');
  expect(back?.disabled).toBe(true);
  tabs.find(tab => tab.getAttribute('data-tab') === 'fields').events.click();
  links.find(link => link.getAttribute('data-target') === 'UnityZodiac').events.click({ stopPropagation() {} });
  expect(back.disabled).toBe(false);
  tabs.find(tab => tab.getAttribute('data-tab') === 'fields').events.click();
  links.find(link => link.getAttribute('data-target') === 'AstroElementType').events.click({ stopPropagation() {} });
  expect(detail.innerHTML).toContain('<div class="node-name">AstroElementType</div>');
  back.events.click();
  expect(detail.innerHTML).toContain('<div class="node-name">UnityZodiac</div>');
  expect(detail.innerHTML).toContain('data-active-tab="fields"');
  expect(back.disabled).toBe(false);
  back.events.click();
  expect(detail.innerHTML).toContain('<div class="node-name">UnityData</div>');
  expect(detail.innerHTML).toContain('data-active-tab="fields"');
  expect(back.disabled).toBe(true);
  links.find(link => link.getAttribute('data-target') === 'UnityZodiac').events.click({ stopPropagation() {} });
  search.value = 'DilationTree';
  search.events.input();
  expect(back.disabled).toBe(false);
  results[0].events.click();
  expect(detail.innerHTML).toContain('<div class="node-name">DilationTree</div>');
  expect(detail.innerHTML).toContain('data-active-tab="fields"');
  expect(back.disabled).toBe(true);
  links.find(link => link.getAttribute('data-target') === 'DilationTreeUpgrade').events.click({ stopPropagation() {} });
  expect(back.disabled).toBe(false);
  search.value = 'Relic';
  search.events.input();
  search.events.keydown({ key: 'Enter', preventDefault() {} });
  expect(detail.innerHTML).toContain('<div class="node-name">Relic</div>');
  expect(detail.innerHTML).toContain('data-active-tab="fields"');
  expect(back.disabled).toBe(true);
  back.events.click();
  expect(detail.innerHTML).toContain('<div class="node-name">Relic</div>');
});
