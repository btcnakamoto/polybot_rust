import { useState } from 'react';
import { NavLink, Outlet } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  LayoutDashboard,
  Fish,
  ArrowLeftRight,
  Briefcase,
  Layers,
  BarChart3,
  Settings,
  Radio,
  Wallet,
  Circle,
  Pause,
  Play,
  Menu,
  X,
} from 'lucide-react';
import { fetchSystemStatus, controlStop, controlResume, clearToken } from '../services/api';

const navItems = [
  { to: '/', label: '仪表盘', icon: LayoutDashboard },
  { to: '/whales', label: '巨鲸', icon: Fish },
  { to: '/trades', label: '交易', icon: ArrowLeftRight },
  { to: '/positions', label: '持仓', icon: Briefcase },
  { to: '/baskets', label: '篮子', icon: Layers },
  { to: '/signals', label: '信号', icon: Radio },
  { to: '/analytics', label: '分析', icon: BarChart3 },
  { to: '/settings', label: '设置', icon: Settings },
];

// Bottom tab bar shows only the most used items
const mobileTabItems = navItems.slice(0, 5);

export default function Layout() {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const queryClient = useQueryClient();
  const { data: status } = useQuery({
    queryKey: ['system-status'],
    queryFn: fetchSystemStatus,
    refetchInterval: 10_000,
  });

  const pauseMutation = useMutation({
    mutationFn: controlStop,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['system-status'] }),
  });

  const resumeMutation = useMutation({
    mutationFn: controlResume,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['system-status'] }),
  });

  const isPaused = status?.paused ?? false;
  const mode = status?.mode ?? 'unknown';
  const modeColor = mode === 'live' ? 'text-emerald-400' : 'text-amber-400';
  const statusDotColor = isPaused ? 'text-red-400' : 'text-emerald-400';

  const handleLogout = () => {
    clearToken();
    window.location.reload();
  };

  return (
    <div className="flex h-screen bg-slate-900">
      {/* Desktop sidebar — hidden on mobile */}
      <aside className="hidden md:flex w-56 bg-slate-950 border-r border-slate-800 flex-col">
        <div className="p-4 border-b border-slate-800">
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-indigo-500 to-cyan-500 flex items-center justify-center">
              <Fish size={16} className="text-white" />
            </div>
            <div>
              <h1 className="text-base font-bold text-white tracking-tight">Polybot</h1>
              <p className="text-[10px] text-slate-500 leading-none">Whale Copy Trader</p>
            </div>
          </div>
        </div>
        <nav className="flex-1 p-2 space-y-0.5">
          {navItems.map(({ to, label, icon: Icon }) => (
            <NavLink
              key={to}
              to={to}
              end={to === '/'}
              className={({ isActive }) =>
                `flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-all ${
                  isActive
                    ? 'bg-indigo-500/10 text-indigo-400 border border-indigo-500/20'
                    : 'text-slate-400 hover:text-white hover:bg-slate-800/50 border border-transparent'
                }`
              }
            >
              <Icon size={16} />
              {label}
            </NavLink>
          ))}
        </nav>

        {/* System status in sidebar footer */}
        <div className="p-3 border-t border-slate-800 space-y-2">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-1.5">
              <Circle size={8} className={`${statusDotColor} fill-current`} />
              <span className="text-xs text-slate-400">
                {isPaused ? '已暂停' : '运行中'}
              </span>
            </div>
            <span className={`text-xs font-mono ${modeColor}`}>
              {mode === 'live' ? 'LIVE' : 'DRY'}
            </span>
          </div>

          {status?.wallet && (
            <div className="flex items-center gap-1.5 text-xs text-slate-500">
              <Wallet size={12} />
              <span className="font-mono truncate">
                {status.wallet.slice(0, 6)}...{status.wallet.slice(-4)}
              </span>
            </div>
          )}

          {status?.usdc_balance && (
            <div className="text-xs text-slate-400 font-mono">
              ${Number(status.usdc_balance).toFixed(2)} USDC
            </div>
          )}

          <button
            onClick={() => isPaused ? resumeMutation.mutate() : pauseMutation.mutate()}
            disabled={pauseMutation.isPending || resumeMutation.isPending}
            className={`w-full flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
              isPaused
                ? 'bg-emerald-500/10 text-emerald-400 hover:bg-emerald-500/20 border border-emerald-500/20'
                : 'bg-red-500/10 text-red-400 hover:bg-red-500/20 border border-red-500/20'
            }`}
          >
            {isPaused ? <Play size={12} /> : <Pause size={12} />}
            {isPaused ? '恢复运行' : '暂停引擎'}
          </button>

          <button
            onClick={handleLogout}
            className="w-full text-xs text-slate-500 hover:text-slate-300 py-1"
          >
            退出登录
          </button>
        </div>
      </aside>

      {/* Mobile top bar — visible only on mobile */}
      <div className="md:hidden fixed top-0 left-0 right-0 z-30 bg-slate-950 border-b border-slate-800 px-4 py-2.5 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-indigo-500 to-cyan-500 flex items-center justify-center">
            <Fish size={14} className="text-white" />
          </div>
          <span className="text-sm font-bold text-white">Polybot</span>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5">
            <Circle size={6} className={`${statusDotColor} fill-current`} />
            <span className={`text-[10px] font-mono ${modeColor}`}>
              {mode === 'live' ? 'LIVE' : 'DRY'}
            </span>
          </div>
          <button
            onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
            className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800"
          >
            {mobileMenuOpen ? <X size={20} /> : <Menu size={20} />}
          </button>
        </div>
      </div>

      {/* Mobile slide-down menu */}
      {mobileMenuOpen && (
        <div className="md:hidden fixed inset-0 z-20 bg-slate-950/95 pt-14">
          <nav className="p-4 space-y-1">
            {navItems.map(({ to, label, icon: Icon }) => (
              <NavLink
                key={to}
                to={to}
                end={to === '/'}
                onClick={() => setMobileMenuOpen(false)}
                className={({ isActive }) =>
                  `flex items-center gap-3 px-4 py-3 rounded-xl text-base transition-all ${
                    isActive
                      ? 'bg-indigo-500/10 text-indigo-400'
                      : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
                  }`
                }
              >
                <Icon size={20} />
                {label}
              </NavLink>
            ))}
          </nav>

          <div className="px-4 pt-4 border-t border-slate-800 mx-4 space-y-3">
            {status?.wallet && (
              <div className="flex items-center gap-2 text-sm text-slate-500">
                <Wallet size={14} />
                <span className="font-mono">
                  {status.wallet.slice(0, 6)}...{status.wallet.slice(-4)}
                </span>
              </div>
            )}
            {status?.usdc_balance && (
              <div className="text-sm text-slate-400 font-mono">
                ${Number(status.usdc_balance).toFixed(2)} USDC
              </div>
            )}
            <button
              onClick={() => {
                isPaused ? resumeMutation.mutate() : pauseMutation.mutate();
                setMobileMenuOpen(false);
              }}
              className={`w-full flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl text-sm font-medium ${
                isPaused
                  ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
                  : 'bg-red-500/10 text-red-400 border border-red-500/20'
              }`}
            >
              {isPaused ? <Play size={14} /> : <Pause size={14} />}
              {isPaused ? '恢复运行' : '暂停引擎'}
            </button>
            <button
              onClick={handleLogout}
              className="w-full text-sm text-slate-500 hover:text-slate-300 py-2"
            >
              退出登录
            </button>
          </div>
        </div>
      )}

      {/* Main content */}
      <main className="flex-1 overflow-auto pt-12 md:pt-0 pb-16 md:pb-0">
        <div className="p-4 md:p-6 max-w-[1400px] mx-auto">
          <Outlet />
        </div>
      </main>

      {/* Mobile bottom tab bar */}
      <nav className="md:hidden fixed bottom-0 left-0 right-0 z-30 bg-slate-950 border-t border-slate-800 px-1 pb-[env(safe-area-inset-bottom)]">
        <div className="flex items-center justify-around">
          {mobileTabItems.map(({ to, label, icon: Icon }) => (
            <NavLink
              key={to}
              to={to}
              end={to === '/'}
              className={({ isActive }) =>
                `flex flex-col items-center gap-0.5 px-2 py-2 min-w-0 ${
                  isActive ? 'text-indigo-400' : 'text-slate-500'
                }`
              }
            >
              <Icon size={18} />
              <span className="text-[10px] leading-tight truncate">{label}</span>
            </NavLink>
          ))}
          <button
            onClick={() => setMobileMenuOpen(true)}
            className="flex flex-col items-center gap-0.5 px-2 py-2 text-slate-500"
          >
            <Menu size={18} />
            <span className="text-[10px] leading-tight">更多</span>
          </button>
        </div>
      </nav>
    </div>
  );
}
