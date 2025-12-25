import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { User } from '@/types';
import api from '@/lib/api';

interface AuthState {
  user: User | null;
  token: string | null;
  isLoading: boolean;
  error: string | null;
  login: (email: string, password: string) => Promise<boolean>;
  logout: () => void;
  checkAuth: () => Promise<boolean>;
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set, get) => ({
      user: null,
      token: null,
      isLoading: false,
      error: null,

      login: async (email: string, password: string) => {
        set({ isLoading: true, error: null });

        const response = await api.login(email, password);

        if (response.success && response.data) {
          set({
            user: response.data.user,
            token: response.data.token,
            isLoading: false,
          });
          return true;
        } else {
          set({
            error: response.error?.message || 'Login failed',
            isLoading: false,
          });
          return false;
        }
      },

      logout: () => {
        api.logout();
        set({ user: null, token: null, error: null });
      },

      checkAuth: async () => {
        const { token } = get();
        if (!token) return false;

        // Token exists, assume authenticated
        // In production, you'd validate the token with the server
        api.setToken(token);
        return true;
      },
    }),
    {
      name: 'guardrail-auth',
      partialize: (state) => ({
        user: state.user,
        token: state.token,
      }),
    }
  )
);

export default useAuthStore;
