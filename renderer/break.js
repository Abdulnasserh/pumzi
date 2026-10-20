"use strict";

const $ = (id) => document.getElementById(id);
const card = $("card");
const PAD = 16;

const demoApi = {
  action(name) {
    if (name === "done") {
      const count = Number(card.dataset.done || 0) + 1;
      card.dataset.done = String(count);
      window.noodleDemoCelebrate(count);
    }
    if (name !== "closed") console.log("card ->", name);
  },
  resize() {},
  onBreak() {},
  onCelebrated() {},
};

const api = window.noodle || demoApi;
let current = null;
let state = {
  phase: "idle",
  duration: 0,
  startedAt: 0,
  timer: null,
  interval: null,
  autoTimer: null,
  autoRemaining: 0,
  autoDeadline: 0,
  hovering: false,
  closing: false,
  elapsed: 0,
};

const AUTO_CLOSE_SECONDS = 55;
const fmt = (seconds) => {
  const s = Math.max(0, Math.ceil(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

// ---- Sound ----
let audio = null;
function tone(freq, when, duration = 0.22, gain = 0.035) {
  if (!audio) return;
  const osc = audio.createOscillator();
  const amp = audio.createGain();
  osc.type = "sine";
  osc.frequency.value = freq;
  amp.gain.setValueAtTime(0, when);
  amp.gain.linearRampToValueAtTime(gain, when + 0.02);
  amp.gain.exponentialRampToValueAtTime(0.0001, when + duration);
  osc.connect(amp).connect(audio.destination);
  osc.start(when);
  osc.stop(when + duration + 0.02);
}
function chime(kind) {
  if (!current || !current.sound) return;
  try {
    audio = audio || new AudioContext();
    if (audio.state === "suspended") audio.resume();
    const now = audio.currentTime + 0.02;
    if (kind === "done") {
      tone(523.25, now);
      tone(659.25, now + 0.1);
      tone(783.99, now + 0.2, 0.3);
    } else {
      tone(440, now, 0.18, 0.022);
      tone(587.33, now + 0.09, 0.22, 0.018);
    }
  } catch {
    /* audio is a nicety */
  }
}

// ---- Sizing ----
let resizeFrame = null;
function syncSize() {
  if (resizeFrame) cancelAnimationFrame(resizeFrame);
  resizeFrame = requestAnimationFrame(() => {
    resizeFrame = null;
    api.resize(Math.ceil(card.offsetHeight + PAD * 2));
  });
}

// ---- Auto close ----
function clearAuto() {
  if (state.autoTimer) clearTimeout(state.autoTimer);
  state.autoTimer = null;
  state.autoDeadline = 0;
}
function armAuto(seconds = AUTO_CLOSE_SECONDS) {
  clearAuto();
  if (state.phase === "running" || state.phase === "done" || state.closing) return;
  state.autoRemaining = seconds;
  state.autoDeadline = Date.now() + seconds * 1000;
  state.autoTimer = setTimeout(() => dismiss("ignored"), seconds * 1000);
}
function pauseAuto() {
  if (!state.autoTimer) return;
  state.autoRemaining = Math.max(1, (state.autoDeadline - Date.now()) / 1000);
  clearAuto();
}
function resumeAuto() {
  if (state.closing || state.phase === "running" || state.phase === "done") return;
  armAuto(state.autoRemaining || 20);
}

// ---- Content ----
function render(payload) {
  current = payload;
  const b = payload.break;
  const cat = payload.category;

  clearAuto();
  clearInterval(state.interval);
  state = {
    phase: "idle",
    duration: b.durationSeconds,
    startedAt: 0,
    timer: null,
    interval: null,
    autoTimer: null,
    autoRemaining: 0,
    autoDeadline: 0,
    hovering: false,
    closing: false,
    elapsed: 0,
  };

  document.documentElement.style.setProperty("--cat", cat.color);
  document.body.className = payload.position || "bottom-right";

  card.classList.remove("in", "out", "open", "running", "done");
  $("cat").textContent = cat.name;
  $("name").textContent = b.name;
  $("instruction").textContent = b.instruction;
  $("spec").textContent = b.spec;
  $("time").textContent = fmt(b.durationSeconds);
  $("count").textContent = payload.doneToday ? `${payload.doneToday} today` : "";
  $("primary").textContent = "Start";
  $("snooze").textContent = "Snooze";
  $("snooze").disabled = false;
  $("howtoLabel").textContent = "How to";
  $("howto").setAttribute("aria-expanded", "false");
  $("doneSub").textContent = "";
  $("bar").style.transform = "scaleX(0)";
  card.dataset.done = String(payload.doneToday || 0);

  const fig = $("fig");
  if (payload.figureSVG) {
    fig.innerHTML = payload.figureSVG;
  } else {
    fig.innerHTML = '<div class="missing"></div>';
  }

  const list = $("stepList");
  list.innerHTML = "";
  (b.steps || []).forEach((step, i) => {
    const li = document.createElement("li");
    const num = document.createElement("b");
    num.textContent = String(i + 1);
    const text = document.createElement("span");
    text.textContent = step;
    li.append(num, text);
    list.appendChild(li);
  });
  $("tipText").textContent = b.tip || "";
  $("tip").style.display = b.tip ? "" : "none";

  syncSize();
  requestAnimationFrame(() => {
    card.classList.add("in");
    syncSize();
    armAuto();
    chime("show");
  });

  if (payload.expandSteps) setTimeout(openSteps, 420);
}

// ---- Timer ----
function start() {
  if (state.phase !== "idle") return;
  clearAuto();
  state.phase = "running";
  state.startedAt = performance.now();
  state.elapsed = 0;
  card.classList.add("running");
  $("primary").textContent = "Done";
  $("snooze").textContent = "Stop";
  $("bar").style.transform = "scaleX(0)";
  state.interval = setInterval(tick, 200);
  tick();
}

function tick() {
  if (state.phase !== "running") return;
  const elapsed = (performance.now() - state.startedAt) / 1000;
  state.elapsed = elapsed;
  const remaining = Math.max(0, state.duration - elapsed);
  $("time").textContent = fmt(remaining);
  $("bar").style.transform = `scaleX(${Math.min(1, elapsed / state.duration)})`;
  if (remaining <= 0) complete(state.duration);
}

function stopTimer() {
  clearInterval(state.interval);
  state.interval = null;
  state.phase = "idle";
  state.elapsed = 0;
  card.classList.remove("running");
  $("primary").textContent = "Start";
  $("snooze").textContent = "Snooze";
  $("time").textContent = fmt(state.duration);
  $("bar").style.transform = "scaleX(0)";
  armAuto();
}

async function complete(seconds) {
  if (state.phase === "done") return;
  state.phase = "done";
  clearAuto();
  clearInterval(state.interval);
  state.interval = null;
  card.classList.remove("running");
  card.classList.add("done");
  $("bar").style.transform = "scaleX(1)";
  $("time").textContent = "0:00";

  const n = Number(card.dataset.done || 0) + 1;
  card.dataset.done = String(n);
  $("doneSub").textContent = n === 1 ? "First one today. Good start." : `That's ${n} today.`;
  try {
    const result = await api.action("done", { seconds: Math.max(1, Math.round(seconds)) });
    if (result && result.doneToday) {
      card.dataset.done = String(result.doneToday);
      $("doneSub").textContent = result.doneToday === 1 ? "First one today. Good start." : `That's ${result.doneToday} today.`;
    }
  } catch {
    /* keep the optimistic count */
  }
  chime("done");
  setTimeout(() => hide("closed"), 1900);
}

// ---- Steps ----
function openSteps() {
  if (card.classList.contains("open")) return;
  card.classList.add("open");
  $("howtoLabel").textContent = "Hide steps";
  $("howto").setAttribute("aria-expanded", "true");
  pauseAuto();
  setTimeout(syncSize, 10);
}
function closeSteps() {
  if (!card.classList.contains("open")) return;
  card.classList.remove("open");
  $("howtoLabel").textContent = "How to";
  $("howto").setAttribute("aria-expanded", "false");
  setTimeout(syncSize, 10);
  resumeAuto();
}
function toggleSteps() {
  if (card.classList.contains("open")) closeSteps();
  else openSteps();
}

// ---- Exits ----
function showToast(text) {
  const toast = $("toast");
  toast.textContent = text;
  toast.classList.add("show");
}
function dismiss(action) {
  if (state.closing) return;
  if (action === "snooze") {
    pauseAuto();
    api.action("snooze");
    showToast(`See you in ${current && current.snoozeMinutes ? current.snoozeMinutes : 10} min.`);
    setTimeout(() => hide("closed"), 750);
    return;
  }
  if (action === "ignored") {
    hide("ignored");
    return;
  }
  hide(action);
}
function hide(action) {
  if (state.closing) return;
  state.closing = true;
  clearAuto();
  clearInterval(state.interval);
  state.interval = null;
  card.classList.remove("in");
  card.classList.add("out");
  setTimeout(() => api.action(action), 190);
}

// ---- Wiring ----
$("primary").addEventListener("click", () => {
  if (state.phase === "idle") start();
  else if (state.phase === "running") complete(state.elapsed);
});
$("snooze").addEventListener("click", () => {
  if (state.phase === "running") stopTimer();
  else dismiss("snooze");
});
$("howto").addEventListener("click", toggleSteps);
$("skip").addEventListener("click", () => dismiss("skip"));

card.addEventListener("mouseenter", () => {
  state.hovering = true;
  pauseAuto();
});
card.addEventListener("mouseleave", () => {
  state.hovering = false;
  resumeAuto();
});

// Re-measure when fonts or the figure settle.
window.addEventListener("load", syncSize);
if (document.fonts && document.fonts.ready) document.fonts.ready.then(syncSize);

if (window.noodle) {
  window.noodle.onBreak(render);
} else {
  // Browser demo: python3 -m http.server -d desktop, then open /renderer/break.html
  window.noodleDemoCelebrate = (count) => {
    card.dataset.done = String(count);
    $("doneSub").textContent = `That's ${count} today.`;
  };
  fetch("../data/breaks.json")
    .then((r) => r.json())
    .then(async (data) => {
      const b = data.breaks[Math.floor(Math.random() * data.breaks.length)];
      const svg = await fetch(`../assets/${b.figure}`).then((r) => (r.ok ? r.text() : "")).catch(() => "");
      render({
        break: b,
        category: data.categories[b.category],
        figureSVG: svg,
        doneToday: 2,
        snoozeMinutes: 10,
        expandSteps: false,
        sound: false,
        position: "bottom-right",
      });
    });
}
