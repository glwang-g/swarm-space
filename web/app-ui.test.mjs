import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const [html, script] = await Promise.all([
  readFile(new URL("../index.html", import.meta.url), "utf8"),
  readFile(new URL("./app.js", import.meta.url), "utf8"),
]);

test("Canvas inspector exposes a Rule Mission panel beside native world events", () => {
  assert.match(html, /RULE MISSIONS/);
  assert.match(html, /id="rule-mission-list"/);
  assert.match(html, /WORLD EVENTS/);
});

test("Rule Mission panel filters projections by the selected observation view", () => {
  assert.match(script, /function renderRuleMissions\(\)/);
  assert.match(script, /mission\.visible_to\.includes\(state\.view\.toUpperCase\(\)\)/);
  assert.match(script, /renderRuleMissions\(\);/);
  assert.match(script, /updateInterface\(\);\n    draw\(\);/);
});
