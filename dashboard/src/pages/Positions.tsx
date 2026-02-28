import { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { fetchPositions } from '../services/api';
import StatusBadge from '../components/StatusBadge';
import StatCard from '../components/StatCard';

export default function Positions() {
  const [showClosed, setShowClosed] = useState(false);

  const { data: positions, isLoading } = useQuery({
    queryKey: ['positions'],
    queryFn: fetchPositions,
    refetchInterval: 15_000,
  });

  const filtered = useMemo(() => {
    if (showClosed) return positions ?? [];
    return (positions ?? []).filter((p) => p.status !== 'closed');
  }, [positions, showClosed]);

  const totalUnrealized = (positions ?? [])
    .filter((p) => p.status !== 'closed')
    .reduce((sum, p) => sum + Number(p.unrealized_pnl ?? 0), 0);

  const totalRealized = (positions ?? [])
    .filter((p) => p.status === 'closed')
    .reduce((sum, p) => sum + Number(p.realized_pnl ?? 0), 0);

  const openCount = (positions ?? []).filter((p) => p.status !== 'closed').length;
  const closedCount = (positions ?? []).filter((p) => p.status === 'closed').length;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold text-white">持仓管理</h2>
          <p className="text-xs text-slate-500 mt-0.5">{openCount} 个活跃持仓</p>
        </div>
        <button
          onClick={() => setShowClosed(!showClosed)}
          className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            showClosed
              ? 'bg-indigo-500/20 text-indigo-400 ring-1 ring-indigo-500/30'
              : 'text-slate-400 hover:text-white bg-slate-800 border border-slate-700'
          }`}
        >
          {showClosed ? '隐藏已平仓' : '显示已平仓'}
        </button>
      </div>

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        <StatCard label="活跃持仓" value={openCount} accent="indigo" />
        <StatCard label="已平仓" value={closedCount} accent="default" />
        <StatCard
          label="浮动盈亏"
          value={`$${totalUnrealized.toFixed(2)}`}
          accent={totalUnrealized >= 0 ? 'emerald' : 'red'}
          trend={totalUnrealized >= 0 ? 'up' : 'down'}
        />
        <StatCard
          label="已实现盈亏"
          value={`$${totalRealized.toFixed(2)}`}
          accent={totalRealized >= 0 ? 'emerald' : 'red'}
          trend={totalRealized >= 0 ? 'up' : 'down'}
        />
      </div>

      <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-xs text-slate-400 uppercase border-b border-slate-700/50">
                <th className="text-left px-4 py-3 font-medium">市场</th>
                <th className="text-left px-4 py-3 font-medium">结果</th>
                <th className="text-right px-4 py-3 font-medium">数量</th>
                <th className="text-right px-4 py-3 font-medium">均价</th>
                <th className="text-right px-4 py-3 font-medium">浮动盈亏</th>
                <th className="text-right px-4 py-3 font-medium">已实现</th>
                <th className="text-left px-4 py-3 font-medium">状态</th>
                <th className="text-left px-4 py-3 font-medium">开仓时间</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={8} className="px-4 py-12 text-center text-slate-500">
                    <div className="flex items-center justify-center gap-2">
                      <div className="w-4 h-4 border-2 border-slate-600 border-t-indigo-400 rounded-full animate-spin" />
                      加载中...
                    </div>
                  </td>
                </tr>
              ) : filtered.length === 0 ? (
                <tr>
                  <td colSpan={8} className="px-4 py-12 text-center text-slate-500">
                    暂无持仓
                  </td>
                </tr>
              ) : (
                filtered.map((p) => (
                  <tr key={p.id} className="border-b border-slate-700/30 hover:bg-slate-700/20 transition-colors">
                    <td className="px-4 py-2 font-mono text-xs text-slate-300">
                      {p.market_id.slice(0, 14)}...
                    </td>
                    <td className="px-4 py-2 text-slate-300">{p.outcome}</td>
                    <td className="px-4 py-2 text-right font-mono text-slate-300">
                      {Number(p.size).toFixed(2)}
                    </td>
                    <td className="px-4 py-2 text-right font-mono text-slate-300">
                      {Number(p.avg_entry_price).toFixed(4)}
                    </td>
                    <td className={`px-4 py-2 text-right font-mono font-medium ${
                      Number(p.unrealized_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'
                    }`}>
                      ${Number(p.unrealized_pnl ?? 0).toFixed(2)}
                    </td>
                    <td className={`px-4 py-2 text-right font-mono font-medium ${
                      Number(p.realized_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'
                    }`}>
                      ${Number(p.realized_pnl ?? 0).toFixed(2)}
                    </td>
                    <td className="px-4 py-2">
                      <StatusBadge status={p.status ?? 'open'} />
                    </td>
                    <td className="px-4 py-2 text-xs text-slate-500">
                      {p.opened_at ? new Date(p.opened_at).toLocaleString('zh-CN') : '--'}
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
