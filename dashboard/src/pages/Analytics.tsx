import { useQuery } from '@tanstack/react-query';
import { fetchPnlHistory, fetchPerformance } from '../services/api';
import StatCard from '../components/StatCard';
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';

export default function Analytics() {
  const { data: performance } = useQuery({
    queryKey: ['performance'],
    queryFn: fetchPerformance,
    refetchInterval: 30_000,
  });

  const { data: pnlHistory } = useQuery({
    queryKey: ['pnl-history'],
    queryFn: fetchPnlHistory,
    refetchInterval: 60_000,
  });

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-semibold text-white">交易分析</h2>

      {/* Performance stats */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard label="总交易数" value={performance?.total_trades ?? 0} />
        <StatCard label="盈利次数" value={performance?.win_count ?? 0} />
        <StatCard label="亏损次数" value={performance?.loss_count ?? 0} />
        <StatCard
          label="胜率"
          value={`${(Number(performance?.win_rate ?? 0) * 100).toFixed(1)}%`}
        />
        <StatCard
          label="总利润"
          value={`$${Number(performance?.total_profit ?? 0).toFixed(2)}`}
        />
        <StatCard
          label="平均每笔"
          value={`$${Number(performance?.avg_profit_per_trade ?? 0).toFixed(2)}`}
        />
        <StatCard
          label="最佳交易"
          value={`$${Number(performance?.best_trade ?? 0).toFixed(2)}`}
        />
        <StatCard
          label="最差交易"
          value={`$${Number(performance?.worst_trade ?? 0).toFixed(2)}`}
        />
      </div>

      {/* PnL Chart */}
      <div className="bg-slate-800 rounded-xl border border-slate-700 p-4">
        <h3 className="text-sm font-medium text-white mb-4">累计盈亏曲线</h3>
        {pnlHistory && pnlHistory.length > 0 ? (
          <ResponsiveContainer width="100%" height={400}>
            <AreaChart data={pnlHistory}>
              <defs>
                <linearGradient id="analyticsGradient" x1="0" y1="0" x2="0" y2="1">
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
                fill="url(#analyticsGradient)"
              />
            </AreaChart>
          </ResponsiveContainer>
        ) : (
          <div className="text-center text-slate-500 py-16">暂无盈亏数据</div>
        )}
      </div>
    </div>
  );
}
