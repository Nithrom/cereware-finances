import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  X, Home, KeyRound, Zap, Wifi, ShoppingCart, ShoppingBasket, Utensils, Car, Fuel, TrainFront, Gamepad2, Repeat,
  Plane, ShoppingBag, Cpu, Shirt, HeartPulse, Briefcase, Landmark, Receipt, Package, CircleEllipsis, Wallet,
  FileText, PlusCircle, ArrowLeftRight, Tag, Check, Search, Smartphone, Undo2, TrendingUp, Gift, Banknote, Shield,
  GraduationCap, PawPrint, type LucideIcon,
} from "lucide-react";
import type { Category } from "../lib/api";

const ICONS: Record<string, LucideIcon> = {
  home: Home, key: KeyRound, zap: Zap, wifi: Wifi, "shopping-cart": ShoppingCart, "shopping-basket": ShoppingBasket,
  utensils: Utensils, car: Car, fuel: Fuel, "train-front": TrainFront, "gamepad-2": Gamepad2, repeat: Repeat, plane: Plane,
  "shopping-bag": ShoppingBag, cpu: Cpu, shirt: Shirt, "heart-pulse": HeartPulse, briefcase: Briefcase, landmark: Landmark,
  receipt: Receipt, package: Package, "circle-ellipsis": CircleEllipsis, wallet: Wallet, "file-text": FileText,
  "plus-circle": PlusCircle, "arrow-left-right": ArrowLeftRight, smartphone: Smartphone, "undo-2": Undo2,
  "trending-up": TrendingUp, tag: Tag, gift: Gift, banknote: Banknote, shield: Shield, "graduation-cap": GraduationCap,
  "paw-print": PawPrint,
};

export function CatIcon({ icon, color, size = 36 }: { icon?: string | null; color?: string | null; size?: number }) {
  const I = (icon && ICONS[icon]) || Tag;
  const c = color || "var(--text-3)";
  return (
    <span className="ic" style={{ width: size, height: size, background: `color-mix(in srgb, ${c} 16%, transparent)`, color: c }}>
      <I size={Math.round(size * 0.5)} strokeWidth={2.2} />
    </span>
  );
}

export function Modal({ title, onClose, children, width = 520, footer }: {
  title: string; onClose: () => void; children: ReactNode; width?: number; footer?: ReactNode;
}) {
  useEffect(() => {
    const k = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [onClose]);
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal card" style={{ width }} role="dialog" aria-label={title}>
        <header className="modal-head">
          <h2>{title}</h2>
          <button className="icon-btn" onClick={onClose} aria-label="Cerrar"><X size={18} /></button>
        </header>
        <div className="modal-body">{children}</div>
        {footer && <footer className="modal-foot">{footer}</footer>}
      </div>
    </div>
  );
}

export function Field({ label, children, hint }: { label: string; children: ReactNode; hint?: string }) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
      {hint && <small className="muted">{hint}</small>}
    </label>
  );
}

/** Selector de categoría con buscador, agrupado por categoría padre. */
export function CategoryPicker({ categories, value, onChange, onClose, allowNone = true, sign = 0 }: {
  categories: Category[]; value: string | null; onChange: (id: string | null) => void; onClose: () => void; allowNone?: boolean;
  /** >0 ingreso (primero categorías de ingreso), <0 gasto */
  sign?: number;
}) {
  const [q, setQ] = useState("");
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const h = (e: MouseEvent) => ref.current && !ref.current.contains(e.target as Node) && onClose();
    const k = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    setTimeout(() => document.addEventListener("mousedown", h));
    window.addEventListener("keydown", k);
    return () => { document.removeEventListener("mousedown", h); window.removeEventListener("keydown", k); };
  }, [onClose]);
  const n = q.trim().toLowerCase();
  const list = categories.filter((c) => !n || c.name.toLowerCase().includes(n));
  const rank = (c: Category) => (sign > 0 ? (c.kind === "income" ? 0 : c.kind === "transfer" ? 1 : 2) : c.kind === "expense" ? 0 : c.kind === "transfer" ? 1 : 2);
  // Una categoría principal sube si ella o alguna subcategoría es del tipo del movimiento
  const prank = (p: Category) => Math.min(rank(p), ...categories.filter((c) => c.parent_id === p.id).map(rank));
  const parents = categories.filter((c) => !c.parent_id).sort((a, b) => (sign ? prank(a) - prank(b) : 0));
  const ordered = n ? list : parents.flatMap((p) => [p, ...categories.filter((c) => c.parent_id === p.id)]);
  return (
    <div className="picker card" ref={ref} onClick={(e) => e.stopPropagation()}>
      <div className="picker-search"><Search size={15} /><input autoFocus placeholder="Buscar categoría" value={q} onChange={(e) => setQ(e.target.value)} /></div>
      <div className="picker-list">
        {allowNone && !n && (
          <button className="picker-item" onClick={() => onChange(null)}><CatIcon size={24} /> Sin categoría {value === null && <Check size={15} className="ml" />}</button>
        )}
        {ordered.map((c) => (
          <button key={c.id} className={`picker-item ${c.parent_id && !n ? "child" : ""}`} onClick={() => onChange(c.id)}>
            <CatIcon icon={c.icon} color={c.color} size={24} /> {c.name}
            {value === c.id && <Check size={15} className="ml" />}
          </button>
        ))}
        {ordered.length === 0 && <div className="muted picker-empty">Sin resultados</div>}
      </div>
    </div>
  );
}

export function Toast({ msg, onDone }: { msg: string; onDone: () => void }) {
  useEffect(() => { const t = setTimeout(onDone, 3500); return () => clearTimeout(t); }, [msg, onDone]);
  return <div className="toast" role="status">{msg}</div>;
}
