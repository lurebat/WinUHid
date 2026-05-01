// WinUHid web UI — vanilla JS, no build step.
//
// Talks to the Rust backend via REST for commands and WebSocket
// (per-device subscription) for live events from the host OS.

const $  = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));

const TOKEN_KEY = 'winuhid_token';
const ACTIVE_DEVICE_KEY = 'winuhid_active_device';

// If the page was opened with `#token=…`, stash the token in
// sessionStorage and strip it from the URL so it doesn't end up in
// browser history / bookmarks. The token is never logged.
(function captureTokenFromHash() {
  const h = location.hash || '';
  if (!h.startsWith('#')) return;
  const params = new URLSearchParams(h.slice(1));
  const tok = params.get('token');
  if (!tok) return;
  try { sessionStorage.setItem(TOKEN_KEY, tok); } catch (_) {}
  params.delete('token');
  const remaining = params.toString();
  const newHash = remaining ? '#' + remaining : '';
  history.replaceState(null, '', location.pathname + location.search + newHash);
})();

function getToken() {
  try { return sessionStorage.getItem(TOKEN_KEY); } catch (_) { return null; }
}

const state = {
  devices: [],
  activeId: null,
  ws: null,
  wsDeviceId: null,    // device id the current ws / reconnect timer belongs to
  wsBackoffMs: 1000,   // current reconnect backoff
  wsTimer: null,       // pending reconnect setTimeout id
  wsStatus: 'idle',    // 'live' | 'reconnecting' | 'dead' | 'idle'
  feedback: {},   // deviceId -> rumble/led/trigger state for the inspector
  controllerState: {}, // deviceId -> last controller form state
};

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------

window.addEventListener('DOMContentLoaded', async () => {
  wireMainTabs();
  wireTabs();
  wireCreateButtons();
  wireDocs();
  await refreshHealth();
  await refreshDevices();
  setInterval(refreshDevices, 5000);
});

function wireMainTabs() {
  $$('#main-tabs button').forEach(btn => {
    btn.addEventListener('click', () => {
      $$('#main-tabs button').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      const view = btn.dataset.view;
      $$('.view-body').forEach(el => el.classList.add('hidden'));
      $('#view-' + view).classList.remove('hidden');
    });
  });
}

function wireTabs() {
  $$('#create-tabs button').forEach(btn => {
    btn.addEventListener('click', () => {
      $$('#create-tabs button').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      const tab = btn.dataset.tab;
      $$('.create-tab-body').forEach(el => el.classList.add('hidden'));
      $('#tab-' + tab).classList.remove('hidden');
    });
  });
}

function wireCreateButtons() {
  $('#create-mouse').addEventListener('click', () => createPreset('mouse', $('#mouse-name').value));
  $('#create-ps4').addEventListener('click', () => createPreset('ps4', $('#ps4-name').value));
  $('#create-ps5').addEventListener('click', () => createPreset('ps5', $('#ps5-name').value));
  $('#create-xone').addEventListener('click', () => createPreset('xone', $('#xone-name').value));
  $('#create-generic').addEventListener('click', createGeneric);
  wireGenericPresets();
}

function wireDocs() {
  highlightCodeSamples();
  $$('.copy-code').forEach(btn => {
    btn.addEventListener('click', async () => {
      const code = btn.closest('.code-sample')?.querySelector('code')?.innerText || '';
      if (!code) return;
      try {
        await copyText(code);
        const oldText = btn.textContent;
        btn.textContent = 'Copied';
        setTimeout(() => { btn.textContent = oldText; }, 1200);
      } catch (e) {
        showErr(e);
      }
    });
  });
}

function highlightCodeSamples() {
  $$('.code-sample code.language-c').forEach(code => {
    code.innerHTML = highlightC(code.textContent);
  });
}

function highlightC(source) {
  const keywords = new Set([
    'APP_STATE',
    'BOOL', 'FALSE', 'NULL', 'PCWINUHID_PS5_TRIGGER_EFFECT', 'PWINUHID_PS5_GAMEPAD',
    'PVOID', 'TRUE', 'UCHAR', 'UINT', 'VOID', 'WINUHID_PRESET_DEVICE_INFO',
    'WINUHID_PS5_GAMEPAD_INFO', 'WINUHID_PS5_INPUT_REPORT', 'char', 'const', 'for',
    'if', 'return', 'sizeof', 'static', 'struct', 'typedef', 'void',
  ]);
  const escapeHtml = value => value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
  const wrap = (cls, value) => `<span class="tok-${cls}">${escapeHtml(value)}</span>`;
  let out = '';
  let i = 0;
  while (i < source.length) {
    const ch = source[i];
    const prev = i === 0 ? '\n' : source[i - 1];
    if (ch === '#' && prev === '\n') {
      const end = source.indexOf('\n', i);
      const next = end === -1 ? source.length : end;
      out += wrap('preproc', source.slice(i, next));
      i = next;
    } else if (source.startsWith('//', i)) {
      const end = source.indexOf('\n', i);
      const next = end === -1 ? source.length : end;
      out += wrap('comment', source.slice(i, next));
      i = next;
    } else if (source.startsWith('/*', i)) {
      const end = source.indexOf('*/', i + 2);
      const next = end === -1 ? source.length : end + 2;
      out += wrap('comment', source.slice(i, next));
      i = next;
    } else if (ch === '"' || ch === "'") {
      let next = i + 1;
      while (next < source.length) {
        if (source[next] === '\\') {
          next += 2;
        } else if (source[next] === ch) {
          next += 1;
          break;
        } else {
          next += 1;
        }
      }
      out += wrap('string', source.slice(i, next));
      i = next;
    } else if (/\d/.test(ch)) {
      const match = source.slice(i).match(/^0x[0-9a-fA-F]+|^\d+(?:\.\d+)?f?/);
      out += wrap('number', match[0]);
      i += match[0].length;
    } else if (/[A-Za-z_]/.test(ch)) {
      const match = source.slice(i).match(/^[A-Za-z_][A-Za-z0-9_]*/)[0];
      out += keywords.has(match) ? wrap('keyword', match) : escapeHtml(match);
      i += match.length;
    } else {
      out += escapeHtml(ch);
      i += 1;
    }
  }
  return out;
}

async function copyText(text) {
  if (navigator.clipboard && window.isSecureContext) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const ta = document.createElement('textarea');
  ta.value = text;
  ta.setAttribute('readonly', '');
  ta.style.position = 'fixed';
  ta.style.opacity = '0';
  document.body.appendChild(ta);
  ta.select();
  try {
    if (!document.execCommand('copy')) throw new Error('copy failed');
  } finally {
    document.body.removeChild(ta);
  }
}

// ---------------------------------------------------------------------------
// Generic HID descriptor presets
// ---------------------------------------------------------------------------

const GENERIC_PRESETS = {
  'boot-keyboard': {
    name: 'Boot keyboard',
    vid: '046d', pid: '0001', ver: '0110',
    desc: `
      05 01 09 06 a1 01 05 07 19 e0 29 e7 15 00 25 01
      75 01 95 08 81 02 95 01 75 08 81 01 95 05 75 01
      05 08 19 01 29 05 91 02 95 01 75 03 91 01 95 06
      75 08 15 00 25 65 05 07 19 00 29 65 81 00 c0
    `,
  },
  'joystick': {
    name: 'Simple joystick',
    vid: 'cafe', pid: 'b001', ver: '0100',
    desc: `
      05 01 09 04 a1 01 a1 00 09 30 09 31 15 00 26 ff
      00 75 08 95 02 81 02 c0 05 09 19 01 29 04 15 00
      25 01 75 01 95 04 81 02 75 04 95 01 81 03 c0
    `,
  },
  'paddle': {
    name: 'Two-axis paddle',
    vid: 'cafe', pid: 'b002', ver: '0100',
    desc: `
      05 01 09 05 a1 01 a1 00 09 30 15 00 26 ff 00 75
      08 95 01 81 02 c0 05 09 19 01 29 02 15 00 25 01
      75 01 95 02 81 02 75 06 95 01 81 03 c0
    `,
  },
  'vendor-io': {
    name: 'Vendor-defined I/O',
    vid: 'cafe', pid: 'f000', ver: '0100',
    desc: `
      06 00 ff 09 01 a1 01 09 02 15 00 26 ff 00 75 08
      95 10 81 02 09 03 95 10 91 02 c0
    `,
  },
};

function wireGenericPresets() {
  const sel = document.getElementById('gen-preset');
  if (!sel) return;
  sel.addEventListener('change', () => {
    const key = sel.value;
    if (key === 'custom') return;
    const p = GENERIC_PRESETS[key];
    if (!p) return;
    $('#gen-name').value = p.name;
    $('#gen-vid').value = p.vid;
    $('#gen-pid').value = p.pid;
    $('#gen-ver').value = p.ver;
    $('#gen-desc').value = p.desc.trim().replace(/\s+/g, ' ');
  });
}

// ---------------------------------------------------------------------------
// REST helpers
// ---------------------------------------------------------------------------

async function api(method, path, body) {
  const opts = { method, headers: { 'Content-Type': 'application/json' } };
  const tok = getToken();
  if (tok) opts.headers['Authorization'] = `Bearer ${tok}`;
  if (body !== undefined) opts.body = JSON.stringify(body);
  const resp = await fetch(path, opts);
  let payload = null;
  try { payload = await resp.json(); } catch (_) { /* might be empty */ }
  if (!resp.ok) throw new Error((payload && payload.error) || `HTTP ${resp.status}`);
  return payload;
}

async function refreshHealth() {
  const el = $('#health');
  try {
    const h = await api('GET', '/api/health');
    if (h.driver_version === 0) {
      const err = h.driver_last_error == null ? '' : ` · Win32 0x${hex(h.driver_last_error, 8)}`;
      el.textContent = 'driver: control device unavailable' + err + ' · devs: ' + (h.devs_available ? 'ok' : 'unavailable');
      el.classList.remove('good'); el.classList.add('bad');
    } else {
      el.textContent = `driver: v${h.driver_version} · devs: ${h.devs_available ? 'ok' : 'unavailable'}`;
      el.classList.remove('bad'); el.classList.add(h.devs_available ? 'good' : 'warn');
      if (!h.devs_available) {
        ['#create-mouse', '#create-ps4', '#create-ps5', '#create-xone'].forEach(s => {
          const b = $(s); if (b) b.disabled = true;
        });
        ['mouse', 'ps4', 'ps5', 'xone'].forEach(t => {
          const tb = $('#tab-' + t);
          if (tb) tb.insertAdjacentHTML('beforeend', '<p class="muted">⚠ WinUHidDevs.dll not loaded — preset unavailable.</p>');
        });
      }
    }
  } catch (e) {
    el.textContent = 'unreachable: ' + e.message;
    el.classList.add('bad');
  }
}

async function refreshDevices() {
  try {
    const list = await api('GET', '/api/devices');
    state.devices = list;
    if (state.activeId && !list.some(d => d.id === state.activeId)) {
      // The active device was destroyed out from under us.
      cancelReconnect();
      if (state.ws) { try { state.ws.close(); } catch (_) {} state.ws = null; }
      setWsStatus('dead');
      state.activeId = null;
      try { sessionStorage.removeItem(ACTIVE_DEVICE_KEY); } catch (_) {}
    }
    if (!state.activeId && list.length) {
      let saved = null;
      try { saved = sessionStorage.getItem(ACTIVE_DEVICE_KEY); } catch (_) {}
      const restored = list.find(d => d.id === saved) || list[0];
      selectDevice(restored.id);
      return;
    }
    renderDeviceList();
  } catch (e) {
    console.warn(e);
  }
}

function renderDeviceList() {
  const ul = $('#device-list');
  if (!state.devices.length) {
    ul.innerHTML = '<li class="muted" style="cursor:default">no devices yet — create one above</li>';
    return;
  }
  ul.innerHTML = '';
  for (const d of state.devices) {
    const li = document.createElement('li');
    if (d.id === state.activeId) li.classList.add('active');
    li.innerHTML = `
      <div>
        <strong>${escape(d.name)}</strong>
        <div class="meta">${d.kind} · VID 0x${hex(d.vendor_id, 4)} · PID 0x${hex(d.product_id, 4)} · ${d.id.slice(0, 8)}</div>
      </div>
      <button class="danger" data-act="destroy" data-id="${d.id}">destroy</button>
    `;
    li.addEventListener('click', e => {
      if (e.target.dataset.act === 'destroy') return;
      selectDevice(d.id);
    });
    li.querySelector('.danger').addEventListener('click', async e => {
      e.stopPropagation();
      try {
        await api('DELETE', '/api/devices/' + d.id);
      } catch (err) {
        toast('Failed to destroy device: ' + err.message, 'error');
        return;
      }
      if (state.activeId === d.id) {
        state.activeId = null;
        try { sessionStorage.removeItem(ACTIVE_DEVICE_KEY); } catch (_) {}
        cancelReconnect();
        if (state.ws) { try { state.ws.close(); } catch (_) {} state.ws = null; }
      }
      await refreshDevices();
      renderDebug();
    });
    ul.appendChild(li);
  }
}

function selectDevice(id) {
  state.activeId = id;
  try { sessionStorage.setItem(ACTIVE_DEVICE_KEY, id); } catch (_) {}
  cancelReconnect();
  if (state.ws) { try { state.ws.close(); } catch (_) {} state.ws = null; }
  state.wsDeviceId = id;
  state.wsBackoffMs = 1000;
  setWsStatus('reconnecting');
  renderDeviceList();
  renderDebug();
  openSocket(id);
}

function cancelReconnect() {
  if (state.wsTimer != null) {
    clearTimeout(state.wsTimer);
    state.wsTimer = null;
  }
}

function setWsStatus(status) {
  state.wsStatus = status;
  const el = document.getElementById('ws-status');
  if (!el) return;
  el.classList.remove('live', 'reconnecting', 'dead');
  switch (status) {
    case 'live':
      el.classList.add('live');
      el.textContent = '🟢 live';
      break;
    case 'reconnecting':
      el.classList.add('reconnecting');
      el.textContent = '🟠 reconnecting…';
      break;
    case 'dead':
      el.classList.add('dead');
      el.textContent = '🔴 disconnected (device gone)';
      break;
    default:
      el.textContent = '';
  }
}

function openSocket(id) {
  // Bail if the user has switched away in between.
  if (state.activeId !== id) return;
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  const tok = getToken();
  const qs = tok ? `?token=${encodeURIComponent(tok)}` : '';
  const url = `${proto}://${location.host}/api/devices/${id}/events${qs}`;
  let ws;
  try {
    ws = new WebSocket(url);
  } catch (e) {
    scheduleReconnect(id);
    return;
  }
  state.ws = ws;
  state.wsDeviceId = id;
  ws.addEventListener('open', () => {
    if (state.ws !== ws) return;
    state.wsBackoffMs = 1000;
    setWsStatus('live');
  });
  ws.addEventListener('message', m => {
    try {
      const ev = JSON.parse(m.data);
      pushEvent(id, ev);
    } catch (_) {}
  });
  ws.addEventListener('close', () => {
    if (state.ws === ws) state.ws = null;
    if (state.activeId !== id) return;
    // If the device no longer exists in the latest list, stop retrying.
    if (!state.devices.some(d => d.id === id)) {
      setWsStatus('dead');
      return;
    }
    setWsStatus('reconnecting');
    scheduleReconnect(id);
  });
  ws.addEventListener('error', () => {
    // onclose will follow; nothing else to do here.
  });
}

function scheduleReconnect(id) {
  cancelReconnect();
  if (state.activeId !== id) return;
  const delay = state.wsBackoffMs;
  state.wsBackoffMs = Math.min(state.wsBackoffMs * 2, 15000);
  state.wsTimer = setTimeout(() => {
    state.wsTimer = null;
    if (state.activeId !== id) return;
    openSocket(id);
  }, delay);
}

// ---------------------------------------------------------------------------
// Create handlers
// ---------------------------------------------------------------------------

async function createPreset(kind, name) {
  const body = name ? { name } : {};
  try {
    const dev = await api('POST', '/api/devices/' + kind, body);
    toast(`Created ${kind} device`, 'success');
    await refreshDevices();
    selectDevice(dev.id);
  } catch (e) {
    toast('Failed to create device: ' + e.message, 'error');
  }
}

async function createGeneric() {
  const desc = $('#gen-desc').value.replace(/\s+/g, '');
  if (!desc) { toast('Provide a HID report descriptor in hex', 'error'); return; }
  const body = {
    name: $('#gen-name').value || null,
    vendor_id: parseInt($('#gen-vid').value, 16) || 0,
    product_id: parseInt($('#gen-pid').value, 16) || 0,
    version: parseInt($('#gen-ver').value, 16) || 0,
    report_descriptor_hex: desc,
    enable_read_events: $('#gen-read').checked,
  };
  try {
    const dev = await api('POST', '/api/devices/generic', body);
    toast('Created generic device', 'success');
    await refreshDevices();
    selectDevice(dev.id);
  } catch (e) {
    toast('Failed to create device: ' + e.message, 'error');
  }
}

// ---------------------------------------------------------------------------
// Debug pane
// ---------------------------------------------------------------------------

function renderDebug() {
  const body = $('#debug-body');
  if (!state.activeId) {
    body.innerHTML = '<p class="muted">Select a device above to inspect it.</p>';
    return;
  }
  const dev = state.devices.find(d => d.id === state.activeId);
  if (!dev) {
    body.innerHTML = '<p class="muted">Device disappeared.</p>';
    return;
  }

  body.innerHTML = `
    <div class="debug-grid">
      <div class="col">
        <h3>Drive ${dev.kind} ${escape(dev.name)}<span id="ws-status" class="ws-status"></span></h3>
        <div id="device-controls"></div>
      </div>
      <div class="col">
        <h3>Live events from the OS</h3>
        <div id="feedback-summary"></div>
        <div class="event-log" id="event-log"></div>
      </div>
    </div>
  `;

  setWsStatus(state.wsStatus);

  const controls = $('#device-controls');
  switch (dev.kind) {
    case 'mouse':   renderMouseControls(controls, dev); break;
    case 'ps4':     renderGamepadControls(controls, dev, 'ps4'); break;
    case 'ps5':     renderGamepadControls(controls, dev, 'ps5'); break;
    case 'xone':    renderGamepadControls(controls, dev, 'xone'); break;
    case 'generic': renderGenericControls(controls, dev); break;
  }
  renderFeedback(dev);
}

// ----- Mouse ---------------------------------------------------------------

function renderMouseControls(root, dev) {
  root.innerHTML = `
    <div class="trackpad" id="m-trackpad">
      <span class="hint">drag here to move the cursor</span>
    </div>
    <div class="grid">
      <label>dx <input id="m-dx" type="number" value="0" /></label>
      <label>dy <input id="m-dy" type="number" value="0" /></label>
    </div>
    <button class="primary" id="m-move">Submit motion</button>
    <h4 style="margin-top:14px">Buttons</h4>
    <div class="btn-grid" id="m-btns">
      ${['Left:1','Right:2','Middle:3','X1:4','X2:5'].map(s => {
        const [n,i] = s.split(':');
        return `<button data-button="${i}">${n}</button>`;
      }).join('')}
    </div>
    <h4 style="margin-top:14px">Scroll</h4>
    <div class="grid">
      <label>delta (1/120 detents) <input id="m-scroll" type="number" value="120" /></label>
      <label><input type="checkbox" id="m-scroll-h"> horizontal</label>
    </div>
    <button class="primary" id="m-scroll-go">Scroll</button>
  `;

  $('#m-move', root).addEventListener('click', () =>
    api('POST', `/api/devices/${dev.id}/mouse/motion`, {
      dx: parseInt($('#m-dx').value, 10) || 0,
      dy: parseInt($('#m-dy').value, 10) || 0,
    }).catch(showErr));

  $('#m-btns', root).addEventListener('mousedown', e => mouseBtn(e, dev, true));
  $('#m-btns', root).addEventListener('mouseup',   e => mouseBtn(e, dev, false));
  $('#m-btns', root).addEventListener('mouseleave', e => {
    $$('#m-btns button.held').forEach(b => {
      b.classList.remove('held');
      api('POST', `/api/devices/${dev.id}/mouse/button`,
          { button: parseInt(b.dataset.button, 10), down: false }).catch(showErr);
    });
  });

  $('#m-scroll-go', root).addEventListener('click', () =>
    api('POST', `/api/devices/${dev.id}/mouse/scroll`, {
      value: parseInt($('#m-scroll').value, 10) || 0,
      horizontal: $('#m-scroll-h').checked,
    }).catch(showErr));

  wireMouseTrackpad($('#m-trackpad', root), dev);
}

function wireMouseTrackpad(pad, dev) {
  let dragging = false;
  let lastX = 0, lastY = 0;
  let pendingDx = 0, pendingDy = 0;
  let rafScheduled = false;

  function flush() {
    rafScheduled = false;
    if (!pendingDx && !pendingDy) return;
    const dx = pendingDx, dy = pendingDy;
    pendingDx = 0; pendingDy = 0;
    api('POST', `/api/devices/${dev.id}/mouse/motion`, { dx, dy }).catch(showErr);
  }

  pad.addEventListener('pointerdown', e => {
    dragging = true;
    pad.classList.add('active');
    pad.setPointerCapture(e.pointerId);
    lastX = e.clientX;
    lastY = e.clientY;
  });
  pad.addEventListener('pointermove', e => {
    if (!dragging) return;
    const dx = e.clientX - lastX;
    const dy = e.clientY - lastY;
    lastX = e.clientX;
    lastY = e.clientY;
    pendingDx += Math.round(dx);
    pendingDy += Math.round(dy);
    if (!rafScheduled) {
      rafScheduled = true;
      requestAnimationFrame(flush);
    }
  });
  const end = e => {
    if (!dragging) return;
    dragging = false;
    pad.classList.remove('active');
    try { pad.releasePointerCapture(e.pointerId); } catch (_) {}
    if (!rafScheduled) {
      rafScheduled = true;
      requestAnimationFrame(flush);
    }
  };
  pad.addEventListener('pointerup', end);
  pad.addEventListener('pointercancel', end);
}

function mouseBtn(e, dev, down) {
  const t = e.target.closest('button[data-button]');
  if (!t) return;
  const button = parseInt(t.dataset.button, 10);
  if (down) t.classList.add('held'); else t.classList.remove('held');
  api('POST', `/api/devices/${dev.id}/mouse/button`, { button, down }).catch(showErr);
}

// ----- Gamepad (PS4/PS5/XOne) ---------------------------------------------

const GAMEPAD_DEFS = {
  ps4: {
    sticks: { center: 0x80, max: 0xff },
    triggers: { max: 0xff },
    face: ['btn_square', 'btn_cross', 'btn_circle', 'btn_triangle'],
    shoulder: ['btn_l1', 'btn_r1', 'btn_l2', 'btn_r2'],
    misc: ['btn_share', 'btn_options', 'btn_l3', 'btn_r3', 'btn_home', 'btn_touchpad'],
  },
  ps5: {
    sticks: { center: 0x80, max: 0xff },
    triggers: { max: 0xff },
    face: ['btn_square', 'btn_cross', 'btn_circle', 'btn_triangle'],
    shoulder: ['btn_l1', 'btn_r1', 'btn_l2', 'btn_r2'],
    misc: ['btn_share', 'btn_options', 'btn_l3', 'btn_r3', 'btn_home', 'btn_touchpad', 'btn_mute'],
  },
  xone: {
    sticks: { center: 0x8000, max: 0xffff },
    triggers: { max: 1023 },
    face: ['btn_a', 'btn_b', 'btn_x', 'btn_y'],
    shoulder: ['btn_lb', 'btn_rb'],
    misc: ['btn_back', 'btn_menu', 'btn_ls', 'btn_rs', 'btn_home'],
  },
};

const TOUCHPAD_BOUNDS = {
  ps4: { x: 1919, y: 942 },
  ps5: { x: 1919, y: 1079 },
};

function defaultControllerState(kind) {
  const def = GAMEPAD_DEFS[kind];
  const s = {
    left_stick_x: def.sticks.center, left_stick_y: def.sticks.center,
    right_stick_x: def.sticks.center, right_stick_y: def.sticks.center,
    left_trigger: 0, right_trigger: 0,
    hat_x: 0, hat_y: 0,
    battery_level: 0xff,
  };
  for (const k of [...def.face, ...def.shoulder, ...def.misc]) s[k] = false;
  if (kind === 'ps4' || kind === 'ps5') {
    s.touchpad_active = false;
    s.touchpad_x = 0;
    s.touchpad_y = 0;
    s.touchpad2_active = false;
    s.touchpad2_x = 0;
    s.touchpad2_y = 0;
    s.accel_x = 0.0;
    s.accel_y = 0.0;
    s.accel_z = 0.0;
    s.gyro_x = 0.0;
    s.gyro_y = 0.0;
    s.gyro_z = 0.0;
  }
  if (kind === 'ps5') {
    s.trigger_right_status = 0;
    s.trigger_right_stop_location = 0;
    s.trigger_left_status = 0;
    s.trigger_left_stop_location = 0;
    s.trigger_right_effect = 0;
    s.trigger_left_effect = 0;
  }
  return s;
}

function renderGamepadControls(root, dev, kind) {
  const def = GAMEPAD_DEFS[kind];
  if (!state.controllerState[dev.id]) {
    state.controllerState[dev.id] = defaultControllerState(kind);
  } else {
    // Merge in any newly-introduced fields (e.g. touchpad/IMU).
    const defaults = defaultControllerState(kind);
    for (const k of Object.keys(defaults)) {
      if (!(k in state.controllerState[dev.id])) {
        state.controllerState[dev.id][k] = defaults[k];
      }
    }
  }
  const hasTouchImu = (kind === 'ps4' || kind === 'ps5');
  const hasTriggerStatus = (kind === 'ps5');
  const touchImuHtml = hasTouchImu ? `
      <div class="group" style="grid-column: 1 / -1">
        <h4>Touchpad</h4>
        <div class="touchpad" data-touchpad>
          <div class="hint">drag · Shift = 2nd touch · Ctrl = hold</div>
          <div class="dot" data-dot="1" hidden></div>
          <div class="dot dot2" data-dot="2" hidden></div>
        </div>
        <div style="display:flex; gap:8px; align-items:center; margin-top:6px; flex-wrap:wrap">
          <span class="meta" data-touch-readout>—</span>
        </div>
      </div>
      <div class="group" style="grid-column: 1 / -1">
        <h4>IMU (accelerometer / gyroscope)</h4>
        <div data-imu></div>
        <button class="danger" data-imu-reset style="margin-top:6px">reset IMU</button>
      </div>
  ` : '';
  const triggerStatusHtml = hasTriggerStatus ? `
      <div class="group" style="grid-column: 1 / -1">
        <h4>Adaptive trigger status (input report)</h4>
        <p class="meta" style="margin:0 0 8px 0">These fields go into the input report the virtual controller sends to the host.<br>They reflect the <em>physical state</em> of the adaptive triggers.</p>
        <div class="trigger-status-grid" data-trigger-status>
          <div class="imu-row">
            <span>L status</span>
            <select data-ts-key="trigger_left_status">${triggerStatusOptions()}</select>
            <span></span>
          </div>
          <div class="imu-row">
            <span>L stop</span>
            <input type="range" min="0" max="15" value="0" data-ts-key="trigger_left_stop_location" />
            <span class="imu-val" data-ts-val="trigger_left_stop_location">0</span>
          </div>
          <div class="imu-row">
            <span>L effect</span>
            <select data-ts-key="trigger_left_effect">${triggerEffectOptions()}</select>
            <span></span>
          </div>
          <div class="imu-row">
            <span>R status</span>
            <select data-ts-key="trigger_right_status">${triggerStatusOptions()}</select>
            <span></span>
          </div>
          <div class="imu-row">
            <span>R stop</span>
            <input type="range" min="0" max="15" value="0" data-ts-key="trigger_right_stop_location" />
            <span class="imu-val" data-ts-val="trigger_right_stop_location">0</span>
          </div>
          <div class="imu-row">
            <span>R effect</span>
            <select data-ts-key="trigger_right_effect">${triggerEffectOptions()}</select>
            <span></span>
          </div>
        </div>
      </div>
  ` : '';
  root.innerHTML = `
    <div class="gamepad">
      <div class="group">
        <h4>Left stick</h4>
        <div class="stick" data-stick="left"><div class="knob"></div></div>
        <div class="meta" id="${kind}-l-readout">center</div>
        <button class="danger" data-stick-reset="left">recenter</button>
      </div>
      <div class="group">
        <h4>Right stick</h4>
        <div class="stick" data-stick="right"><div class="knob"></div></div>
        <div class="meta" id="${kind}-r-readout">center</div>
        <button class="danger" data-stick-reset="right">recenter</button>
      </div>
      <div class="group">
        <h4>Triggers</h4>
        <div class="trigger"><span>L${kind === 'xone' ? '' : '2'}</span><input type="range" min="0" max="${def.triggers.max}" value="0" data-trigger="left" /></div>
        <div class="trigger"><span>R${kind === 'xone' ? '' : '2'}</span><input type="range" min="0" max="${def.triggers.max}" value="0" data-trigger="right" /></div>
      </div>
      <div class="group">
        <h4>D-pad</h4>
        <div class="btn-grid" style="grid-template-columns: repeat(3, 1fr); gap: 4px">
          <span></span><button data-hat="0,-1">↑</button><span></span>
          <button data-hat="-1,0">←</button><button data-hat="0,0">○</button><button data-hat="1,0">→</button>
          <span></span><button data-hat="0,1">↓</button><span></span>
        </div>
      </div>
      <div class="group" style="grid-column: 1 / -1">
        <h4>Face buttons</h4>
        <div class="btn-grid">${def.face.map(b => `<button data-btn="${b}">${b.replace('btn_','').toUpperCase()}</button>`).join('')}</div>
      </div>
      <div class="group" style="grid-column: 1 / -1">
        <h4>Shoulders / sticks</h4>
        <div class="btn-grid">${def.shoulder.map(b => `<button data-btn="${b}">${b.replace('btn_','').toUpperCase()}</button>`).join('')}</div>
      </div>
      <div class="group" style="grid-column: 1 / -1">
        <h4>Other</h4>
        <div class="btn-grid">${def.misc.map(b => `<button data-btn="${b}">${b.replace('btn_','').toUpperCase()}</button>`).join('')}</div>
      </div>
      ${touchImuHtml}
      ${triggerStatusHtml}
    </div>
  `;

  // Wire sticks (drag).
  $$('.stick', root).forEach(el => wireStick(el, dev, kind, el.dataset.stick));
  $$('[data-stick-reset]', root).forEach(b => b.addEventListener('click', e => {
    const which = e.currentTarget.dataset.stickReset;
    const s = state.controllerState[dev.id];
    s[which + '_stick_x'] = def.sticks.center;
    s[which + '_stick_y'] = def.sticks.center;
    refreshStickKnob(dev.id, kind, which);
    submitGamepad(dev, kind);
  }));

  // Triggers.
  $$('input[data-trigger]', root).forEach(inp => {
    inp.addEventListener('input', () => {
      const which = inp.dataset.trigger;
      state.controllerState[dev.id][which + '_trigger'] = parseInt(inp.value, 10);
      submitGamepad(dev, kind);
    });
  });

  // Hat.
  $$('button[data-hat]', root).forEach(b => b.addEventListener('click', () => {
    const [hx, hy] = b.dataset.hat.split(',').map(n => parseInt(n, 10));
    state.controllerState[dev.id].hat_x = hx;
    state.controllerState[dev.id].hat_y = hy;
    submitGamepad(dev, kind);
  }));

  // Buttons (toggle).
  $$('button[data-btn]', root).forEach(b => b.addEventListener('click', () => {
    const key = b.dataset.btn;
    const cur = state.controllerState[dev.id][key];
    state.controllerState[dev.id][key] = !cur;
    if (!cur) b.classList.add('held'); else b.classList.remove('held');
    submitGamepad(dev, kind);
  }));

  refreshStickKnob(dev.id, kind, 'left');
  refreshStickKnob(dev.id, kind, 'right');
  if (hasTouchImu) wireTouchpadAndImu(root, dev, kind);
  if (hasTriggerStatus) wireTriggerStatus(root, dev, kind);
  // Push initial neutral state so the OS sees a connected device.
  submitGamepad(dev, kind);
}

function wireTouchpadAndImu(root, dev, kind) {
  const bounds = TOUCHPAD_BOUNDS[kind];
  const pad = root.querySelector('[data-touchpad]');
  const dot1 = pad.querySelector('[data-dot="1"]');
  const dot2 = pad.querySelector('[data-dot="2"]');
  const readout = root.querySelector('[data-touch-readout]');
  const s = state.controllerState[dev.id];

  function updateReadout() {
    const p1 = s.touchpad_active  ? `T1 ${s.touchpad_x},${s.touchpad_y}`  : 'T1 –';
    const p2 = s.touchpad2_active ? `T2 ${s.touchpad2_x},${s.touchpad2_y}` : 'T2 –';
    readout.textContent = `${p1} · ${p2}`;
  }

  function placeDot(dot, x, y) {
    const rect = pad.getBoundingClientRect();
    dot.style.left = `${(x / bounds.x) * rect.width}px`;
    dot.style.top  = `${(y / bounds.y) * rect.height}px`;
  }

  function pointerToCoord(e) {
    const rect = pad.getBoundingClientRect();
    const fx = (e.clientX - rect.left) / rect.width;
    const fy = (e.clientY - rect.top)  / rect.height;
    return {
      x: clamp(Math.round(fx * bounds.x), 0, bounds.x),
      y: clamp(Math.round(fy * bounds.y), 0, bounds.y),
    };
  }

  // Track which slot each active pointer drives.
  const slots = new Map(); // pointerId -> 1 | 2

  pad.addEventListener('pointerdown', e => {
    pad.setPointerCapture(e.pointerId);
    const slot = e.shiftKey ? 2 : 1;
    slots.set(e.pointerId, slot);
    const c = pointerToCoord(e);
    if (slot === 1) {
      s.touchpad_active = true; s.touchpad_x = c.x; s.touchpad_y = c.y;
      dot1.hidden = false; placeDot(dot1, c.x, c.y);
    } else {
      s.touchpad2_active = true; s.touchpad2_x = c.x; s.touchpad2_y = c.y;
      dot2.hidden = false; placeDot(dot2, c.x, c.y);
    }
    submitGamepad(dev, kind);
    updateReadout();
  });

  pad.addEventListener('pointermove', e => {
    const slot = slots.get(e.pointerId);
    if (!slot) return;
    const c = pointerToCoord(e);
    if (slot === 1) {
      s.touchpad_x = c.x; s.touchpad_y = c.y;
      placeDot(dot1, c.x, c.y);
    } else {
      s.touchpad2_x = c.x; s.touchpad2_y = c.y;
      placeDot(dot2, c.x, c.y);
    }
    submitGamepad(dev, kind);
    updateReadout();
  });

  const release = e => {
    const slot = slots.get(e.pointerId);
    if (!slot) return;
    slots.delete(e.pointerId);
    if (e.ctrlKey) return; // Ctrl held — keep touch persisted
    if (slot === 1) {
      s.touchpad_active = false; dot1.hidden = true;
    } else {
      s.touchpad2_active = false; dot2.hidden = true;
    }
    submitGamepad(dev, kind);
    updateReadout();
  };
  pad.addEventListener('pointerup', release);
  pad.addEventListener('pointercancel', release);

  updateReadout();

  // IMU sliders.
  const imu = root.querySelector('[data-imu]');
  const axes = [
    { key: 'accel_x', label: 'aX', min: -20, max: 20, step: 0.1, unit: 'm/s²' },
    { key: 'accel_y', label: 'aY', min: -20, max: 20, step: 0.1, unit: 'm/s²' },
    { key: 'accel_z', label: 'aZ', min: -20, max: 20, step: 0.1, unit: 'm/s²' },
    { key: 'gyro_x',  label: 'gX', min: -10, max: 10, step: 0.1, unit: 'rad/s' },
    { key: 'gyro_y',  label: 'gY', min: -10, max: 10, step: 0.1, unit: 'rad/s' },
    { key: 'gyro_z',  label: 'gZ', min: -10, max: 10, step: 0.1, unit: 'rad/s' },
  ];
  imu.innerHTML = axes.map(a => `
    <div class="imu-row">
      <span>${a.label}</span>
      <input type="range" min="${a.min}" max="${a.max}" step="${a.step}" value="${s[a.key]}" data-imu-key="${a.key}" />
      <span class="imu-val" data-imu-val="${a.key}">${(+s[a.key]).toFixed(1)} ${a.unit}</span>
    </div>
  `).join('');
  imu.querySelectorAll('input[data-imu-key]').forEach(inp => {
    inp.addEventListener('input', () => {
      const k = inp.dataset.imuKey;
      const v = parseFloat(inp.value) || 0;
      s[k] = v;
      const lbl = imu.querySelector(`[data-imu-val="${k}"]`);
      const axis = axes.find(a => a.key === k);
      if (lbl && axis) lbl.textContent = `${v.toFixed(1)} ${axis.unit}`;
      submitGamepad(dev, kind);
    });
  });
  root.querySelector('[data-imu-reset]').addEventListener('click', () => {
    for (const a of axes) {
      s[a.key] = 0;
      const inp = imu.querySelector(`input[data-imu-key="${a.key}"]`);
      if (inp) inp.value = '0';
      const lbl = imu.querySelector(`[data-imu-val="${a.key}"]`);
      if (lbl) lbl.textContent = `0.0 ${a.unit}`;
    }
    submitGamepad(dev, kind);
  });
}

// Adaptive trigger status options for <select> dropdowns.
const TRIGGER_STATUS_LABELS = {
  0: '0 – None', 1: '1 – Ready', 2: '2 – Actuating', 3: '3 – Completed',
  4: '4 – Paused', 5: '5', 6: '6', 7: '7', 8: '8', 9: '9',
  10: '10', 11: '11', 12: '12', 13: '13', 14: '14', 15: '15',
};
const TRIGGER_EFFECT_LABELS = {
  0: '0 – Off', 1: '1 – Continuous', 2: '2 – Section',
  3: '3', 4: '4', 5: '5 – Vibrate', 6: '6 – Multi-Vibrate',
  7: '7', 8: '8', 9: '9', 10: '10', 11: '11', 12: '12', 13: '13', 14: '14', 15: '15',
};
function triggerStatusOptions() {
  return Object.entries(TRIGGER_STATUS_LABELS).map(
    ([v, l]) => `<option value="${v}">${l}</option>`
  ).join('');
}
function triggerEffectOptions() {
  return Object.entries(TRIGGER_EFFECT_LABELS).map(
    ([v, l]) => `<option value="${v}">${l}</option>`
  ).join('');
}

function wireTriggerStatus(root, dev, kind) {
  const s = state.controllerState[dev.id];
  const container = root.querySelector('[data-trigger-status]');
  if (!container) return;

  container.querySelectorAll('select[data-ts-key]').forEach(sel => {
    const key = sel.dataset.tsKey;
    sel.value = s[key] || 0;
    sel.addEventListener('change', () => {
      s[key] = parseInt(sel.value, 10);
      submitGamepad(dev, kind);
    });
  });
  container.querySelectorAll('input[data-ts-key]').forEach(inp => {
    const key = inp.dataset.tsKey;
    inp.value = s[key] || 0;
    const lbl = container.querySelector(`[data-ts-val="${key}"]`);
    inp.addEventListener('input', () => {
      const v = parseInt(inp.value, 10);
      s[key] = v;
      if (lbl) lbl.textContent = String(v);
      submitGamepad(dev, kind);
    });
  });
}

function wireStick(el, dev, kind, which) {
  const knob = $('.knob', el);
  const def = GAMEPAD_DEFS[kind];
  let dragging = false;
  const setFromPointer = e => {
    const rect = el.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top  + rect.height / 2;
    const r  = Math.min(rect.width, rect.height) / 2 - 12;
    let dx = (e.clientX - cx) / r;
    let dy = (e.clientY - cy) / r;
    const mag = Math.hypot(dx, dy);
    if (mag > 1) { dx /= mag; dy /= mag; }
    const half = def.sticks.max - def.sticks.center;
    const x = clamp(Math.round(def.sticks.center + dx * half), 0, def.sticks.max);
    const y = clamp(Math.round(def.sticks.center + dy * half), 0, def.sticks.max);
    state.controllerState[dev.id][which + '_stick_x'] = x;
    state.controllerState[dev.id][which + '_stick_y'] = y;
    refreshStickKnob(dev.id, kind, which);
    submitGamepad(dev, kind);
  };
  el.addEventListener('pointerdown', e => {
    dragging = true;
    el.setPointerCapture(e.pointerId);
    setFromPointer(e);
  });
  el.addEventListener('pointermove', e => { if (dragging) setFromPointer(e); });
  el.addEventListener('pointerup',   () => { dragging = false; });
}

function refreshStickKnob(deviceId, kind, which) {
  const def = GAMEPAD_DEFS[kind];
  const s = state.controllerState[deviceId];
  const x = s[which + '_stick_x'];
  const y = s[which + '_stick_y'];
  const half = def.sticks.max - def.sticks.center;
  const nx = (x - def.sticks.center) / half; // -1..1
  const ny = (y - def.sticks.center) / half;
  const root = document.querySelector(`.stick[data-stick="${which}"]`);
  if (!root) return;
  const r = root.getBoundingClientRect();
  const px = nx * (r.width  / 2 - 12);
  const py = ny * (r.height / 2 - 12);
  $('.knob', root).style.transform = `translate(${px}px, ${py}px)`;
  const readout = $(`#${kind}-${which[0]}-readout`);
  if (readout) readout.textContent = `${x}, ${y}`;
}

async function submitGamepad(dev, kind) {
  const s = state.controllerState[dev.id];
  try {
    await api('POST', `/api/devices/${dev.id}/${kind}/state`, s);
  } catch (e) {
    showErr(e);
  }
}

// ----- Generic device ------------------------------------------------------

function renderGenericControls(root, dev) {
  root.innerHTML = `
    <p class="muted">Submit any input report you like. The first byte is the report ID if the device uses numbered reports.</p>
    <label class="full">
      Report (hex)
      <textarea id="g-report" rows="4" placeholder="01 00 00 …"></textarea>
    </label>
    <button class="primary" id="g-submit">Submit input report</button>
  `;
  $('#g-submit', root).addEventListener('click', async () => {
    const hex = $('#g-report').value.replace(/\s+/g, '');
    if (!hex) return;
    try {
      await api('POST', `/api/devices/${dev.id}/generic/input`, { hex });
    } catch (e) { showErr(e); }
  });
}

// ----- Feedback / event log -----------------------------------------------

function pushEvent(id, ev) {
  if (id !== state.activeId) return;
  const log = $('#event-log');
  if (!log) return;
  const ts = new Date(ev.ts_ms).toLocaleTimeString([], { hour12: false });
  let line = '';
  switch (ev.type) {
    case 'hid_event':
      line = `<span class="ev-ts">${ts}</span> <span class="ev-kind">${ev.kind}</span> rid=${ev.report_id} ${ev.data_hex ? 'data=' + ev.data_hex : ''}`;
      break;
    case 'rumble': {
      const trig = (ev.left_trigger != null || ev.right_trigger != null)
        ? ` triggers=L:${ev.left_trigger ?? 0} R:${ev.right_trigger ?? 0}` : '';
      line = `<span class="ev-ts">${ts}</span> <span class="ev-rumble">RUMBLE</span> L=${ev.left} R=${ev.right}${trig}`;
      state.feedback[id] = state.feedback[id] || {};
      state.feedback[id].rumble = ev;
      break;
    }
    case 'led':
      line = `<span class="ev-ts">${ts}</span> <span class="ev-led">LED</span> #${hex(ev.red,2)}${hex(ev.green,2)}${hex(ev.blue,2)}`;
      state.feedback[id] = state.feedback[id] || {};
      state.feedback[id].led = ev;
      break;
    case 'player_led':
      line = `<span class="ev-ts">${ts}</span> <span class="ev-led">PLAYER_LED</span> 0x${hex(ev.value,2)}`;
      state.feedback[id] = state.feedback[id] || {};
      state.feedback[id].player = ev;
      break;
    case 'trigger_effect':
      line = `<span class="ev-ts">${ts}</span> <span class="ev-rumble">TRIGGER_FX</span> L=${ev.left_kind ?? '-'} R=${ev.right_kind ?? '-'}`;
      state.feedback[id] = state.feedback[id] || {};
      state.feedback[id].trigger = ev;
      break;
    case 'mic_led':
      line = `<span class="ev-ts">${ts}</span> <span class="ev-led">MIC_LED</span> ${['off','on','pulse'][ev.state] ?? 'state=' + ev.state}`;
      state.feedback[id] = state.feedback[id] || {};
      state.feedback[id].mic = ev;
      break;
    case 'input_snapshot':
      line = `<span class="ev-ts">${ts}</span> <span class="ev-info">→ submit</span> ${ev.hex}`;
      break;
    case 'diag':
      line = `<span class="ev-ts">${ts}</span> <span class="ev-info">${ev.level}</span> ${escape(ev.msg)}`;
      break;
    default:
      line = `<span class="ev-ts">${ts}</span> ${escape(JSON.stringify(ev))}`;
  }
  const div = document.createElement('div');
  div.innerHTML = line;
  log.appendChild(div);
  while (log.children.length > 250) log.removeChild(log.firstChild);
  log.scrollTop = log.scrollHeight;

  const dev = state.devices.find(d => d.id === id);
  if (dev) renderFeedback(dev);
}

function renderFeedback(dev) {
  const el = $('#feedback-summary');
  if (!el) return;
  const f = state.feedback[dev.id] || {};
  const kind = dev.kind;
  const blocks = [];

  // Rumble — all gamepad types.
  if (kind === 'ps4' || kind === 'ps5' || kind === 'xone') {
    const r = f.rumble || { left: 0, right: 0 };
    blocks.push(`
      <div class="feedback-row"><span>Rumble L</span>
        <div class="bar"><span style="width:${(r.left/255)*100}%"></span></div></div>
      <div class="feedback-row"><span>Rumble R</span>
        <div class="bar"><span style="width:${(r.right/255)*100}%"></span></div></div>
    `);
    if (kind === 'xone') {
      const lt = r.left_trigger ?? 0;
      const rt = r.right_trigger ?? 0;
      blocks.push(`
      <div class="feedback-row"><span>Trig L motor</span>
        <div class="bar"><span style="width:${(lt/255)*100}%"></span></div></div>
      <div class="feedback-row"><span>Trig R motor</span>
        <div class="bar"><span style="width:${(rt/255)*100}%"></span></div></div>`);
    }
  }

  // Lightbar LED — PS4 and PS5.
  if (kind === 'ps4' || kind === 'ps5') {
    const led = f.led || { red: 0, green: 0, blue: 0 };
    const c = `rgb(${led.red},${led.green},${led.blue})`;
    blocks.push(`<div class="feedback-row"><span>Lightbar</span><div><span class="swatch lg" style="background:${c}"></span> #${hex(led.red,2)}${hex(led.green,2)}${hex(led.blue,2)}</div></div>`);
  }

  // Player LED — PS5 only.
  if (kind === 'ps5') {
    const pl = f.player || { value: 0 };
    const bits = pl.value & 0x1f;
    const dots = [0,1,2,3,4].map(i => `<span class="pled${bits & (1 << i) ? ' on' : ''}"></span>`).join('');
    blocks.push(`<div class="feedback-row"><span>Player LED</span><div class="pled-row">${dots} <span class="meta">0x${hex(pl.value,2)}</span></div></div>`);
  }

  // Trigger FX — PS5 only.
  if (kind === 'ps5') {
    const t = f.trigger || {};
    const trigKind = k => ({ 0:'Off', 1:'Continuous', 2:'Section', 5:'Vibrate', 6:'Multi-Vibrate' }[k] ?? `0x${hex(k,2)}`);
    const trigDetail = (label, kv, dataHex) => {
      if (kv == null) return `${label}: \u2013`;
      return `${label}: ${trigKind(kv)}` + (dataHex ? ` <span class="meta">[${dataHex}]</span>` : '');
    };
    blocks.push(`<div class="feedback-row"><span>Trigger FX</span><div>${trigDetail('L', t.left_kind, t.left_data_hex)}<br>${trigDetail('R', t.right_kind, t.right_data_hex)}</div></div>`);
  }

  // Mic LED — PS5 only.
  if (kind === 'ps5') {
    const m = f.mic || { state: 0 };
    const micLabels = ['off', 'on (solid)', 'pulse (blink)'];
    const micClass = ['off', 'on', 'pulse'][m.state] || 'off';
    blocks.push(`<div class="feedback-row"><span>Mic LED</span><div><span class="mic-dot ${micClass}"></span> ${micLabels[m.state] || 'unknown'}</div></div>`);
  }

  if (blocks.length) {
    el.innerHTML = blocks.join('');
  } else {
    el.innerHTML = '<p class="muted">No feedback available for this device type.</p>';
  }
}

// ---------------------------------------------------------------------------
// Tiny utils
// ---------------------------------------------------------------------------

function escape(s) {
  return String(s).replace(/[&<>"']/g, c => ({ '&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;' }[c]));
}
function hex(n, w) {
  return Number(n).toString(16).padStart(w, '0');
}
function clamp(v, lo, hi) {
  return Math.max(lo, Math.min(hi, v));
}
function showErr(e) {
  console.error(e);
  toast('Error: ' + (e && e.message ? e.message : String(e)), 'error');
}

function toast(message, level) {
  level = level || 'info';
  const root = document.getElementById('toasts');
  if (!root) {
    // Fallback if the container is missing for any reason.
    console[level === 'error' ? 'error' : 'log'](message);
    return;
  }
  const el = document.createElement('div');
  el.className = `toast ${level}`;
  el.textContent = message;
  root.appendChild(el);
  // Trigger CSS transition.
  requestAnimationFrame(() => el.classList.add('show'));
  const ttl = level === 'error' ? 7000 : level === 'info' ? 3000 : 5000;
  const remove = () => {
    el.classList.remove('show');
    setTimeout(() => { if (el.parentNode) el.parentNode.removeChild(el); }, 200);
  };
  setTimeout(remove, ttl);
  el.addEventListener('click', remove);
}
