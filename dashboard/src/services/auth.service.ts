import type { User } from '@/types'
import { api } from '@/lib/api-client'

interface LoginResponse {
  user: User
  expiresAt: string
}

export const authService = {
  login: async (username: string, password: string): Promise<User> => {
    if (!username.trim() || !password) {
      throw new Error('无效的账号或密码')
    }

    const response = await api.post<LoginResponse>('/admin/auth/login', {
      username: username.trim(),
      password,
    })
    return response.user
  },

  logout: (): Promise<{ ok: boolean }> => api.post('/admin/auth/logout'),

  getCurrentUser: (): Promise<User> => api.get('/admin/auth/me'),
}
