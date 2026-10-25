// Sets window.Motet and window.MotetDemo, which the export's script reads.
import './motet.js';

(function () {
  'use strict';
  var M = window.Motet, D = window.MotetDemo, h = M.h;
  var root = document.getElementById('root');
  var cur = { view: null, mounted: [], key: null };

  var GROUND = { surface: 'page', 'surface-quiet': 'quiet surface', 'surface-raised': 'raised surface', canvas: 'canvas', accent: 'accent', 'accent-soft': 'soft accent', danger: 'danger' };
  var ROLE = { ink: 'Text', 'ink-muted': 'Secondary text', 'line-strong': 'Strong line', accent: 'Accent', 'on-accent': 'Text on accent', 'accent-ink': 'Links and accent text',
    'on-accent-soft': 'Text on soft accent', ornament: 'Ornament', focus: 'Focus ring', 'danger-ink': 'Danger text', 'on-danger': 'Text on danger' };

  function world(sample, record) {
    var w = D.byKey(sample) || D.worlds[0];
    return Object.assign({}, w, { record: record });
  }
  function setTheme(m) {
    document.documentElement.setAttribute('data-theme', (m.contrast === 'high' ? 'contrast-' : '') + m.mode);
  }
  function scope(record, m, extra) {
    var s = h('div', { 'class': 'm-world__root' });
    root.appendChild(s);
    var mounted = M.mountWorld(s, record, m);
    cur.mounted.push(mounted);
    s.appendChild(h('div', { 'class': 'm-world__backdrop', 'aria-hidden': 'true' }));
    return s;
  }

  function front(w, m) {
    var s = scope(w.record, m);
    s.appendChild(M.Chrome({ world: w.name, worldMark: w.mark, account: 'lena.bsky.social', inbox: 3 }));
    s.appendChild(M.Masthead({ name: w.name, mark: w.mark, gms: w.gms, state: w.state, stateIcon: w.stateIcon, language: w.language, current: 'Overview' }));
    var cast = w.cast.map(function (c) { return c.name; });
    var facts = [['Played since', 'March 2026'], ['Scenes', '4']];
    s.appendChild(h('main', { 'class': 'm-world__page' },
      h('div', { 'class': 'm-world__sceneHeader' }, h('h2', { 'class': 'm-world__sceneTitle m-flanked' }, w.scene.arc)),
      h('div', { style: { 'container-type': 'inline-size' } },
        M.SceneRow({ title: w.scene.title, location: w.scene.location, cast: cast.concat(['Ada', 'Bo']), turn: true, unread: true, unreadCount: 2, updated: '2d' }),
        M.SceneRow({ title: (w.scenes || [])[0] || 'Letters from the Mill', location: w.scene.location, cast: cast.slice(0, 1), draft: true, updated: '6d' }),
        M.SceneRow({ title: (w.scenes || [])[1] || 'What the Ice Keeps', location: w.scene.location, cast: cast, updated: '3w' })),
      h('section', { 'class': 'pv-section' },
        h('h2', { 'class': 'm-world__sceneTitle m-flanked' }, 'Characters'),
        h('div', { 'class': 'pv-chars' }, w.cast.map(function (c, i) {
          return h('div', null, M.CharacterCard({ name: c.name, epithet: c.epithet, portrait: c.portrait, account: c.account, facts: facts, status: i ? 'pending' : 'approved' }));
        })))));
    M.enhance(s);
  }

  function components(w, m) {
    var s = scope(w.record, m);
    s.appendChild(M.Chrome({ world: w.name, worldMark: w.mark, account: 'lena.bsky.social', inbox: 3 }));
    s.appendChild(M.Masthead({ name: w.name, mark: w.mark, gms: w.gms, state: w.state, stateIcon: w.stateIcon, language: w.language, size: 'slim', current: 'Characters' }));
    var card = function (title, body) { return h('div', { 'class': 'm-character__root m-ornamented m-marked pv-card' }, h('h3', null, title), body); };
    var c0 = w.cast[0];
    var grid = h('div', { 'class': 'pv-cards' },
      card('Actions', h('div', { 'class': 'pv-stack' },
        h('div', { 'class': 'pv-row' }, M.Button({ variant: 'filled', label: 'Post reply', icon: 'send' }), M.Button({ variant: 'tonal', label: 'Save draft' }), M.Button({ variant: 'outline', label: 'Preview' }), M.Button({ variant: 'text', label: 'Cancel' })),
        h('div', { 'class': 'pv-row' }, M.IconButton({ icon: 'format_bold', label: 'Bold' }), M.IconButton({ icon: 'format_italic', label: 'Italic' }), M.IconButton({ icon: 'casino', label: 'Roll dice' }),
          M.Chip({ label: 'In character', selected: true }), M.Chip({ label: 'Out of character' })),
        h('div', { 'class': 'pv-row' }, M.Flag({ kind: 'turn', icon: 'front_hand', label: 'Your turn' }), M.Flag({ kind: 'draft', icon: 'draft', label: 'Draft' }), M.Flag({ kind: 'caution', icon: 'hourglass_top', label: 'Awaiting review' })),
        h('div', { 'class': 'pv-row' }, M.LaneFilter({ value: 'both' }), M.LaneFilter({ value: 'ic' }), M.LaneFilter({ value: 'ooc' })))),
      card('Fields', h('div', { 'class': 'pv-stack' },
        M.TextInput({ label: 'Character name', value: c0.name }),
        M.Select({ label: 'Speaking as', value: 'a', options: w.cast.map(function (c, i) { return { value: i ? 'b' : 'a', label: c.name }; }) }),
        M.Slider({ label: 'Reading size', min: 85, max: 160, value: 110, unit: '%' }),
        h('div', { 'class': 'pv-row', style: { 'justify-content': 'space-between' } }, h('span', { style: { font: '14px/20px var(--font-ui)' } }, 'Email me when it is my turn'), M.Switch({ checked: true, label: 'Email me when it is my turn' })),
        M.Checkbox({ label: 'Hide content notes I chose to hide', checked: true }))),
      card('Feedback', h('div', { 'class': 'pv-stack' },
        M.Alert({ tone: 'info', title: 'Scene paused', description: 'The GM paused this scene until Friday.' }),
        M.Alert({ tone: 'caution', title: 'Your turn ends in 6 hours' }),
        M.Alert({ tone: 'danger', title: 'Draft not saved', description: 'Check your connection.' }),
        M.Progress({ label: 'Uploading portrait', value: 64, showValue: true }))),
      card('Rolls', h('div', { 'class': 'pv-stack' }, M.DiceRoll(w.roll))));
    s.appendChild(h('main', { 'class': 'm-world__page' },
      h('div', { 'class': 'm-world__sceneHeader' }, h('h2', { 'class': 'm-world__sceneTitle m-flanked' }, 'Under this theme')),
      grid,
      h('section', { 'class': 'pv-section' },
        h('div', { 'class': 'pv-chars' }, h('div', null, M.CharacterCard({ name: c0.name, epithet: c0.epithet, portrait: c0.portrait, account: c0.account, facts: [['Home', w.scene.location], ['Arc', w.scene.arc]], status: 'approved' }))))));
    M.enhance(s);
  }

  function fmt(n) { return (Math.round(n * 10) / 10).toFixed(1); }
  function paletteMode(record, m, mode) {
    var mm = Object.assign({}, m, { mode: mode });
    var s = h('section', { 'class': 'pal__mode' });
    var mounted = M.mountWorld(s, record, mm);
    cur.mounted.push(mounted);
    var res = mounted.result, pal = res.palette, hx = pal.hex;
    var high = mm.contrast === 'high';
    var checks = M.roleChecks;
    var surfaces = ['canvas', 'surface-quiet', 'surface', 'surface-raised', 'surface-overlay'];
    var names = { canvas: 'Canvas', 'surface-quiet': 'Quiet', surface: 'Page', 'surface-raised': 'Raised', 'surface-overlay': 'Overlay' };
    var pass = 0, total = 0;
    var rows = Object.keys(checks).map(function (role) {
      var chk = checks[role], min = high ? chk.high : chk.min;
      var worst = Infinity, at = null;
      chk.on.forEach(function (g) { var r = M.color.contrast(hx[role], hx[g]); if (r < worst) { worst = r; at = g; } });
      var ok = worst >= min - 0.005; total++; if (ok) pass++;
      return h('tr', null,
        h('td', null, h('span', { 'class': 'pal__chip', style: { background: hx[role] } }), ROLE[role] || role),
        h('td', null, GROUND[at] || at),
        h('td', { 'class': 'num' }, fmt(worst) + ':1'),
        h('td', { 'class': 'num' }, min + ':1'),
        h('td', null, ok ? h('span', { 'class': 'pal__ok' }, M.icon('check'), 'Passes') : h('span', { 'class': 'pal__bad' }, M.icon('close'), 'Below')));
    });
    var fill = function (bg, fg, label) { return h('div', { 'class': 'pal__fill', style: { background: 'var(--' + bg + ')', color: 'var(--' + fg + ')' } }, h('span', null, label), h('code', null, hx[bg])); };
    s.appendChild(h('h2', { 'class': 'pal__head' }, mode === 'dark' ? 'Dark' : 'Light', h('small', null, pass + ' of ' + total + ' contrast duties pass' + (high ? ' at high contrast' : ''))));
    s.appendChild(h('div', { 'class': 'pal__surfaces' }, surfaces.map(function (k) {
      return h('div', { 'class': 'pal__surface', style: { background: 'var(--' + k + ')' } }, h('b', null, names[k]), h('span', null, hx[k]));
    })));
    s.appendChild(h('div', { 'class': 'pal__text' },
      h('p', null, 'The ice had begun to talk in the night. ', h('a', { href: '#', style: { color: 'var(--accent-ink)' } }, 'Ilse heard it'), ' from the loft.'),
      h('p', { style: { color: 'var(--ink-muted)', font: '400 14px/20px var(--font-ui)' } }, 'Mar 3, edited Mar 10'),
      M.Divider()));
    s.appendChild(h('div', { 'class': 'pal__fills' },
      fill('accent', 'on-accent', 'Accent'), fill('accent-soft', 'on-accent-soft', 'Soft accent'), h('div', { 'class': 'pal__fill', style: { background: 'var(--surface)', color: 'var(--ornament)', 'box-shadow': 'inset 0 0 0 1px var(--line)' } }, h('span', null, 'Ornament'), h('code', null, hx.ornament)),
      fill('danger', 'on-danger', 'Danger'), fill('caution', 'on-caution', 'Caution'), fill('positive', 'on-positive', 'Positive')));
    s.appendChild(h('div', { style: { 'overflow-x': 'auto' } }, h('table', { 'class': 'pal__table' },
      h('thead', null, h('tr', null, h('th', null, 'Role'), h('th', null, 'Weakest on'), h('th', null, 'Ratio'), h('th', null, 'Needs'), h('th', null, ''))),
      h('tbody', null, rows))));
    if (pal.adjustments && pal.adjustments.length) {
      s.appendChild(h('ul', { 'class': 'pal__notes' }, pal.adjustments.map(function (a) {
        return h('li', null,
          a.from ? h('span', { 'class': 'pal__swap', 'aria-hidden': 'true' }, h('i', { style: { background: a.from } }), M.icon('arrow_forward'), h('i', { style: { background: a.to } })) : h('span', { 'class': 'pal__swap', 'aria-hidden': 'true' }, M.icon('info')),
          h('span', null, a.text));
      })));
    }
    return s;
  }
  function palette(w, m) {
    var wrap = h('div', { 'class': 'pal' });
    root.appendChild(wrap);
    wrap.appendChild(paletteMode(w.record, m, 'light'));
    wrap.appendChild(paletteMode(w.record, m, 'dark'));
  }

  function render(msg) {
    root.innerHTML = '';
    cur.mounted = [];
    cur.view = msg.view; cur.sample = msg.sample;
    var m = msg.mount || {};
    setTheme(m);
    var w = world(msg.sample, msg.record);
    if (msg.view === 'front') front(w, m);
    else if (msg.view === 'components') components(w, m);
    else if (msg.view === 'palette') palette(w, m);
    else { var mounted = D.scenePage(root, w, { mountOpts: m }); cur.mounted.push(mounted); }
    if (msg.scroll === 'top') window.scrollTo(0, 0);
  }
  function update(msg) {
    var m = msg.mount || {};
    setTheme(m);
    if (cur.view === 'palette' || !cur.mounted.length) { var y = window.scrollY; render(Object.assign({}, msg, { view: cur.view, sample: cur.sample })); window.scrollTo(0, y); return; }
    cur.mounted.forEach(function (x) { x.update(msg.record, m); });
  }
  window.addEventListener('message', function (e) {
    if (e.source !== window.parent) return;
    var msg = e.data || {};
    try {
      if (msg.type === 'render') render(msg);
      else if (msg.type === 'update') update(msg);
    } catch (err) {
      window.parent.postMessage({ type: 'error', message: String(err && err.message || err) }, '*');
    }
  });
  window.parent.postMessage({ type: 'ready' }, '*');
})();
