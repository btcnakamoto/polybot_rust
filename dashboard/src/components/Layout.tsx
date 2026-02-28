import { NavLink, Outlet } from 'react-router-dom';
import { LayoutDashboard, Fish, ArrowLeftRight, Briefcase, Layers, BarChart3, Settings } from 'lucide-react';

const navItems = [
  { to: '/', label: '仪表盘', icon: LayoutDashboard },
  { to: '/whales', label: '巨鲸', icon: Fish },
  { to: '/trades', label: '交易', icon: ArrowLeftRight },
  { to: '/positions', label: '持仓', icon: Briefcase },
  { to: '/baskets', label: '篮子', icon: Layers },
  { to: '/analytics', label: '分析', icon: BarChart3 },
  { to: '/settings', label: '设置', icon: Settings },
];

export default function Layout() {
  return (
    <div className="flex h-screen bg-slate-900">
      {/* Sidebar */}
      <aside className="w-56 bg-slate-950 border-r border-slate-800 flex flex-col">
        <div className="p-4 border-b border-slate-800">
          <h1 className="text-lg font-bold text-white tracking-tight">Polybot</h1>
          <p className="text-xs text-slate-500">巨鲸跟单机器人</p>
        </div>
        <nav className="flex-1 p-2 space-y-1">
          {navItems.map(({ to, label, icon: Icon }) => (
            <NavLink
              key={to}
              to={to}
              className={({ isActive }) =>
                `flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors ${
                  isActive
                    ? 'bg-slate-800 text-white'
                    : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
                }`
              }
            >
              <Icon size={18} />
              {label}
            </NavLink>
          ))}
        </nav>
      </aside>

      {/* Main content */}
      <main className="flex-1 overflow-auto p-6">
        <Outlet />
      </main>
    </div>
  );
}
