const colorMap: Record<string, string> = {
  // Order/Position status
  filled: 'bg-emerald-500/20 text-emerald-400 ring-emerald-500/30',
  open: 'bg-emerald-500/20 text-emerald-400 ring-emerald-500/30',
  pending: 'bg-yellow-500/20 text-yellow-400 ring-yellow-500/30',
  partial: 'bg-yellow-500/20 text-yellow-400 ring-yellow-500/30',
  failed: 'bg-red-500/20 text-red-400 ring-red-500/30',
  cancelled: 'bg-slate-500/20 text-slate-400 ring-slate-500/30',
  closed: 'bg-slate-500/20 text-slate-400 ring-slate-500/30',
  // Classification
  informed: 'bg-emerald-500/20 text-emerald-400 ring-emerald-500/30',
  top_tier: 'bg-amber-500/20 text-amber-300 ring-amber-500/30',
  high_performer: 'bg-indigo-500/20 text-indigo-400 ring-indigo-500/30',
  profitable: 'bg-cyan-500/20 text-cyan-400 ring-cyan-500/30',
  market_maker: 'bg-yellow-500/20 text-yellow-400 ring-yellow-500/30',
  bot: 'bg-red-500/20 text-red-400 ring-red-500/30',
  // Category
  politics: 'bg-blue-500/20 text-blue-400 ring-blue-500/30',
  crypto: 'bg-amber-500/20 text-amber-400 ring-amber-500/30',
  sports: 'bg-purple-500/20 text-purple-400 ring-purple-500/30',
  // Signals
  consensus: 'bg-cyan-500/20 text-cyan-400 ring-cyan-500/30',
  // System
  live: 'bg-emerald-500/20 text-emerald-400 ring-emerald-500/30',
  dry_run: 'bg-amber-500/20 text-amber-400 ring-amber-500/30',
  paused: 'bg-red-500/20 text-red-400 ring-red-500/30',
  running: 'bg-emerald-500/20 text-emerald-400 ring-emerald-500/30',
  BUY: 'bg-emerald-500/20 text-emerald-400 ring-emerald-500/30',
  SELL: 'bg-red-500/20 text-red-400 ring-red-500/30',
};

const labelMap: Record<string, string> = {
  top_tier: 'Top Tier',
  high_performer: 'High Perf',
  profitable: 'Profitable',
  market_maker: 'MM',
  informed: 'Informed',
  dry_run: 'Dry Run',
};

export default function StatusBadge({ status }: { status: string }) {
  const colors = colorMap[status] ?? 'bg-slate-500/20 text-slate-400 ring-slate-500/30';
  const label = labelMap[status] ?? status;
  return (
    <span className={`inline-block px-2 py-0.5 rounded-md text-xs font-medium ring-1 ${colors}`}>
      {label}
    </span>
  );
}
