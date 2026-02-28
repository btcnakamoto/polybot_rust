import { useQuery } from '@tanstack/react-query';
import { fetchWhales } from '../services/api';
import StatusBadge from '../components/StatusBadge';

export default function Whales() {
  const { data: whales, isLoading } = useQuery({
    queryKey: ['whales'],
    queryFn: fetchWhales,
    refetchInterval: 30_000,
  });

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-semibold text-white">跟踪巨鲸</h2>

      <div className="bg-slate-800 rounded-xl border border-slate-700">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-xs text-slate-400 uppercase border-b border-slate-700">
                <th className="text-left px-4 py-3">地址</th>
                <th className="text-left px-4 py-3">分类</th>
                <th className="text-right px-4 py-3">夏普比率</th>
                <th className="text-right px-4 py-3">胜率</th>
                <th className="text-right px-4 py-3">凯利系数</th>
                <th className="text-right px-4 py-3">交易数</th>
                <th className="text-right px-4 py-3">总盈亏</th>
                <th className="text-left px-4 py-3">状态</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={8} className="px-4 py-8 text-center text-slate-500">
                    加载中...
                  </td>
                </tr>
              ) : (whales ?? []).length === 0 ? (
                <tr>
                  <td colSpan={8} className="px-4 py-8 text-center text-slate-500">
                    暂无跟踪巨鲸
                  </td>
                </tr>
              ) : (
                whales!.map((w) => (
                  <tr key={w.id} className="border-b border-slate-700/50 hover:bg-slate-700/30">
                    <td className="px-4 py-2 font-mono text-xs text-slate-300">
                      {w.address.slice(0, 6)}...{w.address.slice(-4)}
                    </td>
                    <td className="px-4 py-2">
                      <StatusBadge status={w.classification ?? 'unknown'} />
                    </td>
                    <td className="px-4 py-2 text-right font-mono">
                      {Number(w.sharpe_ratio ?? 0).toFixed(2)}
                    </td>
                    <td className="px-4 py-2 text-right font-mono">
                      {(Number(w.win_rate ?? 0) * 100).toFixed(1)}%
                    </td>
                    <td className="px-4 py-2 text-right font-mono">
                      {Number(w.kelly_fraction ?? 0).toFixed(3)}
                    </td>
                    <td className="px-4 py-2 text-right font-mono">
                      {w.total_trades ?? 0}
                    </td>
                    <td className={`px-4 py-2 text-right font-mono ${
                      Number(w.total_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'
                    }`}>
                      ${Number(w.total_pnl ?? 0).toFixed(2)}
                    </td>
                    <td className="px-4 py-2">
                      <StatusBadge status={w.is_active ? 'open' : 'closed'} />
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
