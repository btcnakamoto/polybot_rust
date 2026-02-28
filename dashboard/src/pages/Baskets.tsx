import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  fetchBaskets,
  fetchBasketWhales,
  fetchConsensusHistory,
  createBasket,
} from '../services/api';
import StatusBadge from '../components/StatusBadge';
import type { WhaleBasket, Whale, ConsensusSignal } from '../types';

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

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold text-white">Whale Baskets</h2>
        <button
          onClick={() => setShowCreate(!showCreate)}
          className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-sm rounded-lg transition-colors"
        >
          {showCreate ? 'Cancel' : 'New Basket'}
        </button>
      </div>

      {/* Create form */}
      {showCreate && (
        <div className="bg-slate-800 rounded-xl border border-slate-700 p-4 space-y-3">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs text-slate-400 mb-1">Name</label>
              <input
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                className="w-full bg-slate-900 border border-slate-600 rounded px-3 py-2 text-sm text-white"
                placeholder="e.g. US Politics Whales"
              />
            </div>
            <div>
              <label className="block text-xs text-slate-400 mb-1">Category</label>
              <select
                value={form.category}
                onChange={(e) => setForm({ ...form, category: e.target.value })}
                className="w-full bg-slate-900 border border-slate-600 rounded px-3 py-2 text-sm text-white"
              >
                <option value="politics">Politics</option>
                <option value="crypto">Crypto</option>
                <option value="sports">Sports</option>
              </select>
            </div>
          </div>
          <button
            onClick={() => createMutation.mutate()}
            disabled={!form.name || createMutation.isPending}
            className="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white text-sm rounded-lg transition-colors"
          >
            {createMutation.isPending ? 'Creating...' : 'Create Basket'}
          </button>
        </div>
      )}

      {/* Basket list */}
      <div className="bg-slate-800 rounded-xl border border-slate-700">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-xs text-slate-400 uppercase border-b border-slate-700">
                <th className="text-left px-4 py-2">Name</th>
                <th className="text-left px-4 py-2">Category</th>
                <th className="text-right px-4 py-2">Wallets</th>
                <th className="text-right px-4 py-2">Threshold</th>
                <th className="text-right px-4 py-2">Window</th>
                <th className="text-left px-4 py-2">Status</th>
                <th className="text-left px-4 py-2"></th>
              </tr>
            </thead>
            <tbody>
              {(baskets ?? []).length === 0 ? (
                <tr>
                  <td colSpan={7} className="px-4 py-8 text-center text-slate-500">
                    No baskets created yet
                  </td>
                </tr>
              ) : (
                baskets!.map((b) => (
                  <BasketRow
                    key={b.id}
                    basket={b}
                    isExpanded={expandedId === b.id}
                    onToggle={() => setExpandedId(expandedId === b.id ? null : b.id)}
                  />
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function BasketRow({
  basket,
  isExpanded,
  onToggle,
}: {
  basket: WhaleBasket;
  isExpanded: boolean;
  onToggle: () => void;
}) {
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
    <>
      <tr className="border-b border-slate-700/50 hover:bg-slate-700/30 cursor-pointer" onClick={onToggle}>
        <td className="px-4 py-2 text-white font-medium">{basket.name}</td>
        <td className="px-4 py-2">
          <StatusBadge status={basket.category} />
        </td>
        <td className="px-4 py-2 text-right font-mono text-slate-300">
          {basket.min_wallets}-{basket.max_wallets}
        </td>
        <td className="px-4 py-2 text-right font-mono text-slate-300">
          {(Number(basket.consensus_threshold) * 100).toFixed(0)}%
        </td>
        <td className="px-4 py-2 text-right font-mono text-slate-300">
          {basket.time_window_hours}h
        </td>
        <td className="px-4 py-2">
          <StatusBadge status={basket.is_active ? 'open' : 'closed'} />
        </td>
        <td className="px-4 py-2 text-slate-400 text-xs">
          {isExpanded ? '▲' : '▼'}
        </td>
      </tr>

      {isExpanded && (
        <tr>
          <td colSpan={7} className="px-4 py-3 bg-slate-900/50">
            <div className="grid grid-cols-2 gap-4">
              {/* Members */}
              <div>
                <h4 className="text-xs font-medium text-slate-400 uppercase mb-2">
                  Members ({whales?.length ?? 0})
                </h4>
                {(whales ?? []).length === 0 ? (
                  <p className="text-xs text-slate-500">No whales in this basket</p>
                ) : (
                  <ul className="space-y-1">
                    {whales!.map((w: Whale) => (
                      <li key={w.id} className="flex items-center justify-between text-xs">
                        <span className="font-mono text-slate-300">
                          {w.address.slice(0, 10)}...{w.address.slice(-6)}
                        </span>
                        <span className="text-emerald-400">
                          WR {(Number(w.win_rate ?? 0) * 100).toFixed(1)}%
                        </span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>

              {/* Consensus history */}
              <div>
                <h4 className="text-xs font-medium text-slate-400 uppercase mb-2">
                  Recent Consensus ({signals?.length ?? 0})
                </h4>
                {(signals ?? []).length === 0 ? (
                  <p className="text-xs text-slate-500">No consensus signals yet</p>
                ) : (
                  <ul className="space-y-1">
                    {signals!.slice(0, 5).map((s: ConsensusSignal) => (
                      <li key={s.id} className="flex items-center justify-between text-xs">
                        <span className="font-mono text-slate-300">
                          {s.market_id.slice(0, 12)}...
                        </span>
                        <span>
                          <StatusBadge status={`consensus`} />{' '}
                          <span className="text-white">{s.direction}</span>{' '}
                          <span className="text-slate-400">
                            {(Number(s.consensus_pct) * 100).toFixed(0)}% ({s.participating_whales}/{s.total_whales})
                          </span>
                        </span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </div>
          </td>
        </tr>
      )}
    </>
  );
}
