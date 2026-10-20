(function () {
  "use strict";

  const tauri = window.__TAURI__;
  if (!tauri) return;

  const invoke = tauri.core.invoke;
  const breakHandlers = new Set();
  const stateHandlers = new Set();

  tauri.event.listen("break:show", (event) => {
    for (const handler of breakHandlers) handler(event.payload);
  });
  tauri.event.listen("state:changed", (event) => {
    for (const handler of stateHandlers) handler(event.payload);
  });

  window.noodle = {
    state: () => invoke("app_state"),
    updateConfig: (patch) => invoke("update_config", { patch }),
    action: (action, data) => invoke("card_action", { action, seconds: data && data.seconds }),
    resize: (height) => invoke("resize_card", { height }),
    takeBreak: () => invoke("take_break"),
    togglePause: () => invoke("toggle_pause"),
    addTask: (title, minutes) => invoke("add_task", { title, minutes }),
    startTask: (id) => invoke("start_task", { id }),
    stopTask: () => invoke("stop_task"),
    deleteTask: (id) => invoke("delete_task", { id }),
    openDashboard: () => invoke("open_dashboard"),
    closeDashboard: () => invoke("close_dashboard"),
    resetStats: () => invoke("reset_stats"),
    quit: () => invoke("quit_app"),
    onBreak: (handler) => breakHandlers.add(handler),
    onState: (handler) => stateHandlers.add(handler),
  };
})();
