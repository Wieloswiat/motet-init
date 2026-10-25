/* @ds-bundle: {"format":4,"namespace":"Motet","components":[{"name":"ScenePage"},{"name":"WorldGallery"},{"name":"Chrome"},{"name":"Palette"},{"name":"Materials"},{"name":"Shape"},{"name":"Ornaments"},{"name":"Typography"},{"name":"Icons"},{"name":"Motion"},{"name":"Button"},{"name":"IconButton"},{"name":"Chip"},{"name":"Segmented"},{"name":"Switch"},{"name":"TextField"},{"name":"Tabs"},{"name":"Menu"},{"name":"Dialog"},{"name":"Tooltip"},{"name":"Post"},{"name":"DiceRoll"},{"name":"CharacterCard"},{"name":"Masthead"},{"name":"Divider"},{"name":"Composer"},{"name":"SceneList"},{"name":"Inbox"},{"name":"Avatar"},{"name":"Checkbox"},{"name":"RadioGroup"},{"name":"Textarea"},{"name":"PasswordInput"},{"name":"NumberInput"},{"name":"Select"},{"name":"Combobox"},{"name":"TagsInput"},{"name":"DatePicker"},{"name":"Slider"},{"name":"FileUpload"},{"name":"Accordion"},{"name":"Collapsible"},{"name":"TreeView"},{"name":"Pagination"},{"name":"Breadcrumb"},{"name":"Steps"},{"name":"Popover"},{"name":"HoverCard"},{"name":"Drawer"},{"name":"Toast"},{"name":"Alert"},{"name":"Progress"},{"name":"Skeleton"},{"name":"DataList"},{"name":"ColorPicker"},{"name":"LaneFilter"}]} */
(function () {
  'use strict';

  /* ================================================================
     Color math: OKLCH <-> sRGB, gamut mapping, WCAG 2 contrast
     ================================================================ */
  var clamp = function (x, a, b) { return Math.min(b, Math.max(a, x)); };
  var round = function (x, n) { var p = Math.pow(10, n || 0); return Math.round(x * p) / p; };

  function oklabToLinear(L, a, b) {
    var l_ = L + 0.3963377774 * a + 0.2158037573 * b;
    var m_ = L - 0.1055613458 * a - 0.0638541728 * b;
    var s_ = L - 0.0894841775 * a - 1.2914855480 * b;
    var l = l_ * l_ * l_, m = m_ * m_ * m_, s = s_ * s_ * s_;
    return [
      4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
      -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
      -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s
    ];
  }
  function linearToOklab(r, g, b) {
    var l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
    var m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
    var s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
    return [
      0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
      1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
      0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s
    ];
  }
  function toLin(c) { return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); }
  function toGam(c) { return c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(c, 1 / 2.4) - 0.055; }

  function lchToLinear(c) {
    var hr = c.h * Math.PI / 180;
    return oklabToLinear(c.l, c.c * Math.cos(hr), c.c * Math.sin(hr));
  }
  function inGamut(rgb) { return rgb.every(function (v) { return v >= -0.00002 && v <= 1.0002; }); }
  /* Reduce chroma (keeping lightness and hue) until the colour fits sRGB. */
  function gamut(c) {
    var x = { l: clamp(c.l, 0, 1), c: Math.max(0, c.c), h: ((c.h % 360) + 360) % 360 };
    if (inGamut(lchToLinear(x))) return x;
    var lo = 0, hi = x.c;
    for (var i = 0; i < 22; i++) {
      var mid = (lo + hi) / 2;
      if (inGamut(lchToLinear({ l: x.l, c: mid, h: x.h }))) lo = mid; else hi = mid;
    }
    return { l: x.l, c: lo, h: x.h };
  }
  function lchToRgb(c) {
    return lchToLinear(gamut(c)).map(function (v) { return clamp(toGam(clamp(v, 0, 1)), 0, 1); });
  }
  function hex2(n) { var s = Math.round(clamp(n, 0, 1) * 255).toString(16); return s.length < 2 ? '0' + s : s; }
  function toHex(c, alpha) {
    var rgb = lchToRgb(c);
    return '#' + rgb.map(hex2).join('') + (alpha != null && alpha < 1 ? hex2(alpha) : '');
  }
  function parseHex(hex) {
    var h = String(hex).replace('#', '').trim();
    if (h.length === 3 || h.length === 4) h = h.split('').map(function (x) { return x + x; }).join('');
    return [0, 2, 4].map(function (i) { return parseInt(h.slice(i, i + 2), 16) / 255; });
  }
  function fromHex(hex) {
    var lab = linearToOklab.apply(null, parseHex(hex).map(toLin));
    var c = Math.sqrt(lab[1] * lab[1] + lab[2] * lab[2]);
    var h = Math.atan2(lab[2], lab[1]) * 180 / Math.PI;
    return { l: lab[0], c: c, h: (h + 360) % 360 };
  }
  function lumRgb(rgb) {
    var lin = rgb.map(toLin);
    return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
  }
  function lum(c) { return lumRgb(typeof c === 'string' ? parseHex(c) : lchToRgb(c)); }
  function contrast(a, b) {
    var la = lum(a), lb = lum(b);
    return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
  }
  /* Composite a translucent colour over an opaque one (sRGB-encoded, as browsers do). */
  function over(top, alpha, under) {
    var t = typeof top === 'string' ? parseHex(top) : lchToRgb(top);
    var u = typeof under === 'string' ? parseHex(under) : lchToRgb(under);
    return t.map(function (v, i) { return v * alpha + u[i] * (1 - alpha); });
  }
  function hueDist(a, b) { var d = Math.abs(a - b) % 360; return d > 180 ? 360 - d : d; }
  function mixHue(a, b, t) {
    var d = ((b - a + 540) % 360) - 180;
    return (a + d * t + 360) % 360;
  }
  /* Mix two colours in OKLab; keepL keeps the first colour's lightness. */
  function mix(a, b, t, keepL) {
    var ha = a.h * Math.PI / 180, hb = b.h * Math.PI / 180;
    var A = [a.l, a.c * Math.cos(ha), a.c * Math.sin(ha)], B = [b.l, b.c * Math.cos(hb), b.c * Math.sin(hb)];
    var M = A.map(function (v, i) { return v + (B[i] - v) * t; });
    var c = Math.sqrt(M[1] * M[1] + M[2] * M[2]);
    return { l: keepL ? a.l : M[0], c: c, h: (Math.atan2(M[2], M[1]) * 180 / Math.PI + 360) % 360 };
  }
  /* Walk lightness from `start` toward `dir` (-1 darker, +1 lighter) until the colour
     reaches `target` contrast against every colour in `grounds`. */
  function solveL(h, c, grounds, target, dir, start) {
    var l = start;
    for (var i = 0; i < 220; i++) {
      var col = { l: l, c: c, h: h };
      var ok = grounds.every(function (g) { return contrast(col, g) >= target; });
      if (ok) return gamut(col);
      l += dir * 0.005;
      if (l < 0 || l > 1) break;
    }
    return gamut({ l: dir < 0 ? 0 : 1, c: 0, h: h });
  }
  function minContrast(col, grounds) {
    return Math.min.apply(null, grounds.map(function (g) { return contrast(col, g); }));
  }

  /* ================================================================
     Palette: seeds + scheme -> semantic roles, contrast-clamped
     ================================================================ */
  var SCHEME_NEUTRAL_C = { neutral: 0.005, tonal: 0.012, vibrant: 0.02, expressive: 0.024, fidelity: 0.014, monochrome: 0 };
  /* Surface lightness per mode at depth 0, and the step per depth unit. */
  var SURFACES = {
    light: { canvas: 0.935, surface: 0.982, quiet: 0.958, raised: 0.997, overlay: 0.993, step: 0.012 },
    dark: { canvas: 0.15, surface: 0.198, quiet: 0.178, raised: 0.236, overlay: 0.262, step: 0.02 }
  };
  var RESERVED = { danger: 27, caution: 78, positive: 185, info: 248 };

  function buildPalette(rec, mode, opts) {
    opts = opts || {};
    var p = rec.palette || {};
    var high = opts.contrast === 'high';
    var dark = mode === 'dark';
    var scheme = p.scheme || 'tonal';
    var ceil = (p.chroma != null ? p.chroma : 100) / 1000;
    var temp = (p.temperature || 0) / 100;
    var depth = (p.depth && p.depth[mode]) || 0;
    var mat = rec.material || {};
    /* Every place the engine moves a colour away from what the record asked for is reported,
       so an editor can say what happened and why, in words. */
    var adjustments = [];
    var adjust = function (role, kind, from, to, target, text) {
      adjustments.push({ role: role, kind: kind, from: toHex(from), to: toHex(to), target: target || null, text: text });
    };
    var way = function (from, to) { return to.l < from.l ? 'darkened' : 'lightened'; };
    var SCHEME_NAMES = { neutral: 'Neutral', tonal: 'Tonal', vibrant: 'Vibrant', expressive: 'Expressive', fidelity: 'Fidelity', monochrome: 'Monochrome' };

    var prim = fromHex(p.primary || '#2b6b8a');
    var sec = p.secondary ? fromHex(p.secondary) : { l: prim.l, c: prim.c, h: (prim.h + 40) % 360 };

    /* neutral family */
    /* temperature picks the neutral family directly (warm stone / cool slate) instead of
       rotating the accent hue, which would pass through magenta on its way */
    var nh = p.neutral ? fromHex(p.neutral).h : prim.h;
    if (!p.neutral && temp >= 0.3) nh = 68 - (temp - 0.3) * 20;
    else if (!p.neutral && temp <= -0.3) nh = 238 + (temp + 0.3) * 15;
    var nc = (SCHEME_NEUTRAL_C[scheme] != null ? SCHEME_NEUTRAL_C[scheme] : 0.012) * clamp(ceil / 0.1, 0.25, 1.7) * (1 + Math.abs(temp) * 0.3);
    if (p.neutral) nc = Math.min(fromHex(p.neutral).c, Math.max(nc, 0.004));

    /* accent chroma by scheme */
    var ac;
    switch (scheme) {
      case 'neutral': ac = Math.min(prim.c, 0.06); break;
      case 'monochrome': ac = 0.012; break;
      case 'vibrant': case 'expressive': ac = Math.max(Math.min(prim.c * 1.2, ceil * 1.4), Math.min(prim.c, ceil)); break;
      case 'fidelity': ac = prim.c; break;
      default: ac = Math.min(prim.c, Math.max(ceil, 0.04));
    }

    var S = SURFACES[mode];
    var dL = depth * S.step;
    var dominant = (rec.imagery && rec.imagery.dominant) || p.dominant;
    var tint = clamp((mat.tint || 0) / 100, 0, 0.12);
    var surf = function (L, cMul) {
      var base = { l: clamp(L + dL, 0, 1), c: nc * (cMul == null ? 1 : cMul), h: nh };
      if (tint) base = mix(base, { l: base.l, c: Math.max(ac, 0.05), h: prim.h }, tint * 2.2, true);
      if (dominant && mat.mica) base = mix(base, fromHex(dominant), 0.07, true);
      return gamut(base);
    };
    var lc = dark ? 1 : 0.55;
    var canvas = surf(S.canvas, dark ? 1.1 : 1.2), surface = surf(S.surface, lc), quiet = surf(S.quiet, dark ? 1.15 : 0.9),
      raised = surf(S.raised, dark ? 1 : 0.3), overlay = surf(S.overlay, dark ? 1 : 0.3);
    var grounds = [surface, quiet, raised, canvas];
    var dir = dark ? 1 : -1;
    var grainBump = [0, 0.04, 0.12, 0.24, 0.36][clamp(Math.round(mat.grain || 0), 0, 4)];

    var ink = solveL(nh, Math.min(nc * 1.5, 0.02), grounds, high ? 15 : 12, dir, dark ? 0.86 : 0.3);
    var inkMuted = solveL(nh, Math.min(nc * 2.2, 0.03), grounds, (high ? 7.6 : 4.95) + grainBump, dir, dark ? 0.66 : 0.55);
    var line = solveL(nh, nc * 1.6, [surface, raised], high ? 3.1 : 1.32, dir, dark ? 0.26 : 0.9);
    var lineStrong = solveL(nh, nc * 2, [surface, raised, quiet], high ? 4.6 : 3.05, dir, dark ? 0.4 : 0.72);

    /* accent fill: boundary >= 3:1 against surfaces, its label >= 4.5:1 */
    var aL = scheme === 'fidelity' ? prim.l : (dark ? 0.76 : 0.5);
    if (scheme !== 'monochrome' && scheme !== 'fidelity' && prim.c - ac > 0.035)
      adjust('accent', 'chroma', prim, { l: prim.l, c: ac, h: prim.h }, null,
        'Accent toned down by the ' + SCHEME_NAMES[scheme] + ' scheme. Raise chroma, or pick Vibrant or Fidelity, to keep more of the seed.');
    var wantAccent = { l: aL, c: ac, h: prim.h };
    var accent = gamut(wantAccent);
    if (ac - accent.c > 0.015) { adjust('accent', 'gamut', wantAccent, accent, null, 'Accent saturation reduced to fit screen colours (sRGB) at this lightness.'); adjustments[adjustments.length - 1].from = null; }
    var onLight = gamut({ l: 0.985, c: 0.008, h: prim.h }), onDark = gamut({ l: 0.17, c: Math.min(ac, 0.03), h: prim.h });
    var labelTarget = high ? 7 : 4.6;
    var pickOn = function (fill) { return contrast(fill, onLight) >= contrast(fill, onDark) ? onLight : onDark; };
    var onAccent = pickOn(accent);
    var accent0 = accent;
    for (var k = 0; k < 120 && contrast(accent, onAccent) < labelTarget; k++) {
      accent = gamut({ l: accent.l + (onAccent === onLight ? -0.005 : 0.005), c: ac, h: prim.h });
      onAccent = pickOn(accent);
    }
    if (Math.abs(accent.l - accent0.l) > 0.025)
      adjust('accent', 'contrast', accent0, accent, labelTarget, 'Accent ' + way(accent0, accent) + ' so button text on it stays readable (' + labelTarget + ':1).');
    var bTarget = high ? 4.5 : 3;
    if (minContrast(accent, [surface, raised]) < bTarget) {
      var accent1 = accent;
      accent = solveL(prim.h, ac, [surface, raised], bTarget, dir, accent.l);
      onAccent = pickOn(accent);
      adjust('accent', 'contrast', accent1, accent, bTarget, 'Accent ' + way(accent1, accent) + ' to stand out from the page (' + bTarget + ':1).');
    }
    var accentInk = solveL(prim.h, Math.min(ac, 0.16), grounds, high ? 7.2 : 4.75, dir, accent.l);
    var accentSoft = gamut({ l: clamp((dark ? 0.31 : 0.925) + dL * 0.5, 0, 1), c: Math.min(Math.max(ac * 0.4, 0.02), 0.06), h: prim.h });
    var onAccentSoft = solveL(prim.h, Math.min(ac, 0.14), [accentSoft], high ? 7.2 : 4.75, dir, dark ? 0.85 : 0.4);

    /* ornament (secondary seed): decorative graphics at >= 3:1 */
    var oc = scheme === 'monochrome' ? nc * 2 : Math.min(sec.c, Math.max(ceil * 1.2, 0.03));
    var oStart = { l: dark ? Math.max(sec.l, 0.62) : Math.min(sec.l, 0.6), c: oc, h: sec.h };
    var oTarget = high ? 4.5 : 3;
    var ornament = solveL(sec.h, oc, [surface, raised], oTarget, dir, oStart.l);
    if (Math.abs(ornament.l - oStart.l) > 0.025)
      adjust('ornament', 'contrast', oStart, ornament, oTarget, 'Ornament ' + way(oStart, ornament) + ' to stay visible on the page (' + oTarget + ':1).');
    var ornamentSoft = gamut({ l: dark ? 0.32 + dL : 0.88 + dL * 0.5, c: Math.min(oc * 0.5, 0.05), h: sec.h });

    /* platform-reserved hues; danger steps away from a red-ish accent */
    var dh = RESERVED.danger;
    if (ac > 0.05 && hueDist(prim.h, dh) < 32) {
      dh = prim.h <= dh ? 52 : 4;
      adjust('danger', 'hue', { l: dark ? 0.72 : 0.52, c: 0.17, h: RESERVED.danger }, { l: dark ? 0.72 : 0.52, c: 0.17, h: dh }, null,
        'Danger shifted toward ' + (dh === 52 ? 'orange' : 'crimson') + ' so it can’t be mistaken for the accent.');
    }
    var status = {};
    [['danger', dh, 0.17], ['caution', RESERVED.caution, 0.14], ['positive', RESERVED.positive, 0.11], ['info', RESERVED.info, 0.12]].forEach(function (s) {
      var name = s[0], h = s[1], c = s[2];
      var fill = gamut({ l: dark ? 0.72 : 0.52, c: c, h: h });
      var on = pickOn(fill);
      for (var j = 0; j < 120 && contrast(fill, on) < labelTarget; j++) { fill = gamut({ l: fill.l + (on === onLight ? -0.005 : 0.005), c: c, h: h }); on = pickOn(fill); }
      status[name] = fill;
      status['on-' + name] = on;
      status[name + '-ink'] = solveL(h, c * 0.9, grounds, high ? 7.2 : 4.75, dir, fill.l);
      status[name + '-soft'] = gamut({ l: dark ? 0.29 + dL : 0.93 + dL * 0.5, c: Math.min(c * 0.35, 0.05), h: h });
    });

    /* focus: two-tone ring; outer ring must reach 3:1 on every ground and on the soft accent */
    var focus = solveL(prim.h, Math.min(Math.max(ac, 0.08), 0.15), grounds.concat([accentSoft]), high ? 4.5 : 3.6, dir, dark ? 0.82 : 0.42);

    var roles = {
      canvas: canvas, surface: surface, 'surface-quiet': quiet, 'surface-raised': raised, 'surface-overlay': overlay,
      ink: ink, 'ink-muted': inkMuted, line: line, 'line-strong': lineStrong,
      accent: accent, 'on-accent': onAccent, 'accent-ink': accentInk, 'accent-soft': accentSoft, 'on-accent-soft': onAccentSoft,
      ornament: ornament, 'ornament-soft': ornamentSoft, focus: focus
    };
    Object.keys(status).forEach(function (k2) { roles[k2] = status[k2]; });

    /* authored overrides: accepted only when they keep their contrast target */
    var ov = (p.overrides && p.overrides[mode]) || {};
    Object.keys(ov).forEach(function (role) {
      if (!roles[role]) return;
      var want = fromHex(ov[role]);
      var check = ROLE_CHECKS[role];
      if (!check) { roles[role] = want; return; }
      var gr = check.on.map(function (n) { return roles[n]; });
      var t = high ? check.high : check.min;
      if (minContrast(want, gr) >= t) roles[role] = want;
      else {
        roles[role] = solveL(want.h, want.c, gr, t, dir, want.l);
        adjust(role, 'override', want, roles[role], t, 'Your ' + (ROLE_WORDS[role] || role) + ' colour was ' + way(want, roles[role]) + ' to keep ' + t + ':1 on ' + (GROUND_WORDS[check.on[0]] || check.on[0]) + '.');
      }
    });

    var hex = {};
    Object.keys(roles).forEach(function (k3) { hex[k3] = toHex(roles[k3]); });
    hex.scrim = dark ? '#000000a3' : '#0f141a73';
    hex.selection = toHex(accentSoft);
    return { mode: mode, roles: roles, hex: hex, adjustments: adjustments, notes: adjustments.map(function (a) { return a.text; }), accentHue: prim.h, neutral: { h: nh, c: nc } };
  }

  var ROLE_WORDS = { ink: 'text', 'ink-muted': 'secondary text', 'line-strong': 'strong line', accent: 'accent', 'on-accent': 'text-on-accent', 'accent-ink': 'accent text',
    'on-accent-soft': 'text-on-soft-accent', ornament: 'ornament', focus: 'focus ring', 'danger-ink': 'danger text', 'on-danger': 'text-on-danger' };
  var GROUND_WORDS = { surface: 'the page', accent: 'the accent', 'accent-soft': 'soft accent fills', danger: 'danger fills' };
  /* Contrast duties of each role: what it sits on, and the minimum ratio. */
  var ROLE_CHECKS = {
    ink: { on: ['surface', 'surface-quiet', 'surface-raised', 'canvas'], min: 7, high: 15 },
    'ink-muted': { on: ['surface', 'surface-quiet', 'surface-raised', 'canvas'], min: 4.5, high: 7 },
    'line-strong': { on: ['surface', 'surface-raised'], min: 3, high: 4.5 },
    accent: { on: ['surface', 'surface-raised'], min: 3, high: 4.5 },
    'on-accent': { on: ['accent'], min: 4.5, high: 7 },
    'accent-ink': { on: ['surface', 'surface-quiet', 'surface-raised', 'canvas'], min: 4.5, high: 7 },
    'on-accent-soft': { on: ['accent-soft'], min: 4.5, high: 7 },
    ornament: { on: ['surface', 'surface-raised'], min: 3, high: 4.5 },
    focus: { on: ['surface', 'surface-raised', 'canvas', 'accent-soft'], min: 3, high: 4.5 },
    'danger-ink': { on: ['surface', 'surface-raised'], min: 4.5, high: 7 },
    'on-danger': { on: ['danger'], min: 4.5, high: 7 }
  };

  /* Audit: every contrast duty in a compiled palette, for previews and the theme editor. */
  function audit(pal) {
    return Object.keys(ROLE_CHECKS).map(function (role) {
      var chk = ROLE_CHECKS[role];
      var worst = Math.min.apply(null, chk.on.map(function (g) { return contrast(pal.roles[role], pal.roles[g]); }));
      return { role: role, on: chk.on, ratio: round(worst, 2), min: chk.min, pass: worst >= chk.min - 0.005 };
    });
  }

  /* ================================================================
     Material: finish, grain, edge light, elevation
     ================================================================ */
  function svgUri(svg) {
    return 'url("data:image/svg+xml,' + svg.replace(/"/g, "'").replace(/#/g, '%23').replace(/</g, '%3C').replace(/>/g, '%3E').replace(/\s+/g, ' ') + '")';
  }
  /* Five levels: 0 none, 1 fine, 2 medium, 3 coarse, 4 heavy. */
  function grain(level, mode) {
    level = clamp(Math.round(level || 0), 0, 4);
    if (!level) return 'none';
    var freq = [0, 1.05, 0.95, 0.78, 0.6][level];
    var oct = [0, 2, 2, 3, 4][level];
    var a = (mode === 'dark' ? [0, 0.028, 0.055, 0.085, 0.12] : [0, 0.02, 0.04, 0.06, 0.09])[level];
    return svgUri("<svg xmlns='http://www.w3.org/2000/svg' width='220' height='220'><filter id='g'><feTurbulence type='fractalNoise' baseFrequency='" + freq + "' numOctaves='" + oct + "' seed='7' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0.9 0  0 0 0 0.9 0  0 0 0 0.9 0  0 0 0 0 " + a + "'/></filter><rect width='220' height='220' filter='url(#g)'/></svg>");
  }
  function finishLayer(finish, mode) {
    var w = mode === 'dark' ? '255 255 255' : '255 255 255';
    if (finish === 'satin') return mode === 'dark'
      ? 'linear-gradient(180deg, rgb(' + w + ' / 0.035), rgb(' + w + ' / 0) 38%)'
      : 'linear-gradient(180deg, rgb(255 255 255 / 0.5), rgb(255 255 255 / 0) 42%)';
    if (finish === 'sheen') return mode === 'dark'
      ? 'linear-gradient(162deg, rgb(255 255 255 / 0.075), rgb(255 255 255 / 0) 34%, rgb(255 255 255 / 0) 72%, rgb(255 255 255 / 0.03))'
      : 'linear-gradient(162deg, rgb(255 255 255 / 0.75), rgb(255 255 255 / 0) 36%, rgb(0 0 0 / 0) 70%, rgb(0 0 0 / 0.025))';
    return 'none';
  }
  function edgeLight(level, mode) {
    if (!level) return '0 0 #0000';
    var top = mode === 'dark' ? [0, 0.06, 0.11][level] : [0, 0.7, 1][level];
    var bottom = mode === 'dark' ? [0, 0.18, 0.28][level] : [0, 0.035, 0.06][level];
    return 'inset 0 1px 0 0 rgb(255 255 255 / ' + top + '), inset 0 -1px 0 0 rgb(0 0 0 / ' + bottom + ')';
  }
  /* Elevation, after Fluent 2: a directional key shadow plus a soft ambient one. */
  var ELEVATION = [
    [1, 2, 0, 2], [2, 4, 0, 2], [8, 16, 0, 4], [14, 28, 0, 8]
  ];
  function shadows(strength, mode) {
    var s = strength == null ? 1 : strength;
    var dark = mode === 'dark';
    return ELEVATION.map(function (e, i) {
      if (s === 0) return '0 0 #0000';
      var key = (dark ? (i < 2 ? 0.32 : 0.4) : (i < 2 ? 0.12 : 0.2)) * (s === 2 ? 1.35 : 1);
      var amb = (dark ? 0.26 : (i < 2 ? 0.1 : 0.14)) * (s === 2 ? 1.25 : 1);
      var mul = s === 2 ? 1.3 : 1;
      return '0 ' + round(e[0] * mul, 1) + 'px ' + round(e[1] * mul, 1) + 'px rgb(0 0 0 / ' + round(key, 3) + '), 0 0 ' + e[3] + 'px rgb(0 0 0 / ' + round(amb, 3) + ')';
    });
  }
  /* ================================================================
     Shape: corner geometry, radius scale (Material 3 steps), frame profile
     ================================================================ */
  var RADII = { xs: 4, sm: 8, md: 12, lg: 16, xl: 28 };
  var CORNERS = ['round', 'squircle', 'bevel', 'square'];
  var PROFILES = ['even', 'cut', 'clipped'];
  function shapeVars(shape, supports) {
    var corner = CORNERS.indexOf(shape.corner) >= 0 ? shape.corner : 'round';
    var profile = PROFILES.indexOf(shape.profile) >= 0 ? shape.profile : 'even';
    var scale = clamp(shape.scale != null ? shape.scale : 100, 0, 150) / 100;
    var native = !!supports.cornerShape;
    var v = {};
    /* squircles read softer than round corners of the same radius; bevels without corner-shape fall back to small round corners */
    var k0 = corner === 'squircle' && native ? 1.35 : (corner === 'bevel' && !native ? 0.35 : 1);
    Object.keys(RADII).forEach(function (k) { v['radius-' + k] = (corner === 'square' ? 0 : round(RADII[k] * scale * k0, 1)) + 'px'; });
    v['radius-control'] = corner === 'square' ? '0px' : round(Math.min(RADII.md * scale * (corner === 'squircle' && native ? 1.15 : k0), 14), 1) + 'px';
    v['corner-shape'] = native && (corner === 'squircle' || corner === 'bevel') ? corner : 'round';
    v['corner-shape-control'] = v['corner-shape'];
    /* Frame profile: cards, framed posts, the composer. Asymmetric profiles chamfer one corner
       (cut: bottom right) or two opposite ones (clipped), after the panels of Cyberpunk 2077
       and Destiny. Without corner-shape support they degrade to small even corners. */
    var chamfer = round(Math.max(10, RADII.lg * Math.max(scale, 0.75)), 1);
    var small = corner === 'square' ? 0 : round(Math.min(RADII.xs * Math.max(scale, 0.5), 4), 1);
    if (profile === 'even') { v['frame-radius'] = v['radius-lg']; v['frame-shape'] = v['corner-shape']; }
    else if (!native) { v['frame-radius'] = small + 'px'; v['frame-shape'] = 'round'; }
    else if (profile === 'cut') { v['frame-radius'] = [small, small, chamfer, small].join('px ') + 'px'; v['frame-shape'] = 'round round bevel round'; }
    else { v['frame-radius'] = [chamfer, small, chamfer, small].join('px ') + 'px'; v['frame-shape'] = 'bevel round bevel round'; }
    var bw = { none: 0, hairline: 1, rule: 1, heavy: 2 }[shape.border || 'hairline'];
    v['border-width'] = bw + 'px';
    v['border-role'] = shape.border === 'rule' || shape.border === 'heavy' ? 'var(--line-strong)' : 'var(--line)';
    /* borders are always one solid colour; a World may tint them with its ornament colour */
    v['frame-line'] = shape.tone === 'ornament' ? 'var(--ornament)' : v['border-role'];
    return { vars: v, corner: corner, profile: profile };
  }

  /* ================================================================
     Ornament library: vector-only, single ink, masked with CSS
     ================================================================ */
  /* rule and fade are plain separators; filigree is the one pictorial set; segment is the
     geometric accent language of angular sci-fi UIs: an offset bar on a hairline, and the
     same bar notched into the top edge of framed surfaces.
     Paths are fills unless `s` (stroke width) is given; `m` mirrors around the centre. */
  var ORNAMENTS = {
    rule: { line: 'solid' },
    fade: { line: 'fade' },
    filigree: {
      line: 'fade', align: 'center',
      motif: { vb: [168, 28], p: [
        { d: 'M84 4l9 10-9 10-9-10zM84 8.5 79 14l5 5.5 5-5.5z', rule: 'evenodd' },
        { d: 'M84 12a2 2 0 1 1 0 4 2 2 0 0 1 0-4z' },
        { s: 1.3, m: true, d: 'M73 14c-6 0-8-7-14-7-5 0-6.5 7-2 7 3 0 3-4 0.5-4M57 14c-8 0-10 5-17 5-5 0-7-3-7-5M40 19c-6 0-10-5-18-5H6' },
        { m: true, d: 'M33 12.6a1.6 1.6 0 1 1 0 3.2 1.6 1.6 0 0 1 0-3.2z' }
      ] },
      corner: { vb: [30, 30], p: [
        { s: 1.2, d: 'M2 28V14C2 7.4 7.4 2 14 2h14' },
        { s: 1.1, d: 'M7 22c0-6 3-9 7-9 3 0 4 3 2 4M22 7c-6 0-9 3-9 7' },
        { d: 'M1 1h4v4H1z' }
      ] },
      flank: { vb: [30, 14], p: [{ s: 1.2, d: 'M0 7h12M14 7c3-5 8-5 8-1 0 3-4 3-4 0' }, { d: 'M26 4l3 3-3 3-3-3z' }] }
    },
    segment: {
      line: 'solid', align: 'start', mark: true,
      motif: { vb: [64, 6], p: [{ d: 'M0 1.5h44v3H0zM50 0h6v6h-6z' }] },
      flank: { vb: [18, 8], p: [{ d: 'M0 3h10v2H0zM13 1h5v6h-5z' }] }
    }
  };
  var PATH_RE = /^[MmLlHhVvCcSsQqTtAaZz0-9eE.,+\-\s]*$/;
  /* Validate author-supplied ornament artwork: path data only, bounded size. */
  function validateOrnament(o) {
    var errors = [];
    if (!o || !Array.isArray(o.vb) || o.vb.length !== 2) errors.push('vb must be [width, height]');
    else if (o.vb.some(function (n) { return !(n > 0 && n <= 512 && Math.round(n) === n); })) errors.push('vb values must be integers 1–512');
    if (!o || !Array.isArray(o.p) || !o.p.length || o.p.length > 16) errors.push('p must hold 1–16 paths');
    else o.p.forEach(function (x, i) {
      if (typeof x.d !== 'string' || x.d.length > 4000 || !PATH_RE.test(x.d)) errors.push('path ' + i + ': only SVG path commands and numbers');
      if (x.s != null && !(x.s > 0 && x.s <= 8)) errors.push('path ' + i + ': stroke width 0–8');
    });
    return { ok: errors.length === 0, errors: errors };
  }
  function ornamentSvg(o, rotate) {
    var w = o.vb[0], h = o.vb[1];
    var body = o.p.map(function (x) {
      var attrs = x.s ? "fill='none' stroke='black' stroke-width='" + x.s + "' stroke-linecap='round' stroke-linejoin='round'" : "fill='black'" + (x.rule ? " fill-rule='" + x.rule + "'" : '');
      var one = "<path d='" + x.d + "' " + attrs + "/>";
      if (x.m) one += "<path transform='matrix(-1 0 0 1 " + w + " 0)' d='" + x.d + "' " + attrs + "/>";
      return one;
    }).join('');
    var t = rotate ? " transform='rotate(" + rotate + " " + w / 2 + " " + h / 2 + ")'" : '';
    return "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 " + w + ' ' + h + "' width='" + w + "' height='" + h + "'><g" + t + '>' + body + '</g></svg>';
  }
  function ornamentVars(orn) {
    var set = ORNAMENTS[orn.set] || ORNAMENTS.rule;
    var level = clamp(orn.level || 0, 0, 3);
    var custom = orn.custom || {};
    var pick = function (slot) {
      if (custom[slot] && validateOrnament(custom[slot]).ok) return custom[slot];
      return set[slot];
    };
    var motif = pick('motif'), corner = pick('corner'), flank = pick('flank');
    var on = level >= 1;
    var v = { 'orn-level': String(level), 'orn-line': set.line || 'solid' };
    v['orn-motif'] = motif && on ? svgUri(ornamentSvg(motif)) : 'none';
    v['orn-motif-w'] = motif && on ? motif.vb[0] + 'px' : '0px';
    v['orn-motif-h'] = motif && on ? motif.vb[1] + 'px' : '1px';
    v['orn-motif-display'] = motif && on ? 'block' : 'none';
    v['orn-flank'] = flank && on ? svgUri(ornamentSvg(flank)) : 'none';
    v['orn-flank-w'] = flank && on ? flank.vb[0] + 'px' : '0px';
    v['orn-flank-h'] = flank && on ? flank.vb[1] + 'px' : '0px';
    v['orn-flank-display'] = flank && on ? 'inline-block' : 'none';
    var corners = corner && level >= 2;
    v['orn-corner-tl'] = corners ? svgUri(ornamentSvg(corner)) : 'none';
    v['orn-corner-tr'] = corners ? svgUri(ornamentSvg(corner, 90)) : 'none';
    v['orn-corner-br'] = corners ? svgUri(ornamentSvg(corner, 180)) : 'none';
    v['orn-corner-bl'] = corners ? svgUri(ornamentSvg(corner, 270)) : 'none';
    v['orn-corner-size'] = corners ? Math.max(corner.vb[0], corner.vb[1]) + 'px' : '0px';
    v['orn-corner-display'] = corners ? 'block' : 'none';
    v['orn-pad'] = corners ? Math.round(Math.max(corner.vb[0], corner.vb[1]) * 0.7 + 8) + 'px' : '0px';
    v['orn-mark-display'] = set.mark && level >= 2 ? 'block' : 'none';
    v['divider-color'] = on ? 'var(--ornament)' : 'var(--line)';
    return v;
  }

  /* Where art meets the page: a soft fade (default) or a shaped edge. One edge line, used two
     ways: as the bottom of header art (`artMask`), and as the top of the content sheet that
     rises over fixed art (`sheetEdge`). Lines run right to left across a 1000 × 100 box. */
  function seeded(seed) { var s = seed; return function () { s = (s * 16807) % 2147483647; return (s - 1) / 2147483646; }; }
  function edgeLine(kind) {
    var W = 1000;
    if (kind === 'torn') {
      var r = seeded(11), pts = [];
      for (var x = 0; x <= W; x += 9) pts.push(x + ' ' + round(70 + r() * 22 + (r() > 0.85 ? 6 : 0), 1));
      return 'V' + pts[pts.length - 1].split(' ')[1] + 'L' + pts.reverse().join('L') + 'L0 ' + pts[pts.length - 1].split(' ')[1];
    }
    if (kind === 'burned') {
      var r2 = seeded(5), p2 = [], y = 60;
      for (var x2 = 0; x2 <= W; x2 += 20) { y = clamp(y + (r2() - 0.5) * 26, 34, 92); p2.push([x2, y]); }
      var d = 'V' + round(p2[p2.length - 1][1], 1);
      for (var i = p2.length - 1; i > 0; i--) {
        var a = p2[i], b = p2[i - 1];
        d += 'Q' + round(a[0] - 10, 1) + ' ' + round((a[1] + b[1]) / 2 + (r2() - 0.5) * 18, 1) + ' ' + b[0] + ' ' + round(b[1], 1);
      }
      return d;
    }
    if (kind === 'arch') return 'V58C760 58 640 100 500 100S240 58 0 58';
    return null;
  }
  function edgeSvg(d) { return svgUri("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1000 100' preserveAspectRatio='none'><path d='" + d + "' fill='black'/></svg>"); }
  function artMask(kind) {
    if (kind === 'straight') return 'none';
    var line = edgeLine(kind);
    if (!line) return 'linear-gradient(to bottom, #000 0%, #000 45%, rgb(0 0 0 / 0.84) 58%, rgb(0 0 0 / 0.6) 70%, rgb(0 0 0 / 0.38) 80%, rgb(0 0 0 / 0.18) 89%, rgb(0 0 0 / 0.05) 96%, transparent 100%)';
    return edgeSvg('M0 0H1000' + line + 'Z') + ' bottom / 100% 64px no-repeat, linear-gradient(#000 0 0) top / 100% calc(100% - 63px) no-repeat';
  }
  function sheetEdge(kind) {
    if (kind === 'straight') return { image: 'linear-gradient(#000 0 0)', lip: '0px' };
    var line = edgeLine(kind);
    if (!line) return { image: 'linear-gradient(to bottom, transparent 0%, rgb(0 0 0 / 0.04) 14%, rgb(0 0 0 / 0.12) 28%, rgb(0 0 0 / 0.25) 42%, rgb(0 0 0 / 0.43) 56%, rgb(0 0 0 / 0.64) 70%, rgb(0 0 0 / 0.84) 84%, #000 100%)', lip: 'min(160px, calc(var(--art-window, var(--art-h, 200px)) * 0.55))' };
    return { image: edgeSvg('M0 100H1000' + line + 'L0 100Z'), lip: '64px' };
  }

  /* ================================================================
     Typography library (vetted faces with Polish coverage)
     ================================================================ */
  var FACES = {
    motet: { stack: '"IBM Plex Sans", system-ui, sans-serif', adjust: 1, axes: "'wdth' 100" },
    'motet-condensed': { stack: '"IBM Plex Sans", system-ui, sans-serif', adjust: 1.04, axes: "'wdth' 82" },
    'source-serif': { stack: '"Source Serif 4", Georgia, serif', adjust: 1.02, axes: 'normal' },
    oxanium: { stack: 'Oxanium, "IBM Plex Sans", sans-serif', adjust: 0.98, axes: 'normal' },
    'big-shoulders-stencil': { stack: '"Big Shoulders Stencil", "Archivo", sans-serif', adjust: 1.18, axes: 'normal' },
    cormorant: { stack: '"Cormorant Garamond", "Source Serif 4", serif', adjust: 1.16, axes: 'normal' },
    'grenze-gotisch': { stack: '"Grenze Gotisch", "Source Serif 4", serif', adjust: 1.06, axes: 'normal' },
    mono: { stack: '"IBM Plex Mono", ui-monospace, monospace', adjust: 0.9, axes: 'normal' },
    archivo: { stack: 'Archivo, "IBM Plex Sans", sans-serif', adjust: 1.02, axes: "'wdth' 78" }
  };
  var READING = {
    serif: '"Source Serif 4", Georgia, "Times New Roman", serif',
    sans: '"IBM Plex Sans", system-ui, sans-serif',
    system: 'system-ui, -apple-system, "Segoe UI", sans-serif'
  };
  function typeVars(t, reader) {
    var face = FACES[t.display] || FACES.motet;
    var v = {};
    v['font-display'] = face.stack;
    v['display-axes'] = face.axes;
    v['display-adjust'] = String(face.adjust);
    v['display-case'] = t['case'] === 'upper' ? 'uppercase' : 'none';
    v['display-tracking'] = round(clamp(t.tracking || 0, -40, 160) / 1000, 3) + 'em';
    v['display-weight'] = String(clamp(t.weight || 600, 100, 900));
    v['font-name'] = t.names === 'display' ? face.stack : 'var(--font-ui)';
    v['name-case'] = t.names === 'display' && t['case'] === 'upper' ? 'uppercase' : 'none';
    v['name-adjust'] = t.names === 'display' ? String(face.adjust) : '1';
    v['name-axes'] = t.names === 'display' ? face.axes : 'normal';
    v['name-tracking'] = t.names === 'display' ? round(clamp(t.tracking || 0, -40, 80) / 1000, 3) + 'em' : '0em';
    var reading = (reader && reader.readingFont && reader.readingFont !== 'world') ? reader.readingFont : (t.reading || 'serif');
    v['font-reading'] = READING[reading] || READING.serif;
    v['numerals'] = t.numerals === 'tabular' ? 'tabular-nums' : t.numerals === 'oldstyle' ? 'oldstyle-nums' : 'normal';
    return v;
  }

  /* ================================================================
     Motion: springs as (duration, bounce), sampled to CSS linear()
     ================================================================ */
  function spring(opts) {
    var duration = (opts && opts.duration) || 0.3, bounce = clamp((opts && opts.bounce) || 0, 0, 0.8);
    var w = 2 * Math.PI / duration, z = 1 - bounce;
    var x = function (t) {
      if (z >= 1) return 1 - Math.exp(-w * t) * (1 + w * t);
      var wd = w * Math.sqrt(1 - z * z);
      return 1 - Math.exp(-z * w * t) * (Math.cos(wd * t) + (z * w / wd) * Math.sin(wd * t));
    };
    var settle = 0, dt = 0.001;
    for (var t = 0; t < duration * 5; t += dt) if (Math.abs(1 - x(t)) > 0.0015) settle = t;
    settle += dt;
    var n = (opts && opts.points) || (bounce > 0.05 ? 26 : 16);
    var pts = [];
    for (var i = 0; i <= n; i++) pts.push(i === n ? 1 : round(x(settle * i / n), 3));
    return { easing: 'linear(' + pts.join(', ') + ')', ms: Math.round(settle * 1000), duration: duration, bounce: bounce };
  }
  /* Material 3 Expressive springs, converted with (duration = 2π/√k, bounce = 1 − ζ). */
  var SPRINGS = {
    standard: { fast: [0.168, 0.1], 'default': [0.237, 0.1], slow: [0.363, 0.1] },
    expressive: { fast: [0.222, 0.4], 'default': [0.322, 0.2], slow: [0.444, 0.2] },
    effects: { fast: [0.102, 0], 'default': [0.157, 0], slow: [0.222, 0] }
  };
  var CHARACTERS = {
    standard: { spatial: 'standard', bounceCap: 0.1, time: 1, decor: 1 },
    crisp: { spatial: 'standard', bounceCap: 0, time: 0.85, decor: 0.85 },
    soft: { spatial: 'expressive', bounceCap: 0.15, time: 1.08, decor: 1.25 },
    eerie: { spatial: 'standard', bounceCap: 0, time: 1, decor: 2.4 }
  };
  var FUNCTIONAL_CAP_MS = 420;
  function motionVars(m, reader) {
    var ch = CHARACTERS[(m && m.character) || 'standard'] || CHARACTERS.standard;
    var v = {};
    var reduce = reader && reader.reduceMotion;
    ['fast', 'default', 'slow'].forEach(function (sp) {
      var s = SPRINGS[ch.spatial][sp];
      var sp1 = spring({ duration: s[0] * ch.time, bounce: Math.min(s[1], ch.bounceCap) });
      var ef = SPRINGS.effects[sp];
      var e1 = spring({ duration: ef[0], bounce: 0 });
      v['ease-spatial-' + sp] = sp1.easing;
      v['dur-spatial-' + sp] = (reduce ? 0 : Math.min(sp1.ms, FUNCTIONAL_CAP_MS)) + 'ms';
      v['ease-effects-' + sp] = e1.easing;
      v['dur-effects-' + sp] = (reduce ? Math.min(e1.ms, 100) : e1.ms) + 'ms';
    });
    var d = spring({ duration: SPRINGS.effects.slow[0] * ch.decor, bounce: 0 });
    v['ease-decor'] = d.easing;
    v['dur-decor'] = (reduce ? 0 : Math.min(d.ms, 900)) + 'ms';
    return v;
  }

  /* ================================================================
     Icons and density
     ================================================================ */
  var ICON_FAMILIES = { outlined: '"Material Symbols Outlined"', rounded: '"Material Symbols Rounded"', sharp: '"Material Symbols Sharp"' };
  function iconVars(ic, mode) {
    return {
      'icon-family': ICON_FAMILIES[(ic && ic.style) || 'outlined'] || ICON_FAMILIES.outlined,
      'icon-wght': String(clamp((ic && ic.weight) || 400, 200, 600)),
      'icon-grad': mode === 'dark' ? '-25' : '0'
    };
  }
  var DENSITY = {
    comfortable: { control: 44, gap: 1.15, post: 40 },
    standard: { control: 40, gap: 1, post: 32 },
    compact: { control: 36, gap: 0.85, post: 24 }
  };

  /* ================================================================
     Compile a World theme record into CSS custom properties
     ================================================================ */
  function detectSupports() {
    var ok = typeof CSS !== 'undefined' && CSS.supports;
    return {
      cornerShape: !!(ok && CSS.supports('corner-shape', 'bevel')),
      backdrop: !!(ok && (CSS.supports('backdrop-filter', 'blur(1px)') || CSS.supports('-webkit-backdrop-filter', 'blur(1px)')))
    };
  }
  /* Lowest opacity at which `ink` keeps `target` contrast on `bg` over the given backgrounds
     (pure black and white unless the art's measured range is known). */
  function minAlphaFor(bg, ink, target, unders) {
    for (var a = 0.5; a <= 1.0001; a += 0.01) {
      var ok = (unders || ['#000000', '#ffffff']).every(function (u) {
        var lc = lumRgb(over(bg, a, u)), li = lum(ink);
        return (Math.max(lc, li) + 0.05) / (Math.min(lc, li) + 0.05) >= target;
      });
      if (ok) return round(a, 2);
    }
    return 1;
  }
  function chromePalette(rec, mode, opts, worldPal) {
    var base = buildPalette(DEFAULT_RECORD, mode, opts);
    var tint = clamp(((rec.chrome && rec.chrome.tint) != null ? rec.chrome.tint : 4) / 100, 0, 0.08);
    var bg = base.roles['surface-raised'];
    if (worldPal && tint) bg = gamut(mix(bg, { l: bg.l, c: 0.08, h: worldPal.accentHue }, tint * 3, true));
    var ink = base.roles.ink;
    /* The lowest opacity at which chrome text keeps 4.5:1 over pure black and pure white art. */
    var minAlpha = 1;
    for (var a = 0.55; a <= 1.0001; a += 0.01) {
      var okA = ['#000000', '#ffffff'].every(function (u) {
        var comp = over(bg, a, u);
        var lc = lumRgb(comp), li = lum(ink);
        return (Math.max(lc, li) + 0.05) / (Math.min(lc, li) + 0.05) >= 4.6;
      });
      if (okA) { minAlpha = a; break; }
    }
    return { base: base, bg: bg, minAlpha: round(minAlpha, 2) };
  }

  /* imagery.treatments: each 0–100, 0 is off. Records from before treatments combined name one
     `treatment` with a `strength`; they read as that single treatment. */
  var TREATMENTS = ['veil', 'vignette', 'duotone', 'blur', 'parallax'];
  function treatmentsOf(img) {
    var out = {};
    TREATMENTS.forEach(function (k) { out[k] = 0; });
    if (!img) return out;
    if (img.treatments && typeof img.treatments === 'object') TREATMENTS.forEach(function (k) { out[k] = clamp(Math.round(+img.treatments[k] || 0), 0, 100); });
    else if (TREATMENTS.indexOf(img.treatment) >= 0) out[img.treatment] = clamp(Math.round(img.strength != null ? +img.strength : 50), 0, 100);
    return out;
  }
  function compileWorld(rec, opts) {
    opts = opts || {};
    var attrs0 = {};
    rec = rec || DEFAULT_RECORD;
    var reader = opts.reader || {};
    var supports = opts.supports || detectSupports();
    var mode = opts.mode || 'light';
    var high = reader.contrast === 'high' || opts.contrast === 'high';
    var plain = reader.plain;
    var source = plain ? DEFAULT_RECORD : rec;
    var pal = buildPalette(source, mode, { contrast: high ? 'high' : 'standard' });
    var mat = source.material || {};
    var v = {};
    Object.keys(pal.hex).forEach(function (k) { v[k] = pal.hex[k]; });

    /* material */
    var decorOff = reader.decoration === 'off' || plain;
    v['grain'] = decorOff ? 'none' : grain(mat.grain || 0, mode);
    v['finish'] = decorOff ? 'none' : finishLayer(mat.finish || 'matte', mode);
    v['edge-light'] = edgeLight(decorOff ? 0 : (mat.edge || 0), mode);
    var sh = shadows(mat.shadow, mode);
    v['elevation-1'] = sh[0]; v['elevation-2'] = sh[1]; v['elevation-3'] = sh[2]; v['elevation-4'] = sh[3];

    /* chrome */
    var cp = chromePalette(source, mode, { contrast: high ? 'high' : 'standard' }, plain ? null : pal);
    var cm = reader.chromeMaterial || 'veil';
    if (reader.reduceTransparency || !supports.backdrop) cm = 'solid';
    var alpha = cm === 'solid' ? 1 : cm === 'veil' ? Math.max(cp.minAlpha, 0.84) : Math.max(cp.minAlpha, 0.72);
    v['chrome-bg'] = toHex(cp.bg, alpha);
    v['chrome-blur'] = cm === 'solid' ? '0px' : cm === 'veil' ? '10px' : '28px';
    v['chrome-saturate'] = cm === 'solid' ? '1' : '1.35';
    v['chrome-grain'] = cm === 'frosted' ? grain(1, mode) : 'none';
    v['chrome-ink'] = cp.base.hex.ink;
    v['chrome-ink-muted'] = cp.base.hex['ink-muted'];
    v['chrome-line'] = toHex(cp.base.roles.line, mode === 'dark' ? 0.9 : 0.75);
    v['chrome-accent'] = cp.base.hex.accent;
    v['chrome-on-accent'] = cp.base.hex['on-accent'];
    v['chrome-hover'] = mode === 'dark' ? '#ffffff14' : '#0f172a0f';
    v['chrome-bg-solid'] = toHex(cp.bg);
    v['chrome-accent-soft'] = cp.base.hex['accent-soft'];
    v['chrome-on-accent-soft'] = cp.base.hex['on-accent-soft'];
    v['chrome-shadow'] = mode === 'dark'
      ? 'inset 0 1px 0 0 rgb(255 255 255 / 0.07), 0 2px 6px rgb(0 0 0 / 0.36), 0 0 2px rgb(0 0 0 / 0.3)'
      : 'inset 0 1px 0 0 rgb(255 255 255 / 0.65), 0 2px 6px rgb(15 23 42 / 0.12), 0 0 2px rgb(15 23 42 / 0.1)';

    /* shape */
    var sv = shapeVars(source.shape || {}, supports);
    Object.keys(sv.vars).forEach(function (k) { v[k] = sv.vars[k]; });

    /* ornament */
    var ov = ornamentVars(decorOff ? { set: 'rule', level: 0 } : (source.ornament || {}));
    Object.keys(ov).forEach(function (k) { v[k] = ov[k]; });
    var edge = decorOff ? 'fade' : (source.ornament && source.ornament.edge) || 'fade';
    v['art-mask'] = artMask(edge);
    /* Art placement: header (art at the top, scrolls away), continuous (art fixed behind the page,
       seen in full at the top and in the margins), backdrop (art behind the page; only a band
       under the chrome). The content sheet's top edge takes the World's edge. */
    var hasArt = !plain && !decorOff && rec && rec.imagery && rec.imagery.art;
    var placement = !hasArt ? 'none' : (['header', 'continuous', 'backdrop'].indexOf(rec.imagery.placement) >= 0 ? rec.imagery.placement : 'continuous');
    /* The content sheet's material. solid (default) takes the World's edge, and feathers its sides
       only when that edge is a fade. veil and frosted are translucent like the chrome, with hard
       straight edges and a hairline: the lowest opacity at which muted text and links keep 4.6:1
       (and body text 7:1) over the art's measured range, or over pure black and white when it is
       unknown; veil never goes below 0.72 and blurs 16px, frosted never below 0.6 and blurs 32px.
       They fall back to solid without art behind, without backdrop-filter, in high contrast and
       under reduced transparency. */
    var sheet = mat.sheet === 'veil' || mat.sheet === 'frosted' ? mat.sheet : 'solid';
    if (placement !== 'continuous' && placement !== 'backdrop') sheet = 'solid';
    if (high || reader.reduceTransparency || !supports.backdrop) sheet = 'solid';
    var glass = sheet !== 'solid';
    /* imagery.title: 'page' (default); 'art' sets the World's mark, title and byline on the art
       above the edge, and the sheet starts at the World's navigation; 'hidden' is for art that
       carries the World's own wordmark: mark and title leave the screen (the title stays for screen
       readers) and come back whenever the art is not shown */
    var titleMode = placement === 'none' ? 'page' : rec.imagery.title === 'art' ? 'art' : rec.imagery.title === 'hidden' ? 'hidden' : 'page';
    var titleArt = titleMode === 'art';
    var se = sheetEdge(glass ? 'straight' : edge);
    if (titleArt && !glass && edge === 'fade') se.lip = '72px';
    v['sheet-edge'] = se.image;
    v['lip'] = se.lip;
    /* a fade dissolves the art into the page across the full width; every other edge is a
       column with hard sides, so the art shows in the margins */
    v['sheet-w'] = !glass && edge === 'fade' ? '100%' : 'min(100%, 1080px)';
    if (glass) {
      /* imagery.range: the darkest and lightest colours of the art after a 32px blur, measured once
         when the theme is published (like `dominant`). Without it, assume pure black and white. */
      var range = rec.imagery.range && rec.imagery.range.length === 2 ? rec.imagery.range : null;
      var sa = Math.max(minAlphaFor(pal.roles.surface, pal.roles['ink-muted'], 4.6, range), minAlphaFor(pal.roles.surface, pal.roles['accent-ink'], 4.6, range), minAlphaFor(pal.roles.surface, pal.roles.ink, 7, range));
      sa = Math.max(sa, sheet === 'veil' ? 0.72 : 0.6);
      v['sheet-bg'] = toHex(pal.roles.surface, sa);
      v['sheet-filter'] = 'blur(' + (sheet === 'veil' ? 16 : 32) + 'px) saturate(1.25)';
      v['sheet-line'] = 'var(--frame-line)';
      v['sheet-grain'] = sheet === 'frosted' ? grain(1, mode) : 'none';
    } else {
      v['sheet-bg'] = 'var(--surface)';
      v['sheet-filter'] = 'none';
      v['sheet-line'] = 'transparent';
      v['sheet-grain'] = 'var(--grain)';
    }
    var room = 'calc(var(--chrome-offset, 72px) + var(--title-room, 200px) + var(--lip, 0px))';
    v['art-window'] = placement === 'none' ? 'var(--chrome-offset, 72px)'
      : placement === 'backdrop' ? (titleArt ? room : 'calc(var(--chrome-offset, 72px) + 40px)')
      : titleArt ? 'max(var(--art-h), ' + room + ')' : 'initial';
    v['mark-lift'] = placement === 'none' || titleArt ? '0px' : 'initial';
    var extra = [];
    if (titleArt) {
      /* Title colour from the art itself: dark ink over light art, light ink over dark art. Its
         backing (imagery.titleBacking) is a 'shade' just opaque enough to keep 4.6:1 over the art's
         worst measured colour (pure black or white when unmeasured; none at all when the art already
         contrasts), a 'halo' behind the letters only, or 'none', which is kept only while the art
         itself gives 4.6:1 and otherwise becomes a halo. */
      var rng = rec.imagery.range && rec.imagery.range.length === 2 ? rec.imagery.range : ['#000000', '#ffffff'];
      var dom = rec.imagery.dominant ? fromHex(rec.imagery.dominant) : null;
      var lightArt = dom ? dom.l > 0.64 : fromHex(rng[0]).l > 0.5;
      var nh0 = pal.neutral.h, nc0 = Math.min(pal.neutral.c, 0.012);
      var tInk = gamut(lightArt ? { l: 0.2, c: nc0 * 1.5, h: nh0 } : { l: 0.985, c: nc0, h: nh0 });
      var scrim = gamut(lightArt ? { l: 0.97, c: nc0, h: nh0 } : { l: 0.12, c: nc0 * 1.5, h: nh0 });
      var worst = lightArt ? rng[0] : rng[1], tl = lum(tInk), sa = 0.95;
      for (var ta = 0; ta <= 0.95; ta += 0.02) {
        var lc = lumRgb(over(scrim, ta, worst));
        if ((Math.max(lc, tl) + 0.05) / (Math.min(lc, tl) + 0.05) >= 4.6) { sa = ta; break; }
      }
      var backing = ['shade', 'halo', 'none'].indexOf(rec.imagery.titleBacking) >= 0 ? rec.imagery.titleBacking : 'shade';
      if (backing === 'none' && contrast(tInk, worst) < 4.6) {
        backing = 'halo';
        extra.push({ role: 'title', kind: 'contrast', from: null, to: toHex(tInk), target: 4.6,
          text: 'Title given a halo: on the ' + (lightArt ? 'darkest' : 'lightest') + ' parts of this art it would fall below 4.6:1 without one.' });
      }
      v['title-ink'] = toHex(tInk);
      v['title-scrim'] = backing === 'shade' && sa > 0 ? toHex(scrim, round(sa, 2)) : 'transparent';
      v['title-halo'] = backing === 'halo' ? '0 0 1px ' + toHex(scrim, 0.9) + ', 0 1px 3px ' + toHex(scrim, 0.85) + ', 0 0 14px ' + toHex(scrim, 0.75) : 'none';
      v['title-gap'] = 'calc(var(--lip, 0px) + 20px)';
      attrs0['data-title-backing'] = backing;
    }

    /* Treatments combine: veil, vignette, duotone, blur and parallax, each 0–1 here (0 is off).
       Parallax stops for readers who reduce motion. */
    var tr = treatmentsOf(!plain && !decorOff && rec && rec.imagery);
    TREATMENTS.forEach(function (k) { v['art-' + k] = String(round(tr[k] / 100, 2)); });
    if (reader.reduceMotion) v['art-parallax'] = '0';

    /* type, motion, icons, density */
    var tv = typeVars(source.type || {}, reader);
    Object.keys(tv).forEach(function (k) { v[k] = tv[k]; });
    var mv = motionVars(source.motion, reader);
    Object.keys(mv).forEach(function (k) { v[k] = mv[k]; });
    var iv = iconVars(source.icons, mode);
    Object.keys(iv).forEach(function (k) { v[k] = iv[k]; });
    var dn = DENSITY[reader.density || source.density || 'standard'] || DENSITY.standard;
    v['control-h'] = Math.max(dn.control, reader.coarse ? 44 : 0) + 'px';
    v['density'] = String(dn.gap);
    v['post-gap'] = dn.post + 'px';
    if (reader.textScale) v['text-scale'] = String(clamp(reader.textScale, 0.85, 1.6));

    var comp = source.composition || {};
    v['plate-bg'] = comp.plate === 'accent' ? 'var(--accent)' : 'var(--ink)';
    v['plate-ink'] = comp.plate === 'accent' ? 'var(--on-accent)' : 'var(--surface)';
    var attrs = {
      'data-world': rec.id || rec.name || 'world',
      'data-mode': mode,
      'data-contrast': high ? 'high' : 'standard',
      'data-post': ['open', 'card', 'dossier', 'log'].indexOf(comp.post) >= 0 ? comp.post : 'open',
      'data-portrait': comp.portrait || 'gutter',
      'data-corner': sv.corner,
      'data-profile': sv.profile,
      'data-names': comp.names || 'plain',
      'data-orn': (decorOff ? 'rule' : ((source.ornament && source.ornament.set) || 'rule')),
      'data-orn-level': ov['orn-level'],
      'data-orn-line': ov['orn-line'],
      'data-paragraphs': (source.type && source.type.paragraphs) || 'spaced',
      'data-dropcap': (!decorOff && source.ornament && source.ornament.dropCap) ? 'on' : 'off',
      'data-motion': (source.motion && source.motion.character) || 'standard',
      'data-parallax': +v['art-parallax'] > 0 ? 'on' : 'off',
      'data-art': placement,
      'data-title': titleMode,
      'data-sheet': sheet,
      'data-frame': (source.shape && source.shape.frame) || 'none'
    };
    Object.keys(attrs0).forEach(function (k) { attrs[k] = attrs0[k]; });
    var adj = pal.adjustments.concat(extra);
    return { vars: v, attrs: attrs, palette: pal, mode: mode, notes: adj.map(function (a) { return a.text; }), adjustments: adj };
  }

  function cssText(vars) {
    return Object.keys(vars).map(function (k) { return '--' + k + ': ' + vars[k] + ';'; }).join(' ');
  }
  function applyWorld(el, rec, opts) {
    var out = compileWorld(rec, opts);
    Object.keys(out.vars).forEach(function (k) { el.style.setProperty('--' + k, out.vars[k]); });
    Object.keys(out.attrs).forEach(function (k) { el.setAttribute(k, out.attrs[k]); });
    if (rec && rec.imagery && rec.imagery.art && !(opts && opts.reader && (opts.reader.plain || opts.reader.decoration === 'off'))) {
      el.style.setProperty('--art', 'url("' + rec.imagery.art + '")');
      var fo = rec.imagery.focus || [50, 50];
      el.style.setProperty('--art-focus', fo.join('% ') + '%');
      el.style.setProperty('--art-fy', String(clamp(fo[1], 0, 100) / 100));
    } else {
      el.style.setProperty('--art', 'none');
    }
    if (out.attrs['data-parallax'] === 'on') drift.start();
    return out;
  }
  /* Parallax for browsers without scroll-driven animations. The recipes translate the art layer by
     --art-drift: the px scrolled through the first screen for fixed art, and through the art's own
     box as it leaves the top of the viewport for header art. One passive listener, one write per
     frame; nothing runs where CSS does the work or the reader reduces motion. */
  var drift = (function () {
    var started = false, frame = 0, still = null;
    var SEL = '[data-parallax=on] .m-world__backdrop, [data-parallax=on] .m-masthead__art';
    function cssDoesIt() {
      return !!(window.CSS && CSS.supports && CSS.supports('animation-timeline: view()') && CSS.supports('view-timeline-name: --m-art'));
    }
    function tick() {
      frame = 0;
      var els = document.querySelectorAll(SEL), d = [], y = window.scrollY || 0, vh = window.innerHeight || 0, off = still && still.matches;
      for (var i = 0; i < els.length; i++) {
        if (off) { d.push(0); continue; }
        if (els[i].classList.contains('m-world__backdrop')) { d.push(clamp(y, 0, vh)); continue; }
        var r = els[i].getBoundingClientRect();
        d.push(clamp(-r.top, 0, r.height));
      }
      for (var j = 0; j < els.length; j++) els[j].style.setProperty('--art-drift', round(d[j], 1) + 'px');
    }
    function request() { if (started && !frame) frame = requestAnimationFrame(tick); }
    return {
      start: function () {
        if (typeof window === 'undefined' || !window.requestAnimationFrame) return;
        if (!started) {
          if (cssDoesIt()) return;
          started = true;
          still = window.matchMedia ? window.matchMedia('(prefers-reduced-motion: reduce)') : null;
          window.addEventListener('scroll', request, { passive: true });
          window.addEventListener('resize', request, { passive: true });
          if (still && still.addEventListener) still.addEventListener('change', request);
        }
        request();
      },
      update: request,
    };
  })();
  /* Mount a World on an element and keep it in step with the page's light/dark theme. */
  function currentMode() {
    var t = document.documentElement.getAttribute('data-theme');
    if (t === 'dark' || t === 'contrast-dark') return 'dark';
    if (t === 'light' || t === 'contrast-light') return 'light';
    return window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  function currentContrast() {
    var t = document.documentElement.getAttribute('data-theme') || '';
    return t.indexOf('contrast') === 0 ? 'high' : 'standard';
  }
  function mountWorld(el, rec, opts) {
    opts = opts || {};
    var run = function () {
      var o = {};
      Object.keys(opts).forEach(function (k) { o[k] = opts[k]; });
      /* 'world' = the reader chose "World default": the World's preferred mode, else the page's */
      o.mode = opts.mode === 'world' ? ((rec.palette && rec.palette.mode) || currentMode()) : (opts.mode || currentMode());
      o.contrast = opts.contrast || currentContrast();
      return applyWorld(el, rec, o);
    };
    var res = run();
    if ((!opts.mode || opts.mode === 'world') && typeof MutationObserver !== 'undefined') {
      new MutationObserver(run).observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    }
    return { update: function (r, o) { rec = r || rec; if (o) opts = o; return run(); }, result: res };
  }

  /* ================================================================
     Interchange: place.mode.standard (v1) and DTCG 2025.10
     ================================================================ */
  function toStandardMode(rec, opts) {
    opts = opts || {};
    var out = compileWorld(rec, { mode: opts.mode || 'light', supports: { cornerShape: false, backdrop: false } });
    var h = out.vars;
    var ci = function (bg, fg, border) { return { background: bg, foreground: fg, border: border }; };
    var face = FACES[(rec.type && rec.type.display) || 'motet'] || FACES.motet;
    var family = function (stack) { return stack.split(',')[0].replace(/"/g, '').trim(); };
    return {
      $type: 'place.mode.standard.theme',
      displayName: (rec.name || 'World').slice(0, 64),
      description: opts.description,
      aspects: {
        color: {
          control: ci(h['surface-raised'], h.ink, h['line-strong']),
          action: {
            primary: ci(h.accent, h['on-accent'], h.accent),
            secondary: ci(h['accent-soft'], h['on-accent-soft'], h['accent-soft']),
            auxiliary: ci(h.surface, h['accent-ink'], h['line-strong'])
          },
          surface: {
            primary: ci(h['surface-raised'], h.ink, h.line),
            secondary: ci(h['surface-overlay'], h.ink, h.line),
            auxiliary: ci(h.surface, h.ink, h.line)
          }
        },
        typography: {
          text: {
            primary: { size: '1.125rem', weight: '400', family: family(h['font-reading']), lineHeight: '1.6' },
            secondary: { size: '0.9375rem', weight: '400', family: 'IBM Plex Sans', lineHeight: '1.45' },
            auxiliary: { size: '0.8125rem', weight: '500', family: family(face.stack), lineHeight: '1.35' }
          }
        }
      },
      fonts: opts.fonts || [],
      createdAt: opts.createdAt || new Date().toISOString()
    };
  }
  function toDTCG(rec, opts) {
    var modes = ['light', 'dark'];
    var res = {};
    modes.forEach(function (m) {
      var o = compileWorld(rec, { mode: m, supports: { cornerShape: true, backdrop: true } });
      var g = { color: {}, radius: {}, duration: {} };
      Object.keys(o.palette.hex).forEach(function (k) { g.color[k] = { $type: 'color', $value: o.palette.hex[k] }; });
      ['xs', 'sm', 'md', 'lg', 'xl'].forEach(function (k) { g.radius[k] = { $type: 'dimension', $value: { value: parseFloat(o.vars['radius-' + k]), unit: 'px' } }; });
      ['fast', 'default', 'slow'].forEach(function (k) { g.duration['spatial-' + k] = { $type: 'duration', $value: { value: parseFloat(o.vars['dur-spatial-' + k]), unit: 'ms' } }; });
      res[m] = g;
    });
    return res;
  }

  /* ================================================================
     Records: Motet's own appearance and six starting themes
     ================================================================ */
  var DEFAULT_RECORD = {
    id: 'motet', name: 'Motet',
    palette: { primary: '#2b6b8a', secondary: '#5a7d8f', scheme: 'tonal', chroma: 95, temperature: -15 },
    material: { finish: 'satin', grain: 0, tint: 0, edge: 1, shadow: 1 },
    shape: { corner: 'squircle', scale: 100, border: 'hairline', profile: 'even' },
    ornament: { set: 'rule', level: 0, edge: 'fade' },
    type: { display: 'motet', 'case': 'none', tracking: -12, weight: 600, names: 'ui', reading: 'serif', numerals: 'proportional' },
    icons: { style: 'outlined', weight: 400 },
    motion: { character: 'standard' },
    density: 'standard',
    composition: { post: 'open', portrait: 'gutter', names: 'plain' },
    chrome: { tint: 0 }
  };
  var PRESETS = {
    motet: DEFAULT_RECORD,
    /* Angular sci-fi after Cyberpunk 2077 and Destiny: clipped panels, an offset accent bar
       notched into each frame, technical uppercase display type, crisp motion. */
    neon: {
      id: 'neon', name: 'Neon Meridian',
      palette: { primary: '#19c3d6', secondary: '#e2408e', scheme: 'vibrant', chroma: 160, temperature: -60, depth: { dark: -1, light: 0 }, mode: 'dark' },
      material: { finish: 'sheen', grain: 2, tint: 5, edge: 1, shadow: 1, sheet: 'frosted' },
      shape: { corner: 'bevel', scale: 60, border: 'rule', profile: 'clipped', tone: 'neutral' },
      ornament: { set: 'segment', level: 2, edge: 'fade' },
      type: { display: 'oxanium', 'case': 'upper', tracking: 60, weight: 600, names: 'display', reading: 'sans', numerals: 'tabular' },
      icons: { style: 'sharp', weight: 300 },
      motion: { character: 'crisp' },
      density: 'standard',
      composition: { post: 'card', portrait: 'gutter', names: 'plain' },
      imagery: { placement: 'backdrop', treatments: { duotone: 85 }, focus: [50, 70] }
    },
    /* Post-apocalyptic: cut steel plates, stencilled name plates, heavy grain, a burned edge. */
    ashfall: {
      id: 'ashfall', name: 'Ashfall',
      palette: { primary: '#b5532c', secondary: '#c9a24a', scheme: 'tonal', chroma: 75, temperature: 65, depth: { dark: 0, light: -1 }, mode: 'dark' },
      material: { finish: 'matte', grain: 4, tint: 6, edge: 0, shadow: 1 },
      shape: { corner: 'square', scale: 0, border: 'heavy', profile: 'cut', tone: 'neutral' },
      ornament: { set: 'rule', level: 0, edge: 'burned' },
      type: { display: 'big-shoulders-stencil', 'case': 'upper', tracking: 40, weight: 800, names: 'display', reading: 'serif', numerals: 'tabular' },
      icons: { style: 'sharp', weight: 500 },
      motion: { character: 'crisp' },
      density: 'standard',
      composition: { post: 'card', portrait: 'gutter', names: 'plate', plate: 'ink' },
      imagery: { placement: 'continuous', treatments: { vignette: 90 }, focus: [50, 60] }
    },
    /* Restrained high fantasy: gilt hairlines, filigree, drop caps, chapter rhythm. */
    vellum: {
      id: 'vellum', name: 'The Gilded Reach',
      palette: { primary: '#2d4f8f', secondary: '#b8862b', scheme: 'fidelity', chroma: 100, temperature: 45, depth: { light: 0, dark: 0 }, mode: 'light' },
      material: { finish: 'satin', grain: 2, tint: 3, edge: 1, shadow: 2 },
      shape: { corner: 'round', scale: 90, border: 'hairline', profile: 'even', tone: 'ornament', frame: 'double' },
      ornament: { set: 'filigree', level: 3, edge: 'arch', dropCap: true },
      type: { display: 'cormorant', 'case': 'none', tracking: 0, weight: 600, names: 'display', reading: 'serif', numerals: 'oldstyle', paragraphs: 'indented' },
      icons: { style: 'rounded', weight: 400 },
      motion: { character: 'soft' },
      density: 'comfortable',
      composition: { post: 'open', portrait: 'gutter', names: 'plain' },
      imagery: { placement: 'continuous', treatments: { veil: 10 }, focus: [50, 52] }
    },
    /* Gothic horror: near-black, blackletter names, fading rules, slow decorative fades. */
    hollow: {
      id: 'hollow', name: 'Hollowmere',
      palette: { primary: '#8e2a3a', secondary: '#7d6f86', scheme: 'neutral', chroma: 30, temperature: 10, depth: { dark: -2, light: -1 }, mode: 'dark' },
      material: { finish: 'matte', grain: 3, tint: 4, edge: 0, shadow: 2 },
      shape: { corner: 'round', scale: 35, border: 'hairline', profile: 'even' },
      ornament: { set: 'fade', level: 1, edge: 'torn' },
      type: { display: 'grenze-gotisch', 'case': 'none', tracking: 10, weight: 500, names: 'display', reading: 'serif', numerals: 'oldstyle' },
      icons: { style: 'outlined', weight: 300 },
      motion: { character: 'eerie' },
      density: 'comfortable',
      composition: { post: 'open', portrait: 'gutter', names: 'plain' },
      imagery: { placement: 'backdrop', treatments: { vignette: 100 }, focus: [50, 40] }
    },
    /* Contemporary strategy after Papers, Please and Orwell: posts filed as documents with a form header. */
    ledger: {
      id: 'ledger', name: 'Cold Front',
      palette: { primary: '#1f3f73', secondary: '#e0662b', scheme: 'tonal', chroma: 85, temperature: -20, depth: { light: 0, dark: 0 }, mode: 'light' },
      material: { finish: 'matte', grain: 0, tint: 0, edge: 1, shadow: 0 },
      shape: { corner: 'squircle', scale: 45, border: 'rule', profile: 'even' },
      ornament: { set: 'rule', level: 0, edge: 'straight' },
      type: { display: 'archivo', 'case': 'upper', tracking: 30, weight: 700, names: 'ui', reading: 'sans', numerals: 'tabular' },
      icons: { style: 'outlined', weight: 400 },
      motion: { character: 'standard' },
      density: 'compact',
      composition: { post: 'dossier', portrait: 'inline', names: 'plain' },
      imagery: { placement: 'header', treatments: { veil: 25 }, focus: [50, 50] }
    },
    /* Diegetic terminal after Alien: Isolation and the Pip-Boy: one phosphor hue, monospaced
       labels, log-style posts without portraits. Prose stays proportional and readable. */
    kepler: {
      id: 'kepler', name: 'Kepler Relay',
      palette: { primary: '#f0a33a', secondary: '#f0a33a', neutral: '#2e2414', scheme: 'tonal', chroma: 140, temperature: 0, depth: { dark: -1, light: 0 }, mode: 'dark' },
      material: { finish: 'matte', grain: 3, tint: 0, edge: 0, shadow: 0, sheet: 'veil' },
      shape: { corner: 'square', scale: 0, border: 'rule', profile: 'even', tone: 'ornament' },
      ornament: { set: 'segment', level: 1, edge: 'straight' },
      type: { display: 'mono', 'case': 'upper', tracking: 80, weight: 500, names: 'display', reading: 'sans', numerals: 'tabular' },
      icons: { style: 'sharp', weight: 300 },
      motion: { character: 'crisp' },
      density: 'compact',
      composition: { post: 'log', portrait: 'none', names: 'plain' },
      imagery: { placement: 'backdrop', treatments: {}, focus: [50, 60] }
    }
  };

  /* ================================================================
     DOM factories: framework-neutral markup for the Panda recipes.
     Class names are exactly what the recipes emit, so a Svelte or
     React layer can use the generated recipe functions instead.
     ================================================================ */
  function h(tag, attrs) {
    var el = document.createElement(tag);
    var a = attrs || {};
    Object.keys(a).forEach(function (k) {
      var val = a[k];
      if (val == null || val === false) return;
      if (k === 'class') el.className = Array.isArray(val) ? val.filter(Boolean).join(' ') : val;
      else if (k === 'html') el.innerHTML = val;
      else if (k === 'style' && typeof val === 'object') Object.keys(val).forEach(function (s) { el.style.setProperty(s, val[s]); });
      else if (k.slice(0, 2) === 'on' && typeof val === 'function') el.addEventListener(k.slice(2), val);
      else el.setAttribute(k, val === true ? '' : val);
    });
    for (var i = 2; i < arguments.length; i++) append(el, arguments[i]);
    return el;
  }
  function append(el, c) {
    if (c == null || c === false) return;
    if (Array.isArray(c)) { c.forEach(function (x) { append(el, x); }); return; }
    el.appendChild(typeof c === 'string' || typeof c === 'number' ? document.createTextNode(String(c)) : c);
  }
  /* Slot recipe classes, as Panda's runtime would return them: `m-post__name m-post__name--lane_ooc`. */
  function slot(recipe, part, variants) {
    var base = recipe + '__' + part;
    var out = [base];
    Object.keys(variants || {}).forEach(function (k) {
      if (variants[k] != null) out.push(base + '--' + k + '_' + variants[k]);
    });
    return out.join(' ');
  }
  function icon(name, opts) {
    opts = opts || {};
    return h('span', { 'class': ['m-icon', opts.fill && 'm-icon--fill', opts.size && 'm-icon--' + opts.size, opts['class']], 'aria-hidden': 'true' }, name);
  }
  function Button(p) {
    p = p || {};
    return h(p.href ? 'a' : 'button', {
      'class': ['m-button', 'm-button--variant_' + (p.variant || 'tonal'), p.tone && p.tone !== 'accent' && 'm-button--tone_' + p.tone, p.size === 'sm' && 'm-button--size_sm', p['class']],
      type: p.href ? null : (p.type || 'button'), href: p.href, disabled: p.disabled, 'aria-pressed': p.pressed, onclick: p.onClick,
      'aria-label': p.ariaLabel
    }, p.icon && icon(p.icon), h('span', null, p.label), p.trailingIcon && icon(p.trailingIcon));
  }
  function IconButton(p) {
    p = p || {};
    return h('button', {
      'class': ['m-icon-button', 'm-icon-button--variant_' + (p.variant || 'standard'), p.size === 'sm' && 'm-icon-button--size_sm', p['class']],
      type: 'button', 'aria-label': p.label, 'data-tip': p.tip === false ? null : p.label, 'aria-pressed': p.pressed,
      'aria-expanded': p.expanded, 'aria-haspopup': p.haspopup, onclick: p.onClick, 'data-optional': p.optional
    }, icon(p.icon, { fill: p.pressed === 'true' || p.pressed === true }),
      p.count ? h('span', { 'class': 'm-icon-button__count', 'aria-hidden': 'true' }, p.count > 99 ? '99+' : String(p.count)) : null);
  }
  function Chip(p) {
    p = p || {};
    var kind = p.kind || 'filter';
    var isBtn = kind === 'filter';
    return h(isBtn ? 'button' : 'span', {
      'class': ['m-chip', 'm-chip--kind_' + kind, p.tone && 'm-chip--tone_' + p.tone],
      type: isBtn ? 'button' : null, 'aria-pressed': isBtn ? String(!!p.selected) : null, title: p.title,
      onclick: isBtn ? function (e) { var b = e.currentTarget; var on = b.getAttribute('aria-pressed') !== 'true'; b.setAttribute('aria-pressed', String(on)); var ic = b.querySelector('.m-icon'); if (on && !ic) b.insertBefore(icon('check'), b.firstChild); if (!on && ic && !p.icon) ic.remove(); } : null
    }, (p.selected && isBtn) ? icon('check') : (p.icon ? icon(p.icon) : null), h('span', null, p.label));
  }
  function Flag(p) {
    return h('span', { 'class': ['m-flag', 'm-flag--kind_' + (p.kind || 'draft')] }, p.icon && icon(p.icon, { size: 'sm' }), p.label);
  }
  function hueOf(name) {
    return (String(name || '').split('').reduce(function (a, c) { return a + c.charCodeAt(0); }, 0) * 37) % 360;
  }
  function initials(name) {
    return String(name || '?').replace(/^@/, '').split(/[\s.]+/).filter(Boolean).slice(0, 2).map(function (w) { return w[0]; }).join('').toUpperCase();
  }
  function Avatar(p) {
    p = p || {};
    return h('span', {
      'class': ['m-avatar', 'm-avatar--kind_' + (p.kind || 'account'), p.size !== false && 'm-avatar--size_' + (p.size || 'md'), p['class']],
      style: { '--avatar-hue': String(p.hue != null ? p.hue : hueOf(p.name)) },
      role: p.decorative ? null : 'img', 'aria-label': p.decorative ? null : p.name, 'aria-hidden': p.decorative ? 'true' : null
    }, p.src ? h('img', { src: p.src, alt: '' }) : h('span', { 'aria-hidden': 'true' }, initials(p.name)));
  }
  function Divider() {
    return h('div', { 'class': 'm-divider__root', role: 'separator' },
      h('span', { 'class': 'm-divider__line' }), h('span', { 'class': 'm-divider__motif' }), h('span', { 'class': 'm-divider__line' }));
  }
  function roleBadge(role) {
    if (!role) return null;
    var map = { gm: ['shield_person', 'GM'], mod: ['gavel', 'Moderator'], bot: ['data_object', 'Automation'] };
    var r = map[role] || [null, role];
    return h('span', { 'class': 'm-role' }, r[0] && icon(r[0]), r[1]);
  }
  function Meta(iconName, text, title) {
    return h('span', { 'class': 'm-meta', title: title || null }, icon(iconName, { size: 'sm' }), h('span', null, text));
  }
  /* Post: the character speaks; the account that wrote it is always on the second line. */
  function Post(p) {
    p = p || {};
    var ch = p.character, acct = p.account || {};
    var ooc = p.lane === 'ooc' || !ch;
    var V = { lane: ooc ? 'ooc' : 'ic' };
    var S = function (part) { return slot('m-post', part, V); };
    var speakerName = ooc ? (acct.display || acct.handle) : ch.name;
    var portrait = function (size) {
      return ooc ? Avatar({ kind: 'account', name: acct.display || acct.handle, src: acct.avatar, size: size === 'lg' ? 'md' : 'sm', decorative: true })
        : Avatar({ kind: 'character', name: ch.name, src: ch.portrait, size: size, hue: ch.hue, decorative: true });
    };
    return h('article', { 'class': [S('root'), 'm-marked', p.unread && 'm-post__root--unread'], 'aria-label': speakerName + (ooc ? ', out of character' : '') },
      h('div', { 'class': S('portrait') }, portrait('lg')),
      h('div', { 'class': S('main') },
        h('header', { 'class': S('header') },
          h('div', { 'class': S('mini') }, portrait('sm')),
          h('div', { 'class': S('speaker') },
            h('span', { 'class': S('name') }, speakerName),
            !ooc && ch.epithet ? h('span', { 'class': S('epithet') }, ch.epithet) : null),
          h('div', { 'class': S('author') },
            ooc ? null : icon('edit_note'),
            h('span', { 'class': S('handle'), title: acct.did || null }, ooc ? '@' + acct.handle : (acct.display ? acct.display + ' ' : '') + '@' + acct.handle),
            roleBadge(p.role)),
          h('div', { 'class': S('meta') },
            p.unread ? Flag({ kind: 'turn', label: 'New' }) : null,
            p.edited ? h('span', { 'class': 'm-meta', title: 'Edited ' + p.edited, 'data-tip': 'Edited ' + p.edited }, icon('history', { size: 'sm' }), h('span', { 'class': 'm-visually-hidden' }, 'Edited')) : null,
            h('time', { datetime: p.datetime || null, title: p.timeFull || null }, p.time || ''),
            IconButton({ icon: 'more_horiz', label: 'Post actions', size: 'sm', haspopup: 'menu' }))),
        p.follows ? h('a', { 'class': S('follows'), href: p.follows.href || '#' }, icon(p.follows.icon || 'casino'), h('span', null, 'Follows ', h('b', null, p.follows.label))) : null,
        h('div', { 'class': [S('body'), 'm-prose', ooc && 'm-prose--ui', p.lead && 'm-prose--lead'], html: p.body || '' }),
        h('footer', { 'class': S('actions') },
          Button({ variant: 'text', size: 'sm', icon: 'reply', label: 'Reply' }),
          Button({ variant: 'text', size: 'sm', icon: 'format_quote', label: 'Quote' }),
          (p.notes || []).map(function (n) { return Chip({ kind: 'note', label: n, icon: 'info' }); }))));
  }
  /* Dice faces from DiceFont (fponticelli/dicefont, SIL OFL 1.1): d2, d4, d6, d8, d10, d12, d20 and Fate. */
  var DICE = {"F-minus":61841,"F-plus":61842,"F-zero":61843,"d10-0":61696,"d10-1":61697,"d10-10":61698,"d10-2":61699,"d10-3":61700,"d10-4":61701,"d10-5":61702,"d10-6":61703,"d10-7":61704,"d10-8":61705,"d10-9":61706,"d12-1":61707,"d12-10":61708,"d12-11":61709,"d12-12":61710,"d12-2":61711,"d12-3":61712,"d12-4":61713,"d12-5":61714,"d12-6":61715,"d12-7":61716,"d12-8":61717,"d12-9":61718,"d2-1":61719,"d2-2":61720,"d20-1":61721,"d20-10":61722,"d20-11":61723,"d20-12":61724,"d20-13":61725,"d20-14":61726,"d20-15":61727,"d20-16":61728,"d20-17":61729,"d20-18":61730,"d20-19":61731,"d20-2":61732,"d20-20":61733,"d20-3":61734,"d20-4":61735,"d20-5":61736,"d20-6":61737,"d20-7":61738,"d20-8":61739,"d20-9":61740,"d4-1":61741,"d4-2":61742,"d4-3":61743,"d4-4":61744,"d6-1":61745,"d6-2":61746,"d6-3":61747,"d6-4":61748,"d6-5":61749,"d6-6":61750,"d8-1":61751,"d8-2":61752,"d8-3":61753,"d8-4":61754,"d8-5":61755,"d8-6":61756,"d8-7":61757,"d8-8":61758};
  function dieGlyph(sides, value) {
    var key = sides === 'F' ? ('F-' + (value > 0 ? 'plus' : value < 0 ? 'minus' : 'zero')) : ('d' + sides + '-' + (sides === 10 && value === 10 ? 0 : value));
    return DICE[key] ? String.fromCharCode(DICE[key]) : null;
  }
  function DiceRoll(p) {
    p = p || {};
    var S = function (part) { return 'm-roll__' + part; };
    var sides = p.sides || (/d(\d+|F)/i.exec(p.expr || '') || [0, 6])[1];
    if (sides !== 'F' && sides !== 'f') sides = +sides; else sides = 'F';
    return h('figure', { 'class': S('root') },
      p.caption !== false && (p.label || p.expr) ? h('figcaption', { 'class': S('label') }, h('span', { 'class': S('what') }, p.label), h('code', { 'class': S('expr') }, p.expr)) : null,
      h('div', { 'class': S('dice'), role: 'list', 'aria-label': 'Dice' }, (p.results || []).map(function (r, i) {
        var dropped = p.dropped && p.dropped.indexOf(i) >= 0;
        var g = dieGlyph(sides, r);
        return h('span', { 'class': [S('die'), g && 'm-roll__die--glyph'], role: 'listitem', 'data-dropped': dropped || null, title: 'd' + sides + ': ' + r + (dropped ? ', dropped' : '') },
          g ? h('span', { 'class': 'm-die', 'aria-hidden': 'true' }, g) : String(r),
          g ? h('span', { 'class': 'm-visually-hidden' }, r + (dropped ? ', dropped' : '')) : null);
      })),
      h('div', { 'class': S('total') }, h('span', { 'class': 'm-visually-hidden' }, 'Total '), String(p.total)),
      p.outcome ? h('div', { 'class': S('outcome'), 'data-tone': p.outcomeTone || 'positive' }, icon(p.outcomeIcon || 'check_circle'), p.outcome) : null);
  }
  /* A scene event: something that happened between posts. Rolls, roll requests, a round starting,
     a tool's output. Never inside a post: its own block in the stream, in time order.
     p: { kind: 'roll'|'request'|'system'|'tool', icon, title, expr, detail, character, account {display, handle, did},
          time, timeFull, datetime, roll (DiceRoll props), body (a node: any tool's UI), source {icon, label, href},
          actions [nodes], lane ('ic'|'ooc'), label (accessible name) } */
  function SceneEvent(p) {
    p = p || {};
    var kind = p.kind || (p.roll ? 'roll' : 'tool');
    var S = function (part) { return slot('m-event', part, { kind: kind }); };
    var acct = p.account || {};
    var detail = p.detail;
    if (detail == null && (p.character || acct.handle)) {
      detail = [p.character ? h('span', null, p.character) : null,
        acct.handle ? h('span', { title: acct.did || null }, (p.character ? 'by ' : '') + (acct.display ? acct.display + ' ' : '') + '@' + acct.handle) : null];
    }
    var glyph = p.icon || { roll: 'casino', request: 'front_hand', system: 'flag', tool: 'widgets' }[kind] || 'widgets';
    var body = p.body || (p.roll ? DiceRoll(Object.assign({}, p.roll, { caption: false })) : null);
    var name = p.label || (typeof p.title === 'string' ? p.title : '') + (p.roll && p.roll.total != null ? ', ' + p.roll.total : '');
    return h('section', { 'class': S('root'), 'data-lane': p.lane === 'ooc' ? 'ooc' : 'ic', 'aria-label': name || null },
      h('div', { 'class': S('marker'), 'aria-hidden': 'true' }, icon(glyph)),
      h('div', { 'class': S('card') },
        h('div', { 'class': S('header') },
          h('p', { 'class': S('title') }, p.title, p.expr ? h('code', null, p.expr) : null),
          p.time ? h('time', { 'class': S('time'), datetime: p.datetime || null, title: p.timeFull || null }, p.time) : null,
          detail ? h('div', { 'class': S('detail') }, detail) : null),
        body ? h('div', { 'class': S('body') }, body) : null,
        p.source || (p.actions && p.actions.length) ? h('div', { 'class': S('footer') },
          p.source ? h('span', { 'class': S('source') }, icon(p.source.icon || 'widgets'), p.source.href ? h('a', { href: p.source.href }, p.source.label) : p.source.label) : null,
          p.actions && p.actions.length ? h('div', { 'class': S('actions') }, p.actions) : null) : null));
  }
  function SceneRow(p) {
    p = p || {};
    var V = { unread: p.unread ? 'true' : 'false' };
    var S = function (part) { return slot('m-scene-row', part, V); };
    var cast = p.cast || [];
    return h('a', { 'class': S('root'), href: p.href || '#' },
      h('span', { 'class': S('marker'), 'aria-hidden': 'true' }),
      h('span', { 'class': S('main') },
        h('span', { 'class': S('title') }, p.title, p.unread ? h('span', { 'class': 'm-visually-hidden' }, ', has unread posts') : null),
        h('span', { 'class': S('where') },
          p.location ? Meta('location_on', p.location) : null,
          p.arc ? Meta('auto_stories', p.arc) : null)),
      h('span', { 'class': S('state') },
        p.turn ? Flag({ kind: 'turn', icon: 'front_hand', label: 'Your turn' }) : null,
        p.draft ? Flag({ kind: 'draft', icon: 'draft', label: 'Draft' }) : null,
        p.unreadCount ? h('span', { 'class': 'm-count', title: p.unreadCount + ' unread' }, String(p.unreadCount)) : null),
      h('span', { 'class': S('cast'), title: 'Cast: ' + cast.join(', ') }, cast.length > 3 ? h('span', { 'class': S('more') }, '+' + (cast.length - 3)) : null,
        cast.slice(0, 3).map(function (c) { return Avatar({ kind: 'character', name: c, size: 'xs', decorative: true }); })),
      h('time', { 'class': S('time') }, p.updated || ''));
  }
  var INBOX_KINDS = { turn: ['front_hand', 'Response requested'], mention: ['alternate_email', 'Mention'], application: ['how_to_reg', 'Application'], post: ['forum', 'New posts'], decision: ['gavel', 'GM decision'], draft: ['draft', 'Draft'] };
  function InboxItem(p) {
    p = p || {};
    var V = { kind: p.kind || 'post', unread: p.unread ? 'true' : 'false' };
    var S = function (part) { return slot('m-inbox-item', part, V); };
    var k = INBOX_KINDS[p.kind] || INBOX_KINDS.post;
    return h('a', { 'class': S('root'), href: p.href || '#' },
      h('span', { 'class': S('world') }, Avatar({ kind: 'world', name: p.world, src: p.worldMark, size: 'sm', hue: p.worldHue, decorative: true })),
      h('span', { 'class': S('main') },
        h('span', { 'class': S('kind') }, icon(k[0], { size: 'sm' }), k[1], h('span', { 'class': S('in') }, p.world)),
        h('span', { 'class': S('title') }, p.title),
        p.excerpt ? h('span', { 'class': S('excerpt') }, p.excerpt) : null),
      h('span', { 'class': S('side') },
        h('time', null, p.time || ''),
        p.resolved ? Flag({ kind: 'done', icon: 'check', label: 'Done' }) : null));
  }
  function CharacterCard(p) {
    p = p || {};
    var S = function (part) { return 'm-character__' + part; };
    return h('article', { 'class': [S('root'), 'm-ornamented', 'm-marked'] },
      h('div', { 'class': S('portrait') }, p.portrait ? h('img', { src: p.portrait, alt: p.portraitAlt || '' }) : Avatar({ kind: 'character', name: p.name, size: 'xl' })),
      h('div', { 'class': S('body') },
        h('h3', { 'class': S('name') }, p.name),
        p.epithet ? h('span', { 'class': S('epithet') }, p.epithet) : null,
        h('dl', { 'class': S('facts') }, (p.facts || []).map(function (f) { return [h('dt', null, f[0]), h('dd', null, f[1])]; })),
        h('div', { 'class': S('owner') }, Avatar({ kind: 'account', name: p.account, size: 'xs', decorative: true }), h('span', null, 'Played by @' + p.account))),
      h('div', { 'class': S('foot') },
        p.status === 'pending' ? Flag({ kind: 'caution', icon: 'hourglass_top', label: 'Awaiting review' }) : Flag({ kind: 'done', icon: 'check_circle', label: 'Approved' }),
        h('span', { style: { 'margin-inline-start': 'auto', display: 'flex', gap: '4px' } },
          Button({ variant: 'text', size: 'sm', icon: 'description', label: 'Full card' }))));
  }
  function Tabs(items, current) {
    return h('nav', { 'class': 'm-tabs__root', 'aria-label': 'World' }, items.map(function (t) {
      return h('a', { 'class': 'm-tabs__tab', href: '#', 'aria-current': t[1] === current ? 'page' : null }, icon(t[0]), t[1]);
    }));
  }
  /* Masthead: full-bleed header art that fades (or tears, burns, arches) into the page; the
     identity sits below the art, on the page surface, never on the image. */
  function Masthead(p) {
    p = p || {};
    var V = { size: p.size || 'full' };
    var S = function (part) { return slot('m-masthead', part, V); };
    return h('header', { 'class': S('root') },
      h('div', { 'class': S('art'), role: p.artAlt ? 'img' : null, 'aria-label': p.artAlt || null }, h('span', { 'class': S('scrim'), 'aria-hidden': 'true' })),
      h('div', { 'class': S('inner') },
        h('div', { 'class': S('identity') },
          Avatar({ kind: 'world', name: p.name, src: p.mark, size: false, 'class': S('mark'), hue: p.hue, decorative: true }),
          h('div', { 'class': S('titles') },
            h('h1', { 'class': S('title') }, p.name),
            h('div', { 'class': S('byline') },
              h('span', { 'class': 'm-meta' }, icon('shield_person', { size: 'sm' }), h('span', null, (p.gms || []).join(', '))),
              p.state ? h('span', { 'class': 'm-meta' }, icon(p.stateIcon || 'how_to_reg', { size: 'sm' }), h('span', null, p.state)) : null,
              p.language ? h('span', { 'class': 'm-meta' }, icon('translate', { size: 'sm' }), h('span', null, p.language)) : null))),
        p.nav === false ? null : h('div', { 'class': S('nav') }, Tabs([['explore', 'Overview'], ['forum', 'Play'], ['group', 'Characters'], ['menu_book', 'Lore'], ['groups', 'Members']], p.current || 'Play'))));
  }
  /* Chrome: Motet's corner islands. A World can tint them a little; nothing else. */
  function Chrome(p) {
    p = p || {};
    var S = function (part) { return 'm-chrome__' + part; };
    return h('div', { 'class': S('root') },
      h('nav', { 'class': S('island'), 'aria-label': 'Motet' },
        h('a', { 'class': S('home'), href: '#', 'aria-label': 'Motet home' }, 'motet'),
        h('span', { 'class': S('sep'), 'aria-hidden': 'true' }),
        h('button', { 'class': S('world'), type: 'button', 'aria-haspopup': 'menu', 'aria-expanded': 'false', 'aria-label': 'Current World: ' + (p.world || 'none') + '. Switch World', 'data-tip': 'Switch World' },
          Avatar({ kind: 'world', name: p.world, src: p.worldMark, size: 'sm', hue: p.worldHue, decorative: true }), icon('unfold_more'))),
      h('div', { 'class': S('island'), role: 'toolbar', 'aria-label': 'Account and activity' },
        h('button', { 'class': S('search'), type: 'button', 'aria-label': 'Search', 'data-tip': 'Search' }, icon('search'), h('span', { 'class': 'm-chrome__label' }, 'Search'), h('span', { 'class': S('kbd') }, '/')),
        IconButton({ icon: 'inbox', label: p.inbox ? 'Inbox, ' + p.inbox + ' items need you' : 'Inbox', count: p.inbox }),
        h('button', { 'class': ['m-icon-button', 'm-icon-button--variant_standard'], type: 'button', 'aria-haspopup': 'menu', 'aria-label': 'Account @' + (p.account || ''), 'data-tip': '@' + (p.account || '') },
          Avatar({ kind: 'account', name: p.account, src: p.accountAvatar, size: 'sm', decorative: true }))));
  }
  /* Composer: speaker, destination and audience above the editor; tools, the sync state (an
     icon with its words in the tooltip and accessible name) and Post below. */
  var SYNC = {
    saved: ['cloud_done', 'Draft saved on this device', null],
    syncing: ['sync', 'Saving draft', null],
    synced: ['cloud_done', 'Draft saved to your account', null],
    local: ['cloud_off', 'Offline. Draft saved on this device', 'caution'],
    failed: ['error', 'Draft not saved', 'danger']
  };
  function Composer(p) {
    p = p || {};
    var S = function (part) { return 'm-composer__' + part; };
    var ta = h('textarea', { 'class': S('editor'), 'aria-label': 'Post as ' + (p.speaker || 'yourself'), placeholder: p.placeholder || ('Write as ' + (p.speaker || 'yourself') + '…') });
    if (p.text) ta.value = p.text;
    var sync = SYNC[p.sync || 'saved'] || SYNC.saved;
    var root = h('section', { 'class': [S('root'), 'm-marked'], 'aria-label': 'Reply' },
      h('div', { 'class': S('head') },
        h('button', { 'class': S('speaker'), type: 'button', 'aria-haspopup': 'listbox', 'aria-label': 'Speaker: ' + p.speaker + '. Change speaker', 'data-tip': 'Speaker' },
          Avatar({ kind: p.lane === 'ooc' ? 'account' : 'character', name: p.speaker, src: p.portrait, size: 'sm', hue: p.speakerHue, decorative: true }),
          h('span', { 'class': S('speakerName') }, p.speaker), icon('expand_more')),
        h('span', { 'class': S('destination') }, icon('forum'), h('strong', null, p.scene), Chip({ kind: 'fact', label: p.lane === 'ooc' ? 'Out of character' : 'In character' })),
        h('span', { 'class': S('audience') },
          Button({ variant: 'text', size: 'sm', icon: p.audienceIcon || 'group', label: p.audience || 'Scene members', trailingIcon: 'expand_more' }),
          IconButton({ icon: 'open_in_full', label: 'Expand editor', size: 'sm', onClick: function (e) { toggleExpand(root, e.currentTarget); } }))),
      ta,
      h('div', { 'class': S('foot') },
        h('div', { 'class': S('tools'), role: 'toolbar', 'aria-label': 'Formatting' },
          IconButton({ icon: 'format_bold', label: 'Bold', size: 'sm' }),
          IconButton({ icon: 'format_italic', label: 'Italic', size: 'sm' }),
          IconButton({ icon: 'format_quote', label: 'Quote a post', size: 'sm', optional: true }),
          IconButton({ icon: 'casino', label: 'Roll dice', size: 'sm' }),
          IconButton({ icon: 'image', label: 'Add image', size: 'sm', optional: true })),
        h('span', { 'class': S('submit') },
          h('span', { 'class': S('status'), role: 'status', 'aria-label': sync[1], 'data-tip': sync[1], 'data-tone': sync[2], tabindex: '0' }, icon(sync[0])),
          Button({ variant: 'filled', icon: 'send', label: p.submit || 'Post' }))));
    return root;
  }
  function toggleExpand(root, btn) {
    var open = !root.hasAttribute('data-expanded');
    transition(function () {
      if (open) root.setAttribute('data-expanded', ''); else root.removeAttribute('data-expanded');
      if (btn) {
        btn.setAttribute('aria-label', open ? 'Close full editor' : 'Expand editor');
        var g = btn.querySelector('.m-icon'); if (g) g.textContent = open ? 'close_fullscreen' : 'open_in_full';
      }
    }, { types: [open ? 'expand' : 'collapse'] });
  }

  /* View transitions: the one entry point for a state change that moves things. Runs `update`
     inside document.startViewTransition when the browser has it and the reader allows motion,
     otherwise straight away. `opts.types` name the change ('forward', 'back', 'expand', 'post'…);
     they go to the transition and to <html data-vt>, for browsers without transition types.
     `update` may return a promise; the new state is captured once it settles. Returns the
     ViewTransition, or a stand-in with the same promises when nothing animates. */
  function stillMotion() {
    if (typeof document === 'undefined' || !document.startViewTransition) return true;
    if (window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches) return true;
    return !!document.querySelector('[data-reduce-motion]');
  }
  function transition(update, opts) {
    opts = opts || {};
    var types = [].concat(opts.types || []);
    if (stillMotion()) {
      var out, err = null;
      try { out = update(); } catch (e) { err = e; }
      var done = err ? Promise.reject(err) : Promise.resolve(out);
      done.catch(function () {});
      return { skipped: true, finished: done, ready: done, updateCallbackDone: done, types: types, skipTransition: function () {} };
    }
    /* data-vt is present for the whole transition, empty when there are no types: names that
       only matter while things move (.m-vt, the chrome, a World's mark) switch on with it */
    var html = document.documentElement, mark = types.join(' ');
    html.setAttribute('data-vt', mark);
    var vt;
    try { vt = document.startViewTransition({ update: update, types: types }); }
    catch (e) { vt = document.startViewTransition(update); }
    var clear = function () { if (html.getAttribute('data-vt') === mark) html.removeAttribute('data-vt'); };
    vt.finished.then(clear, clear);
    return vt;
  }
  /* Full page loads: a page that draws itself after load (a client-rendered route) would be
     revealed half-drawn, so its transition is skipped unless the page is already there. */
  if (typeof window !== 'undefined') {
    window.addEventListener('pagereveal', function (e) {
      if (!e.viewTransition) return;
      if (stillMotion() || !document.querySelector('.m-world__root, .m-page__root, [data-vt-ready]')) e.viewTransition.skipTransition();
    });
  }
  function Segmented(p) {
    return h('div', { 'class': 'm-segmented__root', role: 'radiogroup', 'aria-label': p.label },
      p.items.map(function (it) {
        return h('button', { 'class': 'm-segmented__item', type: 'button', role: 'radio', 'aria-checked': String(it.value === p.value), 'data-value': it.value }, it.icon && icon(it.icon), it.label);
      }));
  }
  /* Lane filter: which posts a Scene shows, in character, out of character or both. Two toggles
     in one split button; one of them is always on. LANE_POLICY.fromBoth decides a click while
     both are on: 'unselect' hides the clicked lane, 'solo' keeps only the clicked lane. */
  var LANE_POLICY = { fromBoth: 'unselect' };
  function nextLane(cur, clicked, policy) {
    var other = clicked === 'ic' ? 'ooc' : 'ic';
    if (cur === 'both') return (policy || LANE_POLICY.fromBoth) === 'solo' ? clicked : other;
    return cur === clicked ? other : 'both';
  }
  function LaneFilter(p) {
    p = p || {};
    var L = { group: 'Show posts', ic: 'In character', ooc: 'Out of character',
      both: 'Showing both lanes', icOnly: 'Showing in-character posts only', oocOnly: 'Showing out-of-character posts only' };
    Object.keys(p.labels || {}).forEach(function (k) { L[k] = p.labels[k]; });
    var btn = function (lane, ic) {
      return h('button', { 'class': 'm-lanes__item', type: 'button', 'data-lane': lane, 'aria-pressed': 'false', 'aria-label': L[lane], 'data-tip': L[lane] }, icon(ic));
    };
    var bIc = btn('ic', 'theater_comedy'), bOoc = btn('ooc', 'chat');
    var status = h('span', { 'class': 'm-visually-hidden', 'aria-live': 'polite' });
    var root = h('div', { 'class': ['m-lanes__root', p['class']], role: 'group', 'aria-label': L.group, 'data-m': 'lanes' }, bIc, bOoc, status);
    var state;
    function set(v, say) {
      state = v === 'ic' || v === 'ooc' ? v : 'both';
      root.value = state; root.setAttribute('data-value', state);
      bIc.setAttribute('aria-pressed', String(state !== 'ooc'));
      bOoc.setAttribute('aria-pressed', String(state !== 'ic'));
      if (say) { status.textContent = ''; setTimeout(function () { status.textContent = state === 'ic' ? L.icOnly : state === 'ooc' ? L.oocOnly : L.both; }, 30); }
    }
    [bIc, bOoc].forEach(function (b) {
      b.addEventListener('click', function () {
        set(nextLane(state, b.getAttribute('data-lane'), p.policy), true);
        root.dispatchEvent(new CustomEvent('change', { bubbles: true, detail: { value: state } }));
      });
    });
    root.setValue = function (v) { set(v, false); };
    set(p.value || 'both', false);
    return root;
  }
  function Switch(p) {
    return h('button', { 'class': 'm-switch__root', type: 'button', role: 'switch', 'aria-checked': String(!!p.checked), 'aria-label': p.label, id: p.id }, h('span', { 'class': 'm-switch__thumb' }));
  }

  /* Progressive behaviours: tooltips from data-tip, segmented groups, menus. */
  function enhance(root) {
    root = root || document;
    root.querySelectorAll('.m-segmented__root').forEach(function (g) {
      if (g.hasAttribute('data-m-ready')) return;
      g.setAttribute('data-m-ready', '');
      var items = Array.prototype.slice.call(g.querySelectorAll('[role="radio"]'));
      items.forEach(function (it, i) {
        it.tabIndex = it.getAttribute('aria-checked') === 'true' ? 0 : -1;
        it.addEventListener('click', function () { select(i); });
        it.addEventListener('keydown', function (e) {
          var d = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
          if (d) { e.preventDefault(); select((i + d + items.length) % items.length, true); }
        });
      });
      function select(i, focus) {
        items.forEach(function (x, j) { x.setAttribute('aria-checked', String(i === j)); x.tabIndex = i === j ? 0 : -1; });
        if (focus) items[i].focus();
        g.dispatchEvent(new CustomEvent('change', { detail: { value: items[i].dataset.value } }));
      }
    });
    root.querySelectorAll('.m-switch__root').forEach(function (s) {
      if (s.hasAttribute('data-m-ready')) return;
      s.setAttribute('data-m-ready', '');
      s.addEventListener('click', function () {
        var on = s.getAttribute('aria-checked') !== 'true';
        s.setAttribute('aria-checked', String(on));
        s.dispatchEvent(new CustomEvent('change', { bubbles: true, detail: { checked: on } }));
      });
    });
    var tip = null;
    var show = function (e) {
      var t = e.target.closest && e.target.closest('[data-tip]');
      if (!t || !root.contains(t)) return;
      if (!tip) { tip = h('div', { 'class': 'm-tooltip', role: 'tooltip', id: 'm-tip' }); document.body.appendChild(tip); }
      tip.textContent = t.getAttribute('data-tip');
      var r = t.getBoundingClientRect();
      tip.style.left = (r.left + r.width / 2 + window.scrollX) + 'px';
      tip.style.top = (r.bottom + 8 + window.scrollY) + 'px';
      tip.setAttribute('data-open', '');
      t.setAttribute('aria-describedby', 'm-tip');
    };
    var hide = function (e) {
      var t = e.target.closest && e.target.closest('[data-tip]');
      if (tip) tip.removeAttribute('data-open');
      if (t) t.removeAttribute('aria-describedby');
    };
    root.addEventListener('pointerover', show); root.addEventListener('pointerout', hide);
    root.addEventListener('focusin', function (e) { if (e.target.matches && e.target.matches(':focus-visible')) show(e); }); root.addEventListener('focusout', hide);
    document.addEventListener('keydown', function (e) { if (e.key === 'Escape' && tip) tip.removeAttribute('data-open'); });
  }

  var api = {
    version: '0.1.0',
    color: {
      fromHex: fromHex, toHex: toHex, contrast: contrast, gamut: gamut, mix: mix,
      oklabToLinear: oklabToLinear, toGamma: toGam, luminance: lum,
      inGamut: function (c) { return inGamut(lchToLinear(c)); },
      maxChroma: function (l, h) { return gamut({ l: l, c: 0.4, h: h }).c; }
    },
    buildPalette: buildPalette, audit: audit, roleChecks: ROLE_CHECKS,
    compileWorld: compileWorld, applyWorld: applyWorld, mountWorld: mountWorld, cssText: cssText,
    spring: spring, springs: SPRINGS, motionCharacters: CHARACTERS,
    ornaments: ORNAMENTS, validateOrnament: validateOrnament, ornamentSvg: ornamentSvg,
    faces: FACES, readingFaces: READING,
    presets: PRESETS, defaultRecord: DEFAULT_RECORD,
    toStandardMode: toStandardMode, toDTCG: toDTCG, treatments: TREATMENTS, treatmentsOf: treatmentsOf,
    h: h, icon: icon, slot: slot, Button: Button, IconButton: IconButton, Chip: Chip, Flag: Flag, Avatar: Avatar, Divider: Divider, Meta: Meta,
    Post: Post, DiceRoll: DiceRoll, SceneEvent: SceneEvent, SceneRow: SceneRow, InboxItem: InboxItem, CharacterCard: CharacterCard, Tabs: Tabs,
    Masthead: Masthead, Chrome: Chrome, Composer: Composer, Segmented: Segmented, Switch: Switch, enhance: enhance,
    LaneFilter: LaneFilter, nextLane: nextLane, lanePolicy: LANE_POLICY,
    currentMode: currentMode, currentContrast: currentContrast, transition: transition
  };
  if (typeof window !== 'undefined') { window.Motet = window.Motet || {}; Object.keys(api).forEach(function (k) { window.Motet[k] = api[k]; }); }
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();

/* Motet kit: form, disclosure, navigation, overlay and feedback components.
   Each factory returns markup with the recipe classes and wires its behaviour. The same
   behaviour attaches to server-rendered markup through Motet.enhance(), keyed by data-m. */
(function () {
  'use strict';
  if (typeof window === 'undefined' || !window.Motet) return;
  var M = window.Motet;
  var h = M.h, icon = M.icon, IconButton = M.IconButton, Button = M.Button, Avatar = M.Avatar;

  /* ---------------------------------------------------------------- utilities */
  var seq = 0;
  function uid(p) { seq += 1; return (p || 'm') + '-' + seq.toString(36); }
  function lang() {
    var l = String(document.documentElement.lang || navigator.language || 'en').toLowerCase().split('@')[0];
    try { Intl.getCanonicalLocales(l); return l; } catch (e) { return 'en'; }
  }
  function isPl() { return lang().slice(0, 2) === 'pl'; }
  function fold(ch) { return ch.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ł/g, 'l'); }
  function norm(s) { return String(s || '').split('').map(fold).join(''); }
  /* wrap the first match of `q` in <mark>, comparing without case or diacritics */
  function highlight(label, q) {
    if (!q) return [label];
    var folded = '', map = [];
    for (var i = 0; i < label.length; i++) { var f = fold(label[i]); for (var j = 0; j < f.length; j++) { folded += f[j]; map.push(i); } }
    var at = folded.indexOf(norm(q));
    if (at < 0) return [label];
    var s = map[at], e = map[at + norm(q).length - 1] + 1;
    return [label.slice(0, s), h('mark', null, label.slice(s, e)), label.slice(e)];
  }
  function emit(el, name, detail) { el.dispatchEvent(new CustomEvent(name, { bubbles: true, detail: detail })); }
  function reduced() { return window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches; }
  var live;
  function announce(text) {
    if (!live) { live = h('div', { 'class': 'm-visually-hidden', 'aria-live': 'polite', role: 'status' }); document.body.appendChild(live); }
    live.textContent = ''; setTimeout(function () { live.textContent = text; }, 30);
  }
  function fmtBytes(n) {
    var nf = new Intl.NumberFormat(lang(), { maximumFractionDigits: n >= 1048576 ? 1 : 0 });
    return n >= 1048576 ? nf.format(n / 1048576) + ' MB' : nf.format(Math.max(1, Math.round(n / 1024))) + ' KB';
  }

  /* A labelled field: label above, control, optional help or error below, all linked. */
  function Field(p) {
    var id = p.id || (p.control && p.control.id) || uid('f');
    var helpId = id + '-help';
    var ctl = p.control;
    var focusable = ctl && (ctl.matches('input, textarea, select, button') ? ctl : ctl.querySelector('input, textarea, select, button[role=combobox]'));
    if (focusable && !focusable.id) focusable.id = id;
    var help = p.error || p.help;
    if (focusable && help) focusable.setAttribute('aria-describedby', helpId);
    if (focusable && p.error) focusable.setAttribute('aria-invalid', 'true');
    return h('div', { 'class': ['m-field__root', p['class']] },
      p.label ? h('label', { 'class': 'm-field__label', 'for': focusable ? focusable.id : null, id: id + '-label' }, p.label) : null,
      ctl,
      help ? h('div', { 'class': 'm-field__help', id: helpId, 'data-tone': p.error ? 'danger' : null }, p.error ? icon('error') : null, h('span', null, help)) : null);
  }

  /* ---------------------------------------------------------------- popups */
  var hasPopover = typeof HTMLElement !== 'undefined' && Object.prototype.hasOwnProperty.call(HTMLElement.prototype, 'popover');
  function place(pop, anchor, o) {
    if (pop.hasAttribute('data-inline')) return;
    var r = anchor.getBoundingClientRect(), vw = document.documentElement.clientWidth, vh = window.innerHeight, gap = o.offset == null ? 6 : o.offset;
    pop.style.position = 'fixed';
    pop.style.maxHeight = '';
    if (o.matchWidth) pop.style.minWidth = Math.round(r.width) + 'px';
    var pw = pop.offsetWidth, ph = pop.offsetHeight;
    var below = vh - r.bottom - gap - 8, above = r.top - gap - 8;
    var up = ph > below && above > below;
    if (ph > (up ? above : below)) { pop.style.maxHeight = Math.max(160, up ? above : below) + 'px'; ph = pop.offsetHeight; }
    var top = up ? r.top - gap - ph : r.bottom + gap;
    var left = o.align === 'end' ? r.right - pw : o.align === 'center' ? r.left + (r.width - pw) / 2 : r.left;
    if (document.documentElement.dir === 'rtl' && !o.align) left = r.right - pw;
    pop.style.left = Math.round(Math.max(8, Math.min(left, vw - pw - 8))) + 'px';
    pop.style.top = Math.round(Math.max(8, top)) + 'px';
    pop.setAttribute('data-side', up ? 'top' : 'bottom');
  }
  function openPopup(pop, anchor, o) {
    o = o || {};
    if (pop.hasAttribute('data-inline')) { pop.hidden = false; return; }
    if (hasPopover) {
      if (!pop.hasAttribute('popover')) pop.setAttribute('popover', 'manual');
      if (!pop.matches(':popover-open')) pop.showPopover();
    } else { pop.hidden = false; pop.style.zIndex = '60'; }
    place(pop, anchor, o);
    var re = function () { place(pop, anchor, o); };
    window.addEventListener('scroll', re, true); window.addEventListener('resize', re);
    if (pop._unplace) pop._unplace();
    pop._unplace = function () { window.removeEventListener('scroll', re, true); window.removeEventListener('resize', re); pop._unplace = null; };
  }
  function closePopup(pop) {
    if (pop._unplace) pop._unplace();
    if (pop.hasAttribute('data-inline')) return;
    if (hasPopover && pop.hasAttribute('popover')) { if (pop.matches(':popover-open')) pop.hidePopover(); }
    else pop.hidden = true;
  }
  function isOpen(pop) { return pop.hasAttribute('data-inline') ? !pop.hidden : hasPopover && pop.hasAttribute('popover') ? pop.matches(':popover-open') : !pop.hidden; }
  /* close when the pointer goes down outside any of `els` */
  function outside(els, cb) {
    var fn = function (e) { if (!els.some(function (el) { return el && el.contains(e.target); })) cb(e); };
    document.addEventListener('pointerdown', fn, true);
    return function () { document.removeEventListener('pointerdown', fn, true); };
  }
  function newPopup(cls, attrs) {
    var pop = h('div', Object.assign({ 'class': cls }, attrs || {}));
    if (hasPopover) pop.setAttribute('popover', 'manual'); else pop.hidden = true;
    return pop;
  }

  /* ---------------------------------------------------------------- listbox (Select, Combobox, tag suggestions) */
  function optionNode(o, id, q) {
    var lead = o.avatar ? Avatar(Object.assign({ size: 'sm', decorative: true }, o.avatar)) : o.icon ? icon(o.icon) : null;
    return h('div', { 'class': 'm-listbox__option', role: 'option', id: id, 'aria-selected': 'false', 'aria-disabled': o.disabled ? 'true' : null, 'data-value': o.value },
      lead,
      h('span', { 'class': 'm-listbox__optionText' },
        h('span', { 'class': 'm-listbox__optionLabel' }, highlight(o.label, q)),
        o.description ? h('span', { 'class': 'm-listbox__optionDescription' }, o.description) : null),
      icon('check', { 'class': 'm-listbox__check' }));
  }
  function Listbox(o) {
    var root = newPopup('m-listbox__root', { role: 'listbox', id: o.id || uid('lb'), 'aria-label': o.label || null, tabindex: '-1' });
    if (o.inline) { root.setAttribute('data-inline', ''); root.removeAttribute('popover'); root.hidden = true; }
    var api = { el: root, items: [], active: -1, selected: o.multiple ? [] : null };
    api.render = function (options, q) {
      root.innerHTML = '';
      api.items = [];
      var groups = {}, order = [];
      options.forEach(function (op) { var g = op.group || ''; if (!groups[g]) { groups[g] = []; order.push(g); } groups[g].push(op); });
      order.forEach(function (g) {
        var box = root;
        if (g) { var gid = uid('g'); box = h('div', { 'class': 'm-listbox__group', role: 'group', 'aria-labelledby': gid }, h('div', { 'class': 'm-listbox__groupLabel', id: gid, role: 'presentation' }, g)); root.appendChild(box); }
        groups[g].forEach(function (op) {
          var el = optionNode(op, root.id + '-' + api.items.length, q);
          el.addEventListener('pointerdown', function (e) { e.preventDefault(); });
          el.addEventListener('click', function () { if (!op.disabled) o.onPick(op); });
          el.addEventListener('pointermove', function () { var i = api.items.indexOf(it); if (i !== api.active) api.setActive(i, true); });
          var it = { op: op, el: el };
          api.items.push(it); box.appendChild(el);
        });
      });
      if (!options.length) root.appendChild(h('div', { 'class': 'm-listbox__empty', role: 'presentation' }, o.empty || (isPl() ? 'Brak wyników' : 'No matches')));
      api.markSelected();
      api.active = -1;
    };
    api.markSelected = function () {
      api.items.forEach(function (it) {
        var sel = o.multiple ? (api.selected || []).indexOf(it.op.value) >= 0 : api.selected === it.op.value;
        it.el.setAttribute('aria-selected', String(!!sel));
      });
    };
    api.setActive = function (i, fromPointer) {
      if (api.active >= 0 && api.items[api.active]) api.items[api.active].el.removeAttribute('data-active');
      api.active = i;
      var it = api.items[i];
      if (it) { it.el.setAttribute('data-active', ''); if (!fromPointer) it.el.scrollIntoView({ block: 'nearest' }); }
      if (o.owner) { if (it) o.owner.setAttribute('aria-activedescendant', it.el.id); else o.owner.removeAttribute('aria-activedescendant'); }
    };
    api.move = function (d) {
      var n = api.items.length; if (!n) return;
      var i = api.active;
      for (var k = 0; k < n; k++) { i = d === 'first' ? (k === 0 ? 0 : i + 1) : d === 'last' ? (k === 0 ? n - 1 : i - 1) : (i + d + n) % n; if (i < 0) i = 0; if (!api.items[i].op.disabled) break; }
      api.setActive(i);
    };
    api.indexOf = function (value) { for (var i = 0; i < api.items.length; i++) if (api.items[i].op.value === value) return i; return -1; };
    api.typeahead = (function () {
      var buf = '', t;
      return function (ch) {
        clearTimeout(t); t = setTimeout(function () { buf = ''; }, 600); buf += fold(ch);
        var start = Math.max(api.active, 0);
        for (var k = 0; k < api.items.length; k++) {
          var i = (start + (buf.length === 1 ? 1 : 0) + k) % api.items.length;
          if (norm(api.items[i].op.label).indexOf(buf) === 0 && !api.items[i].op.disabled) return i;
        }
        return -1;
      };
    })();
    return api;
  }

  /* ---------------------------------------------------------------- checkbox, radio, fieldset */
  function Checkbox(p) {
    var input = h('input', { 'class': 'm-checkbox__control', type: 'checkbox', name: p.name, value: p.value, checked: p.checked, disabled: p.disabled, id: p.id || uid('cb') });
    if (p.indeterminate) input.indeterminate = true;
    var descId = p.description ? input.id + '-d' : null;
    if (descId) input.setAttribute('aria-describedby', descId);
    if (p.onChange) input.addEventListener('change', function () { p.onChange(input.checked); });
    return h('label', { 'class': 'm-checkbox__root', 'for': input.id },
      input,
      h('span', { 'class': 'm-checkbox__text' },
        h('span', { 'class': 'm-checkbox__label' }, p.label),
        p.description ? h('span', { 'class': 'm-checkbox__description', id: descId }, p.description) : null));
  }
  function Fieldset(p) {
    var helpId = p.help ? uid('fh') : null;
    return h('fieldset', { 'class': ['m-fieldset__root', p['class']], 'aria-describedby': helpId, role: p.role },
      h('legend', { 'class': ['m-fieldset__legend', p.hideLegend && 'm-visually-hidden'] }, p.legend),
      p.help ? h('div', { 'class': 'm-fieldset__help', id: helpId }, p.help) : null,
      h('div', { 'class': 'm-fieldset__content' }, p.children));
  }
  /* A parent checkbox that reflects and sets a group of children (mixed when some are on). */
  function CheckboxGroup(p) {
    var kids = p.items.map(function (it) { return Checkbox(Object.assign({ name: p.name }, it)); });
    var all = p.allLabel ? Checkbox({ label: p.allLabel }) : null;
    var inputs = kids.map(function (k) { return k.querySelector('input'); });
    function sync() {
      if (!all) return;
      var on = inputs.filter(function (i) { return i.checked; }).length;
      var a = all.querySelector('input'); a.checked = on === inputs.length; a.indeterminate = on > 0 && on < inputs.length;
    }
    inputs.forEach(function (i) { i.addEventListener('change', sync); });
    if (all) all.querySelector('input').addEventListener('change', function (e) { inputs.forEach(function (i) { if (!i.disabled) i.checked = e.target.checked; }); sync(); });
    sync();
    return Fieldset({ legend: p.legend, help: p.help, children: [all, all && h('div', { style: { 'padding-inline-start': '32px', display: 'grid' } }, kids), !all && kids] });
  }
  function RadioGroup(p) {
    var name = p.name || uid('rg');
    var cards = p.variant === 'cards';
    var items = p.items.map(function (it) {
      var input = h('input', { 'class': 'm-radio__control', type: 'radio', name: name, value: it.value, checked: it.value === p.value, disabled: it.disabled, id: uid('r') });
      var descId = it.description ? input.id + '-d' : null;
      if (descId) input.setAttribute('aria-describedby', descId);
      return h('label', { 'class': ['m-radio__item', cards && 'm-radio__item--variant_cards'], 'for': input.id },
        input,
        h('span', { 'class': ['m-radio__text', cards && 'm-radio__text--variant_cards'] },
          h('span', { 'class': ['m-radio__label', cards && 'm-radio__label--variant_cards'] }, it.label),
          it.description ? h('span', { 'class': 'm-radio__description', id: descId }, it.description) : null),
        it.icon ? icon(it.icon, { 'class': ['m-radio__icon', cards && 'm-radio__icon--variant_cards'].filter(Boolean).join(' ') }) : null);
    });
    var root = h('div', { 'class': ['m-radio__root', cards && 'm-radio__root--variant_cards'] }, items);
    root.querySelectorAll('.m-radio__control').forEach(function (c) { if (cards) c.classList.add('m-radio__control--variant_cards'); });
    if (p.onChange) root.addEventListener('change', function (e) { p.onChange(e.target.value); });
    return Fieldset({ legend: p.legend, help: p.help, hideLegend: p.hideLegend, children: root });
  }

  /* ---------------------------------------------------------------- text entry */
  function InputBox(p, control) {
    return h('div', { 'class': 'm-input__root', 'data-m': p.m || null },
      p.leading ? h('span', { 'class': 'm-input__addon', 'data-side': 'start', 'aria-hidden': p.leadingText ? null : 'true' }, typeof p.leading === 'string' && /^[a-z_]+$/.test(p.leading) ? icon(p.leading) : p.leading) : null,
      control,
      p.trailing ? h('span', { 'class': 'm-input__addon', 'data-side': 'end', title: p.trailingTitle || null }, p.trailing) : null,
      (p.actions || []).map(function (a) { return h('span', { 'class': 'm-input__action' }, a); }));
  }
  function TextInput(p) {
    var input = h('input', { 'class': 'm-input__control', type: p.type || 'text', name: p.name, value: p.value, placeholder: p.placeholder, autocomplete: p.autocomplete, inputmode: p.inputmode, disabled: p.disabled, required: p.required });
    return Field({ label: p.label, help: p.help, error: p.error, control: InputBox(p, input) });
  }
  function Textarea(p) {
    var ta = h('textarea', { 'class': ['m-textarea__control', 'm-textarea__control--face_' + (p.face || 'reading')], name: p.name, placeholder: p.placeholder, rows: p.rows || 3, maxlength: null, id: uid('ta'), style: { '--rows': String(p.rows || 3), '--max-rows': String(p.maxRows || 12) } });
    if (p.value) ta.value = p.value;
    var counter = p.maxLength ? h('span', { 'class': 'm-textarea__counter', 'aria-live': 'off' }) : null;
    var helpId = ta.id + '-help';
    var footer = (p.help || counter) ? h('div', { 'class': 'm-textarea__footer' }, p.help ? h('span', { id: helpId }, p.help) : null, counter) : null;
    if (p.help) ta.setAttribute('aria-describedby', helpId);
    var root = h('div', { 'class': 'm-textarea__root', 'data-m': 'textarea' },
      p.label ? h('label', { 'class': 'm-field__label', 'for': ta.id }, p.label) : null, ta, footer);
    if (p.maxLength) root.setAttribute('data-max', p.maxLength);
    wireTextarea(root);
    return root;
  }
  function wireTextarea(root) {
    var ta = root.querySelector('textarea'), counter = root.querySelector('.m-textarea__counter'), max = +root.getAttribute('data-max') || 0;
    var nf = new Intl.NumberFormat(lang());
    var sized = typeof CSS !== 'undefined' && CSS.supports && CSS.supports('field-sizing', 'content');
    function update() {
      if (!sized) { ta.style.height = 'auto'; ta.style.height = ta.scrollHeight + 2 + 'px'; }
      if (counter) {
        var n = ta.value.length;
        counter.textContent = nf.format(n) + ' / ' + nf.format(max);
        counter.setAttribute('data-state', n > max ? 'over' : n > max * 0.9 ? 'near' : 'ok');
        ta.setAttribute('aria-invalid', n > max ? 'true' : 'false');
        /* say it once, when it matters: entering the last tenth and going over */
        var st = counter.getAttribute('data-state');
        if (st !== root._st && st !== 'ok') announce(st === 'over' ? (isPl() ? 'Przekroczono limit znaków' : 'Over the character limit') : nf.format(max - n) + (isPl() ? ' znaków do limitu' : ' characters left'));
        root._st = st;
      }
    }
    ta.addEventListener('input', update); update();
  }
  function PasswordInput(p) {
    var input = h('input', { 'class': 'm-input__control', type: 'password', name: p.name, autocomplete: p.autocomplete || 'current-password', spellcheck: 'false', id: uid('pw') });
    var L = isPl() ? ['Pokaż hasło', 'Ukryj hasło', 'Caps Lock jest włączony'] : ['Show password', 'Hide password', 'Caps Lock is on'];
    var btn = IconButton({ icon: 'visibility', label: L[0], pressed: 'false' });
    var box = InputBox({ m: 'password', actions: [btn] }, input);
    var caps = h('div', { 'class': 'm-field__help', 'data-tone': 'caution', hidden: true, 'aria-live': 'polite' }, icon('keyboard_capslock'), h('span', null, L[2]));
    btn.addEventListener('click', function () {
      var show = input.type === 'password';
      input.type = show ? 'text' : 'password';
      btn.setAttribute('aria-pressed', String(show));
      btn.setAttribute('aria-label', show ? L[1] : L[0]); btn.setAttribute('data-tip', show ? L[1] : L[0]);
      btn.querySelector('.m-icon').textContent = show ? 'visibility_off' : 'visibility';
      input.focus();
    });
    var check = function (e) { if (e.getModifierState) caps.hidden = !e.getModifierState('CapsLock'); };
    input.addEventListener('keydown', check); input.addEventListener('keyup', check);
    input.addEventListener('blur', function () { caps.hidden = true; });
    var f = Field({ label: p.label, help: p.help, control: box });
    f.appendChild(caps);
    return f;
  }
  function NumberInput(p) {
    var min = p.min != null ? p.min : -Infinity, max = p.max != null ? p.max : Infinity, step = p.step || 1;
    var nf = new Intl.NumberFormat(lang(), { maximumFractionDigits: 4 });
    var input = h('input', { 'class': 'm-input__control', type: 'text', inputmode: step % 1 ? 'decimal' : 'numeric', role: 'spinbutton', name: p.name, autocomplete: 'off', id: uid('num'),
      'aria-valuemin': isFinite(min) ? min : null, 'aria-valuemax': isFinite(max) ? max : null, style: { 'text-align': 'center' } });
    var L = isPl() ? ['Zmniejsz', 'Zwiększ'] : ['Decrease', 'Increase'];
    var dec = IconButton({ icon: 'remove', label: L[0], tip: false }), inc = IconButton({ icon: 'add', label: L[1], tip: false });
    dec.tabIndex = -1; inc.tabIndex = -1;
    var box = h('div', { 'class': 'm-input__root', 'data-m': 'number' },
      h('span', { 'class': 'm-input__action' }, dec), input,
      p.unit ? h('span', { 'class': 'm-input__addon', 'data-side': 'end' }, p.unit) : null,
      h('span', { 'class': 'm-input__action' }, inc));
    var value = p.value != null ? p.value : (isFinite(min) ? min : 0);
    function parse(s) { var n = parseFloat(String(s).replace(/\s/g, '').replace(',', '.')); return isNaN(n) ? null : n; }
    function set(v, quiet) {
      if (v == null) v = value;
      v = Math.min(max, Math.max(min, Math.round(v / step) * step));
      value = +v.toFixed(6);
      input.value = nf.format(value);
      input.setAttribute('aria-valuenow', value);
      if (p.unit) input.setAttribute('aria-valuetext', nf.format(value) + ' ' + p.unit);
      dec.disabled = value <= min; inc.disabled = value >= max;
      if (!quiet) emit(box, 'change', { value: value });
    }
    dec.addEventListener('click', function () { set(value - step); });
    inc.addEventListener('click', function () { set(value + step); });
    [dec, inc].forEach(function (b) { b.addEventListener('pointerdown', function (e) { e.preventDefault(); input.focus(); }); });
    input.addEventListener('keydown', function (e) {
      var big = step * 10;
      var map = { ArrowUp: value + step, ArrowDown: value - step, PageUp: value + big, PageDown: value - big, Home: isFinite(min) ? min : null, End: isFinite(max) ? max : null };
      if (e.key in map && map[e.key] != null) { e.preventDefault(); set(map[e.key]); }
      if (e.key === 'Enter') set(parse(input.value));
    });
    input.addEventListener('blur', function () { set(parse(input.value)); });
    set(value, true);
    return Field({ label: p.label, help: p.help, control: box });
  }

  /* ---------------------------------------------------------------- select */
  function Select(p) {
    var labelId = uid('sl'), valueId = uid('sv');
    var trigger = h('button', { 'class': 'm-select__trigger', type: 'button', role: 'combobox', 'aria-haspopup': 'listbox', 'aria-expanded': 'false', 'aria-labelledby': labelId + ' ' + valueId, disabled: p.disabled },
      p.icon ? icon(p.icon) : null, h('span', { 'class': 'm-select__value', id: valueId }), icon('expand_more', { 'class': 'm-select__indicator' }));
    var lb = Listbox({ owner: trigger, label: p.label, inline: p.inline, onPick: function (op) { choose(op.value); close(true); } });
    trigger.setAttribute('aria-controls', lb.el.id);
    var root = h('div', { 'class': 'm-select__root', 'data-m': 'select' },
      p.label ? h('span', { 'class': 'm-field__label', id: labelId }, p.label) : null, trigger, lb.el,
      p.help ? h('div', { 'class': 'm-field__help' }, p.help) : null);
    if (p.name) root.appendChild(h('input', { type: 'hidden', name: p.name }));
    var options = p.options || [];
    lb.render(options);
    var unOut = null;
    function choose(v) {
      lb.selected = v; lb.markSelected();
      var op = options.filter(function (o) { return o.value === v; })[0];
      var val = trigger.querySelector('.m-select__value');
      val.textContent = op ? op.label : (p.placeholder || '');
      if (op) val.removeAttribute('data-placeholder'); else val.setAttribute('data-placeholder', '');
      var hid = root.querySelector('input[type=hidden]'); if (hid) hid.value = v || '';
      root.value = v;
      if (op && !quiet) emit(root, 'change', { value: v, option: op });
    }
    var quiet = false;
    function open(at) {
      if (isOpen(lb.el)) return;
      trigger.setAttribute('aria-expanded', 'true');
      openPopup(lb.el, trigger, { matchWidth: true });
      var i = lb.indexOf(lb.selected);
      lb.setActive(at === 'last' ? lb.items.length - 1 : at === 'first' || i < 0 ? 0 : i);
      unOut = outside([root, lb.el], function () { close(); });
    }
    function close(focus) {
      trigger.setAttribute('aria-expanded', 'false'); trigger.removeAttribute('aria-activedescendant');
      closePopup(lb.el); if (unOut) { unOut(); unOut = null; }
      if (focus) trigger.focus();
    }
    trigger.addEventListener('click', function () { if (isOpen(lb.el)) close(); else open(); });
    trigger.addEventListener('keydown', function (e) {
      var opened = isOpen(lb.el);
      if (!opened) {
        if (['ArrowDown', 'ArrowUp', 'Enter', ' '].indexOf(e.key) >= 0) { e.preventDefault(); open(e.key === 'ArrowUp' ? 'last' : null); }
        else if (e.key === 'Home' || e.key === 'End') { e.preventDefault(); open(e.key === 'Home' ? 'first' : 'last'); }
        else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) { var t = lb.typeahead(e.key); if (t >= 0) choose(lb.items[t].op.value); }
        return;
      }
      if (e.key === 'ArrowDown') { e.preventDefault(); lb.move(1); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); lb.move(-1); }
      else if (e.key === 'Home') { e.preventDefault(); lb.move('first'); }
      else if (e.key === 'End') { e.preventDefault(); lb.move('last'); }
      else if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); var it = lb.items[lb.active]; if (it && !it.op.disabled) choose(it.op.value); close(true); }
      else if (e.key === 'Escape') { e.preventDefault(); close(true); }
      else if (e.key === 'Tab') { var it2 = lb.items[lb.active]; if (it2 && !it2.op.disabled) choose(it2.op.value); close(); }
      else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) { var j = lb.typeahead(e.key); if (j >= 0) lb.setActive(j); }
    });
    choose(p.value);
    root.open = open; root.close = close;
    /* set the value from outside without announcing a change */
    root.setValue = function (v) { quiet = true; choose(v); quiet = false; };
    return root;
  }

  /* ---------------------------------------------------------------- combobox */
  function Combobox(p) {
    var input = h('input', { 'class': 'm-input__control', type: 'text', role: 'combobox', 'aria-autocomplete': 'list', 'aria-expanded': 'false', autocomplete: 'off', placeholder: p.placeholder, id: uid('cb'), spellcheck: 'false' });
    var L = isPl() ? ['Pokaż opcje', 'Wyczyść'] : ['Show options', 'Clear'];
    var toggle = IconButton({ icon: 'expand_more', label: L[0], tip: false }); toggle.tabIndex = -1;
    var clear = IconButton({ icon: 'close', label: L[1], tip: false }); clear.tabIndex = -1; clear.hidden = true;
    var lb = Listbox({ owner: input, label: p.label, inline: p.inline, empty: p.empty, onPick: function (op) { pick(op); close(); input.focus(); } });
    input.setAttribute('aria-controls', lb.el.id);
    var box = InputBox({ leading: p.leading, actions: [clear, toggle] }, input);
    var root = Field({ label: p.label, help: p.help, control: box });
    root.setAttribute('data-m', 'combobox'); root.style.position = 'relative';
    root.appendChild(lb.el);
    var options = p.options || [], current = null, unOut = null;
    function filter(q) {
      var nq = norm(q);
      var list = nq ? options.filter(function (o) { return norm(o.label).indexOf(nq) >= 0 || (o.keywords && norm(o.keywords).indexOf(nq) >= 0); }) : options;
      lb.render(list, q);
      if (q && lb.items.length) lb.setActive(0);
    }
    function pick(op) {
      current = op; lb.selected = op ? op.value : null; lb.markSelected();
      input.value = op ? op.label : '';
      clear.hidden = !op;
      root.value = op ? op.value : null;
      emit(root, 'change', { value: root.value, option: op });
    }
    function open() {
      if (isOpen(lb.el)) return;
      input.setAttribute('aria-expanded', 'true'); toggle.setAttribute('aria-expanded', 'true');
      openPopup(lb.el, box, { matchWidth: true });
      unOut = outside([box, lb.el], function () { close(true); });
    }
    function close(revert) {
      input.setAttribute('aria-expanded', 'false'); toggle.setAttribute('aria-expanded', 'false'); input.removeAttribute('aria-activedescendant');
      closePopup(lb.el); if (unOut) { unOut(); unOut = null; }
      if (revert && !p.allowCustom) input.value = current ? current.label : '';
    }
    input.addEventListener('input', function () { filter(input.value); clear.hidden = !input.value; open(); });
    input.addEventListener('keydown', function (e) {
      var opened = isOpen(lb.el);
      if (e.key === 'ArrowDown') { e.preventDefault(); if (!opened) { filter(e.altKey ? '' : input.value === (current && current.label) ? '' : input.value); open(); if (e.altKey) return; } lb.move(1); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); if (!opened) { filter(''); open(); } lb.move(-1); }
      else if (e.key === 'Enter') { if (opened && lb.items[lb.active]) { e.preventDefault(); pick(lb.items[lb.active].op); close(); } }
      else if (e.key === 'Escape') { if (opened) { e.preventDefault(); close(true); } else if (input.value) { e.preventDefault(); pick(null); } }
      else if (e.key === 'Tab' && opened) close(true);
    });
    toggle.addEventListener('pointerdown', function (e) { e.preventDefault(); });
    toggle.addEventListener('click', function () { if (isOpen(lb.el)) close(true); else { filter(''); open(); input.focus(); var i = lb.indexOf(lb.selected); if (i >= 0) lb.setActive(i); } });
    clear.addEventListener('pointerdown', function (e) { e.preventDefault(); });
    clear.addEventListener('click', function () { pick(null); filter(''); input.focus(); });
    filter('');
    if (p.value) pick(options.filter(function (o) { return o.value === p.value; })[0] || null);
    root.open = function (q) { if (q != null) { input.value = q; filter(q); clear.hidden = !q; } open(); };
    return root;
  }

  /* ---------------------------------------------------------------- tags input */
  function TagsInput(p) {
    var input = h('input', { 'class': 'm-tags__control', type: 'text', placeholder: p.placeholder, autocomplete: 'off', id: uid('tg'), spellcheck: 'false' });
    var root = h('div', { 'class': 'm-tags__root', 'data-m': 'tags' }, input);
    var values = [], hi = -1, lb = null, unOut = null;
    var sugg = p.suggestions || [];
    var L = isPl() ? ['Usuń', 'Dodano', 'Usunięto'] : ['Remove', 'Added', 'Removed'];
    if (sugg.length) {
      input.setAttribute('role', 'combobox'); input.setAttribute('aria-autocomplete', 'list'); input.setAttribute('aria-expanded', 'false');
      lb = Listbox({ owner: input, label: p.label, inline: p.inline, empty: p.empty, onPick: function (op) { add(op.label); input.value = ''; refresh(); input.focus(); } });
      input.setAttribute('aria-controls', lb.el.id);
    }
    function tagEl(v) {
      var rm = h('button', { 'class': 'm-tags__remove', type: 'button', tabindex: '-1', 'aria-label': L[0] + ' ' + v }, icon('close'));
      rm.addEventListener('click', function () { remove(values.indexOf(v)); input.focus(); });
      return h('span', { 'class': 'm-tags__tag', 'data-value': v }, h('span', null, v), rm);
    }
    function render() {
      root.querySelectorAll('.m-tags__tag').forEach(function (t) { t.remove(); });
      values.forEach(function (v, i) { var t = tagEl(v); if (i === hi) t.setAttribute('data-highlighted', ''); root.insertBefore(t, input); });
      root.value = values.slice();
    }
    function add(raw) {
      String(raw).split(/[,\n]/).map(function (s) { return s.trim(); }).filter(Boolean).forEach(function (v) {
        if (values.some(function (x) { return norm(x) === norm(v); })) return;
        if (p.max && values.length >= p.max) return;
        values.push(v); announce(L[1] + ': ' + v);
      });
      hi = -1; render(); emit(root, 'change', { value: values.slice() });
    }
    function remove(i) { if (i < 0) return; var v = values.splice(i, 1)[0]; announce(L[2] + ': ' + v); hi = -1; render(); emit(root, 'change', { value: values.slice() }); }
    function refresh() {
      if (!lb) return;
      var nq = norm(input.value);
      var list = sugg.filter(function (s) { return values.every(function (v) { return norm(v) !== norm(s.label); }) && (!nq || norm(s.label).indexOf(nq) >= 0); });
      lb.render(list, input.value);
      if (nq && lb.items.length) lb.setActive(0);
      if (document.activeElement === input && (list.length || nq)) { input.setAttribute('aria-expanded', 'true'); openPopup(lb.el, root, { matchWidth: true }); if (!unOut) unOut = outside([root, lb.el], closeList); }
      else closeList();
    }
    function closeList() { if (!lb) return; input.setAttribute('aria-expanded', 'false'); input.removeAttribute('aria-activedescendant'); closePopup(lb.el); if (unOut) { unOut(); unOut = null; } }
    root.addEventListener('click', function (e) { if (e.target === root) input.focus(); });
    input.addEventListener('focus', refresh);
    input.addEventListener('input', function () { hi = -1; render(); refresh(); });
    input.addEventListener('paste', function (e) {
      var t = (e.clipboardData || window.clipboardData).getData('text');
      if (/[,\n]/.test(t)) { e.preventDefault(); add(t); refresh(); }
    });
    input.addEventListener('keydown', function (e) {
      var atStart = input.selectionStart === 0 && input.selectionEnd === 0;
      var opened = lb && isOpen(lb.el);
      if (opened && e.key === 'ArrowDown') { e.preventDefault(); lb.move(1); return; }
      if (opened && e.key === 'ArrowUp') { e.preventDefault(); lb.move(-1); return; }
      if (e.key === 'Enter' || e.key === ',') {
        if (opened && lb.items[lb.active]) { e.preventDefault(); add(lb.items[lb.active].op.label); input.value = ''; refresh(); return; }
        if (input.value.trim()) { e.preventDefault(); add(input.value); input.value = ''; refresh(); }
        else if (e.key === ',') e.preventDefault();
        return;
      }
      if (e.key === 'Escape') { if (opened) { e.preventDefault(); closeList(); } else if (hi >= 0) { hi = -1; render(); } return; }
      if ((e.key === 'Backspace' || e.key === 'Delete') && hi >= 0) { e.preventDefault(); remove(hi); return; }
      if (e.key === 'Backspace' && atStart && values.length) { e.preventDefault(); hi = values.length - 1; render(); announce(values[hi]); return; }
      if (e.key === 'ArrowLeft' && atStart && values.length) { e.preventDefault(); hi = hi < 0 ? values.length - 1 : Math.max(0, hi - 1); render(); announce(values[hi]); return; }
      if (e.key === 'ArrowRight' && hi >= 0) { e.preventDefault(); hi = hi + 1 >= values.length ? -1 : hi + 1; render(); if (hi >= 0) announce(values[hi]); return; }
      if (hi >= 0) { hi = -1; render(); }
    });
    input.addEventListener('blur', function () { if (hi >= 0) { hi = -1; render(); } if (p.addOnBlur !== false && input.value.trim()) { add(input.value); input.value = ''; } });
    (p.value || []).forEach(function (v) { values.push(v); });
    render();
    var f = Field({ label: p.label, help: p.help, control: root });
    f.style.position = 'relative';
    if (lb) { f.appendChild(lb.el); lb.render([], ''); }
    f.open = function (q) { input.value = q || ''; input.focus(); refresh(); };
    return f;
  }

  /* ---------------------------------------------------------------- inline select */
  /* A value chosen inside a sentence: "While I'm away, GMs may move [Lady Maren and Anwen] along."
     One value, or several (multiple); several are summarised in words: p.all when every option is
     picked, p.none when none is, otherwise a list ("Lady Maren and Anwen", "Lady Maren and 2 more").
     Options may carry `short` (the word used in the sentence), `icon`, `avatar` and `description`. */
  function InlineSelect(p) {
    var multiple = !!p.multiple, options = p.options || [];
    var ALL = '\u0000all', NONE = '\u0000none';
    var trigger = h('button', { 'class': 'm-inline-select__trigger', type: 'button', 'aria-haspopup': 'listbox', 'aria-expanded': 'false', disabled: p.disabled },
      h('span', { 'class': 'm-inline-select__value' }), icon('expand_more', { 'class': 'm-inline-select__indicator' }));
    var lb = Listbox({ owner: trigger, label: p.label, multiple: multiple, onPick: function (op) { pick(op); } });
    trigger.setAttribute('aria-controls', lb.el.id);
    var root = h('span', { 'class': 'm-inline-select__root', 'data-m': 'inline-select' }, trigger, lb.el);
    var selected = multiple ? (p.value || []).slice() : (p.value != null ? p.value : null);
    var quick = multiple ? [p.all ? { value: ALL, label: p.all, icon: 'done_all' } : null, p.none ? { value: NONE, label: p.none, icon: 'remove' } : null].filter(Boolean) : [];
    var list = quick.concat(options.map(function (o) { return quick.length && !o.group ? Object.assign({ group: p.groupLabel || (isPl() ? 'Wybrane' : 'Pick') }, o) : o; }));
    var unOut = null;
    function find(v) { for (var i = 0; i < options.length; i++) if (options[i].value === v) return options[i]; return null; }
    function word(o) { return o.short || o.label; }
    function summary() {
      if (!multiple) { var op = find(selected); return op ? word(op) : (p.placeholder || ''); }
      if (!selected.length) return p.none || p.placeholder || '';
      if (p.all && selected.length === options.length) return p.all;
      var names = selected.map(function (v) { var o = find(v); return o ? word(o) : String(v); });
      var max = p.maxNames || 2;
      if (names.length > max) { var rest = names.length - max + 1; names = names.slice(0, max - 1).concat([isPl() ? 'jeszcze ' + rest : rest + ' more']); }
      try { return new Intl.ListFormat(lang(), { style: 'long', type: 'conjunction' }).format(names); } catch (e) { return names.join(', '); }
    }
    function sync(quiet) {
      var text = summary();
      trigger.querySelector('.m-inline-select__value').textContent = text;
      if (p.label) trigger.setAttribute('aria-label', p.label + ': ' + text);
      lb.selected = multiple ? selected.concat(!selected.length ? [NONE] : selected.length === options.length ? [ALL] : []) : selected;
      lb.markSelected();
      root.value = multiple ? selected.slice() : selected;
      if (!quiet) emit(root, 'change', { value: root.value, text: text });
    }
    function pick(op) {
      if (op.disabled) return;
      if (!multiple) { selected = op.value; sync(); close(true); return; }
      if (op.value === ALL) selected = options.filter(function (o) { return !o.disabled; }).map(function (o) { return o.value; });
      else if (op.value === NONE) selected = [];
      else {
        var i = selected.indexOf(op.value);
        if (i >= 0) selected.splice(i, 1); else selected.push(op.value);
        selected.sort(function (a, b) { return options.indexOf(find(a)) - options.indexOf(find(b)); });
      }
      sync();
    }
    function open(at) {
      if (isOpen(lb.el)) return;
      trigger.setAttribute('aria-expanded', 'true');
      openPopup(lb.el, trigger, { align: p.align });
      var i = multiple ? 0 : lb.indexOf(selected);
      lb.setActive(at === 'last' ? lb.items.length - 1 : i < 0 ? 0 : i);
      unOut = outside([root, lb.el], function () { close(); });
    }
    function close(focus) {
      trigger.setAttribute('aria-expanded', 'false'); trigger.removeAttribute('aria-activedescendant');
      closePopup(lb.el); if (unOut) { unOut(); unOut = null; }
      if (focus) trigger.focus();
    }
    trigger.addEventListener('click', function () { if (isOpen(lb.el)) close(); else open(); });
    trigger.addEventListener('keydown', function (e) {
      var opened = isOpen(lb.el);
      if (!opened) { if (['ArrowDown', 'ArrowUp', 'Enter', ' '].indexOf(e.key) >= 0) { e.preventDefault(); open(e.key === 'ArrowUp' ? 'last' : null); } return; }
      if (e.key === 'ArrowDown') { e.preventDefault(); lb.move(1); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); lb.move(-1); }
      else if (e.key === 'Home' || e.key === 'End') { e.preventDefault(); lb.move(e.key === 'Home' ? 'first' : 'last'); }
      else if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); var it = lb.items[lb.active]; if (it) pick(it.op); }
      else if (e.key === 'Escape') { e.preventDefault(); close(true); }
      else if (e.key === 'Tab') close();
      else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) { var j = lb.typeahead(e.key); if (j >= 0) lb.setActive(j); }
    });
    lb.render(list);
    sync(true);
    root.open = open; root.close = close;
    root.setValue = function (v) { selected = multiple ? (v || []).slice() : v; sync(true); };
    return root;
  }

  /* ---------------------------------------------------------------- calendar, date picker, time */
  function ymd(d) { return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0'); }
  function fromYmd(s) { var m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s || ''); return m ? new Date(+m[1], +m[2] - 1, +m[3]) : null; }
  function addDays(d, n) { var x = new Date(d); x.setDate(x.getDate() + n); return x; }
  function addMonths(d, n) { var x = new Date(d.getFullYear(), d.getMonth() + n, 1); var last = new Date(x.getFullYear(), x.getMonth() + 1, 0).getDate(); x.setDate(Math.min(d.getDate(), last)); return x; }
  function weekStart(loc) {
    try { var L = new Intl.Locale(loc); var w = (L.getWeekInfo && L.getWeekInfo()) || L.weekInfo; if (w && w.firstDay) return w.firstDay % 7; } catch (e) { /* older engines */ }
    return /^en-(us|ca)|^ja|^he/.test(loc) ? 0 : 1;
  }
  function Calendar(p) {
    var loc = p.locale || lang();
    var first = p.weekStart != null ? p.weekStart : weekStart(loc);
    var today = p.today ? fromYmd(p.today) : new Date(); today.setHours(0, 0, 0, 0);
    var selected = fromYmd(p.value);
    var focus = selected || today;
    var min = fromYmd(p.min), max = fromYmd(p.max);
    var titleId = uid('ct');
    var L = isPl() ? ['Poprzedni miesiąc', 'Następny miesiąc', 'Dziś'] : ['Previous month', 'Next month', 'Today'];
    var title = h('h2', { 'class': 'm-calendar__title', id: titleId, 'aria-live': 'polite' });
    var prev = IconButton({ icon: 'chevron_left', label: L[0], size: 'sm' }), next = IconButton({ icon: 'chevron_right', label: L[1], size: 'sm' });
    var grid = h('table', { 'class': 'm-calendar__grid', role: 'grid', 'aria-labelledby': titleId });
    var root = h('div', { 'class': 'm-calendar__root', 'data-m': 'calendar' }, h('div', { 'class': 'm-calendar__header' }, title, prev, next), grid);
    if (p.footer !== false) {
      var tb = Button({ variant: 'text', size: 'sm', label: L[2] });
      tb.addEventListener('click', function () { go(today, true); select(today); });
      root.appendChild(h('div', { 'class': 'm-calendar__footer' }, tb));
    }
    var fmtTitle = new Intl.DateTimeFormat(loc, { month: 'long', year: 'numeric' });
    var fmtDay = new Intl.DateTimeFormat(loc, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
    var wdShort = new Intl.DateTimeFormat(loc, { weekday: 'short' }), wdLong = new Intl.DateTimeFormat(loc, { weekday: 'long' });
    function disabled(d) { return (min && d < min) || (max && d > max) || (p.isDisabled && p.isDisabled(d)); }
    function render(moveFocus) {
      title.textContent = fmtTitle.format(focus);
      grid.innerHTML = '';
      var head = h('tr', null);
      for (var i = 0; i < 7; i++) {
        var wd = new Date(2024, 0, 7 + ((first + i) % 7)); /* 7 Jan 2024 is a Sunday */
        head.appendChild(h('th', { 'class': 'm-calendar__weekday', scope: 'col' }, h('abbr', { title: wdLong.format(wd) }, wdShort.format(wd).replace(/\.$/, '').slice(0, 3))));
      }
      grid.appendChild(h('thead', null, head));
      var body = h('tbody', null);
      var start = new Date(focus.getFullYear(), focus.getMonth(), 1);
      start = addDays(start, -((start.getDay() - first + 7) % 7));
      for (var w = 0; w < 6; w++) {
        var row = h('tr', null);
        for (var k = 0; k < 7; k++) {
          var d = addDays(start, w * 7 + k);
          var cell = h('td', {
            'class': 'm-calendar__day', role: 'gridcell', tabindex: ymd(d) === ymd(focus) ? '0' : '-1', 'data-date': ymd(d),
            'aria-label': fmtDay.format(d), 'aria-selected': selected && ymd(d) === ymd(selected) ? 'true' : 'false',
            'aria-current': ymd(d) === ymd(today) ? 'date' : null, 'aria-disabled': disabled(d) ? 'true' : null,
            'data-outside': d.getMonth() !== focus.getMonth() ? '' : null
          }, String(d.getDate()));
          row.appendChild(cell);
        }
        body.appendChild(row);
        if (addDays(start, (w + 1) * 7).getMonth() !== focus.getMonth() && w >= 4) break;
      }
      grid.appendChild(body);
      if (moveFocus) { var c = grid.querySelector('[tabindex="0"]'); if (c) c.focus(); }
    }
    function go(d, moveFocus) { focus = d; render(moveFocus); }
    function select(d) {
      if (disabled(d)) return;
      selected = d; render(false);
      var c = grid.querySelector('[data-date="' + ymd(d) + '"]'); if (c && root.contains(document.activeElement)) c.focus();
      root.value = ymd(d);
      emit(root, 'change', { value: ymd(d), date: d });
    }
    grid.addEventListener('click', function (e) { var c = e.target.closest('.m-calendar__day'); if (c) { focus = fromYmd(c.getAttribute('data-date')); select(focus); } });
    grid.addEventListener('keydown', function (e) {
      var d = null, rtl = document.documentElement.dir === 'rtl';
      switch (e.key) {
        case 'ArrowLeft': d = addDays(focus, rtl ? 1 : -1); break;
        case 'ArrowRight': d = addDays(focus, rtl ? -1 : 1); break;
        case 'ArrowUp': d = addDays(focus, -7); break;
        case 'ArrowDown': d = addDays(focus, 7); break;
        case 'Home': d = addDays(focus, -((focus.getDay() - first + 7) % 7)); break;
        case 'End': d = addDays(focus, 6 - ((focus.getDay() - first + 7) % 7)); break;
        case 'PageUp': d = addMonths(focus, e.shiftKey ? -12 : -1); break;
        case 'PageDown': d = addMonths(focus, e.shiftKey ? 12 : 1); break;
        case 'Enter': case ' ': e.preventDefault(); select(focus); return;
        default: return;
      }
      e.preventDefault(); go(d, true);
    });
    prev.addEventListener('click', function () { go(addMonths(focus, -1), false); });
    next.addEventListener('click', function () { go(addMonths(focus, 1), false); });
    render(false);
    root.focusGrid = function () { var c = grid.querySelector('[tabindex="0"]'); if (c) c.focus(); };
    root.setValue = function (v) { selected = fromYmd(v); if (selected) focus = selected; render(false); };
    return root;
  }
  function datePattern(loc) {
    var parts = new Intl.DateTimeFormat(loc, { day: '2-digit', month: '2-digit', year: 'numeric' }).formatToParts(new Date(2026, 9, 15));
    var letters = loc.slice(0, 2) === 'pl' ? { day: 'DD', month: 'MM', year: 'RRRR' } : { day: 'DD', month: 'MM', year: 'YYYY' };
    return { order: parts.filter(function (x) { return x.type in letters; }).map(function (x) { return x.type; }),
      placeholder: parts.map(function (x) { return letters[x.type] || x.value; }).join('') };
  }
  function DatePicker(p) {
    var loc = p.locale || lang();
    var pat = datePattern(loc);
    var fmt = new Intl.DateTimeFormat(loc, { day: '2-digit', month: '2-digit', year: 'numeric' });
    var long = new Intl.DateTimeFormat(loc, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
    var L = isPl() ? ['Wybierz datę', 'Wpisz datę jako '] : ['Choose date', 'Enter a date as '];
    var input = h('input', { 'class': 'm-input__control', type: 'text', inputmode: 'numeric', placeholder: pat.placeholder, autocomplete: 'off', name: p.name, id: uid('dp'), style: { 'font-variant-numeric': 'tabular-nums' } });
    var btn = IconButton({ icon: 'calendar_month', label: L[0], haspopup: 'dialog', expanded: 'false' });
    var box = InputBox({ actions: [btn] }, input);
    var cal = Calendar({ value: p.value, min: p.min, max: p.max, today: p.today, locale: loc, isDisabled: p.isDisabled });
    var pop = newPopup('m-calendar__popup', { role: 'dialog', 'aria-modal': 'false', 'aria-label': L[0] });
    if (p.inline) { pop.setAttribute('data-inline', ''); pop.removeAttribute('popover'); pop.hidden = true; }
    pop.appendChild(cal);
    var help = h('div', { 'class': 'm-field__help', id: input.id + '-help' });
    input.setAttribute('aria-describedby', help.id);
    var root = h('div', { 'class': 'm-field__root', 'data-m': 'datepicker', style: { position: 'relative' } },
      p.label ? h('label', { 'class': 'm-field__label', 'for': input.id }, p.label) : null, box, help, pop);
    var unOut = null;
    function show(v) {
      var d = fromYmd(v);
      input.value = d ? fmt.format(d) : '';
      help.removeAttribute('data-tone'); help.innerHTML = '';
      if (d) help.appendChild(h('span', null, long.format(d))); else if (p.help) help.appendChild(h('span', null, p.help));
      input.removeAttribute('aria-invalid');
      root.value = d ? v : null;
    }
    function parse(s) {
      s = s.trim(); if (!s) return '';
      if (/^\d{4}-\d{1,2}-\d{1,2}$/.test(s)) { var q = s.split('-'); return ymd(new Date(+q[0], +q[1] - 1, +q[2])); }
      var nums = s.split(/\D+/).filter(Boolean);
      if (nums.length !== 3) return null;
      var o = {}; pat.order.forEach(function (k, i) { o[k] = +nums[i]; });
      if (o.year < 100) o.year += 2000;
      var d = new Date(o.year, o.month - 1, o.day);
      return d.getMonth() === o.month - 1 && d.getDate() === o.day ? ymd(d) : null;
    }
    function commit() {
      var v = parse(input.value);
      if (v === null) {
        input.setAttribute('aria-invalid', 'true'); help.setAttribute('data-tone', 'danger');
        help.innerHTML = ''; help.appendChild(icon('error')); help.appendChild(h('span', null, L[1] + pat.placeholder + '.'));
        return;
      }
      show(v); cal.setValue(v); emit(root, 'change', { value: v || null });
    }
    function open() {
      btn.setAttribute('aria-expanded', 'true');
      openPopup(pop, box, { align: 'end' });
      cal.focusGrid();
      unOut = outside([pop, box], function () { close(false); });
    }
    function close(focus) {
      btn.setAttribute('aria-expanded', 'false'); closePopup(pop); if (unOut) { unOut(); unOut = null; }
      if (focus) input.focus();
    }
    btn.addEventListener('click', function () { if (isOpen(pop)) close(true); else open(); });
    pop.addEventListener('keydown', function (e) { if (e.key === 'Escape') { e.preventDefault(); close(true); } });
    cal.addEventListener('change', function (e) { show(e.detail.value); emit(root, 'change', { value: e.detail.value }); if (!p.inline) close(true); });
    input.addEventListener('change', commit);
    input.addEventListener('keydown', function (e) { if (e.key === 'ArrowDown' && e.altKey) { e.preventDefault(); open(); } });
    show(p.value || '');
    root.open = open;
    return root;
  }
  function zoneLabel(loc) {
    try {
      var parts = new Intl.DateTimeFormat(loc, { timeZoneName: 'shortOffset' }).formatToParts(new Date());
      var z = parts.filter(function (x) { return x.type === 'timeZoneName'; })[0];
      return z ? z.value.replace(/^GMT/, 'UTC').replace(/^UTC$/, 'UTC') : '';
    } catch (e) { return ''; }
  }
  /* Native time entry (segment editing, the platform's own accessibility), with the reader's zone printed:
     deadlines in a play-by-forum cross time zones. */
  function TimeField(p) {
    var loc = p.locale || lang();
    var input = h('input', { 'class': 'm-input__control', type: 'time', name: p.name, value: p.value, step: p.step, id: uid('tm') });
    var zone = p.zone || zoneLabel(loc);
    var full = p.zoneName || (Intl.DateTimeFormat().resolvedOptions().timeZone || '');
    return Field({ label: p.label, help: p.help, control: InputBox({ leading: 'schedule', trailing: zone, trailingTitle: full }, input) });
  }

  /* ---------------------------------------------------------------- slider */
  function Slider(p) {
    var min = p.min != null ? p.min : 0, max = p.max != null ? p.max : 100, step = p.step || 1;
    var nf = new Intl.NumberFormat(lang(), { maximumFractionDigits: 2 });
    var format = p.format || function (v) { return nf.format(v) + (p.unit || ''); };
    var input = h('input', { 'class': 'm-slider__control', type: 'range', min: min, max: max, step: step, value: p.value != null ? p.value : min, name: p.name, id: uid('sd'), disabled: p.disabled });
    var out = h('output', { 'class': 'm-slider__value', 'for': input.id });
    var root = h('div', { 'class': 'm-slider__root', 'data-m': 'slider' },
      h('div', { 'class': 'm-slider__header' }, h('label', { 'class': 'm-slider__label', 'for': input.id }, p.label), out),
      input,
      p.marks ? h('div', { 'class': 'm-slider__marks', 'aria-hidden': 'true' }, p.marks.map(function (m) { return h('span', null, m); })) : null);
    function update() {
      var v = +input.value;
      input.style.setProperty('--fill', ((v - min) / (max - min) * 100) + '%');
      out.textContent = format(v);
      input.setAttribute('aria-valuetext', format(v));
      root.value = v;
    }
    input.addEventListener('input', function () { update(); emit(root, 'input', { value: +input.value }); });
    input.addEventListener('change', function () { emit(root, 'change', { value: +input.value }); });
    [input, root.querySelector('.m-slider__header')].forEach(function (n) { ['input', 'change'].forEach(function (t) { n.addEventListener(t, function (e) { if (e.target === input) e.stopPropagation(); }); }); });
    root.setValue = function (v) { input.value = v; update(); };
    root.setDisabled = function (on) { input.disabled = !!on; };
    update();
    return root;
  }

  /* ---------------------------------------------------------------- colour picker (OKLCH) */
  /* A plane of lightness (up) against chroma (right) at one hue, with sRGB's edge drawn in and the
     colours past it hatched; a hue slider; L, C and H as numbers. Optional `against` grounds draw
     their contrast threshold onto the plane and print the ratio, so a colour is picked with its
     duties in view. Emits input while dragging and change when a value is committed. */
  var CMAX = 0.33;
  function normHex(s) {
    s = String(s || '').trim().toLowerCase();
    if (s[0] !== '#') s = '#' + s;
    if (s.length === 4) s = '#' + s[1] + s[1] + s[2] + s[2] + s[3] + s[3];
    return /^#[0-9a-f]{6}$/.test(s) ? s : null;
  }
  function ColorPicker(p) {
    var C = M.color;
    var L = isPl()
      ? { area: 'Jasność i nasycenie', hue: 'Odcień', l: 'Jasność', c: 'Chroma', h: 'Odcień', hex: 'Hex', choose: 'Wybierz kolor', clear: 'Wyczyść',
          gamut: 'Chroma ograniczona do ', gamut2: ' na ekranach sRGB', needs: 'wymaga ', on: 'na: ', suggestions: 'Propozycje', invalid: 'Wpisz kolor jako #rrggbb' }
      : { area: 'Lightness and chroma', hue: 'Hue', l: 'Lightness', c: 'Chroma', h: 'Hue', hex: 'Hex', choose: 'Choose colour', clear: 'Clear',
          gamut: 'Chroma limited to ', gamut2: ' on sRGB screens', needs: 'needs ', on: 'on ', suggestions: 'Suggestions', invalid: 'Enter a colour as #rrggbb' };
    var nf = function (v, d) { return new Intl.NumberFormat(lang(), { minimumFractionDigits: d, maximumFractionDigits: d }).format(v); };
    var empty = !p.value;
    var start = C.fromHex(normHex(p.value) || normHex(p.placeholder) || '#808080');
    var want = { l: start.l, c: start.c, h: start.c < 0.004 && p.hue != null ? p.hue : start.h };
    var against = p.against || [];
    var label = p.label || '';

    /* panel */
    var canvas = h('canvas', { 'class': 'm-colorpicker__canvas', 'aria-hidden': 'true' });
    var thumb = h('div', { 'class': 'm-colorpicker__thumb', role: 'slider', tabindex: '0', 'aria-label': L.area, 'aria-valuemin': '0', 'aria-valuemax': '100' });
    var area = h('div', { 'class': 'm-colorpicker__area' }, canvas, thumb);
    var hue = h('input', { 'class': 'm-colorpicker__hue', type: 'range', min: '0', max: '360', step: '1', 'aria-label': L.hue });
    var mini = function (key, name, mode) {
      var input = h('input', { 'class': 'm-colorpicker__valueInput', type: 'text', inputmode: mode || 'decimal', autocomplete: 'off', spellcheck: 'false', 'aria-label': name, id: uid('cv') });
      return { input: input, el: h('label', { 'class': 'm-colorpicker__value', 'for': input.id }, h('span', { 'class': 'm-colorpicker__valueLabel', 'aria-hidden': 'true' }, key), input) };
    };
    var fL = mini('L', L.l), fC = mini('C', L.c), fH = mini('H', L.h), fX = p.inline ? mini('#', L.hex, 'text') : null;
    var values = h('div', { 'class': 'm-colorpicker__values', 'data-count': fX ? '4' : '3' }, fX ? fX.el : null, fL.el, fC.el, fH.el);
    var note = h('div', { 'class': 'm-colorpicker__note', hidden: true }, icon('info'), h('span'));
    var checks = h('ul', { 'class': 'm-colorpicker__checks' });
    var swatches = p.swatches && p.swatches.length ? h('div', { 'class': 'm-colorpicker__swatches', role: 'group', 'aria-label': p.swatchesLabel || L.suggestions },
      p.swatches.map(function (s) {
        var hx = normHex(s.value || s);
        var b = h('button', { 'class': 'm-colorpicker__swatchButton', type: 'button', 'aria-label': (s.label ? s.label + ', ' : '') + hx, 'data-tip': s.label || hx, style: { '--pick': hx } });
        b.addEventListener('click', function () { setHex(hx, true); emitChange(); });
        return b;
      })) : null;
    var panelBody = [area, hue, values, note, checks, swatches, p.footer || null];
    var panel;
    if (p.inline) panel = h('div', { 'class': 'm-colorpicker__panel', 'data-static': '' }, panelBody);
    else { panel = newPopup('m-colorpicker__panel', { role: 'dialog', 'aria-label': label || L.choose }); panelBody.forEach(function (n) { if (n) panel.appendChild(n); }); }

    /* control: swatch button opens the panel, hex is typed or pasted directly */
    var trigger = null, hexInput = null, control = null, help = null;
    var helpId = uid('ch');
    if (!p.inline) {
      trigger = h('button', { 'class': 'm-colorpicker__trigger', type: 'button', 'aria-haspopup': 'dialog', 'aria-expanded': 'false' }, h('span', { 'class': 'm-colorpicker__swatch' }));
      hexInput = h('input', { 'class': 'm-input__control m-colorpicker__hex', type: 'text', autocomplete: 'off', spellcheck: 'false', id: uid('cx'), placeholder: p.placeholder || '#rrggbb', 'aria-describedby': helpId });
      var actions = [];
      if (p.clearable) {
        var clr = IconButton({ icon: 'close', label: L.clear + (label ? ': ' + label : ''), size: 'sm', tip: false });
        clr.addEventListener('click', function () { empty = true; hexInput.value = ''; render(); emit(root, 'change', { value: null }); });
        actions.push(clr);
      }
      control = InputBox({ leading: trigger, leadingText: true, actions: actions }, hexInput);
      control.classList.add('m-colorpicker__control');
      help = h('div', { 'class': 'm-field__help', id: helpId, hidden: !p.help }, h('span', null, p.help || ''));
    }
    var root = h('div', { 'class': 'm-colorpicker__root', 'data-m': 'colorpicker' },
      label ? h(p.inline ? 'span' : 'label', { 'class': 'm-field__label', 'for': hexInput ? hexInput.id : null }, label) : null,
      control, help, panel);
    if (p.inline && label) panel.setAttribute('aria-label', label);

    /* state -> view */
    var drawnHue = null, drawnAgainst = null, raf = 0;
    function shown() { return C.gamut(want); }
    function render(skip) {
      var s = shown(), hex = C.toHex(s);
      root.value = empty ? null : hex;
      root.style.setProperty('--pick', hex);
      if (trigger) {
        trigger.setAttribute('aria-label', L.choose + (label ? ', ' + label : '') + (empty ? '' : ', ' + hex));
        trigger.toggleAttribute('data-empty', empty);
      }
      if (hexInput && skip !== hexInput) hexInput.value = empty ? '' : hex;
      if (fX && skip !== fX.input) fX.input.value = hex;
      if (skip !== fL.input) fL.input.value = nf(s.l * 100, 1);
      if (skip !== fC.input) fC.input.value = nf(s.c, 3);
      if (skip !== fH.input) fH.input.value = nf(want.h, 0);
      hue.value = String(Math.round(want.h));
      hue.setAttribute('aria-valuetext', nf(want.h, 0) + '°');
      thumb.style.left = (Math.min(s.c, CMAX) / CMAX * 100) + '%';
      thumb.style.top = ((1 - s.l) * 100) + '%';
      thumb.setAttribute('aria-valuenow', String(Math.round(s.l * 100)));
      thumb.setAttribute('aria-valuetext', L.l + ' ' + nf(s.l * 100, 0) + ', ' + L.c.toLowerCase() + ' ' + nf(s.c, 3));
      thumb.setAttribute('data-tone', s.l > 0.62 ? 'light' : 'dark');
      var stops = [];
      for (var a = 0; a <= 360; a += 30) stops.push(C.toHex({ l: want.l, c: Math.max(want.c, 0.09), h: a }));
      hue.style.setProperty('--track', 'linear-gradient(to right, ' + stops.join(', ') + ')');
      var lost = want.c - s.c;
      note.hidden = !(lost > 0.004);
      if (!note.hidden) note.lastChild.textContent = L.gamut + nf(s.c, 3) + L.gamut2;
      renderChecks(hex);
      var key = JSON.stringify(against.map(function (g) { return [g.hex, g.min]; }));
      if (drawnHue !== Math.round(want.h) || drawnAgainst !== key) { drawnAgainst = key; schedule(); }
    }
    function renderChecks(hex) {
      checks.innerHTML = '';
      checks.hidden = !against.length;
      against.forEach(function (g) {
        var r = C.contrast(hex, g.hex), pass = r >= (g.min || 0) - 0.005;
        checks.appendChild(h('li', { 'class': 'm-colorpicker__check', 'data-pass': String(pass) },
          h('span', { 'class': 'm-colorpicker__sample', 'aria-hidden': 'true', style: { '--g': g.hex } }, g.sample || 'Aa'),
          h('span', { 'class': 'm-colorpicker__checkLabel' }, L.on + g.label),
          h('span', { 'class': 'm-colorpicker__ratio' }, nf(r, 1) + ':1'),
          g.min ? h('span', { 'class': 'm-colorpicker__status' }, icon(pass ? 'check' : 'close'), L.needs + nf(g.min, g.min % 1 ? 1 : 0) + ':1') : null));
      });
    }
    function schedule() { if (!raf) raf = requestAnimationFrame(function () { raf = 0; draw(); }); }
    function draw() {
      var W = Math.round(area.clientWidth), H = Math.round(area.clientHeight);
      if (!W || !H) return;
      drawnHue = Math.round(want.h);
      canvas.width = W; canvas.height = H;
      var ctx = canvas.getContext('2d');
      var img = ctx.createImageData(W, H), d = img.data;
      var hr = want.h * Math.PI / 180, ca = Math.cos(hr), sa = Math.sin(hr);
      var inG = new Uint8Array(W * H), Y = new Float32Array(W * H), Ls = new Float32Array(H);
      var g = C.toGamma, lab = C.oklabToLinear;
      var duties = against.filter(function (a) { return a.min; }).map(function (a) { return { y: C.luminance(a.hex), min: a.min }; });
      var passes = function (yy) {
        for (var q = 0; q < duties.length; q++) {
          var u = duties[q], hi = Math.max(yy, u.y), lo = Math.min(yy, u.y);
          if ((hi + 0.05) / (lo + 0.05) < u.min) return false;
        }
        return true;
      };
      for (var y = 0; y < H; y++) {
        var l = 1 - y / (H - 1); Ls[y] = l;
        var grey = g(l * l * l);
        for (var x = 0; x < W; x++) {
          var c = x / (W - 1) * CMAX;
          var rgb = lab(l, c * ca, c * sa);
          var ok = rgb[0] >= -1e-5 && rgb[0] <= 1.0001 && rgb[1] >= -1e-5 && rgb[1] <= 1.0001 && rgb[2] >= -1e-5 && rgb[2] <= 1.0001;
          var i = y * W + x, o = i * 4;
          var r = Math.min(1, Math.max(0, rgb[0])), gg = Math.min(1, Math.max(0, rgb[1])), b = Math.min(1, Math.max(0, rgb[2]));
          inG[i] = ok ? 1 : 0;
          var yy = Y[i] = 0.2126 * r + 0.7152 * gg + 0.0722 * b;
          r = g(r); gg = g(gg); b = g(b);
          if (!ok) {
            var t = (x + y) % 6 < 1 ? 0.5 : 0.78;
            var gr = grey + ((x + y) % 6 < 1 ? (l > 0.55 ? -0.12 : 0.12) : 0);
            r = r + (gr - r) * t; gg = gg + (gr - gg) * t; b = b + (gr - b) * t;
          } else if (!passes(yy)) {
            r = r + (grey - r) * 0.62; gg = gg + (grey - gg) * 0.62; b = b + (grey - b) * 0.62;
          }
          d[o] = r * 255; d[o + 1] = gg * 255; d[o + 2] = b * 255; d[o + 3] = 255;
        }
      }
      /* the sRGB edge, and the line where the colour starts to meet every contrast duty */
      var paint = function (i, alpha, l) {
        var o = i * 4, v = l > 0.6 ? 18 : 245;
        d[o] = d[o] + (v - d[o]) * alpha; d[o + 1] = d[o + 1] + (v - d[o + 1]) * alpha; d[o + 2] = d[o + 2] + (v - d[o + 2]) * alpha;
      };
      for (var y2 = 1; y2 < H; y2++) {
        for (var x2 = 1; x2 < W; x2++) {
          var j = y2 * W + x2;
          if (inG[j] !== inG[j - 1] || inG[j] !== inG[j - W]) paint(j, 0.55, Ls[y2]);
          else if (duties.length && inG[j] && x2 % 6 < 4 && passes(Y[j]) !== passes(Y[j - W])) paint(j, 0.85, Ls[y2]);
        }
      }
      ctx.putImageData(img, 0, 0);
    }

    /* view -> state */
    function setHex(hex, keepHue) {
      var c = C.fromHex(hex);
      want = { l: c.l, c: c.c, h: c.c < 0.004 && keepHue ? want.h : c.h };
      empty = false;
    }
    function emitInput() { render(); emit(root, 'input', { value: root.value, lch: shown() }); }
    function emitChange() { render(); emit(root, 'change', { value: root.value, lch: shown() }); }
    function fromPointer(e) {
      var r = area.getBoundingClientRect();
      var x = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)), y = Math.min(1, Math.max(0, (e.clientY - r.top) / r.height));
      var l = 1 - y, c = x * CMAX;
      want = { l: l, c: Math.min(c, C.maxChroma(l, want.h)), h: want.h };
      empty = false;
      emitInput();
    }
    area.addEventListener('pointerdown', function (e) {
      if (e.button !== 0) return;
      e.preventDefault(); thumb.focus();
      area.setPointerCapture(e.pointerId); area.setAttribute('data-dragging', '');
      fromPointer(e);
    });
    area.addEventListener('pointermove', function (e) { if (area.hasPointerCapture(e.pointerId)) fromPointer(e); });
    var endDrag = function (e) { if (area.hasPointerCapture && area.hasPointerCapture(e.pointerId)) { area.releasePointerCapture(e.pointerId); } if (area.hasAttribute('data-dragging')) { area.removeAttribute('data-dragging'); emitChange(); } };
    area.addEventListener('pointerup', endDrag); area.addEventListener('pointercancel', endDrag);
    thumb.addEventListener('keydown', function (e) {
      var s = shown(), dl = 0, dc = 0, big = e.shiftKey;
      if (e.key === 'ArrowUp') dl = big ? 0.1 : 0.01; else if (e.key === 'ArrowDown') dl = big ? -0.1 : -0.01;
      else if (e.key === 'PageUp') dl = 0.1; else if (e.key === 'PageDown') dl = -0.1;
      else if (e.key === 'ArrowRight') dc = big ? 0.02 : 0.005; else if (e.key === 'ArrowLeft') dc = big ? -0.02 : -0.005;
      else if (e.key === 'Home') dc = -1; else if (e.key === 'End') dc = 1;
      else return;
      e.preventDefault();
      var l = Math.min(1, Math.max(0, s.l + dl));
      var c = Math.min(Math.max(0, s.c + dc), C.maxChroma(l, want.h));
      want = { l: l, c: c, h: want.h }; empty = false;
      emitChange();
    });
    hue.addEventListener('input', function () { want = { l: want.l, c: want.c, h: +hue.value }; empty = false; emitInput(); });
    hue.addEventListener('change', function () { emitChange(); });
    var parse = function (v) { return parseFloat(String(v).replace(',', '.')); };
    [[fL, function (v) { return { l: Math.min(100, Math.max(0, v)) / 100 }; }], [fC, function (v) { return { c: Math.min(0.37, Math.max(0, v)) }; }], [fH, function (v) { return { h: ((v % 360) + 360) % 360 }; }]].forEach(function (pair) {
      var f = pair[0];
      f.input.addEventListener('change', function () {
        var v = parse(f.input.value);
        if (!isNaN(v)) { var u = pair[1](v); want = { l: u.l != null ? u.l : want.l, c: u.c != null ? u.c : want.c, h: u.h != null ? u.h : want.h }; empty = false; }
        emitChange();
      });
      f.input.addEventListener('keydown', function (e) { if (e.key === 'Enter') { e.preventDefault(); f.input.dispatchEvent(new Event('change')); } });
    });
    [hexInput, fX && fX.input].forEach(function (inp) {
      if (!inp) return;
      inp.addEventListener('input', function () {
        var hx = normHex(inp.value);
        inp.removeAttribute('aria-invalid');
        if (hx && inp.value.replace('#', '').length === 6) { setHex(hx, true); render(inp); emit(root, 'input', { value: root.value, lch: shown() }); }
      });
      inp.addEventListener('change', function () {
        if (!inp.value.trim() && p.clearable) { empty = true; render(); emit(root, 'change', { value: null }); return; }
        var hx = normHex(inp.value);
        if (hx) { inp.removeAttribute('aria-invalid'); setHex(hx, true); emitChange(); root.setHelp(p.help); }
        else { inp.setAttribute('aria-invalid', 'true'); root.setHelp(L.invalid, 'danger'); }
      });
      inp.addEventListener('keydown', function (e) { if (e.key === 'Enter') { e.preventDefault(); inp.dispatchEvent(new Event('change')); } });
    });

    /* popup behaviour */
    var unOut = null;
    function open() {
      if (!trigger) return;
      trigger.setAttribute('aria-expanded', 'true');
      openPopup(panel, control, { align: 'start' });
      drawnHue = null; render();
      thumb.focus();
      unOut = outside([panel, control], function () { close(false); });
    }
    function close(focus) {
      if (!trigger) return;
      trigger.setAttribute('aria-expanded', 'false');
      closePopup(panel);
      if (unOut) { unOut(); unOut = null; }
      if (focus) trigger.focus();
    }
    if (trigger) {
      trigger.addEventListener('click', function () { if (isOpen(panel)) close(true); else open(); });
      panel.addEventListener('keydown', function (e) { if (e.key === 'Escape') { e.preventDefault(); close(true); } });
    }
    if (typeof ResizeObserver !== 'undefined') new ResizeObserver(function () { drawnHue = null; schedule(); }).observe(area);

    [panel, control].forEach(function (n) {
      if (!n) return;
      ['input', 'change'].forEach(function (t) { n.addEventListener(t, function (e) { e.stopPropagation(); }); });
    });
    root.open = open; root.close = close;
    root.setValue = function (hex) { var hx = normHex(hex); if (!hx) { empty = true; render(); return; } setHex(hx, true); render(); };
    root.setAgainst = function (list) { against = list || []; render(); };
    root.setHelp = function (text, tone) { if (!help) return; help.hidden = !text; help.lastChild.textContent = text || ''; if (tone) help.setAttribute('data-tone', tone); else help.removeAttribute('data-tone'); };
    render();
    return root;
  }

  /* ---------------------------------------------------------------- file upload */
  function FileUpload(p) {
    var accept = p.accept || 'image/png,image/jpeg,image/webp';
    var maxBytes = p.maxBytes || 1048576;
    var L = isPl()
      ? { title: 'Upuść obraz tutaj', choose: 'Wybierz plik', remove: 'Usuń', over: 'Ponad limit ', type: 'Tego typu pliku nie można dodać', done: 'Gotowe' }
      : { title: 'Drop an image here', choose: 'Choose file', remove: 'Remove', over: 'Over the limit of ', type: 'This file type can’t be added', done: 'Ready' };
    var input = h('input', { type: 'file', accept: accept, multiple: p.multiple, id: uid('up') });
    var list = h('ul', { 'class': 'm-upload__list', 'aria-live': 'polite' });
    var choose = Button({ variant: 'tonal', size: 'sm', icon: 'upload', label: L.choose });
    choose.setAttribute('tabindex', '-1'); choose.setAttribute('aria-hidden', 'true');
    var zone = h('label', { 'class': 'm-upload__dropzone', 'for': input.id },
      input, icon('add_photo_alternate', { 'class': 'm-upload__icon' }),
      h('span', { 'class': 'm-upload__title' }, p.title || L.title),
      choose,
      h('span', { 'class': 'm-upload__hint' }, p.hint || ('PNG, JPEG, WebP · ' + fmtBytes(maxBytes)).replace(' · ', isPl() ? ', do ' : ', up to ')));
    choose.addEventListener('click', function (e) { e.preventDefault(); input.click(); });
    var root = h('div', { 'class': 'm-upload__root', 'data-m': 'upload' }, p.label ? h('span', { 'class': 'm-field__label' }, p.label) : null, zone, list);
    var api = {
      add: function (f) {
        var id = uid('file');
        var bad = f.size > maxBytes ? L.over + fmtBytes(maxBytes) : (accept && f.type && accept.split(',').indexOf(f.type) < 0 ? L.type : null);
        var thumb = h('span', { 'class': 'm-upload__thumb' }, f.url ? h('img', { src: f.url, alt: '' }) : icon(bad ? 'error' : 'image'));
        var meta = h('span', { 'class': 'm-upload__meta' });
        var rm = IconButton({ icon: 'close', label: L.remove + ' ' + f.name, size: 'sm' });
        var item = h('li', { 'class': 'm-upload__item', id: id, 'data-state': bad ? 'error' : (f.state || 'ready') }, thumb,
          h('span', { style: { 'min-width': '0' } }, h('span', { 'class': 'm-upload__name', style: { display: 'block' } }, f.name), meta), rm);
        rm.addEventListener('click', function () { item.remove(); emit(root, 'remove', { id: id }); });
        list.appendChild(item);
        api.setState(id, bad ? 'error' : f.state || 'ready', bad || null, f.progress, f.size);
        return id;
      },
      setState: function (id, state, message, progress, size) {
        var item = list.querySelector('#' + id); if (!item) return;
        item.setAttribute('data-state', state);
        var meta = item.querySelector('.m-upload__meta'); meta.innerHTML = '';
        if (size != null) item._size = size;
        if (state === 'error') { meta.appendChild(icon('error')); meta.appendChild(h('span', null, message)); }
        else if (state === 'uploading') { meta.appendChild(Progress({ value: progress || 0, bare: true, label: item.querySelector('.m-upload__name').textContent }).querySelector('.m-progress__track')); meta.appendChild(h('span', null, Math.round(progress || 0) + '%')); }
        else meta.appendChild(h('span', null, fmtBytes(item._size || 0)));
      }
    };
    function take(files) {
      Array.prototype.forEach.call(files, function (f) {
        var url = /^image\//.test(f.type) ? URL.createObjectURL(f) : null;
        var id = api.add({ name: f.name, size: f.size, type: f.type, url: url });
        emit(root, 'files', { id: id, file: f, api: api });
      });
    }
    input.addEventListener('change', function () { take(input.files); input.value = ''; });
    ['dragenter', 'dragover'].forEach(function (t) { zone.addEventListener(t, function (e) { e.preventDefault(); zone.setAttribute('data-dragover', ''); }); });
    ['dragleave', 'drop'].forEach(function (t) { zone.addEventListener(t, function (e) { e.preventDefault(); zone.removeAttribute('data-dragover'); if (t === 'drop' && e.dataTransfer) take(e.dataTransfer.files); }); });
    (p.files || []).forEach(api.add);
    root.api = api;
    return root;
  }

  /* ---------------------------------------------------------------- accordion, collapsible */
  function Accordion(p) {
    var name = p.multiple ? null : uid('acc');
    var v = p.variant || 'flush';
    var cls = function (part) { return 'm-accordion__' + part + ' m-accordion__' + part + '--variant_' + v; };
    return h('div', { 'class': cls('root') }, p.items.map(function (it) {
      var content = typeof it.content === 'string' ? h('div', { 'class': cls('content'), html: it.content }) : h('div', { 'class': cls('content') }, it.content);
      return h('details', { 'class': cls('item'), name: name, open: it.open },
        h('summary', { 'class': cls('trigger') },
          it.icon ? icon(it.icon) : null,
          h('span', { 'class': cls('title') }, it.title),
          it.meta ? h('span', { 'class': cls('meta') }, it.meta) : null,
          icon('expand_more', { 'class': cls('indicator') })),
        content);
    }));
  }
  function Collapsible(p) {
    var v = p.variant || 'default';
    var cls = function (part) { return 'm-collapsible__' + part + ' m-collapsible__' + part + '--variant_' + v; };
    var region = h('div', { 'class': cls('content'), id: uid('cl'), role: 'region' });
    if (typeof p.content === 'string') region.innerHTML = p.content; else region.appendChild(p.content);
    var L = isPl() ? ['Spoiler', 'Pokaż', 'Ukryj'] : ['Spoiler', 'Show', 'Hide'];
    var trigger = v === 'spoiler'
      ? h('button', { 'class': cls('trigger'), type: 'button', 'aria-expanded': 'false', 'aria-controls': region.id },
          icon('visibility_off'), h('span', { 'class': cls('label') }, p.label || L[0]), h('span', { 'class': cls('topic') }, p.topic || ''), h('span', { 'class': cls('action') }, L[1]))
      : h('button', { 'class': cls('trigger'), type: 'button', 'aria-expanded': 'false', 'aria-controls': region.id },
          h('span', { 'class': cls('label') }, p.label), icon('expand_more', { 'class': cls('indicator') }));
    region.setAttribute('aria-labelledby', trigger.id = uid('ct'));
    var root = h('div', { 'class': cls('root'), 'data-m': 'collapsible' }, trigger, region);
    function set(open) {
      trigger.setAttribute('aria-expanded', String(open));
      if (open) region.removeAttribute('hidden'); else region.setAttribute('hidden', 'until-found');
      if (region.getAttribute('hidden') === 'until-found' && !('onbeforematch' in document.body)) region.hidden = true;
      var a = trigger.querySelector('.m-collapsible__action'); if (a) a.textContent = open ? L[2] : L[1];
      var ic = trigger.querySelector('.m-icon'); if (v === 'spoiler' && ic) ic.textContent = open ? 'visibility' : 'visibility_off';
    }
    trigger.addEventListener('click', function () { set(trigger.getAttribute('aria-expanded') !== 'true'); emit(root, 'toggle', { open: trigger.getAttribute('aria-expanded') === 'true' }); });
    region.addEventListener('beforematch', function () { set(true); });
    set(!!p.open);
    return root;
  }

  /* ---------------------------------------------------------------- tree view */
  function TreeView(p) {
    var root = h('ul', { 'class': 'm-tree__root', role: 'tree', 'aria-label': p.label, 'data-m': 'tree' });
    function build(items, parent, level) {
      items.forEach(function (it, i) {
        var kids = it.children && it.children.length;
        var li = h('li', { 'class': 'm-tree__item', role: 'treeitem', tabindex: '-1', 'aria-level': level, 'aria-setsize': items.length, 'aria-posinset': i + 1,
          'aria-expanded': kids ? String(!!it.expanded) : null, 'aria-selected': it.selected ? 'true' : 'false', 'data-leaf': kids ? null : '', 'data-id': it.id || null,
          style: { '--level': String(level) } },
          h('div', { 'class': 'm-tree__row' },
            h('span', { 'class': 'm-tree__toggle', 'aria-hidden': 'true' }, icon('chevron_right')),
            it.icon ? icon(it.icon, { 'class': 'm-tree__icon' }) : null,
            h('span', { 'class': 'm-tree__label' }, it.label),
            it.count != null ? h('span', { 'class': 'm-tree__count' }, String(it.count)) : null));
        li._item = it;
        if (kids) { var g = h('ul', { 'class': 'm-tree__group', role: 'group' }); li.appendChild(g); build(it.children, g, level + 1); }
        parent.appendChild(li);
      });
    }
    build(p.items, root, 1);
    wireTree(root);
    return root;
  }
  function wireTree(root) {
    function visible() { return Array.prototype.filter.call(root.querySelectorAll('[role=treeitem]'), function (n) { var a = n.parentElement.closest('[role=treeitem]'); while (a) { if (a.getAttribute('aria-expanded') === 'false') return false; a = a.parentElement.closest('[role=treeitem]'); } return true; }); }
    function focusItem(n) { root.querySelectorAll('[role=treeitem]').forEach(function (x) { x.tabIndex = -1; }); n.tabIndex = 0; n.focus(); }
    function setOpen(n, open) { if (n.hasAttribute('aria-expanded')) { n.setAttribute('aria-expanded', String(open)); emit(n, 'toggle', { open: open }); } }
    function select(n) { root.querySelectorAll('[aria-selected=true]').forEach(function (x) { x.setAttribute('aria-selected', 'false'); }); n.setAttribute('aria-selected', 'true'); emit(root, 'select', { id: n.getAttribute('data-id'), item: n._item }); }
    var start = root.querySelector('[aria-selected=true]') || root.querySelector('[role=treeitem]');
    if (start) start.tabIndex = 0;
    root.addEventListener('click', function (e) {
      var n = e.target.closest('[role=treeitem]'); if (!n) return;
      if (e.target.closest('.m-tree__toggle')) setOpen(n, n.getAttribute('aria-expanded') !== 'true');
      else { select(n); if (n.getAttribute('aria-expanded') === 'false') setOpen(n, true); }
      focusItem(n); e.stopPropagation();
    });
    var buf = '', t;
    root.addEventListener('keydown', function (e) {
      var n = e.target.closest('[role=treeitem]'); if (!n) return;
      var vis = visible(), i = vis.indexOf(n), exp = n.getAttribute('aria-expanded');
      var parent = n.parentElement.closest('[role=treeitem]');
      var rtl = document.documentElement.dir === 'rtl';
      var key = e.key === 'ArrowRight' && rtl ? 'ArrowLeft' : e.key === 'ArrowLeft' && rtl ? 'ArrowRight' : e.key;
      if (key === 'ArrowDown' && vis[i + 1]) focusItem(vis[i + 1]);
      else if (key === 'ArrowUp' && vis[i - 1]) focusItem(vis[i - 1]);
      else if (key === 'Home') focusItem(vis[0]);
      else if (key === 'End') focusItem(vis[vis.length - 1]);
      else if (key === 'ArrowRight') { if (exp === 'false') setOpen(n, true); else if (exp === 'true') focusItem(n.querySelector('[role=treeitem]')); }
      else if (key === 'ArrowLeft') { if (exp === 'true') setOpen(n, false); else if (parent) focusItem(parent); }
      else if (key === 'Enter' || key === ' ') select(n);
      else if (key === '*') { Array.prototype.forEach.call(n.parentElement.children, function (s) { setOpen(s, true); }); }
      else if (key.length === 1 && /\S/.test(key)) {
        clearTimeout(t); t = setTimeout(function () { buf = ''; }, 600); buf += fold(key);
        for (var k = 1; k <= vis.length; k++) { var c = vis[(i + (buf.length === 1 ? k : k - 1)) % vis.length]; if (norm(c.querySelector('.m-tree__label').textContent).indexOf(buf) === 0) { focusItem(c); break; } }
      } else return;
      e.preventDefault(); e.stopPropagation();
    });
  }

  /* ---------------------------------------------------------------- pagination, breadcrumb, steps */
  function pageWindow(page, count) {
    var out = [], add = function (x) { if (out[out.length - 1] !== x) out.push(x); };
    for (var n = 1; n <= count; n++) {
      if (n === 1 || n === count || Math.abs(n - page) <= 1 || (page <= 3 && n <= 4) || (page >= count - 2 && n >= count - 3)) add(n);
      else add('…');
    }
    return out;
  }
  function Pagination(p) {
    var L = isPl() ? { nav: 'Strony', prev: 'Poprzednia strona', next: 'Następna strona', page: 'Strona ', of: ' z ' } : { nav: 'Pages', prev: 'Previous page', next: 'Next page', page: 'Page ', of: ' of ' };
    var root = h('nav', { 'class': 'm-pagination__root', 'aria-label': p.label || L.nav, 'data-m': 'pagination' });
    var page = p.page || 1, count = p.count || 1;
    function link(n, content, label, extra) {
      var dis = n < 1 || n > count;
      var a = h(p.href ? 'a' : 'button', Object.assign({ 'class': 'm-pagination__page', href: p.href && !dis ? p.href(n) : null, type: p.href ? null : 'button', 'aria-label': label, 'data-tip': extra && extra.tip ? label : null,
        'aria-current': n === page && !(extra && extra.arrow) ? 'page' : null, 'aria-disabled': dis ? 'true' : null }, {}), content);
      if (!p.href) a.addEventListener('click', function () { if (!dis) { page = n; render(); emit(root, 'change', { page: n }); root.querySelector('[aria-current=page]') && root.querySelector('[aria-current=page]').focus(); } });
      return a;
    }
    function render() {
      root.innerHTML = '';
      var ul = h('ul', { 'class': 'm-pagination__list' });
      ul.appendChild(h('li', null, link(page - 1, icon('chevron_left'), L.prev, { arrow: true, tip: true })));
      pageWindow(page, count).forEach(function (x) {
        ul.appendChild(x === '…' ? h('li', { 'data-page': '', 'aria-hidden': 'true' }, h('span', { 'class': 'm-pagination__ellipsis' }, '…'))
          : h('li', { 'data-page': '' }, link(x, String(x), L.page + x)));
      });
      ul.appendChild(h('li', { 'aria-hidden': 'true' }, h('span', { 'class': 'm-pagination__status' }, page + ' / ' + count)));
      ul.appendChild(h('li', null, link(page + 1, icon('chevron_right'), L.next, { arrow: true, tip: true })));
      root.appendChild(ul);
    }
    render();
    return root;
  }
  function Breadcrumb(p) {
    var L = isPl() ? ['Ścieżka', 'Pokaż pełną ścieżkę'] : ['Breadcrumb', 'Show full path'];
    var items = p.items, max = p.max || 4;
    var ol = h('ol', { 'class': 'm-breadcrumb__list' });
    var hidden = items.length > max ? items.slice(1, items.length - (max - 2)) : [];
    items.forEach(function (it, i) {
      var last = i === items.length - 1;
      var li = h('li', { 'class': 'm-breadcrumb__item', hidden: hidden.indexOf(it) >= 0 },
        last ? h('span', { 'class': 'm-breadcrumb__current', 'aria-current': 'page' }, it.label)
          : h('a', { 'class': 'm-breadcrumb__link', href: it.href || '#' }, it.icon ? icon(it.icon) : null, it.label),
        last ? null : icon('chevron_right', { 'class': 'm-breadcrumb__separator' }));
      ol.appendChild(li);
      if (i === 0 && hidden.length) {
        var more = h('button', { 'class': 'm-breadcrumb__more', type: 'button', 'aria-label': L[1], 'data-tip': L[1] }, icon('more_horiz'));
        var mli = h('li', { 'class': 'm-breadcrumb__item' }, more, icon('chevron_right', { 'class': 'm-breadcrumb__separator' }));
        more.addEventListener('click', function () { ol.querySelectorAll('.m-breadcrumb__item[hidden]').forEach(function (x) { x.hidden = false; }); mli.remove(); var f = ol.querySelectorAll('.m-breadcrumb__link')[1]; if (f) f.focus(); });
        ol.appendChild(mli);
      }
    });
    return h('nav', { 'class': 'm-breadcrumb__root', 'aria-label': p.label || L[0] }, ol);
  }
  function Steps(p) {
    var L = isPl() ? ['Ukończono', 'Bieżący krok', 'Do zrobienia'] : ['Completed', 'Current step', 'Not started'];
    var cur = p.current || 0;
    return h('div', { 'class': 'm-steps__root' }, h('ol', { 'class': ['m-steps__list', p.orientation === 'vertical' && 'm-steps__list--orientation_vertical'], 'aria-label': p.label || null },
      p.items.map(function (it, i) {
        var st = i < cur ? 'complete' : i === cur ? 'current' : 'upcoming';
        return h('li', { 'class': ['m-steps__step', p.orientation === 'vertical' && 'm-steps__step--orientation_vertical'], 'data-state': st, 'aria-current': st === 'current' ? 'step' : null },
          h('span', { 'class': 'm-steps__marker', 'aria-hidden': 'true' }, st === 'complete' ? icon('check') : String(i + 1)),
          h('span', { 'class': 'm-steps__text' },
            h('span', { 'class': 'm-steps__title' }, it.title, h('span', { 'class': 'm-visually-hidden' }, ', ' + L[['complete', 'current', 'upcoming'].indexOf(st)])),
            it.description ? h('span', { 'class': 'm-steps__description' }, it.description) : null));
      })));
  }

  /* ---------------------------------------------------------------- popover, hover card, drawer, toast */
  var FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), select, textarea, [tabindex]:not([tabindex="-1"])';
  function Popover(p) {
    var titleId = uid('pt');
    var L = isPl() ? 'Zamknij' : 'Close';
    var x = IconButton({ icon: 'close', label: L, size: 'sm' });
    var pop = newPopup('m-popover__root', { role: 'dialog', 'aria-labelledby': p.title ? titleId : null, 'aria-label': p.title ? null : p.label, style: p.width ? { '--popover-w': p.width + 'px' } : null });
    pop.appendChild(h('div', { 'class': 'm-popover__header' }, p.title ? h('h2', { 'class': 'm-popover__title', id: titleId }, p.title) : h('span', { style: { flex: '1' } }), x));
    pop.appendChild(h('div', { 'class': 'm-popover__body' }, p.content));
    if (p.footer) pop.appendChild(h('div', { 'class': 'm-popover__footer' }, p.footer));
    if (p.inline) { pop.setAttribute('data-inline', ''); pop.removeAttribute('popover'); }
    var trigger = p.trigger;
    trigger.setAttribute('aria-haspopup', 'dialog'); trigger.setAttribute('aria-expanded', 'false');
    var unOut = null;
    function open() {
      trigger.setAttribute('aria-expanded', 'true');
      openPopup(pop, trigger, { align: p.align });
      var f = pop.querySelector('.m-popover__body ' + FOCUSABLE.split(', ').join(', .m-popover__body ')) || x; f.focus();
      unOut = outside([pop, trigger], function () { close(false); });
    }
    function close(focus) { trigger.setAttribute('aria-expanded', 'false'); closePopup(pop); if (unOut) { unOut(); unOut = null; } if (focus) trigger.focus(); }
    trigger.addEventListener('click', function () { if (isOpen(pop)) close(true); else open(); });
    x.addEventListener('click', function () { close(true); });
    pop.addEventListener('keydown', function (e) { if (e.key === 'Escape') { e.preventDefault(); close(true); } });
    var wrap = h('span', { style: { position: 'relative', display: 'inline-block' }, 'data-m': 'popover' }, trigger, pop);
    wrap.open = open; wrap.close = close;
    if (p.inline && p.open) { trigger.setAttribute('aria-expanded', 'true'); pop.hidden = false; } else if (p.inline) pop.hidden = true;
    return wrap;
  }
  function CharacterPreview(c) {
    return h('div', null,
      h('div', { 'class': 'm-hovercard__head' },
        Avatar({ kind: 'character', name: c.name, src: c.portrait, size: 'lg', hue: c.hue, decorative: true }),
        h('div', null, h('span', { 'class': 'm-hovercard__name' }, c.name), c.epithet ? h('span', { 'class': 'm-hovercard__epithet' }, c.epithet) : null)),
      c.facts ? h('div', { 'class': 'm-hovercard__body' }, DataList({ items: c.facts, compact: true })) : null,
      c.account ? h('div', { 'class': 'm-hovercard__owner' }, Avatar({ kind: 'account', name: c.account, size: 'xs', decorative: true }), h('span', null, (isPl() ? 'Gra: @' : 'Played by @') + c.account)) : null);
  }
  /* A hover card previews; it is never the only way to the information, so it holds no controls.
     Pointer: opens after 500 ms, closes 250 ms after leaving. Keyboard focus opens it at once. */
  function HoverCard(p) {
    var card = newPopup('m-hovercard__root', { role: 'tooltip', id: uid('hc') }, null);
    card.appendChild(p.content);
    if (p.inline) { card.setAttribute('data-inline', ''); card.removeAttribute('popover'); card.hidden = !p.open; }
    var t = p.trigger, tOpen, tClose;
    t.setAttribute('aria-describedby', card.id);
    var coarse = window.matchMedia && matchMedia('(pointer: coarse)').matches;
    function open() { clearTimeout(tClose); if (!isOpen(card)) openPopup(card, t, { offset: 8 }); }
    function close() { clearTimeout(tOpen); closePopup(card); }
    if (!coarse) {
      t.addEventListener('pointerenter', function () { clearTimeout(tClose); tOpen = setTimeout(open, p.delay || 500); });
      t.addEventListener('pointerleave', function () { clearTimeout(tOpen); tClose = setTimeout(close, 250); });
      card.addEventListener('pointerenter', function () { clearTimeout(tClose); });
      card.addEventListener('pointerleave', function () { tClose = setTimeout(close, 250); });
    }
    t.addEventListener('focus', function () { if (t.matches(':focus-visible')) open(); });
    t.addEventListener('blur', close);
    t.addEventListener('keydown', function (e) { if (e.key === 'Escape') close(); });
    return h('span', { style: { position: 'relative', display: 'inline-block' }, 'data-m': 'hovercard' }, t, card);
  }
  function Drawer(p) {
    var titleId = uid('dt');
    var L = isPl() ? 'Zamknij' : 'Close';
    var x = IconButton({ icon: 'close', label: L });
    var d = h('dialog', { 'class': 'm-drawer__root', 'aria-labelledby': titleId },
      h('div', { 'class': 'm-drawer__header' }, h('h2', { 'class': 'm-drawer__title', id: titleId }, p.title), x),
      h('div', { 'class': 'm-drawer__body' }, p.content),
      p.footer ? h('div', { 'class': 'm-drawer__footer' }, p.footer) : null);
    x.addEventListener('click', function () { d.close(); });
    d.addEventListener('click', function (e) {
      if (e.target !== d) return;
      var r = d.getBoundingClientRect();
      if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) d.close();
    });
    /* `open` is the dialog's own state; openModal() attaches the drawer if needed and shows it modally */
    d.openModal = function () { if (!d.isConnected) (p.parent || document.body).appendChild(d); d.showModal(); };
    return d;
  }
  var region;
  function toast(o) {
    if (!region) { region = h('ol', { 'class': 'm-toast__region', 'aria-label': isPl() ? 'Powiadomienia' : 'Notifications', tabindex: '-1' }); document.body.appendChild(region); }
    var tone = o.tone || 'neutral';
    var ic = o.icon || { positive: 'check_circle', caution: 'warning', danger: 'error', info: 'info', neutral: null }[tone];
    var x = IconButton({ icon: 'close', label: isPl() ? 'Zamknij' : 'Dismiss', size: 'sm', tip: false });
    x.classList.add('m-toast__close');
    var act = o.action ? h('button', { 'class': 'm-toast__action', type: 'button' }, o.action.label) : null;
    var el = h('li', { 'class': 'm-toast__root', role: tone === 'danger' ? 'alert' : 'status', 'data-tone': tone },
      ic ? icon(ic, { 'class': 'm-toast__icon' }) : null, h('span', { 'class': 'm-toast__message' }, o.message), act, x);
    var timer, left = o.duration === 0 || o.action && o.duration == null ? 0 : (o.duration || 6000), started;
    function dismiss() {
      clearTimeout(timer);
      if (reduced()) { el.remove(); return; }
      el.setAttribute('data-leaving', ''); setTimeout(function () { el.remove(); }, 220);
    }
    function run() { if (left > 0) { started = Date.now(); timer = setTimeout(dismiss, left); } }
    function pause() { if (timer) { clearTimeout(timer); left -= Date.now() - started; timer = null; } }
    el.addEventListener('pointerenter', pause); el.addEventListener('pointerleave', run);
    el.addEventListener('focusin', pause); el.addEventListener('focusout', run);
    x.addEventListener('click', dismiss);
    if (act) act.addEventListener('click', function () { o.action.onClick && o.action.onClick(); dismiss(); });
    region.appendChild(el);
    while (region.children.length > 3) region.firstChild.remove();
    run();
    return { el: el, dismiss: dismiss };
  }

  /* ---------------------------------------------------------------- alert, progress, skeleton, data list */
  function Alert(p) {
    var tone = p.tone || 'info';
    var v = p.variant || 'inline';
    var cls = function (part) { return 'm-alert__' + part + ' m-alert__' + part + '--tone_' + tone + ' m-alert__' + part + '--variant_' + v; };
    var ic = p.icon || { info: 'info', caution: 'warning', danger: 'error', positive: 'check_circle', neutral: 'info' }[tone];
    var root = h('div', { 'class': cls('root'), role: p.live ? (tone === 'danger' ? 'alert' : 'status') : null },
      icon(ic, { 'class': cls('icon') }),
      h('div', { 'class': cls('content') },
        p.title ? h('p', { 'class': cls('title') }, p.title) : null,
        p.description ? h('div', { 'class': cls('description') }, p.description) : null,
        p.actions ? h('div', { 'class': cls('actions') }, p.actions) : null),
      p.onClose ? h('div', { 'class': cls('close') }, (function () { var b = IconButton({ icon: 'close', label: isPl() ? 'Zamknij' : 'Dismiss', size: 'sm' }); b.addEventListener('click', function () { p.onClose(root); }); return b; })()) : h('span'));
    return root;
  }
  function Progress(p) {
    var ind = p.value == null;
    var max = p.max || 100;
    var pct = ind ? null : Math.max(0, Math.min(100, p.value / max * 100));
    var labelId = uid('pg');
    var bar = h('div', { 'class': 'm-progress__track', role: 'progressbar', 'aria-labelledby': p.label && !p.bare ? labelId : null, 'aria-label': p.bare ? p.label : null,
      'aria-valuemin': ind ? null : 0, 'aria-valuemax': ind ? null : max, 'aria-valuenow': ind ? null : p.value, 'aria-valuetext': p.valueText || null },
      h('div', { 'class': 'm-progress__fill', style: ind ? null : { '--value': pct + '%' } }));
    return h('div', { 'class': 'm-progress__root', 'data-state': ind ? 'indeterminate' : 'determinate', 'data-tone': p.tone || null },
      p.bare ? null : h('div', { 'class': 'm-progress__header' },
        h('span', { 'class': 'm-progress__label', id: labelId }, p.label),
        h('span', { 'class': 'm-progress__value' }, p.valueText || (ind ? '' : Math.round(pct) + '%'))),
      bar);
  }
  function ring(p, ind) {
    var size = p.size || 40, stroke = p.stroke || (size <= 24 ? 3 : 4), r = 50 - stroke * 50 / size;
    var NS = 'http://www.w3.org/2000/svg';
    var svg = document.createElementNS(NS, 'svg'); svg.setAttribute('class', 'm-ring__svg'); svg.setAttribute('viewBox', '0 0 100 100'); svg.setAttribute('aria-hidden', 'true');
    [['m-ring__track', null], ['m-ring__arc', 1]].forEach(function (c) {
      var e = document.createElementNS(NS, 'circle'); e.setAttribute('class', c[0]); e.setAttribute('cx', '50'); e.setAttribute('cy', '50'); e.setAttribute('r', String(r));
      e.setAttribute('pathLength', '100'); e.style.strokeWidth = String(stroke * 100 / size); svg.appendChild(e);
    });
    var pct = ind ? 25 : Math.max(0, Math.min(100, p.value / (p.max || 100) * 100));
    return h('span', { 'class': 'm-ring__root', role: 'progressbar', 'aria-label': p.label, 'aria-valuemin': ind ? null : 0, 'aria-valuemax': ind ? null : 100, 'aria-valuenow': ind ? null : Math.round(pct),
      'data-state': ind ? 'indeterminate' : 'determinate', style: { '--ring-size': size + 'px', '--value': String(pct) } },
      svg, !ind && p.showValue ? h('span', { 'class': 'm-ring__value', 'aria-hidden': 'true' }, Math.round(pct) + '%') : null);
  }
  function ProgressRing(p) { return ring(p, p.value == null); }
  function Spinner(p) { p = p || {}; return ring({ size: p.size || 20, label: p.label || (isPl() ? 'Wczytywanie' : 'Loading') }, true); }
  function Skeleton(p) {
    p = p || {};
    return h('span', { 'class': 'm-skeleton m-skeleton--shape_' + (p.shape || 'line'), 'aria-hidden': 'true', style: { width: p.width || null, height: p.height || null } });
  }
  function PostSkeleton() {
    return h('div', { 'aria-hidden': 'true', style: { display: 'grid', 'grid-template-columns': '56px minmax(0, 1fr)', gap: '20px', 'padding-block': '12px' } },
      Skeleton({ shape: 'block', width: '56px', height: '56px' }),
      h('div', null, Skeleton({ width: '30%', height: '1em' }), Skeleton({ width: '22%', height: '0.8em' }),
        h('div', { style: { height: '14px' } }), Skeleton({ width: '96%' }), Skeleton({ width: '92%' }), Skeleton({ width: '64%' })));
  }
  function DataList(p) {
    var v = p.variant || 'plain';
    var cls = function (part) { return 'm-datalist__' + part + ' m-datalist__' + part + '--variant_' + v; };
    return h('dl', { 'class': cls('root'), style: p.labelWidth ? { '--label-w': p.labelWidth } : null },
      p.items.map(function (it) { return h('div', { 'class': cls('item') }, h('dt', { 'class': cls('label') }, it[0]), h('dd', { 'class': cls('value') }, it[1])); }));
  }

  /* ---------------------------------------------------------------- enhance server-rendered markup */
  var base = M.enhance;
  function enhance(root) {
    base(root);
    root = root || document;
    root.querySelectorAll('[data-m=textarea]:not([data-m-ready])').forEach(function (n) { n.setAttribute('data-m-ready', ''); wireTextarea(n); });
    root.querySelectorAll('[data-m=tree]:not([data-m-ready])').forEach(function (n) { n.setAttribute('data-m-ready', ''); wireTree(n); });
  }

  var api = {
    Field: Field, TextInput: TextInput, Textarea: Textarea, PasswordInput: PasswordInput, NumberInput: NumberInput,
    Checkbox: Checkbox, CheckboxGroup: CheckboxGroup, Fieldset: Fieldset, RadioGroup: RadioGroup,
    Select: Select, Combobox: Combobox, TagsInput: TagsInput, InlineSelect: InlineSelect, Calendar: Calendar, DatePicker: DatePicker, TimeField: TimeField,
    Slider: Slider, FileUpload: FileUpload, ColorPicker: ColorPicker,
    Accordion: Accordion, Collapsible: Collapsible, TreeView: TreeView, Pagination: Pagination, Breadcrumb: Breadcrumb, Steps: Steps,
    Popover: Popover, HoverCard: HoverCard, CharacterPreview: CharacterPreview, Drawer: Drawer, toast: toast,
    Alert: Alert, Progress: Progress, ProgressRing: ProgressRing, Spinner: Spinner, Skeleton: Skeleton, PostSkeleton: PostSkeleton, DataList: DataList,
    enhance: enhance, popup: { open: openPopup, close: closePopup, place: place }
  };
  Object.keys(api).forEach(function (k) { M[k] = api[k]; });
})();
