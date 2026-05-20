import { Anchor, Button, Card, Group, Stack, Text, Title } from '@mantine/core';
import { useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';

import { IssueDetail } from '../components/IssueDetail';
import { PageState } from '../components/PageState';
import { useAppContext } from '../context/AppContext';
import type { CreateCommentValues, IssueAccessOverview, IssueDetailData, UpdateIssueValues } from '../types';

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? '';

export function IssueDetailPage() {
  const navigate = useNavigate();
  const { issueId } = useParams();
  const {
    issues,
    dataLoading,
    createIssueComment,
    getIssueDetail,
    getIssueAccess,
    syncIssueComments,
    updateIssue,
    updateIssueAccess,
    t,
  } = useAppContext();
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [issueDetail, setIssueDetail] = useState<IssueDetailData | null>(null);
  const [accessOverview, setAccessOverview] = useState<IssueAccessOverview | null>(null);
  const [accessLoading, setAccessLoading] = useState(false);
  const [accessSaving, setAccessSaving] = useState(false);
  const [commentsSyncLoading, setCommentsSyncLoading] = useState(false);
  const [commentCreateLoading, setCommentCreateLoading] = useState(false);
  const [issueSaving, setIssueSaving] = useState(false);

  const selectedIssue = issues.find((issue) => issue.id === issueId) ?? null;

  useEffect(() => {
    if (!issueId) {
      return;
    }

    const currentIssueId = issueId;
    const controller = new AbortController();

    async function loadIssueDetail() {
      try {
        setDetailLoading(true);
        setDetailError(null);
        setIssueDetail(null);
        setAccessOverview(null);
        const detail = await getIssueDetail(currentIssueId, controller.signal);
        setIssueDetail(detail);
        if (detail.issue.capabilities.can_manage_access) {
          setAccessLoading(true);
          try {
            const access = await getIssueAccess(currentIssueId, controller.signal);
            setAccessOverview(access);
          } finally {
            setAccessLoading(false);
          }
        }
      } catch (error) {
        if (!(error instanceof DOMException && error.name === 'AbortError')) {
          setDetailError(error instanceof Error ? error.message : 'Nepodarilo se nacist detail issue.');
        }
      } finally {
        setDetailLoading(false);
      }
    }

    void loadIssueDetail();
    return () => controller.abort();
  }, [getIssueAccess, getIssueDetail, issueId]);

  async function handleSaveAccess(values: {
    assignments: Array<{ user_id: string; permission: string }>;
  }) {
    if (!issueId) return;
    setAccessSaving(true);
    try {
      const overview = await updateIssueAccess(issueId, values);
      setAccessOverview(overview);
    } finally {
      setAccessSaving(false);
    }
  }

  async function handleSyncComments() {
    if (!issueId) return;
    setCommentsSyncLoading(true);
    try {
      await syncIssueComments(issueId);
      const detail = await getIssueDetail(issueId);
      setIssueDetail(detail);
    } catch (error) {
      setDetailError(error instanceof Error ? error.message : 'Nepodarilo se synchronizovat komentare.');
    } finally {
      setCommentsSyncLoading(false);
    }
  }

  async function handleUpdateIssue(values: UpdateIssueValues) {
    if (!issueId) return;
    setIssueSaving(true);
    try {
      await updateIssue(issueId, values);
      const detail = await getIssueDetail(issueId);
      setIssueDetail(detail);
    } catch (error) {
      setDetailError(error instanceof Error ? error.message : 'Nepodarilo se upravit issue.');
    } finally {
      setIssueSaving(false);
    }
  }

  async function handleCreateComment(values: CreateCommentValues) {
    if (!issueId) return;
    setCommentCreateLoading(true);
    try {
      await createIssueComment(issueId, values);
      const detail = await getIssueDetail(issueId);
      setIssueDetail(detail);
    } catch (error) {
      setDetailError(error instanceof Error ? error.message : 'Nepodarilo se vytvorit komentar.');
    } finally {
      setCommentCreateLoading(false);
    }
  }

  return (
    <Stack gap="lg">
      <Group justify="space-between" align="flex-start">
        <Stack gap={4}>
          <Anchor component="button" type="button" onClick={() => navigate('/issues')}>
            {t('issues.title')}
          </Anchor>
          <Title order={2}>{issueDetail?.issue.title ?? selectedIssue?.title ?? 'Issue detail'}</Title>
          <Text c="dimmed">
            {issueDetail?.issue.project_name ?? selectedIssue?.project_name ?? ''}
          </Text>
        </Stack>
        <Button variant="default" onClick={() => navigate('/issues')}>
          Back to issues
        </Button>
      </Group>

      {detailLoading ? (
        <Card withBorder radius="md" padding="lg">
          <PageState loading />
        </Card>
      ) : detailError ? (
        <Card withBorder radius="md" padding="lg">
          <PageState error={detailError} />
        </Card>
      ) : (
        <IssueDetail
          apiBaseUrl={API_BASE_URL}
          issueDetail={issueDetail}
          showAccessPanel={issueDetail?.issue.capabilities.can_manage_access}
          accessOverview={accessOverview}
          accessLoading={accessLoading}
          accessSaving={accessSaving}
          commentsSyncLoading={commentsSyncLoading}
          commentCreateLoading={commentCreateLoading}
          issueSaving={issueSaving}
          onSyncComments={issueDetail?.issue.capabilities.can_sync_comments ? handleSyncComments : undefined}
          onCreateComment={issueDetail?.issue.capabilities.can_comment ? handleCreateComment : undefined}
          onUpdateIssue={handleUpdateIssue}
          onSaveAccess={handleSaveAccess}
        />
      )}
    </Stack>
  );
}
