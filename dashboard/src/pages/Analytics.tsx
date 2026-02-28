import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { fetchPnlHistory, fetchPerformance, fetchWhales, fetchTrades } from '../services/api';
import StatCard from '../components/StatCard';
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
  PieChart,
  Pie,
  Cell,
  Legend,
  ComposedChart,
} from 'recharts';

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

  const { data: whales } = useQuery({
    queryKey: ['whales'],
    queryFn: fetchWhales,
    refetchInterval: 60_000,
  });

  const { data: trades } = useQuery({
    queryKey: ['trades'],
    queryFn: fetchTrades,
    refetchInterval: 30_000,
  });

  const winRate = Number(performance?.win_rate ?? 0) * 100;
  const totalProfit = Number(performance?.total_profit ?? 0);

  // Compute drawdown from PnL history
  const { drawdownData, maxDrawdown } = useMemo(() => {
    const points = (pnlHistory ?? []).map((p) => ({
      date: p.date.slice(5),
      cumulative: Number(p.cumulative_pnl),
    }));

    let peak = 0;
    let maxDd = 0;
    const ddData = points.map((p) => {
      if (p.cumulative > peak) peak = p.cumulative;
      const dd = peak > 0 ? ((peak - p.cumulative) / peak) * 100 : 0;
      if (dd > maxDd) maxDd = dd;
      return { date: p.date, drawdown: -dd, cumulative: p.cumulative };
    });

    return { drawdownData: ddData, maxDrawdown: maxDd };
  }, [pnlHistory, totalProfit]);

  // Win/Loss pie
  const winLossData = [
    { name: '盈利', value: performance?.win_count ?? 0, color: '#10b981' },
    { name: '亏损', value: performance?.loss_count ?? 0, color: '#ef4444' },
  ];

  // Strategy distribution from trades
  const strategyDist = useMemo(() => {
    const map = new Map<string, number>();
    (trades ?? []).forEach((t) => {
      map.set(t.strategy, (map.get(t.strategy) ?? 0) + 1);
    });
    return [...map.entries()].map(([name, value]) => ({ name, value }));
  }, [trades]);

  // Whale PnL distribution
  const whalePnlDist = useMemo(() => {
    return [...(whales ?? [])]
      .sort((a, b) => Number(b.total_pnl ?? 0) - Number(a.total_pnl ?? 0))
      .slice(0, 10)
      .map((w) => ({
        name: `${w.address.slice(0, 6)}...`,
        pnl: Number(w.total_pnl ?? 0),
        win_rate: Number(w.win_rate ?? 0) * 100,
      }));
  }, [whales]);

  // Daily PnL for bar chart
  const dailyBars = (pnlHistory ?? []).map((p) => ({
    date: p.date.slice(5),
    daily_pnl: Number(p.daily_pnl),
    cumulative_pnl: Number(p.cumulative_pnl),
  }));

  // Backtest simulation: compute hypothetical returns with different strategies
  const backtestData = useMemo(() => {
    if (!pnlHistory || pnlHistory.length === 0) return [];
    let conservative = 0;
    let moderate = 0;
    let aggressive = 0;
    return pnlHistory.map((p) => {
      const daily = Number(p.daily_pnl);
      conservative += daily * 0.5;
      moderate += daily * 1.0;
      aggressive += daily * 1.5;
      return {
        date: p.date.slice(5),
        conservative,
        moderate,
        aggressive,
      };
    });
  }, [pnlHistory]);

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-white">交易分析</h2>
        <p className="text-xs text-slate-500 mt-0.5">绩效指标、回测模拟与风险分析</p>
      </div>

      {/* Key metrics */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        <StatCard label="总交易数" value={performance?.total_trades ?? 0} accent="indigo" />
        <StatCard
          label="胜率"
          value={`${winRate.toFixed(1)}%`}
          accent={winRate >= 55 ? 'emerald' : winRate >= 45 ? 'amber' : 'red'}
        />
        <StatCard
          label="总利润"
          value={`$${totalProfit.toFixed(2)}`}
          accent={totalProfit >= 0 ? 'emerald' : 'red'}
          trend={totalProfit >= 0 ? 'up' : 'down'}
        />
        <StatCard
          label="平均每笔"
          value={`$${Number(performance?.avg_profit_per_trade ?? 0).toFixed(2)}`}
          accent="cyan"
        />
      </div>

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        <StatCard
          label="最佳交易"
          value={`$${Number(performance?.best_trade ?? 0).toFixed(2)}`}
          accent="emerald"
        />
        <StatCard
          label="最差交易"
          value={`$${Number(performance?.worst_trade ?? 0).toFixed(2)}`}
          accent="red"
        />
        <StatCard
          label="最大回撤"
          value={`${maxDrawdown.toFixed(1)}%`}
          accent="red"
        />
        <StatCard
          label="盈亏比"
          value={(performance?.win_count ?? 0) > 0 && (performance?.loss_count ?? 0) > 0
            ? (Number(performance!.best_trade) / Math.abs(Number(performance!.worst_trade))).toFixed(2)
            : 'N/A'
          }
          accent="amber"
        />
      </div>

      {/* Charts row 1: PnL + Drawdown */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Cumulative PnL + Daily bars */}
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4">
          <h3 className="text-sm font-medium text-white mb-3">累计盈亏 & 每日收益</h3>
          {dailyBars.length > 0 ? (
            <ResponsiveContainer width="100%" height={280}>
              <ComposedChart data={dailyBars}>
                <defs>
                  <linearGradient id="analyticsGradient" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#10b981" stopOpacity={0.2} />
                    <stop offset="95%" stopColor="#10b981" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
                <XAxis dataKey="date" tick={{ fill: '#64748b', fontSize: 10 }} />
                <YAxis yAxisId="left" tick={{ fill: '#64748b', fontSize: 10 }} />
                <YAxis yAxisId="right" orientation="right" tick={{ fill: '#64748b', fontSize: 10 }} />
                <Tooltip
                  contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #334155', borderRadius: 8 }}
                  labelStyle={{ color: '#e2e8f0' }}
                />
                <Bar yAxisId="right" dataKey="daily_pnl" name="日盈亏" fill="#6366f1" opacity={0.6} radius={[2, 2, 0, 0]} />
                <Area
                  yAxisId="left"
                  type="monotone"
                  dataKey="cumulative_pnl"
                  name="累计盈亏"
                  stroke="#10b981"
                  fill="url(#analyticsGradient)"
                  strokeWidth={2}
                />
              </ComposedChart>
            </ResponsiveContainer>
          ) : (
            <div className="h-[280px] flex items-center justify-center text-slate-500 text-sm">暂无数据</div>
          )}
        </div>

        {/* Drawdown chart */}
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4">
          <h3 className="text-sm font-medium text-white mb-3">
            回撤曲线
            <span className="text-xs text-slate-500 ml-2">最大回撤 {maxDrawdown.toFixed(1)}%</span>
          </h3>
          {drawdownData.length > 0 ? (
            <ResponsiveContainer width="100%" height={280}>
              <AreaChart data={drawdownData}>
                <defs>
                  <linearGradient id="drawdownGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#ef4444" stopOpacity={0.3} />
                    <stop offset="95%" stopColor="#ef4444" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
                <XAxis dataKey="date" tick={{ fill: '#64748b', fontSize: 10 }} />
                <YAxis tick={{ fill: '#64748b', fontSize: 10 }} />
                <Tooltip
                  contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #334155', borderRadius: 8 }}
                  labelStyle={{ color: '#e2e8f0' }}
                  formatter={(v: number | undefined) => [`${(v ?? 0).toFixed(2)}%`, '回撤']}
                />
                <Area
                  type="monotone"
                  dataKey="drawdown"
                  stroke="#ef4444"
                  fill="url(#drawdownGrad)"
                  strokeWidth={2}
                />
              </AreaChart>
            </ResponsiveContainer>
          ) : (
            <div className="h-[280px] flex items-center justify-center text-slate-500 text-sm">暂无数据</div>
          )}
        </div>
      </div>

      {/* Charts row 2: Win/Loss pie + Whale PnL distribution */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Win/Loss pie */}
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4">
          <h3 className="text-sm font-medium text-white mb-3">胜率分布</h3>
          {(performance?.total_trades ?? 0) > 0 ? (
            <ResponsiveContainer width="100%" height={220}>
              <PieChart>
                <Pie
                  data={winLossData}
                  cx="50%"
                  cy="50%"
                  innerRadius={50}
                  outerRadius={80}
                  paddingAngle={4}
                  dataKey="value"
                  label={({ name, value }) => `${name}: ${value}`}
                >
                  {winLossData.map((entry, i) => (
                    <Cell key={i} fill={entry.color} />
                  ))}
                </Pie>
                <Legend
                  formatter={(value) => <span className="text-xs text-slate-400">{value}</span>}
                />
              </PieChart>
            </ResponsiveContainer>
          ) : (
            <div className="h-[220px] flex items-center justify-center text-slate-500 text-sm">暂无数据</div>
          )}
        </div>

        {/* Whale PnL ranking */}
        <div className="lg:col-span-2 bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4">
          <h3 className="text-sm font-medium text-white mb-3">巨鲸盈亏排行 (Top 10)</h3>
          {whalePnlDist.length > 0 ? (
            <ResponsiveContainer width="100%" height={220}>
              <BarChart data={whalePnlDist} layout="vertical">
                <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
                <XAxis type="number" tick={{ fill: '#64748b', fontSize: 10 }} />
                <YAxis type="category" dataKey="name" tick={{ fill: '#94a3b8', fontSize: 10 }} width={70} />
                <Tooltip
                  contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #334155', borderRadius: 8 }}
                  labelStyle={{ color: '#e2e8f0' }}
                  formatter={(v: number | undefined, name?: string) => [
                    name === 'pnl' ? `$${(v ?? 0).toLocaleString()}` : `${(v ?? 0).toFixed(1)}%`,
                    name === 'pnl' ? '盈亏' : '胜率',
                  ]}
                />
                <Bar dataKey="pnl" name="pnl" radius={[0, 4, 4, 0]}>
                  {whalePnlDist.map((entry, i) => (
                    <Cell key={i} fill={entry.pnl >= 0 ? '#10b981' : '#ef4444'} />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          ) : (
            <div className="h-[220px] flex items-center justify-center text-slate-500 text-sm">暂无数据</div>
          )}
        </div>
      </div>

      {/* Backtest simulation */}
      <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4">
        <div className="flex items-center justify-between mb-3">
          <div>
            <h3 className="text-sm font-medium text-white">回测模拟</h3>
            <p className="text-[10px] text-slate-500 mt-0.5">基于历史数据，不同仓位倍数的模拟回报曲线</p>
          </div>
          <div className="flex gap-4 text-[10px]">
            <span className="flex items-center gap-1"><span className="w-3 h-0.5 bg-amber-500 inline-block" /> 保守 (0.5x)</span>
            <span className="flex items-center gap-1"><span className="w-3 h-0.5 bg-indigo-500 inline-block" /> 标准 (1.0x)</span>
            <span className="flex items-center gap-1"><span className="w-3 h-0.5 bg-emerald-500 inline-block" /> 激进 (1.5x)</span>
          </div>
        </div>
        {backtestData.length > 0 ? (
          <ResponsiveContainer width="100%" height={300}>
            <AreaChart data={backtestData}>
              <defs>
                <linearGradient id="conservativeGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#f59e0b" stopOpacity={0.1} />
                  <stop offset="95%" stopColor="#f59e0b" stopOpacity={0} />
                </linearGradient>
                <linearGradient id="moderateGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#6366f1" stopOpacity={0.1} />
                  <stop offset="95%" stopColor="#6366f1" stopOpacity={0} />
                </linearGradient>
                <linearGradient id="aggressiveGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#10b981" stopOpacity={0.1} />
                  <stop offset="95%" stopColor="#10b981" stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
              <XAxis dataKey="date" tick={{ fill: '#64748b', fontSize: 10 }} />
              <YAxis tick={{ fill: '#64748b', fontSize: 10 }} />
              <Tooltip
                contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #334155', borderRadius: 8 }}
                labelStyle={{ color: '#e2e8f0' }}
                formatter={(v: number | undefined, name?: string) => {
                  const labels: Record<string, string> = {
                    conservative: '保守 (0.5x)',
                    moderate: '标准 (1.0x)',
                    aggressive: '激进 (1.5x)',
                  };
                  return [`$${(v ?? 0).toFixed(2)}`, labels[name ?? ''] ?? name ?? ''];
                }}
              />
              <Area type="monotone" dataKey="conservative" stroke="#f59e0b" fill="url(#conservativeGrad)" strokeWidth={1.5} />
              <Area type="monotone" dataKey="moderate" stroke="#6366f1" fill="url(#moderateGrad)" strokeWidth={2} />
              <Area type="monotone" dataKey="aggressive" stroke="#10b981" fill="url(#aggressiveGrad)" strokeWidth={1.5} />
            </AreaChart>
          </ResponsiveContainer>
        ) : (
          <div className="h-[300px] flex items-center justify-center text-slate-500 text-sm">
            暂无历史数据用于回测模拟
          </div>
        )}
      </div>

      {/* Strategy distribution */}
      {strategyDist.length > 0 && (
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4">
          <h3 className="text-sm font-medium text-white mb-3">策略分布</h3>
          <div className="flex gap-4">
            {strategyDist.map((s) => (
              <div key={s.name} className="bg-slate-700/30 rounded-lg px-4 py-3 text-center">
                <p className="text-xs text-slate-400">{s.name}</p>
                <p className="text-xl font-bold font-mono text-white">{s.value}</p>
                <p className="text-[10px] text-slate-500">笔交易</p>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
