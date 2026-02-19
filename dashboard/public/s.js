/* Sparklytics tracking script — public/s.js
 * Usage: <script defer src="/s.js" data-website-id="your-id"></script>
 * Options:
 *   data-api-host       Override the API host (default: same origin)
 *   data-exclude-hash   Set to "true" to skip hash-only URL changes
 */
(function () {
  var script = document.currentScript;
  if (!script) return;

  var websiteId = script.getAttribute('data-website-id');
  if (!websiteId) return;

  var excludeHash = script.getAttribute('data-exclude-hash') === 'true';
  var apiHost = script.getAttribute('data-api-host') || '';
  var endpoint = apiHost + '/api/collect';

  // ── Visitor ID ────────────────────────────────────────────────────────────
  // A client-side identifier stored in localStorage with a 24-hour TTL.
  // The server computes its own visitor_id from IP + UA; this ID is for
  // client-side session continuity and is not sent in the collect payload.
  var STORAGE_KEY = '_spl_vid';
  var TTL_MS = 24 * 60 * 60 * 1000;

  function getVisitorId() {
    try {
      var raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        var stored = JSON.parse(raw);
        if (stored && stored.exp > Date.now()) return stored.id;
      }
    } catch (e) { /* ignore */ }
    // Generate a new random visitor ID and persist it.
    var id = (Math.random().toString(36) + Math.random().toString(36)).replace(/\./g, '').slice(2, 18);
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ id: id, exp: Date.now() + TTL_MS }));
    } catch (e) { /* ignore */ }
    return id;
  }

  // Initialise on load (refreshes TTL if already present).
  getVisitorId();

  // ── Pageview sender ───────────────────────────────────────────────────────
  var lastSentUrl = '';

  function sendPageview(url) {
    try {
      var payload = {
        website_id: websiteId,
        type: 'pageview',
        url: url,
      };

      var ref = document.referrer;
      if (ref) payload.referrer = ref;

      var lang = navigator.language;
      if (lang) payload.language = lang;

      if (screen.width && screen.height) {
        payload.screen = screen.width + 'x' + screen.height;
      }

      fetch(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
        credentials: 'omit',
        keepalive: true,
      }).catch(function () { /* fail silently */ });
    } catch (e) { /* fail silently */ }
  }

  // ── SPA navigation detection ──────────────────────────────────────────────
  function onNavigation() {
    var current = window.location.href;
    if (current === lastSentUrl) return;
    if (excludeHash && current.split('#')[0] === lastSentUrl.split('#')[0]) return;
    lastSentUrl = current;
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

  // ── Initial pageview ──────────────────────────────────────────────────────
  lastSentUrl = window.location.href;
  sendPageview(lastSentUrl);
})();
