import { createRootRoute, Outlet } from "@tanstack/react-router";
import { GettingStarted } from "../components/GettingStarted";
import { Layout } from "../components/Layout";
import { useIsAuthenticated } from "../hooks/useIsAuthenticated";

export const Route = createRootRoute({
  component: RootComponent,
  beforeLoad: async ({ location }) => {
    // Skip auth check for the root path since it redirects to /chat
    if (location.pathname === "/") {
      return;
    }
  },
});

function RootComponent() {
  const { isAuthenticated, ready } = useIsAuthenticated();

  if (!ready) {
    return (
      <Layout>
        <></>
      </Layout>
    );
  }

  return <Layout>{isAuthenticated ? <Outlet /> : <GettingStarted />}</Layout>;
}
