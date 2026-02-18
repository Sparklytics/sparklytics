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
  endpoint?: string
  /** Optional. Respect DNT and GPC signals. Default: true. */
  respectDnt?: boolean
  /** Optional. CSP nonce for inline scripts. */
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

interface BatchEvent {
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
        navigator.sendBeacon(collectUrl, body)
      } else {
        await fetch(collectUrl, {
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

    // SPA navigation detection via History.pushState monkey-patch
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

    return () => {
      window.removeEventListener('beforeunload', handleUnload)
      window.removeEventListener('popstate', handlePopState)
      // Restore original pushState
      history.pushState = originalPushState
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
    children,
  )
}

// ============================================================
// useSparklytics hook
// Safe to call in Server Components — returns no-op on server.
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
