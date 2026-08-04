const ui = Object.fromEntries([
  "arena", "canvas-wrap", "loading", "canvas-error", "error-message", "retry-button",
  "turn-value", "seed-value", "runtime-badge", "azure-score", "amber-score",
  "progress-label", "progress-fill", "match-title", "match-explanation",
  "azure-strategy", "amber-strategy", "play-button", "step-button", "restart-button",
  "new-seed-button", "drone-empty", "drone-detail", "agent-swatch", "agent-name",
  "agent-role", "agent-cargo", "agent-position", "agent-target", "agent-reason",
  "event-list", "clear-events"
].map((id) => [id.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase()), document.getElementById(id)]));

const state = {
  game: null,
  snapshot: null,
  seed: 42,
  running: false,
  speed: 4,
  view: "all",
  selected: null,
  accumulator: 0,
  previousTime: performance.now(),
  lastLoggedTurn: -1,
  events: [],
  geometry: null,
};

const teamColor = { azure: "#47c4ff", amber: "#f5a24c" };
const roleLabel = { courier: "运输", scout: "侦察", harvester: "采集" };

function start() {
  if (state.game) return;
  try {
    const WebMatch = window.wasmBindings?.WebMatch;
    if (!WebMatch) throw new Error("浏览器适配器没有导出 WebMatch");
    state.game = new WebMatch(state.seed);
    refreshSnapshot(true);
    ui.loading.hidden = true;
    ui.runtimeBadge.textContent = "核心就绪";
    ui.runtimeBadge.className = "runtime-badge ready";
    bindControls();
    resizeCanvas();
    requestAnimationFrame(frame);
  } catch (error) {
    showError(error);
  }
}

function showError(error) {
  ui.loading.hidden = true;
  ui.canvasError.hidden = false;
  ui.errorMessage.textContent = error instanceof Error ? error.message : String(error);
  ui.runtimeBadge.textContent = "加载失败";
  ui.runtimeBadge.className = "runtime-badge error";
}

function refreshSnapshot(forceLog = false) {
  state.snapshot = JSON.parse(state.game.snapshot_json());
  const snapshot = state.snapshot;
  if (forceLog || snapshot.turn !== state.lastLoggedTurn) {
    if (snapshot.lastEvent) {
      state.events.unshift({ turn: snapshot.turn, text: snapshot.lastEvent });
      state.events = state.events.slice(0, 18);
    }
    state.lastLoggedTurn = snapshot.turn;
  }
  if (snapshot.finished) state.running = false;
  if (state.selected && !snapshot.drones.some(sameAgent(state.selected))) state.selected = null;
  updateInterface();
  draw();
}

function frame(now) {
  const elapsed = Math.min(now - state.previousTime, 250);
  state.previousTime = now;
  if (state.running && state.snapshot && !state.snapshot.finished) {
    state.accumulator += elapsed;
    const interval = 240 / state.speed;
    if (state.accumulator >= interval) {
      const steps = Math.min(32, Math.floor(state.accumulator / interval));
      state.accumulator -= steps * interval;
      state.game.run_steps(steps);
      refreshSnapshot();
    }
  }
  requestAnimationFrame(frame);
}

function updateInterface() {
  const s = state.snapshot;
  ui.turnValue.textContent = s.turn;
  ui.seedValue.textContent = state.seed;
  ui.azureScore.textContent = s.scores[0];
  ui.amberScore.textContent = s.scores[1];
  ui.progressLabel.textContent = `${s.turn} / ${s.maxTurns}`;
  ui.progressFill.style.width = `${Math.min(100, s.turn / s.maxTurns * 100)}%`;
  ui.azureStrategy.textContent = s.strategies[0];
  ui.amberStrategy.textContent = s.strategies[1];
  ui.matchTitle.textContent = s.finished ? winnerText(s) : `第 ${s.turn} 回合 · 群体决策中`;
  ui.matchExplanation.textContent = s.turnExplanation || "无人机仅依据局部观察与共享记忆行动。";
  ui.playButton.textContent = state.running ? "Ⅱ 暂停" : (s.finished ? "比赛结束" : "▶ 开始");
  ui.playButton.disabled = s.finished;
  renderEvents();
  renderSelectedAgent();
}

function winnerText(snapshot) {
  if (snapshot.scores[0] === snapshot.scores[1]) return "比赛结束 · 平局";
  return snapshot.scores[0] > snapshot.scores[1] ? "比赛结束 · AZURE 胜" : "比赛结束 · AMBER 胜";
}

function renderEvents() {
  if (!state.events.length) {
    ui.eventList.innerHTML = "<p><time>0</time><span>竞技场尚未产生事件。</span></p>";
    return;
  }
  ui.eventList.replaceChildren(...state.events.map((event) => {
    const row = document.createElement("p");
    const time = document.createElement("time");
    const text = document.createElement("span");
    time.textContent = event.turn;
    text.textContent = event.text;
    row.append(time, text);
    return row;
  }));
}

function renderSelectedAgent() {
  const drone = state.selected && state.snapshot.drones.find((candidate) => sameAgent(state.selected)(candidate) && isDroneVisible(candidate));
  ui.droneEmpty.hidden = Boolean(drone);
  ui.droneDetail.hidden = !drone;
  if (!drone) return;
  state.selected = { team: drone.team, id: drone.id };
  ui.agentSwatch.style.background = teamColor[drone.team];
  ui.agentName.textContent = `${drone.team.toUpperCase()}-${drone.id + 1}`;
  ui.agentRole.textContent = roleLabel[drone.role] ?? drone.role;
  ui.agentCargo.textContent = `${drone.cargo} / 3`;
  ui.agentPosition.textContent = boardLabel(drone.x, drone.y);
  ui.agentTarget.textContent = drone.target ? boardLabel(drone.target.x, drone.target.y) : "—";
  ui.agentReason.textContent = drone.reason;
}

function draw() {
  const s = state.snapshot;
  if (!s) return;
  const canvas = ui.arena;
  const ctx = canvas.getContext("2d");
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  const padding = Math.max(18, Math.min(width, height) * .035);
  const cell = Math.min((width - padding * 2) / s.width, (height - padding * 2) / s.height);
  const boardWidth = cell * s.width;
  const boardHeight = cell * s.height;
  const originX = (width - boardWidth) / 2;
  const originY = (height - boardHeight) / 2;
  state.geometry = { cell, originX, originY };

  ctx.clearRect(0, 0, width, height);
  const gradient = ctx.createRadialGradient(width * .5, height * .45, 0, width * .5, height * .45, Math.max(width, height) * .7);
  gradient.addColorStop(0, "#0b2133");
  gradient.addColorStop(1, "#050c16");
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, width, height);

  const explored = state.view === "all" ? null : new Set(s.explored[state.view === "azure" ? 0 : 1].map(pointKey));
  for (let y = 0; y < s.height; y++) {
    for (let x = 0; x < s.width; x++) {
      const visible = !explored || explored.has(`${x},${y}`);
      ctx.fillStyle = visible ? ((x + y) % 2 ? "#0d2637" : "#0f2b3d") : "#07111d";
      ctx.fillRect(originX + x * cell, originY + y * cell, cell - .5, cell - .5);
    }
  }

  drawTargets(ctx, s, explored);
  for (const wall of s.walls) drawWall(ctx, wall, explored);
  for (const crystal of s.crystals) drawCrystal(ctx, crystal, explored);
  s.bases.forEach((base, index) => drawBase(ctx, base, index));
  for (const drone of s.drones) drawDrone(ctx, drone, explored);

  ctx.strokeStyle = "rgba(97, 159, 194, .28)";
  ctx.lineWidth = 1;
  ctx.strokeRect(originX - .5, originY - .5, boardWidth + 1, boardHeight + 1);
}

function drawWall(ctx, wall, explored) {
  if (explored && !explored.has(pointKey(wall))) return;
  const { cell, originX, originY } = state.geometry;
  const x = originX + wall.x * cell;
  const y = originY + wall.y * cell;
  ctx.fillStyle = "#26394a";
  ctx.fillRect(x + cell * .16, y + cell * .16, cell * .68, cell * .68);
  ctx.strokeStyle = "#3e576c";
  ctx.strokeRect(x + cell * .2, y + cell * .2, cell * .6, cell * .6);
}

function drawCrystal(ctx, crystal, explored) {
  if (explored && !explored.has(pointKey(crystal))) return;
  const { cell, originX, originY } = state.geometry;
  const x = originX + (crystal.x + .5) * cell;
  const y = originY + (crystal.y + .5) * cell;
  const radius = cell * (.18 + Math.min(crystal.amount, 5) * .018);
  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(Math.PI / 4);
  ctx.shadowColor = "#67f3db";
  ctx.shadowBlur = cell * .4;
  ctx.fillStyle = "#68dccd";
  ctx.fillRect(-radius, -radius, radius * 2, radius * 2);
  ctx.restore();
}

function drawBase(ctx, base, index) {
  const { cell, originX, originY } = state.geometry;
  const x = originX + (base.x + .5) * cell;
  const y = originY + (base.y + .5) * cell;
  ctx.save();
  ctx.translate(x, y);
  ctx.strokeStyle = index === 0 ? teamColor.azure : teamColor.amber;
  ctx.lineWidth = Math.max(1.5, cell * .08);
  ctx.globalAlpha = .72;
  ctx.strokeRect(-cell * .34, -cell * .34, cell * .68, cell * .68);
  ctx.strokeRect(-cell * .2, -cell * .2, cell * .4, cell * .4);
  ctx.restore();
}

function drawTargets(ctx, snapshot, explored) {
  const selected = state.selected && snapshot.drones.find(sameAgent(state.selected));
  if (!selected?.target) return;
  if (!isDroneVisible(selected)) return;
  if (explored && !explored.has(`${selected.x},${selected.y}`)) return;
  const { cell, originX, originY } = state.geometry;
  ctx.save();
  ctx.setLineDash([cell * .18, cell * .15]);
  ctx.strokeStyle = teamColor[selected.team];
  ctx.globalAlpha = .5;
  ctx.beginPath();
  ctx.moveTo(originX + (selected.x + .5) * cell, originY + (selected.y + .5) * cell);
  ctx.lineTo(originX + (selected.target.x + .5) * cell, originY + (selected.target.y + .5) * cell);
  ctx.stroke();
  ctx.restore();
}

function drawDrone(ctx, drone, explored) {
  if (!isDroneVisible(drone)) return;
  if (explored && !explored.has(`${drone.x},${drone.y}`)) return;
  const { cell, originX, originY } = state.geometry;
  const x = originX + (drone.x + .5) * cell;
  const y = originY + (drone.y + .5) * cell;
  const selected = state.selected && sameAgent(state.selected)(drone);
  ctx.save();
  ctx.translate(x, y);
  ctx.shadowColor = teamColor[drone.team];
  ctx.shadowBlur = selected ? cell * .8 : cell * .35;
  ctx.fillStyle = teamColor[drone.team];
  ctx.beginPath();
  ctx.moveTo(0, -cell * .3);
  ctx.lineTo(cell * .27, cell * .18);
  ctx.lineTo(0, cell * .1);
  ctx.lineTo(-cell * .27, cell * .18);
  ctx.closePath();
  ctx.fill();
  if (selected) {
    ctx.shadowBlur = 0;
    ctx.strokeStyle = "#ffffff";
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.arc(0, 0, cell * .42, 0, Math.PI * 2);
    ctx.stroke();
  }
  ctx.shadowBlur = 0;
  ctx.fillStyle = "#03101a";
  ctx.font = `700 ${Math.max(8, cell * .27)}px ui-monospace, monospace`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(String(drone.id + 1), 0, cell * .01);
  for (let i = 0; i < drone.cargo; i++) {
    ctx.fillStyle = "#d7fff5";
    ctx.fillRect(-cell * .16 + i * cell * .13, cell * .28, cell * .08, cell * .08);
  }
  ctx.restore();
}

function bindControls() {
  ui.playButton.addEventListener("click", () => {
    if (!state.snapshot.finished) state.running = !state.running;
    updateInterface();
  });
  ui.stepButton.addEventListener("click", () => {
    state.running = false;
    state.game.step();
    refreshSnapshot();
  });
  ui.restartButton.addEventListener("click", () => restart(state.seed));
  ui.newSeedButton.addEventListener("click", () => restart(crypto.getRandomValues(new Uint32Array(1))[0]));
  ui.retryButton.addEventListener("click", () => location.reload());
  ui.clearEvents.addEventListener("click", () => { state.events = []; renderEvents(); });
  document.querySelectorAll("[data-speed]").forEach((button) => button.addEventListener("click", () => {
    state.speed = Number(button.dataset.speed);
    document.querySelectorAll("[data-speed]").forEach((item) => item.classList.toggle("active", item === button));
  }));
  document.querySelectorAll("[data-view]").forEach((button) => button.addEventListener("click", () => {
    state.view = button.dataset.view;
    document.querySelectorAll("[data-view]").forEach((item) => item.classList.toggle("active", item === button));
    draw();
  }));
  ui.arena.addEventListener("click", selectAgent);
  window.addEventListener("resize", resizeCanvas);
  window.addEventListener("keydown", onKeydown);
}

function restart(seed) {
  state.seed = seed >>> 0;
  state.running = false;
  state.selected = null;
  state.events = [];
  state.lastLoggedTurn = -1;
  state.game.restart(state.seed);
  refreshSnapshot(true);
}

function onKeydown(event) {
  if (event.target instanceof HTMLButtonElement) return;
  if (event.code === "Space") { event.preventDefault(); ui.playButton.click(); }
  if (event.key.toLowerCase() === "n") ui.stepButton.click();
  if (event.key.toLowerCase() === "r") ui.restartButton.click();
  if (event.key.toLowerCase() === "g") ui.newSeedButton.click();
  const speedButton = document.querySelector(`[data-speed="${{ "1": 1, "2": 4, "3": 16 }[event.key]}"]`);
  if (speedButton) speedButton.click();
}

function resizeCanvas() {
  const ratio = Math.min(devicePixelRatio || 1, 2);
  const rect = ui.arena.getBoundingClientRect();
  ui.arena.width = Math.max(1, Math.round(rect.width * ratio));
  ui.arena.height = Math.max(1, Math.round(rect.height * ratio));
  const ctx = ui.arena.getContext("2d");
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  draw();
}

function selectAgent(event) {
  if (!state.snapshot || !state.geometry) return;
  const rect = ui.arena.getBoundingClientRect();
  const { cell, originX, originY } = state.geometry;
  const mapX = (event.clientX - rect.left - originX) / cell;
  const mapY = (event.clientY - rect.top - originY) / cell;
  const drone = state.snapshot.drones.find((candidate) => isDroneVisible(candidate) && Math.hypot(candidate.x + .5 - mapX, candidate.y + .5 - mapY) < .48);
  state.selected = drone ? { team: drone.team, id: drone.id } : null;
  updateInterface();
  draw();
}

function sameAgent(reference) {
  return (drone) => drone.team === reference.team && drone.id === reference.id;
}

function isDroneVisible(drone) {
  if (state.view === "all") return true;
  return drone.visibleTo[state.view === "azure" ? 0 : 1];
}

function pointKey(point) { return `${point.x},${point.y}`; }

function boardLabel(x, y) {
  let value = x + 1;
  let column = "";
  while (value > 0) {
    column = String.fromCharCode(65 + (value - 1) % 26) + column;
    value = Math.floor((value - 1) / 26);
  }
  return `${column}${y}`;
}

window.addEventListener("TrunkApplicationStarted", start, { once: true });
window.addEventListener("error", (event) => { if (!state.game) showError(event.error ?? event.message); });
window.addEventListener("unhandledrejection", (event) => { if (!state.game) showError(event.reason); });

// Trunk's inline WASM loader uses top-level await. The external app module can
// therefore execute before `wasmBindings` exists and miss the startup event.
// Keep checking briefly so a slow or cached browser never leaves the controls
// inert on the loading screen.
function waitForBindings() {
  if (state.game) return;
  if (window.wasmBindings?.WebMatch) {
    start();
    return;
  }
  window.setTimeout(waitForBindings, 25);
}

waitForBindings();
