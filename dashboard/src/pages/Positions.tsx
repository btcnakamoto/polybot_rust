import { useQuery } from '@tanstack/react-query';
import { fetchPositions } from '../services/api';
import StatusBadge from '../components/StatusBadge';

export default function Positions() {
  const { data: positions, isLoading } = useQuery({
    queryKey: ['positions'],
    queryFn: fetchPositions,
    refetchInterval: 15_000,
  });

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-semibold text-white">Positions</h2>

      <div className="bg-slate-800 rounded-xl border border-slate-700">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-xs text-slate-400 uppercase border-b border-slate-700">
                <th className="text-left px-4 py-3">Market</th>
                <th className="text-left px-4 py-3">Outcome</th>
                <th className="text-right px-4 py-3">Size</th>
                <th className="text-right px-4 py-3">Avg Entry</th>
                <th className="text-right px-4 py-3">Unrealized PnL</th>
                <th className="text-right px-4 py-3">Realized PnL</th>
                <th className="text-left px-4 py-3">Status</th>
                <th className="text-left px-4 py-3">Opened At</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={8} className="px-4 py-8 text-center text-slate-500">
                    Loading...
                  </td>
                </tr>
              ) : (positions ?? []).length === 0 ? (
                <tr>
                  <td colSpan={8} className="px-4 py-8 text-center text-slate-500">
                    No positions
                  </td>
                </tr>
              ) : (
                positions!.map((p) => (
                  <tr key={p.id} className="border-b border-slate-700/50 hover:bg-slate-700/30">
                    <td className="px-4 py-2 font-mono text-xs text-slate-300">
                      {p.market_id.slice(0, 12)}...
                    </td>
                    <td className="px-4 py-2 text-slate-300">{p.outcome}</td>
                    <td className="px-4 py-2 text-right font-mono">
                      {Number(p.size).toFixed(2)}
                    </td>
                    <td className="px-4 py-2 text-right font-mono">
                      {Number(p.avg_entry_price).toFixed(4)}
                    </td>
                    <td className={`px-4 py-2 text-right font-mono ${
                      Number(p.unrealized_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'
                    }`}>
                      ${Number(p.unrealized_pnl ?? 0).toFixed(2)}
                    </td>
                    <td className={`px-4 py-2 text-right font-mono ${
                      Number(p.realized_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'
                    }`}>
                      ${Number(p.realized_pnl ?? 0).toFixed(2)}
                    </td>
                    <td className="px-4 py-2">
                      <StatusBadge status={p.status ?? 'open'} />
                    </td>
                    <td className="px-4 py-2 text-xs text-slate-400">
                      {p.opened_at ? new Date(p.opened_at).toLocaleString() : '—'}
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
