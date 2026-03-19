export function buildTrackingSnippet(websiteId: string, analyticsOrigin: string): string {
  const origin = analyticsOrigin.trim().replace(/\/+$/, '');
  return `<script defer src="${origin}/s.js" data-website-id="${websiteId}"></script>`;
}
