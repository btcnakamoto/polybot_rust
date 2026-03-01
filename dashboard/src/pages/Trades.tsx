import { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { fetchTrades } from '../services/api';
import StatusBadge from '../components/StatusBadge';
import StatCard from '../components/StatCard';

export default function Trades() {
  const [filterStatus, setFilterStatus] = useState('all');

  const { data: trades, isLoading } = useQuery({
    queryKey: ['trades'],
    queryFn: fetchTrades,
    refetchInterval: 10_000,
  });

  const filtered = useMemo(() => {
    if (filterStatus === 'all') return trades ?? [];
    return (trades ?? []).filter((t) => t.status === filterStatus);
  }, [trades, filterStatus]);

  const statuses = useMemo(() => {
    const set = new Set((trades ?? []).map((t) => t.status));
    return ['all', ...set];
  }, [trades]);

  // Stats
  const filledCount = (trades ?? []).filter((t) => t.status === 'filled').length;
  const failedCount = (trades ?? []).filter((t) => t.status === 'failed').length;
  const avgSlippage = (() => {
    const slips = (trades ?? [])
      .filter((t) => t.slippage)
      .map((t) => Math.abs(Number(t.slippage)));
    return slips.length > 0 ? slips.reduce((a, b) => a + b, 0) / slips.length : 0;
  })();

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-xl font-semibold text-white">跟单交易</h2>
        <p className="text-xs text-slate-500 mt-0.5">复制引擎执行的所有订单</p>
      </div>

      <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 sm:gap-3">
        <StatCard label="总订单" value={(trades ?? []).length} accent="indigo" />
        <StatCard label="已成交" value={filledCount} accent="emerald" />
        <StatCard label="失败" value={failedCount} accent="red" />
        <StatCard
          label="平均滑点"
          value={`${(avgSlippage * 100).toFixed(2)}%`}
          accent={avgSlippage < 0.01 ? 'emerald' : 'amber'}
        />
      </div>

      {/* Filter tabs */}
      <div className="flex gap-1 overflow-x-auto">
        {statuses.map((s) => (
          <button
            key={s}
            onClick={() => setFilterStatus(s)}
            className={`px-3 py-1 rounded-lg text-xs font-medium transition-colors ${
              filterStatus === s
                ? 'bg-indigo-500/20 text-indigo-400 ring-1 ring-indigo-500/30'
                : 'text-slate-400 hover:text-white hover:bg-slate-800'
            }`}
          >
            {s === 'all' ? '全部' : s}
          </button>
        ))}
      </div>

      <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50">
        <div className="overflow-x-auto">
          <table className="w-full text-sm min-w-[750px]">
            <thead>
              <tr className="text-xs text-slate-400 uppercase border-b border-slate-700/50">
                <th className="text-left px-4 py-3 font-medium">跟单来源</th>
                <th className="text-left px-4 py-3 font-medium">市场</th>
                <th className="text-left px-4 py-3 font-medium">方向</th>
                <th className="text-right px-4 py-3 font-medium">数量</th>
                <th className="text-right px-4 py-3 font-medium">目标价</th>
                <th className="text-right px-4 py-3 font-medium">成交价</th>
                <th className="text-right px-4 py-3 font-medium">滑点</th>
                <th className="text-left px-4 py-3 font-medium">策略</th>
                <th className="text-left px-4 py-3 font-medium">状态</th>
                <th className="text-left px-4 py-3 font-medium">时间</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={10} className="px-4 py-12 text-center text-slate-500">
                    <div className="flex items-center justify-center gap-2">
                      <div className="w-4 h-4 border-2 border-slate-600 border-t-indigo-400 rounded-full animate-spin" />
                      加载中...
                    </div>
                  </td>
                </tr>
              ) : filtered.length === 0 ? (
                <tr>
                  <td colSpan={10} className="px-4 py-12 text-center text-slate-500">
                    暂无跟单交易
                  </td>
                </tr>
              ) : (
                filtered.map((t) => (
                  <tr key={t.id} className="border-b border-slate-700/30 hover:bg-slate-700/20 transition-colors">
                    <td className="px-4 py-2">
                      {t.whale_address ? (
                        <Link
                          to={`/whales/${t.whale_address}`}
                          className="font-mono text-xs text-indigo-400 hover:text-indigo-300 hover:underline transition-colors"
                          title={t.whale_address}
                        >
                          {t.whale_label || `${t.whale_address.slice(0, 6)}...${t.whale_address.slice(-4)}`}
                        </Link>
                      ) : (
                        <span className="text-xs text-slate-500">--</span>
                      )}
                    </td>
                    <td className="px-4 py-2 text-xs text-slate-300 max-w-[200px] truncate" title={t.market_question || t.market_id}>
                      {t.market_question || `${t.market_id.slice(0, 14)}...`}
                    </td>
                    <td className="px-4 py-2">
                      <StatusBadge status={t.side} />
                    </td>
                    <td className="px-4 py-2 text-right font-mono text-slate-300">
                      {Number(t.size).toFixed(2)}
                    </td>
                    <td className="px-4 py-2 text-right font-mono text-slate-300">
                      {Number(t.target_price).toFixed(4)}
                    </td>
                    <td className="px-4 py-2 text-right font-mono text-slate-300">
                      {t.fill_price ? Number(t.fill_price).toFixed(4) : '--'}
                    </td>
                    <td className={`px-4 py-2 text-right font-mono text-xs ${
                      t.slippage && Math.abs(Number(t.slippage)) > 0.01 ? 'text-amber-400' : 'text-slate-400'
                    }`}>
                      {t.slippage ? `${(Number(t.slippage) * 100).toFixed(2)}%` : '--'}
                    </td>
                    <td className="px-4 py-2">
                      <span className="text-xs text-slate-400 bg-slate-700/50 px-2 py-0.5 rounded">
                        {t.strategy}
                      </span>
                    </td>
                    <td className="px-4 py-2">
                      <StatusBadge status={t.status} />
                    </td>
                    <td className="px-4 py-2 text-xs text-slate-500">
                      {t.placed_at ? new Date(t.placed_at).toLocaleString('zh-CN') : '--'}
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
