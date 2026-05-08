import { create } from "zustand";

/** `/sessions` 와 `/wiki` 가 공유하는 검색 모드. wiki 만 'hybrid' 추가 사용. */
export type GlobalSearchMode = "keyword" | "semantic" | "hybrid";

interface UiState {
  sidebarOpen: boolean;
  graphOverlayOpen: boolean;
  selectedSessionId: string | null;
  helpDialogOpen: boolean;
  /** TopNav HeaderSearch 가 lift 한 검색어. /sessions 와 /wiki 가 구독. */
  query: string;
  /** TopNav HeaderSearch 가 lift 한 모드. */
  searchMode: GlobalSearchMode;
  toggleSidebar: () => void;
  toggleGraphOverlay: () => void;
  setSelectedSession: (id: string | null) => void;
  toggleHelpDialog: () => void;
  setHelpDialogOpen: (open: boolean) => void;
  setQuery: (q: string) => void;
  setSearchMode: (m: GlobalSearchMode) => void;
}

export const useUi = create<UiState>((set) => ({
  sidebarOpen: true,
  graphOverlayOpen: false,
  selectedSessionId: null,
  helpDialogOpen: false,
  query: "",
  searchMode: "keyword",
  toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
  toggleGraphOverlay: () => set((s) => ({ graphOverlayOpen: !s.graphOverlayOpen })),
  setSelectedSession: (id) => set({ selectedSessionId: id }),
  toggleHelpDialog: () => set((s) => ({ helpDialogOpen: !s.helpDialogOpen })),
  setHelpDialogOpen: (open) => set({ helpDialogOpen: open }),
  setQuery: (q) => set({ query: q }),
  setSearchMode: (m) => set({ searchMode: m }),
}));
