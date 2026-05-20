import { lazy, Suspense } from 'react';
import { Navigate, Route, Routes, useLocation } from 'react-router-dom';

import { AppLoader } from './components/AppLoader';
import { AppProvider, useAppContext } from './context/AppContext';

const AppLayout = lazy(async () => {
  const module = await import('./layouts/AppLayout');
  return { default: module.AppLayout };
});
const OverviewPage = lazy(async () => {
  const module = await import('./pages/OverviewPage');
  return { default: module.OverviewPage };
});
const ProjectsPage = lazy(async () => {
  const module = await import('./pages/ProjectsPage');
  return { default: module.ProjectsPage };
});
const ProfilePage = lazy(async () => {
  const module = await import('./pages/ProfilePage');
  return { default: module.ProfilePage };
});
const UsersPage = lazy(async () => {
  const module = await import('./pages/UsersPage');
  return { default: module.UsersPage };
});
const HealthPage = lazy(async () => {
  const module = await import('./pages/HealthPage');
  return { default: module.HealthPage };
});
const IssuesPage = lazy(async () => {
  const module = await import('./pages/IssuesPage');
  return { default: module.IssuesPage };
});
const IssueDetailPage = lazy(async () => {
  const module = await import('./pages/IssueDetailPage');
  return { default: module.IssueDetailPage };
});
const LoginPage = lazy(async () => {
  const module = await import('./pages/LoginPage');
  return { default: module.LoginPage };
});

function ProtectedApp() {
  const { currentUser, authLoading } = useAppContext();
  const location = useLocation();

  if (authLoading) {
    return <AppLoader />;
  }

  if (!currentUser) {
    return <Navigate to="/login" replace state={{ from: `${location.pathname}${location.search}${location.hash}` }} />;
  }

  return (
    <Suspense fallback={<AppLoader />}>
      <Routes>
        <Route element={<AppLayout />}>
          <Route path="/" element={<Navigate to="/overview" replace />} />
          <Route path="/overview" element={<OverviewPage />} />
          <Route path="/projects" element={<ProjectsPage />} />
          <Route path="/projects/new" element={<ProjectsPage />} />
          <Route path="/projects/:projectId" element={<ProjectsPage />} />
          <Route path="/projects/:projectId/edit" element={<Navigate to=".." replace />} />
          <Route path="/profile" element={<ProfilePage />} />
          <Route path="/users" element={<UsersPage />} />
          <Route path="/admin/health" element={<HealthPage />} />
          <Route path="/issues" element={<IssuesPage />} />
          <Route path="/issues/:issueId" element={<IssueDetailPage />} />
        </Route>
        <Route path="*" element={<Navigate to="/overview" replace />} />
      </Routes>
    </Suspense>
  );
}

function PublicApp() {
  const { authLoading } = useAppContext();

  if (authLoading) {
    return <AppLoader />;
  }

  return (
    <Suspense fallback={<AppLoader />}>
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="*" element={<ProtectedApp />} />
      </Routes>
    </Suspense>
  );
}

export default function App() {
  return (
    <AppProvider>
      <PublicApp />
    </AppProvider>
  );
}
