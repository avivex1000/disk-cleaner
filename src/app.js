const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// ── State ──
let categories = [];
let actions = [];               // PruneCommand items
let selectedItems = new Map();
let pendingDeleteItems = null;
let expandedCategories = new Set();
let totalScanners = 0;
let scannersCompleted = 0;
let scanning = false;
let scanStartTime = 0;
let selectMode = false;
let deletingPaths = new Set();  // items currently being deleted
let viewMode = 'grouped';       // 'grouped' or 'flat'

const COLORS = [
  '#3b82f6','#f97316','#eab308','#22c55e',
  '#a855f7','#ef4444','#06b6d4','#ec4899',
  '#84cc16','#f59e0b'
];

const COLOR_MAP = {};
let colorIdx = 0;
function colorFor(id) {
  if (!COLOR_MAP[id]) COLOR_MAP[id] = COLORS[colorIdx++ % COLORS.length];
  return COLOR_MAP[id];
}

// ── Formatting ──

function formatSize(bytes) {
  if (bytes === 0) return '0 B';
  const units = ['B','KB','MB','GB','TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const v = bytes / Math.pow(1024, i);
  return v.toFixed(v >= 100 ? 0 : v >= 10 ? 1 : 2) + ' ' + units[i];
}

function shortenPath(p) {
  const parts = p.split('/');
  return (parts.length >= 3 && parts[1] === 'Users') ? '~/' + parts.slice(3).join('/') : p;
}

function totalBytes() {
  return categories.reduce((s, c) => s + c.total_bytes, 0);
}

// ── Ingest: split PruneCommands from file items ──

const IRRELEVANT_THRESHOLD = 50_000; // actions under 50KB are "irrelevant"

function ingestCategory(cat) {
  const fileItems = [];
  const cmdItems = [];

  for (const item of cat.items) {
    if (item.item_type.kind === 'PruneCommand') {
      cmdItems.push({ ...item, _icon: cat.icon, _catName: cat.name, _scannerId: cat.id, _status: 'idle' });
    } else {
      fileItems.push(item);
    }
  }

  // Merge actions — always take the latest scan data, only preserve "running" state
  for (const cmd of cmdItems) {
    const idx = actions.findIndex(a => a.path === cmd.path);
    if (idx >= 0) {
      const wasRunning = actions[idx]._running;
      actions[idx] = cmd;
      if (wasRunning) actions[idx]._running = true;
    } else {
      actions.push(cmd);
    }
  }
  actions.sort((a, b) => {
    // Impactful (large size) first, then unknown (0), then low-impact (small)
    function weight(x) {
      if (x.size_bytes >= IRRELEVANT_THRESHOLD) return 0; // impactful
      if (x.size_bytes === 0) return 1; // unknown
      return 2; // low impact
    }
    const w = weight(a) - weight(b);
    if (w !== 0) return w;
    return b.size_bytes - a.size_bytes || a.description.localeCompare(b.description);
  });

  // Merge category (file items only)
  const fileCat = { ...cat, items: fileItems, total_bytes: fileItems.reduce((s, i) => s + i.size_bytes, 0) };
  const idx = categories.findIndex(c => c.id === cat.id);
  if (idx >= 0) categories[idx] = fileCat;
  else if (fileCat.items.length > 0) categories.push(fileCat);
  categories.sort((a, b) => b.total_bytes - a.total_bytes);
}

// ── Streaming listeners ──

(async () => {
  await listen('scan-category', e => { ingestCategory(e.payload); renderAll(); });
  await listen('scan-scanner-done', () => {
    scannersCompleted++;
    updateScanInfo();
    if (scannersCompleted >= totalScanners) finishScan();
  });
  await listen('scan-total-scanners', e => { totalScanners = e.payload; });
})();

// ── Scanning ──

async function startScan() {
  if (scanning) return;
  scanning = true;

  $('btn-scan').disabled = true;
  $('btn-scan').textContent = 'Scanning...';

  categories = [];
  actions = [];
  scannersCompleted = 0;
  totalScanners = 0;
  scanStartTime = performance.now();
  exitSelectMode();

  hide('empty-state');
  show('widgets');
  show('main');
  renderAll();

  try { await invoke('start_scan'); }
  catch (e) { showToast('Scan failed: ' + e); finishScan(); }
}

function finishScan() {
  scanning = false;
  $('btn-scan').disabled = false;
  $('btn-scan').textContent = 'Scan';
  // Show view controls once scan is done
  $('btn-select-mode').classList.remove('hidden');
  $('btn-view-toggle').classList.remove('hidden');
  updateScanInfo();
}

function updateScanInfo() {
  const el = $('scan-info');
  const t = ((performance.now() - scanStartTime) / 1000).toFixed(1);
  el.textContent = scanning ? `Scanning (${scannersCompleted}/${totalScanners || '?'}) ${t}s` : `${t}s`;
}

// ── Select mode ──

function enterSelectMode() {
  selectMode = true;
  document.body.classList.add('select-mode');
  selectedItems.clear();
  show('select-bar');
  $('btn-select-mode').classList.add('hidden');
  updateSelectBar();
  renderTable(); // re-render to show checkboxes
}

function exitSelectMode() {
  selectMode = false;
  document.body.classList.remove('select-mode');
  selectedItems.clear();
  hide('select-bar');
  if (!scanning && categories.length > 0) $('btn-select-mode').classList.remove('hidden');
  renderTable();
}

function updateSelectBar() {
  const n = selectedItems.size;
  const bytes = Array.from(selectedItems.values()).reduce((s, i) => s + i.size_bytes, 0);
  $('select-bar-info').textContent = n === 0
    ? 'No items selected'
    : `${n} item${n > 1 ? 's' : ''} selected (${formatSize(bytes)})`;
  $('btn-delete-selected').disabled = n === 0;
}

// ── View toggle ──

function toggleView() {
  viewMode = viewMode === 'grouped' ? 'flat' : 'grouped';
  $('btn-view-toggle').textContent = viewMode === 'grouped' ? 'By Size' : 'By Category';
  renderTable();
}

// ── Render all ──

function renderAll() {
  renderDashboard();
  renderSuggestions();
  renderTable();
}

// ── Dashboard ──

function renderDashboard() {
  const total = totalBytes() || 1;
  $('total-size').textContent = formatSize(totalBytes());

  const svg = $('donut-chart');
  svg.innerHTML = '';
  const cx = 100, cy = 100, r = 78, sw = 22;
  let cum = -90;

  categories.forEach(cat => {
    const pct = cat.total_bytes / total;
    if (pct < 0.005) return;
    const ang = pct * 360;
    const s = cum, e = cum + ang; cum = e;
    const sr = s * Math.PI / 180, er = e * Math.PI / 180;

    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', `M ${cx+r*Math.cos(sr)} ${cy+r*Math.sin(sr)} A ${r} ${r} 0 ${ang>180?1:0} 1 ${cx+r*Math.cos(er)} ${cy+r*Math.sin(er)}`);
    path.setAttribute('fill', 'none');
    path.setAttribute('stroke', colorFor(cat.id));
    path.setAttribute('stroke-width', sw);
    path.setAttribute('stroke-linecap', 'round');
    svg.appendChild(path);
  });

  const legend = $('legend');
  legend.innerHTML = '';
  categories.forEach(cat => {
    const el = document.createElement('div');
    el.className = 'legend-item';
    el.onclick = () => {
      expandedCategories.add(cat.id);
      renderTable();
      const row = document.getElementById(`cat-${cat.id}`);
      if (row) row.scrollIntoView({ behavior: 'smooth', block: 'center' });
    };
    el.innerHTML = `
      <span class="legend-dot" style="background:${colorFor(cat.id)}"></span>
      <span>${cat.icon} ${cat.name}</span>
      <span class="legend-size">${formatSize(cat.total_bytes)}</span>
    `;
    legend.appendChild(el);
  });
}

// ── Suggestions ──

function renderSuggestions() {
  const list = $('suggestions-list');
  list.innerHTML = '';

  if (actions.length === 0 && getOrphanedItems().length === 0 && scanning) {
    list.innerHTML = '<div style="color:var(--text3);font-size:12px;padding:8px">Scanning for suggestions...</div>';
    return;
  }

  // Orphaned items summary at the top
  const orphaned = getOrphanedItems();
  if (orphaned.length > 0) {
    const totalOrphaned = orphaned.reduce((s, i) => s + i.size_bytes, 0);
    const row = document.createElement('div');
    row.className = 'sug-row sug-orphan';
    row.innerHTML = `
      <span class="sug-icon">👻</span>
      <div class="sug-info">
        <div class="sug-desc">Orphaned app data (${orphaned.length} items)</div>
        <div class="sug-hint">Leftover files from uninstalled apps — safe to remove</div>
      </div>
      <span class="sug-size">${formatSize(totalOrphaned)}</span>
      <button class="btn-sug-run btn-sug-orphan">Clean All</button>
    `;
    row.querySelector('button').onclick = () => showDeleteModal(orphaned);
    list.appendChild(row);
  }

  for (const action of actions) {
    const running = action._running;
    const hasSize = action.size_bytes > 0;
    const lowImpact = !running && hasSize && action.size_bytes < IRRELEVANT_THRESHOLD;
    const unknownSize = !running && !hasSize;

    const row = document.createElement('div');
    row.className = 'sug-row'
      + (running ? ' running' : '')
      + (lowImpact ? ' low-impact' : '');

    const cmdLabel = action.item_type.command + ' ' + action.item_type.args.join(' ');

    // Button
    let btnHtml;
    if (running) {
      btnHtml = '<button class="btn-sug-run btn-sug-running" disabled><span class="sug-spinner"></span>Running</button>';
    } else {
      btnHtml = '<button class="btn-sug-run">Run</button>';
    }

    // Size badge — always reflects current disk state
    let sizeHtml = '';
    if (hasSize) {
      sizeHtml = `<span class="sug-size">${formatSize(action.size_bytes)}</span>`;
    } else if (unknownSize) {
      sizeHtml = '<span class="sug-size sug-size-unknown">—</span>';
    }

    // Hint
    let hintHtml = '';
    if (lowImpact) {
      hintHtml = '<div class="sug-hint">Cache is small — low impact</div>';
    } else if (unknownSize && !running) {
      hintHtml = '<div class="sug-hint">Run to check reclaimable space</div>';
    }

    row.innerHTML = `
      <span class="sug-icon">${action._icon || '⚡'}</span>
      <div class="sug-info">
        <div class="sug-desc">${esc(action.description)}</div>
        <div class="sug-cmd">${esc(cmdLabel)}</div>
        ${hintHtml}
      </div>
      ${sizeHtml}
      ${btnHtml}
    `;

    const btn = row.querySelector('.btn-sug-run');
    if (!running) {
      btn.onclick = () => runSuggestion(action);
    }

    list.appendChild(row);
  }
}

async function runSuggestion(action) {
  action._running = true;
  renderSuggestions();

  try {
    const result = await invoke('delete_selected', { items: [action] });

    if (result.errors.length > 0) {
      showToast('Failed: ' + result.errors[0].error);
    } else {
      showToast(action.item_type.command + ' cleanup complete');
    }

    // Always rescan to get the true state from disk
    action._running = false;
    const scannerIds = collectScannerIds(action);
    if (scannerIds.length > 0) {
      await invoke('rescan', { scannerIds });
    }
  } catch (e) {
    action._running = false;
    showToast('Failed: ' + e);
  }

  renderAll();
}

// Figure out which scanner IDs to rescan after running an action
function collectScannerIds(action) {
  const ids = new Set();
  // The action's own scanner
  if (action._scannerId) ids.add(action._scannerId);
  // Docker actions affect both docker and caches scanners
  if (action.item_type.command === 'docker' || action.item_type.command === 'orb') {
    ids.add('docker');
  }
  // Most tool actions affect both tools and caches
  ids.add('tools');
  ids.add('caches');
  return [...ids];
}

// ── Tree table ──

function renderTable() {
  viewMode === 'flat' ? renderTableFlat() : renderTableGrouped();
}

function renderTableGrouped() {
  const tbody = $('tree-body');
  tbody.innerHTML = '';

  const maxSize = Math.max(1, ...categories.flatMap(c => c.items.map(i => i.size_bytes)));

  categories.forEach(cat => {
    if (cat.items.length === 0) return;
    const open = expandedCategories.has(cat.id);

    // Category row
    const cr = document.createElement('tr');
    cr.className = 'row-category' + (open ? ' open' : '');
    cr.id = `cat-${cat.id}`;
    cr.innerHTML = `
      <td class="col-check"></td>
      <td class="col-name">
        <span class="tree-toggle">▶</span>
        <span class="cat-icon">${cat.icon}</span>
        <span>${esc(cat.name)}</span>
        <span class="cat-count">${cat.items.length}</span>
      </td>
      <td class="col-bar"></td>
      <td class="col-size cat-size-val">${formatSize(cat.total_bytes)}</td>
      <td class="col-action"></td>
    `;
    cr.querySelector('.col-name').onclick = () => {
      expandedCategories.has(cat.id) ? expandedCategories.delete(cat.id) : expandedCategories.add(cat.id);
      renderTable();
    };
    tbody.appendChild(cr);

    if (!open) return;

    cat.items.forEach(item => {
      const r = document.createElement('tr');
      const isDeleting = deletingPaths.has(item.path);
      r.className = 'row-item'
        + (isDeleting ? ' deleting' : '')
        + (item.orphaned ? ' orphaned' : '');
      const barPct = (item.size_bytes / maxSize) * 100;

      let actionHtml;
      if (isDeleting) {
        actionHtml = '<span class="item-deleting"><span class="item-spinner"></span>Deleting</span>';
      } else if (selectMode) {
        actionHtml = '';
      } else {
        actionHtml = '<button class="btn-item-delete" onclick="deleteSingle(this)">Delete</button>';
      }

      const orphanBadge = item.orphaned ? '<span class="orphan-badge">Orphaned</span>' : '';

      r.innerHTML = `
        <td class="col-check col-check-item">
          ${selectMode && !isDeleting ? `<input type="checkbox" onchange="toggleItem(this)" ${selectedItems.has(item.path)?'checked':''}/>` : ''}
        </td>
        <td class="col-name item-name-cell">
          <div class="item-desc">${esc(item.description)} ${orphanBadge}</div>
          <div class="item-path">${esc(shortenPath(item.path))}</div>
        </td>
        <td class="col-bar">
          <div class="size-bar-track"><div class="size-bar-fill" style="width:${barPct}%;background:${colorFor(cat.id)}"></div></div>
        </td>
        <td class="col-size">${formatSize(item.size_bytes)}</td>
        <td class="col-action">
          ${actionHtml}
        </td>
      `;

      // Store data
      if (selectMode) {
        const cb = r.querySelector('input[type=checkbox]');
        if (cb) { cb._diskItem = item; cb._catId = cat.id; }
      } else {
        const btn = r.querySelector('.btn-item-delete');
        if (btn) { btn._diskItem = item; btn._catId = cat.id; }
      }

      tbody.appendChild(r);
    });
  });
}

function renderTableFlat() {
  const tbody = $('tree-body');
  tbody.innerHTML = '';

  // Build a flat list of all items with their category info
  const allItems = [];
  for (const cat of categories) {
    for (const item of cat.items) {
      allItems.push({ ...item, _catIcon: cat.icon, _catId: cat.id });
    }
  }
  allItems.sort((a, b) => b.size_bytes - a.size_bytes);

  const maxSize = allItems.length > 0 ? allItems[0].size_bytes : 1;

  for (const item of allItems) {
    const r = document.createElement('tr');
    const isDeleting = deletingPaths.has(item.path);
    r.className = 'row-item'
      + (isDeleting ? ' deleting' : '')
      + (item.orphaned ? ' orphaned' : '');
    const barPct = (item.size_bytes / maxSize) * 100;

    let actionHtml;
    if (isDeleting) {
      actionHtml = '<span class="item-deleting"><span class="item-spinner"></span>Deleting</span>';
    } else if (selectMode) {
      actionHtml = '';
    } else {
      actionHtml = '<button class="btn-item-delete" onclick="deleteSingle(this)">Delete</button>';
    }

    const orphanBadge = item.orphaned ? '<span class="orphan-badge">Orphaned</span>' : '';

    r.innerHTML = `
      <td class="col-check col-check-item">
        ${selectMode && !isDeleting ? `<input type="checkbox" onchange="toggleItem(this)" ${selectedItems.has(item.path)?'checked':''}/>` : ''}
      </td>
      <td class="col-name flat-name-cell">
        <span class="flat-cat-icon">${item._catIcon}</span>
        <div class="item-info-wrap">
          <div class="item-desc">${esc(item.description)} ${orphanBadge}</div>
          <div class="item-path">${esc(shortenPath(item.path))}</div>
        </div>
      </td>
      <td class="col-bar">
        <div class="size-bar-track"><div class="size-bar-fill" style="width:${barPct}%;background:${colorFor(item._catId)}"></div></div>
      </td>
      <td class="col-size">${formatSize(item.size_bytes)}</td>
      <td class="col-action">
        ${actionHtml}
      </td>
    `;

    if (selectMode) {
      const cb = r.querySelector('input[type=checkbox]');
      if (cb) { cb._diskItem = item; cb._catId = item._catId; }
    } else {
      const btn = r.querySelector('.btn-item-delete');
      if (btn) { btn._diskItem = item; btn._catId = item._catId; }
    }

    tbody.appendChild(r);
  }
}

// ── Item interactions ──

function toggleItem(cb) {
  const item = cb._diskItem;
  cb.checked ? selectedItems.set(item.path, item) : selectedItems.delete(item.path);
  updateSelectBar();
}

function deleteSingle(btn) {
  showDeleteModal([btn._diskItem]);
}

function deleteSelected() {
  if (selectedItems.size === 0) return;
  showDeleteModal(Array.from(selectedItems.values()));
}

// ── Modal / Deletion ──

function showDeleteModal(items) {
  pendingDeleteItems = items;
  const tb = items.reduce((s, i) => s + i.size_bytes, 0);
  $('modal-title').textContent = 'Confirm Deletion';
  $('modal-message').textContent = `Delete ${items.length} item${items.length > 1 ? 's' : ''} to free up ${formatSize(tb)}?`;
  $('modal-items').innerHTML = items.map(i => `<div>${shortenPath(i.path)}</div>`).join('');
  show('modal-overlay');
}

function closeModal() { hide('modal-overlay'); pendingDeleteItems = null; }

async function confirmDelete() {
  const items = pendingDeleteItems;
  closeModal();
  if (!items || !items.length) return;

  // If this is a suggestion action, run it through the action flow
  if (items.length === 1 && items[0].item_type.kind === 'PruneCommand') {
    runSuggestion(items[0]);
    return;
  }

  // Mark items as deleting and re-render immediately
  for (const item of items) deletingPaths.add(item.path);
  renderTable();

  try {
    const result = await invoke('delete_selected', { items });

    showToast(`Freed ${formatSize(result.bytes_freed)}`);

    // Local update
    const deleted = new Set(result.deleted);
    for (const cat of categories) {
      const pre = cat.items.length;
      cat.items = cat.items.filter(i => !deleted.has(i.path));
      if (cat.items.length !== pre) cat.total_bytes = cat.items.reduce((s, i) => s + i.size_bytes, 0);
    }
    categories = categories.filter(c => c.items.length > 0);
    for (const item of items) selectedItems.delete(item.path);

    if (result.errors.length > 0) console.warn('Errors:', result.errors);
    renderAll();
    if (selectMode) updateSelectBar();
  } catch (e) {
    showToast('Failed: ' + e);
  } finally {
    for (const item of items) deletingPaths.delete(item.path);
    renderTable();
  }
}

// ── Orphan detection ──

function getOrphanedItems() {
  return categories.flatMap(c => c.items.filter(i => i.orphaned));
}

// ── Helpers ──

function showToast(msg) {
  const t = document.createElement('div');
  t.className = 'toast';
  t.textContent = msg;
  document.body.appendChild(t);
  setTimeout(() => t.remove(), 3000);
}

function $(id) { return document.getElementById(id); }
function show(id) { $(id).classList.remove('hidden'); }
function hide(id) { $(id).classList.add('hidden'); }

function esc(s) {
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}
