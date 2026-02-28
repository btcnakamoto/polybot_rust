import { useQuery } from '@tanstack/react-query';
import { fetchRecentConsensus, fetchBaskets } from '../services/api';
import StatusBadge from '../components/StatusBadge';
import StatCard from '../components/StatCard';

export default function Signals() {
  const { data: signals, isLoading } = useQuery({
    queryKey: ['recent-consensus'],
    queryFn: fetchRecentConsensus,
    refetchInterval: 10_000,
  });

  const { data: baskets } = useQuery({
    queryKey: ['baskets'],
    queryFn: fetchBaskets,
  });

  // Stats
  const total = (signals ?? []).length;
  const buySignals = (signals ?? []).filter((s) => s.direction === 'BUY').length;
  const sellSignals = (signals ?? []).filter((s) => s.direction === 'SELL').length;
  const avgConsensus =
    total > 0
      ? (signals ?? []).reduce((sum, s) => sum + Number(s.consensus_pct), 0) / total
      : 0;

  // Basket name lookup
  const basketMap = new Map((baskets ?? []).map((b) => [b.id, b.name]));

  // Group by day
  const grouped = new Map<string, typeof signals>();
  (signals ?? []).forEach((s) => {
    const day = new Date(s.triggered_at).toLocaleDateString('zh-CN');
    const list = grouped.get(day) ?? [];
    list.push(s);
    grouped.set(day, list);
  });

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-white">共识信号</h2>
        <p className="text-xs text-slate-500 mt-0.5">篮子共识信号历史记录</p>
      </div>

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        <StatCard label="总信号数" value={total} accent="cyan" />
        <StatCard label="买入信号" value={buySignals} accent="emerald" />
        <StatCard label="卖出信号" value={sellSignals} accent="red" />
        <StatCard
          label="平均共识"
          value={`${(avgConsensus * 100).toFixed(1)}%`}
          accent="indigo"
        />
      </div>

      {/* Signal timeline */}
      <div className="space-y-4">
        {isLoading ? (
          <div className="flex items-center justify-center py-16">
            <div className="w-5 h-5 border-2 border-slate-600 border-t-indigo-400 rounded-full animate-spin" />
          </div>
        ) : total === 0 ? (
          <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-12 text-center text-slate-500">
            暂无共识信号
          </div>
        ) : (
          [...grouped.entries()].map(([day, daySignals]) => (
            <div key={day}>
              <div className="flex items-center gap-3 mb-2">
                <span className="text-xs font-medium text-slate-400">{day}</span>
                <div className="flex-1 h-px bg-slate-700/50" />
                <span className="text-xs text-slate-500">{daySignals!.length} 个信号</span>
              </div>
              <div className="space-y-1">
                {daySignals!.map((s) => (
                  <div
                    key={s.id}
                    className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 px-4 py-3 flex items-center gap-4 hover:border-slate-600 transition-colors"
                  >
                    <div className="flex-shrink-0">
                      <StatusBadge status={s.direction} />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="font-mono text-xs text-slate-300 truncate">
                        {s.market_id}
                      </div>
                      <div className="text-[10px] text-slate-500 mt-0.5">
                        篮子: {basketMap.get(s.basket_id) ?? s.basket_id.slice(0, 8)}
                      </div>
                    </div>
                    <div className="text-right flex-shrink-0">
                      <div className="text-sm font-mono text-cyan-400">
                        {(Number(s.consensus_pct) * 100).toFixed(0)}%
                      </div>
                      <div className="text-[10px] text-slate-500">
                        {s.participating_whales}/{s.total_whales} 巨鲸
                      </div>
                    </div>
                    <div className="text-xs text-slate-500 flex-shrink-0 w-16 text-right">
                      {new Date(s.triggered_at).toLocaleTimeString('zh-CN', {
                        hour: '2-digit',
                        minute: '2-digit',
                      })}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
