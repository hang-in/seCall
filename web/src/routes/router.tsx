import { lazy, Suspense } from "react";
import { createBrowserRouter, Navigate } from "react-router";
import Layout from "./Layout";
import { RouteFallback } from "@/components/RouteFallback";
import { SessionEmptyState } from "@/components/SessionEmptyState";

// 라우트 단위 lazy chunks
const SessionsRoute = lazy(() => import("./SessionsRoute"));
const SessionDetailRoute = lazy(() => import("./SessionDetailRoute"));
const DailyRoute = lazy(() => import("./DailyRoute"));
const WikiRoute = lazy(() => import("./WikiRoute"));
const CommandsRoute = lazy(() => import("./CommandsRoute"));

const lazyEl = (Comp: React.LazyExoticComponent<React.ComponentType>) => (
  <Suspense fallback={<RouteFallback />}>
    <Comp />
  </Suspense>
);

export const router = createBrowserRouter([
  {
    path: "/",
    element: <Layout />,
    children: [
      { index: true, element: <Navigate to="/sessions" replace /> },
      {
        path: "sessions",
        element: lazyEl(SessionsRoute),
        children: [
          { index: true, element: <SessionEmptyState /> },
          { path: ":id", element: lazyEl(SessionDetailRoute) },
        ],
      },
      { path: "daily", element: lazyEl(DailyRoute) },
      { path: "daily/:date", element: lazyEl(DailyRoute) },
      { path: "wiki", element: lazyEl(WikiRoute) },
      { path: "wiki/:project", element: lazyEl(WikiRoute) },
      { path: "commands", element: lazyEl(CommandsRoute) },
    ],
  },
]);
