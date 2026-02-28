import { useQuery } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import {
  fetchDashboardSummary,
  fetchPositions,
  fetchPnlHistory,
  fetchWhales,
  fetchRecentConsensus,
} from '../services/api';
import StatCard from '../components/StatCard';
import StatusBadge from '../components/StatusBadge';
import {
  AreaChart,
  Area,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  CartesianGrid,
} from 'recharts';

export default function Dashboard() {
  const navigate = useNavigate();

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

  const { data: whales } = useQuery({
    queryKey: ['whales'],
    queryFn: fetchWhales,
    refetchInterval: 30_000,
  });

  const { data: recentSignals } = useQuery({
    queryKey: ['recent-consensus'],
    queryFn: fetchRecentConsensus,
    refetchInterval: 15_000,
  });

  const totalPnl = Number(summary?.total_pnl ?? 0);
  const todayPnl = Number(summary?.today_pnl ?? 0);

  // Top whales by PnL
  const topWhales = [...(whales ?? [])]
    .sort((a, b) => Number(b.total_pnl ?? 0) - Number(a.total_pnl ?? 0))
    .slice(0, 5);

  // Daily PnL for bar chart
  const dailyBars = (pnlHistory ?? []).slice(-14).map((p) => ({
    date: p.date.slice(5), // MM-DD
    daily_pnl: Number(p.daily_pnl),
    cumulative_pnl: Number(p.cumulative_pnl),
  }));

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold text-white">仪表盘</h2>
          <p className="text-xs text-slate-500 mt-0.5">系统运行概览</p>
        </div>
        <div className="text-xs text-slate-500 font-mono">
          {new Date().toLocaleString('zh-CN')}
        </div>
      </div>

      {/* Stats grid */}
      <div className="grid grid-cols-2 lg:grid-cols-3 xl:grid-cols-6 gap-3">
        <StatCard
          label="跟踪巨鲸"
          value={summary?.tracked_whales ?? 0}
          accent="indigo"
          sub={`${topWhales.length} 高排名`}
        />
        <StatCard
          label="持仓中"
          value={summary?.open_positions ?? 0}
          accent="cyan"
        />
        <StatCard
          label="总盈亏"
          value={`$${totalPnl.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`}
          accent={totalPnl >= 0 ? 'emerald' : 'red'}
          trend={totalPnl >= 0 ? 'up' : 'down'}
        />
        <StatCard
          label="今日盈亏"
          value={`$${todayPnl.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`}
          accent={todayPnl >= 0 ? 'emerald' : 'red'}
          trend={todayPnl >= 0 ? 'up' : 'down'}
        />
        <StatCard label="活跃篮子" value={summary?.active_baskets ?? 0} accent="amber" />
        <StatCard label="24h共识" value={summary?.recent_consensus_count ?? 0} accent="cyan" />
      </div>

      {/* Charts row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Cumulative PnL */}
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4">
          <h3 className="text-sm font-medium text-white mb-3">累计盈亏曲线</h3>
          {dailyBars.length > 0 ? (
            <ResponsiveContainer width="100%" height={220}>
              <AreaChart data={dailyBars}>
                <defs>
                  <linearGradient id="pnlGradient" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#10b981" stopOpacity={0.3} />
                    <stop offset="95%" stopColor="#10b981" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
                <XAxis dataKey="date" tick={{ fill: '#64748b', fontSize: 10 }} />
                <YAxis tick={{ fill: '#64748b', fontSize: 10 }} />
                <Tooltip
                  contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #334155', borderRadius: 8 }}
                  labelStyle={{ color: '#e2e8f0' }}
                  formatter={(v: number | undefined) => [`$${(v ?? 0).toFixed(2)}`, '累计盈亏']}
                />
                <Area
                  type="monotone"
                  dataKey="cumulative_pnl"
                  stroke="#10b981"
                  fill="url(#pnlGradient)"
                  strokeWidth={2}
                />
              </AreaChart>
            </ResponsiveContainer>
          ) : (
            <div className="h-[220px] flex items-center justify-center text-slate-500 text-sm">
              暂无盈亏数据
            </div>
          )}
        </div>

        {/* Daily PnL bars */}
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4">
          <h3 className="text-sm font-medium text-white mb-3">每日盈亏</h3>
          {dailyBars.length > 0 ? (
            <ResponsiveContainer width="100%" height={220}>
              <BarChart data={dailyBars}>
                <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
                <XAxis dataKey="date" tick={{ fill: '#64748b', fontSize: 10 }} />
                <YAxis tick={{ fill: '#64748b', fontSize: 10 }} />
                <Tooltip
                  contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #334155', borderRadius: 8 }}
                  labelStyle={{ color: '#e2e8f0' }}
                  formatter={(v: number | undefined) => [`$${(v ?? 0).toFixed(2)}`, '日盈亏']}
                />
                <Bar
                  dataKey="daily_pnl"
                  fill="#6366f1"
                  radius={[4, 4, 0, 0]}
                />
              </BarChart>
            </ResponsiveContainer>
          ) : (
            <div className="h-[220px] flex items-center justify-center text-slate-500 text-sm">
              暂无每日数据
            </div>
          )}
        </div>
      </div>

      {/* Bottom row: whale leaderboard + recent signals + positions */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Whale leaderboard */}
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50">
          <div className="px-4 py-3 border-b border-slate-700/50 flex items-center justify-between">
            <h3 className="text-sm font-medium text-white">巨鲸排行</h3>
            <button
              onClick={() => navigate('/whales')}
              className="text-xs text-indigo-400 hover:text-indigo-300"
            >
              查看全部
            </button>
          </div>
          <div className="p-2">
            {topWhales.length === 0 ? (
              <p className="text-sm text-slate-500 text-center py-6">暂无巨鲸数据</p>
            ) : (
              <div className="space-y-1">
                {topWhales.map((w, i) => (
                  <div
                    key={w.id}
                    onClick={() => navigate(`/whales/${w.address}`)}
                    className="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-slate-700/30 cursor-pointer transition-colors"
                  >
                    <span className={`text-xs font-bold w-5 ${
                      i === 0 ? 'text-amber-400' : i === 1 ? 'text-slate-300' : i === 2 ? 'text-amber-600' : 'text-slate-500'
                    }`}>
                      #{i + 1}
                    </span>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="font-mono text-xs text-slate-300 truncate">
                          {w.address.slice(0, 6)}...{w.address.slice(-4)}
                        </span>
                        {w.classification && <StatusBadge status={w.classification} />}
                      </div>
                      <div className="text-[10px] text-slate-500 mt-0.5">
                        胜率 {(Number(w.win_rate ?? 0) * 100).toFixed(1)}% | {w.total_trades ?? 0} 笔
                      </div>
                    </div>
                    <span className={`text-sm font-mono font-medium ${
                      Number(w.total_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'
                    }`}>
                      ${Number(w.total_pnl ?? 0).toLocaleString('en-US', { maximumFractionDigits: 0 })}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Recent signals */}
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50">
          <div className="px-4 py-3 border-b border-slate-700/50 flex items-center justify-between">
            <h3 className="text-sm font-medium text-white">近期信号</h3>
            <button
              onClick={() => navigate('/signals')}
              className="text-xs text-indigo-400 hover:text-indigo-300"
            >
              查看全部
            </button>
          </div>
          <div className="p-2">
            {(recentSignals ?? []).length === 0 ? (
              <p className="text-sm text-slate-500 text-center py-6">暂无共识信号</p>
            ) : (
              <div className="space-y-1">
                {(recentSignals ?? []).slice(0, 6).map((s) => (
                  <div key={s.id} className="flex items-center justify-between px-3 py-2 rounded-lg hover:bg-slate-700/30">
                    <div>
                      <div className="flex items-center gap-2">
                        <StatusBadge status={s.direction} />
                        <span className="font-mono text-xs text-slate-400">
                          {s.market_id.slice(0, 16)}...
                        </span>
                      </div>
                      <div className="text-[10px] text-slate-500 mt-0.5">
                        共识 {(Number(s.consensus_pct) * 100).toFixed(0)}% ({s.participating_whales}/{s.total_whales})
                      </div>
                    </div>
                    <span className="text-[10px] text-slate-500">
                      {new Date(s.triggered_at).toLocaleTimeString('zh-CN')}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Open positions */}
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50">
          <div className="px-4 py-3 border-b border-slate-700/50 flex items-center justify-between">
            <h3 className="text-sm font-medium text-white">当前持仓</h3>
            <button
              onClick={() => navigate('/positions')}
              className="text-xs text-indigo-400 hover:text-indigo-300"
            >
              查看全部
            </button>
          </div>
          <div className="p-2">
            {(positions ?? []).length === 0 ? (
              <p className="text-sm text-slate-500 text-center py-6">暂无持仓</p>
            ) : (
              <div className="space-y-1">
                {(positions ?? []).slice(0, 6).map((p) => (
                  <div key={p.id} className="flex items-center justify-between px-3 py-2 rounded-lg hover:bg-slate-700/30">
                    <div>
                      <span className="font-mono text-xs text-slate-300">
                        {p.market_id.slice(0, 16)}...
                      </span>
                      <div className="text-[10px] text-slate-500 mt-0.5">
                        {p.outcome} | {Number(p.size).toFixed(2)} @ {Number(p.avg_entry_price).toFixed(4)}
                      </div>
                    </div>
                    <span className={`text-sm font-mono font-medium ${
                      Number(p.unrealized_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'
                    }`}>
                      ${Number(p.unrealized_pnl ?? 0).toFixed(2)}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
