import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from 'react';
import { notifications } from '@mantine/notifications';

import { ApiError, apiFetch, requestJson, requestVoid } from '../lib/api';
import { detectBrowserLocale, translate, type Locale } from '../lib/i18n';
import type {
  AcceptInvitationValues,
  AdminHealth,
  ChangePasswordValues,
  Comment,
  CreateInvitationValues,
  CreateCommentValues,
  CreateIssueValues,
  GitLabCommentImportResult,
  GitLabIssueImportResult,
  GitLabIntegrationValidationResult,
  InvitationPreview,
  IntegrationFormValues,
  IssueAccessOverview,
  Issue,
  IssueDetailData,
  IssueUpload,
  ManagedUser,
  Overview,
  PasswordRecoveryPreview,
  PasswordRecoveryRequestValues,
  PasswordResetValues,
  Project,
  ProjectAccessOverview,
  ProjectFormValues,
  UpdateProfileValues,
  UpdateProjectAccessValues,
  UpdateUserAccessValues,
  UpdateUserValues,
  UpdateIssueAccessValues,
  UpdateIssueValues,
  User,
  UserAccessOverview,
  UserInvitation,
  UserManagementOverview,
} from '../types';

type BannerState =
  | { type: 'error'; message: string }
  | { type: 'success'; message: string }
  | null;

type AppContextValue = {
  currentUser: User | null;
  authLoading: boolean;
  loginLoading: boolean;
  dataLoading: boolean;
  overview: Overview | null;
  projects: Project[];
  issues: Issue[];
  banner: BannerState;
  locale: Locale;
  t: (key: string) => string;
  login: (values: { email: string; password: string }) => Promise<void>;
  logout: () => Promise<void>;
  updateProfile: (values: UpdateProfileValues) => Promise<User>;
  changePassword: (values: ChangePasswordValues) => Promise<void>;
  requestPasswordRecovery: (values: PasswordRecoveryRequestValues) => Promise<void>;
  getPasswordRecoveryPreview: (token: string) => Promise<PasswordRecoveryPreview>;
  resetPassword: (token: string, values: PasswordResetValues) => Promise<void>;
  refreshAll: () => Promise<void>;
  createProject: (values: ProjectFormValues) => Promise<Project>;
  updateProject: (projectId: string, values: ProjectFormValues) => Promise<Project>;
  deleteProject: (projectId: string) => Promise<void>;
  saveIntegration: (projectId: string, values: IntegrationFormValues) => Promise<Project>;
  deleteIntegration: (projectId: string) => Promise<void>;
  getProjectAccess: (projectId: string, signal?: AbortSignal) => Promise<ProjectAccessOverview>;
  updateProjectAccess: (projectId: string, values: UpdateProjectAccessValues) => Promise<ProjectAccessOverview>;
  validateIntegration: (
    projectId: string,
    values: IntegrationFormValues,
  ) => Promise<GitLabIntegrationValidationResult>;
  importProjectIssues: (projectId: string) => Promise<GitLabIssueImportResult>;
  syncIssueComments: (issueId: string) => Promise<GitLabCommentImportResult>;
  createIssue: (values: CreateIssueValues) => Promise<Issue>;
  updateIssue: (issueId: string, values: UpdateIssueValues) => Promise<Issue>;
  uploadProjectIssueAttachment: (projectId: string, file: File) => Promise<IssueUpload>;
  createIssueComment: (issueId: string, values: CreateCommentValues) => Promise<Comment>;
  uploadIssueAttachment: (issueId: string, file: File) => Promise<IssueUpload>;
  deleteIssueUpload: (uploadId: string) => Promise<void>;
  getIssueDetail: (issueId: string, signal?: AbortSignal) => Promise<IssueDetailData>;
  getIssueAccess: (issueId: string, signal?: AbortSignal) => Promise<IssueAccessOverview>;
  updateIssueAccess: (issueId: string, values: UpdateIssueAccessValues) => Promise<IssueAccessOverview>;
  getUserManagementOverview: () => Promise<UserManagementOverview>;
  updateUser: (userId: string, values: UpdateUserValues) => Promise<ManagedUser>;
  getUserAccess: (userId: string, signal?: AbortSignal) => Promise<UserAccessOverview>;
  updateUserAccess: (userId: string, values: UpdateUserAccessValues) => Promise<UserAccessOverview>;
  removeIssuePermission: (issueId: string, subjectType: string, subjectId: string) => Promise<void>;
  inviteUser: (values: CreateInvitationValues) => Promise<UserInvitation>;
  resendInvitation: (invitationId: string) => Promise<UserInvitation>;
  deleteInvitation: (invitationId: string) => Promise<void>;
  getInvitationPreview: (inviteToken: string) => Promise<InvitationPreview>;
  acceptInvitation: (inviteToken: string, values: AcceptInvitationValues) => Promise<void>;
  getAdminHealth: (signal?: AbortSignal) => Promise<AdminHealth>;
  clearBanner: () => void;
  setSuccessBanner: (message: string) => void;
  setErrorBanner: (message: string) => void;
};

const AppContext = createContext<AppContextValue | null>(null);

export function AppProvider({ children }: { children: ReactNode }) {
  const [currentUser, setCurrentUser] = useState<User | null>(null);
  const [authLoading, setAuthLoading] = useState(true);
  const [loginLoading, setLoginLoading] = useState(false);
  const [dataLoading, setDataLoading] = useState(false);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [issues, setIssues] = useState<Issue[]>([]);
  const [banner, setBannerState] = useState<BannerState>(null);
  const [locale, setLocale] = useState<Locale>(detectBrowserLocale());

  function setBanner(nextBanner: BannerState) {
    setBannerState(nextBanner);

    if (!nextBanner) {
      return;
    }

    notifications.show({
      title: nextBanner.type === 'error' ? translate(locale, 'common.error') : translate(locale, 'common.saved'),
      message: nextBanner.message,
      color: nextBanner.type === 'error' ? 'red' : 'teal',
      autoClose: nextBanner.type === 'error' ? 8000 : 3500,
      withBorder: true,
      withCloseButton: true,
    });
  }

  useEffect(() => {
    const controller = new AbortController();

    async function bootstrap() {
      try {
        setAuthLoading(true);
        setBanner(null);
        const authResponse = await apiFetch('/api/v1/auth/me', { signal: controller.signal });

        if (authResponse.status === 401) {
          setCurrentUser(null);
          resetData();
          return;
        }

        if (!authResponse.ok) {
          throw new Error('Nepodarilo se nacist aktualniho uzivatele.');
        }

        const authPayload = (await authResponse.json()) as { user: User };
        setCurrentUser(authPayload.user);
        setLocale((authPayload.user.preferred_language as Locale | null) ?? detectBrowserLocale());
        await loadAppData(controller.signal);
      } catch (error) {
        if (!(error instanceof DOMException && error.name === 'AbortError')) {
          setBanner({
            type: 'error',
            message: error instanceof Error ? error.message : 'Neznama chyba aplikace.',
          });
        }
      } finally {
        setAuthLoading(false);
      }
    }

    void bootstrap();
    return () => controller.abort();
  }, []);

  async function loadAppData(signal?: AbortSignal) {
    setDataLoading(true);
    try {
      const [overviewData, projectsData, issuesData] = await Promise.all([
        requestJson<Overview>('/api/v1/overview', { signal }),
        requestJson<Project[]>('/api/v1/projects', { signal }),
        requestJson<Issue[]>('/api/v1/issues', { signal }),
      ]);

      setOverview(overviewData);
      setProjects(projectsData);
      setIssues(issuesData);
    } finally {
      setDataLoading(false);
    }
  }

  function resetData() {
    setOverview(null);
    setProjects([]);
    setIssues([]);
  }

  async function refreshAll() {
    try {
      await loadAppData();
    } catch (error) {
      setBanner({
        type: 'error',
        message: error instanceof Error ? error.message : 'Nepodarilo se obnovit data.',
      });
      throw error;
    }
  }

  async function login(values: { email: string; password: string }) {
    setLoginLoading(true);
    setBanner(null);
    try {
      const payload = await requestJson<{ user: User }>('/api/v1/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(values),
      });

      setCurrentUser(payload.user);
      setLocale((payload.user.preferred_language as Locale | null) ?? detectBrowserLocale());
      await loadAppData();
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Prihlaseni se nezdarilo.';
      setBanner({ type: 'error', message });
      throw error;
    } finally {
      setLoginLoading(false);
      setAuthLoading(false);
    }
  }

  async function logout() {
    await apiFetch('/api/v1/auth/logout', { method: 'POST' });
    setCurrentUser(null);
    setLocale(detectBrowserLocale());
    resetData();
    setBanner(null);
  }

  async function updateProfile(values: UpdateProfileValues) {
    const payload = await requestJson<{ user: User }>('/api/v1/auth/me', {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(values),
    });

    setCurrentUser(payload.user);
    setLocale((payload.user.preferred_language as Locale | null) ?? detectBrowserLocale());
    setBanner({
      type: 'success',
      message: values.preferred_language === 'en' ? 'Profile saved.' : 'Profil byl uložen.',
    });
    return payload.user;
  }

  async function changePassword(values: ChangePasswordValues) {
    await requestVoid('/api/v1/auth/change-password', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(values),
    });

    setBanner({
      type: 'success',
      message: locale === 'en' ? 'Password changed.' : 'Heslo bylo změněno.',
    });
  }

  async function requestPasswordRecovery(values: PasswordRecoveryRequestValues) {
    await requestVoid('/api/v1/auth/password-recovery', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(values),
    });

    setBanner({
      type: 'success',
      message:
        locale === 'en'
          ? 'If the account exists, a recovery email has been sent.'
          : 'Pokud účet existuje, byl odeslán recovery email.',
    });
  }

  async function getPasswordRecoveryPreview(token: string) {
    return requestJson<PasswordRecoveryPreview>(`/api/v1/auth/password-recovery/${token}`);
  }

  async function resetPassword(token: string, values: PasswordResetValues) {
    await requestVoid(`/api/v1/auth/password-recovery/${token}/reset`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(values),
    });

    setBanner({
      type: 'success',
      message:
        locale === 'en'
          ? 'Password reset completed. You can sign in with the new password.'
          : 'Reset hesla byl dokončen. Můžeš se přihlásit novým heslem.',
    });
  }

  async function createProject(values: ProjectFormValues) {
    const project = await requestJson<Project>('/api/v1/projects', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        slug: values.slug,
        name: values.name,
        description: values.description || null,
      }),
    });

    await refreshAll();
    setBanner({ type: 'success', message: `Projekt ${project.name} byl vytvoren.` });
    return project;
  }

  async function updateProject(projectId: string, values: ProjectFormValues) {
    const project = await requestJson<Project>(`/api/v1/projects/${projectId}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        slug: values.slug,
        name: values.name,
        description: values.description || null,
        active: values.active,
      }),
    });

    await refreshAll();
    setBanner({ type: 'success', message: `Projekt ${project.name} byl upraven.` });
    return project;
  }

  async function deleteProject(projectId: string) {
    await requestVoid(`/api/v1/projects/${projectId}`, { method: 'DELETE' });
    await refreshAll();
    setBanner({ type: 'success', message: 'Projekt byl smazan.' });
  }

  async function saveIntegration(projectId: string, values: IntegrationFormValues) {
    const project = await requestJson<Project>(`/api/v1/projects/${projectId}/gitlab-integration`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        gitlab_base_url: values.gitlab_base_url,
        gitlab_api_base_url: values.gitlab_api_base_url,
        gitlab_project_id: Number(values.gitlab_project_id),
        token: values.token,
        webhook_secret: values.webhook_secret,
        verify_tls: values.verify_tls,
        sync_enabled: values.sync_enabled,
      }),
    });

    await refreshAll();
    setBanner({
      type: 'success',
      message: `GitLab integrace pro projekt ${project.name} byla ulozena.`,
    });
    return project;
  }

  async function deleteIntegration(projectId: string) {
    await requestVoid(`/api/v1/projects/${projectId}/gitlab-integration`, { method: 'DELETE' });
    await refreshAll();
    setBanner({ type: 'success', message: 'GitLab integrace byla smazana.' });
  }

  async function getProjectAccess(projectId: string, signal?: AbortSignal) {
    return requestJson<ProjectAccessOverview>(`/api/v1/projects/${projectId}/access`, { signal });
  }

  async function updateProjectAccess(projectId: string, values: UpdateProjectAccessValues) {
    const overview = await requestJson<ProjectAccessOverview>(`/api/v1/projects/${projectId}/access`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(values),
    });
    setBanner({
      type: 'success',
      message: 'Projektova opravneni byla ulozena.',
    });
    return overview;
  }

  async function validateIntegration(projectId: string, values: IntegrationFormValues) {
    return requestJson<GitLabIntegrationValidationResult>(
      `/api/v1/projects/${projectId}/gitlab-integration/validate`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          gitlab_base_url: values.gitlab_base_url,
          gitlab_api_base_url: values.gitlab_api_base_url,
          gitlab_project_id: Number(values.gitlab_project_id),
          token: values.token || null,
          verify_tls: values.verify_tls,
        }),
      },
    );
  }

  async function importProjectIssues(projectId: string) {
    const result = await requestJson<GitLabIssueImportResult>(
      `/api/v1/projects/${projectId}/gitlab-integration/import`,
      {
        method: 'POST',
      },
    );

    await refreshAll();
    setBanner({
      type: 'success',
      message: `Import dokonceny: ${result.imported_count} issues, ${result.created_count} novych, ${result.updated_count} aktualizovanych.`,
    });
    return result;
  }

  async function createIssue(values: CreateIssueValues) {
    const issue = await requestJson<Issue>(`/api/v1/projects/${values.project_id}/issues`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        title: values.title,
        description: values.description || null,
      }),
    });

    await refreshAll();
    setBanner({
      type: 'success',
      message: `Issue ${issue.title} byl vytvoren.`,
    });
    return issue;
  }

  async function updateIssue(issueId: string, values: UpdateIssueValues) {
    const issue = await requestJson<Issue>(`/api/v1/issues/${issueId}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(values),
    });

    await refreshAll();
    setBanner({
      type: 'success',
      message: 'Issue bylo ulozeno.',
    });
    return issue;
  }

  async function uploadProjectIssueAttachment(projectId: string, file: File) {
    const formData = new FormData();
    formData.append('file', file);

    return requestJson<IssueUpload>(`/api/v1/projects/${projectId}/uploads`, {
      method: 'POST',
      body: formData,
    });
  }

  async function createIssueComment(issueId: string, values: CreateCommentValues) {
    const comment = await requestJson<Comment>(`/api/v1/issues/${issueId}/comments`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(values),
    });

    setBanner({
      type: 'success',
      message: 'Komentar byl ulozen.',
    });
    return comment;
  }

  async function uploadIssueAttachment(issueId: string, file: File) {
    const formData = new FormData();
    formData.append('file', file);

    return requestJson<IssueUpload>(`/api/v1/issues/${issueId}/uploads`, {
      method: 'POST',
      body: formData,
    });
  }

  async function deleteIssueUpload(uploadId: string) {
    await requestVoid(`/api/v1/uploads/${uploadId}`, {
      method: 'DELETE',
    });
  }

  async function syncIssueComments(issueId: string) {
    const result = await requestJson<GitLabCommentImportResult>(`/api/v1/issues/${issueId}/comments/sync`, {
      method: 'POST',
    });

    setBanner({
      type: 'success',
      message: `Sync comments dokoncen: ${result.imported_count} comments, ${result.created_count} novych, ${result.updated_count} aktualizovanych.`,
    });
    return result;
  }

  async function getIssueDetail(issueId: string, signal?: AbortSignal) {
    return requestJson<IssueDetailData>(`/api/v1/issues/${issueId}`, { signal });
  }

  async function getIssueAccess(issueId: string, signal?: AbortSignal) {
    return requestJson<IssueAccessOverview>(`/api/v1/issues/${issueId}/access`, { signal });
  }

  async function updateIssueAccess(issueId: string, values: UpdateIssueAccessValues) {
    const overview = await requestJson<IssueAccessOverview>(`/api/v1/issues/${issueId}/access`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(values),
    });

    await refreshAll();
    setBanner({
      type: 'success',
      message: 'Pristupy k issue byly ulozeny.',
    });
    return overview;
  }

  async function getUserManagementOverview() {
    return requestJson<UserManagementOverview>('/api/v1/users');
  }

  async function updateUser(userId: string, values: UpdateUserValues) {
    const user = await requestJson<ManagedUser>(`/api/v1/users/${userId}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(values),
    });
    setBanner({ type: 'success', message: `Uzivatel ${user.email} byl upraven.` });
    return user;
  }

  async function removeIssuePermission(issueId: string, subjectType: string, subjectId: string) {
    await requestVoid(`/api/v1/issues/${issueId}/access/${subjectType}/${subjectId}`, { method: 'DELETE' });
  }

  async function getUserAccess(userId: string, signal?: AbortSignal) {
    return requestJson<UserAccessOverview>(`/api/v1/users/${userId}/access`, { signal });
  }

  async function updateUserAccess(userId: string, values: UpdateUserAccessValues) {
    const overview = await requestJson<UserAccessOverview>(`/api/v1/users/${userId}/access`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(values),
    });
    setBanner({ type: 'success', message: 'Přístupy uživatele byly uloženy.' });
    return overview;
  }

  async function inviteUser(values: CreateInvitationValues) {
    const invitation = await requestJson<UserInvitation>('/api/v1/users/invitations', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(values),
    });
    setBanner({
      type: 'success',
      message: `Pozvanka pro ${invitation.email} byla vytvorena a zarazena do fronty pro odeslani.`,
    });
    return invitation;
  }

  async function resendInvitation(invitationId: string) {
    const invitation = await requestJson<UserInvitation>(`/api/v1/users/invitations/${invitationId}/resend`, {
      method: 'POST',
    });
    setBanner({
      type: 'success',
      message: `Pozvanka pro ${invitation.email} byla odeslana znovu.`,
    });
    return invitation;
  }

  async function deleteInvitation(invitationId: string) {
    await requestVoid(`/api/v1/users/invitations/${invitationId}`, {
      method: 'DELETE',
    });
    setBanner({
      type: 'success',
      message: 'Pozvanka byla smazana.',
    });
  }

  async function getInvitationPreview(inviteToken: string) {
    return requestJson<InvitationPreview>(`/api/v1/invitations/${inviteToken}`);
  }

  async function acceptInvitation(inviteToken: string, values: AcceptInvitationValues) {
    setBanner(null);
    const payload = await requestJson<{ user: User }>(`/api/v1/invitations/${inviteToken}/accept`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(values),
    });
    setCurrentUser(payload.user);
    await loadAppData();
    setBanner({ type: 'success', message: 'Ucet byl aktivovan a jsi prihlaseny.' });
  }

  async function getAdminHealth(signal?: AbortSignal) {
    return requestJson<AdminHealth>('/api/v1/admin/health', { signal });
  }

  const value: AppContextValue = {
    currentUser,
    authLoading,
    loginLoading,
    dataLoading,
    overview,
    projects,
    issues,
    banner,
    locale,
    t: (key) => translate(locale, key),
    login,
    logout,
    updateProfile,
    changePassword,
    requestPasswordRecovery,
    getPasswordRecoveryPreview,
    resetPassword,
    refreshAll,
    createProject,
    updateProject,
    deleteProject,
    saveIntegration,
    deleteIntegration,
    getProjectAccess,
    updateProjectAccess,
    validateIntegration,
    importProjectIssues,
    syncIssueComments,
    createIssue,
    updateIssue,
    uploadProjectIssueAttachment,
    createIssueComment,
    uploadIssueAttachment,
    deleteIssueUpload,
    getIssueDetail,
    getIssueAccess,
    updateIssueAccess,
    getUserManagementOverview,
    updateUser,
    getUserAccess,
    updateUserAccess,
    removeIssuePermission,
    inviteUser,
    resendInvitation,
    deleteInvitation,
    getInvitationPreview,
    acceptInvitation,
    getAdminHealth,
    clearBanner: () => setBanner(null),
    setSuccessBanner: (message) => setBanner({ type: 'success', message }),
    setErrorBanner: (message) => setBanner({ type: 'error', message }),
  };

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useAppContext() {
  const context = useContext(AppContext);
  if (!context) {
    throw new Error('useAppContext must be used inside AppProvider');
  }
  return context;
}

export function isUnauthorizedError(error: unknown) {
  return error instanceof ApiError && error.status === 401;
}
