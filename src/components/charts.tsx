import { useEffect, useMemo, useRef, useState } from "react";
import { money } from "../lib/format";

/** Ancho real del contenedor para que el texto del SVG no se escale. */
function useWidth() {
  const ref = useRef<HTMLDivElement>(null);
  const [w, setW] = useState(700);
  useEffect(() => {
    if (!ref.current) return;
    const ro = new ResizeObserver(([e]) => setW(Math.max(200, Math.round(e.contentRect.width))));
    ro.observe(ref.current);
    return () => ro.disconnect();
  }, []);
  return [ref, w] as const;
}

function niceRange(min: number, max: number) {
  if (min === max) { min -= 1000; max += 1000; }
  const pad = (max - min) * 0.12;
  return [min - pad, max + pad];
}

/** Línea con área. Los puntos desde `futureFrom` se dibujan discontinuos (previsión). */
export function LineArea({ points, height, futureFrom, labelEvery = 1, color = "var(--chart)" }: {
  points: { label: string; value: number; tip?: string }[]; height?: number; futureFrom?: number; labelEvery?: number; color?: string;
}) {
  const [hover, setHover] = useState<number | null>(null);
  const [ref, W] = useWidth();
  const H = height ?? Math.round(Math.min(320, Math.max(200, W * 0.28))), top = 12, bottom = 26;
  const { xs, ys, lo, hi } = useMemo(() => {
    const vals = points.map((p) => p.value);
    const [lo, hi] = niceRange(Math.min(...vals, 0 < Math.min(...vals) ? Math.min(...vals) : Math.min(...vals)), Math.max(...vals));
    const xs = points.map((_, i) => (points.length === 1 ? W / 2 : (i / (points.length - 1)) * (W - 8) + 4));
    const ys = vals.map((v) => top + (1 - (v - lo) / (hi - lo)) * (H - top - bottom));
    return { xs, ys, lo, hi };
  }, [points, H, W]);
  if (!points.length) return null;
  const split = futureFrom ?? points.length - 1;
  const path = (a: number, b: number) => xs.slice(a, b + 1).map((x, i) => `${i ? "L" : "M"}${x.toFixed(1)} ${ys[a + i].toFixed(1)}`).join(" ");
  const zeroY = top + (1 - (0 - lo) / (hi - lo)) * (H - top - bottom);
  const gid = useMemo(() => "g" + Math.random().toString(36).slice(2, 8), []);
  const onMove = (e: React.MouseEvent<SVGSVGElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const x = ((e.clientX - r.left) / r.width) * W;
    let best = 0;
    xs.forEach((xx, i) => { if (Math.abs(xx - x) < Math.abs(xs[best] - x)) best = i; });
    setHover(best);
  };
  return (
    <div className="chart-box" ref={ref}>
      <svg viewBox={`0 0 ${W} ${H}`} width={W} height={H} onMouseMove={onMove} onMouseLeave={() => setHover(null)} role="img">
        <defs>
          <linearGradient id={gid} x1="0" x2="0" y1="0" y2="1">
            <stop offset="0" stopColor={color} stopOpacity=".25" /><stop offset="1" stopColor={color} stopOpacity="0" />
          </linearGradient>
        </defs>
        {[0.25, 0.5, 0.75].map((f) => <line key={f} x1="0" x2={W} y1={top + f * (H - top - bottom)} y2={top + f * (H - top - bottom)} stroke="var(--border)" />)}
        {lo < 0 && hi > 0 && <line x1="0" x2={W} y1={zeroY} y2={zeroY} stroke="var(--expense)" strokeDasharray="3 4" opacity=".6" />}
        <path d={`${path(0, split)} L${xs[split]} ${H - bottom} L${xs[0]} ${H - bottom}Z`} fill={`url(#${gid})`} />
        <path d={path(0, split)} fill="none" stroke={color} strokeWidth="2.5" strokeLinejoin="round" strokeLinecap="round" />
        {split < points.length - 1 && <path d={path(split, points.length - 1)} fill="none" stroke={color} strokeWidth="2.5" strokeDasharray="6 6" opacity=".65" />}
        {futureFrom !== undefined && <circle cx={xs[split]} cy={ys[split]} r="5.5" fill="var(--brand-cherry)" stroke="var(--surface)" strokeWidth="3" />}
        {points.map((p, i) => (i % labelEvery === 0 || i === points.length - 1) && (
          <text key={i} x={xs[i]} y={H - 6} textAnchor={i === 0 ? "start" : i === points.length - 1 ? "end" : "middle"} fontSize="12" fill="var(--text-3)">{p.label}</text>
        ))}
        {hover !== null && <>
          <line x1={xs[hover]} x2={xs[hover]} y1={top} y2={H - bottom} stroke="var(--text-3)" strokeDasharray="2 3" />
          <circle cx={xs[hover]} cy={ys[hover]} r="4" fill={color} />
        </>}
      </svg>
      {hover !== null && (
        <div className="tip" style={{ left: `${(xs[hover] / W) * 100}%` }}>
          <b className="num">{money(points[hover].value)}</b><span>{points[hover].tip ?? points[hover].label}</span>
        </div>
      )}
    </div>
  );
}

export function Donut({ slices, total, label = "gastado", size = 160 }: { slices: { color: string; value: number; name: string }[]; total: number; label?: string; size?: number }) {
  const r = 44, C = 2 * Math.PI * r;
  let acc = 0;
  const sum = slices.reduce((s, x) => s + x.value, 0) || 1;
  return (
    <svg viewBox="0 0 120 120" width={size} height={size} className="donut" role="img">
      <circle cx="60" cy="60" r={r} fill="none" stroke="var(--surface-2)" strokeWidth="15" />
      <g transform="rotate(-90 60 60)" fill="none" strokeWidth="15">
        {slices.map((s, i) => {
          const len = (s.value / sum) * C;
          const el = <circle key={i} cx="60" cy="60" r={r} stroke={s.color} strokeDasharray={`${Math.max(len - 1.5, 0)} ${C}`} strokeDashoffset={-acc}><title>{s.name}</title></circle>;
          acc += len;
          return el;
        })}
      </g>
      <text x="60" y="58" textAnchor="middle" fontWeight="800" fontSize="13" fill="var(--text)">{money(total).replace(/,\d\d\s/, " ")}</text>
      <text x="60" y="73" textAnchor="middle" fontSize="8" fill="var(--text-3)">{label}</text>
    </svg>
  );
}

export function Bars({ data, height }: { data: { label: string; a: number; b: number }[]; height?: number }) {
  const [hover, setHover] = useState<number | null>(null);
  const [ref, W] = useWidth();
  const max = Math.max(1, ...data.flatMap((d) => [d.a, d.b]));
  const H = height ?? Math.round(Math.min(300, Math.max(190, W * 0.2))), bottom = 24, gw = W / Math.max(data.length, 1), bw = Math.min(34, gw / 3.2);
  return (
    <div className="chart-box" ref={ref}>
      <svg viewBox={`0 0 ${W} ${H}`} width={W} height={H} role="img" onMouseLeave={() => setHover(null)}>
        {[0.25, 0.5, 0.75].map((f) => <line key={f} x1="0" x2={W} y1={f * (H - bottom)} y2={f * (H - bottom)} stroke="var(--border)" />)}
        {data.map((d, i) => {
          const cx = gw * i + gw / 2;
          const ha = (d.a / max) * (H - bottom - 8), hb = (d.b / max) * (H - bottom - 8);
          return (
            <g key={i} onMouseEnter={() => setHover(i)}>
              <rect x={cx - gw / 2} y="0" width={gw} height={H} fill="transparent" />
              <rect x={cx - bw - 2} y={H - bottom - ha} width={bw} height={ha} rx="5" fill="var(--income)" opacity={hover === null || hover === i ? 0.9 : 0.4} />
              <rect x={cx + 2} y={H - bottom - hb} width={bw} height={hb} rx="5" fill="var(--expense)" opacity={hover === null || hover === i ? 0.9 : 0.4} />
              <text x={cx} y={H - 6} textAnchor="middle" fontSize="12" fill="var(--text-3)">{d.label}</text>
            </g>
          );
        })}
      </svg>
      {hover !== null && (
        <div className="tip" style={{ left: `${((hover + 0.5) / data.length) * 100}%` }}>
          <b className="num inc">{money(data[hover].a, { sign: true })}</b><b className="num exp">{money(-data[hover].b)}</b><span>{data[hover].label}</span>
        </div>
      )}
    </div>
  );
}

export function Progress({ value, max, color }: { value: number; max: number; color?: string }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  const c = color ?? (pct >= 100 ? "var(--expense)" : pct >= 80 ? "var(--warn)" : "var(--primary)");
  return <div className="bar"><i style={{ width: `${pct}%`, background: c }} /></div>;
}

export function Ring({ value, max, color = "var(--brand-cherry)", size = 52 }: { value: number; max: number; color?: string; size?: number }) {
  const C = 2 * Math.PI * 15, pct = max > 0 ? Math.min(1, value / max) : 0;
  return (
    <svg viewBox="0 0 36 36" width={size} height={size}>
      <circle cx="18" cy="18" r="15" fill="none" stroke="var(--surface-2)" strokeWidth="5" />
      <circle cx="18" cy="18" r="15" fill="none" stroke={color} strokeWidth="5" strokeDasharray={`${pct * C} ${C}`} strokeLinecap="round" transform="rotate(-90 18 18)" />
      <text x="18" y="21" textAnchor="middle" fontSize="8" fontWeight="800" fill="var(--text)">{Math.round(pct * 100)}%</text>
    </svg>
  );
}
