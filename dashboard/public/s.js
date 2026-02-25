/* Sparklytics tracking script — public/s.js
 * Usage: <script defer src="/s.js" data-website-id="your-id"></script>
 * Options:
 *   data-api-host            Override the API host (default: same origin)
 *   data-exclude-hash        Set to "true" to skip hash-only URL changes
 *   data-respect-dnt         Set to "false" to ignore DNT/GPC signals (default: "true")
 *   data-disabled            Set to "true" to disable all tracking (e.g. dev/staging)
 *   data-track-links         "true" (all links) or "outbound" (external only)
 *   data-track-scroll-depth  "true" (25/50/75/100%) or comma-separated thresholds e.g. "33,66,100"
 *   data-track-forms         "true" to track form submissions
 *
 * Public API (available after script loads):
 *   window.sparklytics.track(eventName, eventData?)  — fire a custom event
 *   window.sparklytics.identify(visitorId)            — set a stable visitor ID
 *   window.sparklytics.reset()                        — clear the identified visitor ID
 */
(function () {
  var script = document.currentScript;
  if (!script) return;

  var websiteId = script.getAttribute('data-website-id');
  if (!websiteId) return;

  // ── Configuration ─────────────────────────────────────────────────────────
  var excludeHash = script.getAttribute('data-exclude-hash') === 'true';
  var apiHost = script.getAttribute('data-api-host') || '';
  var endpoint = apiHost + '/api/collect';
  var respectDnt = script.getAttribute('data-respect-dnt') !== 'false';
  var disabled = script.getAttribute('data-disabled') === 'true';

  var trackLinksAttr = script.getAttribute('data-track-links');
  var trackLinks = trackLinksAttr === 'true' ? true : trackLinksAttr === 'outbound' ? 'outbound' : false;

  var trackScrollDepth = (function () {
    var attr = script.getAttribute('data-track-scroll-depth');
    if (!attr || attr === 'false') return false;
    if (attr === 'true') return [25, 50, 75, 100];
    var parsed = attr.split(',').map(function (n) { return parseInt(n.trim(), 10); }).filter(function (n) { return !isNaN(n); });
    return parsed.length ? parsed : false;
  })();

  var trackForms = script.getAttribute('data-track-forms') === 'true';

  // ── Privacy signals (DNT + GPC) ──────────────────────────────────────────
  function isBlocked() {
    if (disabled) return true;
    if (!respectDnt) return false;
    if (navigator.doNotTrack === '1') return true;
    if (navigator.globalPrivacyControl === true) return true;
    return false;
  }

  if (isBlocked()) return;

  // ── Visitor identification ────────────────────────────────────────────────
  var IDENTIFY_KEY = 'sparklytics_visitor_id';

  function getIdentifiedVisitor() {
    try { return localStorage.getItem(IDENTIFY_KEY); } catch (e) { return null; }
  }

  function setIdentifiedVisitor(id) {
    try { localStorage.setItem(IDENTIFY_KEY, id); } catch (e) { /* ignore */ }
  }

  function clearIdentifiedVisitor() {
    try { localStorage.removeItem(IDENTIFY_KEY); } catch (e) { /* ignore */ }
  }

  // ── UTM parameter capture ────────────────────────────────────────────────
  var UTM_KEYS = ['utm_source', 'utm_medium', 'utm_campaign', 'utm_term', 'utm_content'];
  var UTM_SESSION_KEY = '_spl_utm';

  function resolveUtmParams() {
    try {
      var params = new URLSearchParams(window.location.search);
      var fromUrl = {};
      var hasUrl = false;
      for (var i = 0; i < UTM_KEYS.length; i++) {
        var val = params.get(UTM_KEYS[i]);
        if (val) { fromUrl[UTM_KEYS[i]] = val; hasUrl = true; }
      }
      if (hasUrl) {
        try { sessionStorage.setItem(UTM_SESSION_KEY, JSON.stringify(fromUrl)); } catch (e) { /* ignore */ }
        return fromUrl;
      }
      var stored = sessionStorage.getItem(UTM_SESSION_KEY);
      if (stored) return JSON.parse(stored);
    } catch (e) { /* ignore */ }
    return {};
  }

  // ── Event sender ─────────────────────────────────────────────────────────
  function send(payload) {
    if (isBlocked()) return;

    var visitorId = getIdentifiedVisitor();
    if (visitorId) payload.visitor_id = visitorId;

    var body = JSON.stringify(payload);

    function doSend() {
      if (navigator.sendBeacon) {
        var queued = navigator.sendBeacon(endpoint, new Blob([body], { type: 'application/json' }));
        if (queued) return true;
      }
      fetch(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: body,
        credentials: 'omit',
        keepalive: true,
      }).catch(function () { /* fail silently */ });
      return true;
    }

    try {
      if (!doSend()) {
        setTimeout(function () { try { doSend(); } catch (e) { /* drop */ } }, 2000);
      }
    } catch (e) { /* fail silently */ }
  }

  // ── Payload builders ─────────────────────────────────────────────────────
  function buildPageviewExtras() {
    var extras = {};
    if (navigator.language) extras.language = navigator.language;
    if (screen.width && screen.height) {
      extras.screen = screen.width + 'x' + screen.height;
      extras.screen_width = screen.width;
      extras.screen_height = screen.height;
    }
    var utm = resolveUtmParams();
    for (var k in utm) {
      if (utm.hasOwnProperty(k)) extras[k] = utm[k];
    }
    return extras;
  }

  function buildPageview(url) {
    var payload = {
      website_id: websiteId,
      type: 'pageview',
      url: url,
    };
    var ref = document.referrer;
    if (ref) payload.referrer = ref;
    var extras = buildPageviewExtras();
    for (var k in extras) {
      if (extras.hasOwnProperty(k)) payload[k] = extras[k];
    }
    return payload;
  }

  // ── Pageview dedup (URL + 100ms timing window) ───────────────────────────
  var lastSentUrl = '';
  var lastSentTs = 0;

  function sendPageview(url) {
    var now = Date.now();
    if (url === lastSentUrl && now - lastSentTs < 100) return;
    lastSentUrl = url;
    lastSentTs = now;
    send(buildPageview(url));
  }

  // ── Scroll depth state ────────────────────────────────────────────────────
  var scrollFired = {};
  var lastScrollUrl = '';

  function resetScrollDepth() {
    scrollFired = {};
    lastScrollUrl = window.location.href;
  }

  // ── SPA navigation detection ─────────────────────────────────────────────
  function onNavigation() {
    var current = window.location.href;
    if (current === lastSentUrl) return;
    if (excludeHash && current.split('#')[0] === lastSentUrl.split('#')[0]) return;
    resetScrollDepth();
    sendPageview(current);
  }

  function patchHistory(method) {
    var orig = history[method];
    history[method] = function () {
      orig.apply(this, arguments);
      onNavigation();
    };
  }

  patchHistory('pushState');
  patchHistory('replaceState');
  window.addEventListener('popstate', onNavigation);

  // ── Link click delegation ─────────────────────────────────────────────────
  if (trackLinks) {
    document.addEventListener('click', function (e) {
      if (isBlocked()) return;
      var anchor = e.target && e.target.closest ? e.target.closest('a[href]') : null;
      if (!anchor) return;

      var rawHref = anchor.getAttribute('href') || '';
      if (!rawHref || rawHref.charAt(0) === '#' || rawHref.indexOf('javascript:') === 0) return;

      var href = rawHref;
      var external = false;
      try {
        var url = new URL(rawHref, window.location.href);
        external = url.origin !== window.location.origin;
        href = external ? url.href : url.pathname + url.search + url.hash;
      } catch (ex) {
        external = true;
      }

      if (trackLinks === 'outbound' && !external) return;

      var text = (anchor.textContent || '').trim().replace(/\s+/g, ' ').slice(0, 100) || undefined;
      var eventData = { href: href };
      if (text) eventData.text = text;
      if (external) eventData.external = true;

      send({
        website_id: websiteId,
        type: 'event',
        url: window.location.href,
        event_name: 'link_click',
        event_data: eventData,
      });
    }, true); // capture phase — fires before React/framework onClick handlers
  }

  // ── Scroll depth tracking ─────────────────────────────────────────────────
  if (trackScrollDepth) {
    lastScrollUrl = window.location.href;

    window.addEventListener('scroll', function () {
      if (isBlocked()) return;

      var currentUrl = window.location.href;
      if (currentUrl !== lastScrollUrl) resetScrollDepth();

      var scrollTop = window.scrollY !== undefined ? window.scrollY : document.documentElement.scrollTop;
      var docHeight = document.documentElement.scrollHeight - window.innerHeight;
      if (docHeight <= 0) return;

      var pct = Math.round((scrollTop / docHeight) * 100);

      for (var i = 0; i < trackScrollDepth.length; i++) {
        var threshold = trackScrollDepth[i];
        if (pct >= threshold && !scrollFired[threshold]) {
          scrollFired[threshold] = true;
          send({
            website_id: websiteId,
            type: 'event',
            url: currentUrl,
            event_name: 'scroll_depth',
            event_data: { depth: threshold },
          });
        }
      }
    }, { passive: true });
  }

  // ── Form submission tracking ──────────────────────────────────────────────
  if (trackForms) {
    document.addEventListener('submit', function (e) {
      if (isBlocked()) return;
      var form = e.target;
      if (!form || form.tagName !== 'FORM') return;

      var eventData = {};
      if (form.id) eventData.form_id = form.id;
      if (form.name) eventData.form_name = form.name;
      if (form.action && form.action.indexOf('javascript:') !== 0) {
        eventData.action = form.action;
      }

      send({
        website_id: websiteId,
        type: 'event',
        url: window.location.href,
        event_name: 'form_submit',
        event_data: eventData,
      });
    }, true); // capture phase — fires before the form's own submit handler
  }

  // ── Public API ───────────────────────────────────────────────────────────
  window.sparklytics = {
    track: function (eventName, eventData) {
      if (isBlocked()) return;
      if (!eventName || typeof eventName !== 'string') return;
      if (eventName.length > 50) eventName = eventName.slice(0, 50);
      if (eventData) {
        try {
          if (JSON.stringify(eventData).length > 4096) return;
        } catch (e) { return; }
      }
      var payload = {
        website_id: websiteId,
        type: 'event',
        url: window.location.href,
        event_name: eventName,
      };
      if (eventData) payload.event_data = eventData;
      var ref = document.referrer;
      if (ref) payload.referrer = ref;
      if (navigator.language) payload.language = navigator.language;
      send(payload);
    },
    identify: function (visitorId) {
      if (visitorId && typeof visitorId === 'string' && visitorId.length <= 64) {
        setIdentifiedVisitor(visitorId);
      }
    },
    reset: function () {
      clearIdentifiedVisitor();
    },
  };

  // ── Initial pageview ─────────────────────────────────────────────────────
  lastSentUrl = window.location.href;
  lastSentTs = Date.now();
  send(buildPageview(lastSentUrl));
})();
