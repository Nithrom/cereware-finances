/** Logo Cf: la C usa el color de texto del tema; f, rabo y hoja en granate; cereza roja. */
export function LogoMark({ size = 40 }: { size?: number }) {
  return (
    <svg viewBox="10 8 88 88" width={size} height={size} aria-hidden="true">
      <path d="M60 21a30 30 0 1 0 0 54" fill="none" stroke="currentColor" strokeWidth="9" strokeLinecap="round" />
      <path d="M56 76V40a12 12 0 0 1 12-12h5M47 50h20" fill="none" stroke="var(--brand-granate)" strokeWidth="9" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M65 77c4-8 9-12 15-12" fill="none" stroke="var(--brand-granate)" strokeWidth="3" strokeLinecap="round" />
      <path d="M80 65c6-2 11 0 13 5-6 2-11 1-13-5z" fill="var(--brand-granate)" transform="rotate(-30 80 65)" />
      <circle cx="61" cy="78" r="9" fill="var(--brand-cherry)" />
      <circle cx="58" cy="75" r="2.2" fill="#fff" opacity=".5" />
    </svg>
  );
}

export function Logo({ size = 40 }: { size?: number }) {
  return (
    <div className="logo">
      <LogoMark size={size} />
      <div>
        <b>CereWare</b>
        <span>FINANCES</span>
      </div>
    </div>
  );
}
