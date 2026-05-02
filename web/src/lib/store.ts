import { create } from "zustand";

interface UiState {
  sidebarOpen: boolean;
  graphOverlayOpen: boolean;
  selectedSessionId: string | null;
  helpDialogOpen: boolean;
  toggleSidebar: () => void;
  toggleGraphOverlay: () => void;
  setSelectedSession: (id: string | null) => void;
  toggleHelpDialog: () => void;
  setHelpDialogOpen: (open: boolean) => void;
}

export const useUi = create<UiState>((set) => ({
  sidebarOpen: true,
  graphOverlayOpen: false,
  selectedSessionId: null,
  helpDialogOpen: false,
  toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
  toggleGraphOverlay: () => set((s) => ({ graphOverlayOpen: !s.graphOverlayOpen })),
  setSelectedSession: (id) => set({ selectedSessionId: id }),
  toggleHelpDialog: () => set((s) => ({ helpDialogOpen: !s.helpDialogOpen })),
  setHelpDialogOpen: (open) => set({ helpDialogOpen: open }),
}));
