"use client";

import { useState, useRef, useEffect } from "react";
import Link from "next/link";
import Image from "next/image";
import { usePathname } from "next/navigation";
import { useWalletConnection } from "@solana/react-hooks";
import { Menu, X, ChevronDown, LogOut, ExternalLink } from "lucide-react";
import { Button } from "../ui/button";

function PhantomIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 128 128" fill="none" xmlns="http://www.w3.org/2000/svg">
      <rect width="128" height="128" rx="26" fill="#AB9FF2"/>
      <path d="M110.584 64.914H99.142C99.142 41.047 79.893 21.798 56.026 21.798C32.159 21.798 14.163 41.047 14.163 64.914C14.163 67.387 14.379 69.86 14.594 72.333H18.536C33.843 72.333 46.464 59.712 46.464 44.405V42.148C50.839 40.107 55.862 38.714 61.101 38.714C79.893 38.714 95.2 54.021 95.2 72.813V74.854C95.2 79.013 98.494 82.307 102.653 82.307H110.584C114.743 82.307 118.037 79.013 118.037 74.854V72.813C118.037 68.654 114.743 64.914 110.584 64.914Z" fill="white"/>
      <circle cx="44.5" cy="62.5" r="5.5" fill="#4C3F8C"/>
      <circle cx="72.5" cy="62.5" r="5.5" fill="#4C3F8C"/>
    </svg>
  );
}

function MetaMaskIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 128 128" fill="none" xmlns="http://www.w3.org/2000/svg">
      <rect width="128" height="128" rx="26" fill="#F6851B"/>
      <path d="M100 38L68 62l6-14z" fill="#E2761B" stroke="#E2761B"/>
      <path d="M28 38l31.6 24.3L54 48z" fill="#E4761B" stroke="#E4761B"/>
      <path d="M89 83l-8.5 13 18.2 5L102 84z" fill="#E4761B" stroke="#E4761B"/>
      <path d="M26.3 84l3.2 17 18.2-5L39 83z" fill="#E4761B" stroke="#E4761B"/>
      <path d="M46.6 58.6L40 68.5l18 .8-.7-19.6z" fill="#E4761B" stroke="#E4761B"/>
      <path d="M81.4 58.6L70.4 49l-.4 20.3 18-.8z" fill="#E4761B" stroke="#E4761B"/>
      <path d="M47.7 96L57 91.5l-8-6.2z" fill="#E4761B" stroke="#E4761B"/>
      <path d="M71 91.5l9.3 4.5-1.3-10.7z" fill="#E4761B" stroke="#E4761B"/>
    </svg>
  );
}

const FEATURED_WALLETS = [
  {
    id: "phantom",
    name: "Phantom",
    matchNames: ["phantom"],
    Icon: PhantomIcon,
    downloadUrl: "https://phantom.app/download",
  },
  {
    id: "metamask",
    name: "MetaMask",
    matchNames: ["metamask"],
    Icon: MetaMaskIcon,
    downloadUrl: "https://metamask.io/download/",
  },
] as const;

const NAV_LINKS = [
  { href: "/", label: "Swap" },
  { href: "/dashboard", label: "Dashboard" },
  { href: "/settlement", label: "Settlement" },
  { href: "/admin", label: "Admin" },
] as const;

function truncateAddress(address: string): string {
  if (address.length <= 8) return address;
  return `${address.slice(0, 4)}...${address.slice(-4)}`;
}

export function Navbar() {
  const pathname = usePathname();
  const { connectors, connect, disconnect, wallet, status } = useWalletConnection();
  const [mobileOpen, setMobileOpen] = useState(false);
  const [walletMenuOpen, setWalletMenuOpen] = useState(false);
  const [connectorMenuOpen, setConnectorMenuOpen] = useState(false);
  const walletMenuRef = useRef<HTMLDivElement>(null);
  const connectorMenuRef = useRef<HTMLDivElement>(null);

  const isConnected = status === "connected" && wallet;
  const address = wallet?.account.address.toString() ?? "";

  // Close dropdown when clicking outside
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (walletMenuRef.current && !walletMenuRef.current.contains(e.target as Node)) {
        setWalletMenuOpen(false);
      }
      if (connectorMenuRef.current && !connectorMenuRef.current.contains(e.target as Node)) {
        setConnectorMenuOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  return (
    <nav className="fixed top-0 left-0 right-0 z-50 h-16 border-b border-border-subtle bg-surface-base/80 backdrop-blur-lg">
      <div className="mx-auto flex h-full max-w-7xl items-center justify-between px-4 sm:px-6">
        {/* Left: Logo + Brand */}
        <Link href="/" className="flex items-center gap-2.5">
          <Image src="/logo.png" alt="StableRail" width={32} height={32} className="rounded-md" />
          <span className="text-lg font-semibold text-text-primary">StableRail</span>
        </Link>

        {/* Center: Navigation links (hidden on mobile) */}
        <div className="hidden items-center gap-1 md:flex">
          {NAV_LINKS.map((link) => {
            const isActive = pathname === link.href;
            return (
              <Link
                key={link.href}
                href={link.href}
                className={`relative px-3 py-2 text-sm font-medium transition-colors duration-200 ${
                  isActive
                    ? "text-text-primary"
                    : "text-text-secondary hover:text-text-primary"
                }`}
              >
                {link.label}
                {isActive && (
                  <span className="absolute bottom-0 left-3 right-3 h-0.5 rounded-full bg-brand-primary" />
                )}
              </Link>
            );
          })}
        </div>

        {/* Right: Wallet + Mobile menu */}
        <div className="flex items-center gap-3">
          {/* Wallet Section */}
          {isConnected ? (
            <div className="relative" ref={walletMenuRef}>
              <button
                onClick={() => setWalletMenuOpen(!walletMenuOpen)}
                className="flex items-center gap-2 rounded-full bg-surface-2 px-3 py-1.5 text-sm font-mono text-text-primary transition-colors hover:bg-surface-3 cursor-pointer"
              >
                <span className="h-2 w-2 rounded-full bg-success" />
                {truncateAddress(address)}
                <ChevronDown className="h-3.5 w-3.5 text-text-tertiary" />
              </button>

              {walletMenuOpen && (
                <div className="absolute right-0 mt-2 w-48 rounded-lg border border-border-default bg-surface-1 py-1 shadow-lg">
                  <button
                    onClick={() => {
                      disconnect();
                      setWalletMenuOpen(false);
                    }}
                    className="flex w-full items-center gap-2 px-4 py-2.5 text-sm text-text-secondary transition-colors hover:bg-surface-2 hover:text-text-primary cursor-pointer"
                  >
                    <LogOut className="h-4 w-4" />
                    Disconnect
                  </button>
                </div>
              )}
            </div>
          ) : (
            <div className="relative" ref={connectorMenuRef}>
              <Button
                variant="primary"
                size="sm"
                onClick={() => setConnectorMenuOpen(!connectorMenuOpen)}
                disabled={status === "connecting"}
              >
                {status === "connecting" ? "Connecting..." : "Connect Wallet"}
              </Button>

              {connectorMenuOpen && (
                <div className="absolute right-0 mt-2 w-64 rounded-xl border border-border-default bg-surface-1 py-2 shadow-lg">
                  <p className="px-4 pb-2 text-xs font-medium uppercase tracking-wider text-text-tertiary">
                    Connect a wallet
                  </p>

                  {/* Featured wallets — always visible */}
                  {FEATURED_WALLETS.map((featured) => {
                    const installed = connectors.find((c) =>
                      featured.matchNames.some((m) =>
                        c.name.toLowerCase().includes(m)
                      )
                    );

                    if (installed) {
                      return (
                        <button
                          key={featured.id}
                          onClick={() => {
                            connect(installed.id);
                            setConnectorMenuOpen(false);
                          }}
                          className="flex w-full items-center gap-3 px-4 py-2.5 text-sm text-text-primary transition-colors hover:bg-surface-2 cursor-pointer"
                        >
                          <featured.Icon className="h-6 w-6 rounded-md" />
                          <span className="flex-1 text-left">{featured.name}</span>
                          <span className="rounded-full bg-success/15 px-2 py-0.5 text-[10px] font-medium text-success">
                            Detected
                          </span>
                        </button>
                      );
                    }

                    return (
                      <a
                        key={featured.id}
                        href={featured.downloadUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="flex w-full items-center gap-3 px-4 py-2.5 text-sm text-text-secondary transition-colors hover:bg-surface-2 hover:text-text-primary"
                      >
                        <featured.Icon className="h-6 w-6 rounded-md opacity-50" />
                        <span className="flex-1 text-left">{featured.name}</span>
                        <span className="flex items-center gap-1 text-[10px] text-text-tertiary">
                          Install <ExternalLink className="h-3 w-3" />
                        </span>
                      </a>
                    );
                  })}

                  {/* Other discovered connectors */}
                  {connectors.filter(
                    (c) =>
                      !FEATURED_WALLETS.some((f) =>
                        f.matchNames.some((m) => c.name.toLowerCase().includes(m))
                      )
                  ).length > 0 && (
                    <>
                      <div className="my-2 border-t border-border-subtle" />
                      <p className="px-4 pb-1 text-[10px] font-medium uppercase tracking-wider text-text-tertiary">
                        Other wallets
                      </p>
                      {connectors
                        .filter(
                          (c) =>
                            !FEATURED_WALLETS.some((f) =>
                              f.matchNames.some((m) => c.name.toLowerCase().includes(m))
                            )
                        )
                        .map((connector) => (
                          <button
                            key={connector.id}
                            onClick={() => {
                              connect(connector.id);
                              setConnectorMenuOpen(false);
                            }}
                            className="flex w-full items-center gap-3 px-4 py-2.5 text-sm text-text-secondary transition-colors hover:bg-surface-2 hover:text-text-primary cursor-pointer"
                          >
                            <span className="flex h-6 w-6 items-center justify-center rounded-md bg-surface-3 text-xs">
                              ?
                            </span>
                            {connector.name}
                          </button>
                        ))}
                    </>
                  )}
                </div>
              )}
            </div>
          )}

          {/* Mobile hamburger */}
          <button
            className="flex items-center justify-center rounded-md p-2 text-text-secondary hover:bg-surface-2 hover:text-text-primary md:hidden cursor-pointer"
            onClick={() => setMobileOpen(!mobileOpen)}
            aria-label="Toggle navigation"
          >
            {mobileOpen ? <X className="h-5 w-5" /> : <Menu className="h-5 w-5" />}
          </button>
        </div>
      </div>

      {/* Mobile navigation drawer */}
      {mobileOpen && (
        <div className="border-b border-border-subtle bg-surface-base/95 backdrop-blur-lg md:hidden">
          <div className="flex flex-col gap-1 px-4 py-3">
            {NAV_LINKS.map((link) => {
              const isActive = pathname === link.href;
              return (
                <Link
                  key={link.href}
                  href={link.href}
                  onClick={() => setMobileOpen(false)}
                  className={`rounded-md px-3 py-2.5 text-sm font-medium transition-colors ${
                    isActive
                      ? "bg-surface-2 text-text-primary"
                      : "text-text-secondary hover:bg-surface-2 hover:text-text-primary"
                  }`}
                >
                  {link.label}
                </Link>
              );
            })}
          </div>
        </div>
      )}
    </nav>
  );
}
