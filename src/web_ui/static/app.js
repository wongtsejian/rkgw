// === Auth ===

function initAuth() {
  if (!getApiKey()) {
    var key = prompt('Enter your Kiro Gateway API key:');
    if (key) sessionStorage.setItem('apiKey', key);
  }
}

function getApiKey() {
  return sessionStorage.getItem('apiKey');
}

function authHeaders() {
  return { 'Authorization': 'Bearer ' + getApiKey() };
}

// === SSE (Dashboard) ===

var sparklineData = [];
var metricsSource = null;
var logsSource = null;

function connectMetricsSSE() {
  if (metricsSource) metricsSource.close();
  metricsSource = new EventSource('/_ui/api/metrics/stream?api_key=' + encodeURIComponent(getApiKey()));
  setConnectionStatus(true);

  metricsSource.addEventListener('metrics', function(e) {
    var data = JSON.parse(e.data);
    updateDashboard(data);
  });

  metricsSource.onerror = function() {
    setConnectionStatus(false);
    metricsSource.close();
    setTimeout(connectMetricsSSE, 3000);
  };
}

function connectLogsSSE() {
  if (logsSource) logsSource.close();
  logsSource = new EventSource('/_ui/api/logs/stream?api_key=' + encodeURIComponent(getApiKey()));

  logsSource.addEventListener('log', function(e) {
    var entry = JSON.parse(e.data);
    appendLog(entry);
  });

  logsSource.onerror = function() {
    logsSource.close();
    setTimeout(connectLogsSSE, 3000);
  };
}

function setConnectionStatus(connected) {
  var dot = document.getElementById('status-dot');
  var text = document.getElementById('status-text');
  if (!dot) return;
  dot.className = 'status-dot ' + (connected ? 'connected' : 'disconnected');
  if (text) text.textContent = connected ? 'Connected' : 'Disconnected';
}

// === Dashboard Updates ===

function updateDashboard(data) {
  if (data.active_connections !== undefined) updateGauge('connections', data.active_connections, data.max_connections || 100);
  if (data.cpu_percent !== undefined) updateGauge('cpu', data.cpu_percent, 100);
  if (data.memory_mb !== undefined) updateGauge('memory', data.memory_mb, data.max_memory_mb || 1024);

  if (data.request_rate !== undefined) {
    sparklineData.push(data.request_rate);
    if (sparklineData.length > 60) sparklineData.shift();
    drawSparkline('sparkline', sparklineData);
  }

  if (data.latency) {
    setText('latency-p50', (data.latency.p50 || 0) + ' ms');
    setText('latency-p95', (data.latency.p95 || 0) + ' ms');
    setText('latency-p99', (data.latency.p99 || 0) + ' ms');
  }

  if (data.models) updateModelTable(data.models);
  if (data.errors) updateErrors(data.errors);
}

function updateGauge(id, value, max) {
  var bar = document.getElementById('gauge-bar-' + id);
  var val = document.getElementById('gauge-value-' + id);
  if (!bar || !val) return;
  var pct = Math.min(100, (value / max) * 100);
  bar.style.width = pct + '%';
  val.textContent = typeof value === 'number' && value % 1 !== 0 ? value.toFixed(1) : value;
}

function drawSparkline(canvasId, dataPoints) {
  var canvas = document.getElementById(canvasId);
  if (!canvas || dataPoints.length < 2) return;
  var ctx = canvas.getContext('2d');
  var w = canvas.width = canvas.offsetWidth;
  var h = canvas.height = canvas.offsetHeight;
  ctx.clearRect(0, 0, w, h);

  var max = Math.max.apply(null, dataPoints) || 1;
  var step = w / (dataPoints.length - 1);

  ctx.beginPath();
  for (var i = 0; i < dataPoints.length; i++) {
    var x = i * step;
    var y = h - (dataPoints[i] / max) * (h - 10) - 5;
    if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
  }
  ctx.strokeStyle = '#4a9eff';
  ctx.lineWidth = 2;
  ctx.stroke();

  // Gradient fill
  ctx.lineTo((dataPoints.length - 1) * step, h);
  ctx.lineTo(0, h);
  ctx.closePath();
  var grad = ctx.createLinearGradient(0, 0, 0, h);
  grad.addColorStop(0, 'rgba(74,158,255,0.3)');
  grad.addColorStop(1, 'rgba(74,158,255,0)');
  ctx.fillStyle = grad;
  ctx.fill();
}

function updateModelTable(models) {
  var tbody = document.getElementById('model-tbody');
  if (!tbody) return;
  tbody.innerHTML = '';
  for (var i = 0; i < models.length; i++) {
    var m = models[i];
    var tr = document.createElement('tr');
    tr.innerHTML = '<td>' + esc(m.name) + '</td><td>' + (m.requests || 0) +
      '</td><td>' + (m.avg_latency_ms || 0) + '</td><td>' + (m.input_tokens || 0) +
      '</td><td>' + (m.output_tokens || 0) + '</td>';
    tbody.appendChild(tr);
  }
}

function updateErrors(errors) {
  var container = document.getElementById('errors-container');
  if (!container) return;
  container.innerHTML = '';
  var keys = Object.keys(errors);
  for (var i = 0; i < keys.length; i++) {
    var div = document.createElement('div');
    div.className = 'error-item';
    div.innerHTML = '<div class="error-count">' + errors[keys[i]] + '</div><div class="error-label">' + esc(keys[i]) + '</div>';
    container.appendChild(div);
  }
}

// === Log Viewer ===

var autoScroll = true;

function appendLog(entry) {
  var container = document.getElementById('log-container');
  if (!container) return;
  var div = document.createElement('div');
  var level = entry.level || 'INFO';
  div.className = 'log-entry log-' + level;
  div.setAttribute('data-level', level);
  div.textContent = (entry.timestamp || '') + ' [' + level + '] ' + (entry.message || '');
  container.appendChild(div);
  if (container.children.length > 500) container.removeChild(container.firstChild);
  if (autoScroll) container.scrollTop = container.scrollHeight;
}

function filterLogs(level) {
  var entries = document.querySelectorAll('.log-entry');
  for (var i = 0; i < entries.length; i++) {
    if (!level || level === 'ALL') {
      entries[i].style.display = '';
    } else {
      entries[i].style.display = entries[i].getAttribute('data-level') === level ? '' : 'none';
    }
  }
  // Update button states
  var btns = document.querySelectorAll('.log-filter-btn');
  for (var j = 0; j < btns.length; j++) {
    btns[j].className = 'btn btn-sm log-filter-btn ' + (btns[j].getAttribute('data-level') === (level || 'ALL') ? 'btn-active' : 'btn-inactive');
  }
}

function searchLogs(query) {
  var entries = document.querySelectorAll('.log-entry');
  var q = query.toLowerCase();
  for (var i = 0; i < entries.length; i++) {
    entries[i].style.display = (!q || entries[i].textContent.toLowerCase().indexOf(q) !== -1) ? '' : 'none';
  }
}

function toggleAutoScroll() {
  autoScroll = !autoScroll;
  var btn = document.getElementById('autoscroll-btn');
  if (btn) btn.textContent = 'Auto-scroll: ' + (autoScroll ? 'ON' : 'OFF');
}

// === Config Page ===

function loadConfig() {
  fetch('/_ui/api/config', { headers: authHeaders() })
    .then(function(r) { return r.json(); })
    .then(function(cfg) {
      var fields = document.querySelectorAll('[data-config]');
      for (var i = 0; i < fields.length; i++) {
        var key = fields[i].getAttribute('data-config');
        if (cfg[key] === undefined) continue;
        if (fields[i].type === 'checkbox') fields[i].checked = !!cfg[key];
        else fields[i].value = cfg[key];
      }
    })
    .catch(function(err) { showToast('Failed to load config: ' + err.message, 'error'); });
}

function saveConfig(event) {
  event.preventDefault();
  var fields = document.querySelectorAll('[data-config]');
  var cfg = {};
  for (var i = 0; i < fields.length; i++) {
    var key = fields[i].getAttribute('data-config');
    if (fields[i].type === 'checkbox') cfg[key] = fields[i].checked;
    else if (fields[i].type === 'number') cfg[key] = Number(fields[i].value);
    else cfg[key] = fields[i].value;
  }
  fetch('/_ui/api/config', {
    method: 'PUT',
    headers: Object.assign({ 'Content-Type': 'application/json' }, authHeaders()),
    body: JSON.stringify(cfg)
  })
    .then(function(r) {
      if (!r.ok) throw new Error('HTTP ' + r.status);
      return r.json();
    })
    .then(function() {
      showToast('Configuration saved', 'success');
      loadConfigHistory();
    })
    .catch(function(err) { showToast('Failed to save: ' + err.message, 'error'); });
}

function loadConfigHistory() {
  fetch('/_ui/api/config/history', { headers: authHeaders() })
    .then(function(r) { return r.json(); })
    .then(function(history) {
      var tbody = document.getElementById('history-tbody');
      if (!tbody) return;
      tbody.innerHTML = '';
      for (var i = 0; i < history.length; i++) {
        var h = history[i];
        var tr = document.createElement('tr');
        tr.innerHTML = '<td>' + esc(h.timestamp || '') + '</td><td>' + esc(h.field || '') +
          '</td><td>' + esc(String(h.old_value || '')) + '</td><td>' + esc(String(h.new_value || '')) + '</td>';
        tbody.appendChild(tr);
      }
    })
    .catch(function() {});
}

function showToast(message, type) {
  var container = document.getElementById('toast-container');
  if (!container) return;
  var toast = document.createElement('div');
  toast.className = 'toast toast-' + (type || 'success');
  toast.textContent = message;
  container.appendChild(toast);
  setTimeout(function() { toast.remove(); }, 3000);
}

// === Helpers ===

function setText(id, text) {
  var el = document.getElementById(id);
  if (el) el.textContent = text;
}

function esc(s) {
  var d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}

function togglePasswordVisibility(inputId) {
  var input = document.getElementById(inputId);
  if (!input) return;
  input.type = input.type === 'password' ? 'text' : 'password';
}

// === Init ===

document.addEventListener('DOMContentLoaded', function() {
  initAuth();
  if (document.getElementById('dashboard')) {
    connectMetricsSSE();
    connectLogsSSE();
  }
  if (document.getElementById('config-form')) {
    loadConfig();
    loadConfigHistory();
  }
});
