import { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { fetchTrades } from '../services/api';
import StatusBadge from '../components/StatusBadge';
import StatCard from '../components/StatCard';
import { Search, ChevronUp, ChevronDown, X, Filter, Calendar } from 'lucide-react';

type SortKey = 'placed_at' | 'size' | 'slippage';
type SortDir = 'asc' | 'desc';

export default function Trades() {
  const [filterStatus, setFilterStatus] = useState('all');
  const [filterSide, setFilterSide] = useState('all');
  const [searchText, setSearchText] = useState('');
  const [dateFrom, setDateFrom] = useState('');
  const [dateTo, setDateTo] = useState('');
  const [showFilters, setShowFilters] = useState(false);
  const [sortKey, setSortKey] = useState<SortKey>('placed_at');
  const [sortDir, setSortDir] = useState<SortDir>('desc');

  const { data: trades, isLoading } = useQuery({
    queryKey: ['trades'],
    queryFn: fetchTrades,
    refetchInterval: 10_000,
  });

  const allTrades = trades ?? [];

  const statuses = useMemo(() => {
    const set = new Set(allTrades.map((t) => t.status));
    return ['all', ...set];
  }, [allTrades]);

  const filtered = useMemo(() => {
    let list = [...allTrades];

    if (filterStatus !== 'all') list = list.filter((t) => t.status === filterStatus);
    if (filterSide !== 'all') list = list.filter((t) => t.side === filterSide);

    if (searchText.trim()) {
      const q = searchText.toLowerCase();
      list = list.filter(
        (t) =>
          (t.market_question ?? '').toLowerCase().includes(q) ||
          (t.whale_address ?? '').toLowerCase().includes(q) ||
          (t.whale_label ?? '').toLowerCase().includes(q) ||
          t.market_id.toLowerCase().includes(q),
      );
    }

    if (dateFrom) {
      const from = new Date(dateFrom);
      list = list.filter((t) => t.placed_at && new Date(t.placed_at) >= from);
    }
    if (dateTo) {
      const to = new Date(dateTo + 'T23:59:59');
      list = list.filter((t) => t.placed_at && new Date(t.placed_at) <= to);
    }

    list.sort((a, b) => {
      let va: number, vb: number;
      switch (sortKey) {
        case 'placed_at':
          va = a.placed_at ? new Date(a.placed_at).getTime() : 0;
          vb = b.placed_at ? new Date(b.placed_at).getTime() : 0;
          break;
        case 'size':
          va = Number(a.size);
          vb = Number(b.size);
          break;
        case 'slippage':
          va = Math.abs(Number(a.slippage ?? 0));
          vb = Math.abs(Number(b.slippage ?? 0));
          break;
        default:
          va = 0; vb = 0;
      }
      return sortDir === 'asc' ? va - vb : vb - va;
    });

    return list;
  }, [allTrades, filterStatus, filterSide, searchText, dateFrom, dateTo, sortKey, sortDir]);

  const hasActiveFilters = searchText || dateFrom || dateTo || filterSide !== 'all';

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

  // Stats
  const filledCount = allTrades.filter((t) => t.status === 'filled').length;
  const failedCount = allTrades.filter((t) => t.status === 'failed').length;
  const avgSlippage = (() => {
    const slips = allTrades.filter((t) => t.slippage).map((t) => Math.abs(Number(t.slippage)));
    return slips.length > 0 ? slips.reduce((a, b) => a + b, 0) / slips.length : 0;
  })();

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-xl font-semibold text-white">跟单交易</h2>
        <p className="text-xs text-slate-500 mt-0.5">复制引擎执行的所有订单</p>
      </div>

      <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 sm:gap-3">
        <StatCard label="总订单" value={allTrades.length} accent="indigo" />
        <StatCard label="已成交" value={filledCount} accent="emerald" />
        <StatCard label="失败" value={failedCount} accent="red" />
        <StatCard
          label="平均滑点"
          value={`${(avgSlippage * 100).toFixed(2)}%`}
          accent={avgSlippage < 0.01 ? 'emerald' : 'amber'}
        />
      </div>

      {/* Status tabs + search + filter */}
      <div className="flex flex-col sm:flex-row gap-2 sm:items-center justify-between">
        <div className="flex gap-1 overflow-x-auto flex-wrap">
          {statuses.map((s) => (
            <button
              key={s}
              onClick={() => setFilterStatus(s)}
              className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                filterStatus === s
                  ? 'bg-indigo-500/20 text-indigo-400 ring-1 ring-indigo-500/30'
                  : 'text-slate-400 hover:text-white hover:bg-slate-800'
              }`}
            >
              {s === 'all' ? '全部' : s}
              <span className="ml-1 text-[10px] opacity-70">
                {s === 'all' ? allTrades.length : allTrades.filter((t) => t.status === s).length}
              </span>
            </button>
          ))}
        </div>

        <div className="flex gap-2 items-center">
          <div className="relative flex-1 sm:w-56">
            <Search size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-500" />
            <input
              value={searchText}
              onChange={(e) => setSearchText(e.target.value)}
              placeholder="搜索市场/巨鲸..."
              className="w-full pl-8 pr-3 py-1.5 bg-slate-800 border border-slate-700 rounded-lg text-xs text-white placeholder-slate-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
            />
            {searchText && (
              <button onClick={() => setSearchText('')} className="absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 hover:text-white">
                <X size={12} />
              </button>
            )}
          </div>
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
              <button
                onClick={() => { setSearchText(''); setDateFrom(''); setDateTo(''); setFilterSide('all'); }}
                className="text-[10px] text-indigo-400 hover:text-indigo-300"
              >
                清除所有筛选
              </button>
            )}
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <div>
              <label className="block text-[10px] text-slate-500 mb-1">方向</label>
              <select
                value={filterSide}
                onChange={(e) => setFilterSide(e.target.value)}
                className="w-full px-3 py-1.5 bg-slate-900 border border-slate-600 rounded-lg text-xs text-white focus:outline-none focus:ring-1 focus:ring-indigo-500"
              >
                <option value="all">全部</option>
                <option value="BUY">BUY</option>
                <option value="SELL">SELL</option>
              </select>
            </div>
            <div>
              <label className="flex items-center gap-1 text-[10px] text-slate-500 mb-1">
                <Calendar size={10} /> 起始日期
              </label>
              <input
                type="date"
                value={dateFrom}
                onChange={(e) => setDateFrom(e.target.value)}
                className="w-full px-3 py-1.5 bg-slate-900 border border-slate-600 rounded-lg text-xs text-white focus:outline-none focus:ring-1 focus:ring-indigo-500"
              />
            </div>
            <div>
              <label className="flex items-center gap-1 text-[10px] text-slate-500 mb-1">
                <Calendar size={10} /> 截止日期
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

      <div className="text-[10px] text-slate-500">
        显示 {filtered.length} / {allTrades.length} 条记录
      </div>

      <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50">
        <div className="overflow-x-auto">
          <table className="w-full text-sm min-w-[750px]">
            <thead>
              <tr className="text-xs text-slate-400 uppercase border-b border-slate-700/50">
                <th className="text-left px-4 py-3 font-medium">跟单来源</th>
                <th className="text-left px-4 py-3 font-medium">市场</th>
                <th className="text-left px-4 py-3 font-medium">方向</th>
                <th
                  className="text-right px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => toggleSort('size')}
                >
                  数量 <SortIcon col="size" />
                </th>
                <th className="text-right px-4 py-3 font-medium">目标价</th>
                <th className="text-right px-4 py-3 font-medium">成交价</th>
                <th
                  className="text-right px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => toggleSort('slippage')}
                >
                  滑点 <SortIcon col="slippage" />
                </th>
                <th className="text-left px-4 py-3 font-medium">策略</th>
                <th className="text-left px-4 py-3 font-medium">状态</th>
                <th
                  className="text-left px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => toggleSort('placed_at')}
                >
                  时间 <SortIcon col="placed_at" />
                </th>
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
                    {hasActiveFilters ? '没有符合筛选条件的订单' : '暂无跟单交易'}
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
                    <td className="px-4 py-2 text-right font-mono text-slate-300 text-xs">
                      {Number(t.size).toFixed(2)}
                    </td>
                    <td className="px-4 py-2 text-right font-mono text-slate-300 text-xs">
                      {Number(t.target_price).toFixed(4)}
                    </td>
                    <td className="px-4 py-2 text-right font-mono text-slate-300 text-xs">
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
                    <td className="px-4 py-2 text-[11px] text-slate-500 whitespace-nowrap">
                      {t.placed_at ? new Date(t.placed_at).toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' }) : '--'}
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
