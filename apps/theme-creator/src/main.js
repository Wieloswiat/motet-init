// Sets window.Motet and window.MotetDemo, which the export's script reads.
import './motet.js';

(function () {
  'use strict';
  if (!document.documentElement.lang) document.documentElement.lang = 'en';
  var M = window.Motet, D = window.MotetDemo, h = M.h, icon = M.icon, C = M.color;
  var $ = function (id) { return document.getElementById(id); };
  var clone = function (o) { return JSON.parse(JSON.stringify(o)); };

  /* The editor wears Motet itself, in the viewer's light, dark or contrast theme. */
  var platform = M.mountWorld(document.documentElement, M.defaultRecord, {});
  try { matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function () { platform.update(); if (S.mode === 'world') updateFrame(); }); } catch (e) {}

  /* ---------------------------------------------------------------- capabilities (optional) */
  var caps = {};
  function cap(name) {
    if (!caps[name]) caps[name] = (window.claude && typeof window.claude.use === 'function')
      ? Promise.resolve(window.claude.use(name)).catch(function () { return null; })
      : Promise.resolve(null);
    return caps[name];
  }

  /* ---------------------------------------------------------------- the record's fields */
  var FACE_NAMES = { motet: 'IBM Plex Sans', 'motet-condensed': 'IBM Plex Sans Condensed', 'source-serif': 'Source Serif 4', oxanium: 'Oxanium',
    'big-shoulders-stencil': 'Big Shoulders Stencil', cormorant: 'Cormorant Garamond', 'grenze-gotisch': 'Grenze Gotisch', mono: 'IBM Plex Mono', archivo: 'Archivo Condensed' };
  var E = function (list, def) { return { t: 'enum', v: list, def: def }; };
  var I = function (min, max, def) { return { t: 'int', min: min, max: max, def: def }; };
  var SPEC = {
    'palette.primary': { t: 'color', def: '#2b6b8a' }, 'palette.secondary': { t: 'color', def: '#5a7d8f' }, 'palette.neutral': { t: 'color' },
    'palette.scheme': E(['neutral', 'tonal', 'vibrant', 'expressive', 'fidelity', 'monochrome'], 'tonal'),
    'palette.chroma': I(0, 200, 100), 'palette.temperature': I(-100, 100, 0),
    'palette.depth.light': I(-2, 2, 0), 'palette.depth.dark': I(-2, 2, 0), 'palette.mode': E(['light', 'dark']),
    'material.finish': E(['matte', 'satin', 'sheen'], 'matte'), 'material.grain': I(0, 4, 0), 'material.tint': I(0, 12, 0),
    'material.edge': I(0, 2, 0), 'material.shadow': I(0, 2, 1), 'material.mica': { t: 'bool', def: false }, 'material.sheet': E(['solid', 'veil', 'frosted'], 'solid'),
    'shape.corner': E(['round', 'squircle', 'bevel', 'square'], 'squircle'), 'shape.scale': I(0, 150, 100), 'shape.profile': E(['even', 'cut', 'clipped'], 'even'),
    'shape.border': E(['none', 'hairline', 'rule', 'heavy'], 'hairline'), 'shape.tone': E(['neutral', 'ornament'], 'neutral'), 'shape.frame': E(['none', 'double'], 'none'),
    'ornament.set': E(['rule', 'fade', 'filigree', 'segment'], 'rule'), 'ornament.level': I(0, 3, 0),
    'ornament.edge': E(['fade', 'straight', 'torn', 'burned', 'arch'], 'fade'), 'ornament.dropCap': { t: 'bool', def: false },
    'type.display': E(Object.keys(FACE_NAMES), 'motet'), 'type.case': E(['none', 'upper'], 'none'), 'type.tracking': I(-40, 160, 0), 'type.weight': I(100, 900, 600),
    'type.names': E(['ui', 'display'], 'ui'), 'type.reading': E(['serif', 'sans'], 'serif'), 'type.numerals': E(['proportional', 'tabular', 'oldstyle'], 'proportional'),
    'type.paragraphs': E(['spaced', 'indented'], 'spaced'),
    'icons.style': E(['outlined', 'rounded', 'sharp'], 'outlined'), 'icons.weight': I(200, 600, 400),
    'motion.character': E(['standard', 'crisp', 'soft', 'eerie'], 'standard'),
    'density': E(['comfortable', 'standard', 'compact'], 'standard'),
    'composition.post': E(['open', 'card', 'dossier', 'log'], 'open'), 'composition.portrait': E(['gutter', 'inline', 'none'], 'gutter'),
    'composition.names': E(['plain', 'plate'], 'plain'), 'composition.plate': E(['ink', 'accent'], 'ink'),
    'imagery.placement': E(['continuous', 'backdrop', 'header'], 'continuous'),
    'imagery.treatments.veil': I(0, 100, 0), 'imagery.treatments.vignette': I(0, 100, 0), 'imagery.treatments.duotone': I(0, 100, 0),
    'imagery.treatments.blur': I(0, 100, 0), 'imagery.treatments.parallax': I(0, 100, 0),
    'imagery.title': E(['page', 'art', 'hidden'], 'page'), 'imagery.titleBacking': E(['shade', 'halo', 'none'], 'shade'), 'imagery.focus': { t: 'pair', def: [50, 50] }, 'imagery.dominant': { t: 'color' }, 'imagery.range': { t: 'colors' },
    'chrome.tint': I(0, 8, 0)
  };
  var OVERRIDABLE = ['ink', 'ink-muted', 'line-strong', 'accent', 'on-accent', 'accent-ink', 'on-accent-soft', 'ornament', 'focus', 'canvas', 'surface', 'surface-raised', 'accent-soft', 'ornament-soft'];
  var ROLE_NAMES = { ink: 'Text', 'ink-muted': 'Secondary text', 'line-strong': 'Strong line', accent: 'Accent', 'on-accent': 'Text on accent', 'accent-ink': 'Links and accent text',
    'on-accent-soft': 'Text on soft accent', ornament: 'Ornament', focus: 'Focus ring', canvas: 'Canvas', surface: 'Page', 'surface-raised': 'Raised surface',
    'accent-soft': 'Soft accent', 'ornament-soft': 'Soft ornament', 'danger-ink': 'Danger text', 'on-danger': 'Text on danger' };
  var GROUND_NAMES = { surface: 'page', 'surface-quiet': 'quiet surface', 'surface-raised': 'raised surface', canvas: 'canvas', accent: 'accent', 'accent-soft': 'soft accent', danger: 'danger' };

  function get(o, path) { return path.split('.').reduce(function (a, k) { return a == null ? undefined : a[k]; }, o); }
  function put(o, path, v) {
    var ks = path.split('.'), a = o;
    for (var i = 0; i < ks.length - 1; i++) { if (a[ks[i]] == null || typeof a[ks[i]] !== 'object' || Array.isArray(a[ks[i]])) a[ks[i]] = {}; a = a[ks[i]]; }
    if (v === undefined || v === null) delete a[ks[ks.length - 1]]; else a[ks[ks.length - 1]] = v;
  }
  function hex(v) {
    if (typeof v !== 'string') return null;
    var s = v.trim().toLowerCase(); if (s[0] !== '#') s = '#' + s;
    if (s.length === 4) s = '#' + s[1] + s[1] + s[2] + s[2] + s[3] + s[3];
    return /^#[0-9a-f]{6}$/.test(s) ? s : null;
  }
  function checkField(s, v) {
    if (s.t === 'color') return hex(v) || undefined;
    if (s.t === 'enum') return s.v.indexOf(v) >= 0 ? v : undefined;
    if (s.t === 'int') { var n = Number(v); return isFinite(n) ? Math.max(s.min, Math.min(s.max, Math.round(n))) : undefined; }
    if (s.t === 'bool') return typeof v === 'boolean' ? v : undefined;
    if (s.t === 'pair') return Array.isArray(v) && v.length === 2 && v.every(function (x) { return isFinite(Number(x)); }) ? v.map(function (x) { return Math.max(0, Math.min(100, Math.round(Number(x)))); }) : undefined;
    if (s.t === 'colors') return Array.isArray(v) && v.length === 2 && v.every(hex) ? v.map(hex) : undefined;
  }
  function cleanOrnament(o) {
    if (!o || typeof o !== 'object' || !Array.isArray(o.vb) || !Array.isArray(o.p)) return null;
    var c = { vb: o.vb.slice(0, 2).map(Number), p: o.p.slice(0, 16).map(function (x) {
      var y = { d: String(x && x.d || '') };
      if (x && x.s != null) y.s = Number(x.s);
      if (x && x.m) y.m = true;
      if (x && x.rule === 'evenodd') y.rule = 'evenodd';
      return y;
    }) };
    return M.validateOrnament(c).ok ? c : null;
  }
  /* Records name the engine they were made for. Engine 2 split grain into five levels (old 1–3
     are 2–4 now) and dropped the ledger post layout. */
  var ENGINE = 2;
  function migrate(src, engine) {
    if (!src || typeof src !== 'object' || (engine || 1) >= ENGINE) return src;
    var r = clone(src);
    var g = get(r, 'material.grain');
    if (typeof g === 'number' && g > 0) put(r, 'material.grain', Math.min(4, g + 1));
    if (get(r, 'composition.post') === 'ledger') put(r, 'composition.post', 'open');
    return r;
  }
  /* Untrusted input (imports, shared library rows, old drafts) passes through here: known fields
     are checked, unknown ones dropped, and anything invalid is reported and left to the engine. */
  function sanitize(src) {
    var out = {}, issues = [];
    if (!src || typeof src !== 'object') return { record: out, issues: ['not an object'] };
    var img = src.imagery;
    if (img && typeof img === 'object' && !img.treatments && img.treatment) {
      src = clone(src);
      src.imagery.treatments = M.treatmentsOf(img);
      delete src.imagery.treatment; delete src.imagery.strength;
    }
    Object.keys(SPEC).forEach(function (path) {
      var v = get(src, path);
      if (v === undefined || v === null || v === '') return;
      var ok = checkField(SPEC[path], v);
      if (ok === undefined) issues.push(path);
      else { put(out, path, ok); if (SPEC[path].t === 'int' && ok !== Number(v)) issues.push(path + ' (moved into range)'); }
    });
    var ov = get(src, 'palette.overrides');
    if (ov && typeof ov === 'object') ['light', 'dark'].forEach(function (m) {
      var o = ov[m]; if (!o || typeof o !== 'object') return;
      Object.keys(o).forEach(function (role) {
        if (OVERRIDABLE.indexOf(role) >= 0 && hex(o[role])) put(out, 'palette.overrides.' + m + '.' + role, hex(o[role]));
        else issues.push('palette.overrides.' + m + '.' + role);
      });
    });
    var cu = get(src, 'ornament.custom');
    if (cu && typeof cu === 'object') ['motif', 'corner', 'flank'].forEach(function (slot) {
      if (!cu[slot]) return;
      var o = cleanOrnament(cu[slot]);
      if (o) put(out, 'ornament.custom.' + slot, o); else issues.push('ornament.custom.' + slot);
    });
    return { record: out, issues: issues };
  }
  /* the value the engine will use: the record's, else the field's default */
  function val(path) {
    var v = get(S.record, path);
    if (v !== undefined) return v;
    var d = get(M.defaultRecord, path);
    return d !== undefined ? d : SPEC[path] && SPEC[path].def;
  }

  /* ---------------------------------------------------------------- state */
  var S = {
    record: {}, name: '', basedOn: 'saltroad', sample: 'saltroad', art: null,
    docId: null, savedSnap: null, savedAt: null,
    view: 'scene', mode: 'world', device: 'desktop',
    reader: { contrast: false, decoration: true, reduceTransparency: false, plain: false }
  };
  var presetName = function (key) { var w = D.byKey(key); return key === 'saltroad' ? 'The Salt Road' : w ? w.name : key; };
  var PRESET_NOTES = { saltroad: 'Motet’s own look', neon: 'Angular sci-fi', ashfall: 'Post-apocalyptic', vellum: 'Decorated fantasy', hollow: 'Gothic horror', ledger: 'Modern strategy', kepler: 'Diegetic terminal',
    elarid: 'Heroic fantasy, Wieloświat', oskad: 'Weird west, Wieloświat', mglawica: 'Space opera archive' };
  function presetRecord(key) {
    var w = D.byKey(key) || D.worlds[0];
    return sanitize(w.record).record;
  }
  function applyPreset(key, keepName) {
    S.record = presetRecord(key);
    S.basedOn = key; S.sample = key;
    S.art = { kind: 'sample', key: key };
    if (!keepName) S.name = presetName(key);
  }

  function artUrl(a) {
    a = a === undefined ? S.art : a;
    if (!a) return null;
    if (a.kind === 'sample') return D.art[a.key] || null;
    if (a.kind === 'asset') return '/_blob/' + a.id;
    if (a.kind === 'local') return a.url;
    return null;
  }
  function liveRecord() {
    var r = clone(S.record);
    r.id = 'draft'; r.name = S.name || 'Untitled theme';
    var url = artUrl();
    if (url) { r.imagery = r.imagery || {}; r.imagery.art = url; }
    else if (r.imagery) delete r.imagery.art;
    return r;
  }

  /* ---------------------------------------------------------------- history and persistence */
  function snapshot() { return JSON.stringify({ record: S.record, name: S.name, art: S.art, basedOn: S.basedOn }); }
  var past = [], future = [], lastSnap = null;
  function commit() {
    var now = snapshot();
    if (now === lastSnap) { changed(); return; }
    if (lastSnap) { past.push(lastSnap); if (past.length > 200) past.shift(); }
    future = []; lastSnap = now;
    changed();
  }
  function restore(snap) {
    var o = JSON.parse(snap);
    S.record = o.record; S.name = o.name; S.art = o.art; S.basedOn = o.basedOn;
    lastSnap = snap;
    syncControls(); changed(true);
  }
  function undo() { if (!past.length) return; future.push(lastSnap); restore(past.pop()); }
  function redo() { if (!future.length) return; past.push(lastSnap); restore(future.pop()); }

  var DRAFT_KEY = 'motet.themeCreator.draft.v1';  /* the draft carries its own engine version */
  var draftTimer = 0;
  function saveDraft() {
    clearTimeout(draftTimer);
    draftTimer = setTimeout(function () {
      try {
        localStorage.setItem(DRAFT_KEY, JSON.stringify({
          engine: ENGINE, record: S.record, name: S.name, art: S.art && S.art.kind !== 'local' ? S.art : null, basedOn: S.basedOn, sample: S.sample,
          docId: S.docId, savedSnap: S.savedSnap, view: S.view, mode: S.mode, device: S.device
        }));
      } catch (e) {}
    }, 400);
  }
  function loadDraft() {
    try {
      var raw = localStorage.getItem(DRAFT_KEY);
      if (!raw) return false;
      var d = JSON.parse(raw);
      var clean = sanitize(migrate(d.record, d.engine));
      S.record = clean.record;
      S.name = typeof d.name === 'string' ? d.name.slice(0, 64) : 'Untitled theme';
      S.basedOn = D.byKey(d.basedOn) ? d.basedOn : 'saltroad';
      S.sample = D.byKey(d.sample) ? d.sample : S.basedOn;
      S.art = cleanArt(d.art);
      S.docId = typeof d.docId === 'string' ? d.docId : null;
      S.savedSnap = typeof d.savedSnap === 'string' ? d.savedSnap : null;
      if (['scene', 'front', 'components', 'palette'].indexOf(d.view) >= 0) S.view = d.view;
      if (['world', 'light', 'dark'].indexOf(d.mode) >= 0) S.mode = d.mode;
      if (['desktop', 'phone'].indexOf(d.device) >= 0) S.device = d.device;
      return true;
    } catch (e) { return false; }
  }
  function cleanArt(a) {
    if (!a || typeof a !== 'object') return null;
    if (a.kind === 'sample' && D.art[a.key]) return { kind: 'sample', key: a.key };
    if (a.kind === 'asset' && /^[0-9a-f]{32}$/.test(String(a.id))) return { kind: 'asset', id: a.id, type: String(a.type || 'image/png').slice(0, 40), name: String(a.name || '').slice(0, 120) };
    return null;
  }

  /* ---------------------------------------------------------------- preview frame */
  var frame = $('frame'), frameReady = false;
  window.addEventListener('message', function (e) {
    if (e.source !== frame.contentWindow) return;
    var m = e.data || {};
    if (m.type === 'ready') { frameReady = true; renderFrame(true); }
    else if (m.type === 'error' && window.console) console.warn('Preview:', m.message);
  });
  function previewMode() {
    if (S.mode !== 'world') return S.mode;
    return (S.record.palette && S.record.palette.mode) || M.currentMode();
  }
  function mountOpts() {
    var r = S.reader;
    return { mode: previewMode(), contrast: r.contrast ? 'high' : 'standard', reader: { decoration: r.decoration ? 'on' : 'off', reduceTransparency: r.reduceTransparency, plain: r.plain } };
  }
  function renderFrame(top) {
    if (!frameReady) return;
    frame.contentWindow.postMessage({ type: 'render', view: S.view, sample: S.sample, record: liveRecord(), mount: mountOpts(), scroll: top ? 'top' : null }, '*');
  }
  var upRaf = 0;
  function updateFrame() {
    if (!frameReady || upRaf) return;
    upRaf = requestAnimationFrame(function () {
      upRaf = 0;
      frame.contentWindow.postMessage({ type: 'update', record: liveRecord(), mount: mountOpts() }, '*');
    });
  }
  var zoomLabel = h('span', { 'class': 'tc-zoom', 'aria-hidden': 'true' });
  function layoutDevice() {
    var desk = $('desk'), dev = $('device');
    var phone = S.device === 'phone';
    var W = phone ? 390 : 1280;
    var aw = Math.max(200, desk.clientWidth - 34), ah = Math.max(200, desk.clientHeight - 34);
    var s, H;
    if (phone) { H = 844; s = Math.min(1, aw / W, (ah - 12) / H); }
    else { s = Math.min(1, aw / W); H = Math.max(640, Math.floor(ah / s)); }
    frame.style.width = W + 'px'; frame.style.height = H + 'px';
    frame.style.transform = s < 1 ? 'scale(' + s + ')' : 'none';
    dev.style.width = Math.floor(W * s) + 'px'; dev.style.height = Math.floor(H * s) + 'px';
    dev.setAttribute('data-device', S.device);
    zoomLabel.textContent = Math.round(s * 100) + '%';
  }
  if (typeof ResizeObserver !== 'undefined') new ResizeObserver(layoutDevice).observe($('desk'));
  window.addEventListener('resize', layoutDevice);

  /* ---------------------------------------------------------------- control binders */
  var refreshers = [];
  var syncing = false;
  function syncControls() { syncing = true; try { refreshers.forEach(function (f) { f(); }); } finally { syncing = false; } }
  function live() { updateFrame(); scheduleDerived(); }
  function setPath(path, v) { put(S.record, path, v); }

  function label(text, forId) { return h(forId ? 'label' : 'span', { 'class': 'm-field__label', 'for': forId || null }, text); }
  function help(text, tone) { return text == null ? null : h('p', { 'class': 'tc-help', 'data-tone': tone || null, hidden: text === '' }, text); }
  function setSeg(g, v) {
    g.querySelectorAll('[role=radio]').forEach(function (b) { var on = b.getAttribute('data-value') === String(v); b.setAttribute('aria-checked', String(on)); b.tabIndex = on ? 0 : -1; });
  }
  function seg(path, items, text, opts) {
    opts = opts || {};
    var cur = function () { var v = val(path); return v === undefined || v === null ? '' : String(v); };
    var g = M.Segmented({ label: text, value: cur(), items: items.map(function (it) { return { value: String(it[0]), label: it[1], icon: it[2] }; }) });
    g.addEventListener('change', function (e) {
      if (syncing) return;
      var it = items.filter(function (x) { return String(x[0]) === e.detail.value; })[0];
      var v = it ? it[0] : e.detail.value;
      setPath(path, v === '' ? undefined : v);
      if (opts.after) opts.after(v);
      commit();
    });
    refreshers.push(function () { setSeg(g, cur()); });
    var hp = help(opts.help);
    var box = h('div', { 'class': 'tc-field' }, label(text), g, hp);
    box.helpEl = hp;
    return box;
  }
  function slider(path, o) {
    var s = M.Slider({ label: o.label, min: o.min, max: o.max, step: o.step || 1, value: val(path), format: o.format, marks: o.marks });
    s.addEventListener('input', function (e) { if (syncing) return; setPath(path, e.detail.value); live(); });
    s.addEventListener('change', function (e) { if (syncing) return; setPath(path, e.detail.value); commit(); });
    refreshers.push(function () { s.setValue(val(path)); });
    return o.help ? h('div', { 'class': 'tc-field' }, s, help(o.help)) : s;
  }
  function toggle(path, text, helpText, opts) {
    opts = opts || {};
    var on = function () { var v = val(path); return opts.on ? v === opts.on : !!v; };
    var sw = M.Switch({ checked: on(), label: text });
    sw.setAttribute('data-m-ready', '');
    sw.addEventListener('click', function () {
      var next = sw.getAttribute('aria-checked') !== 'true';
      sw.setAttribute('aria-checked', String(next));
      setPath(path, opts.on ? (next ? opts.on : opts.off) : next);
      commit();
    });
    refreshers.push(function () { sw.setAttribute('aria-checked', String(on())); });
    return h('div', { 'class': 'tc-switch' }, h('div', { 'class': 'tc-switch__text' }, h('span', { 'class': 'tc-switch__label', 'aria-hidden': 'true' }, text), help(helpText)), sw);
  }
  function select(path, options, text, helpText, opts) {
    opts = opts || {};
    var s = M.Select({ label: text, value: val(path), options: options, help: helpText });
    s.addEventListener('change', function (e) { if (syncing) return; setPath(path, e.detail.value); if (opts.after) opts.after(e.detail.value); commit(); });
    refreshers.push(function () { s.setValue(val(path)); });
    return s;
  }
  function radios(path, items, legend, helpText) {
    var g = M.RadioGroup({ legend: legend, value: String(val(path)), variant: 'cards', help: helpText, items: items.map(function (it) { return { value: String(it[0]), label: it[1], description: it[2], icon: it[3] }; }),
      onChange: function (v) { if (syncing) return; var it = items.filter(function (x) { return String(x[0]) === v; })[0]; setPath(path, it ? it[0] : v); commit(); } });
    refreshers.push(function () { g.querySelectorAll('input[type=radio]').forEach(function (r) { r.checked = r.value === String(val(path)); }); });
    return g;
  }
  function color(path, o) {
    var cp = M.ColorPicker({ label: o.label, value: get(S.record, path) || null, placeholder: o.placeholder, clearable: o.clearable, help: o.help, swatches: o.swatches, against: o.against });
    cp.addEventListener('input', function (e) { if (syncing) return; setPath(path, e.detail.value); live(); });
    cp.addEventListener('change', function (e) { if (syncing) return; setPath(path, e.detail.value || undefined); commit(); });
    refreshers.push(function () { cp.setValue(get(S.record, path) || (o.clearable ? null : SPEC[path].def)); });
    return cp;
  }

  /* ---------------------------------------------------------------- panel */
  var panel = $('panel');
  var derived = [];   // functions run after every change

  /* identity */
  var nameInput = M.TextInput({ label: 'Theme name', value: S.name, autocomplete: 'off' });
  var nameEl = nameInput.querySelector('input');
  nameEl.maxLength = 64;
  nameEl.addEventListener('input', function () { S.name = nameEl.value; $('docName').textContent = S.name || 'Untitled theme'; });
  nameEl.addEventListener('change', function () { S.name = nameEl.value.trim().slice(0, 64); commit(); });
  refreshers.push(function () { if (document.activeElement !== nameEl) nameEl.value = S.name; });
  var worldOptions = D.worlds.map(function (w) { return { value: w.key, label: presetName(w.key), description: PRESET_NOTES[w.key] }; });
  var basedSel = M.Select({ label: 'Based on', value: S.basedOn, options: worldOptions, help: 'Replaces every setting. Undo brings yours back.' });
  basedSel.addEventListener('change', function (e) {
    if (syncing) return;
    var prev = S.name, wasPreset = prev === presetName(S.basedOn);
    applyPreset(e.detail.value, !wasPreset && !!prev);
    commit(); syncControls(); renderFrame(true);
    M.toast({ message: 'Started from ' + presetName(e.detail.value), action: { label: 'Undo', onClick: undo }, duration: 6000 });
  });
  refreshers.push(function () { basedSel.setValue(S.basedOn); });
  var sampleSel = M.Select({ label: 'Sample scene', value: S.sample, options: D.worlds.map(function (w) { return { value: w.key, label: w.scene.title, description: presetName(w.key) }; }), help: 'Preview text and characters only; not part of the theme.' });
  sampleSel.addEventListener('change', function (e) { if (syncing) return; S.sample = e.detail.value; saveDraft(); renderFrame(true); });
  refreshers.push(function () { sampleSel.setValue(S.sample); });
  panel.appendChild(h('div', { 'class': 'tc-ident' }, nameInput, h('div', { 'class': 'tc-pair' }, basedSel, sampleSel)));

  /* colour */
  var presetSwatches = D.worlds.map(function (w) { return { value: w.record.palette.primary, label: presetName(w.key) }; });
  var resolvedBox = h('div', { 'class': 'tc-resolved' });
  var notesBox = h('ul', { 'class': 'tc-notes' });
  var primary = color('palette.primary', { label: 'Primary', help: 'Accent, links, buttons and the focus ring.', swatches: presetSwatches });
  var secondary = color('palette.secondary', { label: 'Secondary', help: 'Ornaments, dividers and flourishes.', swatches: D.worlds.map(function (w) { return { value: w.record.palette.secondary, label: presetName(w.key) }; }) });
  var neutral = color('palette.neutral', { label: 'Neutral', clearable: true, placeholder: 'From temperature', help: 'Leave empty to tint the greys by temperature.' });
  var overridesBox = h('div', { 'class': 'tc-overrides' });
  var ovMode = 'light';
  var ovModeSeg = M.Segmented({ label: 'Mode', value: 'light', items: [{ value: 'light', label: 'Light' }, { value: 'dark', label: 'Dark' }] });
  ovModeSeg.addEventListener('change', function (e) { ovMode = e.detail.value; renderOverrides(); });
  function groundsFor(role, mode) {
    var chk = M.roleChecks[role];
    if (!chk) return [];
    var pal = M.buildPalette(liveRecord(), mode, { contrast: S.reader.contrast ? 'high' : 'standard' });
    return chk.on.map(function (g) { return { label: GROUND_NAMES[g] || g, hex: pal.hex[g], min: S.reader.contrast ? chk.high : chk.min }; });
  }
  var ovPickers = {};
  function renderOverrides() {
    overridesBox.innerHTML = '';
    ovPickers = {};
    var set = get(S.record, 'palette.overrides.' + ovMode) || {};
    Object.keys(set).forEach(function (role) {
      var path = 'palette.overrides.' + ovMode + '.' + role;
      var cp = M.ColorPicker({ label: ROLE_NAMES[role] || role, value: set[role], against: groundsFor(role, ovMode), help: ' ' });
      cp.addEventListener('input', function (e) { setPath(path, e.detail.value); live(); });
      cp.addEventListener('change', function (e) { setPath(path, e.detail.value); commit(); });
      var rm = M.IconButton({ icon: 'delete', label: 'Remove ' + (ROLE_NAMES[role] || role) });
      rm.addEventListener('click', function () {
        setPath(path, undefined);
        var rest = get(S.record, 'palette.overrides.' + ovMode);
        if (rest && !Object.keys(rest).length) setPath('palette.overrides.' + ovMode, undefined);
        var all = get(S.record, 'palette.overrides');
        if (all && !Object.keys(all).length) setPath('palette.overrides', undefined);
        commit(); renderOverrides();
      });
      ovPickers[role] = cp;
      overridesBox.appendChild(h('div', { 'class': 'tc-override' }, cp, h('div', { style: { 'padding-bottom': '22px' } }, rm)));
    });
    var free = OVERRIDABLE.filter(function (r) { return !set[r]; });
    var addBox = M.Select({ label: Object.keys(set).length ? 'Add another' : 'Set an exact colour', placeholder: 'Choose a role', options: free.map(function (r) { return { value: r, label: ROLE_NAMES[r] || r }; }) });
    addBox.addEventListener('change', function (e) {
      if (!e.detail.value) return;
      var pal = M.buildPalette(liveRecord(), ovMode, { contrast: S.reader.contrast ? 'high' : 'standard' });
      setPath('palette.overrides.' + ovMode + '.' + e.detail.value, pal.hex[e.detail.value]);
      commit(); renderOverrides();
      var cp = ovPickers[e.detail.value]; if (cp) cp.querySelector('.m-colorpicker__trigger').focus();
    });
    overridesBox.appendChild(addBox);
    refreshOverrideNotes();
  }
  function refreshOverrideNotes() {
    var pal = M.buildPalette(liveRecord(), ovMode, { contrast: S.reader.contrast ? 'high' : 'standard' });
    Object.keys(ovPickers).forEach(function (role) {
      var a = (pal.adjustments || []).filter(function (x) { return x.kind === 'override' && x.role === role; })[0];
      ovPickers[role].setHelp(a ? 'Shown as ' + a.to + ': ' + a.text.replace(/^Your [^ ]+( [a-z-]+)? colour was /, '') : 'Used as set.', a ? 'caution' : null);
      ovPickers[role].setAgainst(groundsFor(role, ovMode));
    });
  }
  refreshers.push(renderOverrides);
  derived.push(function () { refreshOverrideNotes(); });

  var SCHEMES = [['neutral', 'Neutral', 'Near-grey surfaces, a quiet accent'], ['tonal', 'Tonal', 'Tinted surfaces, a measured accent'], ['vibrant', 'Vibrant', 'A saturated accent over tinted surfaces'],
    ['expressive', 'Expressive', 'Saturated accent, more colour in the surfaces'], ['fidelity', 'Fidelity', 'Keeps the primary as picked where contrast allows'], ['monochrome', 'Monochrome', 'Greys only; the accent is a weight, not a hue']];
  var nf3 = new Intl.NumberFormat('en', { minimumFractionDigits: 3, maximumFractionDigits: 3 });
  var signed = function (v) { return v > 0 ? '+' + v : v < 0 ? '−' + Math.abs(v) : '0'; };
  var colourSec = h('div', { 'class': 'tc-sec' },
    primary, secondary, neutral,
    resolvedBox, notesBox,
    select('palette.scheme', SCHEMES.map(function (s) { return { value: s[0], label: s[1], description: s[2] }; }), 'Scheme'),
    slider('palette.chroma', { label: 'Chroma ceiling', min: 0, max: 200, step: 5, format: function (v) { return nf3.format(v / 1000); }, help: 'The most colour the accent and surfaces may take, in OKLCH chroma.' }),
    slider('palette.temperature', { label: 'Temperature', min: -100, max: 100, step: 5, format: function (v) { return v < 0 ? 'Cool ' + Math.abs(v) : v > 0 ? 'Warm ' + v : 'Neutral'; }, marks: ['Cool', 'Warm'] }),
    seg('palette.mode', [['', 'Reader’s choice'], ['light', 'Light'], ['dark', 'Dark']], 'Preferred mode', { help: 'Readers who choose “World default” get this mode. Both modes are always built.' }),
    h('div', { 'class': 'tc-pair' },
      slider('palette.depth.light', { label: 'Light depth', min: -2, max: 2, format: signed }),
      slider('palette.depth.dark', { label: 'Dark depth', min: -2, max: 2, format: signed })),
    M.Collapsible({ label: 'Exact colours', content: h('div', { 'class': 'tc-sec', style: { 'padding-top': '10px' } },
      help('Pin a role to a colour. The engine keeps it only while it meets that role’s contrast duty, and otherwise moves its lightness until it does.'),
      ovModeSeg, overridesBox) }));

  function swatchChip(hx, name) { return h('span', { 'class': 'tc-chip', style: { background: hx }, 'data-tip': name + ' ' + hx, role: 'img', 'aria-label': name + ' ' + hx }); }
  function renderResolved() {
    resolvedBox.innerHTML = '';
    notesBox.innerHTML = '';
    var r = liveRecord(), contrast = S.reader.contrast ? 'high' : 'standard';
    var keys = [['surface', 'Page'], ['ink', 'Text'], ['accent', 'Accent'], ['accent-ink', 'Links'], ['ornament', 'Ornament'], ['focus', 'Focus']];
    ['light', 'dark'].forEach(function (mode) {
      var pal = M.buildPalette(r, mode, { contrast: contrast });
      resolvedBox.appendChild(h('div', { 'class': 'tc-resolved__row' }, h('span', null, mode === 'light' ? 'Light' : 'Dark'), keys.map(function (k) { return swatchChip(pal.hex[k[0]], k[1]); })));
      (pal.adjustments || []).filter(function (a) { return a.kind !== 'override'; }).forEach(function (a) {
        notesBox.appendChild(h('li', null,
          a.from ? h('span', { 'class': 'tc-swap', 'aria-hidden': 'true' }, h('i', { style: { background: a.from } }), icon('arrow_forward'), h('i', { style: { background: a.to } })) : h('span', { 'class': 'tc-swap', 'aria-hidden': 'true' }, icon('info')),
          h('span', null, h('b', null, mode === 'light' ? 'Light: ' : 'Dark: '), a.text)));
      });
    });
    notesBox.hidden = !notesBox.children.length;
  }
  derived.push(renderResolved);
  derived.push(function () {
    var sch = val('palette.scheme');
    primary.setHelp(sch === 'fidelity' ? 'Used as picked for the accent wherever contrast allows.' : sch === 'monochrome' ? 'Only its hue tints the greys; the Monochrome scheme has no colour accent.' : 'Its hue and strength set the accent; the engine picks the lightness for each mode.');
  });

  /* surfaces */
  var sheetPick = radios('material.sheet', [['solid', 'Solid', 'Opaque; meets the art with the World’s edge'], ['veil', 'Veil', 'Translucent like Motet’s chrome, 16px blur'], ['frosted', 'Frosted', 'More see-through, 32px blur and fine grain']], 'Content sheet');
  var sheetNote = help('', 'caution');
  derived.push(function () {
    var place = val('imagery.placement'), sheet = val('material.sheet');
    var msg = '';
    if (sheet !== 'solid' && !artUrl()) msg = 'Needs art behind the page; without it the sheet stays solid.';
    else if (sheet !== 'solid' && place === 'header') msg = 'Header art scrolls away, so the sheet stays solid. Use continuous or backdrop placement.';
    else if (sheet !== 'solid') msg = 'Turns solid for readers in high contrast or who reduce transparency.';
    sheetNote.textContent = msg; sheetNote.hidden = !msg;
    sheetNote.setAttribute('data-tone', /stays solid/.test(msg) ? 'caution' : '');
  });
  var surfacesSec = h('div', { 'class': 'tc-sec' },
    seg('material.finish', [['matte', 'Matte'], ['satin', 'Satin'], ['sheen', 'Sheen']], 'Finish'),
    seg('material.grain', [[0, 'None'], [1, 'Fine'], [2, 'Medium'], [3, 'Coarse'], [4, 'Heavy']], 'Grain', { help: 'Grain raises the contrast of secondary text a little to stay readable.' }),
    slider('material.tint', { label: 'Accent tint', min: 0, max: 12, format: function (v) { return v + '%'; }, help: 'Pulls the surfaces toward the accent.' }),
    seg('material.edge', [[0, 'None'], [1, 'Soft'], [2, 'Bright']], 'Edge light'),
    seg('material.shadow', [[0, 'Flat'], [1, 'Soft'], [2, 'Deep']], 'Shadows'),
    toggle('material.mica', 'Mica', 'Tints the surfaces with the art’s dominant colour.'),
    h('div', { 'class': 'tc-field' }, sheetPick, sheetNote));

  /* shape */
  var shapeSec = h('div', { 'class': 'tc-sec' },
    seg('shape.corner', [['round', 'Round'], ['squircle', 'Squircle'], ['bevel', 'Bevel'], ['square', 'Square']], 'Corners', { help: 'Bevel needs a browser with corner-shape; elsewhere it draws as a small round corner.' }),
    slider('shape.scale', { label: 'Corner size', min: 0, max: 150, step: 5, format: function (v) { return v + '%'; } }),
    seg('shape.profile', [['even', 'Even'], ['cut', 'Cut'], ['clipped', 'Clipped']], 'Panel profile', { help: 'Cut and clipped notch the corners of framed panels; controls stay even.' }),
    seg('shape.border', [['none', 'None'], ['hairline', 'Hairline'], ['rule', 'Rule'], ['heavy', 'Heavy']], 'Borders'),
    seg('shape.tone', [['neutral', 'Neutral'], ['ornament', 'Ornament']], 'Border colour'),
    toggle('shape.frame', 'Double frame', 'An inner rule inside framed panels.', { on: 'double', off: 'none' }));

  /* ornament and edges */
  var ornSlot = 'motif';
  var ornText = M.Textarea({ label: 'SVG or path data', rows: 4, maxRows: 10, face: 'ui', placeholder: '<svg viewBox="0 0 64 8"><path d="M0 3h44v2H0z"/></svg>' });
  var ornTa = ornText.querySelector('textarea');
  var ornErr = help('', 'caution'); ornErr.hidden = true;
  var ornPrev = h('div', { 'class': 'tc-orn__preview' });
  var ornApply = M.Button({ variant: 'tonal', size: 'sm', icon: 'check', label: 'Use as motif' });
  var ornClear = M.Button({ variant: 'text', size: 'sm', icon: 'delete', label: 'Remove' });
  var ornSeg = M.Segmented({ label: 'Slot', value: 'motif', items: [{ value: 'motif', label: 'Motif' }, { value: 'corner', label: 'Corner' }, { value: 'flank', label: 'Flank' }] });
  var ORN_HELP = { motif: 'Centred on dividers from level 1.', corner: 'Top-left corner piece, rotated for the others, from level 2.', flank: 'Beside headings from level 1.' };
  var ornSlotHelp = help(ORN_HELP.motif);
  var pendingOrn = null;
  function parseOrnament(text) {
    text = String(text || '').trim();
    if (!text) return { errors: [] };
    var o, skipped = 0;
    if (/^</.test(text)) {
      var doc = new DOMParser().parseFromString(text, 'image/svg+xml');
      var svg = doc.querySelector('svg');
      if (!svg || doc.querySelector('parsererror')) return { errors: ['That isn’t a readable SVG document.'] };
      var vb = (svg.getAttribute('viewBox') || '').trim().split(/[\s,]+/).map(Number);
      var w = vb.length === 4 ? vb[2] : parseFloat(svg.getAttribute('width')), hh = vb.length === 4 ? vb[3] : parseFloat(svg.getAttribute('height'));
      if (!(w > 0 && hh > 0)) return { errors: ['Give the SVG a viewBox or a width and height.'] };
      if (vb.length === 4 && (vb[0] || vb[1])) return { errors: ['The viewBox must start at 0 0.'] };
      var paths = Array.prototype.slice.call(svg.querySelectorAll('path')).map(function (p) {
        var x = { d: (p.getAttribute('d') || '').replace(/\s+/g, ' ').trim() };
        var stroke = p.getAttribute('stroke'), sw = parseFloat(p.getAttribute('stroke-width'));
        if (p.getAttribute('fill') === 'none' || (stroke && stroke !== 'none')) x.s = Math.min(8, sw > 0 ? sw : 1);
        if (p.getAttribute('fill-rule') === 'evenodd') x.rule = 'evenodd';
        return x;
      });
      skipped = svg.querySelectorAll('circle, rect, ellipse, line, polyline, polygon, text, image, use').length;
      o = { vb: [Math.round(w), Math.round(hh)], p: paths };
    } else {
      var ns = 'http://www.w3.org/2000/svg';
      var tmp = document.createElementNS(ns, 'svg'); tmp.setAttribute('width', '0'); tmp.setAttribute('height', '0'); tmp.style.position = 'absolute';
      var path = document.createElementNS(ns, 'path'); path.setAttribute('d', text); tmp.appendChild(path); document.body.appendChild(tmp);
      var bb; try { bb = path.getBBox(); } catch (e) { bb = null; } tmp.remove();
      if (!bb || !(bb.width + bb.x > 0)) return { errors: ['That path doesn’t draw anything.'] };
      o = { vb: [Math.max(1, Math.ceil(bb.x + bb.width)), Math.max(1, Math.ceil(bb.y + bb.height))], p: [{ d: text.replace(/\s+/g, ' ') }] };
    }
    var v = M.validateOrnament(o);
    var errors = v.errors.map(function (e) { return e.replace(/^vb/, 'Size').replace(/^p must/, 'Artwork must').replace(/^path (\d+)/, function (m, i) { return 'Path ' + (+i + 1); }); });
    if (skipped) errors.push(skipped + ' shape' + (skipped > 1 ? 's aren’t' : ' isn’t') + ' a path and was left out. Convert shapes to paths first.');
    return { o: v.ok ? o : null, errors: errors };
  }
  function showOrn(o) {
    ornPrev.innerHTML = '';
    if (!o) { ornPrev.appendChild(h('span', { 'class': 'tc-help' }, 'Paste artwork to see it here.')); return; }
    var uri = 'url("data:image/svg+xml,' + encodeURIComponent(M.ornamentSvg(o)) + '")';
    var scale = Math.min(2, 260 / o.vb[0], 80 / o.vb[1]);
    ornPrev.appendChild(h('div', { 'class': 'tc-orn__art', role: 'img', 'aria-label': 'Ornament preview', style: { '--m': uri, width: Math.round(o.vb[0] * scale) + 'px', height: Math.round(o.vb[1] * scale) + 'px' } }));
  }
  function refreshOrn() {
    var cur = get(S.record, 'ornament.custom.' + ornSlot);
    ornApply.querySelector('span:last-child').textContent = 'Use as ' + ornSlot;
    ornClear.hidden = !cur;
    ornSlotHelp.textContent = ORN_HELP[ornSlot] + (cur ? ' This slot uses your artwork.' : ' This slot uses the set’s artwork.');
    if (!pendingOrn) showOrn(cur || null);
  }
  ornTa.addEventListener('input', function () {
    var r = parseOrnament(ornTa.value);
    pendingOrn = r.o || null;
    ornErr.textContent = r.errors.join(' '); ornErr.hidden = !r.errors.length;
    ornApply.disabled = !pendingOrn;
    showOrn(pendingOrn || get(S.record, 'ornament.custom.' + ornSlot) || null);
  });
  ornSeg.addEventListener('change', function (e) { ornSlot = e.detail.value; pendingOrn = null; ornTa.value = ''; ornErr.hidden = true; ornApply.disabled = true; refreshOrn(); });
  ornApply.disabled = true;
  ornApply.addEventListener('click', function () {
    if (!pendingOrn) return;
    setPath('ornament.custom.' + ornSlot, pendingOrn);
    if (!(val('ornament.level') >= (ornSlot === 'corner' ? 2 : 1))) setPath('ornament.level', ornSlot === 'corner' ? 2 : 1);
    pendingOrn = null; ornTa.value = ''; ornApply.disabled = true;
    commit(); syncControls();
    M.toast({ message: 'Your ' + ornSlot + ' is in use', tone: 'positive' });
  });
  ornClear.addEventListener('click', function () {
    setPath('ornament.custom.' + ornSlot, undefined);
    var cu = get(S.record, 'ornament.custom'); if (cu && !Object.keys(cu).length) setPath('ornament.custom', undefined);
    commit(); syncControls();
  });
  refreshers.push(refreshOrn);
  var ornamentSec = h('div', { 'class': 'tc-sec' },
    seg('ornament.set', [['rule', 'Rule'], ['fade', 'Fade'], ['filigree', 'Filigree'], ['segment', 'Segment']], 'Set'),
    seg('ornament.level', [[0, 'Off'], [1, 'Dividers'], [2, 'Surfaces'], [3, 'Rich']], 'Level', { help: 'Dividers adds the motif and heading flanks; Surfaces adds corner pieces or the segment mark to framed panels.' }),
    select('ornament.edge', [['fade', 'Fade', 'Dissolves into the page across the full width'], ['straight', 'Straight', 'A clean edge; the art shows in the margins'], ['torn', 'Torn', 'Ragged paper'], ['burned', 'Burned', 'A scorched, uneven edge'], ['arch', 'Arch', 'A shallow arch, like a chapter opening']].map(function (e) { return { value: e[0], label: e[1], description: e[2] }; }), 'Where art meets the page'),
    toggle('ornament.dropCap', 'Drop cap', 'On the first paragraph of each scene’s opening post.'),
    M.Collapsible({ label: 'Your own artwork', content: h('div', { 'class': 'tc-orn', style: { 'padding-top': '10px' } },
      help('Single-colour vector paths only, drawn in the ornament colour. Up to 16 paths in a box of at most 512 by 512.'),
      ornSeg, ornSlotHelp, ornText, ornErr, ornPrev, h('div', { 'class': 'tc-row' }, ornApply, ornClear)) }));

  /* type */
  var faceSelect = select('type.display', Object.keys(FACE_NAMES).map(function (k) { return { value: k, label: FACE_NAMES[k] }; }), 'Display face', 'Titles, scene headings and, if chosen, character names. All faces cover Polish.');
  faceSelect.querySelectorAll('.m-listbox__option').forEach(function (o) { var f = M.faces[o.getAttribute('data-value')]; if (f) { var l = o.querySelector('.m-listbox__optionLabel'); l.style.fontFamily = f.stack; l.style.fontVariationSettings = f.axes; l.style.fontSize = Math.round(15 * f.adjust) + 'px'; } });
  var typeSec = h('div', { 'class': 'tc-sec' },
    faceSelect,
    seg('type.case', [['none', 'As written'], ['upper', 'Uppercase']], 'Display case'),
    h('div', { 'class': 'tc-pair' },
      slider('type.weight', { label: 'Weight', min: 300, max: 900, step: 50 }),
      slider('type.tracking', { label: 'Tracking', min: -40, max: 160, step: 5, format: function (v) { return (v / 1000).toFixed(3).replace(/0+$/, '').replace(/\.$/, '') + ' em'; } })),
    seg('type.names', [['ui', 'Interface face'], ['display', 'Display face']], 'Character names'),
    seg('type.reading', [['serif', 'Serif'], ['sans', 'Sans']], 'Reading face', { help: 'Readers can still choose their own reading face.' }),
    seg('type.numerals', [['proportional', 'Proportional'], ['tabular', 'Tabular'], ['oldstyle', 'Oldstyle']], 'Numerals'),
    seg('type.paragraphs', [['spaced', 'Spaced'], ['indented', 'Indented']], 'Paragraphs'));

  /* imagery */
  var artGrid = h('div', { 'class': 'tc-art', role: 'group', 'aria-label': 'Art' });
  var artTiles = [];
  function artTile(a, text, child) {
    var b = h('button', { 'class': 'tc-art__tile', type: 'button', 'aria-pressed': 'false' }, h('span', { 'class': 'tc-art__img' }, child), h('span', { 'class': 'tc-art__label' }, text));
    b.addEventListener('click', function () { setArt(a); });
    artTiles.push({ el: b, art: a });
    return b;
  }
  artGrid.appendChild(artTile(null, 'None', icon('hide_image')));
  D.worlds.forEach(function (w) { artGrid.appendChild(artTile({ kind: 'sample', key: w.key }, presetName(w.key), h('img', { src: D.art[w.key], alt: '' }))); });
  var fileInput = h('input', { type: 'file', accept: 'image/png,image/jpeg,image/webp', tabindex: '-1', 'aria-hidden': 'true' });
  var uploadImg = h('span', { 'class': 'tc-art__img' }, icon('add_photo_alternate'));
  var uploadLabel = h('span', { 'class': 'tc-art__label' }, 'Upload');
  var uploadTile = h('button', { 'class': 'tc-art__tile', type: 'button', 'aria-pressed': 'false' }, uploadImg, uploadLabel, fileInput);
  uploadTile.addEventListener('click', function (e) { if (e.target !== fileInput) fileInput.click(); });
  fileInput.addEventListener('change', function () { var f = fileInput.files && fileInput.files[0]; fileInput.value = ''; if (f) uploadArt(f); });
  artGrid.appendChild(uploadTile);
  ['dragover', 'drop'].forEach(function (t) {
    artGrid.addEventListener(t, function (e) {
      e.preventDefault();
      if (t === 'drop') { var f = e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files[0]; if (f) uploadArt(f); }
    });
  });
  function sameArt(a, b) { return (!a && !b) || (a && b && a.kind === b.kind && (a.key || a.id || a.url) === (b.key || b.id || b.url)); }
  function refreshArtTiles() {
    var custom = S.art && S.art.kind !== 'sample';
    artTiles.forEach(function (t) { t.el.setAttribute('aria-pressed', String(sameArt(t.art, S.art))); });
    uploadTile.setAttribute('aria-pressed', String(!!custom));
    uploadImg.innerHTML = '';
    if (custom) { uploadImg.appendChild(h('img', { src: artUrl(), alt: '' })); uploadLabel.textContent = S.art.name || 'Your image'; }
    else { uploadImg.appendChild(icon('add_photo_alternate')); uploadLabel.textContent = 'Upload'; }
    var url = artUrl();
    focusBox.style.setProperty('--tc-art', url ? 'url("' + url.replace(/"/g, '%22') + '")' : 'none');
  }
  function setArt(a) {
    S.art = a;
    if (!a) { if (S.record.imagery) { delete S.record.imagery.dominant; delete S.record.imagery.range; } }
    else if (!get(S.record, 'imagery.placement')) setPath('imagery.placement', 'continuous');
    commit(); syncControls();
    measureArt();
  }
  var uploading = false;
  function uploadArt(file) {
    if (uploading) return;
    if (!/^image\/(png|jpeg|webp)$/.test(file.type)) { M.toast({ tone: 'danger', message: 'Choose a PNG, JPEG or WebP image.' }); return; }
    if (file.size > 12 * 1048576) { M.toast({ tone: 'danger', message: 'Choose an image under 12 MB.' }); return; }
    cap('assets').then(function (assets) {
      if (!assets) {
        setArt({ kind: 'local', url: URL.createObjectURL(file), name: file.name, type: file.type });
        M.toast({ tone: 'info', message: 'This image stays in this browser tab. Keeping it with a saved theme needs edit access to this page.' });
        return;
      }
      uploading = true;
      uploadImg.innerHTML = ''; uploadImg.appendChild(M.Spinner({ label: 'Uploading' })); uploadLabel.textContent = 'Uploading';
      assets.upload(file, { type: file.type }).then(function (res) {
        uploading = false;
        setArt({ kind: 'asset', id: res.id, type: res.contentType, name: file.name });
        M.toast({ tone: 'positive', message: 'Image uploaded' });
      }, function (err) {
        uploading = false; refreshArtTiles();
        var code = err && err.code;
        var msg = code === 'too_large' ? 'That image is too large to store here.' : code === 'quota_or_state' ? 'This page’s storage is full. Remove unused images first.' :
          code === 'unsupported_type' ? 'Choose a PNG, JPEG or WebP image.' : code === 'rate_limited' ? 'Too many uploads at once. Try again in a moment.' : 'The image didn’t upload. Try again.';
        M.toast({ tone: 'danger', message: msg });
      });
    });
  }
  /* Publishing measures the art once: the dominant colour (for mica) and the darkest and lightest
     colours after a 32px blur (so a translucent sheet is only as clear as this art allows). */
  var measureSeq = 0;
  function measureArt() {
    var url = artUrl(), seq = ++measureSeq;
    if (!url) { scheduleDerived(); return; }
    var img = new Image();
    img.src = url;
    (img.decode ? img.decode() : new Promise(function (ok, no) { img.onload = ok; img.onerror = no; })).then(function () {
      if (seq !== measureSeq) return;
      var W = 320, H = 180, c = document.createElement('canvas'); c.width = W; c.height = H;
      var x = c.getContext('2d', { willReadFrequently: true });
      var iw = img.naturalWidth || 1600, ih = img.naturalHeight || 900, sc = Math.max(W / iw, H / ih);
      x.filter = 'blur(7px)';
      x.drawImage(img, (W - iw * sc) / 2 - 16, (H - ih * sc) / 2 - 16, iw * sc + 32, ih * sc + 32);
      var d = x.getImageData(8, 8, W - 16, H - 16).data, lo = [1e9], hi = [-1], sum = [0, 0, 0], n = 0;
      for (var i = 0; i < d.length; i += 16) {
        var L = 0.2126 * d[i] + 0.7152 * d[i + 1] + 0.0722 * d[i + 2];
        if (L < lo[0]) lo = [L, d[i], d[i + 1], d[i + 2]];
        if (L > hi[0]) hi = [L, d[i], d[i + 1], d[i + 2]];
        sum[0] += d[i] * d[i]; sum[1] += d[i + 1] * d[i + 1]; sum[2] += d[i + 2] * d[i + 2]; n++;
      }
      var hx = function (a) { return '#' + a.map(function (v) { return Math.round(v).toString(16).padStart(2, '0'); }).join(''); };
      setPath('imagery.range', [hx(lo.slice(1)), hx(hi.slice(1))]);
      setPath('imagery.dominant', hx(sum.map(function (s) { return Math.sqrt(s / n); })));
      lastSnap = snapshot();
      saveDraft(); updateFrame(); scheduleDerived();
    }).catch(function () {
      if (seq !== measureSeq) return;
      M.toast({ tone: 'caution', message: 'Couldn’t read this image’s colours. Translucent sheets will assume black and white behind them.' });
    });
  }
  var focusMark = h('span', { 'class': 'tc-focus__mark' });
  var focusBox = h('div', { 'class': 'tc-focus', 'aria-hidden': 'true' }, focusMark);
  function setFocus(fx, fy, done) { setPath('imagery.focus', [Math.round(fx), Math.round(fy)]); refreshFocus(); if (done) commit(); else live(); }
  function refreshFocus() { var f = val('imagery.focus') || [50, 50]; focusMark.style.left = f[0] + '%'; focusMark.style.top = f[1] + '%'; fxS.setValue(f[0]); fyS.setValue(f[1]); }
  function fromPointer(e, done) {
    var r = focusBox.getBoundingClientRect();
    setFocus(Math.max(0, Math.min(100, (e.clientX - r.left) / r.width * 100)), Math.max(0, Math.min(100, (e.clientY - r.top) / r.height * 100)), done);
  }
  focusBox.addEventListener('pointerdown', function (e) { focusBox.setPointerCapture(e.pointerId); fromPointer(e); });
  focusBox.addEventListener('pointermove', function (e) { if (focusBox.hasPointerCapture(e.pointerId)) fromPointer(e); });
  focusBox.addEventListener('pointerup', function (e) { if (focusBox.hasPointerCapture(e.pointerId)) { focusBox.releasePointerCapture(e.pointerId); fromPointer(e, true); } });
  var fxS = M.Slider({ label: 'Focus across', min: 0, max: 100, value: 50, format: function (v) { return v + '%'; } });
  var fyS = M.Slider({ label: 'Focus down', min: 0, max: 100, value: 50, format: function (v) { return v + '%'; } });
  [[fxS, 0], [fyS, 1]].forEach(function (p) {
    p[0].addEventListener('input', function (e) { if (syncing) return; var f = (val('imagery.focus') || [50, 50]).slice(); f[p[1]] = e.detail.value; setFocus(f[0], f[1]); });
    p[0].addEventListener('change', function () { if (!syncing) commit(); });
  });
  refreshers.push(refreshFocus, refreshArtTiles);
  var measured = h('div', { 'class': 'tc-measured' });
  derived.push(function () {
    measured.innerHTML = '';
    var dm = get(S.record, 'imagery.dominant'), rg = get(S.record, 'imagery.range');
    if (!artUrl() || !rg) { measured.hidden = true; return; }
    measured.hidden = false;
    [['Dominant', dm], ['Darkest', rg[0]], ['Lightest', rg[1]]].forEach(function (x) { if (x[1]) measured.appendChild(h('span', null, h('i', { style: { background: x[1] } }), x[0], h('code', null, x[1]))); });
  });
  var pctOff = function (v) { return v ? v + '%' : 'Off'; };
  var backingSeg = seg('imagery.titleBacking', [['shade', 'Shade'], ['halo', 'Halo'], ['none', 'None']], 'Behind the title',
    { help: 'Shade tints the art under the title just enough for 4.6:1, and not at all where the art already contrasts. Halo outlines the letters only. None holds only while the art itself gives 4.6:1.' });
  derived.push(function () {
    var on = val('imagery.title') === 'art' && !!artUrl();
    backingSeg.hidden = !on;
    if (!on) return;
    var out = M.compileWorld(liveRecord(), { mode: previewMode(), supports: { backdrop: true, cornerShape: true } });
    var a = (out.adjustments || []).filter(function (x) { return x.role === 'title'; })[0];
    var hp = backingSeg.helpEl;
    hp.textContent = a ? a.text : 'Shade tints the art under the title just enough for 4.6:1, and not at all where the art already contrasts. Halo outlines the letters only. None holds only while the art itself gives 4.6:1.';
    hp.setAttribute('data-tone', a ? 'caution' : '');
  });
  var imagerySec = h('div', { 'class': 'tc-sec' },
    h('div', { 'class': 'tc-field' }, label('Art'), artGrid, help('One image per World: behind the page, at its top, or both. Drop a file on the grid to upload.')),
    radios('imagery.placement', [['continuous', 'Continuous', 'Fixed behind the page; seen in full at the top and in the margins'], ['backdrop', 'Backdrop', 'Behind the page; a band shows under the chrome'], ['header', 'Header', 'At the top of the page; scrolls away and ends in the World’s edge']], 'Placement'),
    h('div', { 'class': 'tc-field' }, label('Focus'), focusBox, h('div', { 'class': 'tc-pair' }, fxS, fyS), help('The part of the image that stays in view on narrow screens.')),
    seg('imagery.title', [['page', 'On the page'], ['art', 'On the art'], ['hidden', 'Hidden']], 'World title', { help: 'On the art stands the mark, title and byline on the art above the edge. Hidden is for art that carries the World’s own wordmark: mark and title leave the screen, screen readers still get the title, and both come back on phones and whenever the art isn’t shown.' }),
    backingSeg,
    h('p', { 'class': 'tc-sub' }, 'Treatments'),
    help('They combine; 0% is off.'),
    slider('imagery.treatments.veil', { label: 'Veil', min: 0, max: 100, step: 5, format: pctOff, help: 'Lays the page colour over the art.' }),
    slider('imagery.treatments.vignette', { label: 'Vignette', min: 0, max: 100, step: 5, format: pctOff, help: 'Fades the edges into the page.' }),
    slider('imagery.treatments.duotone', { label: 'Duotone', min: 0, max: 100, step: 5, format: pctOff, help: 'Recolours the art in the accent and ornament hues.' }),
    slider('imagery.treatments.blur', { label: 'Blur', min: 0, max: 100, step: 5, format: pctOff, help: 'Softens detail, up to 24px.' }),
    slider('imagery.treatments.parallax', { label: 'Parallax', min: 0, max: 100, step: 5, format: pctOff, help: 'The art drifts as the page scrolls: fixed art rises at up to half its speed, header art scrolls away at down to half. Not shown to readers who reduce motion.' }),
    measured);

  /* composition */
  var plateSeg = seg('composition.plate', [['ink', 'Ink'], ['accent', 'Accent']], 'Plate colour');
  derived.push(function () { plateSeg.hidden = val('composition.names') !== 'plate'; });
  var compositionSec = h('div', { 'class': 'tc-sec' },
    radios('composition.post', [['open', 'Open', 'Prose on the page, divided by the World’s divider'], ['card', 'Card', 'Each post framed in the World’s profile'], ['dossier', 'Dossier', 'Filed as a document with a form header strip'], ['log', 'Log', 'A transcript in ruled rows, speaker in the accent']], 'Posts', 'Spacing between posts follows density.'),
    seg('composition.portrait', [['gutter', 'Gutter'], ['inline', 'Inline'], ['none', 'None']], 'Portraits'),
    seg('composition.names', [['plain', 'Plain'], ['plate', 'Name plate']], 'Speaker names'),
    plateSeg,
    seg('density', [['comfortable', 'Comfortable'], ['standard', 'Standard'], ['compact', 'Compact']], 'Density', { help: 'Readers on touch screens always get 44px targets.' }));

  /* motion and icons */
  var replay = M.Button({ variant: 'text', size: 'sm', icon: 'replay', label: 'Replay the page' });
  replay.addEventListener('click', function () { renderFrame(true); });
  var motionSec = h('div', { 'class': 'tc-sec' },
    radios('motion.character', [['standard', 'Standard', 'Quick springs with a trace of bounce'], ['crisp', 'Crisp', 'Shorter and exact; no bounce'], ['soft', 'Soft', 'Slower, rounder springs for decorative fades'], ['eerie', 'Eerie', 'Functional motion stays quick; decoration lingers']], 'Motion', 'Motion that carries meaning never takes longer than 420 ms. Readers who reduce motion get none.'),
    replay,
    seg('icons.style', [['outlined', 'Outlined'], ['rounded', 'Rounded'], ['sharp', 'Sharp']], 'Icons'),
    slider('icons.weight', { label: 'Icon weight', min: 200, max: 600, step: 100 }));

  /* chrome */
  var chromeSec = h('div', { 'class': 'tc-sec' },
    slider('chrome.tint', { label: 'Chrome tint', min: 0, max: 8, format: function (v) { return v + '%'; }, help: 'Motet’s corner islands keep their layout, type and colours; a World may tint their glass a little toward its accent.' }));

  var SUMMARY = {
    colour: function () { var s = SCHEMES.filter(function (x) { return x[0] === val('palette.scheme'); })[0]; return (s ? s[1] : '') + ', ' + nf3.format(val('palette.chroma') / 1000); },
    surfaces: function () { return cap1(val('material.finish')) + ', ' + val('material.sheet') + ' sheet'; },
    shape: function () { return cap1(val('shape.corner')) + ', ' + val('shape.border'); },
    ornament: function () { var l = val('ornament.level'); return cap1(val('ornament.set')) + (l ? ', level ' + l : ', off'); },
    type: function () { return FACE_NAMES[val('type.display')] || ''; },
    imagery: function () {
      if (!artUrl()) return 'No art';
      var on = M.treatments.filter(function (k) { return val('imagery.treatments.' + k); });
      return cap1(val('imagery.placement')) + (on.length ? ', ' + on.join(' + ') : '');
    },
    composition: function () { return cap1(val('composition.post')) + ', ' + val('density'); },
    motion: function () { return cap1(val('motion.character')) + ', ' + val('icons.style'); },
    chrome: function () { return 'Tint ' + val('chrome.tint') + '%'; }
  };
  function cap1(s) { s = String(s || ''); return s.charAt(0).toUpperCase() + s.slice(1); }
  var acc = M.Accordion({ multiple: true, items: [
    { title: 'Colour', icon: 'palette', content: colourSec, open: true },
    { title: 'Surfaces', icon: 'layers', content: surfacesSec },
    { title: 'Shape', icon: 'rounded_corner', content: shapeSec },
    { title: 'Ornament and edges', icon: 'draw', content: ornamentSec },
    { title: 'Type', icon: 'text_fields', content: typeSec },
    { title: 'Imagery', icon: 'image', content: imagerySec },
    { title: 'Composition', icon: 'view_agenda', content: compositionSec },
    { title: 'Motion and icons', icon: 'animation', content: motionSec },
    { title: 'Platform chrome', icon: 'web_asset', content: chromeSec }
  ].map(function (it) { it.meta = ' '; return it; }) });
  panel.appendChild(acc);
  var metaEls = acc.querySelectorAll('.m-accordion__meta');
  var SUM_KEYS = ['colour', 'surfaces', 'shape', 'ornament', 'type', 'imagery', 'composition', 'motion', 'chrome'];
  derived.push(function () { SUM_KEYS.forEach(function (k, i) { if (metaEls[i]) metaEls[i].textContent = SUMMARY[k](); }); });

  /* ---------------------------------------------------------------- bar */
  var bar = $('barActions');
  var undoBtn = M.IconButton({ icon: 'undo', label: 'Undo' }), redoBtn = M.IconButton({ icon: 'redo', label: 'Redo' });
  undoBtn.addEventListener('click', undo); redoBtn.addEventListener('click', redo);
  var libBtn = M.Button({ variant: 'text', icon: 'folder_open', label: 'Library' });
  var exportBtn = M.Button({ variant: 'text', icon: 'ios_share', label: 'Export' });
  var saveBtn = M.Button({ variant: 'filled', icon: 'cloud_upload', label: 'Save' });
  saveBtn.hidden = true;
  bar.appendChild(undoBtn); bar.appendChild(redoBtn); bar.appendChild(h('span', { 'class': 'm-chrome__sep', 'aria-hidden': 'true' }));
  bar.appendChild(libBtn); bar.appendChild(exportBtn); bar.appendChild(saveBtn);
  var dbRef = null;
  function refreshBar() {
    undoBtn.disabled = !past.length; redoBtn.disabled = !future.length;
    $('docName').textContent = S.name || 'Untitled theme';
    var st = $('docState'); st.innerHTML = '';
    var snap = snapshot();
    var state = !dbRef ? ['smartphone', 'Draft on this device'] : !S.docId ? ['draft', 'Not in the library yet'] : S.savedSnap === snap ? ['cloud_done', 'Saved to the library'] : ['draft', 'Changes not saved'];
    st.appendChild(icon(state[0])); st.appendChild(h('span', null, state[1]));
    saveBtn.disabled = !!(dbRef && S.docId && S.savedSnap === snap);
  }

  /* ---------------------------------------------------------------- toolbar */
  var tb = $('toolbar');
  var viewSeg = M.Segmented({ label: 'View', value: S.view, items: [{ value: 'scene', label: 'Scene' }, { value: 'front', label: 'Front page' }, { value: 'components', label: 'Components' }, { value: 'palette', label: 'Palette' }] });
  viewSeg.addEventListener('change', function (e) { S.view = e.detail.value; saveDraft(); renderFrame(true); });
  var modeSeg = M.Segmented({ label: 'Mode', value: S.mode, items: [{ value: 'world', label: 'World' }, { value: 'light', label: 'Light' }, { value: 'dark', label: 'Dark' }] });
  modeSeg.addEventListener('change', function (e) { S.mode = e.detail.value; saveDraft(); updateFrame(); });
  var devSeg = M.Segmented({ label: 'Device', value: S.device, items: [{ value: 'desktop', label: 'Desktop', icon: 'desktop_windows' }, { value: 'phone', label: 'Phone', icon: 'smartphone' }] });
  devSeg.addEventListener('change', function (e) { S.device = e.detail.value; saveDraft(); layoutDevice(); });

  function readerRow(key, text, sub, invert) {
    var sw = M.Switch({ checked: invert ? !S.reader[key] : S.reader[key], label: text });
    sw.setAttribute('data-m-ready', '');
    sw.addEventListener('click', function () {
      var on = sw.getAttribute('aria-checked') !== 'true';
      sw.setAttribute('aria-checked', String(on));
      S.reader[key] = invert ? !on : on;
      updateFrame(); scheduleDerived(); refreshOverrideNotes();
    });
    return h('div', { 'class': 'tc-switch' }, h('div', { 'class': 'tc-switch__text' }, h('span', { 'class': 'tc-switch__label', 'aria-hidden': 'true' }, text), help(sub)), sw);
  }
  var readerPop = M.Popover({ trigger: M.Button({ variant: 'text', icon: 'tune', label: 'Reader' }), title: 'Preview as a reader', align: 'end', width: 340,
    content: h('div', { 'class': 'tc-pop' },
      readerRow('contrast', 'High contrast', 'Contrast themes raise every duty and make sheets solid.'),
      readerRow('decoration', 'World decoration', 'Readers can turn off ornaments, art and grain.', false),
      readerRow('reduceTransparency', 'Reduce transparency', 'Chrome and sheets turn solid.'),
      readerRow('plain', 'Motet’s plain look', 'Shows the World without its theme.')) });
  var checksBody = h('div', { 'class': 'tc-pop' });
  var checksBtn = M.Button({ variant: 'text', icon: 'fact_check', label: 'Checks' });
  var checksCount = h('span', { 'class': 'tc-count', 'aria-hidden': 'true', hidden: true });
  checksBtn.appendChild(checksCount);
  var checksPop = M.Popover({ trigger: checksBtn, title: 'Contrast and adjustments', align: 'end', width: 380, content: checksBody });
  function renderChecks() {
    checksBody.innerHTML = '';
    var r = liveRecord(), high = S.reader.contrast, notes = 0;
    ['light', 'dark'].forEach(function (mode) {
      var pal = M.buildPalette(r, mode, { contrast: high ? 'high' : 'standard' });
      var fails = Object.keys(M.roleChecks).filter(function (role) {
        var chk = M.roleChecks[role], min = high ? chk.high : chk.min;
        return Math.min.apply(null, chk.on.map(function (g) { return C.contrast(pal.hex[role], pal.hex[g]); })) < min - 0.005;
      });
      var total = Object.keys(M.roleChecks).length;
      checksBody.appendChild(h('div', { 'class': 'tc-checks__mode', 'data-fail': fails.length ? '' : null },
        icon(fails.length ? 'error' : 'check_circle'),
        h('span', null, h('b', null, mode === 'light' ? 'Light: ' : 'Dark: '), (total - fails.length) + ' of ' + total + ' contrast duties pass' + (fails.length ? '; ' + fails.map(function (f) { return ROLE_NAMES[f] || f; }).join(', ') + ' below' : ''))));
      var list = h('ul', { 'class': 'tc-notes' }, (pal.adjustments || []).map(function (a) {
        notes++;
        return h('li', null,
          a.from ? h('span', { 'class': 'tc-swap', 'aria-hidden': 'true' }, h('i', { style: { background: a.from } }), icon('arrow_forward'), h('i', { style: { background: a.to } })) : h('span', { 'class': 'tc-swap', 'aria-hidden': 'true' }, icon('info')),
          h('span', null, a.text));
      }));
      if (list.children.length) checksBody.appendChild(list);
    });
    if (!notes) checksBody.appendChild(help('The engine used every colour as the record describes it.'));
    var open = M.Button({ variant: 'tonal', size: 'sm', icon: 'palette', label: 'Open the palette view' });
    open.addEventListener('click', function () { checksPop.close(true); S.view = 'palette'; setSeg(viewSeg, 'palette'); saveDraft(); renderFrame(true); });
    checksBody.appendChild(open);
    checksCount.textContent = notes ? String(notes) : '';
    checksCount.hidden = !notes;
    checksBtn.setAttribute('aria-label', notes ? 'Checks, ' + notes + (notes === 1 ? ' adjustment' : ' adjustments') : 'Checks');
  }
  derived.push(renderChecks);
  tb.appendChild(viewSeg); tb.appendChild(modeSeg); tb.appendChild(devSeg);
  tb.appendChild(h('div', { 'class': 'tc-toolbar__end' }, zoomLabel, checksPop, readerPop));

  /* ---------------------------------------------------------------- export */
  var exportKind = 'record';
  var exportCode = h('pre', { 'class': 'tc-code', tabindex: '0', 'aria-label': 'Exported JSON' });
  var exportNote = help('');
  var EXPORT_NOTES = {
    record: 'The at.motet.theme record your repository keeps. The art travels as a blob; its CID is computed from the image.',
    standard: 'A place.mode.standard.theme in the World’s preferred mode, so other mode-aware apps show its colours and type.',
    dtcg: 'Resolved light and dark roles, radii and durations as DTCG 2025.10 tokens for design tools.'
  };
  var exportSeg = M.Segmented({ label: 'Format', value: 'record', items: [{ value: 'record', label: 'Motet record' }, { value: 'standard', label: 'standard.mode' }, { value: 'dtcg', label: 'DTCG' }] });
  exportSeg.addEventListener('change', function (e) { exportKind = e.detail.value; fillExport(); });
  var copyBtn = M.Button({ variant: 'tonal', icon: 'content_copy', label: 'Copy' });
  var dlBtn = M.Button({ variant: 'filled', icon: 'download', label: 'Download' });
  var exportDrawer = M.Drawer({ title: 'Export', content: h('div', { 'class': 'tc-drawer-body' }, h('div', { 'class': 'tc-field' }, exportSeg, exportNote), exportCode), footer: h('div', { 'class': 'tc-row' }, copyBtn, dlBtn) });
  function slug(s) { return String(s || 'theme').toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ł/g, 'l').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'theme'; }
  function b32(bytes) {
    var a = 'abcdefghijklmnopqrstuvwxyz234567', out = '', bits = 0, v = 0;
    for (var i = 0; i < bytes.length; i++) { v = ((v << 8) | bytes[i]) & 0xfff; bits += 8; while (bits >= 5) { out += a[(v >>> (bits - 5)) & 31]; bits -= 5; } }
    if (bits > 0) out += a[(v << (5 - bits)) & 31];
    return out;
  }
  var blobCache = {};
  /* An ATProto blob ref: CIDv1, raw codec, sha2-256 of the bytes, base32. */
  function blobRef() {
    var url = artUrl();
    if (!url) return Promise.resolve(null);
    if (blobCache[url]) return blobCache[url];
    blobCache[url] = fetch(url).then(function (r) { return r.blob(); }).then(function (b) {
      return b.arrayBuffer().then(function (buf) { return crypto.subtle.digest('SHA-256', buf); }).then(function (dg) {
        var bytes = [0x01, 0x55, 0x12, 0x20].concat(Array.prototype.slice.call(new Uint8Array(dg)));
        var type = (S.art && S.art.type) || b.type || (/^data:image\/svg/.test(url) ? 'image/svg+xml' : 'application/octet-stream');
        return { $type: 'blob', ref: { $link: 'b' + b32(bytes) }, mimeType: type.split(';')[0], size: b.size };
      });
    }).catch(function () { return { $type: 'blob', mimeType: (S.art && S.art.type) || 'image/png' }; });
    return blobCache[url];
  }
  var ORDER = ['palette', 'material', 'shape', 'ornament', 'type', 'icons', 'motion', 'density', 'composition', 'imagery', 'chrome'];
  function exportRecord(ref) {
    var r = clone(S.record), out = { $type: 'at.motet.theme', name: S.name || 'Untitled theme' };
    ORDER.forEach(function (k) { if (r[k] !== undefined) out[k] = r[k]; });
    if (ref) { out.imagery = out.imagery || {}; out.imagery.art = ref; }
    out.engine = ENGINE;
    out.createdAt = new Date().toISOString();
    return out;
  }
  var exportText = '';
  function fillExport() {
    exportNote.textContent = EXPORT_NOTES[exportKind];
    var write = function (obj) { exportText = JSON.stringify(obj, null, 2); exportCode.textContent = exportText; };
    if (exportKind === 'record') { write(exportRecord(artUrl() ? { $type: 'blob', mimeType: '…' } : null)); blobRef().then(function (ref) { if (exportKind === 'record') write(exportRecord(ref)); }); }
    else if (exportKind === 'standard') write(M.toStandardMode(liveRecord(), { mode: (S.record.palette && S.record.palette.mode) || 'light', description: 'Compiled from the Motet theme “' + (S.name || 'Untitled') + '”' }));
    else write(M.toDTCG(liveRecord()));
  }
  copyBtn.addEventListener('click', function () {
    var done = function () { M.toast({ tone: 'positive', message: 'Copied' }); };
    var fallback = function () {
      var range = document.createRange(); range.selectNodeContents(exportCode);
      var sel = getSelection(); sel.removeAllRanges(); sel.addRange(range);
      M.toast({ tone: 'info', message: 'Copying isn’t allowed here. The text is selected; copy it with your keyboard.' });
    };
    try { navigator.clipboard.writeText(exportText).then(done, fallback); } catch (e) { fallback(); }
  });
  dlBtn.addEventListener('click', function () {
    cap('downloads').then(function (dl) {
      if (!dl) return;
      var name = slug(S.name) + (exportKind === 'record' ? '.motet-theme.json' : exportKind === 'standard' ? '.standard-mode.json' : '.tokens.json');
      dl.save({ filename: name, data: exportText }).then(function (r) {
        if (r && r.status === 'saved') M.toast({ tone: 'positive', message: 'Downloaded ' + name });
      }, function (err) {
        var code = err && err.code;
        if (code === 'declined') return;
        M.toast({ tone: code === 'rate_limited' ? 'info' : 'danger', message: code === 'rate_limited' ? 'A download is already waiting for you to confirm.' : 'Downloads aren’t available here. Copy the JSON instead.' });
      });
    });
  });
  M.enhance(exportDrawer);
  exportBtn.addEventListener('click', function () { fillExport(); exportDrawer.openModal(); });

  /* ---------------------------------------------------------------- library */
  var savedList = h('ul', { 'class': 'tc-saved' });
  var savedNote = h('div');
  var savedDocs = [];
  var importText = M.Textarea({ label: 'Paste a Motet record', rows: 4, maxRows: 10, face: 'ui', placeholder: '{ "$type": "at.motet.theme", … }' });
  var importTa = importText.querySelector('textarea');
  importTa.style.fontFamily = 'var(--font-mono)'; importTa.style.fontSize = '12px';
  var importBtn = M.Button({ variant: 'tonal', icon: 'input', label: 'Import' });
  var importMsg = help('');
  importMsg.hidden = true;
  function strip(rec) {
    var r = clone(rec || {});
    ORDER.forEach(function (k) { if (r[k] && typeof r[k] === 'object' && !Array.isArray(r[k]) && !Object.keys(r[k]).length) delete r[k]; });
    return r;
  }
  function presetStrip(rec) {
    var pal = M.buildPalette(rec, (rec.palette && rec.palette.mode) || 'light');
    return h('div', { 'class': 'tc-strip', 'aria-hidden': 'true' }, ['canvas', 'surface', 'accent', 'ornament', 'ink'].map(function (k) { return h('span', { style: { background: pal.hex[k] } }); }));
  }
  var presetGrid = h('div', { 'class': 'tc-presets' }, D.worlds.map(function (w) {
    var b = h('button', { 'class': 'tc-preset', type: 'button' }, presetStrip(sanitize(w.record).record), h('span', null, presetName(w.key)), h('small', null, PRESET_NOTES[w.key]));
    b.addEventListener('click', function () {
      applyPreset(w.key); S.docId = null; S.savedSnap = null;
      commit(); syncControls(); renderFrame(true); measureArt();
      libDrawer.close();
      M.toast({ message: 'New theme from ' + presetName(w.key), action: { label: 'Undo', onClick: undo }, duration: 6000 });
    });
    return b;
  }));
  var relTime = function (iso) {
    var t = Date.parse(iso); if (!t) return '';
    var s = (Date.now() - t) / 1000, rtf = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });
    if (s < 60) return 'just now';
    if (s < 3600) return rtf.format(-Math.round(s / 60), 'minute');
    if (s < 86400) return rtf.format(-Math.round(s / 3600), 'hour');
    return new Date(t).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' });
  };
  function renderSaved() {
    savedList.innerHTML = '';
    savedList.hidden = !savedDocs.length;
    savedNote.innerHTML = '';
    if (!dbRef) { savedNote.appendChild(M.Alert({ tone: 'info', title: 'The shared library isn’t available here', description: 'Your draft stays in this browser. Export the record to keep a copy.' })); return; }
    if (!savedDocs.length) { savedNote.appendChild(help('Nothing saved yet. Saved themes are shared with everyone who can open this page.')); return; }
    savedDocs.forEach(function (d) {
      var data = d.data, rec = sanitize(migrate(data.record, data.engine)).record;
      var row = h('li', { 'data-current': d.id === S.docId ? '' : null });
      var open = M.Button({ variant: 'text', size: 'sm', label: 'Open' });
      var dup = M.IconButton({ icon: 'content_copy', label: 'Duplicate ' + data.name, size: 'sm' });
      var del = M.IconButton({ icon: 'delete', label: 'Delete ' + data.name, size: 'sm' });
      open.addEventListener('click', function () { openSaved(d, false); });
      dup.addEventListener('click', function () { openSaved(d, true); });
      del.addEventListener('click', function () {
        if (row.querySelector('.tc-confirm')) return;
        var yes = M.Button({ variant: 'filled', tone: 'danger', size: 'sm', label: 'Delete' });
        var no = M.Button({ variant: 'text', size: 'sm', label: 'Keep' });
        var conf = h('div', { 'class': 'tc-confirm', role: 'group', 'aria-label': 'Confirm delete' }, h('span', null, 'Delete “' + data.name + '” for everyone?'), yes, no);
        row.appendChild(conf); no.focus();
        no.addEventListener('click', function () { conf.remove(); del.focus(); });
        yes.addEventListener('click', function () {
          dbRef.collection('themes').doc(d.id).delete().then(function () {
            if (S.docId === d.id) { S.docId = null; S.savedSnap = null; refreshBar(); saveDraft(); }
            M.toast({ message: 'Deleted “' + data.name + '”' });
          }, function () { M.toast({ tone: 'danger', message: 'Not deleted. Try again.' }); });
        });
      });
      row.appendChild(presetStrip(rec));
      row.appendChild(h('div', { style: { 'min-width': '0' } }, h('div', { 'class': 'tc-saved__name' }, data.name), h('div', { 'class': 'tc-saved__meta' }, 'From ' + presetName(data.basedOn) + ', ' + relTime(data.updatedAt))));
      row.appendChild(h('div', { 'class': 'tc-saved__actions' }, open, dup, del));
      savedList.appendChild(row);
    });
  }
  function openSaved(d, asCopy) {
    var data = d.data, clean = sanitize(migrate(data.record, data.engine));
    S.record = clean.record;
    S.name = String(data.name || 'Untitled theme').slice(0, 64) + (asCopy ? ' (copy)' : '');
    S.basedOn = D.byKey(data.basedOn) ? data.basedOn : 'saltroad';
    S.sample = D.byKey(data.sample) ? data.sample : S.basedOn;
    S.art = cleanArt(data.art);
    S.docId = asCopy ? null : d.id;
    commit();
    S.savedSnap = asCopy ? null : snapshot();
    syncControls(); renderFrame(true); refreshBar(); saveDraft(); measureArt();
    libDrawer.close();
    M.toast({ message: asCopy ? 'Opened a copy of “' + data.name + '”' : 'Opened “' + data.name + '”', action: { label: 'Undo', onClick: undo }, duration: 6000 });
  }
  importBtn.addEventListener('click', function () {
    var obj;
    try { obj = JSON.parse(importTa.value); } catch (e) { importMsg.hidden = false; importMsg.setAttribute('data-tone', 'caution'); importMsg.textContent = 'That isn’t valid JSON.'; return; }
    if (obj && obj.value && obj.value.$type === 'at.motet.theme') obj = obj.value;
    if (!obj || typeof obj !== 'object' || (obj.$type && obj.$type !== 'at.motet.theme')) { importMsg.hidden = false; importMsg.setAttribute('data-tone', 'caution'); importMsg.textContent = 'That isn’t an at.motet.theme record.'; return; }
    var clean = sanitize(migrate(obj, obj.engine));
    S.record = clean.record;
    S.name = typeof obj.name === 'string' && obj.name.trim() ? obj.name.trim().slice(0, 64) : 'Imported theme';
    S.art = null; S.docId = null; S.savedSnap = null;
    commit(); syncControls(); renderFrame(true);
    importTa.value = ''; importMsg.hidden = true;
    libDrawer.close();
    var hasArt = obj.imagery && obj.imagery.art;
    M.toast({ tone: clean.issues.length ? 'caution' : 'positive', duration: 9000,
      message: 'Imported “' + S.name + '”.' + (clean.issues.length ? ' Not used as written: ' + clean.issues.slice(0, 4).join(', ') + (clean.issues.length > 4 ? ' and ' + (clean.issues.length - 4) + ' more' : '') + '.' : '') + (hasArt ? ' Add the art under Imagery.' : '') });
  });
  var libDrawer = M.Drawer({ title: 'Library', content: h('div', { 'class': 'tc-drawer-body' },
    h('section', null, h('h3', { 'class': 'tc-h' }, 'Start a new theme'), presetGrid),
    h('section', null, h('h3', { 'class': 'tc-h' }, 'Saved themes'), savedNote, savedList),
    h('section', null, h('h3', { 'class': 'tc-h' }, 'Import'), h('div', { 'class': 'tc-sec' }, importText, importMsg, h('div', null, importBtn)))) });
  libBtn.addEventListener('click', function () { renderSaved(); libDrawer.openModal(); });

  function saveTheme() {
    if (!dbRef) return;
    var id = S.docId || ('t' + Date.now().toString(36) + Math.random().toString(36).slice(2, 7));
    var local = S.art && S.art.kind === 'local';
    var body = { name: S.name || 'Untitled theme', basedOn: S.basedOn, sample: S.sample, record: S.record, art: local ? null : S.art, updatedAt: new Date().toISOString(), engine: ENGINE };
    saveBtn.disabled = true;
    var snapAtSave = snapshot();
    dbRef.collection('themes').doc(id).set(JSON.parse(JSON.stringify(body))).then(function () {
      S.docId = id; S.savedSnap = snapAtSave; refreshBar(); saveDraft();
      M.toast({ tone: local ? 'caution' : 'positive', message: local ? 'Saved without the image: keeping images needs edit access to this page.' : 'Saved to the library' });
    }, function (err) {
      refreshBar();
      var code = err && err.code;
      M.toast({ tone: 'danger', message: code === 'quota_exceeded' ? 'The library is full. Delete a theme to save this one.' : code === 'invalid_argument' ? 'This theme is too large to save. Remove some custom artwork.' : 'Not saved. Try again.' });
    });
  }
  saveBtn.addEventListener('click', saveTheme);
  cap('db').then(function (db) {
    if (!db) { refreshBar(); return; }
    dbRef = db; saveBtn.hidden = false; refreshBar();
    db.collection('themes').orderBy('updatedAt', 'desc').limit(200).onSnapshot(function (snap) {
      savedDocs = snap.docs.filter(function (d) { var x = d.data(); return x && typeof x.name === 'string' && x.record && typeof x.record === 'object'; })
        .map(function (d) { var x = d.data(); return { id: d.id, data: { name: String(x.name).slice(0, 64), basedOn: x.basedOn, sample: x.sample, record: x.record, art: x.art, updatedAt: x.updatedAt, engine: Number(x.engine) || 1 } }; });
      renderSaved();
    }, function () { savedDocs = []; renderSaved(); });
  });

  /* ---------------------------------------------------------------- change plumbing */
  var derivedRaf = 0;
  function scheduleDerived() {
    if (derivedRaf) return;
    derivedRaf = requestAnimationFrame(function () { derivedRaf = 0; derived.forEach(function (f) { f(); }); refreshBar(); });
  }
  function changed() { updateFrame(); scheduleDerived(); saveDraft(); }
  document.addEventListener('keydown', function (e) {
    var mod = e.metaKey || e.ctrlKey;
    if (!mod) return;
    var t = e.target, typing = t && (t.isContentEditable || /^(INPUT|TEXTAREA)$/.test(t.tagName));
    var k = e.key.toLowerCase();
    if (k === 'z' && !typing) { e.preventDefault(); if (e.shiftKey) redo(); else undo(); }
    else if (k === 'y' && !typing) { e.preventDefault(); redo(); }
    else if (k === 's') { e.preventDefault(); if (dbRef) saveTheme(); }
  });

  /* ---------------------------------------------------------------- start */
  var restored = loadDraft();
  if (!restored) applyPreset('saltroad');
  lastSnap = snapshot();
  setSeg(viewSeg, S.view); setSeg(modeSeg, S.mode); setSeg(devSeg, S.device);
  if (!restored && matchMedia('(max-width: 860px)').matches) { S.device = 'phone'; setSeg(devSeg, 'phone'); }
  M.enhance(document.getElementById('tc'));
  syncControls();
  layoutDevice();
  scheduleDerived();
  measureArt();
  if (restored) M.toast({ message: 'Picked up your draft of “' + (S.name || 'Untitled theme') + '”', action: { label: 'Start over', onClick: function () { applyPreset('saltroad'); S.docId = null; S.savedSnap = null; commit(); syncControls(); renderFrame(true); measureArt(); } }, duration: 8000 });
})();
