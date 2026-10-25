/* Demo content for the previews: ten sample Worlds with procedural artwork.
   The art is generated vector placeholder material, not production assets. Elarid and Oskad
   follow two games on Wieloświat (wieloswiat.pl) and Mgławica Mocy the archive of a Star Wars
   play-by-forum (archiwum.mglawicamocy.pl): their settings, places and section names, with
   invented characters and posts. Elarid's and Oskad's art is original; Mgławica Mocy uses the
   forum's own header image for review only. */
(function () {
  'use strict';
  var M = window.Motet;
  function rnd(seed) { var s = seed % 2147483647; if (s <= 0) s += 2147483646; return function () { s = (s * 16807) % 2147483647; return (s - 1) / 2147483646; }; }
  function uri(svg) { return 'data:image/svg+xml,' + encodeURIComponent(svg); }
  function svg(w, h, body, defs) {
    return "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 " + w + ' ' + h + "' preserveAspectRatio='xMidYMid slice'><defs>" + (defs || '') + '</defs>' + body + '</svg>';
  }
  function ridge(r, w, base, amp, step, rough) {
    var d = 'M0 ' + base, y = base;
    for (var x = 0; x <= w; x += step) { y = base + Math.sin(x / (w / 3.1) + r() * 0.3) * amp * 0.6 + (r() - 0.5) * amp * rough; d += ' L' + x + ' ' + y.toFixed(1); }
    return d + ' L' + w + ' 2000 L0 2000 Z';
  }
  var grainFilter = "<filter id='gr'><feTurbulence type='fractalNoise' baseFrequency='.8' numOctaves='2' seed='3'/><feColorMatrix values='0 0 0 0 .5 0 0 0 0 .5 0 0 0 0 .5 0 0 0 .09 0'/><feComposite in2='SourceGraphic' operator='in'/></filter>";

  var art = {};
  /* Salt Road: a cold lake between hills (Motet default appearance) */
  art.saltroad = (function () {
    var r = rnd(21), W = 1600, H = 900, b = '';
    b += "<rect width='1600' height='900' fill='url(#sky)'/>";
    b += "<circle cx='1130' cy='250' r='70' fill='#f3f6f4' opacity='.55'/>";
    ['#9fb4bf', '#7f98a6', '#5f7a8a', '#45606f'].forEach(function (c, i) { b += "<path d='" + ridge(r, W, 330 + i * 62, 70 - i * 10, 40, 0.4) + "' fill='" + c + "'/>"; });
    b += "<rect y='560' width='1600' height='340' fill='url(#lake)'/>";
    for (var i = 0; i < 14; i++) b += "<rect x='" + (r() * 1500).toFixed(0) + "' y='" + (590 + r() * 280).toFixed(0) + "' width='" + (60 + r() * 200).toFixed(0) + "' height='2' fill='#e8f0f2' opacity='" + (0.15 + r() * 0.2).toFixed(2) + "'/>";
    b += "<path d='" + ridge(r, W, 720, 30, 30, 0.6) + "' fill='#2a3c47'/>";
    b += "<rect width='1600' height='900' fill='url(#mist)'/>";
    return uri(svg(W, H, b, "<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='#c9d6dc'/><stop offset='.6' stop-color='#e6ecec'/></linearGradient><linearGradient id='lake' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='#6f8b98'/><stop offset='1' stop-color='#33495a'/></linearGradient><linearGradient id='mist' x1='0' y1='0' x2='0' y2='1'><stop offset='.35' stop-color='#fff' stop-opacity='0'/><stop offset='.62' stop-color='#eef3f4' stop-opacity='.55'/><stop offset='.75' stop-color='#fff' stop-opacity='0'/></linearGradient>"));
  })();
  /* Neon Meridian: a night skyline */
  art.neon = (function () {
    var r = rnd(7), W = 1600, H = 900, b = "<rect width='1600' height='900' fill='url(#sky)'/><ellipse cx='800' cy='640' rx='900' ry='180' fill='#e2408e' opacity='.28' filter='url(#blur)'/>";
    [[0.55, '#0b2433', 260], [0.75, '#081a26', 380], [1, '#040f17', 520]].forEach(function (L, li) {
      var x = -20;
      while (x < W) {
        var bw = 40 + r() * 110 * L[0], bh = L[2] * (0.4 + r() * 0.75), y = H - bh;
        b += "<rect x='" + x.toFixed(0) + "' y='" + y.toFixed(0) + "' width='" + bw.toFixed(0) + "' height='" + bh.toFixed(0) + "' fill='" + L[1] + "'/>";
        if (li > 0) for (var wy = y + 12; wy < H - 20; wy += 14) for (var wx = x + 6; wx < x + bw - 8; wx += 10) if (r() < 0.16) b += "<rect x='" + wx.toFixed(0) + "' y='" + wy.toFixed(0) + "' width='4' height='6' fill='" + (r() < 0.75 ? '#5fe6f2' : '#ff5fa8') + "' opacity='" + (0.35 + r() * 0.5).toFixed(2) + "'/>";
        if (li === 2 && r() < 0.25) b += "<rect x='" + (x + bw / 2 - 1).toFixed(0) + "' y='" + (y - 60).toFixed(0) + "' width='2' height='60' fill='#1c3a4a'/><circle cx='" + (x + bw / 2).toFixed(0) + "' cy='" + (y - 62).toFixed(0) + "' r='3' fill='#ff4f8f'/>";
        x += bw + 4 + r() * 18;
      }
    });
    b += "<rect y='700' width='1600' height='3' fill='#19c3d6' opacity='.5'/><rect y='760' width='1600' height='1' fill='#e2408e' opacity='.4'/>";
    return uri(svg(W, H, b, "<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='#02070c'/><stop offset='.7' stop-color='#0e2433'/><stop offset='1' stop-color='#2a1530'/></linearGradient><filter id='blur'><feGaussianBlur stdDeviation='60'/></filter>"));
  })();
  /* Ashfall: rust sky, ruined towers, dust */
  art.ashfall = (function () {
    var r = rnd(13), W = 1600, H = 900, b = "<rect width='1600' height='900' fill='url(#sky)'/><circle cx='420' cy='300' r='110' fill='#f2c58a' opacity='.6' filter='url(#blur)'/><circle cx='420' cy='300' r='58' fill='#f6d7a6' opacity='.85'/>";
    b += "<path d='" + ridge(r, W, 560, 50, 60, 0.5) + "' fill='#8a4b2c' opacity='.6'/>";
    for (var i = 0; i < 9; i++) {
      var x = 120 + i * 170 + r() * 60, w = 50 + r() * 70, top = 330 + r() * 180, d = 'M' + x + ' 900 L' + x + ' ' + top;
      for (var k = 0; k < 5; k++) d += ' L' + (x + (k + 1) * w / 5).toFixed(0) + ' ' + (top + (r() - 0.3) * 50).toFixed(0);
      b += "<path d='" + d + " L" + (x + w) + " 900 Z' fill='#3b2219' opacity='" + (0.55 + r() * 0.35).toFixed(2) + "'/>";
      for (var wy = top + 30; wy < 860; wy += 34) if (r() < 0.5) b += "<rect x='" + (x + 8 + r() * (w - 24)).toFixed(0) + "' y='" + wy.toFixed(0) + "' width='10' height='14' fill='#1e110c'/>";
    }
    b += "<path d='M1180 900 L1210 420 L1240 900 M1195 520 L1290 540 M1150 600 L1270 600' stroke='#2a1712' stroke-width='6' fill='none'/>";
    b += "<path d='" + ridge(r, W, 760, 40, 40, 0.8) + "' fill='#2b1812'/>";
    b += "<rect width='1600' height='900' filter='url(#gr)'/>";
    return uri(svg(W, H, b, "<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='#5a2a1c'/><stop offset='.55' stop-color='#b5603a'/><stop offset='1' stop-color='#d99a63'/></linearGradient><filter id='blur'><feGaussianBlur stdDeviation='40'/></filter>" + grainFilter));
  })();
  /* The Gilded Reach: dawn over mountains and a keep */
  art.vellum = (function () {
    var r = rnd(31), W = 1600, H = 900, b = "<rect width='1600' height='900' fill='url(#sky)'/>";
    for (var i = 0; i < 6; i++) b += "<ellipse cx='" + (r() * 1600).toFixed(0) + "' cy='" + (120 + r() * 220).toFixed(0) + "' rx='" + (160 + r() * 260).toFixed(0) + "' ry='" + (18 + r() * 26).toFixed(0) + "' fill='#fff4dc' opacity='" + (0.25 + r() * 0.3).toFixed(2) + "'/>";
    ['#9fb0c9', '#7a8fb0', '#5a6f96', '#3e527a'].forEach(function (c, i) { b += "<path d='" + ridge(r, W, 380 + i * 80, 140 - i * 25, 70, 1.1) + "' fill='" + c + "'/>"; });
    b += "<path d='M1040 620 L1040 470 L1060 470 L1060 450 L1075 450 L1075 470 L1100 470 L1100 410 L1112 395 L1124 410 L1124 470 L1150 470 L1150 455 L1165 455 L1165 470 L1185 470 L1185 620 Z' fill='#2c3a5c'/><rect x='1108' y='430' width='8' height='14' fill='#f7d27c'/>";
    b += "<path d='" + ridge(r, W, 700, 40, 50, 0.6) + "' fill='#24304d'/>";
    return uri(svg(W, H, b, "<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='#6d86b5'/><stop offset='.45' stop-color='#e9c78f'/><stop offset='.7' stop-color='#f6e2b9'/></linearGradient>"));
  })();
  /* Hollowmere: moon, fog, dead trees */
  art.hollow = (function () {
    var r = rnd(5), W = 1600, H = 900, b = "<rect width='1600' height='900' fill='url(#sky)'/><circle cx='1180' cy='210' r='150' fill='#cfd3d8' opacity='.18' filter='url(#blur)'/><circle cx='1180' cy='210' r='62' fill='#e4e2df' opacity='.9'/>";
    function branch(x, y, len, ang, wid, depth) {
      if (depth === 0 || len < 6) return '';
      var x2 = x + Math.cos(ang) * len, y2 = y + Math.sin(ang) * len;
      var s = "<path d='M" + x.toFixed(1) + ' ' + y.toFixed(1) + ' L' + x2.toFixed(1) + ' ' + y2.toFixed(1) + "' stroke='#0d0b0c' stroke-width='" + wid.toFixed(1) + "' stroke-linecap='round'/>";
      var n = 2 + (r() < 0.3 ? 1 : 0);
      for (var i = 0; i < n; i++) s += branch(x2, y2, len * (0.62 + r() * 0.18), ang + (r() - 0.5) * 1.1, wid * 0.66, depth - 1);
      return s;
    }
    b += "<path d='" + ridge(r, W, 640, 30, 50, 0.4) + "' fill='#1c1a1d'/>";
    b += "<path d='M300 640 L300 540 L340 500 L380 540 L380 640 Z M355 560 h10 v14 h-10z' fill='#141214'/><rect x='357' y='562' width='6' height='10' fill='#d8a14a' opacity='.9'/>";
    [[180, 120, 9], [560, 150, 9], [960, 110, 8], [1420, 160, 9], [760, 70, 7]].forEach(function (t) { b += branch(t[0], 900, t[1], -Math.PI / 2 + (r() - 0.5) * 0.2, 14, t[2]); });
    b += "<rect width='1600' height='900' fill='url(#fog)'/><rect width='1600' height='900' filter='url(#gr)'/>";
    return uri(svg(W, H, b, "<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='#0b0a0d'/><stop offset='.7' stop-color='#2c2a31'/><stop offset='1' stop-color='#3a3639'/></linearGradient><linearGradient id='fog' x1='0' y1='0' x2='0' y2='1'><stop offset='.5' stop-color='#8a8790' stop-opacity='0'/><stop offset='.8' stop-color='#9a96a0' stop-opacity='.35'/><stop offset='1' stop-color='#6e6a72' stop-opacity='.15'/></linearGradient><filter id='blur'><feGaussianBlur stdDeviation='50'/></filter>" + grainFilter));
  })();
  /* Cold Front: a contour map */
  art.ledger = (function () {
    var r = rnd(17), W = 1600, H = 900, b = "<rect width='1600' height='900' fill='#14243c'/>";
    for (var gx = 0; gx <= W; gx += 80) b += "<rect x='" + gx + "' width='1' height='900' fill='#2b4566' opacity='.6'/>";
    for (var gy = 0; gy <= H; gy += 80) b += "<rect y='" + gy + "' width='1600' height='1' fill='#2b4566' opacity='.6'/>";
    function blob(cx, cy, rad, n, jit) {
      var pts = [];
      for (var i = 0; i < n; i++) { var a = i / n * Math.PI * 2, rr = rad * (1 + (Math.sin(a * 3 + cx) * 0.18) + (r() - 0.5) * jit); pts.push([cx + Math.cos(a) * rr * 1.5, cy + Math.sin(a) * rr]); }
      var d = 'M' + pts[0][0].toFixed(1) + ' ' + pts[0][1].toFixed(1);
      for (var j = 0; j < n; j++) { var p0 = pts[j], p1 = pts[(j + 1) % n]; d += ' Q' + p0[0].toFixed(1) + ' ' + p0[1].toFixed(1) + ' ' + ((p0[0] + p1[0]) / 2).toFixed(1) + ' ' + ((p0[1] + p1[1]) / 2).toFixed(1); }
      return d + 'Z';
    }
    b += "<path d='" + blob(700, 470, 300, 22, 0.2) + "' fill='#2f4a3a'/>";
    for (var k = 1; k < 7; k++) b += "<path d='" + blob(700 + k * 12, 470 - k * 6, 300 - k * 42, 22, 0.12) + "' fill='none' stroke='#a8c49a' stroke-opacity='" + (0.25 + k * 0.06).toFixed(2) + "' stroke-width='1.2'/>";
    b += "<path d='" + blob(1300, 240, 120, 16, 0.25) + "' fill='#2f4a3a'/><path d='" + blob(1310, 236, 70, 16, 0.2) + "' fill='none' stroke='#a8c49a' stroke-opacity='.4'/>";
    b += "<path d='M420 300 C 560 360, 640 330, 760 420 S 980 560, 1100 520' stroke='#e0662b' stroke-width='3' fill='none' stroke-dasharray='10 8'/><path d='M1100 520 l-18 -4 l8 16z' fill='#e0662b'/>";
    b += "<circle cx='420' cy='300' r='7' fill='none' stroke='#f2f4f7' stroke-width='2'/><circle cx='1100' cy='520' r='7' fill='#f2f4f7'/>";
    return uri(svg(W, H, b));
  })();
  /* Kepler Relay: a gas giant's limb seen from a relay station, drawn as an instrument would */
  art.kepler = (function () {
    var r = rnd(41), W = 1600, H = 900, b = "<rect width='1600' height='900' fill='#0b0805'/>";
    for (var i = 0; i < 160; i++) b += "<rect x='" + (r() * 1600).toFixed(0) + "' y='" + (r() * 520).toFixed(0) + "' width='2' height='2' fill='#f0c27a' opacity='" + (0.15 + r() * 0.5).toFixed(2) + "'/>";
    b += "<circle cx='1120' cy='1480' r='1020' fill='url(#limb)'/>";
    b += "<circle cx='1120' cy='1480' r='1020' fill='none' stroke='#f0a33a' stroke-width='2' opacity='.8'/>";
    for (var k = 0; k < 7; k++) b += "<ellipse cx='1120' cy='1480' rx='" + (1020 - k * 6) + "' ry='" + (1020 - k * 90) + "' fill='none' stroke='#f0a33a' stroke-opacity='" + (0.12 - k * 0.012).toFixed(3) + "' stroke-width='1.5'/>";
    b += "<ellipse cx='900' cy='560' rx='760' ry='120' fill='none' stroke='#f0a33a' stroke-opacity='.45' stroke-width='1.5' stroke-dasharray='2 10' transform='rotate(-8 900 560)'/>";
    b += "<g transform='translate(430 440) rotate(-8)' fill='#f0a33a'><rect x='-46' y='-3' width='92' height='6'/><rect x='-10' y='-14' width='20' height='28'/><rect x='-78' y='-10' width='26' height='20' opacity='.6'/><rect x='52' y='-10' width='26' height='20' opacity='.6'/></g>";
    b += "<path d='M340 440h-60v-40' stroke='#f0a33a' stroke-opacity='.6' fill='none'/><rect x='210' y='372' width='140' height='22' fill='none' stroke='#f0a33a' stroke-opacity='.6'/>";
    for (var t = 0; t <= 1600; t += 40) b += "<rect x='" + t + "' y='" + (t % 200 === 0 ? 860 : 872) + "' width='1' height='" + (t % 200 === 0 ? 40 : 28) + "' fill='#f0a33a' opacity='.35'/>";
    b += "<rect width='1600' height='900' fill='url(#scan)'/>";
    return uri(svg(W, H, b, "<radialGradient id='limb' cx='.45' cy='.08' r='.7'><stop offset='0' stop-color='#6b4012'/><stop offset='.45' stop-color='#2a1909'/><stop offset='1' stop-color='#0b0805'/></radialGradient><pattern id='scan' width='4' height='4' patternUnits='userSpaceOnUse'><rect width='4' height='1' fill='#000' opacity='.28'/></pattern>"));
  })();

  /* Elarid: a night of magic over Verden: aurora, a citadel on a crag, a pine-dark valley */
  art.elarid = (function () {
    var r = rnd(57), W = 1600, H = 900, b = "<rect width='1600' height='900' fill='url(#sky)'/>";
    for (var i = 0; i < 170; i++) b += "<circle cx='" + (r() * 1600).toFixed(0) + "' cy='" + (r() * 460).toFixed(0) + "' r='" + (0.6 + r() * 1.3).toFixed(1) + "' fill='#e8f0ff' opacity='" + (0.25 + r() * 0.6).toFixed(2) + "'/>";
    b += "<g filter='url(#soft)' fill='none' stroke-linecap='round'>" +
      "<path d='M-80 250 C 260 120, 520 330, 820 200 S 1360 120, 1700 230' stroke='#3fd6a0' stroke-width='90' opacity='.32'/>" +
      "<path d='M-80 320 C 300 230, 600 380, 900 290 S 1400 230, 1700 330' stroke='#7b6cff' stroke-width='70' opacity='.26'/>" +
      "<path d='M200 170 C 480 90, 760 240, 1040 150' stroke='#9ff2d0' stroke-width='36' opacity='.22'/></g>";
    b += "<circle cx='1260' cy='190' r='150' fill='#cfe3ff' opacity='.16' filter='url(#soft)'/><circle cx='1260' cy='190' r='64' fill='#eef4ff' opacity='.92'/><circle cx='1238' cy='176' r='12' fill='#d6e2f5' opacity='.6'/>";
    ['#2b3f63', '#21324f', '#18253c'].forEach(function (c, i) { b += "<path d='" + ridge(r, W, 470 + i * 70, 120 - i * 25, 60, 1) + "' fill='" + c + "'/>"; });
    /* the crag and the citadel */
    b += "<path d='M300 900 L330 640 L370 590 L420 560 L470 575 L520 540 L560 600 L590 700 L620 900 Z' fill='#101a2c'/>";
    b += "<path d='M380 565 L380 470 L396 470 L396 452 L410 452 L410 470 L430 470 L430 390 L441 372 L452 390 L452 470 L470 470 L470 430 L480 418 L490 430 L490 470 L512 470 L512 452 L526 452 L526 470 L540 470 L540 560 Z' fill='#0c1424'/>";
    b += "<path d='M441 372 L441 120' stroke='#7ff0c6' stroke-width='3' opacity='.55' filter='url(#soft)'/><path d='M441 372 L441 160' stroke='#c8fff0' stroke-width='1.2' opacity='.8'/>";
    [[436, 410], [446, 432], [400, 488], [520, 492], [478, 452]].forEach(function (w) { b += "<rect x='" + w[0] + "' y='" + w[1] + "' width='5' height='9' fill='#ffd27a' opacity='.9'/>"; });
    /* pines */
    var pines = '';
    for (var x = -20; x < 1640; x += 14 + r() * 22) { var ph = 40 + r() * 70, base = 780 + r() * 40; pines += 'M' + x.toFixed(0) + ' ' + base.toFixed(0) + ' L' + (x + ph * 0.22).toFixed(0) + ' ' + (base - ph).toFixed(0) + ' L' + (x + ph * 0.44).toFixed(0) + ' ' + base.toFixed(0) + 'Z '; }
    b += "<path d='" + pines + "' fill='#0a111e'/><rect y='820' width='1600' height='80' fill='#0a111e'/>";
    b += "<rect width='1600' height='900' fill='url(#mist)'/>";
    return uri(svg(W, H, b, "<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='#070b1f'/><stop offset='.5' stop-color='#17244a'/><stop offset='.8' stop-color='#2f5566'/></linearGradient><linearGradient id='mist' x1='0' y1='0' x2='0' y2='1'><stop offset='.55' stop-color='#7fd8c0' stop-opacity='0'/><stop offset='.75' stop-color='#7fd8c0' stop-opacity='.12'/><stop offset='.9' stop-color='#7fd8c0' stop-opacity='0'/></linearGradient><filter id='soft' x='-20%' y='-50%' width='140%' height='200%'><feGaussianBlur stdDeviation='28'/></filter>"));
  })();
  /* Oskad: evening on the Great Plains: mesas, the rail line and the telegraph */
  art.oskad = (function () {
    var r = rnd(64), W = 1600, H = 900, b = "<rect width='1600' height='900' fill='url(#sky)'/>";
    b += "<circle cx='1130' cy='520' r='190' fill='#ffd79a' opacity='.45' filter='url(#blur)'/><circle cx='1130' cy='520' r='92' fill='#ffe6b5'/>";
    for (var i = 0; i < 5; i++) b += "<ellipse cx='" + (r() * 1600).toFixed(0) + "' cy='" + (110 + r() * 220).toFixed(0) + "' rx='" + (180 + r() * 240).toFixed(0) + "' ry='" + (10 + r() * 16).toFixed(0) + "' fill='#ffd2a6' opacity='" + (0.25 + r() * 0.25).toFixed(2) + "'/>";
    function mesa(x0, w, top, base, fill) {
      var d = 'M' + x0 + ' ' + base + ' L' + (x0 + w * 0.12).toFixed(0) + ' ' + (top + 30) + ' L' + (x0 + w * 0.2).toFixed(0) + ' ' + top;
      for (var k = 1; k <= 5; k++) d += ' L' + (x0 + w * (0.2 + k * 0.12)).toFixed(0) + ' ' + (top + (r() - 0.5) * 10).toFixed(0);
      d += ' L' + (x0 + w * 0.86).toFixed(0) + ' ' + (top + 26) + ' L' + (x0 + w) + ' ' + base + ' Z';
      return "<path d='" + d + "' fill='" + fill + "'/>";
    }
    b += mesa(-40, 520, 430, 680, '#b86a4b') + mesa(1280, 420, 470, 680, '#b86a4b');
    b += mesa(160, 380, 520, 690, '#8f4632') + mesa(820, 300, 560, 690, '#8f4632') + mesa(1440, 260, 540, 690, '#8f4632');
    b += "<rect y='660' width='1600' height='240' fill='url(#plain)'/>";
    for (var g = 0; g < 90; g++) { var gx = r() * 1600, gy = 700 + r() * 190; b += "<path d='M" + gx.toFixed(0) + ' ' + gy.toFixed(0) + ' l' + (2 + r() * 4).toFixed(1) + ' -' + (6 + r() * 10).toFixed(0) + "' stroke='#5b3a1e' stroke-width='1.4' opacity='.5'/>"; }
    /* rail line with sleepers, the telegraph beside it */
    b += "<rect y='742' width='1600' height='5' fill='#3a2617'/><rect y='754' width='1600' height='4' fill='#3a2617'/>";
    for (var t = 0; t < 1600; t += 18) b += "<rect x='" + t + "' y='739' width='8' height='22' fill='#4a3020' opacity='.75'/>";
    var tops = [];
    for (var px = 40; px < 1640; px += 210) { tops.push(px); b += "<rect x='" + px + "' y='600' width='5' height='140' fill='#3a2617'/><rect x='" + (px - 16) + "' y='610' width='37' height='4' fill='#3a2617'/>"; }
    for (var j = 0; j < tops.length - 1; j++) { var a = tops[j], c = tops[j + 1]; b += "<path d='M" + (a - 14) + " 612 Q " + ((a + c) / 2) + " 640 " + (c - 14) + " 612 M" + (a + 18) + " 612 Q " + ((a + c) / 2 + 32) + " 638 " + (c + 18) + " 612' stroke='#3a2617' stroke-width='1.2' fill='none'/>"; }
    /* a locomotive heading for Dodge City */
    b += "<g fill='#2a1a10'><rect x='1180' y='690' width='120' height='44'/><rect x='1260' y='664' width='44' height='70'/><rect x='1200' y='668' width='16' height='24'/><path d='M1166 734 L1180 704 L1180 734 Z'/><rect x='1304' y='700' width='90' height='34'/><rect x='1398' y='700' width='90' height='34'/></g>";
    [1196, 1232, 1272, 1316, 1360, 1410, 1456].forEach(function (cx) { b += "<circle cx='" + cx + "' cy='738' r='" + (cx < 1290 ? 13 : 9) + "' fill='#1d120b'/>"; });
    b += "<g fill='#f4e3c8'><ellipse cx='1208' cy='648' rx='22' ry='14' opacity='.75'/><ellipse cx='1180' cy='620' rx='34' ry='20' opacity='.55'/><ellipse cx='1130' cy='592' rx='50' ry='26' opacity='.35'/><ellipse cx='1060' cy='570' rx='70' ry='30' opacity='.2'/></g>";
    b += "<rect width='1600' height='900' filter='url(#gr)'/>";
    return uri(svg(W, H, b, "<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='#5d4f86'/><stop offset='.45' stop-color='#d9805a'/><stop offset='.72' stop-color='#f6cf92'/></linearGradient><linearGradient id='plain' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='#c69457'/><stop offset='1' stop-color='#7d5330'/></linearGradient><filter id='blur'><feGaussianBlur stdDeviation='50'/></filter>" + grainFilter));
  })();
  /* Mgławica Mocy: the forum's own header art (styles/nebulae, bg-high.jpg), included only to
     review this sample with the art's author. Replace it before publishing anything. It sits next
     to this script, so its address is resolved from the script's own. */
  /* Motet: the art is not in the repository (see UPSTREAM.md), so this World has no art here. */

  /* Portraits: silhouettes on a tinted ground; variant changes the outline */
  function portrait(hue, variant, light) {
    var bg1 = 'oklch(' + (light ? 0.86 : 0.55) + ' 0.07 ' + hue + ')', bg2 = 'oklch(' + (light ? 0.7 : 0.32) + ' 0.08 ' + hue + ')';
    var fig = 'oklch(0.2 0.03 ' + hue + ')', rim = 'oklch(0.85 0.06 ' + hue + ')';
    var head = "<ellipse cx='60' cy='58' rx='19' ry='23'/>";
    var body = "<path d='M18 160 C 20 118, 38 100, 60 100 C 82 100, 100 118, 102 160 Z'/><rect x='51' y='76' width='18' height='28' rx='6'/>";
    var extra = {
      hood: "<path d='M33 66 C 30 30, 46 22, 60 22 C 74 22, 90 30, 87 66 C 92 92, 98 104, 104 160 L16 160 C 22 104, 28 92, 33 66 Z'/>",
      long: "<path d='M38 60 C 36 34, 48 30, 60 30 C 74 30, 86 36, 83 62 C 86 86, 92 98, 88 112 L32 112 C 28 98, 34 86, 38 60 Z'/>",
      visor: "<path d='M39 48 h42 v14 h-42z' fill='" + rim + "' opacity='.85'/><path d='M36 40 C 40 24, 80 24, 84 40 L84 50 L36 50 Z'/>",
      hat: "<ellipse cx='60' cy='40' rx='40' ry='7'/><path d='M44 40 C 46 20, 74 20, 76 40 Z'/>",
      short: "<path d='M41 50 C 40 34, 50 30, 60 30 C 72 30, 80 36, 79 52 C 74 44, 52 42, 41 50 Z'/>"
    }[variant] || '';
    var inner = "<rect width='120' height='160' fill='url(#g)'/><g fill='" + fig + "' stroke='" + rim + "' stroke-opacity='.35' stroke-width='1.2'>" + body + head + extra + '</g>';
    return uri("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 120 160' preserveAspectRatio='xMidYMid slice'><defs><radialGradient id='g' cx='.35' cy='.25' r='1'><stop offset='0' stop-color='" + bg1 + "'/><stop offset='1' stop-color='" + bg2 + "'/></radialGradient></defs>" + inner + '</svg>');
  }
  /* a five-pointed star, for Oskad's lawmen */
  function starPath(cx, cy, R, rr) {
    var d = '';
    for (var i = 0; i < 10; i++) { var a = -Math.PI / 2 + i * Math.PI / 5, q = i % 2 ? rr : R; d += (i ? 'L' : 'M') + (cx + q * Math.cos(a)).toFixed(2) + ' ' + (cy + q * Math.sin(a)).toFixed(2); }
    return d + 'Z';
  }
  /* World marks: simple emblems */
  function mark(kind, a, b) {
    var g = {
      saltroad: "<path d='M14 40c6-5 12-5 18 0s12 5 18 0' stroke='" + a + "' stroke-width='4' fill='none' stroke-linecap='round'/><path d='M14 30c6-5 12-5 18 0s12 5 18 0' stroke='" + a + "' stroke-width='4' fill='none' stroke-linecap='round' opacity='.6'/><circle cx='42' cy='18' r='5' fill='" + a + "'/>",
      neon: "<path d='M32 10l18 10v24L32 54 14 44V20z' fill='none' stroke='" + a + "' stroke-width='3.5'/><path d='M32 22v20M24 27l8 5 8-5' stroke='" + b + "' stroke-width='3' fill='none'/>",
      ashfall: "<path d='M32 10l22 40H10z' fill='none' stroke='" + a + "' stroke-width='4' stroke-linejoin='round'/><path d='M32 26v12' stroke='" + a + "' stroke-width='4'/><circle cx='32' cy='44' r='2.6' fill='" + a + "'/>",
      vellum: "<path d='M14 42l4-20 9 10 5-16 5 16 9-10 4 20z' fill='" + a + "'/><rect x='14' y='45' width='36' height='5' rx='1.5' fill='" + a + "'/>",
      hollow: "<circle cx='30' cy='33' r='19' fill='" + a + "'/><circle cx='39' cy='27' r='16' fill='" + b + "'/><circle cx='47' cy='44' r='2' fill='" + a + "'/>",
      ledger: "<circle cx='32' cy='32' r='19' fill='none' stroke='" + a + "' stroke-width='3'/><path d='M32 13v38M13 32h38' stroke='" + a + "' stroke-width='2'/><path d='M32 20l5 12-5 12-5-12z' fill='" + b + "'/>",
      kepler: "<circle cx='32' cy='32' r='9' fill='" + a + "'/><ellipse cx='32' cy='32' rx='22' ry='9' fill='none' stroke='" + a + "' stroke-width='3' transform='rotate(-20 32 32)'/><rect x='48' y='12' width='5' height='5' fill='" + a + "'/>",
      elarid: "<path d='M32 8l12 16-12 32-12-32z' fill='none' stroke='" + a + "' stroke-width='3.5' stroke-linejoin='round'/><path d='M20 24h24M32 8v48' stroke='" + a + "' stroke-width='2' opacity='.7'/><circle cx='32' cy='24' r='4' fill='" + a + "'/>",
      oskad: "<circle cx='32' cy='32' r='22' fill='none' stroke='" + a + "' stroke-width='3'/><path d='" + starPath(32, 32, 16, 6.6) + "' fill='" + a + "'/>",
      mglawica: "<path d='M32 14a18 18 0 1 1-17 12' fill='none' stroke='" + a + "' stroke-width='3.5' stroke-linecap='round'/><path d='M32 22a10 10 0 1 0 9 6' fill='none' stroke='" + a + "' stroke-width='3' stroke-linecap='round' opacity='.75'/><circle cx='32' cy='32' r='3.5' fill='" + a + "'/>"
    }[kind];
    return uri("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64'><rect width='64' height='64' fill='" + b + "'/>" + g.replace(new RegExp(b, 'g'), b) + '</svg>');
  }

  var P = M.presets;
  var worlds = [
    {
      key: 'saltroad', record: Object.assign({}, M.defaultRecord, { id: 'saltroad', name: 'The Salt Road', imagery: { treatments: { veil: 45 }, focus: [50, 55] } }),
      name: 'The Salt Road', gms: ['Marta K.'], state: 'Recruiting', language: 'English', hue: 210, markColors: ['#e8f1f5', '#2b6b8a'],
      scene: { title: 'Thaw at the Ferry', location: 'Velka Crossing', arc: 'The Long Winter', ic: 'In character' },
      cast: [
        { name: 'Ilse Varga', epithet: 'Ferry-keeper’s daughter', hue: 205, variant: 'long', account: 'lena.bsky.social', display: 'Lena' },
        { name: 'Tomas Hrad', epithet: 'Salt merchant', hue: 35, variant: 'hat', account: 'kuba.dev', display: 'Kuba' }
      ],
      posts: [
        '<p>The ice had begun to talk in the night. Ilse heard it from the loft, the long groaning creak that meant the river was thinking about moving again, and by morning there was a black seam of water down the middle of the crossing.</p><p>“Nobody goes over today,” she said, before the merchant could open his mouth. “Not you, not your mules, not your salt.”</p>',
        '<p>Tomas set his cup down very carefully, the way a man does when he has decided not to argue yet.</p><p>“Your father said the thaw was a week off.” He looked past her at the seam of water. “Your father is not here, I suppose.”</p>'
      ],
      roll: { label: 'Read the ice', expr: '2d6+1', results: [4, 5], total: 10, outcome: 'Success: the safe line holds near the east bank' }
    },
    {
      key: 'neon', record: Object.assign({}, P.neon), name: 'Neon Meridian', gms: ['vex', 'Ada R.'], state: 'Full', language: 'English', hue: 190, markColors: ['#19c3d6', '#071923'],
      scene: { title: 'Deep Line 7', location: 'Meridian Arcology, level 40', arc: 'Blackwater Contract' },
      cast: [
        { name: 'Kestrel', epithet: 'Netrunner, unlicensed', hue: 190, variant: 'visor', account: 'ola.codes', display: 'Ola' },
        { name: 'Dr. Imre Sato', epithet: 'Corporate liaison', hue: 330, variant: 'short', account: 'pawel.social', display: 'Paweł' }
      ],
      posts: [
        '<p>The maglev doors sighed open on a platform that was not on any public map. Kestrel kept her hands in her jacket and counted cameras: four, one of them dead, one of them pretending to be.</p><p>“You said the drop would be clean,” she subvocalised.</p>',
        '<p>Sato did not look up from his slate. “I said it would be quiet. Clean costs extra.” He turned the screen toward her; the contract had grown a clause since this morning.</p>'
      ],
      roll: { label: 'Slice the door', expr: '3d10 keep 2', results: [9, 3, 7], dropped: [1], total: 16, outcome: 'Hit: the lock opens without an alarm' }
    },
    {
      key: 'ashfall', scenes: ['Targ w Osadzie', 'Stara zajezdnia'], record: Object.assign({}, P.ashfall), name: 'Ashfall', gms: ['Rdza'], state: 'Recruiting', language: 'Polski', hue: 25, markColors: ['#e9b07a', '#3a1d14'],
      scene: { title: 'Wieża ciśnień', location: 'Strefa Żółta, sektor 3', arc: 'Susza' },
      cast: [
        { name: 'Wiktor „Kret” Zając', epithet: 'Zwiadowca', hue: 30, variant: 'hood', account: 'michal.pds.motet.at', display: 'Michał' },
        { name: 'Hanka', epithet: 'Mechaniczka z Osady', hue: 80, variant: 'short', account: 'zosia.bsky.social', display: 'Zosia' }
      ],
      posts: [
        '<p>Wiatr niósł piach poziomo, jakby ktoś sypał go z worka prosto w twarz. Kret przykucnął za zardzewiałym zbiornikiem i policzył: trzy sylwetki przy pompie, jedna na drabinie wieży.</p><p>— Jeśli to Szakale, mamy dwie minuty, zanim nas zwęszą — mruknął.</p>',
        '<p>Hanka nie odpowiedziała od razu. Dokręciła ostatnią śrubę w masce i dopiero wtedy spojrzała na drabinę.</p><p>— To nie Szakale. Szakale nie naprawiają pomp.</p>'
      ],
      roll: { label: 'Skradanie', expr: '1d20+4', results: [13], total: 17, outcome: 'Sukces: nikt was nie zauważył' }
    },
    {
      key: 'vellum', record: Object.assign({}, P.vellum), name: 'The Gilded Reach', gms: ['Aurelia', 'Bram'], state: 'Recruiting', language: 'English', hue: 45, markColors: ['#c9973a', '#1f2f55'],
      scene: { title: 'The Feast of Lanterns', location: 'Harrowgate Keep, the great hall', arc: 'A Crown of Ash' },
      cast: [
        { name: 'Lady Maren of Ost', epithet: 'Envoy of the Northern March', hue: 260, variant: 'long', account: 'iga.bsky.social', display: 'Iga' },
        { name: 'Brother Cael', epithet: 'Keeper of the Archive', hue: 45, variant: 'hood', account: 'tom.writes', display: 'Tom' }
      ],
      posts: [
        '<p>Lanterns hung from every beam of the great hall, a hundred small suns swaying in the draught from the high windows. Maren had counted the banners twice now, and twice she had come up one short.</p><p>The Duke’s steward was smiling at her. She had learned, in three winters at court, to fear that smile more than any blade.</p>',
        '<p>Cael found her by the hearth, a cup of spiced wine he did not intend to drink held like an excuse. “The Corven banner,” he said quietly, before she could ask. “It came down this morning. No one will say by whose order.”</p>'
      ],
      roll: { label: 'Insight', expr: '1d20+5', results: [16], total: 21, outcome: 'The steward is lying about the banner' }
    },
    {
      key: 'hollow', scenes: ['Pusty grób', 'Plebania nocą'], record: Object.assign({}, P.hollow), name: 'Hollowmere', gms: ['Wrona'], state: 'Invitation only', stateIcon: 'lock', language: 'Polski', hue: 355, markColors: ['#d9d4cf', '#1a1214'],
      scene: { title: 'Dzwon na bagnach', location: 'Plebania, Hollowmere', arc: 'Ci, którzy śpią' },
      cast: [
        { name: 'Ksiądz Ambroży', epithet: 'Proboszcz bez parafii', hue: 0, variant: 'hat', account: 'jan.k.bsky.social', display: 'Jan' },
        { name: 'Ida Wilk', epithet: 'Córka grabarza', hue: 280, variant: 'long', account: 'nina.motet.at', display: 'Nina' }
      ],
      posts: [
        '<p>Dzwon odezwał się o trzeciej w nocy, choć wieży nie było już od czterdziestu lat. Ambroży usiadł na łóżku i słuchał, aż dźwięk utonął gdzieś za bagnami, a potem zapalił świecę drżącymi palcami.</p><p>Na progu ktoś zostawił mokre ślady. Bose. Prowadziły do środka i nigdzie nie wychodziły.</p>',
        '<p>Ida stała przy oknie plebanii od kwadransa, zanim zdecydowała się zapukać. — Proszę księdza — szepnęła przez szybę. — Ojciec mówi, że dziś w nocy jeden z grobów jest pusty.</p>'
      ],
      roll: { label: 'Opanowanie', expr: '2d6', results: [2, 3], total: 5, outcome: 'Porażka: Ambroży upuszcza świecę', outcomeIcon: 'error', outcomeTone: 'danger' }
    },
    {
      key: 'ledger', record: Object.assign({}, P.ledger), name: 'Cold Front', gms: ['Desk Officer'], state: 'Turn 14 of 40', stateIcon: 'event', language: 'English', hue: 220, markColors: ['#f2f4f7', '#1f3f73'],
      scene: { title: 'Baltic Corridor: Turn 14', location: 'Operations room', arc: 'Winter 1987' },
      cast: [
        { name: 'Minister Halvorsen', epithet: 'Norway, Ministry of Defence', hue: 220, variant: 'short', account: 'erik.games', display: 'Erik' },
        { name: 'Gen. Kowalczyk', epithet: 'Poland, General Staff', hue: 0, variant: 'hat', account: 'marek.pds.example', display: 'Marek' }
      ],
      posts: [
        '<p>Order 14-3: the 2nd Mechanised Brigade holds at Gdańsk. Two coastal batteries redeploy north of Kołobrzeg by end of turn. We are requesting allied air cover for the corridor between 06:00 and 14:00.</p><ul><li>Fuel reserve: 61%</li><li>Readiness: amber</li></ul>',
        '<p>Norway concurs on air cover. We can commit one squadron from Ørland for four hours, not eight. In exchange we ask for the minesweeping report on sector B before turn 15 resolves.</p>'
      ],
      roll: { label: 'Weather over the corridor', expr: '1d20', results: [6], total: 6, outcome: 'Low cloud after 11:00: air support halved', outcomeIcon: 'warning', outcomeTone: 'caution' }
    },
    {
      key: 'kepler', record: Object.assign({}, P.kepler), name: 'Kepler Relay', gms: ['ctrl'], state: 'Cycle 3', stateIcon: 'event', language: 'English', hue: 70, markColors: ['#f0a33a', '#140d05'],
      scene: { title: 'Silent Hours', location: 'Comms deck, ring B', arc: 'Signal KR-9' },
      cast: [
        { name: 'Oyelaran', epithet: 'Comms officer, night shift', hue: 70, variant: 'short', account: 'dami.bsky.social', display: 'Dami' },
        { name: 'Horvat', epithet: 'Chief engineer', hue: 30, variant: 'visor', account: 'vesna.pds.example', display: 'Vesna' }
      ],
      posts: [
        '<p>0312. The carrier came back on the second band, eleven seconds after it dropped. Same burst length, same gap. Oyelaran logged it by hand, the way the manual says to when the automatics disagree with you.</p><p>“Ring B to engineering. Is anyone else hearing this?”</p>',
        '<p>Horvat took her time answering. When she did, it was from the crawlway behind the dish motors, and the channel was full of the sound of her breathing.</p><p>“I’m hearing it in the hull,” she said. “Not on the speakers.”</p>'
      ],
      roll: { label: 'Keep your nerve', expr: '2d6', results: [3, 5], total: 8, outcome: 'Partial: you hold, but drop the log', outcomeIcon: 'warning', outcomeTone: 'caution' }
    },
    {
      /* Classic fantasy in a world full of magic, where enchanted weapons take the place of firearms */
      key: 'elarid', scenes: ['Leśny Trakt', 'Krwawe Wybrzeże'], name: 'Elarid', gms: ['Kronikarz'], state: 'Recruiting', language: 'Polski', hue: 160, markColors: ['#e7c27a', '#123327'],
      record: {
        id: 'elarid', name: 'Elarid',
        palette: { primary: '#2f9e74', secondary: '#c9953f', scheme: 'expressive', chroma: 120, temperature: 20, depth: { dark: -1, light: 0 }, mode: 'dark' },
        material: { finish: 'satin', grain: 2, tint: 4, edge: 1, shadow: 2 },
        shape: { corner: 'bevel', scale: 70, border: 'rule', profile: 'cut', tone: 'ornament' },
        ornament: { set: 'filigree', level: 2, edge: 'arch' },
        type: { display: 'source-serif', 'case': 'upper', tracking: 70, weight: 700, names: 'display', reading: 'serif', numerals: 'oldstyle' },
        icons: { style: 'rounded', weight: 400 },
        motion: { character: 'soft' },
        density: 'comfortable',
        composition: { post: 'card', portrait: 'gutter', names: 'plain' },
        imagery: { placement: 'continuous', treatments: { vignette: 60, parallax: 40 }, focus: [50, 52], title: 'art' }
      },
      scene: { title: 'Brama Miasta Ur', location: 'Miasto Ur, Verden', arc: 'Cień nad Verden' },
      cast: [
        { name: 'Ewalda Rhun', epithet: 'Magini bitewna z Nirgaldu', hue: 280, variant: 'hood', account: 'agnieszka.bsky.social', display: 'Agnieszka' },
        { name: 'Torvik Żelazny Kieł', epithet: 'Najemnik z Karak’Akes', hue: 30, variant: 'short', account: 'tomek-k.bsky.social', display: 'Tomek' }
      ],
      posts: [
        '<p>Brama Ur zamknęła się za nimi z hukiem, który poniósł się echem po pustym placu. Ewalda strząsnęła z płaszcza piasek Nirgaldu i dopiero wtedy zauważyła, że runy na jej kosturze wciąż się żarzą.</p><p>— Ktoś tu niedawno rzucał czary — powiedziała cicho. — I nie był to nikt z Gildii.</p>',
        '<p>Torvik splunął pod nogi i oparł topór o ramię. W Karak’Akes mówiono, że magia zostawia po sobie zapach burzy; tu pachniało tylko spalenizną i strachem.</p><p>— To czemu straż uciekła z murów, zamiast nas zatrzymać? — mruknął. — Strażnicy nie uciekają przed magami. Uciekają przed tym, co magowie przywołali.</p>'
      ],
      ooc: 'Odpiszę jutro wieczorem, dziś gram jeszcze w Hammer.',
      roll: { label: 'Wyczucie magii', expr: '1d20+3', results: [15], total: 18, outcome: 'Sukces: ślad prowadzi do Cytadeli Cienia' }
    },
    {
      /* Fantasy meets the western: prairies, steamers and locomotives, revolvers, golems and elementals */
      key: 'oskad', scenes: ['Bar', 'Plantacja Fitzsimmonsów'], name: 'Oskad', gms: ['Telegrafista'], state: 'Recruiting', language: 'Polski', hue: 30, markColors: ['#f2d6a2', '#7a2e14'],
      record: {
        id: 'oskad', name: 'Oskad',
        palette: { primary: '#a8431c', secondary: '#6f5a3a', scheme: 'tonal', chroma: 95, temperature: 75, depth: { light: -1, dark: 0 }, mode: 'light' },
        material: { finish: 'matte', grain: 3, tint: 5, edge: 0, shadow: 1 },
        shape: { corner: 'square', scale: 0, border: 'heavy', profile: 'even', tone: 'neutral', frame: 'double' },
        ornament: { set: 'rule', level: 1, edge: 'torn', custom: {
          motif: { vb: [32, 32], p: [{ s: 1.4, d: 'M2 16a14 14 0 1 0 28 0a14 14 0 1 0 -28 0' }, { d: starPath(16, 16, 10.5, 4.4) }] },
          flank: { vb: [14, 14], p: [{ d: starPath(7, 7, 6.5, 2.7) }] } } },
        type: { display: 'archivo', 'case': 'upper', tracking: 30, weight: 800, names: 'display', reading: 'serif', numerals: 'oldstyle' },
        icons: { style: 'sharp', weight: 500 },
        motion: { character: 'crisp' },
        density: 'standard',
        composition: { post: 'open', portrait: 'gutter', names: 'plate', plate: 'accent' },
        imagery: { placement: 'header', treatments: { parallax: 50 }, focus: [50, 64] }
      },
      scene: { title: 'Pociąg do Dodge City', location: 'Wielkie Równiny', arc: 'Srebrna Maska' },
      cast: [
        { name: 'Jebediah Crane', epithet: 'Konstabl, srebrna gwiazda', hue: 40, variant: 'hat', account: 'wiktoria.pds.example', display: 'Wiktoria' },
        { name: 'Szara Chmura', epithet: 'Kojonitka, tropicielka', hue: 200, variant: 'long', account: 'szymon.bsky.social', display: 'Szymon' }
      ],
      posts: [
        '<p>Lokomotywa sapnęła parą i stanęła w szczerym polu, sto mil przed Dodge City. Crane wychylił się przez okno wagonu i zmrużył oczy przed słońcem. Na torach leżał kamienny golem, a raczej to, co z niego zostało.</p><p>— Ktoś tu był przed nami — powiedział, kładąc dłoń na kaburze.</p>',
        '<p>Szara Chmura zeskoczyła na nasyp i obwąchała zwalony kamień. Sierść na karku stanęła jej dęba.</p><p>— Proch i siarka. I coś jeszcze… żywiołak ognia. Konstablu, ten, kto to zrobił, nie chciał zatrzymać pociągu. Chciał, żeby ktoś z niego wysiadł.</p>'
      ],
      ooc: 'Rzucam na tropienie, jak tylko MG potwierdzi trudność.',
      roll: { label: 'Tropienie', expr: '2d6+2', results: [5, 6], total: 13, outcome: 'Sukces: ślady prowadzą do kanionu' }
    },
    {
      /* Space opera: the regions of the galaxy, the HoloNet, the cantina; an archive of 2013–2021 */
      key: 'mglawica', scenes: ['Patrol na Zewnętrznych Rubieżach', 'Bitwa flot'], name: 'Mgławica Mocy', gms: ['Mistrz Gry'], state: 'Archive', stateIcon: 'inventory_2', language: 'Polski', hue: 220, markColors: ['#9fd0ff', '#0b1430'],
      record: {
        id: 'mglawica', name: 'Mgławica Mocy',
        palette: { primary: '#4aa8ff', secondary: '#e3b24f', scheme: 'vibrant', chroma: 140, temperature: -45, depth: { dark: -1, light: 0 }, mode: 'dark' },
        material: { finish: 'sheen', grain: 1, tint: 4, edge: 2, shadow: 1 },
        shape: { corner: 'squircle', scale: 110, border: 'hairline', profile: 'even', tone: 'ornament' },
        ornament: { set: 'fade', level: 1, edge: 'fade' },
        type: { display: 'motet-condensed', 'case': 'upper', tracking: 140, weight: 500, names: 'ui', reading: 'sans', numerals: 'tabular' },
        icons: { style: 'rounded', weight: 300 },
        motion: { character: 'soft' },
        density: 'comfortable',
        composition: { post: 'open', portrait: 'gutter', names: 'plain' },
        imagery: { placement: 'header', treatments: { parallax: 30 }, focus: [42, 24], title: 'hidden' }
      },
      scene: { title: 'Kantyna na Rubieżach', location: 'Przestrzeń Huttów', arc: 'Sygnał z Nieznanej Przestrzeni' },
      cast: [
        { name: 'Tavi Orsen', epithet: 'Przemytniczka, kapitan „Szarej Czapli”', hue: 20, variant: 'short', account: 'ola-m.bsky.social', display: 'Ola' },
        { name: 'Ilan Dorr', epithet: 'Rycerz Jedi w drodze', hue: 210, variant: 'hood', account: 'krzysiek.dev', display: 'Krzysiek' }
      ],
      posts: [
        '<p>Muzyka w kantynie urwała się w pół taktu, kiedy do środka wszedł ktoś w płaszczu pielgrzyma. Tavi nie podniosła wzroku znad kubka. Na Rubieżach przeżywali ci, którzy umieli nie patrzeć.</p><p>— Jeśli szukasz statku — powiedziała, gdy cień padł na stół — to „Szara Czapla” lata tylko za kredyty. Z góry.</p>',
        '<p>Ilan Dorr odsunął kaptur i usiadł naprzeciw, jakby zaproszenie było oczywiste.</p><p>— Szukam kogoś, kto usłyszał sygnał z Nieznanej Przestrzeni i postanowił o nim zapomnieć — powiedział łagodnie. — Moc mówi mi, że to ty. Kredyty mówią mi, że będzie drogo.</p>'
      ],
      ooc: 'Odpis w weekend, wcześniej nie dam rady.',
      roll: { label: 'Perswazja', expr: '2d6+1', results: [4, 6], total: 11, outcome: 'Sukces: Tavi wysłucha oferty' }
    }
  ];
  /* Darkest and lightest colours of each artwork after blurring, as a publish step would measure them */
  var RANGE = {"saltroad":["#2a3c47","#e5ecec"],"neon":["#040c13","#34233c"],"ashfall":["#37221b","#e9c698"],"vellum":["#24304d","#e7d8bc"],"hollow":["#18171a","#c9c8c6"],"ledger":["#14243c","#354d3d"],"kepler":["#0a0705","#613a10"],"elarid":["#0b142b","#d8e0ee"],"oskad":["#4a3523","#fce4b6"],"mglawica":["#000000","#a68f83"]};
  /* ...and their dominant colour (root mean square of the blurred pixels), for mica and for titles on art */
  var DOMINANT = {"saltroad":"#a9b7be","neon":"#0b141e","ashfall":"#8c5237","vellum":"#9b9899","hollow":"#373539","ledger":"#243a3b","kepler":"#2b1a09","elarid":"#273952","oskad":"#b98169","mglawica":"#423c41"};
  worlds.forEach(function (w) {
    w.record.imagery = Object.assign({}, w.record.imagery || {}, { art: art[w.key], range: RANGE[w.key], dominant: DOMINANT[w.key] });
    w.mark = mark(w.key, w.markColors[0], w.markColors[1]);
    w.cast.forEach(function (c) { c.portrait = portrait(c.hue, c.variant, false); });
  });

  /* Render a complete Scene page for a World into `root`: the World's art (fixed behind the page,
     or at its top), chrome islands, the masthead, then the scene on the World's content sheet. */
  function scenePage(root, w, opts) {
    opts = opts || {};
    var h = M.h;
    root.innerHTML = '';
    var scope = h('div', { 'class': 'm-world__root' });
    root.appendChild(scope);
    var mounted = M.mountWorld(scope, w.record, opts.mountOpts || {});
    scope.appendChild(h('div', { 'class': 'm-world__backdrop', 'aria-hidden': 'true' }));
    if (opts.chrome !== false) scope.appendChild(M.Chrome({ world: w.name, worldMark: w.mark, account: 'lena.bsky.social', inbox: 3 }));
    var c0 = w.cast[0], c1 = w.cast[1];
    var post = function (c, i, extra) {
      return M.Post(Object.assign({ character: { name: c.name, epithet: c.epithet, portrait: c.portrait, hue: c.hue }, account: { handle: c.account, display: c.display }, time: ['Mar 3', 'Mar 9', 'Mar 12'][i], timeFull: '2026-03-0' + (3 + i), body: w.posts[i], lead: i === 0 }, extra || {}));
    };
    var stream = h('div', { 'class': 'm-world__stream', 'data-show': 'both' },
      post(c0, 0),
      M.SceneEvent({ kind: 'roll', title: w.roll.label, expr: w.roll.expr, roll: w.roll, character: c1.name, account: { handle: c1.account, display: c1.display }, time: 'Mar 9', source: { icon: 'casino', label: 'Motet dice' } }),
      post(c1, 1, { edited: 'Mar 10' }),
      M.Divider(),
      M.Post({ lane: 'ooc', account: { handle: c0.account, display: c0.display }, time: 'Mar 12', body: '<p>' + (w.ooc || 'Posting my next reply over the weekend.') + '</p>' }),
      h('div', { style: { 'margin-top': '24px' } }, M.Composer({ speaker: c0.name, portrait: c0.portrait, speakerHue: c0.hue, scene: w.scene.title })));
    /* one lane filter in the header on narrow screens, one at the top of the sticky rail on wide ones */
    var filters = [M.LaneFilter({ value: 'both' }), M.LaneFilter({ value: 'both' })];
    filters.forEach(function (f) {
      f.addEventListener('change', function (e) {
        stream.setAttribute('data-show', e.detail.value);
        filters.forEach(function (g) { if (g !== f) g.setValue(e.detail.value); });
      });
    });
    var rail = h('aside', { 'class': 'm-world__rail', 'aria-label': 'Scene' },
      h('div', { 'class': 'm-world__railLanes' }, h('span', null, 'Posts'), filters[1]),
      h('div', { 'class': 'm-character__root m-ornamented m-marked', style: { display: 'block' } },
        h('h3', { style: { margin: '0 0 10px', font: '600 13px/18px var(--font-ui)', color: 'var(--ink-muted)' } }, 'Cast'),
        w.cast.map(function (c) {
          return h('div', { style: { display: 'flex', gap: '10px', 'align-items': 'center', 'margin-block': '8px' } },
            M.Avatar({ kind: 'character', name: c.name, src: c.portrait, size: 'md', decorative: true }),
            h('div', { style: { 'min-width': '0' } },
              h('div', { 'class': 'm-post__name', style: { 'font-size': 'calc(15px * var(--name-adjust, 1))', width: 'fit-content' } }, c.name),
              h('div', { style: { font: '400 13px/18px var(--font-ui)', color: 'var(--ink-muted)' } }, '@' + c.account)));
        }),
        h('div', { style: { 'margin-top': '14px', display: 'grid', gap: '8px', font: '400 13px/18px var(--font-ui)', color: 'var(--ink-muted)' } },
          M.Meta('location_on', w.scene.location), M.Meta('auto_stories', w.scene.arc), M.Meta('visibility', 'Scene members'))));
    scope.appendChild(M.Masthead({ name: w.name, mark: w.mark, gms: w.gms, state: w.state, stateIcon: w.stateIcon, language: w.language, size: opts.masthead || 'slim', current: 'Play' }));
    scope.appendChild(h('main', { 'class': 'm-world__page' },
      h('div', { 'class': 'm-world__sceneHeader' },
        h('h2', { 'class': 'm-world__sceneTitle m-flanked' }, w.scene.title),
        h('div', { 'class': 'm-world__sceneFields' },
          M.Meta('location_on', w.scene.location), M.Meta('auto_stories', w.scene.arc), filters[0])),
      h('div', { 'class': 'm-world__layout' }, stream, rail)));
    M.enhance(scope);
    return mounted;
  }

  /* Render the same content inside several World scopes, side by side. */
  var css = document.createElement('style');
  css.textContent = '.demo-samples{display:grid;gap:12px;padding:12px;grid-template-columns:repeat(auto-fit,minmax(var(--sample-min,280px),1fr))}' +
    '.demo-sample{position:relative;padding:14px 16px 16px;border:1px solid var(--line);border-radius:var(--radius-lg);corner-shape:var(--corner-shape);background-color:var(--surface);background-image:var(--finish),var(--grain);color:var(--ink);min-width:0}' +
    '.demo-sample__label{display:flex;align-items:center;gap:8px;margin-bottom:12px;font:600 12.5px/16px var(--font-ui);color:var(--ink-muted)}' +
    '.demo-row{display:flex;flex-wrap:wrap;gap:8px;align-items:center}' +
    '.demo-pad{padding:16px}.demo-stack{display:grid;gap:12px}' +
    '.demo-caption{font:600 12.5px/16px var(--font-ui);color:var(--ink-muted);margin:0 0 8px}';
  document.head.appendChild(css);
  function byKey(k) { for (var i = 0; i < worlds.length; i++) if (worlds[i].key === k) return worlds[i]; return null; }
  function samples(root, keys, render, opts) {
    var h = M.h;
    var grid = h('div', { 'class': 'demo-samples', style: opts && opts.min ? { '--sample-min': opts.min } : null });
    keys.forEach(function (key) {
      /* 'saltroad@dark' renders a World in a forced reader mode */
      var k = key.split('@')[0], mode = key.split('@')[1];
      var w = byKey(k);
      var scope = h('div', { 'class': 'demo-sample' });
      M.mountWorld(scope, w.record, mode ? Object.assign({}, opts && opts.mountOpts, { mode: mode }) : opts && opts.mountOpts);
      scope.appendChild(h('div', { 'class': 'demo-sample__label' }, M.Avatar({ kind: 'world', name: w.name, src: w.mark, size: 'xs', decorative: true }), (w.key === 'saltroad' ? 'Motet default' : w.name) + (mode ? ', ' + mode : '')));
      var out = render(w, scope);
      if (out) scope.appendChild(out);
      grid.appendChild(scope);
    });
    root.appendChild(grid);
    M.enhance(grid);
    return grid;
  }

  window.MotetDemo = { worlds: worlds, art: art, portrait: portrait, mark: mark, scenePage: scenePage, samples: samples, byKey: byKey };
})();
