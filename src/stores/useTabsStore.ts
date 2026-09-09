import { create } from "zustand";

export type TabKind = "overview" | "table" | "sql" | "erd";

export interface Tab {
  id: string;
  kind: TabKind;
  label: string;
  connectionId: string;
  /** schema.table for "table" tabs, schema name for "erd" tabs; unused otherwise */
  target?: string;
}

interface TabsState {
  tabs: Tab[];
  activeTabId: string | null;
  openTab: (tab: Tab) => void;
  closeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
}

export const useTabsStore = create<TabsState>((set, get) => ({
  tabs: [],
  activeTabId: null,

  openTab: (tab) => {
    const existing = get().tabs.find((t) => t.id === tab.id);
    if (existing) {
      set({ activeTabId: existing.id });
      return;
    }
    set((state) => ({ tabs: [...state.tabs, tab], activeTabId: tab.id }));
  },

  closeTab: (id) => {
    set((state) => {
      const tabs = state.tabs.filter((t) => t.id !== id);
      const activeTabId =
        state.activeTabId === id ? (tabs.at(-1)?.id ?? null) : state.activeTabId;
      return { tabs, activeTabId };
    });
  },

  setActiveTab: (id) => set({ activeTabId: id }),
}));
