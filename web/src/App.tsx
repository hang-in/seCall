import { Outlet } from "react-router";

// 라우팅은 router.tsx에서 처리. App은 보존용 placeholder (Layout이 Outlet 역할).
export default function App() {
  return <Outlet />;
}
