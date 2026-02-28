import { useQuery } from '@tanstack/react-query';
import { fetchDashboardSummary, fetchPositions } from '../services/api';
import StatCard from '../components/StatCard';
import StatusBadge from '../components/StatusBadge';

export default function Dashboard() {
  const { data: summary } = useQuery({
    queryKey: ['dashboard'],
    queryFn: fetchDashboardSummary,
    refetchInterval: 10_000,
  });

  const { data: positions } = useQuery({
    queryKey: ['positions'],
    queryFn: fetchPositions,
    refetchInterval: 15_000,
  });

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-semibold text-white">Dashboard</h2>

      {/* Stats grid */}
      <div className="grid grid-cols-2 lg:grid-cols-3 gap-4">
        <StatCard label="Tracked Whales" value={summary?.tracked_whales ?? 0} />
        <StatCard label="Open Positions" value={summary?.open_positions ?? 0} />
        <StatCard
          label="Total PnL"
          value={`$${Number(summary?.total_pnl ?? 0).toFixed(2)}`}
        />
        <StatCard
          label="Today PnL"
          value={`$${Number(summary?.today_pnl ?? 0).toFixed(2)}`}
        />
        <StatCard label="Active Baskets" value={summary?.active_baskets ?? 0} />
        <StatCard label="Consensus 24h" value={summary?.recent_consensus_count ?? 0} />
      </div>

      {/* Open positions table */}
      <div className="bg-slate-800 rounded-xl border border-slate-700">
        <div className="px-4 py-3 border-b border-slate-700">
          <h3 className="text-sm font-medium text-white">Open Positions</h3>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-xs text-slate-400 uppercase border-b border-slate-700">
                <th className="text-left px-4 py-2">Market</th>
                <th className="text-left px-4 py-2">Outcome</th>
                <th className="text-right px-4 py-2">Size</th>
                <th className="text-right px-4 py-2">Avg Entry</th>
                <th className="text-right px-4 py-2">Unrealized PnL</th>
                <th className="text-left px-4 py-2">Status</th>
              </tr>
            </thead>
            <tbody>
              {(positions ?? []).length === 0 ? (
                <tr>
                  <td colSpan={6} className="px-4 py-8 text-center text-slate-500">
                    No open positions
                  </td>
                </tr>
              ) : (
                positions!.map((p) => (
                  <tr key={p.id} className="border-b border-slate-700/50 hover:bg-slate-700/30">
                    <td className="px-4 py-2 font-mono text-xs text-slate-300">
                      {p.market_id.slice(0, 12)}...
                    </td>
                    <td className="px-4 py-2">{p.outcome}</td>
                    <td className="px-4 py-2 text-right font-mono">{Number(p.size).toFixed(2)}</td>
                    <td className="px-4 py-2 text-right font-mono">
                      {Number(p.avg_entry_price).toFixed(4)}
                    </td>
                    <td className={`px-4 py-2 text-right font-mono ${
                      Number(p.unrealized_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'
                    }`}>
                      ${Number(p.unrealized_pnl ?? 0).toFixed(2)}
                    </td>
                    <td className="px-4 py-2">
                      <StatusBadge status={p.status ?? 'open'} />
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
