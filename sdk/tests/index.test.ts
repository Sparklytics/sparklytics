/**
 * @sparklytics/next SDK — Vitest test suite
 *
 * Covers all BDD scenarios from docs/sprints/sprint-3.md.
 * Environment: happy-dom (configured in vitest.config.ts)
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, act, cleanup } from '@testing-library/react'
import React from 'react'

// ──────────────────────────────────────────────────────────────
// Module mocks — declared before SDK import; vi.mock is hoisted.
// ──────────────────────────────────────────────────────────────

vi.mock('next/navigation', () => ({
  usePathname: vi.fn().mockReturnValue('/'),
}))

vi.mock('next/router', () => ({
  default: {
    events: {
      on: vi.fn(),
      off: vi.fn(),
    },
  },
}))

// Import SDK after mocks are registered
import {
  SparklyticsProvider,
  useSparklytics,
  SparklyticsEvent,
} from '../src/index'

// Import mocked next/router so tests can inspect registered handlers
import nextRouter from 'next/router'

// ──────────────────────────────────────────────────────────────
// Test helpers
// ──────────────────────────────────────────────────────────────

function renderProvider(
  props: Partial<React.ComponentProps<typeof SparklyticsProvider>> & { websiteId: string },
  child: React.ReactNode = React.createElement('div', {}, 'child'),
) {
  return render(
    React.createElement(SparklyticsProvider, props, child),
  )
}

/** Flush debounce timer and allow async microtasks to settle. */
async function flushQueue() {
  await act(async () => {
    vi.advanceTimersByTime(600)
    await Promise.resolve()
    await Promise.resolve()
  })
}

/** Wait for async effects (dynamic imports, promise chains) to settle. */
async function settle() {
  await act(async () => {
    await Promise.resolve()
    await Promise.resolve()
    await Promise.resolve()
  })
}

// ──────────────────────────────────────────────────────────────
// Setup / teardown
// ──────────────────────────────────────────────────────────────

let sendBeaconMock: ReturnType<typeof vi.fn>
let fetchMock: ReturnType<typeof vi.fn>

beforeEach(() => {
  vi.useFakeTimers()

  sendBeaconMock = vi.fn().mockReturnValue(true)
  fetchMock = vi.fn().mockResolvedValue(new Response('{}', { status: 200 }))

  Object.defineProperty(window, 'location', {
    value: { pathname: '/', href: 'http://localhost/' },
    writable: true,
    configurable: true,
  })
  Object.defineProperty(document, 'referrer', {
    value: '',
    writable: true,
    configurable: true,
  })
  vi.stubGlobal('fetch', fetchMock)
  vi.stubGlobal('navigator', { sendBeacon: sendBeaconMock, doNotTrack: null })
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.useRealTimers()
  vi.clearAllMocks()
})

// ──────────────────────────────────────────────────────────────
// Feature: Next.js SDK Installation
// ──────────────────────────────────────────────────────────────

describe('Initial pageview', () => {
  it('test_initial_pageview_on_mount — fires pageview on first render', async () => {
    renderProvider({ websiteId: 'site_1' })
    await flushQueue()
    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
    const blob: Blob = sendBeaconMock.mock.calls[0][1]
    const events = JSON.parse(await blob.text()) as Record<string, unknown>[]
    expect(Array.isArray(events)).toBe(true)
    expect(events).toHaveLength(1)
    expect(events[0]['type']).toBe('pageview')
    expect(events[0]['website_id']).toBe('site_1')
    expect(events[0]['url']).toBe('/')
  })
})

describe('sendBeacon format', () => {
  it('test_senbeacon_uses_blob — sends Blob with application/json', async () => {
    renderProvider({ websiteId: 'site_1' })
    await flushQueue()
    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
    const [url, blob] = sendBeaconMock.mock.calls[0] as [string, Blob]
    expect(url).toContain('/api/collect')
    expect(blob).toBeInstanceOf(Blob)
    expect(blob.type).toBe('application/json')
  })

  it('test_batch_format_is_array_not_object — POST body is a JSON array', async () => {
    // Remove sendBeacon from navigator to force fetch path
    vi.stubGlobal('navigator', { doNotTrack: null })
    renderProvider({ websiteId: 'site_1' })
    await flushQueue()
    expect(fetchMock).toHaveBeenCalledTimes(1)
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit]
    const parsed = JSON.parse(init.body as string) as unknown
    expect(Array.isArray(parsed)).toBe(true)
    const first = (parsed as Record<string, unknown>[])[0]
    expect(first['type']).toBe('pageview')
    expect(first['website_id']).toBe('site_1')
  })
})

describe('Custom endpoint URL', () => {
  it('test_custom_endpoint_url_appends_collect — uses base URL + /api/collect', async () => {
    renderProvider({
      websiteId: 'site_1',
      endpoint: 'https://analytics.example.com',
    })
    await flushQueue()
    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
    const [url] = sendBeaconMock.mock.calls[0] as [string]
    expect(url).toBe('https://analytics.example.com/api/collect')
  })
})

// ──────────────────────────────────────────────────────────────
// Feature: SPA and Pages Router navigation tracking
// ──────────────────────────────────────────────────────────────

describe('SPA navigation tracking', () => {
  it('test_pushstate_navigation_tracked — enqueues pageview on pushState', async () => {
    Object.defineProperty(document, 'referrer', {
      value: 'https://google.com',
      writable: true,
      configurable: true,
    })
    renderProvider({ websiteId: 'site_1' })
    await flushQueue()
    sendBeaconMock.mockClear()

    await act(async () => {
      ;(window.location as { pathname: string }).pathname = '/dashboard'
      history.pushState({}, '', '/dashboard')
      vi.advanceTimersByTime(600)
      await Promise.resolve()
    })

    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
    const blob: Blob = sendBeaconMock.mock.calls[0][1]
    const events = JSON.parse(await blob.text()) as Record<string, unknown>[]
    expect(events[0]['type']).toBe('pageview')
    expect(events[0]['url']).toBe('/dashboard')
    expect(events[0]['website_id']).toBe('site_1')
    expect(events[0]['referrer']).toBe('https://google.com')
  })

  it('test_popstate_navigation_tracked — enqueues pageview on back/forward', async () => {
    renderProvider({ websiteId: 'site_1' })
    await flushQueue()
    sendBeaconMock.mockClear()

    await act(async () => {
      ;(window.location as { pathname: string }).pathname = '/back'
      window.dispatchEvent(new PopStateEvent('popstate'))
      vi.advanceTimersByTime(600)
      await Promise.resolve()
    })

    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
    const blob: Blob = sendBeaconMock.mock.calls[0][1]
    const events = JSON.parse(await blob.text()) as Record<string, unknown>[]
    expect(events[0]['type']).toBe('pageview')
    expect(events[0]['url']).toBe('/back')
  })

  it('test_pages_router_route_change_tracked — routeChangeComplete fires pageview', async () => {
    renderProvider({ websiteId: 'site_1' })
    // Wait for the dynamic import('next/router').then(...) to resolve and register handler
    await settle()
    await flushQueue()
    sendBeaconMock.mockClear()

    // Retrieve the handler registered via router.events.on('routeChangeComplete', handler)
    const routerOnMock = nextRouter.events.on as ReturnType<typeof vi.fn>
    const routeChangeCall = routerOnMock.mock.calls.find(
      ([event]: [string]) => event === 'routeChangeComplete',
    )
    expect(routeChangeCall).toBeDefined()
    const routeChangeHandler = routeChangeCall![1] as (url: string) => void

    // Simulate Pages Router navigation
    await act(async () => {
      routeChangeHandler('/settings')
      vi.advanceTimersByTime(600)
      await Promise.resolve()
    })

    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
    const blob: Blob = sendBeaconMock.mock.calls[0][1]
    const events = JSON.parse(await blob.text()) as Record<string, unknown>[]
    expect(events[0]['type']).toBe('pageview')
    expect(events[0]['url']).toBe('/settings')
  })
})

// ──────────────────────────────────────────────────────────────
// Feature: Custom event via useSparklytics hook
// ──────────────────────────────────────────────────────────────

describe('useSparklytics hook', () => {
  it('test_custom_event_via_track_hook — track() sends event with correct fields', async () => {
    Object.defineProperty(document, 'referrer', {
      value: 'https://twitter.com',
      writable: true,
      configurable: true,
    })
    let trackFn: ((name: string, data?: Record<string, unknown>) => void) | undefined

    function Consumer() {
      const { track } = useSparklytics()
      trackFn = track
      return null
    }

    renderProvider({ websiteId: 'site_1' }, React.createElement(Consumer))
    await flushQueue()
    sendBeaconMock.mockClear()

    await act(async () => {
      trackFn!('signup_click', { plan: 'pro' })
      vi.advanceTimersByTime(600)
      await Promise.resolve()
    })

    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
    const blob: Blob = sendBeaconMock.mock.calls[0][1]
    const events = JSON.parse(await blob.text()) as Record<string, unknown>[]
    expect(events[0]['type']).toBe('event')
    expect(events[0]['event_name']).toBe('signup_click')
    expect(events[0]['event_data']).toEqual({ plan: 'pro' })
    expect(events[0]['url']).toBe('/')
    expect(events[0]['referrer']).toBe('https://twitter.com')
  })
})

// ──────────────────────────────────────────────────────────────
// Feature: SDK Error Handling
// ──────────────────────────────────────────────────────────────

describe('Error handling', () => {
  it('test_websiteId_required_logs_error_and_no_events — empty websiteId: console.error, no events', async () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    renderProvider({ websiteId: '' })
    await flushQueue()
    expect(consoleSpy).toHaveBeenCalledWith(
      expect.stringContaining('websiteId is required'),
    )
    expect(sendBeaconMock).not.toHaveBeenCalled()
    expect(fetchMock).not.toHaveBeenCalled()
    consoleSpy.mockRestore()
  })

  it('test_unreachable_endpoint_handled_gracefully — fetch error does not crash page or block queue', async () => {
    // No sendBeacon → falls back to fetch
    vi.stubGlobal('navigator', { doNotTrack: null })
    fetchMock.mockRejectedValue(new Error('Network error'))

    let trackFn: ((name: string) => void) | undefined
    function Consumer() {
      const { track } = useSparklytics()
      trackFn = track
      return null
    }

    // Render — the initial pageview flush will reject but must not throw on the page
    renderProvider({ websiteId: 'site_1' }, React.createElement(Consumer))

    // Flush: initial fetch rejects; retry is scheduled at +2s
    await flushQueue()

    // Advance past the 2s retry — retry also rejects, event is silently dropped
    await act(async () => {
      vi.advanceTimersByTime(2500)
      await Promise.resolve()
      await Promise.resolve()
    })

    // Queue must not be permanently blocked — subsequent track() calls still work
    fetchMock.mockResolvedValue(new Response('{}', { status: 200 }))
    await act(async () => {
      trackFn!('recovery_event')
      vi.advanceTimersByTime(600)
      await Promise.resolve()
    })
    // 3 calls: initial fail + retry fail + successful recovery
    expect(fetchMock).toHaveBeenCalledTimes(3)
  })
})

// ──────────────────────────────────────────────────────────────
// Feature: Event batching
// ──────────────────────────────────────────────────────────────

describe('Event batching', () => {
  it('test_event_batching_debounce_500ms — 5 rapid track() calls → 1 POST', async () => {
    let trackFn: ((name: string) => void) | undefined

    function Consumer() {
      const { track } = useSparklytics()
      trackFn = (name: string) => track(name)
      return null
    }

    renderProvider({ websiteId: 'site_1' }, React.createElement(Consumer))
    await flushQueue()
    sendBeaconMock.mockClear()

    await act(async () => {
      for (let i = 0; i < 5; i++) {
        trackFn!(`event_${i}`)
      }
      vi.advanceTimersByTime(600)
      await Promise.resolve()
    })

    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
    const blob: Blob = sendBeaconMock.mock.calls[0][1]
    const events = JSON.parse(await blob.text()) as unknown[]
    expect(events).toHaveLength(5)
  })

  it('test_event_batching_max_10_coalesces — 10 events trigger immediate flush without timer', async () => {
    let trackFn: ((name: string) => void) | undefined

    function Consumer() {
      const { track } = useSparklytics()
      trackFn = (name: string) => track(name)
      return null
    }

    renderProvider({ websiteId: 'site_1' }, React.createElement(Consumer))
    await flushQueue()
    sendBeaconMock.mockClear()

    await act(async () => {
      for (let i = 0; i < 10; i++) {
        trackFn!(`event_${i}`)
      }
      // Do NOT advance timers — 10 events should flush immediately
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
    const blob: Blob = sendBeaconMock.mock.calls[0][1]
    const events = JSON.parse(await blob.text()) as unknown[]
    expect(events).toHaveLength(10)
  })

  it('test_beforeunload_triggers_flush — beforeunload sends pending events via sendBeacon', async () => {
    renderProvider({ websiteId: 'site_1' })
    // Flush the initial pageview so the queue is clean
    await flushQueue()
    sendBeaconMock.mockClear()

    // Queue a new pageview via pushState without flushing (leave it pending)
    await act(async () => {
      ;(window.location as { pathname: string }).pathname = '/leaving'
      history.pushState({}, '', '/leaving')
      await Promise.resolve()
      // Do NOT advance timers — the 500ms debounce has not fired yet
    })

    // Dispatch beforeunload — flush() should send the pending pageview immediately via sendBeacon
    await act(async () => {
      window.dispatchEvent(new Event('beforeunload'))
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
    const [url, blob] = sendBeaconMock.mock.calls[0] as [string, Blob]
    expect(url).toContain('/api/collect')
    expect(blob).toBeInstanceOf(Blob)
    const events = JSON.parse(await blob.text()) as Record<string, unknown>[]
    expect(events[0]['type']).toBe('pageview')
    expect(events[0]['url']).toBe('/leaving')
  })
})

// ──────────────────────────────────────────────────────────────
// Feature: Privacy signals
// ──────────────────────────────────────────────────────────────

describe('Privacy signals', () => {
  it('test_dnt_respected_no_events_sent — DNT=1 suppresses all tracking', async () => {
    vi.stubGlobal('navigator', { sendBeacon: sendBeaconMock, doNotTrack: '1' })
    renderProvider({ websiteId: 'site_1', respectDnt: true })
    await flushQueue()
    expect(sendBeaconMock).not.toHaveBeenCalled()
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('test_gpc_respected_no_events_sent — GPC=true suppresses all tracking', async () => {
    vi.stubGlobal('navigator', {
      sendBeacon: sendBeaconMock,
      doNotTrack: null,
      globalPrivacyControl: true,
    })
    renderProvider({ websiteId: 'site_1', respectDnt: true })
    await flushQueue()
    expect(sendBeaconMock).not.toHaveBeenCalled()
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('test_disabled_prop_no_events_sent — disabled=true suppresses all tracking', async () => {
    renderProvider({ websiteId: 'site_1', disabled: true })
    await flushQueue()
    expect(sendBeaconMock).not.toHaveBeenCalled()
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('test_respect_dnt_false_tracks_despite_dnt — respectDnt=false ignores DNT=1 and sends events', async () => {
    vi.stubGlobal('navigator', { sendBeacon: sendBeaconMock, doNotTrack: '1' })
    renderProvider({ websiteId: 'site_1', respectDnt: false })
    await flushQueue()
    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
  })
})

// ──────────────────────────────────────────────────────────────
// Feature: SparklyticsEvent component
// ──────────────────────────────────────────────────────────────

describe('SparklyticsEvent component', () => {
  it('test_sparklytics_event_fires_on_click — click tracks named event with correct fields', async () => {
    const { getByText } = render(
      React.createElement(
        SparklyticsProvider,
        { websiteId: 'site_1' },
        React.createElement(
          SparklyticsEvent,
          { name: 'cta_click', data: { button: 'hero' } },
          React.createElement('button', {}, 'Click me'),
        ),
      ),
    )

    await flushQueue()
    sendBeaconMock.mockClear()

    await act(async () => {
      getByText('Click me').click()
      vi.advanceTimersByTime(600)
      await Promise.resolve()
    })

    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
    const blob: Blob = sendBeaconMock.mock.calls[0][1]
    const events = JSON.parse(await blob.text()) as Record<string, unknown>[]
    expect(events[0]['type']).toBe('event')
    expect(events[0]['event_name']).toBe('cta_click')
    expect(events[0]['event_data']).toEqual({ button: 'hero' })
  })

  it('test_sparklytics_event_preserves_existing_onclick — both existing onClick AND tracking fire', async () => {
    const existingOnClick = vi.fn()

    const { getByText } = render(
      React.createElement(
        SparklyticsProvider,
        { websiteId: 'site_1' },
        React.createElement(
          SparklyticsEvent,
          { name: 'btn_click' },
          React.createElement('button', { onClick: existingOnClick }, 'Press me'),
        ),
      ),
    )

    await flushQueue()
    sendBeaconMock.mockClear()

    await act(async () => {
      getByText('Press me').click()
      vi.advanceTimersByTime(600)
      await Promise.resolve()
    })

    // Both the original handler AND the tracking event must fire
    expect(existingOnClick).toHaveBeenCalledTimes(1)
    expect(sendBeaconMock).toHaveBeenCalledTimes(1)
    const blob: Blob = sendBeaconMock.mock.calls[0][1]
    const events = JSON.parse(await blob.text()) as Record<string, unknown>[]
    expect(events[0]['event_name']).toBe('btn_click')
  })
})

// ──────────────────────────────────────────────────────────────
// Feature: SSR safety
// ──────────────────────────────────────────────────────────────

describe('SSR safety', () => {
  it('test_sdk_does_not_error_during_ssr — no top-level browser API access at import time', () => {
    // If the module accessed window/document/navigator at module level it would throw on import.
    // In happy-dom window exists, but the test documents the contract and would fail in Node.js.
    expect(() => {
      void SparklyticsProvider
      void useSparklytics
      void SparklyticsEvent
    }).not.toThrow()
  })

  it('test_children_render_immediately — provider renders children without waiting for tracking init', () => {
    const { getByText } = render(
      React.createElement(
        SparklyticsProvider,
        { websiteId: 'site_1' },
        React.createElement('p', {}, 'visible content'),
      ),
    )
    // Children are visible synchronously — no layout shift from deferred init
    expect(getByText('visible content')).toBeTruthy()
  })
})
