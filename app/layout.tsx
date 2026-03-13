import type { Metadata } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import "./globals.css";
import { Providers } from "./components/providers";
import { Navbar } from "./components/layout/navbar";
import { AmbientBg } from "./components/ambient-viz/ambient-bg";

const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin"],
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-jetbrains-mono",
  subsets: ["latin"],
  display: "swap",
});

export const metadata: Metadata = {
  title: "StableRail",
  description: "Multi-asset stablecoin AMM on Solana",
  icons: {
    icon: "/favicon-32x32.png",
    shortcut: "/favicon-16x16.png",
    apple: "/apple-touch-icon.png",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="dark">
      <body
        suppressHydrationWarning
        className={`${inter.variable} ${jetbrainsMono.variable} bg-surface-base text-text-primary antialiased`}
      >
        <Providers>
          <AmbientBg />
          <Navbar />
          <main className="relative z-10 pt-16">{children}</main>
        </Providers>
      </body>
    </html>
  );
}
