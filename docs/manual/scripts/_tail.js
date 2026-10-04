<script>
(function () {
  var layout = document.getElementById('layout');
  var toggle = document.getElementById('sideToggle');
  var main = document.getElementById('main');
  var q = document.getElementById('q');
  var stat = document.getElementById('stat');
  var jump = document.getElementById('jump');
  var jumpList = document.getElementById('jumpList');

  /* ── 側欄收合 ─────────────────────── */
  toggle.addEventListener('click', function () {
    if (window.innerWidth <= 900) layout.classList.toggle('nav-open');
    else layout.classList.toggle('nav-collapsed');
  });

  /* ── 捲動時標示目前小節 ───────────── */
  var subLinks = Array.prototype.slice.call(document.querySelectorAll('#subToc a'));
  var sections = subLinks.map(function (a) { return document.querySelector(a.getAttribute('href')); });
  function spy() {
    var y = window.scrollY + 140, cur = 0;
    for (var i = 0; i < sections.length; i++) {
      if (sections[i] && sections[i].offsetTop <= y) cur = i;
    }
    subLinks.forEach(function (a, i) { a.classList.toggle('active', i === cur); });
  }
  window.addEventListener('scroll', spy, { passive: true });
  spy();

  /* ── 字級縮放 ─────────────────────── */
  var ZOOMS = [0.5, 0.6, 0.7, 0.8, 0.9, 1, 1.1, 1.25, 1.4, 1.6, 1.8, 2];
  var zoomVal = 1;
  try { var stored = parseFloat(localStorage.getItem('eryaManualZoom')); if (stored) zoomVal = stored; } catch (e) {}
  var fcVal = document.querySelector('.fontctl .fc-val');

  function applyZoom(v) {
    zoomVal = v;
    document.documentElement.style.setProperty('--zoom', String(v));
    if (fcVal) fcVal.textContent = Math.round(v * 100) + '%';
    try { localStorage.setItem('eryaManualZoom', String(v)); } catch (e) {}
  }

  function stepZoom(dir) {
    var i = 1;
    for (var k = 0; k < ZOOMS.length; k++) { if (Math.abs(ZOOMS[k] - zoomVal) < 0.001) i = k; }
    applyZoom(ZOOMS[Math.min(ZOOMS.length - 1, Math.max(0, i + dir))]);
  }

  Array.prototype.forEach.call(document.querySelectorAll('.fontctl button'), function (b) {
    b.addEventListener('click', function () {
      var d = b.getAttribute('data-fs');
      if (d === '0') applyZoom(1); else stepZoom(d === '+' ? 1 : -1);
    });
  });
  applyZoom(zoomVal);

  /* 用 file:// 直接開檔時，部分瀏覽器不給用 localStorage
     → 把字級掛在網址參數上，翻到下一章時帶過去 */
  try {
    var qz = parseFloat(new URLSearchParams(location.search).get('z'));
    if (qz) applyZoom(qz);
  } catch (e) {}

  document.addEventListener('click', function (e) {
    var a = e.target && e.target.closest ? e.target.closest('a[href]') : null;
    if (!a) return;
    var href = a.getAttribute('href') || '';
    if (/^https?:/i.test(href) || !/\.html(\?|#|$)/i.test(href)) return;
    var hash = href.indexOf('#') === -1 ? '' : href.slice(href.indexOf('#'));
    var file = href.split('#')[0].split('?')[0];
    a.setAttribute('href', file + '?z=' + zoomVal + hash);
  }, true);

  /* ── 搜尋 ─────────────────────────── */
  var marks = [], cur = -1;
  var chapters = Array.prototype.slice.call(document.querySelectorAll('.toc a'));

  function clearHits() {
    document.querySelectorAll('mark.hit').forEach(function (m) {
      var t = document.createTextNode(m.textContent);
      var p = m.parentNode;
      p.replaceChild(t, m);
      p.normalize();
    });
    document.querySelectorAll('.svg-hit').forEach(function (e) { e.classList.remove('svg-hit'); });
    chapters.forEach(function (a) { a.classList.remove('hit-kw'); });
    marks = []; cur = -1;
  }

  function inSvg(node) {
    var p = node.parentNode;
    while (p) { if (p.nodeName.toLowerCase() === 'svg') return true; p = p.parentNode; }
    return false;
  }

  function highlight(term) {
    var walker = document.createTreeWalker(main, NodeFilter.SHOW_TEXT, {
      acceptNode: function (n) {
        if (!n.nodeValue || n.nodeValue.indexOf(term) === -1) return NodeFilter.FILTER_REJECT;
        var pn = n.parentNode.nodeName.toLowerCase();
        if (pn === 'script' || pn === 'style' || pn === 'mark') return NodeFilter.FILTER_REJECT;
        if (inSvg(n)) return NodeFilter.FILTER_REJECT;
        return NodeFilter.FILTER_ACCEPT;
      }
    });
    var targets = [], n;
    while ((n = walker.nextNode())) targets.push(n);

    targets.forEach(function (node) {
      var parts = node.nodeValue.split(term);
      var frag = document.createDocumentFragment();
      parts.forEach(function (piece, i) {
        if (i > 0) {
          var m = document.createElement('mark');
          m.className = 'hit';
          m.textContent = term;
          frag.appendChild(m);
          marks.push(m);
        }
        if (piece) frag.appendChild(document.createTextNode(piece));
      });
      node.parentNode.replaceChild(frag, node);
    });

    /* SVG 內的文字不能包 mark，改標顏色 */
    var svgHits = 0;
    main.querySelectorAll('svg text').forEach(function (t) {
      if (t.textContent.indexOf(term) !== -1) { t.classList.add('svg-hit'); svgHits++; }
    });
    return svgHits;
  }

  function matchChapters(term) {
    var hits = [];
    chapters.forEach(function (a) {
      var ch = a.getAttribute('data-ch') || '';
      var kw = (a.getAttribute('data-kw') || '') + ' ' + a.textContent + ' ' + ch;
      if (kw.indexOf(term) === -1) return;
      a.classList.add('hit-kw');
      if (!a.classList.contains('current')) hits.push(a);
    });
    return hits;
  }

  function render(term) {
    clearHits();
    if (!term) {
      stat.textContent = '本章可全文搜尋，圖上的字也找得到。';
      jump.classList.add('hidden');
      return;
    }
    var svgHits = highlight(term);
    var chs = matchChapters(term);

    var msg = '本章 <b>' + marks.length + '</b> 處';
    if (svgHits) msg += '，圖上 <b>' + svgHits + '</b> 處';
    if (marks.length) {
      msg += '<span class="search-nav"><button type="button" data-nav="prev">↑</button><button type="button" data-nav="next">↓</button></span>';
    }
    if (!marks.length && !svgHits && !chs.length) msg = '沒找到「' + term + '」';
    stat.innerHTML = msg;

    var navBtns = stat.querySelectorAll('[data-nav]');
    Array.prototype.forEach.call(navBtns, function (b) {
      b.addEventListener('click', function () { step(b.getAttribute('data-nav') === 'next' ? 1 : -1); });
    });

    jumpList.innerHTML = '';
    if (chs.length) {
      chs.forEach(function (a) {
        var link = document.createElement('a');
        link.href = a.getAttribute('href');
        link.innerHTML = '<i>' + (a.getAttribute('data-ch') || '') + '</i>' +
                         a.textContent.replace(a.querySelector('i') ? a.querySelector('i').textContent : '', '');
        jumpList.appendChild(link);
      });
      jump.classList.remove('hidden');
    } else {
      jump.classList.add('hidden');
    }

    if (marks.length) step(1);
  }

  function step(dir) {
    if (!marks.length) return;
    if (cur >= 0 && marks[cur]) marks[cur].classList.remove('cur');
    cur = (cur + dir + marks.length) % marks.length;
    var m = marks[cur];
    m.classList.add('cur');
    m.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }

  var timer = null;
  q.addEventListener('input', function () {
    clearTimeout(timer);
    var v = q.value.trim();
    timer = setTimeout(function () { render(v); }, 180);
  });
  q.addEventListener('keydown', function (e) {
    if (e.key === 'Enter') { e.preventDefault(); step(e.shiftKey ? -1 : 1); }
    if (e.key === 'Escape') { q.value = ''; render(''); q.blur(); }
  });
  document.addEventListener('keydown', function (e) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      if (window.innerWidth <= 900) layout.classList.add('nav-open');
      layout.classList.remove('nav-collapsed');
      q.focus(); q.select();
    }
  });
})();
</script>
