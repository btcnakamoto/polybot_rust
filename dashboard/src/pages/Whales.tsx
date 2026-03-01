import { useState, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { fetchWhales } from '../services/api';
import StatusBadge from '../components/StatusBadge';
import { ExternalLink, History } from 'lucide-react';
import type { Whale } from '../types';

type SortKey = 'total_pnl' | 'win_rate' | 'sharpe_ratio' | 'total_trades';

export default function Whales() {
  const navigate = useNavigate();
  const [sortKey, setSortKey] = useState<SortKey>('total_pnl');
  const [sortAsc, setSortAsc] = useState(false);
  const [filter, setFilter] = useState('');

  const { data: whales, isLoading } = useQuery({
    queryKey: ['whales'],
    queryFn: fetchWhales,
    refetchInterval: 30_000,
  });

  const sorted = useMemo(() => {
    let list = [...(whales ?? [])];
    if (filter) {
      const f = filter.toLowerCase();
      list = list.filter(
        (w) =>
          w.address.toLowerCase().includes(f) ||
          (w.classification ?? '').toLowerCase().includes(f) ||
          (w.label ?? '').toLowerCase().includes(f)
      );
    }
    list.sort((a, b) => {
      const av = getVal(a, sortKey);
      const bv = getVal(b, sortKey);
      return sortAsc ? av - bv : bv - av;
    });
    return list;
  }, [whales, sortKey, sortAsc, filter]);

  const handleSort = (key: SortKey) => {
    if (sortKey === key) setSortAsc(!sortAsc);
    else {
      setSortKey(key);
      setSortAsc(false);
    }
  };

  const sortIcon = (key: SortKey) =>
    sortKey === key ? (sortAsc ? ' \u2191' : ' \u2193') : '';

  return (
    <div className="space-y-4">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
        <div>
          <h2 className="text-lg sm:text-xl font-semibold text-white">跟踪巨鲸</h2>
          <p className="text-xs text-slate-500 mt-0.5">{sorted.length} 个活跃巨鲸</p>
        </div>
        <input
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="搜索地址/分类..."
          className="bg-slate-800 border border-slate-700 rounded-lg px-3 py-1.5 text-sm text-white placeholder-slate-500 w-full sm:w-64 focus:outline-none focus:ring-1 focus:ring-indigo-500"
        />
      </div>

      <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50">
        <div className="overflow-x-auto">
          <table className="w-full text-sm min-w-[800px]">
            <thead>
              <tr className="text-xs text-slate-400 uppercase border-b border-slate-700/50">
                <th className="text-left px-4 py-3 font-medium">地址</th>
                <th className="text-left px-4 py-3 font-medium">分类</th>
                <th
                  className="text-right px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => handleSort('sharpe_ratio')}
                >
                  夏普{sortIcon('sharpe_ratio')}
                </th>
                <th
                  className="text-right px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => handleSort('win_rate')}
                >
                  胜率{sortIcon('win_rate')}
                </th>
                <th className="text-right px-4 py-3 font-medium">凯利</th>
                <th
                  className="text-right px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => handleSort('total_trades')}
                >
                  交易数{sortIcon('total_trades')}
                </th>
                <th
                  className="text-right px-4 py-3 font-medium cursor-pointer hover:text-white select-none"
                  onClick={() => handleSort('total_pnl')}
                >
                  总盈亏{sortIcon('total_pnl')}
                </th>
                <th className="text-left px-4 py-3 font-medium">状态</th>
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
              ) : sorted.length === 0 ? (
                <tr>
                  <td colSpan={9} className="px-4 py-12 text-center text-slate-500">
                    {filter ? '无匹配结果' : '暂无跟踪巨鲸'}
                  </td>
                </tr>
              ) : (
                sorted.map((w) => {
                  const pnl = Number(w.total_pnl ?? 0);
                  const winRate = Number(w.win_rate ?? 0) * 100;
                  return (
                    <tr
                      key={w.id}
                      className="border-b border-slate-700/30 hover:bg-slate-700/20 transition-colors"
                    >
                      <td className="px-4 py-3">
                        <a
                          href={`https://polymarket.com/profile/${w.address}`}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="font-mono text-xs text-indigo-400 hover:text-indigo-300 inline-flex items-center gap-1"
                          title="查看 Polymarket 主页"
                        >
                          {w.address.slice(0, 6)}...{w.address.slice(-4)}
                          <ExternalLink size={12} />
                        </a>
                      </td>
                      <td className="px-4 py-3">
                        <StatusBadge status={w.classification ?? 'unknown'} />
                      </td>
                      <td className="px-4 py-3 text-right font-mono text-slate-300">
                        {Number(w.sharpe_ratio ?? 0).toFixed(2)}
                      </td>
                      <td className="px-4 py-3 text-right">
                        <span className={`font-mono ${winRate >= 55 ? 'text-emerald-400' : winRate >= 45 ? 'text-amber-400' : 'text-red-400'}`}>
                          {winRate.toFixed(1)}%
                        </span>
                      </td>
                      <td className="px-4 py-3 text-right font-mono text-slate-300">
                        {Number(w.kelly_fraction ?? 0).toFixed(3)}
                      </td>
                      <td className="px-4 py-3 text-right font-mono text-slate-300">
                        {w.total_trades ?? 0}
                      </td>
                      <td className={`px-4 py-3 text-right font-mono font-medium ${
                        pnl >= 0 ? 'text-emerald-400' : 'text-red-400'
                      }`}>
                        ${pnl.toLocaleString('en-US', { maximumFractionDigits: 0 })}
                      </td>
                      <td className="px-4 py-3">
                        <StatusBadge status={w.is_active ? 'open' : 'closed'} />
                      </td>
                      <td className="px-4 py-3 text-center">
                        <button
                          onClick={() => navigate(`/whales/${w.address}`)}
                          className="inline-flex items-center gap-1 px-2.5 py-1.5 text-xs font-medium text-indigo-300 bg-indigo-500/10 hover:bg-indigo-500/20 border border-indigo-500/30 rounded-lg transition-colors"
                          title="查看交易历史"
                        >
                          <History size={13} />
                          交易记录
                        </button>
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function getVal(w: Whale, key: SortKey): number {
  switch (key) {
    case 'total_pnl': return Number(w.total_pnl ?? 0);
    case 'win_rate': return Number(w.win_rate ?? 0);
    case 'sharpe_ratio': return Number(w.sharpe_ratio ?? 0);
    case 'total_trades': return w.total_trades ?? 0;
  }
}
