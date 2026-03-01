interface StatCardProps {
  label: string;
  value: string | number;
  sub?: string;
  trend?: 'up' | 'down' | 'neutral';
  accent?: 'emerald' | 'red' | 'amber' | 'indigo' | 'cyan' | 'default';
}

const accentBorders: Record<string, string> = {
  emerald: 'border-l-emerald-500',
  red: 'border-l-red-500',
  amber: 'border-l-amber-500',
  indigo: 'border-l-indigo-500',
  cyan: 'border-l-cyan-500',
  default: 'border-l-slate-600',
};

export default function StatCard({ label, value, sub, trend, accent = 'default' }: StatCardProps) {
  const trendColor = trend === 'up' ? 'text-emerald-400' : trend === 'down' ? 'text-red-400' : 'text-white';

  return (
    <div className={`bg-slate-800/80 backdrop-blur rounded-xl p-3 sm:p-4 border border-slate-700/50 border-l-4 ${accentBorders[accent]}`}>
      <p className="text-[10px] sm:text-xs text-slate-400 uppercase tracking-wider mb-0.5 sm:mb-1">{label}</p>
      <p className={`text-lg sm:text-2xl font-bold font-mono ${trendColor} leading-tight truncate`}>{value}</p>
      {sub && <p className="text-[10px] sm:text-xs text-slate-500 mt-0.5 sm:mt-1">{sub}</p>}
    </div>
  );
}
