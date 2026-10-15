// The Creature panel: live status of the ScreenBuddy desktop companion.
// Polls GET /api/screenbuddy/status (the extension's own read route) and
// draws into #creature-root. ABP's extensions.js adds the nav entry and
// loads this script; it reloads on its own hashchange.
(function () {
  "use strict";

  var root = document.getElementById("creature-root");
  if (!root) return;

  function render(status) {
    if (!status || !status.ok) {
      root.innerHTML =
        '<div class="panel-empty">ScreenBuddy is not running.<br>' +
        "<small>" + ((status && status.error) || "start the desktop app") + "</small></div>";
      return;
    }
    var d = status.data || {};
    function row(label, value) {
      return "<div class='creature-row'><span>" + label + "</span><b>" + value + "</b></div>";
    }
    root.innerHTML =
      "<div class='creature-grid'>" +
      row("fps", d.fps != null ? Number(d.fps).toFixed(1) : "?") +
      row("uptime", d.uptime_secs != null ? Math.round(d.uptime_secs) + "s" : "?") +
      row("creatures", d.creature_count != null ? d.creature_count : "?") +
      row("models", d.model_count != null ? d.model_count : "?") +
      row("tools", d.tool_count != null ? d.tool_count : "?") +
      row("memory chunks", d.memory_chunks != null ? d.memory_chunks : "?") +
      "</div>" +
      "<div class='creature-note'>the desktop companion answering your bots</div>";
  }

  function poll() {
    fetch("/api/screenbuddy/status", { cache: "no-store" })
      .then(function (r) { return r.json(); })
      .then(render)
      .catch(function () { render({ ok: false, error: "dashboard unreachable" }); });
  }

  poll();
  setInterval(poll, 5000);
})();
