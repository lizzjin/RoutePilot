import { create } from 'zustand'
import type { User } from '@/types'
import { authService } from '@/services/auth.service'

interface AuthState {
  currentUser: User | null
  isAuthenticated: boolean
  isInitializing: boolean
  login: (username: string, password: string) => Promise<void>
  logout: () => Promise<void>
  initialize: () => Promise<void>
}

export const useAuthStore = create<AuthState>((set) => ({
  currentUser: null,
  isAuthenticated: false,
  isInitializing: true,

  login: async (username: string, password: string) => {
    const adminUser = await authService.login(username, password)
    set({ currentUser: adminUser, isAuthenticated: true, isInitializing: false })
  },

  logout: async () => {
    await authService.logout().catch(() => undefined)
    set({ currentUser: null, isAuthenticated: false, isInitializing: false })
  },

  initialize: async () => {
    try {
      const currentUser = await authService.getCurrentUser()
      set({ currentUser, isAuthenticated: true, isInitializing: false })
    } catch {
      set({ currentUser: null, isAuthenticated: false, isInitializing: false })
    }
  },
}))
