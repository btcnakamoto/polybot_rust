const colorMap: Record<string, string> = {
  filled: 'bg-emerald-500/20 text-emerald-400',
  open: 'bg-emerald-500/20 text-emerald-400',
  pending: 'bg-yellow-500/20 text-yellow-400',
  partial: 'bg-yellow-500/20 text-yellow-400',
  failed: 'bg-red-500/20 text-red-400',
  cancelled: 'bg-slate-500/20 text-slate-400',
  closed: 'bg-slate-500/20 text-slate-400',
  informed: 'bg-emerald-500/20 text-emerald-400',
  market_maker: 'bg-yellow-500/20 text-yellow-400',
  bot: 'bg-red-500/20 text-red-400',
  politics: 'bg-blue-500/20 text-blue-400',
  crypto: 'bg-amber-500/20 text-amber-400',
  sports: 'bg-purple-500/20 text-purple-400',
  consensus: 'bg-cyan-500/20 text-cyan-400',
};

export default function StatusBadge({ status }: { status: string }) {
  const colors = colorMap[status] ?? 'bg-slate-500/20 text-slate-400';
  return (
    <span className={`inline-block px-2 py-0.5 rounded text-xs font-medium ${colors}`}>
      {status}
    </span>
  );
}
