import { useQuery } from '@tanstack/react-query';
import { fetchTrades } from '../services/api';
import StatusBadge from '../components/StatusBadge';

export default function Trades() {
  const { data: trades, isLoading } = useQuery({
    queryKey: ['trades'],
    queryFn: fetchTrades,
    refetchInterval: 10_000,
  });

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-semibold text-white">Copy Trades</h2>

      <div className="bg-slate-800 rounded-xl border border-slate-700">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-xs text-slate-400 uppercase border-b border-slate-700">
                <th className="text-left px-4 py-3">Market</th>
                <th className="text-left px-4 py-3">Side</th>
                <th className="text-right px-4 py-3">Size</th>
                <th className="text-right px-4 py-3">Target Price</th>
                <th className="text-right px-4 py-3">Fill Price</th>
                <th className="text-right px-4 py-3">Slippage</th>
                <th className="text-left px-4 py-3">Strategy</th>
                <th className="text-left px-4 py-3">Status</th>
                <th className="text-left px-4 py-3">Placed At</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={9} className="px-4 py-8 text-center text-slate-500">
                    Loading...
                  </td>
                </tr>
              ) : (trades ?? []).length === 0 ? (
                <tr>
                  <td colSpan={9} className="px-4 py-8 text-center text-slate-500">
                    No copy trades yet
                  </td>
                </tr>
              ) : (
                trades!.map((t) => (
                  <tr key={t.id} className="border-b border-slate-700/50 hover:bg-slate-700/30">
                    <td className="px-4 py-2 font-mono text-xs text-slate-300">
                      {t.market_id.slice(0, 12)}...
                    </td>
                    <td className={`px-4 py-2 font-medium ${
                      t.side === 'BUY' ? 'text-emerald-400' : 'text-red-400'
                    }`}>
                      {t.side}
                    </td>
                    <td className="px-4 py-2 text-right font-mono">
                      {Number(t.size).toFixed(2)}
                    </td>
                    <td className="px-4 py-2 text-right font-mono">
                      {Number(t.target_price).toFixed(4)}
                    </td>
                    <td className="px-4 py-2 text-right font-mono">
                      {t.fill_price ? Number(t.fill_price).toFixed(4) : '—'}
                    </td>
                    <td className="px-4 py-2 text-right font-mono">
                      {t.slippage ? `${(Number(t.slippage) * 100).toFixed(2)}%` : '—'}
                    </td>
                    <td className="px-4 py-2 text-slate-400">{t.strategy}</td>
                    <td className="px-4 py-2">
                      <StatusBadge status={t.status} />
                    </td>
                    <td className="px-4 py-2 text-xs text-slate-400">
                      {t.placed_at ? new Date(t.placed_at).toLocaleString() : '—'}
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
