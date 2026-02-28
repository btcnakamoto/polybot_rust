import { useState, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { fetchConfig, updateConfig } from '../services/api';
import { Save, Check, X } from 'lucide-react';

interface FieldDef {
  key: string;
  label: string;
  type: 'text' | 'number' | 'boolean';
  description?: string;
}

const groups: { title: string; description: string; fields: FieldDef[] }[] = [
  {
    title: '跟单策略',
    description: '控制复制引擎的核心参数',
    fields: [
      { key: 'copy_strategy', label: '策略', type: 'text', description: 'fixed / kelly / proportional' },
      { key: 'bankroll', label: '资金池', type: 'number', description: '总资金 (USDC)' },
      { key: 'base_copy_amount', label: '基础金额', type: 'number', description: '固定策略下每笔金额' },
      { key: 'dry_run', label: '模拟运行', type: 'boolean', description: '开启后不执行真实交易' },
      { key: 'copy_enabled', label: '启用跟单', type: 'boolean', description: '总开关' },
    ],
  },
  {
    title: '信号质量',
    description: '控制哪些信号可以触发跟单',
    fields: [
      { key: 'min_signal_win_rate', label: '最低胜率', type: 'number', description: '例: 0.55 = 55%' },
      { key: 'min_total_trades_for_signal', label: '最低交易数', type: 'number', description: '巨鲸历史交易数门槛' },
      { key: 'min_signal_ev', label: '最低EV', type: 'number', description: '最低期望值 (USDC)' },
      { key: 'assumed_slippage_pct', label: '预估滑点', type: 'number', description: '例: 0.02 = 2%' },
      { key: 'signal_notional_liquidity_pct', label: '流动性比例', type: 'number', description: '市场流动性阈值百分比' },
      { key: 'signal_notional_floor', label: '名义下限', type: 'number', description: '最低名义价值 (USDC)' },
      { key: 'max_signal_notional', label: '名义上限', type: 'number', description: '最大名义价值 (USDC)' },
    ],
  },
  {
    title: '风控',
    description: '止损止盈参数',
    fields: [
      { key: 'default_stop_loss_pct', label: '止损', type: 'number', description: '例: 0.15 = 15%' },
      { key: 'default_take_profit_pct', label: '止盈', type: 'number', description: '例: 0.30 = 30%' },
    ],
  },
  {
    title: '篮子',
    description: '篮子共识参数',
    fields: [
      { key: 'basket_consensus_threshold', label: '共识阈值', type: 'number', description: '例: 0.80 = 80%' },
      { key: 'basket_time_window_hours', label: '时间窗口', type: 'number', description: '小时' },
    ],
  },
  {
    title: '通知',
    description: 'Telegram 通知配置',
    fields: [
      { key: 'notifications_enabled', label: '启用通知', type: 'boolean', description: '开启 Telegram 通知' },
    ],
  },
];

export default function Settings() {
  const queryClient = useQueryClient();
  const [form, setForm] = useState<Record<string, string>>({});
  const [toast, setToast] = useState<{ type: 'success' | 'error'; msg: string } | null>(null);

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
      setToast({ type: 'success', msg: '配置已保存' });
      setTimeout(() => setToast(null), 3000);
    },
    onError: () => {
      setToast({ type: 'error', msg: '保存失败' });
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
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold text-white">运行时设置</h2>
          <p className="text-xs text-slate-500 mt-0.5">修改后需要保存才能生效</p>
        </div>
        <button
          onClick={() => saveMutation.mutate()}
          disabled={saveMutation.isPending}
          className="flex items-center gap-2 px-4 py-2 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white text-sm rounded-lg transition-colors"
        >
          <Save size={14} />
          {saveMutation.isPending ? '保存中...' : '保存配置'}
        </button>
      </div>

      {toast && (
        <div className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm ${
          toast.type === 'success'
            ? 'bg-emerald-900/50 border border-emerald-700 text-emerald-300'
            : 'bg-red-900/50 border border-red-700 text-red-300'
        }`}>
          {toast.type === 'success' ? <Check size={14} /> : <X size={14} />}
          {toast.msg}
        </div>
      )}

      {groups.map((group) => (
        <div key={group.title} className="bg-slate-800/80 backdrop-blur rounded-xl border border-slate-700/50 p-5 space-y-4">
          <div className="border-b border-slate-700/50 pb-3">
            <h3 className="text-sm font-medium text-white">{group.title}</h3>
            <p className="text-[10px] text-slate-500 mt-0.5">{group.description}</p>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-4">
            {group.fields.map((field) => (
              <div key={field.key}>
                <label className="flex items-center justify-between mb-1.5">
                  <span className="text-xs font-medium text-slate-300">{field.label}</span>
                  {field.description && (
                    <span className="text-[10px] text-slate-500">{field.description}</span>
                  )}
                </label>
                {field.type === 'boolean' ? (
                  <button
                    onClick={() => handleToggle(field.key)}
                    className={`relative w-12 h-6 rounded-full transition-colors ${
                      form[field.key] === 'true' ? 'bg-emerald-600' : 'bg-slate-700'
                    }`}
                  >
                    <span
                      className={`absolute top-1 w-4 h-4 bg-white rounded-full transition-transform ${
                        form[field.key] === 'true' ? 'left-7' : 'left-1'
                      }`}
                    />
                  </button>
                ) : (
                  <input
                    value={form[field.key] ?? ''}
                    onChange={(e) => handleChange(field.key, e.target.value)}
                    className="w-full bg-slate-900/80 border border-slate-600/50 rounded-lg px-3 py-2 text-sm text-white font-mono focus:outline-none focus:ring-1 focus:ring-indigo-500 focus:border-indigo-500"
                  />
                )}
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
