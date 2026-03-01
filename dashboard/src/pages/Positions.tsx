import { useMemo, useState } from 'react';
import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query';
import { fetchPositions, closePosition } from '../services/api';
import type { Position } from '../types';
import StatusBadge from '../components/StatusBadge';
import StatCard from '../components/StatCard';
import { Search, ChevronUp, ChevronDown, X, Filter, Calendar } from 'lucide-react';

type StatusFilter = 'all' | 'open' | 'exiting' | 'closed';
type SortKey = 'opened_at' | 'closed_at' | 'unrealized_pnl' | 'realized_pnl' | 'size' | 'pnl_pct' | 'hold_days';
type SortDir = 'asc' | 'desc';
type ExitReasonFilter = '' | 'stop_loss' | 'take_profit' | 'trailing_stop' | 'time_exit' | 'whale_exit';

const EXIT_REASON_LABELS: Record<string, string> = {
  stop_loss: '止损',
  take_profit: '止盈',
  trailing_stop: '移动止损',
  time_exit: '时间止损',
  whale_exit: '巨鲸跟随',
};

function holdDays(p: Position): number {
  if (!p.opened_at) return 0;
  const end = p.closed_at ? new Date(p.closed_at) : new Date();
  return Math.max(0, (end.getTime() - new Date(p.opened_at).getTime()) / 86400000);
}

function pnlPct(p: Position): number {
  const entry = Number(p.avg_entry_price);
  const size = Number(p.size);
  if (entry <= 0 || size <= 0) return 0;
  const cost = entry * size;
  if (p.status === 'closed') return (Number(p.realized_pnl ?? 0) / cost) * 100;
  return (Number(p.unrealized_pnl ?? 0) / cost) * 100;
}

export default function Positions() {
  // --- filters ---
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
  const [searchText, setSearchText] = useState('');
  const [exitReasonFilter, setExitReasonFilter] = useState<ExitReasonFilter>('');
  const [dateFrom, setDateFrom] = useState('');
  const [dateTo, setDateTo] = useState('');
  const [showFilters, setShowFilters] = useState(false);

  // --- sorting ---
  const [sortKey, setSortKey] = useState<SortKey>('opened_at');
  const [sortDir, setSortDir] = useState<SortDir>('desc');

  // --- close modal ---
  const [closingPosition, setClosingPosition] = useState<Position | null>(null);
  const [closePrice, setClosePrice] = useState('');
  const queryClient = useQueryClient();

  const { data: positions, isLoading } = useQuery({
    queryKey: ['positions'],
    queryFn: fetchPositions,
    refetchInterval: 15_000,
  });

  const closeMutation = useMutation({
    mutationFn: ({ id, price }: { id: string; price?: string }) => closePosition(id, price),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['positions'] });
      setClosingPosition(null);
      setClosePrice('');
    },
  });

  // --- computed stats (on ALL positions, ignoring filters) ---
  const allPositions = positions ?? [];
  const openCount = allPositions.filter((p) => p.status === 'open' || p.status === 'exiting').length;
  const closedCount = allPositions.filter((p) => p.status === 'closed').length;

  const totalUnrealized = allPositions
    .filter((p) => p.status !== 'closed')
    .reduce((s, p) => s + Number(p.unrealized_pnl ?? 0), 0);
  const totalRealized = allPositions
    .filter((p) => p.status === 'closed')
    .reduce((s, p) => s + Number(p.realized_pnl ?? 0), 0);

  // Win rate for closed positions
  const closedPositions = allPositions.filter((p) => p.status === 'closed');
  const winCount = closedPositions.filter((p) => Number(p.realized_pnl ?? 0) > 0).length;
  const winRate = closedPositions.length > 0 ? ((winCount / closedPositions.length) * 100).toFixed(1) : '--';

  // Unique exit reasons from data
  const exitReasons = useMemo(() => {
    const reasons = new Set<string>();
    allPositions.forEach((p) => { if (p.exit_reason) reasons.add(p.exit_reason); });
    return Array.from(reasons).sort();
  }, [allPositions]);

  // --- filtering ---
  const filtered = useMemo(() => {
    let list = [...allPositions];

    // Status filter
    if (statusFilter === 'open') list = list.filter((p) => p.status === 'open');
    else if (statusFilter === 'exiting') list = list.filter((p) => p.status === 'exiting');
    else if (statusFilter === 'closed') list = list.filter((p) => p.status === 'closed');

    // Search
    if (searchText.trim()) {
      const q = searchText.toLowerCase();
      list = list.filter(
        (p) =>
          (p.market_question ?? '').toLowerCase().includes(q) ||
          (p.outcome_label ?? p.outcome ?? '').toLowerCase().includes(q) ||
          p.market_id.toLowerCase().includes(q) ||
          p.token_id.toLowerCase().includes(q),
      );
    }

    // Exit reason
    if (exitReasonFilter) {
      list = list.filter((p) => p.exit_reason === exitReasonFilter);
    }

    // Date range (based on opened_at)
    if (dateFrom) {
      const from = new Date(dateFrom);
      list = list.filter((p) => p.opened_at && new Date(p.opened_at) >= from);
    }
    if (dateTo) {
      const to = new Date(dateTo + 'T23:59:59');
      list = list.filter((p) => p.opened_at && new Date(p.opened_at) <= to);
    }

    // Sort
    list.sort((a, b) => {
      let va: number, vb: number;
      switch (sortKey) {
        case 'opened_at':
          va = a.opened_at ? new Date(a.opened_at).getTime() : 0;
          vb = b.opened_at ? new Date(b.opened_at).getTime() : 0;
          break;
        case 'closed_at':
          va = a.closed_at ? new Date(a.closed_at).getTime() : 0;
          vb = b.closed_at ? new Date(b.closed_at).getTime() : 0;
          break;
        case 'unrealized_pnl':
          va = Number(a.unrealized_pnl ?? 0);
          vb = Number(b.unrealized_pnl ?? 0);
          break;
        case 'realized_pnl':
          va = Number(a.realized_pnl ?? 0);
          vb = Number(b.realized_pnl ?? 0);
          break;
        case 'size':
          va = Number(a.size);
          vb = Number(b.size);
          break;
        case 'pnl_pct':
          va = pnlPct(a);
          vb = pnlPct(b);
          break;
        case 'hold_days':
          va = holdDays(a);
          vb = holdDays(b);
          break;
        default:
          va = 0;
          vb = 0;
      }
      return sortDir === 'asc' ? va - vb : vb - va;
    });

    return list;
  }, [allPositions, statusFilter, searchText, exitReasonFilter, dateFrom, dateTo, sortKey, sortDir]);

  const hasActiveFilters = searchText || exitReasonFilter || dateFrom || dateTo;

  const toggleSort = (key: SortKey) => {
    if (sortKey === key) setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
    else { setSortKey(key); setSortDir('desc'); }
  };

  const SortIcon = ({ col }: { col: SortKey }) => {
    if (sortKey !== col) return <span className="ml-0.5 text-slate-600 text-[10px]">&#x2195;</span>;
    return sortDir === 'asc'
      ? <ChevronUp size={12} className="ml-0.5 inline text-indigo-400" />
      : <ChevronDown size={12} className="ml-0.5 inline text-indigo-400" />;
  };

  const openCloseModal = (p: Position) => {
    setClosingPosition(p);
    setClosePrice(p.current_price ?? '');
    closeMutation.reset();
  };

  const handleConfirmClose = () => {
    if (!closingPosition) return;
    closeMutation.mutate({ id: closingPosition.id, price: closePrice.trim() || undefined });
  };

  const clearFilters = () => {
    setSearchText('');
    setExitReasonFilter('');
    setDateFrom('');
    setDateTo('');
  };

  const statusTabs: { key: StatusFilter; label: string; count: number }[] = [
    { key: 'all', label: '全部', count: allPositions.length },
    { key: 'open', label: '持仓中', count: allPositions.filter((p) => p.status === 'open').length },
    { key: 'exiting', label: '退出中', count: allPositions.filter((p) => p.status === 'exiting').length },
    { key: 'closed', label: '已平仓', count: closedCount },
  ];

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
        <div>
          <h2 className="text-lg sm:text-xl font-semibold text-white">持仓管理</h2>
          <p className="text-xs text-slate-500 mt-0.5">
            {openCount} 个活跃 / {closedCount} 个已平仓 / 胜率 {winRate}%
          </p>
        </div>
      </div>

      {/* Stat cards */}
      <div className="grid grid-cols-2 sm:grid-cols-5 gap-2 sm:gap-3">
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
        <StatCard
          label="胜率"
          value={winRate === '--' ? '--' : `${winRate}%`}
          accent={Number(winRate) >= 50 ? 'emerald' : Number(winRate) > 0 ? 'amber' : 'default'}
          sub={closedPositions.length > 0 ? `${winCount}/${closedPositions.length}` : undefined}
        />
      </div>

      {/* Status tabs + search + filter toggle */}
      <div className="flex flex-col sm:flex-row gap-2 sm:items-center justify-between">
        {/* Status tabs */}
        <div className="flex gap-1 flex-wrap">
          {statusTabs.map((tab) => (
            <button
              key={tab.key}
              onClick={() => setStatusFilter(tab.key)}
              className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                statusFilter === tab.key
                  ? 'bg-indigo-500/20 text-indigo-400 ring-1 ring-indigo-500/30'
                  : 'text-slate-400 hover:text-white bg-slate-800 border border-slate-700'
              }`}
            >
              {tab.label}
              <span className="ml-1 text-[10px] opacity-70">{tab.count}</span>
            </button>
          ))}
        </div>

        <div className="flex gap-2 items-center">
          {/* Search */}
          <div className="relative flex-1 sm:w-56">
            <Search size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-500" />
            <input
              value={searchText}
              onChange={(e) => setSearchText(e.target.value)}
              placeholder="搜索市场..."
              className="w-full pl-8 pr-3 py-1.5 bg-slate-800 border border-slate-700 rounded-lg text-xs text-white placeholder-slate-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
            />
            {searchText && (
              <button onClick={() => setSearchText('')} className="absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 hover:text-white">
                <X size={12} />
              </button>
            )}
          </div>
          {/* Filter toggle */}
          <button
            onClick={() => setShowFilters(!showFilters)}
            className={`flex items-center gap-1 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
              showFilters || hasActiveFilters
                ? 'bg-indigo-500/20 text-indigo-400 ring-1 ring-indigo-500/30'
                : 'text-slate-400 hover:text-white bg-slate-800 border border-slate-700'
            }`}
          >
            <Filter size={12} />
            筛选
            {hasActiveFilters && <span className="w-1.5 h-1.5 rounded-full bg-indigo-400" />}
          </button>
        </div>
      </div>

      {/* Expandable filter panel */}
      {showFilters && (
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-4 space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-slate-300">高级筛选</span>
            {hasActiveFilters && (
              <button onClick={clearFilters} className="text-[10px] text-indigo-400 hover:text-indigo-300">
                清除所有筛选
              </button>
            )}
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            {/* Exit reason */}
            <div>
              <label className="block text-[10px] text-slate-500 mb-1">平仓原因</label>
              <select
                value={exitReasonFilter}
                onChange={(e) => setExitReasonFilter(e.target.value as ExitReasonFilter)}
                className="w-full px-3 py-1.5 bg-slate-900 border border-slate-600 rounded-lg text-xs text-white focus:outline-none focus:ring-1 focus:ring-indigo-500"
              >
                <option value="">全部</option>
                {exitReasons.map((r) => (
                  <option key={r} value={r}>{EXIT_REASON_LABELS[r] ?? r}</option>
                ))}
              </select>
            </div>
            {/* Date from */}
            <div>
              <label className="flex items-center gap-1 text-[10px] text-slate-500 mb-1">
                <Calendar size={10} /> 开仓起始
              </label>
              <input
                type="date"
                value={dateFrom}
                onChange={(e) => setDateFrom(e.target.value)}
                className="w-full px-3 py-1.5 bg-slate-900 border border-slate-600 rounded-lg text-xs text-white focus:outline-none focus:ring-1 focus:ring-indigo-500"
              />
            </div>
            {/* Date to */}
            <div>
              <label className="flex items-center gap-1 text-[10px] text-slate-500 mb-1">
                <Calendar size={10} /> 开仓截止
              </label>
              <input
                type="date"
                value={dateTo}
                onChange={(e) => setDateTo(e.target.value)}
                className="w-full px-3 py-1.5 bg-slate-900 border border-slate-600 rounded-lg text-xs text-white focus:outline-none focus:ring-1 focus:ring-indigo-500"
              />
            </div>
          </div>
        </div>
      )}

      {/* Results count */}
      <div className="text-[10px] text-slate-500">
        显示 {filtered.length} / {allPositions.length} 条记录
        {hasActiveFilters && ' (已筛选)'}
      </div>

      {/* Table */}
      <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50">
        <div className="overflow-x-auto">
          <table className="w-full text-sm min-w-[960px]">
            <thead>
              <tr className="text-xs text-slate-400 uppercase border-b border-slate-700/50">
                <th className="text-left px-4 py-3 font-medium">市场</th>
                <th className="text-left px-4 py-3 font-medium">结果</th>
                <th
                  className="text-right px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => toggleSort('size')}
                >
                  数量 <SortIcon col="size" />
                </th>
                <th className="text-right px-4 py-3 font-medium">均价</th>
                <th className="text-right px-4 py-3 font-medium">现价</th>
                <th
                  className="text-right px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => toggleSort('pnl_pct')}
                >
                  收益% <SortIcon col="pnl_pct" />
                </th>
                <th
                  className="text-right px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => toggleSort('unrealized_pnl')}
                >
                  浮动盈亏 <SortIcon col="unrealized_pnl" />
                </th>
                <th
                  className="text-right px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => toggleSort('realized_pnl')}
                >
                  已实现 <SortIcon col="realized_pnl" />
                </th>
                <th className="text-left px-4 py-3 font-medium">状态</th>
                <th
                  className="text-left px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => toggleSort('opened_at')}
                >
                  开仓时间 <SortIcon col="opened_at" />
                </th>
                <th
                  className="text-right px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => toggleSort('hold_days')}
                >
                  持仓天数 <SortIcon col="hold_days" />
                </th>
                <th className="text-center px-4 py-3 font-medium">操作</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={12} className="px-4 py-12 text-center text-slate-500">
                    <div className="flex items-center justify-center gap-2">
                      <div className="w-4 h-4 border-2 border-slate-600 border-t-indigo-400 rounded-full animate-spin" />
                      加载中...
                    </div>
                  </td>
                </tr>
              ) : filtered.length === 0 ? (
                <tr>
                  <td colSpan={12} className="px-4 py-12 text-center text-slate-500">
                    {hasActiveFilters ? '没有符合筛选条件的持仓' : '暂无持仓'}
                  </td>
                </tr>
              ) : (
                filtered.map((p) => {
                  const pct = pnlPct(p);
                  const days = holdDays(p);
                  const isClosed = p.status === 'closed';
                  return (
                    <tr key={p.id} className={`border-b border-slate-700/30 hover:bg-slate-700/20 transition-colors ${isClosed ? 'opacity-70' : ''}`}>
                      {/* Market */}
                      <td className="px-4 py-2 font-mono text-xs max-w-[200px]">
                        {p.market_slug ? (
                          <a
                            href={`https://polymarket.com/event/${p.market_slug}`}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-indigo-400 hover:text-indigo-300 hover:underline block truncate"
                            title={p.market_question ?? p.market_id}
                          >
                            {p.market_question
                              ? p.market_question.length > 36 ? p.market_question.slice(0, 36) + '...' : p.market_question
                              : p.market_id.slice(0, 14) + '...'}
                          </a>
                        ) : (
                          <span className="text-slate-300 block truncate" title={p.market_question ?? p.market_id}>
                            {p.market_question
                              ? p.market_question.length > 36 ? p.market_question.slice(0, 36) + '...' : p.market_question
                              : p.market_id.slice(0, 14) + '...'}
                          </span>
                        )}
                      </td>
                      {/* Outcome */}
                      <td className="px-4 py-2 text-slate-300 text-xs">
                        {(() => {
                          const label = p.outcome_label ?? p.outcome;
                          if (!['Yes', 'No'].includes(label)) {
                            return <>{label} <span className="text-[10px] text-slate-500">({label === p.outcome ? '' : p.outcome})</span></>;
                          }
                          return label;
                        })()}
                      </td>
                      {/* Size */}
                      <td className="px-4 py-2 text-right font-mono text-slate-300 text-xs">
                        {Number(p.size).toFixed(2)}
                      </td>
                      {/* Avg entry */}
                      <td className="px-4 py-2 text-right font-mono text-slate-300 text-xs">
                        {Number(p.avg_entry_price).toFixed(4)}
                      </td>
                      {/* Current price */}
                      <td className="px-4 py-2 text-right font-mono text-slate-300 text-xs">
                        {p.current_price ? Number(p.current_price).toFixed(4) : '--'}
                      </td>
                      {/* PnL % */}
                      <td className={`px-4 py-2 text-right font-mono text-xs font-semibold ${
                        pct >= 0 ? 'text-emerald-400' : 'text-red-400'
                      }`}>
                        {pct >= 0 ? '+' : ''}{pct.toFixed(1)}%
                      </td>
                      {/* Unrealized PnL */}
                      <td className={`px-4 py-2 text-right font-mono text-xs font-medium ${
                        Number(p.unrealized_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'
                      }`}>
                        {isClosed ? <span className="text-slate-600">--</span> : `$${Number(p.unrealized_pnl ?? 0).toFixed(2)}`}
                      </td>
                      {/* Realized PnL */}
                      <td className={`px-4 py-2 text-right font-mono text-xs font-medium ${
                        Number(p.realized_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'
                      }`}>
                        {!isClosed && !Number(p.realized_pnl)
                          ? <span className="text-slate-600">--</span>
                          : `$${Number(p.realized_pnl ?? 0).toFixed(2)}`
                        }
                      </td>
                      {/* Status + exit reason */}
                      <td className="px-4 py-2">
                        <div className="flex flex-col gap-0.5">
                          <StatusBadge status={p.status ?? 'open'} />
                          {p.exit_reason && (
                            <span className="text-[10px] text-slate-500">
                              {EXIT_REASON_LABELS[p.exit_reason] ?? p.exit_reason}
                            </span>
                          )}
                        </div>
                      </td>
                      {/* Opened at */}
                      <td className="px-4 py-2 text-[11px] text-slate-500 whitespace-nowrap">
                        {p.opened_at ? new Date(p.opened_at).toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' }) : '--'}
                        {isClosed && p.closed_at && (
                          <div className="text-[10px] text-slate-600">
                            平 {new Date(p.closed_at).toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })}
                          </div>
                        )}
                      </td>
                      {/* Hold days */}
                      <td className={`px-4 py-2 text-right font-mono text-xs ${
                        days >= 7 ? 'text-amber-400' : days >= 3 ? 'text-slate-300' : 'text-slate-500'
                      }`}>
                        {days.toFixed(1)}d
                      </td>
                      {/* Actions */}
                      <td className="px-4 py-2 text-center">
                        {p.status === 'open' ? (
                          <button
                            onClick={() => openCloseModal(p)}
                            className="px-2.5 py-1 rounded text-xs font-medium bg-red-500/20 text-red-400 hover:bg-red-500/30 transition-colors"
                          >
                            平仓
                          </button>
                        ) : p.status === 'exiting' ? (
                          <span className="text-[10px] text-amber-400">退出中...</span>
                        ) : null}
                      </td>
                    </tr>
                  );
                })
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
                <span className="text-slate-200 text-xs text-right max-w-[250px] truncate">
                  {closingPosition.market_question
                    ? closingPosition.market_question.length > 50
                      ? closingPosition.market_question.slice(0, 50) + '...'
                      : closingPosition.market_question
                    : closingPosition.market_id.slice(0, 20) + '...'}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">方向</span>
                <span className="text-slate-200">{closingPosition.outcome_label ?? closingPosition.outcome}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">数量</span>
                <span className="text-slate-200 font-mono">{Number(closingPosition.size).toFixed(2)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">入场均价</span>
                <span className="text-slate-200 font-mono">{Number(closingPosition.avg_entry_price).toFixed(4)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">当前价</span>
                <span className="text-slate-200 font-mono">{closingPosition.current_price ? Number(closingPosition.current_price).toFixed(4) : '--'}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">浮动盈亏</span>
                <span className={`font-mono ${Number(closingPosition.unrealized_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'}`}>
                  ${Number(closingPosition.unrealized_pnl ?? 0).toFixed(2)} ({pnlPct(closingPosition).toFixed(1)}%)
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
                onClick={() => { setClosingPosition(null); setClosePrice(''); }}
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
