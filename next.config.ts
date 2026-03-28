import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  serverExternalPackages: ["ws"],
  webpack: (config, { isServer }) => {
    if (!isServer) {
      // Prevent Webpack from splitting @solana packages across chunks.
      // The base58 alphabet constant loses its reference during code-splitting,
      // causing "ReferenceError: alphabet4 is not defined" in production.
      config.optimization = {
        ...config.optimization,
        splitChunks: {
          ...config.optimization?.splitChunks,
          cacheGroups: {
            ...(typeof config.optimization?.splitChunks === "object"
              ? config.optimization.splitChunks.cacheGroups
              : {}),
            solana: {
              test: /[\\/]node_modules[\\/]@solana[\\/]/,
              name: "solana-vendor",
              chunks: "all" as const,
              priority: 20,
            },
          },
        },
      };
    }
    return config;
  },
};

export default nextConfig;
