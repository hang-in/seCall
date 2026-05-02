import { createBrowserRouter, Navigate } from "react-router";
import Layout from "./Layout";
import SessionsRoute, { SessionEmptyState } from "./SessionsRoute";
import SessionDetailRoute from "./SessionDetailRoute";
import DailyRoute from "./DailyRoute";
import WikiRoute from "./WikiRoute";
import CommandsRoute from "./CommandsRoute";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <Layout />,
    children: [
      { index: true, element: <Navigate to="/sessions" replace /> },
      {
        path: "sessions",
        element: <SessionsRoute />,
        children: [
          { index: true, element: <SessionEmptyState /> },
          { path: ":id", element: <SessionDetailRoute /> },
        ],
      },
      { path: "daily", element: <DailyRoute /> },
      { path: "daily/:date", element: <DailyRoute /> },
      { path: "wiki", element: <WikiRoute /> },
      { path: "wiki/:project", element: <WikiRoute /> },
      { path: "commands", element: <CommandsRoute /> },
    ],
  },
]);
