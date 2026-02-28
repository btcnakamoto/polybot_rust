import { useState, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { fetchConfig, updateConfig } from '../services/api';

interface FieldDef {
  key: string;
  label: string;
  type: 'text' | 'number' | 'boolean';
}

const groups: { title: string; fields: FieldDef[] }[] = [
  {
    title: '跟单策略',
    fields: [
      { key: 'copy_strategy', label: '策略', type: 'text' },
      { key: 'bankroll', label: '资金池', type: 'number' },
      { key: 'base_copy_amount', label: '基础金额', type: 'number' },
      { key: 'dry_run', label: '模拟运行', type: 'boolean' },
      { key: 'copy_enabled', label: '启用跟单', type: 'boolean' },
    ],
  },
  {
    title: '信号质量',
    fields: [
      { key: 'min_signal_win_rate', label: '最低胜率', type: 'number' },
      { key: 'min_total_trades_for_signal', label: '最低交易数', type: 'number' },
      { key: 'min_signal_ev', label: '最低EV', type: 'number' },
      { key: 'assumed_slippage_pct', label: '预估滑点', type: 'number' },
      { key: 'signal_notional_liquidity_pct', label: '流动性比例', type: 'number' },
      { key: 'signal_notional_floor', label: '名义下限', type: 'number' },
      { key: 'max_signal_notional', label: '名义上限', type: 'number' },
    ],
  },
  {
    title: '风控',
    fields: [
      { key: 'default_stop_loss_pct', label: '止损百分比', type: 'number' },
      { key: 'default_take_profit_pct', label: '止盈百分比', type: 'number' },
    ],
  },
  {
    title: '篮子',
    fields: [
      { key: 'basket_consensus_threshold', label: '共识阈值', type: 'number' },
      { key: 'basket_time_window_hours', label: '时间窗口(小时)', type: 'number' },
    ],
  },
  {
    title: '通知',
    fields: [
      { key: 'notifications_enabled', label: '启用通知', type: 'boolean' },
    ],
  },
];

export default function Settings() {
  const queryClient = useQueryClient();
  const [form, setForm] = useState<Record<string, string>>({});
  const [toast, setToast] = useState<string | null>(null);

  const { data: configEntries } = useQuery({
    queryKey: ['config'],
    queryFn: fetchConfig,
  });

  useEffect(() => {
    if (configEntries) {
      const map: Record<string, string> = {};
      for (const entry of configEntries) {
        map[entry.key] = entry.value;
      }
      setForm(map);
    }
  }, [configEntries]);

  const saveMutation = useMutation({
    mutationFn: () => updateConfig(form),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['config'] });
      setToast('配置保存成功');
      setTimeout(() => setToast(null), 3000);
    },
    onError: () => {
      setToast('保存失败');
      setTimeout(() => setToast(null), 3000);
    },
  });

  const handleChange = (key: string, value: string) => {
    setForm((prev) => ({ ...prev, [key]: value }));
  };

  const handleToggle = (key: string) => {
    setForm((prev) => ({
      ...prev,
      [key]: prev[key] === 'true' ? 'false' : 'true',
    }));
  };

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-semibold text-white">运行时设置</h2>

      {toast && (
        <div className="bg-emerald-900/50 border border-emerald-700 text-emerald-300 px-4 py-2 rounded-lg text-sm">
          {toast}
        </div>
      )}

      {groups.map((group) => (
        <div key={group.title} className="bg-slate-800 rounded-xl border border-slate-700 p-4 space-y-3">
          <h3 className="text-sm font-medium text-white border-b border-slate-700 pb-2">
            {group.title}
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {group.fields.map((field) => (
              <div key={field.key}>
                <label className="block text-xs text-slate-400 mb-1">{field.label}</label>
                {field.type === 'boolean' ? (
                  <button
                    onClick={() => handleToggle(field.key)}
                    className={`px-3 py-2 rounded text-sm font-medium transition-colors ${
                      form[field.key] === 'true'
                        ? 'bg-emerald-600 text-white'
                        : 'bg-slate-700 text-slate-400'
                    }`}
                  >
                    {form[field.key] === 'true' ? '启用' : '禁用'}
                  </button>
                ) : (
                  <input
                    value={form[field.key] ?? ''}
                    onChange={(e) => handleChange(field.key, e.target.value)}
                    className="w-full bg-slate-900 border border-slate-600 rounded px-3 py-2 text-sm text-white"
                  />
                )}
              </div>
            ))}
          </div>
        </div>
      ))}

      <button
        onClick={() => saveMutation.mutate()}
        disabled={saveMutation.isPending}
        className="px-6 py-2 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white text-sm rounded-lg transition-colors"
      >
        {saveMutation.isPending ? '保存中...' : '保存配置'}
      </button>
    </div>
  );
}
