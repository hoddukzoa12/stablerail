import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  serverExternalPackages: ["ws"],
  transpilePackages: [
    "@solana/kit",
    "@solana/keys",
    "@solana/addresses",
    "@solana/codecs-strings",
  ],
};

export default nextConfig;
