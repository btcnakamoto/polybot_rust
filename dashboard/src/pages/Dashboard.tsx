import { useQuery } from '@tanstack/react-query';
import { fetchDashboardSummary, fetchPositions, fetchPnlHistory } from '../services/api';
import StatCard from '../components/StatCard';
import StatusBadge from '../components/StatusBadge';
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';

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

  const { data: pnlHistory } = useQuery({
    queryKey: ['pnl-history'],
    queryFn: fetchPnlHistory,
    refetchInterval: 60_000,
  });

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-semibold text-white">仪表盘</h2>

      {/* Stats grid */}
      <div className="grid grid-cols-2 lg:grid-cols-3 gap-4">
        <StatCard label="跟踪巨鲸" value={summary?.tracked_whales ?? 0} />
        <StatCard label="持仓中" value={summary?.open_positions ?? 0} />
        <StatCard
          label="总盈亏"
          value={`$${Number(summary?.total_pnl ?? 0).toFixed(2)}`}
        />
        <StatCard
          label="今日盈亏"
          value={`$${Number(summary?.today_pnl ?? 0).toFixed(2)}`}
        />
        <StatCard label="活跃篮子" value={summary?.active_baskets ?? 0} />
        <StatCard label="24h共识" value={summary?.recent_consensus_count ?? 0} />
      </div>

      {/* Mini PnL chart */}
      {pnlHistory && pnlHistory.length > 0 && (
        <div className="bg-slate-800 rounded-xl border border-slate-700 p-4">
          <h3 className="text-sm font-medium text-white mb-3">累计盈亏曲线</h3>
          <ResponsiveContainer width="100%" height={200}>
            <AreaChart data={pnlHistory}>
              <defs>
                <linearGradient id="pnlGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#10b981" stopOpacity={0.3} />
                  <stop offset="95%" stopColor="#10b981" stopOpacity={0} />
                </linearGradient>
              </defs>
              <XAxis dataKey="date" tick={{ fill: '#94a3b8', fontSize: 11 }} />
              <YAxis tick={{ fill: '#94a3b8', fontSize: 11 }} />
              <Tooltip
                contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #334155', borderRadius: 8 }}
                labelStyle={{ color: '#e2e8f0' }}
              />
              <Area
                type="monotone"
                dataKey="cumulative_pnl"
                name="累计盈亏"
                stroke="#10b981"
                fill="url(#pnlGradient)"
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      )}

      {/* Open positions table */}
      <div className="bg-slate-800 rounded-xl border border-slate-700">
        <div className="px-4 py-3 border-b border-slate-700">
          <h3 className="text-sm font-medium text-white">持仓中</h3>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-xs text-slate-400 uppercase border-b border-slate-700">
                <th className="text-left px-4 py-2">市场</th>
                <th className="text-left px-4 py-2">结果</th>
                <th className="text-right px-4 py-2">数量</th>
                <th className="text-right px-4 py-2">均价</th>
                <th className="text-right px-4 py-2">浮动盈亏</th>
                <th className="text-left px-4 py-2">状态</th>
              </tr>
            </thead>
            <tbody>
              {(positions ?? []).length === 0 ? (
                <tr>
                  <td colSpan={6} className="px-4 py-8 text-center text-slate-500">
                    暂无持仓
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
