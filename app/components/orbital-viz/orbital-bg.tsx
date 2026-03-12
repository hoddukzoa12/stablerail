/**
 * OrbitalBg — Decorative background ring animation.
 * A large, thin elliptical ring that slowly rotates, giving a
 * subtle "planetary ring" feel to the Celestial Finance theme.
 *
 * Pure CSS, no JS interactivity. Fixed position, pointer-events-none.
 */
export function OrbitalBg() {
  return (
    <div
      aria-hidden="true"
      className="pointer-events-none fixed inset-0 z-0 flex items-center justify-center overflow-hidden"
    >
      {/* Outer elliptical ring */}
      <div
        className="absolute"
        style={{
          width: 800,
          height: 400,
          borderRadius: "50%",
          border: "1px solid transparent",
          backgroundImage:
            "linear-gradient(var(--surface-base), var(--surface-base)), linear-gradient(135deg, #9945FF, #14F195)",
          backgroundOrigin: "border-box",
          backgroundClip: "padding-box, border-box",
          opacity: 0.05,
          animation: "orbital-rotate 60s linear infinite",
          transform: "rotate(-15deg)",
        }}
      />

      {/* Inner secondary ring — slightly smaller, opposite rotation offset */}
      <div
        className="absolute"
        style={{
          width: 650,
          height: 320,
          borderRadius: "50%",
          border: "1px solid transparent",
          backgroundImage:
            "linear-gradient(var(--surface-base), var(--surface-base)), linear-gradient(135deg, #14F195, #9945FF)",
          backgroundOrigin: "border-box",
          backgroundClip: "padding-box, border-box",
          opacity: 0.035,
          animation: "orbital-rotate 80s linear infinite reverse",
          transform: "rotate(10deg)",
        }}
      />
    </div>
  );
}
