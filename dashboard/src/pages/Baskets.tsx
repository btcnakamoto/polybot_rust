import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import {
  fetchBaskets,
  fetchBasketWhales,
  fetchConsensusHistory,
  createBasket,
} from '../services/api';
import StatusBadge from '../components/StatusBadge';
import StatCard from '../components/StatCard';
import type { WhaleBasket, Whale, ConsensusSignal } from '../types';
import { Plus, ChevronDown, ChevronUp, Users, Radio } from 'lucide-react';

export default function Baskets() {
  const queryClient = useQueryClient();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [form, setForm] = useState({ name: '', category: 'crypto' });

  const { data: baskets } = useQuery({
    queryKey: ['baskets'],
    queryFn: fetchBaskets,
    refetchInterval: 15_000,
  });

  const createMutation = useMutation({
    mutationFn: () => createBasket(form),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['baskets'] });
      setShowCreate(false);
      setForm({ name: '', category: 'crypto' });
    },
  });

  const totalBaskets = (baskets ?? []).length;
  const activeBaskets = (baskets ?? []).filter((b) => b.is_active).length;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold text-white">巨鲸篮子</h2>
          <p className="text-xs text-slate-500 mt-0.5">按主题分组的巨鲸集合</p>
        </div>
        <button
          onClick={() => setShowCreate(!showCreate)}
          className="flex items-center gap-1.5 px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-sm rounded-lg transition-colors"
        >
          <Plus size={14} />
          {showCreate ? '取消' : '新建篮子'}
        </button>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <StatCard label="总篮子数" value={totalBaskets} accent="indigo" />
        <StatCard label="活跃篮子" value={activeBaskets} accent="emerald" />
      </div>

      {/* Create form */}
      {showCreate && (
        <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-indigo-500/30 p-4 space-y-3">
          <h3 className="text-sm font-medium text-white">创建新篮子</h3>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs text-slate-400 mb-1.5">名称</label>
              <input
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                className="w-full bg-slate-900/80 border border-slate-600/50 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:ring-1 focus:ring-indigo-500"
                placeholder="例如：美国政治巨鲸"
              />
            </div>
            <div>
              <label className="block text-xs text-slate-400 mb-1.5">分类</label>
              <select
                value={form.category}
                onChange={(e) => setForm({ ...form, category: e.target.value })}
                className="w-full bg-slate-900/80 border border-slate-600/50 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:ring-1 focus:ring-indigo-500"
              >
                <option value="politics">政治</option>
                <option value="crypto">加密货币</option>
                <option value="sports">体育</option>
              </select>
            </div>
          </div>
          <button
            onClick={() => createMutation.mutate()}
            disabled={!form.name || createMutation.isPending}
            className="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white text-sm rounded-lg transition-colors"
          >
            {createMutation.isPending ? '创建中...' : '创建'}
          </button>
        </div>
      )}

      {/* Basket cards */}
      <div className="space-y-3">
        {(baskets ?? []).length === 0 ? (
          <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-12 text-center text-slate-500">
            暂无篮子
          </div>
        ) : (
          baskets!.map((b) => (
            <BasketCard
              key={b.id}
              basket={b}
              isExpanded={expandedId === b.id}
              onToggle={() => setExpandedId(expandedId === b.id ? null : b.id)}
            />
          ))
        )}
      </div>
    </div>
  );
}

function BasketCard({
  basket,
  isExpanded,
  onToggle,
}: {
  basket: WhaleBasket;
  isExpanded: boolean;
  onToggle: () => void;
}) {
  const navigate = useNavigate();

  const { data: whales } = useQuery({
    queryKey: ['basket-whales', basket.id],
    queryFn: () => fetchBasketWhales(basket.id),
    enabled: isExpanded,
  });

  const { data: signals } = useQuery({
    queryKey: ['basket-consensus', basket.id],
    queryFn: () => fetchConsensusHistory(basket.id),
    enabled: isExpanded,
  });

  return (
    <div className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 overflow-hidden">
      {/* Header */}
      <div
        className="flex items-center gap-4 px-4 py-3 cursor-pointer hover:bg-slate-700/20 transition-colors"
        onClick={onToggle}
      >
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-white font-medium">{basket.name}</span>
            <StatusBadge status={basket.category} />
            <StatusBadge status={basket.is_active ? 'open' : 'closed'} />
          </div>
          <div className="flex items-center gap-4 mt-1 text-[10px] text-slate-500">
            <span>{basket.min_wallets}-{basket.max_wallets} 钱包</span>
            <span>共识 {(Number(basket.consensus_threshold) * 100).toFixed(0)}%</span>
            <span>窗口 {basket.time_window_hours}h</span>
          </div>
        </div>
        {isExpanded ? (
          <ChevronUp size={16} className="text-slate-400" />
        ) : (
          <ChevronDown size={16} className="text-slate-400" />
        )}
      </div>

      {/* Expanded content */}
      {isExpanded && (
        <div className="px-4 pb-4 border-t border-slate-700/50">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mt-3">
            {/* Members */}
            <div>
              <div className="flex items-center gap-1.5 mb-2">
                <Users size={12} className="text-slate-400" />
                <h4 className="text-xs font-medium text-slate-400 uppercase">
                  成员 ({whales?.length ?? 0})
                </h4>
              </div>
              {(whales ?? []).length === 0 ? (
                <p className="text-xs text-slate-500 py-3">篮子中暂无巨鲸</p>
              ) : (
                <div className="space-y-1">
                  {whales!.map((w: Whale) => (
                    <div
                      key={w.id}
                      onClick={() => navigate(`/whales/${w.address}`)}
                      className="flex items-center justify-between px-2 py-1.5 rounded-lg hover:bg-slate-700/30 cursor-pointer text-xs"
                    >
                      <span className="font-mono text-slate-300">
                        {w.address.slice(0, 8)}...{w.address.slice(-6)}
                      </span>
                      <span className="text-emerald-400 font-mono">
                        {(Number(w.win_rate ?? 0) * 100).toFixed(1)}%
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Consensus history */}
            <div>
              <div className="flex items-center gap-1.5 mb-2">
                <Radio size={12} className="text-slate-400" />
                <h4 className="text-xs font-medium text-slate-400 uppercase">
                  共识记录 ({signals?.length ?? 0})
                </h4>
              </div>
              {(signals ?? []).length === 0 ? (
                <p className="text-xs text-slate-500 py-3">暂无共识信号</p>
              ) : (
                <div className="space-y-1">
                  {signals!.slice(0, 5).map((s: ConsensusSignal) => (
                    <div key={s.id} className="flex items-center justify-between px-2 py-1.5 rounded-lg hover:bg-slate-700/30 text-xs">
                      <div className="flex items-center gap-2">
                        <StatusBadge status={s.direction} />
                        <span className="font-mono text-slate-400">
                          {s.market_id.slice(0, 12)}...
                        </span>
                      </div>
                      <span className="text-cyan-400 font-mono">
                        {(Number(s.consensus_pct) * 100).toFixed(0)}%
                        <span className="text-slate-500 ml-1">
                          ({s.participating_whales}/{s.total_whales})
                        </span>
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
