"use strict";

const $ = (id) => document.getElementById(id);
const api = window.noodle;

const ICONS = {
  eyes: '<path d="M1.5 8S3.6 3.6 8 3.6 14.5 8 14.5 8 12.4 12.4 8 12.4 1.5 8 1.5 8Z"/><circle cx="8" cy="8" r="2.2"/>',
  hydration: '<path d="M8 1.8s4.4 4.3 4.4 7.1a4.4 4.4 0 1 1-8.8 0C3.6 6.1 8 1.8 8 1.8Z"/>',
  breathing: '<path d="M2 5.5h7.5a2 2 0 1 0-2-2"/><path d="M2 8.5h9.5a2 2 0 1 1-2 2"/><path d="M2 11.5h5"/>',
  movement: '<path d="M1.5 8h3l1.5-4 3 8 1.5-4h3"/>',
};

const ORDER = ["eyes", "hydration", "breathing", "movement"];
let lastSnapshot = "";
let lastTasks = "";
let savedTimer = null;
let toastTimer = null;

function format(ms) {
  if (ms == null) return "";
  const total = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h}h ${String(m).padStart(2, "0")}m`;
  return `${m}:${String(s).padStart(2, "0")}`;
}

function showSaved() {
  const el = $("saved");
  el.classList.add("show");
  clearTimeout(savedTimer);
  savedTimer = setTimeout(() => el.classList.remove("show"), 1300);
}

function toast(text) {
  const el = $("toast");
  el.textContent = text;
  el.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove("show"), 2200);
}

function ensureOption(select, value, label) {
  const stringValue = String(value);
  if ([...select.options].some((o) => o.value === stringValue)) return;
  const option = document.createElement("option");
  option.value = stringValue;
  option.textContent = label || `${stringValue} minutes`;
  select.appendChild(option);
}

function renderMix(state) {
  const mix = $("mix");
  mix.innerHTML = "";
  for (const key of ORDER) {
    const meta = state.categories[key];
    const on = state.config.categories[key] !== false;
    const card = document.createElement("div");
    card.className = `mix-card${on ? "" : " off"}`;
    card.dataset.cat = key;
    card.style.setProperty("--cat", meta.color);
    card.innerHTML = `
      <div class="mix-top">
        <div class="mix-title">
          <span class="mix-icon"><svg viewBox="0 0 16 16" aria-hidden="true">${ICONS[key]}</svg></span>
          <b></b>
        </div>
        <span class="switch small"><input type="checkbox" data-cat="${key}" ${on ? "checked" : ""} aria-label="Enable ${meta.name} breaks"><i></i></span>
      </div>
      <small></small>
      <span class="mix-count">${state.counts[key] || 0} moves</span>`;
    card.querySelector(".mix-title b").textContent = meta.name;
    card.querySelector("small").textContent = meta.blurb;
    mix.appendChild(card);
  }
}

function renderChart(state) {
  const chart = $("chart");
  chart.innerHTML = "";
  const week = state.stats.week;
  const max = Math.max(1, ...week.map((d) => d.breaks));
  for (const day of week) {
    const wrap = document.createElement("div");
    wrap.className = `bar-wrap${day.today ? " today" : ""}`;
    const n = document.createElement("span");
    n.className = "n";
    n.textContent = day.breaks || "";
    const bar = document.createElement("div");
    bar.className = "bar";
    bar.style.height = day.breaks ? `${12 + (day.breaks / max) * 62}px` : "3px";
    bar.title = `${day.breaks} break${day.breaks === 1 ? "" : "s"}`;
    const label = document.createElement("span");
    label.className = "day";
    label.textContent = day.label;
    wrap.append(n, bar, label);
    chart.appendChild(wrap);
  }
  const total = week.reduce((sum, d) => sum + d.breaks, 0);
  $("chartTotal").textContent = `${total} break${total === 1 ? "" : "s"}`;
}

function renderHero(state) {
  const status = state.status;
  const eyebrow = $("statusEyebrow");
  const countdown = $("countdown");
  const detail = $("statusDetail");
  const focus = state.focus;

  $("pauseLabel").textContent = focus
    ? focus.paused ? "Task paused" : "Task running"
    : state.paused ? "Reminders paused" : "Reminders on";
  $("paused").checked = state.paused;

  if (focus) {
    const remaining = Math.max(0, focus.phaseEndsAt - Date.now());
    $("takeBreak").textContent = focus.phase === "break" ? "Open a break card" : "Take a break now";
    if (focus.paused) {
      eyebrow.textContent = "Paused";
      countdown.textContent = "—";
      detail.textContent = "Pomodoro paused. Flip the switch above when you're ready to continue.";
    } else if (focus.phase === "focus") {
      const session = Math.min(focus.completedSessions + 1, focus.totalSessions);
      eyebrow.textContent = "Focus session";
      countdown.textContent = format(remaining);
      detail.textContent = `${focus.title} · session ${session} of ${focus.totalSessions}. A break follows this session.`;
    } else {
      const long = focus.completedSessions > 0 && focus.completedSessions % focus.sessionsBeforeLong === 0;
      eyebrow.textContent = long ? "Long break" : "Break";
      countdown.textContent = format(remaining);
      detail.textContent = `Away from ${focus.title}. The next focus session starts automatically.`;
    }
    return;
  }

  if (status.state === "paused") {
    eyebrow.textContent = "Paused";
    countdown.textContent = "—";
    detail.textContent = "Reminders are off. Flip the switch above when you're ready to start again.";
  } else if (status.state === "away") {
    eyebrow.textContent = "Away";
    countdown.textContent = "—";
    detail.textContent = "The clock resets when you're back at the keyboard.";
  } else if (status.state === "quiet") {
    eyebrow.textContent = "Quiet hours";
    countdown.textContent = "—";
    detail.textContent = `No micro-breaks from ${state.config.quietHours.start} to ${state.config.quietHours.end}. Add a task whenever you're ready.`;
  } else {
    eyebrow.textContent = "No task running";
    countdown.textContent = "—";
    detail.textContent = "Add or start a task below to begin a Pomodoro. PUMZI starts the clock when you start a task.";
  }

  $("takeBreak").textContent = state.stats.totalBreaks ? "Take a break now" : "Take your first break";
}

function renderStats(state) {
  $("statToday").textContent = state.stats.todayBreaks;
  $("statStreak").textContent = state.stats.streak;
  $("streakFoot").textContent = state.stats.streak === 1 ? "day in a row" : "days in a row";
  $("statTotal").textContent = state.stats.totalBreaks;
  $("statMinutes").textContent = Math.round(state.stats.totalSeconds / 60);
  renderChart(state);
}

function applyConfig(state) {
  const c = state.config;
  ensureOption($("interval"), c.intervalMinutes);
  ensureOption($("snooze"), c.snoozeMinutes);
  $("interval").value = String(c.intervalMinutes);
  $("snooze").value = String(c.snoozeMinutes);
  $("position").value = c.position;

  $("quietEnabled").checked = c.quietHours.enabled;
  $("quietStart").value = c.quietHours.start;
  $("quietEnd").value = c.quietHours.end;
  $("quietStart").disabled = !c.quietHours.enabled;
  $("quietEnd").disabled = !c.quietHours.enabled;
  $("quietHint").textContent = c.quietHours.enabled
    ? `No cards from ${c.quietHours.start} to ${c.quietHours.end}.`
    : "No cards during this window.";

  $("resetWhenAway").checked = c.resetWhenAway;
  $("sound").checked = c.sound;
  $("expandSteps").checked = c.expandSteps;
  $("launchAtLogin").checked = c.launchAtLogin;
}

function renderTaskList(state) {
  const list = $("taskList");
  list.innerHTML = "";
  const tasks = state.tasks || [];
  const open = tasks.filter((task) => !task.completedAt && (!state.focus || task.id !== state.focus.taskId));
  const done = tasks.filter((task) => task.completedAt).slice(0, 3);

  if (!tasks.length) {
    const empty = document.createElement("p");
    empty.className = "task-empty";
    empty.textContent = "No tasks yet. Add a focus budget above and PUMZI will schedule the breaks.";
    list.append(empty);
    return;
  }

  for (const task of [...open, ...done]) {
    const row = document.createElement("div");
    row.className = `task-row${task.completedAt ? " done" : ""}`;
    const copy = document.createElement("div");
    copy.className = "task-copy";
    const name = document.createElement("b");
    name.textContent = task.title;
    const meta = document.createElement("small");
    const pomodoros = Math.max(1, Math.ceil(task.minutes / 25));
    meta.textContent = task.completedAt
      ? `Done · ${task.minutes} focus min`
      : `${task.minutes} focus min · ${pomodoros} pomodoro${pomodoros === 1 ? "" : "s"}`;
    copy.append(name, meta);
    row.append(copy);

    if (!task.completedAt) {
      const start = document.createElement("button");
      start.className = "ghost task-start";
      start.textContent = state.focus ? "Running" : "Start";
      start.disabled = Boolean(state.focus);
      start.dataset.start = task.id;
      row.append(start);

      const remove = document.createElement("button");
      remove.className = "task-delete";
      remove.textContent = "×";
      remove.title = "Delete task";
      remove.disabled = Boolean(state.focus);
      remove.dataset.delete = task.id;
      row.append(remove);
    }
    list.append(row);
  }
}

function renderTaskTimer(state) {
  const focus = state.focus;
  const panel = $("taskActive");
  if (!focus) {
    panel.hidden = true;
    return;
  }
  panel.hidden = false;

  const remaining = Math.max(0, focus.phaseEndsAt - Date.now());
  $("taskTimer").textContent = format(remaining);
  $("taskActiveTitle").textContent = focus.title;
  $("taskPhase").textContent = focus.paused
    ? "Paused"
    : focus.phase === "focus"
      ? "Focus"
      : focus.completedSessions > 0 && focus.completedSessions % focus.sessionsBeforeLong === 0
        ? "Long break"
        : "Break";
  $("taskCycle").textContent = focus.phase === "focus"
    ? `Focus session ${Math.min(focus.completedSessions + 1, focus.totalSessions)} of ${focus.totalSessions}`
    : `Break after session ${focus.completedSessions} of ${focus.totalSessions}`;

  const totalMs = focus.totalMinutes * 60 * 1000;
  let doneMs = totalMs - focus.remainingFocusSeconds * 1000;
  if (focus.phase === "focus") {
    doneMs += Math.min(focus.sessionSeconds * 1000, Math.max(0, Date.now() - focus.phaseStartedAt));
  }
  const percent = totalMs > 0 ? Math.min(100, Math.max(0, (doneMs / totalMs) * 100)) : 0;
  $("taskProgress").style.width = `${percent}%`;
  $("taskProgressText").textContent = `${Math.round(percent)}% of ${focus.totalMinutes} focus minutes`;
}

function apply(state) {
  if (!state) return;
  $("version").textContent = state.version;

  const snapshot = JSON.stringify({ config: state.config, stats: state.stats, paused: state.paused, counts: state.counts });
  if (snapshot !== lastSnapshot) {
    lastSnapshot = snapshot;
    applyConfig(state);
    renderStats(state);
    renderMix(state);
  }

  const tasks = JSON.stringify({
    tasks: state.tasks,
    focus: state.focus && {
      id: state.focus.taskId,
      phase: state.focus.phase,
      paused: state.focus.paused,
      completed: state.focus.completedSessions,
    },
  });
  if (tasks !== lastTasks) {
    lastTasks = tasks;
    renderTaskList(state);
  }
  renderTaskTimer(state);
  renderHero(state);
}

async function refresh() {
  try {
    apply(await api.state());
  } catch (error) {
    console.error(error);
  }
}

async function save(patch) {
  try {
    const state = await api.updateConfig(patch);
    apply(state);
    lastSnapshot = "";
    showSaved();
  } catch (error) {
    console.error(error);
    toast("Couldn't save that setting.");
  }
}

// ---- Events ----
$("interval").addEventListener("change", (e) => save({ intervalMinutes: Number(e.target.value) }));
$("snooze").addEventListener("change", (e) => save({ snoozeMinutes: Number(e.target.value) }));
$("position").addEventListener("change", (e) => save({ position: e.target.value }));

$("quietEnabled").addEventListener("change", async (e) => {
  $("quietStart").disabled = !e.target.checked;
  $("quietEnd").disabled = !e.target.checked;
  await save({ quietHours: { enabled: e.target.checked } });
});
$("quietStart").addEventListener("change", (e) => save({ quietHours: { start: e.target.value } }));
$("quietEnd").addEventListener("change", (e) => save({ quietHours: { end: e.target.value } }));

$("resetWhenAway").addEventListener("change", (e) => save({ resetWhenAway: e.target.checked }));
$("sound").addEventListener("change", (e) => save({ sound: e.target.checked }));
$("expandSteps").addEventListener("change", (e) => save({ expandSteps: e.target.checked }));
$("launchAtLogin").addEventListener("change", (e) => save({ launchAtLogin: e.target.checked }));

$("mix").addEventListener("change", (e) => {
  const cat = e.target.dataset && e.target.dataset.cat;
  if (cat) save({ categories: { [cat]: e.target.checked } });
});

$("paused").addEventListener("change", async () => {
  await api.togglePause();
  lastSnapshot = "";
  await refresh();
  toast($("paused").checked ? "Paused." : "Back on.");
});

$("takeBreak").addEventListener("click", async () => {
  await api.takeBreak();
  toast("Break card on its way.");
});

$("openFirst").addEventListener("click", () => $("takeBreak").click());

let resetArmed = false;
let resetTimer = 0;
$("resetStats").addEventListener("click", async () => {
  if (!resetArmed) {
    resetArmed = true;
    $("resetStats").textContent = "Confirm reset";
    clearTimeout(resetTimer);
    resetTimer = setTimeout(() => {
      resetArmed = false;
      $("resetStats").textContent = "Reset stats";
    }, 4000);
    return;
  }
  clearTimeout(resetTimer);
  resetArmed = false;
  $("resetStats").textContent = "Reset stats";
  await api.resetStats();
  lastSnapshot = "";
  await refresh();
  toast("Stats reset.");
});

$("quit").addEventListener("click", () => api.quit());

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") api.closeDashboard();
});

$("taskForm").addEventListener("submit", async (event) => {
  event.preventDefault();
  const title = $("taskTitle").value.trim();
  const minutes = Number($("taskMinutes").value) || 50;
  if (!title) return;
  try {
    const state = await api.addTask(title, minutes);
    apply(state);
    lastTasks = "";
    $("taskForm").reset();
    $("taskMinutes").value = "50";
    toast("Task added. Hit Start when you're ready.");
  } catch (error) {
    console.error(error);
    toast("Couldn't add that task.");
  }
});

$("taskList").addEventListener("click", async (event) => {
  const start = event.target.closest("[data-start]");
  if (start) {
    try {
      const state = await api.startTask(start.dataset.start);
      apply(state);
      lastTasks = "";
      toast("Pomodoro started. An alarm will sound when it's break time.");
    } catch (error) {
      console.error(error);
      toast("Couldn't start that task.");
    }
    return;
  }
  const remove = event.target.closest("[data-delete]");
  if (remove) {
    try {
      const state = await api.deleteTask(remove.dataset.delete);
      apply(state);
      lastTasks = "";
      toast("Task removed.");
    } catch (error) {
      console.error(error);
      toast("Couldn't remove that task.");
    }
  }
});

$("stopTask").addEventListener("click", async () => {
  try {
    const state = await api.stopTask();
    apply(state);
    lastTasks = "";
    lastSnapshot = "";
    toast("Task ended. Back to regular breaks.");
  } catch (error) {
    console.error(error);
    toast("Couldn't stop that task.");
  }
});

refresh();
setInterval(refresh, 1000);
if (api.onState) api.onState(() => { lastSnapshot = ""; refresh(); });
