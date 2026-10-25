import { Hammer } from "lucide-react";

export function Placeholder({ title }: { title: string }) {
  return (
    <div className="page">
      <header className="top"><h1>{title}</h1></header>
      <section className="card empty">
        <Hammer size={28} className="muted" />
        <h2>En construcción</h2>
        <p className="muted">Esta sección llega en los próximos pasos.</p>
      </section>
    </div>
  );
}
