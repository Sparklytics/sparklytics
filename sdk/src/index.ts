'use client'

import React, { createContext, useContext, useEffect, useRef } from 'react'

// ============================================================
// Types
// ============================================================

export interface SparklyticsProviderProps {
  /** Required. The website UUID from your Sparklytics dashboard. */
  websiteId: string
  /**
   * Optional. Base URL of your Sparklytics server.
   * SDK appends /api/collect automatically.
   * Do NOT pass the full collect URL — that will result in a double path.
   * Example: "https://analytics.example.com"
   */
  /**
   * Optional. Base URL of your Sparklytics server.
   * SDK appends /api/collect automatically.
   * Do NOT pass the full collect URL — that will result in a double path.
   * Example: "https://analytics.example.com"
   *
   * Defaults to '' (same-origin relative path → /api/collect).
   * Auto-detection from the script's src attribute is deferred — the self-hosted
   * target (Next.js app on the same origin as the analytics server) does not need it.
   */
  endpoint?: string
  /** Optional. Respect DNT and GPC signals. Default: true. */
  respectDnt?: boolean
  /**
   * Optional. CSP nonce for inline scripts.
   * Declared for forward-compatibility. Currently a no-op: the SDK uses fetch/sendBeacon
   * for all tracking (no inline <script> injection), so there is no element to attach
   * the nonce to. Will be consumed when/if script injection is added.
   */
  nonce?: string
  /** Optional. Disable all tracking (e.g. for dev/staging). Default: false. */
  disabled?: boolean
  children: React.ReactNode
}

export interface SparklyticsHook {
  /**
   * Track a custom event.
   * eventName: max 50 chars, alphanumeric + underscores recommended.
   * eventData: max 4KB when JSON-serialized, max 1 level of nesting recommended.
   */
  track: (eventName: string, eventData?: Record<string, unknown>) => void
}

// ============================================================
// Batch event shape (internal)
// ============================================================

export interface BatchEvent {
  website_id: string
  type: 'pageview' | 'event'
  url: string
  referrer?: string
  event_name?: string
  event_data?: Record<string, unknown>
}

// ============================================================
// Privacy signal check (DNT + GPC)
// ============================================================

function isPrivacyBlocked(respectDnt: boolean): boolean {
  if (!respectDnt) return false
  if (typeof navigator === 'undefined') return false
  if (navigator.doNotTrack === '1') return true
  if ((navigator as unknown as { globalPrivacyControl?: boolean }).globalPrivacyControl === true) return true
  return false
}

// ============================================================
// Context — default is a no-op (safe for SSR / Server Components)
// ============================================================

const SparklyticsContext = createContext<SparklyticsHook>({
  track: () => {},
})

// ============================================================
// Provider
// ============================================================

export function SparklyticsProvider({
  websiteId,
  endpoint = '',
  respectDnt = true,
  disabled = false,
  children,
}: SparklyticsProviderProps) {
  // Validate websiteId at runtime and fail gracefully
  if (!websiteId) {
    if (typeof console !== 'undefined') {
      console.error('[Sparklytics] websiteId is required. No events will be sent.')
    }
  }

  const collectUrl = endpoint ? `${endpoint}/api/collect` : '/api/collect'
  const queueRef = useRef<BatchEvent[]>([])
  const flushTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const blockedRef = useRef<boolean>(false)
  // Keep collectUrl accessible inside stable refs without stale closures
  const collectUrlRef = useRef(collectUrl)
  collectUrlRef.current = collectUrl
  // Dedup tracker: prevents double-pageview when both history.pushState monkey-patch
  // and next/router routeChangeComplete fire for the same Pages Router navigation.
  // A 100ms window is narrow enough to catch near-simultaneous fires and wide enough
  // not to suppress genuine rapid navigations to different URLs.
  const lastPageviewRef = useRef<{ url: string; ts: number } | null>(null)

  // Determine tracking eligibility (SSR-safe)
  useEffect(() => {
    blockedRef.current =
      !websiteId || disabled || isPrivacyBlocked(respectDnt)
  }, [websiteId, disabled, respectDnt])

  // Flush the queue to the server
  const flush = useRef(async () => {
    if (flushTimerRef.current) {
      clearTimeout(flushTimerRef.current)
      flushTimerRef.current = null
    }
    if (blockedRef.current || queueRef.current.length === 0) return

    const batch = queueRef.current.splice(0)

    const send = async () => {
      const body = JSON.stringify(batch)
      if (typeof navigator !== 'undefined' && navigator.sendBeacon) {
        // Must send as Blob with application/json — raw string sends text/plain which the server rejects
        navigator.sendBeacon(
          collectUrlRef.current,
          new Blob([body], { type: 'application/json' }),
        )
      } else {
        await fetch(collectUrlRef.current, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body,
          keepalive: true,
        })
      }
    }

    try {
      await send()
    } catch {
      // Retry once after 2 seconds, then drop — events are fire-and-forget
      setTimeout(async () => {
        try {
          await send()
        } catch {
          // Drop silently — never throw on the host page
        }
      }, 2000)
    }
  })

  // Enqueue an event and schedule a flush
  const enqueue = (event: BatchEvent) => {
    if (blockedRef.current) return

    // Dedup: skip pageview if same URL was enqueued within the last 100ms.
    // Prevents double-pageview when history.pushState monkey-patch and
    // next/router routeChangeComplete both fire for the same Pages Router navigation.
    if (event.type === 'pageview') {
      const now = Date.now()
      if (
        lastPageviewRef.current?.url === event.url &&
        now - lastPageviewRef.current.ts < 100
      ) {
        return
      }
      lastPageviewRef.current = { url: event.url, ts: now }
    }

    queueRef.current.push(event)

    // Flush immediately if batch reaches 10 events
    if (queueRef.current.length >= 10) {
      void flush.current()
      return
    }

    // Otherwise debounce: flush 500ms after first event in batch
    if (!flushTimerRef.current) {
      flushTimerRef.current = setTimeout(() => {
        void flush.current()
      }, 500)
    }
  }

  // Track pageview on mount; wire beforeunload and SPA navigation
  useEffect(() => {
    blockedRef.current =
      !websiteId || disabled || isPrivacyBlocked(respectDnt)

    if (blockedRef.current) return

    // Initial pageview
    enqueue({
      website_id: websiteId,
      type: 'pageview',
      url: window.location.pathname,
      referrer: document.referrer || undefined,
    })

    // Flush on tab close (best-effort via sendBeacon)
    const handleUnload = () => { void flush.current() }
    window.addEventListener('beforeunload', handleUnload)

    // SPA navigation detection via History.pushState monkey-patch.
    // Catches all SPA navigations including App Router and Pages Router transitions.
    const originalPushState = history.pushState.bind(history)
    history.pushState = (...args: Parameters<typeof history.pushState>) => {
      originalPushState(...args)
      enqueue({
        website_id: websiteId,
        type: 'pageview',
        url: window.location.pathname,
        referrer: document.referrer || undefined,
      })
    }

    // Also handle popstate (back/forward)
    const handlePopState = () => {
      enqueue({
        website_id: websiteId,
        type: 'pageview',
        url: window.location.pathname,
        referrer: document.referrer || undefined,
      })
    }
    window.addEventListener('popstate', handlePopState)

    // Pages Router: listen to routeChangeComplete for router.replace() / shallow routing
    // Dynamic import avoids breaking App Router builds where next/router is not in use
    let cleanupPagesRouter: (() => void) | null = null
    import('next/router')
      .then(mod => {
        const router = mod.default
        const handleRouteChange = (url: string) => {
          enqueue({
            website_id: websiteId,
            type: 'pageview',
            url,
            referrer: document.referrer || undefined,
          })
        }
        router.events?.on('routeChangeComplete', handleRouteChange)
        cleanupPagesRouter = () => {
          router.events?.off('routeChangeComplete', handleRouteChange)
        }
      })
      .catch(() => {
        // next/router not available (App Router project) — pushState monkey-patch handles it
      })

    return () => {
      window.removeEventListener('beforeunload', handleUnload)
      window.removeEventListener('popstate', handlePopState)
      history.pushState = originalPushState
      cleanupPagesRouter?.()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [websiteId, disabled, respectDnt])

  // Custom event tracker exposed via hook
  const track = (eventName: string, eventData?: Record<string, unknown>) => {
    enqueue({
      website_id: websiteId,
      type: 'event',
      url: typeof window !== 'undefined' ? window.location.pathname : '/',
      referrer: typeof document !== 'undefined' ? document.referrer || undefined : undefined,
      event_name: eventName,
      event_data: eventData,
    })
  }

  return React.createElement(
    SparklyticsContext.Provider,
    { value: { track } },
    React.createElement(AppRouterTracker, {
      websiteId,
      // Note: blockedRef.current is read at render time (before the blocking useEffect runs),
      // so this prop may be stale on the first render. AppRouterTracker is currently a stub;
      // future usePathname()-based implementations should read blockedRef via a callback instead.
      disabled: blockedRef.current,
      onNavigate: (url: string) => {
        enqueue({
          website_id: websiteId,
          type: 'pageview',
          url,
          referrer: typeof document !== 'undefined' ? document.referrer || undefined : undefined,
        })
      },
    }),
    children,
  )
}

// ============================================================
// AppRouterTracker — internal child component
// Placeholder for App Router navigation detection.
// Primary navigation tracking (pushState + popstate) is handled
// in SparklyticsProvider's main useEffect, which captures all
// Next.js App Router <Link> navigations (App Router uses pushState
// internally for soft navigations).
// This component exists as an extension point for future
// usePathname()-based tracking once Next.js types are added.
// ============================================================

interface AppRouterTrackerProps {
  websiteId: string
  disabled: boolean
  onNavigate: (url: string) => void
}

function AppRouterTracker(_props: AppRouterTrackerProps) {
  return null
}

// ============================================================
// useSparklytics hook
// Safe to call in Client Components.
// Returns no-op context default when called outside a Provider.
// ============================================================

export function useSparklytics(): SparklyticsHook {
  return useContext(SparklyticsContext)
}

// ============================================================
// SparklyticsEvent — declarative click tracker
// ============================================================

export interface SparklyticsEventProps {
  /** Event name to track on click. Max 50 chars. */
  name: string
  /** Optional event payload. Max 4KB JSON-serialized. */
  data?: Record<string, unknown>
  /** Must be a single React element. */
  children: React.ReactElement
}

export function SparklyticsEvent({ name, data, children }: SparklyticsEventProps) {
  const { track } = useSparklytics()
  const child = React.Children.only(children)

  return React.cloneElement(child, {
    onClick: (e: React.MouseEvent) => {
      track(name, data)
      // Preserve the child's existing onClick if present
      if (typeof child.props.onClick === 'function') {
        child.props.onClick(e)
      }
    },
  } as React.HTMLAttributes<HTMLElement>)
}
