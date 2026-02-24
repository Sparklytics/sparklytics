export type RuntimeAuthMode = 'none' | 'password' | 'local';

declare global {
  interface Window {
    __SPARKLYTICS_AUTH_MODE__?: string;
  }
}

export function getRuntimeAuthMode(): RuntimeAuthMode | null {
  if (typeof window !== 'undefined') {
    const runtimeMode = window.__SPARKLYTICS_AUTH_MODE__;
    if (
      runtimeMode === 'none' ||
      runtimeMode === 'password' ||
      runtimeMode === 'local'
    ) {
      return runtimeMode;
    }
  }

  const envMode = process.env.NEXT_PUBLIC_AUTH_MODE;
  if (envMode === 'none' || envMode === 'password' || envMode === 'local') {
    return envMode;
  }

  return null;
}
