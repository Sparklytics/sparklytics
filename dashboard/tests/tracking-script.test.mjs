import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

const scriptSource = readFileSync(new URL('../public/s.js', import.meta.url), 'utf8');

function runTracker({ src = 'https://site.example/s.js', attrs = {}, storedVisitorId = null } = {}) {
  const fetchCalls = [];
  const storage = new Map();
  if (storedVisitorId) storage.set('sparklytics_visitor_id', storedVisitorId);

  const context = {
    Blob,
    Date,
    URL,
    URLSearchParams,
    clearTimeout,
    console,
    fetch: (url, options) => {
      fetchCalls.push({ url, options, body: JSON.parse(options.body) });
      return Promise.resolve({ ok: true });
    },
    history: {
      pushState() {},
      replaceState() {},
    },
    localStorage: {
      getItem: (key) => storage.get(key) ?? null,
      removeItem: (key) => storage.delete(key),
      setItem: (key, value) => storage.set(key, String(value)),
    },
    navigator: {
      doNotTrack: '0',
      globalPrivacyControl: false,
      language: 'en-US',
    },
    screen: {
      height: 900,
      width: 1440,
    },
    sessionStorage: {
      getItem: () => null,
      setItem: () => {},
    },
    setTimeout,
  };

  const currentScript = {
    getAttribute(name) {
      if (name === 'src') return src;
      if (name === 'data-website-id') return 'site_tracker';
      return attrs[name] ?? null;
    },
    src,
  };

  context.window = {
    addEventListener() {},
    location: {
      href: 'https://site.example/pricing?utm_source=newsletter',
      origin: 'https://site.example',
    },
  };
  context.window.window = context.window;
  context.window.history = context.history;
  context.window.localStorage = context.localStorage;
  context.window.navigator = context.navigator;
  context.window.screen = context.screen;
  context.window.sessionStorage = context.sessionStorage;
  context.window.setTimeout = context.setTimeout;
  context.window.fetch = context.fetch;
  context.window.Blob = context.Blob;
  context.window.URL = context.URL;
  context.window.URLSearchParams = context.URLSearchParams;

  context.document = {
    addEventListener() {},
    currentScript,
    documentElement: {
      scrollHeight: 1200,
      scrollTop: 0,
    },
    referrer: 'https://referrer.example/',
  };
  context.window.document = context.document;

  vm.runInNewContext(scriptSource, context);

  return { fetchCalls, storage, window: context.window };
}

test('derives first-party ingest endpoint from script path', () => {
  const { fetchCalls } = runTracker({ src: 'https://site.example/_sl/s.js' });

  assert.equal(fetchCalls.length, 1);
  assert.equal(fetchCalls[0].url, 'https://site.example/_sl/e');
  assert.equal(fetchCalls[0].body.website_id, 'site_tracker');
  assert.equal(fetchCalls[0].body.type, 'pageview');
  assert.equal(fetchCalls[0].body.visitor_id, undefined);
});

test('data-api-host overrides the script-derived ingest endpoint', () => {
  const { fetchCalls } = runTracker({
    src: 'https://site.example/_sl/s.js',
    attrs: { 'data-api-host': 'https://analytics.example/custom/' },
  });

  assert.equal(fetchCalls.length, 1);
  assert.equal(fetchCalls[0].url, 'https://analytics.example/custom/e');
});

test('visitor ID is sent only after explicit identify call', () => {
  const { fetchCalls, window } = runTracker({ src: 'https://site.example/_sl/s.js' });

  assert.equal(fetchCalls[0].body.visitor_id, undefined);

  window.sparklytics.identify('identified-user-123');
  window.sparklytics.track('signup', { plan: 'self-host' });

  assert.equal(fetchCalls.length, 2);
  assert.equal(fetchCalls[1].url, 'https://site.example/_sl/e');
  assert.equal(fetchCalls[1].body.type, 'event');
  assert.equal(fetchCalls[1].body.event_name, 'signup');
  assert.equal(fetchCalls[1].body.visitor_id, 'identified-user-123');
});
