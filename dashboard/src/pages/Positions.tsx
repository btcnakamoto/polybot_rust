import { useMemo, useState } from 'react';
import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { fetchPositions, closePosition } from '../services/api';
import type { Position } from '../types';
import StatusBadge from '../components/StatusBadge';
import StatCard from '../components/StatCard';

export default function Positions() {
  const [showClosed, setShowClosed] = useState(false);
  const [closingPosition, setClosingPosition] = useState<Position | null>(null);
  const [closePrice, setClosePrice] = useState('');
  const queryClient = useQueryClient();

  const { data: positions, isLoading } = useQuery({
    queryKey: ['positions'],
    queryFn: fetchPositions,
    refetchInterval: 15_000,
  });

  const closeMutation = useMutation({
    mutationFn: ({ id, price }: { id: string; price?: string }) =>
      closePosition(id, price),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['positions'] });
      setClosingPosition(null);
      setClosePrice('');
    },
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

  const openCloseModal = (p: Position) => {
    setClosingPosition(p);
    setClosePrice(p.current_price ?? '');
    closeMutation.reset();
  };

  const handleConfirmClose = () => {
    if (!closingPosition) return;
    const price = closePrice.trim() || undefined;
    closeMutation.mutate({ id: closingPosition.id, price });
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
        <div>
          <h2 className="text-lg sm:text-xl font-semibold text-white">持仓管理</h2>
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

      <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 sm:gap-3">
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
          <table className="w-full text-sm min-w-[780px]">
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
                <th className="text-center px-4 py-3 font-medium">操作</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={9} className="px-4 py-12 text-center text-slate-500">
                    <div className="flex items-center justify-center gap-2">
                      <div className="w-4 h-4 border-2 border-slate-600 border-t-indigo-400 rounded-full animate-spin" />
                      加载中...
                    </div>
                  </td>
                </tr>
              ) : filtered.length === 0 ? (
                <tr>
                  <td colSpan={9} className="px-4 py-12 text-center text-slate-500">
                    暂无持仓
                  </td>
                </tr>
              ) : (
                filtered.map((p) => (
                  <tr key={p.id} className="border-b border-slate-700/30 hover:bg-slate-700/20 transition-colors">
                    <td className="px-4 py-2 font-mono text-xs">
                      {p.market_slug ? (
                        <a
                          href={`https://polymarket.com/event/${p.market_slug}`}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-indigo-400 hover:text-indigo-300 hover:underline"
                          title={p.market_question ?? p.market_id}
                        >
                          {p.market_question
                            ? p.market_question.length > 30
                              ? p.market_question.slice(0, 30) + '...'
                              : p.market_question
                            : p.market_id.slice(0, 14) + '...'}
                        </a>
                      ) : (
                        <span className="text-slate-300" title={p.market_id}>
                          {p.market_question
                            ? p.market_question.length > 30
                              ? p.market_question.slice(0, 30) + '...'
                              : p.market_question
                            : p.market_id.slice(0, 14) + '...'}
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-2 text-slate-300">
                      {p.outcome_label && !['Yes', 'No'].includes(p.outcome_label)
                        ? <>{p.outcome_label} <span className="text-xs text-slate-500">({p.outcome})</span></>
                        : p.outcome}
                    </td>
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
                      {Number(p.avg_entry_price) > 0 && Number(p.size) > 0 && (
                        <span className="text-xs opacity-70 ml-1">
                          ({((Number(p.unrealized_pnl ?? 0) / (Number(p.avg_entry_price) * Number(p.size))) * 100).toFixed(1)}%)
                        </span>
                      )}
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
                    <td className="px-4 py-2 text-center">
                      {p.status === 'open' ? (
                        <button
                          onClick={() => openCloseModal(p)}
                          className="px-2.5 py-1 rounded text-xs font-medium bg-red-500/20 text-red-400 hover:bg-red-500/30 transition-colors"
                        >
                          平仓
                        </button>
                      ) : p.status === 'exiting' ? (
                        <span className="text-xs text-yellow-400">退出中...</span>
                      ) : null}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Close position confirmation modal */}
      {closingPosition && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
          <div className="bg-slate-800 border border-slate-700 rounded-xl p-6 w-full max-w-md mx-4 shadow-2xl">
            <h3 className="text-lg font-semibold text-white mb-4">确认平仓</h3>

            <div className="space-y-2 text-sm mb-4">
              <div className="flex justify-between">
                <span className="text-slate-400">市场</span>
                <span className="text-slate-200 text-xs">
                  {closingPosition.market_question
                    ? closingPosition.market_question.length > 40
                      ? closingPosition.market_question.slice(0, 40) + '...'
                      : closingPosition.market_question
                    : closingPosition.market_id.slice(0, 20) + '...'}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">方向</span>
                <span className="text-slate-200">{closingPosition.outcome}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">数量</span>
                <span className="text-slate-200 font-mono">
                  {Number(closingPosition.size).toFixed(2)}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">入场均价</span>
                <span className="text-slate-200 font-mono">
                  {Number(closingPosition.avg_entry_price).toFixed(4)}
                </span>
              </div>
            </div>

            <div className="mb-4">
              <label className="block text-xs text-slate-400 mb-1">
                平仓价格（留空自动获取最优买价）
              </label>
              <input
                type="text"
                value={closePrice}
                onChange={(e) => setClosePrice(e.target.value)}
                placeholder="自动获取 best bid"
                className="w-full px-3 py-2 bg-slate-900 border border-slate-600 rounded-lg text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
              />
            </div>

            {closeMutation.isError && (
              <div className="mb-4 p-2 bg-red-500/10 border border-red-500/30 rounded text-xs text-red-400">
                {(closeMutation.error as Error).message}
              </div>
            )}

            <div className="flex gap-3">
              <button
                onClick={() => {
                  setClosingPosition(null);
                  setClosePrice('');
                }}
                disabled={closeMutation.isPending}
                className="flex-1 px-4 py-2 rounded-lg text-sm font-medium text-slate-300 bg-slate-700 hover:bg-slate-600 transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleConfirmClose}
                disabled={closeMutation.isPending}
                className="flex-1 px-4 py-2 rounded-lg text-sm font-medium text-white bg-red-600 hover:bg-red-500 transition-colors disabled:opacity-50"
              >
                {closeMutation.isPending ? '提交中...' : '确认平仓'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
