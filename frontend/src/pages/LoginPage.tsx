import { useEffect, useState } from 'react';
import { Navigate, useLocation, useNavigate, useSearchParams } from 'react-router-dom';

import { InvitationAcceptanceScreen } from '../components/InvitationAcceptanceScreen';
import { LoginScreen } from '../components/LoginScreen';
import { PageState } from '../components/PageState';
import { PasswordRecoveryRequestScreen } from '../components/PasswordRecoveryRequestScreen';
import { PasswordResetScreen } from '../components/PasswordResetScreen';
import { useAppContext } from '../context/AppContext';
import type { InvitationPreview, PasswordRecoveryPreview } from '../types';

export function LoginPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const [searchParams] = useSearchParams();
  const inviteToken = searchParams.get('invite');
  const recoveryToken = searchParams.get('recovery');
  const returnTo =
    typeof location.state === 'object' &&
    location.state !== null &&
    'from' in location.state &&
    typeof location.state.from === 'string'
      ? location.state.from
      : '/overview';
  const {
    currentUser,
    login,
    loginLoading,
    banner,
    getInvitationPreview,
    acceptInvitation,
	    requestPasswordRecovery,
	    getPasswordRecoveryPreview,
	    resetPassword,
	    t,
	  } = useAppContext();
  const [invitation, setInvitation] = useState<InvitationPreview | null>(null);
  const [inviteLoading, setInviteLoading] = useState(false);
  const [inviteError, setInviteError] = useState<string | null>(null);
  const [acceptLoading, setAcceptLoading] = useState(false);
  const [recoveryMode, setRecoveryMode] = useState(false);
  const [recoveryRequestLoading, setRecoveryRequestLoading] = useState(false);
  const [recoveryPreview, setRecoveryPreview] = useState<PasswordRecoveryPreview | null>(null);
  const [recoveryLoading, setRecoveryLoading] = useState(false);
  const [recoveryError, setRecoveryError] = useState<string | null>(null);
  const [resetLoading, setResetLoading] = useState(false);

  useEffect(() => {
    if (!inviteToken) {
      setInvitation(null);
      setInviteError(null);
      return;
    }

    const currentInviteToken = inviteToken;
    let cancelled = false;

    async function loadInvitation() {
      try {
        setInviteLoading(true);
        setInviteError(null);
        const preview = await getInvitationPreview(currentInviteToken);
        if (!cancelled) {
          setInvitation(preview);
        }
      } catch (error) {
        if (!cancelled) {
          setInviteError(error instanceof Error ? error.message : 'Nepodarilo se nacist pozvanku.');
        }
      } finally {
        if (!cancelled) {
          setInviteLoading(false);
        }
      }
    }

    void loadInvitation();
    return () => {
      cancelled = true;
    };
  }, [inviteToken]);

  useEffect(() => {
    if (!recoveryToken) {
      setRecoveryPreview(null);
      setRecoveryError(null);
      return;
    }

    const currentRecoveryToken = recoveryToken;
    let cancelled = false;

    async function loadRecoveryPreview() {
      try {
        setRecoveryLoading(true);
        setRecoveryError(null);
        const preview = await getPasswordRecoveryPreview(currentRecoveryToken);
        if (!cancelled) {
          setRecoveryPreview(preview);
        }
      } catch (error) {
        if (!cancelled) {
          setRecoveryError(error instanceof Error ? error.message : 'Recovery token není dostupný.');
        }
      } finally {
        if (!cancelled) {
          setRecoveryLoading(false);
        }
      }
    }

    void loadRecoveryPreview();
    return () => {
      cancelled = true;
    };
  }, [getPasswordRecoveryPreview, recoveryToken]);

  if (currentUser) {
    return <Navigate to={returnTo} replace />;
  }

  if (inviteToken) {
    if (inviteLoading) {
      return <PageState loading />;
    }

    if (inviteError || !invitation) {
	      return <PageState error={inviteError ?? t('login.invitationUnavailable')} />;
    }

    return (
      <InvitationAcceptanceScreen
        invitation={invitation}
        loading={acceptLoading}
        error={banner?.type === 'error' ? banner.message : null}
        onSubmit={async (values) => {
          setAcceptLoading(true);
          try {
            await acceptInvitation(inviteToken, values);
            navigate(returnTo, { replace: true });
          } finally {
            setAcceptLoading(false);
          }
        }}
      />
    );
  }

  if (recoveryToken) {
    if (recoveryLoading) {
      return <PageState loading />;
    }

    if (recoveryError || !recoveryPreview) {
      return <PageState error={recoveryError ?? 'Recovery token není dostupný.'} />;
    }

    return (
      <PasswordResetScreen
        preview={recoveryPreview}
        loading={resetLoading}
        error={banner?.type === 'error' ? banner.message : null}
        onSubmit={async (values) => {
          setResetLoading(true);
          try {
            await resetPassword(recoveryToken, values);
            navigate('/login', { replace: true });
          } finally {
            setResetLoading(false);
          }
        }}
      />
    );
  }

  if (recoveryMode) {
    return (
      <PasswordRecoveryRequestScreen
        loading={recoveryRequestLoading}
        error={banner?.type === 'error' ? banner.message : null}
        onSubmit={async (values) => {
          setRecoveryRequestLoading(true);
          try {
            await requestPasswordRecovery(values);
            setRecoveryMode(false);
          } finally {
            setRecoveryRequestLoading(false);
          }
        }}
      />
    );
  }

  return (
    <LoginScreen
      loading={loginLoading}
      error={banner?.type === 'error' ? banner.message : null}
      onForgotPassword={() => setRecoveryMode(true)}
      onSubmit={async (values) => {
        await login(values);
        navigate(returnTo, { replace: true });
      }}
    />
  );
}
