import { useParams, useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { fetchWhaleByAddress, fetchWhaleTrades } from '../services/api';
import StatCard from '../components/StatCard';
import StatusBadge from '../components/StatusBadge';
import { ArrowLeft } from 'lucide-react';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from 'recharts';

export default function WhaleDetail() {
  const { address } = useParams<{ address: string }>();
  const navigate = useNavigate();

  const { data: whale, isLoading } = useQuery({
    queryKey: ['whale', address],
    queryFn: () => fetchWhaleByAddress(address!),
    enabled: !!address,
  });

  const { data: trades } = useQuery({
    queryKey: ['whale-trades', whale?.id],
    queryFn: () => fetchWhaleTrades(whale!.id),
    enabled: !!whale?.id,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="w-6 h-6 border-2 border-slate-600 border-t-indigo-400 rounded-full animate-spin" />
      </div>
    );
  }

  if (!whale) {
    return (
      <div className="text-center py-16">
        <p className="text-slate-500">未找到该巨鲸</p>
        <button onClick={() => navigate('/whales')} className="text-indigo-400 text-sm mt-2 hover:underline">
          返回列表
        </button>
      </div>
    );
  }

  const pnl = Number(whale.total_pnl ?? 0);
  const winRate = Number(whale.win_rate ?? 0) * 100;
  const sharpe = Number(whale.sharpe_ratio ?? 0);
  const kelly = Number(whale.kelly_fraction ?? 0);
  const ev = Number(whale.expected_value ?? 0);

  // Trade volume by day for chart
  const tradesByDay = new Map<string, { buys: number; sells: number }>();
  (trades ?? []).forEach((t) => {
    const day = t.traded_at.slice(0, 10);
    const entry = tradesByDay.get(day) ?? { buys: 0, sells: 0 };
    const notional = Number(t.notional);
    if (t.side === 'BUY') entry.buys += notional;
    else entry.sells += notional;
    tradesByDay.set(day, entry);
  });

  const volumeData = [...tradesByDay.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .slice(-30)
    .map(([date, v]) => ({
      date: date.slice(5),
      buys: v.buys,
      sells: -v.sells,
    }));

  // Side distribution
  const buyCount = (trades ?? []).filter((t) => t.side === 'BUY').length;
  const sellCount = (trades ?? []).filter((t) => t.side === 'SELL').length;
  const totalNotional = (trades ?? []).reduce((sum, t) => sum + Number(t.notional), 0);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center gap-4">
        <button
          onClick={() => navigate('/whales')}
          className="p-2 rounded-lg hover:bg-slate-800 transition-colors"
        >
          <ArrowLeft size={18} className="text-slate-400" />
        </button>
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h2 className="text-lg font-semibold text-white font-mono">
              {whale.address.slice(0, 10)}...{whale.address.slice(-8)}
            </h2>
            {whale.classification && <StatusBadge status={whale.classification} />}
            <StatusBadge status={whale.is_active ? 'open' : 'closed'} />
          </div>
          <p className="text-xs text-slate-500 font-mono mt-1">{whale.address}</p>
        </div>
        <a
          href={`https://polygonscan.com/address/${whale.address}`}
          target="_blank"
          rel="noopener noreferrer"
          className="text-xs text-indigo-400 hover:text-indigo-300"
        >
          Polygonscan
        </a>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
        <StatCard
          label="总盈亏"
          value={`$${pnl.toLocaleString('en-US', { maximumFractionDigits: 0 })}`}
          accent={pnl >= 0 ? 'emerald' : 'red'}
          trend={pnl >= 0 ? 'up' : 'down'}
        />
        <StatCard
          label="胜率"
          value={`${winRate.toFixed(1)}%`}
          accent={winRate >= 55 ? 'emerald' : winRate >= 45 ? 'amber' : 'red'}
        />
        <StatCard label="夏普比率" value={sharpe.toFixed(2)} accent="indigo" />
        <StatCard label="凯利系数" value={kelly.toFixed(3)} accent="cyan" />
        <StatCard label="期望值" value={`$${ev.toFixed(2)}`} accent="amber" />
        <StatCard label="总交易数" value={whale.total_trades ?? 0} accent="default" />
      </div>

      {/* Volume chart */}
      {volumeData.length > 0 && (
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4">
          <h3 className="text-sm font-medium text-white mb-3">交易量 (近30天)</h3>
          <ResponsiveContainer width="100%" height={200}>
            <BarChart data={volumeData} stackOffset="sign">
              <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
              <XAxis dataKey="date" tick={{ fill: '#64748b', fontSize: 10 }} />
              <YAxis tick={{ fill: '#64748b', fontSize: 10 }} />
              <Tooltip
                contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #334155', borderRadius: 8 }}
                labelStyle={{ color: '#e2e8f0' }}
                formatter={(v: number | undefined) => [`$${Math.abs(v ?? 0).toFixed(0)}`, (v ?? 0) >= 0 ? '买入' : '卖出']}
              />
              <Bar dataKey="buys" stackId="a" fill="#10b981" radius={[2, 2, 0, 0]} />
              <Bar dataKey="sells" stackId="a" fill="#ef4444" radius={[0, 0, 2, 2]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      )}

      {/* Trade summary */}
      <div className="grid grid-cols-3 gap-3">
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4 text-center">
          <p className="text-xs text-slate-400">买入交易</p>
          <p className="text-2xl font-bold text-emerald-400 font-mono">{buyCount}</p>
        </div>
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4 text-center">
          <p className="text-xs text-slate-400">卖出交易</p>
          <p className="text-2xl font-bold text-red-400 font-mono">{sellCount}</p>
        </div>
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4 text-center">
          <p className="text-xs text-slate-400">总成交额</p>
          <p className="text-2xl font-bold text-white font-mono">
            ${totalNotional.toLocaleString('en-US', { maximumFractionDigits: 0 })}
          </p>
        </div>
      </div>

      {/* Trade history table */}
      <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50">
        <div className="px-4 py-3 border-b border-slate-700/50">
          <h3 className="text-sm font-medium text-white">
            交易历史 ({(trades ?? []).length} 笔)
          </h3>
        </div>
        <div className="overflow-x-auto max-h-[400px] overflow-y-auto">
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-slate-800">
              <tr className="text-xs text-slate-400 uppercase border-b border-slate-700/50">
                <th className="text-left px-4 py-2 font-medium">时间</th>
                <th className="text-left px-4 py-2 font-medium">市场</th>
                <th className="text-left px-4 py-2 font-medium">方向</th>
                <th className="text-right px-4 py-2 font-medium">数量</th>
                <th className="text-right px-4 py-2 font-medium">价格</th>
                <th className="text-right px-4 py-2 font-medium">名义价值</th>
              </tr>
            </thead>
            <tbody>
              {(trades ?? []).length === 0 ? (
                <tr>
                  <td colSpan={6} className="px-4 py-8 text-center text-slate-500">
                    暂无交易记录
                  </td>
                </tr>
              ) : (
                (trades ?? []).slice(0, 100).map((t) => (
                  <tr key={t.id} className="border-b border-slate-700/20 hover:bg-slate-700/20">
                    <td className="px-4 py-2 text-xs text-slate-400">
                      {new Date(t.traded_at).toLocaleString('zh-CN')}
                    </td>
                    <td className="px-4 py-2 font-mono text-xs text-slate-300">
                      {t.market_id.slice(0, 16)}...
                    </td>
                    <td className="px-4 py-2">
                      <StatusBadge status={t.side} />
                    </td>
                    <td className="px-4 py-2 text-right font-mono text-slate-300">
                      {Number(t.size).toFixed(2)}
                    </td>
                    <td className="px-4 py-2 text-right font-mono text-slate-300">
                      {Number(t.price).toFixed(4)}
                    </td>
                    <td className="px-4 py-2 text-right font-mono text-white">
                      ${Number(t.notional).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
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
