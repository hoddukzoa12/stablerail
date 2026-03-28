import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  serverExternalPackages: ["ws"],
  // Force Webpack bundler — Turbopack breaks @solana/codecs-strings
  // base58 alphabet variable during code-splitting (alphabet4 ReferenceError)
  webpack: (config) => {
    // Keep all @solana modules in one chunk to preserve variable references
    if (!config.resolve) config.resolve = {};
    config.resolve.fallback = {
      ...config.resolve.fallback,
      fs: false,
      net: false,
      tls: false,
    };
    return config;
  },
};

export default nextConfig;
