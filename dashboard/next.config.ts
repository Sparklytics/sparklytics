import type { NextConfig } from 'next';

const isDev = process.env.NODE_ENV === 'development';

const nextConfig: NextConfig = {
  // Static export for production (embedded in Rust binary).
  // Dev runs without export so rewrites/HMR work normally.
  ...(isDev ? {} : { output: 'export' }),
  trailingSlash: true,
  images: { unoptimized: true },
  env: {
    NEXT_PUBLIC_AUTH_MODE:
      process.env.NEXT_PUBLIC_AUTH_MODE ?? process.env.SPARKLYTICS_AUTH ?? '',
  },
  // Proxy /api to the Rust server in development only
  ...(isDev
    ? {
        async rewrites() {
          return [
            {
              source: '/api/:path*',
              destination: 'http://localhost:3000/api/:path*',
            },
          ];
        },
      }
    : {}),
};

export default nextConfig;
