import {
  Anchor,
  Avatar,
  Badge,
  Box,
  Button,
  Card,
  Checkbox,
  Divider,
  Group,
  Input,
  SegmentedControl,
  Stack,
  Text,
  Textarea,
  TextInput,
  Title,
} from '@mantine/core';
import { useForm } from '@mantine/form';
import { useEffect, useRef, useState, type ChangeEvent, type ClipboardEvent, type DragEvent } from 'react';

import { useAppContext } from '../context/AppContext';
import { getIssueDetailActionVisibility } from '../lib/capability-ui';
import { gravatarUrl } from '../lib/gravatar';
import type {
  Attachment,
  Comment,
  CreateCommentValues,
  IssueAccessOverview,
  IssueDetailData,
  IssueUpload,
  UpdateIssueValues,
} from '../types';
import { CommentMarkdown } from './CommentMarkdown';
import { IssueAccessPanel } from './IssueAccessPanel';

type IssueDetailProps = {
  apiBaseUrl: string;
  issueDetail: IssueDetailData | null;
  showAccessPanel?: boolean;
  accessOverview?: IssueAccessOverview | null;
  accessLoading?: boolean;
  accessSaving?: boolean;
  commentsSyncLoading?: boolean;
  commentCreateLoading?: boolean;
  issueSaving?: boolean;
  onSyncComments?: () => Promise<void> | void;
  onUpdateIssue?: (values: UpdateIssueValues) => Promise<void> | void;
  onCreateComment?: (values: CreateCommentValues) => Promise<void> | void;
  onSaveAccess?: (values: { assignments: Array<{ user_id: string; permission: string }> }) => Promise<void> | void;
};

type CommentThreadNode = {
  comment: Comment;
  replyTo: number | null;
  discussionId: string | null;
  children: CommentThreadNode[];
};

type CommentThreadProps = {
  apiBaseUrl: string;
  thread: CommentThreadNode;
  depth: number;
  isLastSibling: boolean;
  gitlabIssueUrl: string | null;
  issueId: string | null;
  activeReplyToNoteId: number | null;
  commentCreateLoading?: boolean;
  uploadingAttachment?: boolean;
  setUploadingAttachment: (value: boolean) => void;
  uploadIssueAttachment: (issueId: string, file: File) => Promise<IssueUpload>;
  deleteIssueUpload: (uploadId: string) => Promise<void>;
  onReplyToggle?: (noteId: number | null) => void;
  onCreateComment?: (values: CreateCommentValues) => Promise<void> | void;
};

export function IssueDetail({
  apiBaseUrl,
  issueDetail,
  showAccessPanel,
  accessOverview,
  accessLoading,
  accessSaving,
  commentsSyncLoading,
  commentCreateLoading,
  issueSaving,
  onSyncComments,
  onUpdateIssue,
  onCreateComment,
  onSaveAccess,
}: IssueDetailProps) {
  const { t, uploadIssueAttachment, deleteIssueUpload } = useAppContext();
	  const commentForm = useForm<CreateCommentValues>({
	    initialValues: { body: '' },
	    validate: {
	      body: (value) => (value.trim() ? null : t('issueDetail.commentRequired')),
	    },
	  });
  const issueForm = useForm<Required<Pick<UpdateIssueValues, 'title' | 'description'>>>({
	    initialValues: { title: '', description: '' },
	    validate: {
	      title: (value) => (value.trim() ? null : t('issueDetail.titleRequired')),
	    },
	  });
  const [replyToNoteId, setReplyToNoteId] = useState<number | null>(null);
  const [editingIssue, setEditingIssue] = useState(false);
  const [showSystemNotes, setShowSystemNotes] = useState(false);
  const [uploadingAttachment, setUploadingAttachment] = useState(false);
  const [dragActive, setDragActive] = useState(false);
	  const [pendingUploads, setPendingUploads] = useState<IssueUpload[]>([]);
	  const [commentEditorMode, setCommentEditorMode] = useState<'write' | 'preview'>('write');
	  const [commentComposerOpen, setCommentComposerOpen] = useState(false);
	  const commentFileInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (!issueDetail) {
      issueForm.setValues({ title: '', description: '' });
      setEditingIssue(false);
      return;
    }

	    issueForm.setValues({
	      title: issueDetail.issue.title,
	      description: issueDetail.issue.description,
	    });
	    setEditingIssue(false);
	    setCommentComposerOpen(false);
	    setPendingUploads([]);
	  }, [issueDetail?.issue.id, issueDetail?.issue.title, issueDetail?.issue.description]);

  const gitlabIssueUrl =
    issueDetail && issueDetail.issue.gitlab_issue_iid > 0
      ? `${apiBaseUrl}/api/v1/issues/${issueDetail.issue.id}/gitlab-link`
      : null;
  const visibleComments = (issueDetail?.comments ?? []).filter((comment) => showSystemNotes || !comment.system_note);
  const threadedComments = buildCommentThreads(visibleComments);
  const actionVisibility = getIssueDetailActionVisibility(issueDetail?.issue.capabilities, {
    canUpdateIssue: Boolean(onUpdateIssue),
    canCreateComment: Boolean(onCreateComment),
    canSyncComments: Boolean(onSyncComments),
  });
  const canEditIssue = actionVisibility.showEditIssue;
  const canChangeIssueState = actionVisibility.showChangeIssueState;
  const issueState = normalizeIssueState(issueDetail?.issue.state ?? 'open');

  return (
    <Stack gap="lg">
      <Card withBorder radius="md" padding="lg">
        <Stack gap="md">
          <Group justify="space-between" align="flex-start">
            <Title order={3}>{t('issueDetail.title')}</Title>
            <Group gap="sm">
              {canChangeIssueState && issueDetail ? (
                <Button
                  variant="light"
                  color={issueState === 'closed' ? 'teal' : 'red'}
                  loading={issueSaving}
                  onClick={() =>
                    void onUpdateIssue?.({
                      title: issueDetail.issue.title,
                      description: issueDetail.issue.description,
                      state: issueState === 'closed' ? 'open' : 'closed',
                    })
                  }
                >
                  {issueState === 'closed' ? t('issueDetail.reopenIssue') : t('issueDetail.closeIssue')}
                </Button>
              ) : null}
              {canEditIssue && issueDetail && !editingIssue ? (
                <Button variant="default" onClick={() => setEditingIssue(true)}>
                  {t('issueDetail.editIssue')}
                </Button>
              ) : null}
              {gitlabIssueUrl ? (
                <Anchor href={gitlabIssueUrl} target="_blank" rel="noreferrer">
                  {t('issueDetail.openInGitLab')}
                </Anchor>
              ) : null}
            </Group>
          </Group>
          {issueDetail ? (
            <>
              {editingIssue && canEditIssue ? (
                <form
                  onSubmit={issueForm.onSubmit(async (values) => {
                    await onUpdateIssue?.({
                      title: values.title.trim(),
                      description: values.description,
                    });
                    setEditingIssue(false);
                  })}
                >
                  <Stack gap="sm">
                    <TextInput label={t('issueDetail.issueTitle')} {...issueForm.getInputProps('title')} />
                    <Textarea
                      label={t('issueDetail.issueDescription')}
                      minRows={8}
                      autosize
                      maxRows={18}
                      {...issueForm.getInputProps('description')}
                    />
                    <Group justify="flex-end">
                      <Button
                        variant="default"
                        onClick={() => {
                          issueForm.setValues({
                            title: issueDetail.issue.title,
                            description: issueDetail.issue.description,
                          });
                          setEditingIssue(false);
                        }}
                      >
                        {t('issueDetail.cancelEdit')}
                      </Button>
                      <Button type="submit" loading={issueSaving}>
                        {t('issueDetail.saveIssue')}
                      </Button>
                    </Group>
                  </Stack>
                </form>
              ) : (
                <Box c="dimmed">
                  <CommentMarkdown
                    apiBaseUrl={apiBaseUrl}
                    body={issueDetail.issue.description}
                    attachments={issueDetail.issue_attachments}
                  />
                </Box>
              )}
              <Group gap="xs">
                <Badge variant="light">{issueDetail.issue.state}</Badge>
                <Badge variant="light" color="blue">
                  {issueDetail.issue.sync_state}
                </Badge>
                {issueDetail.issue.gitlab_issue_iid > 0 ? (
                  <Badge variant="outline">GitLab #{issueDetail.issue.gitlab_issue_iid}</Badge>
                ) : (
                  <Badge variant="outline" color="gray">
                    {t('issueDetail.localIssue')}
                  </Badge>
                )}
              </Group>
	              {getVisibleAttachmentLinks(issueDetail.issue.description, issueDetail.issue_attachments).length > 0 ? (
	                <Group gap="sm">
	                  <Text size="sm" c="dimmed">
	                    {t('issueDetail.attachments')}
	                  </Text>
	                  {getVisibleAttachmentLinks(issueDetail.issue.description, issueDetail.issue_attachments).map((attachment) => (
	                    <Button
                      key={attachment.id}
                      variant="light"
                      component="a"
                      href={`${apiBaseUrl}${attachment.proxy_path}`}
                      target="_blank"
                      rel="noreferrer"
	                    >
	                      {attachment.filename}
	                    </Button>
                  ))}
                </Group>
              ) : null}
            </>
          ) : (
            <Text c="dimmed">{t('issueDetail.noIssue')}</Text>
          )}
        </Stack>
      </Card>

      <Card withBorder radius="md" padding="lg">
        <Stack gap="md">
	          <Group justify="space-between" align="center">
	            <Title order={3}>{t('issueDetail.commentsTitle')}</Title>
	            <Group gap="sm">
	              {actionVisibility.showCommentEditor ? (
	                <Button variant={commentComposerOpen ? 'default' : 'light'} onClick={() => setCommentComposerOpen((open) => !open)}>
	                  {commentComposerOpen ? t('issueDetail.closeComposer') : t('issueDetail.openComposer')}
	                </Button>
	              ) : null}
	              <Checkbox
                checked={showSystemNotes}
                onChange={(event) => setShowSystemNotes(event.currentTarget.checked)}
                label={t('issueDetail.showSystemNotes')}
              />
              {actionVisibility.showSyncComments ? (
                <Button variant="light" loading={commentsSyncLoading} onClick={() => void onSyncComments?.()}>
                  {t('issueDetail.syncComments')}
                </Button>
              ) : null}
            </Group>
          </Group>
	          {actionVisibility.showCommentEditor && commentComposerOpen ? (
	            <form
	              onSubmit={commentForm.onSubmit(async (values) => {
	                await onCreateComment?.({ body: values.body.trim(), reply_to_note_id: null });
	                commentForm.reset();
	                setPendingUploads([]);
	                setCommentComposerOpen(false);
	              })}
	            >
	              <Stack gap="sm">
	                <Text size="sm" fw={600}>
	                  {t('issueDetail.commentComposerTitle')}
	                </Text>
	                <Box
                  p="sm"
                  style={{
                    border: `1px dashed ${dragActive ? 'var(--mantine-color-blue-5)' : 'var(--mantine-color-gray-4)'}`,
                    borderRadius: 'var(--mantine-radius-md)',
                    background: dragActive
                      ? 'light-dark(var(--mantine-color-blue-0), rgba(25, 113, 194, 0.16))'
                      : undefined,
                  }}
                  onDragOver={(event) => {
                    event.preventDefault();
                    setDragActive(true);
                  }}
                  onDragLeave={() => setDragActive(false)}
                  onDrop={(event) =>
                    issueDetail
                      ? void handleDropUpload(
                          event,
                          issueDetail.issue.id,
                          commentForm.values.body,
                          (value) => commentForm.setFieldValue('body', value),
                          uploadIssueAttachment,
                          (upload) => setPendingUploads((current) => [...current, upload]),
                          setUploadingAttachment,
                          setDragActive,
                        )
                      : undefined
                  }
                >
                  <Stack gap="sm">
                    <SegmentedControl
                      value={commentEditorMode}
                      onChange={(value) => setCommentEditorMode(value as 'write' | 'preview')}
                      data={[
                        { label: t('common.write'), value: 'write' },
                        { label: t('common.preview'), value: 'preview' },
                      ]}
                    />
                    {commentEditorMode === 'write' ? (
	                      <Textarea
	                        placeholder={t('issueDetail.commentPlaceholder')}
	                        minRows={4}
	                        autosize
	                        maxRows={12}
                        onPaste={(event) =>
                          issueDetail
                            ? void handlePasteUpload(
                                event,
                                issueDetail.issue.id,
                                commentForm.values.body,
                                (value) => commentForm.setFieldValue('body', value),
                                uploadIssueAttachment,
                                (upload) => setPendingUploads((current) => [...current, upload]),
                                setUploadingAttachment,
                              )
                            : undefined
                        }
                        {...commentForm.getInputProps('body')}
                      />
                    ) : (
                      <Box
	                        p="sm"
	                        mih={140}
                        style={{
                          border: '1px solid var(--mantine-color-gray-3)',
                          borderRadius: 'var(--mantine-radius-md)',
                          background: 'var(--mantine-color-body)',
                        }}
                      >
                        {commentForm.values.body.trim() ? (
                          <CommentMarkdown
                            apiBaseUrl={apiBaseUrl}
                            body={commentForm.values.body}
                            attachments={issueDetail ? mapUploadsToAttachments(pendingUploads) : []}
                          />
                        ) : (
                          <Text size="sm" c="dimmed">
                            {t('common.previewEmpty')}
                          </Text>
                        )}
                      </Box>
                    )}
                    <Group justify="space-between" align="center">
                      <Text size="xs" c="dimmed">
                        {t('issueDetail.dragAndDropHint')}
                      </Text>
                      <Button
                        variant="default"
                        size="xs"
                        onClick={() => commentFileInputRef.current?.click()}
                        loading={uploadingAttachment}
                      >
                        {t('issueDetail.attachFile')}
                      </Button>
                    </Group>
                    <Input
                      ref={commentFileInputRef}
                      type="file"
                      style={{ display: 'none' }}
                      onChange={(event) =>
                        issueDetail
                          ? void handleFileInputUpload(
                              event,
                              issueDetail.issue.id,
                              commentForm.values.body,
                              (value) => commentForm.setFieldValue('body', value),
                              uploadIssueAttachment,
                              (upload) => setPendingUploads((current) => [...current, upload]),
                              setUploadingAttachment,
                            )
                          : undefined
                      }
                    />
                    <PendingUploadsList
                      uploads={pendingUploads}
                      onRemove={
                        issueDetail
                          ? async (upload) => {
                              await deleteIssueUpload(upload.upload_id);
                              setPendingUploads((current) =>
                                current.filter((candidate) => candidate.upload_id !== upload.upload_id),
                              );
                              commentForm.setFieldValue(
                                'body',
                                removeUploadMarkdown(commentForm.values.body, upload.markdown, upload.proxy_path),
                              );
                            }
                          : undefined
                      }
                    />
                  </Stack>
                </Box>
                <Group justify="flex-end">
                  <Button type="submit" loading={commentCreateLoading || uploadingAttachment}>
                    {t('issueDetail.addComment')}
                  </Button>
                </Group>
              </Stack>
            </form>
          ) : null}
          {threadedComments.length ? (
            threadedComments.map((thread, index) => (
              <Box key={thread.comment.id}>
                {index > 0 ? <Divider my="md" /> : null}
                <CommentThread
                  apiBaseUrl={apiBaseUrl}
                  thread={thread}
                  depth={0}
                  isLastSibling
                  gitlabIssueUrl={gitlabIssueUrl}
                  issueId={issueDetail?.issue.id ?? null}
                  activeReplyToNoteId={replyToNoteId}
                  commentCreateLoading={commentCreateLoading}
                  uploadingAttachment={uploadingAttachment}
                  setUploadingAttachment={setUploadingAttachment}
                  uploadIssueAttachment={uploadIssueAttachment}
                  deleteIssueUpload={deleteIssueUpload}
                  onReplyToggle={onCreateComment ? setReplyToNoteId : undefined}
                  onCreateComment={onCreateComment}
                />
              </Box>
            ))
	          ) : (
	            <Stack gap="xs" align="flex-start">
	              <Text c="dimmed">{t('issueDetail.noComments')}</Text>
	              {actionVisibility.showCommentEditor ? (
	                <Button size="xs" variant="light" onClick={() => setCommentComposerOpen(true)}>
	                  {t('issueDetail.openComposer')}
	                </Button>
	              ) : null}
	            </Stack>
	          )}
        </Stack>
      </Card>

      {showAccessPanel && actionVisibility.showAccessPanel ? (
        <IssueAccessPanel
          accessOverview={accessOverview ?? null}
          loading={accessLoading}
          saving={accessSaving}
          onSave={(values) => onSaveAccess?.(values)}
        />
      ) : null}
    </Stack>
  );
}

function CommentThread({
  apiBaseUrl,
  thread,
  depth,
  isLastSibling,
  gitlabIssueUrl,
  issueId,
  activeReplyToNoteId,
  commentCreateLoading,
  uploadingAttachment,
  setUploadingAttachment,
  uploadIssueAttachment,
  deleteIssueUpload,
  onReplyToggle,
  onCreateComment,
}: CommentThreadProps) {
	  const { t } = useAppContext();
	  const { comment, children } = thread;
	  const visibleBody = stripReplyMarker(comment.body_raw);
	  const listedAttachments = getVisibleAttachmentLinks(visibleBody, comment.attachments);
	  const gitlabCommentUrl =
    issueId && comment.gitlab_note_id > 0
      ? `${apiBaseUrl}/api/v1/issues/${issueId}/comments/${comment.gitlab_note_id}/gitlab-link`
      : null;
  const nested = depth > 0;
  const nestedRailContinues = nested && (!isLastSibling || children.length > 0);
  const rootRailContinues = !nested && children.length > 0;
  const replyOpen = activeReplyToNoteId === comment.gitlab_note_id;
  const [dragActive, setDragActive] = useState(false);
  const [pendingReplyUploads, setPendingReplyUploads] = useState<IssueUpload[]>([]);
  const [replyEditorMode, setReplyEditorMode] = useState<'write' | 'preview'>('write');
  const replyFileInputRef = useRef<HTMLInputElement | null>(null);
	  const replyForm = useForm<CreateCommentValues>({
	    initialValues: { body: `@${comment.author_name} ` },
	    validate: {
	      body: (value) => (value.trim() ? null : t('issueDetail.commentRequired')),
	    },
	  });

  return (
    <Stack
      gap="xs"
      pl={nested ? 28 : 0}
      ml={nested ? 34 : 0}
      style={{ position: 'relative' }}
    >
      {nested ? (
        <>
          <Box
            aria-hidden="true"
            style={{
              position: 'absolute',
              left: 10,
              top: -12,
              bottom: nestedRailContinues ? -16 : 'auto',
              height: nestedRailContinues ? 'auto' : 32,
              borderLeft: '2px solid var(--mantine-color-blue-2)',
              zIndex: 0,
            }}
          />
          <Box
            aria-hidden="true"
            style={{
              position: 'absolute',
              left: 10,
              top: 20,
              width: 38,
              borderTop: '2px solid var(--mantine-color-blue-2)',
              zIndex: 0,
            }}
          />
        </>
      ) : null}
      <Group align="flex-start" wrap="nowrap">
        <Box
          style={{
            position: 'relative',
            flex: '0 0 40px',
            width: 40,
            alignSelf: rootRailContinues ? 'stretch' : undefined,
            zIndex: 1,
          }}
        >
          {rootRailContinues ? (
            <Box
              aria-hidden="true"
              style={{
                position: 'absolute',
                left: 44,
                top: 20,
                bottom: -12,
                borderLeft: '2px solid var(--mantine-color-blue-2)',
                zIndex: 0,
              }}
            />
          ) : null}
          {nested ? (
            <Box
              aria-hidden="true"
              style={{
                position: 'absolute',
                left: 40,
                top: 20,
                width: 16,
                borderTop: '2px solid var(--mantine-color-blue-2)',
                zIndex: 0,
              }}
            />
          ) : null}
          <Avatar
            radius="md"
            size={40}
            src={gravatarUrl(comment.author_external_id || comment.author_name, 80)}
            style={{
              position: 'relative',
              zIndex: 1,
              background: 'var(--mantine-color-body)',
            }}
          >
            {comment.author_name.slice(0, 1).toUpperCase()}
          </Avatar>
        </Box>
        <Box
          p={nested ? 'sm' : 0}
          bg={nested ? 'light-dark(var(--mantine-color-gray-0), var(--mantine-color-dark-6))' : undefined}
          bd={nested ? '1px solid var(--mantine-color-blue-1)' : undefined}
          style={{
            flex: 1,
            borderRadius: nested ? 'var(--mantine-radius-md)' : undefined,
          }}
        >
          <Group justify="space-between" align="flex-start">
            <div>
              <Text size="sm">{comment.author_name}</Text>
              <Text size="xs" c="dimmed">
                {new Date(comment.created_at).toLocaleString()}
              </Text>
            </div>
            <Group gap="sm" align="center">
              {thread.replyTo ? (
                <Badge size="sm" variant="light" color="blue">
                  {t('issueDetail.replyToLabel')} #{thread.replyTo}
                </Badge>
              ) : null}
	              {comment.gitlab_note_id > 0 ? (
	                <Text size="xs" c="dimmed">
	                  {t('issueDetail.gitlabNote')} #{comment.gitlab_note_id}
	                </Text>
	              ) : null}
              {gitlabCommentUrl ? (
                <Anchor href={gitlabCommentUrl} target="_blank" rel="noreferrer" size="sm">
                  {t('issueDetail.openNoteInGitLab')}
                </Anchor>
              ) : null}
            </Group>
          </Group>
          <Box pt="sm">
            <CommentMarkdown apiBaseUrl={apiBaseUrl} body={visibleBody} attachments={comment.attachments} />
          </Box>
          <Group gap="sm" mt="sm">
            {onReplyToggle && !comment.system_note ? (
              <Button
                size="xs"
                variant="subtle"
                onClick={() => {
                  replyForm.setFieldValue('body', `@${comment.author_name} `);
                  setPendingReplyUploads([]);
                  onReplyToggle?.(replyOpen ? null : comment.gitlab_note_id);
                }}
              >
                {t('issueDetail.reply')}
              </Button>
            ) : null}
	            {listedAttachments.length > 0 ? (
	              <>
	                <Text size="xs" c="dimmed">
	                  {t('issueDetail.attachments')}
	                </Text>
	                {listedAttachments.map((attachment) => (
	                  <Button
	                    key={attachment.id}
	                    size="xs"
	                    variant="subtle"
	                    component="a"
	                    href={`${apiBaseUrl}${attachment.proxy_path}`}
	                    target="_blank"
	                    rel="noreferrer"
	                  >
	                    {attachment.filename}
	                  </Button>
	                ))}
	              </>
	            ) : null}
          </Group>

          {replyOpen && onCreateComment ? (
            <Box
              mt="md"
              p="sm"
              style={{
                borderLeft: '2px solid var(--mantine-color-blue-4)',
                background: 'light-dark(var(--mantine-color-gray-0), var(--mantine-color-dark-6))',
                borderRadius: 'var(--mantine-radius-md)',
              }}
            >
              <Stack gap="sm">
                <Group justify="space-between" align="center">
                  <Text size="sm" c="dimmed">
                    {t('issueDetail.replyingTo')} #{comment.gitlab_note_id}
                  </Text>
                  <Button variant="subtle" size="compact-sm" onClick={() => onReplyToggle?.(null)}>
                    {t('issueDetail.cancelReply')}
                  </Button>
                </Group>
                <form
                  onSubmit={replyForm.onSubmit(async (values) => {
                    await onCreateComment?.({
                      body: values.body.trim(),
                      reply_to_note_id: comment.gitlab_note_id,
                    });
                    replyForm.reset();
                    replyForm.setFieldValue('body', `@${comment.author_name} `);
                    setPendingReplyUploads([]);
                    onReplyToggle?.(null);
                  })}
                >
                  <Stack gap="sm">
                    <Box
                      p="sm"
                      style={{
                        border: `1px dashed ${dragActive ? 'var(--mantine-color-blue-5)' : 'var(--mantine-color-gray-4)'}`,
                        borderRadius: 'var(--mantine-radius-md)',
                        background: dragActive
                          ? 'light-dark(var(--mantine-color-blue-0), rgba(25, 113, 194, 0.16))'
                          : undefined,
                      }}
                      onDragOver={(event) => {
                        event.preventDefault();
                        setDragActive(true);
                      }}
                      onDragLeave={() => setDragActive(false)}
                      onDrop={(event) =>
                        issueId
                          ? void handleDropUpload(
                              event,
                              issueId,
                              replyForm.values.body,
                              (value) => replyForm.setFieldValue('body', value),
                              uploadIssueAttachment,
                              (upload) => setPendingReplyUploads((current) => [...current, upload]),
                              setUploadingAttachment,
                              setDragActive,
                            )
                          : undefined
                      }
                    >
                      <Stack gap="sm">
                        <SegmentedControl
                          value={replyEditorMode}
                          onChange={(value) => setReplyEditorMode(value as 'write' | 'preview')}
                          data={[
                            { label: t('common.write'), value: 'write' },
                            { label: t('common.preview'), value: 'preview' },
                          ]}
                        />
                        {replyEditorMode === 'write' ? (
	                          <Textarea
	                            minRows={3}
	                            autosize
	                            maxRows={10}
                            onPaste={(event) =>
                              issueId
                                ? void handlePasteUpload(
                                    event,
                                    issueId,
                                    replyForm.values.body,
                                    (value) => replyForm.setFieldValue('body', value),
                                    uploadIssueAttachment,
                                    (upload) => setPendingReplyUploads((current) => [...current, upload]),
                                    setUploadingAttachment,
                                  )
                                : undefined
                            }
                            {...replyForm.getInputProps('body')}
                          />
                        ) : (
                          <Box
	                            p="sm"
	                            mih={120}
                            style={{
                              border: '1px solid var(--mantine-color-gray-3)',
                              borderRadius: 'var(--mantine-radius-md)',
                              background: 'var(--mantine-color-body)',
                            }}
                          >
                            {replyForm.values.body.trim() ? (
                              <CommentMarkdown
                                apiBaseUrl={apiBaseUrl}
                                body={replyForm.values.body}
                                attachments={mapUploadsToAttachments(pendingReplyUploads)}
                              />
                            ) : (
                              <Text size="sm" c="dimmed">
                                {t('common.previewEmpty')}
                              </Text>
                            )}
                          </Box>
                        )}
                        <Group justify="space-between" align="center">
                          <Text size="xs" c="dimmed">
                            {t('issueDetail.dragAndDropHint')}
                          </Text>
                          <Button
                            variant="default"
                            size="xs"
                            onClick={() => replyFileInputRef.current?.click()}
                            loading={uploadingAttachment}
                          >
                            {t('issueDetail.attachFile')}
                          </Button>
                        </Group>
                        <Input
                          ref={replyFileInputRef}
                          type="file"
                          style={{ display: 'none' }}
                          onChange={(event) =>
                            issueId
                              ? void handleFileInputUpload(
                                  event,
                                  issueId,
                                  replyForm.values.body,
                                  (value) => replyForm.setFieldValue('body', value),
                                  uploadIssueAttachment,
                                  (upload) => setPendingReplyUploads((current) => [...current, upload]),
                                  setUploadingAttachment,
                                )
                              : undefined
                          }
                        />
                        <PendingUploadsList
                          uploads={pendingReplyUploads}
                          onRemove={
                            issueId
                              ? async (upload) => {
                                  await deleteIssueUpload(upload.upload_id);
                                  setPendingReplyUploads((current) =>
                                    current.filter((candidate) => candidate.upload_id !== upload.upload_id),
                                  );
                                  replyForm.setFieldValue(
                                    'body',
                                    removeUploadMarkdown(replyForm.values.body, upload.markdown, upload.proxy_path),
                                  );
                                }
                              : undefined
                          }
                        />
                      </Stack>
                    </Box>
                    <Group justify="flex-end">
                      <Button type="submit" loading={commentCreateLoading || uploadingAttachment}>
                        {t('issueDetail.addReply')}
                      </Button>
                    </Group>
                  </Stack>
                </form>
              </Stack>
            </Box>
          ) : null}
        </Box>
      </Group>

      {children.length > 0 ? (
        <Stack gap="md">
          {children.map((child, index) => (
            <CommentThread
              key={child.comment.id}
              apiBaseUrl={apiBaseUrl}
              thread={child}
              depth={depth + 1}
              isLastSibling={index === children.length - 1}
              gitlabIssueUrl={gitlabIssueUrl}
              issueId={issueId}
              activeReplyToNoteId={activeReplyToNoteId}
              commentCreateLoading={commentCreateLoading}
              uploadingAttachment={uploadingAttachment}
              setUploadingAttachment={setUploadingAttachment}
              uploadIssueAttachment={uploadIssueAttachment}
              deleteIssueUpload={deleteIssueUpload}
              onReplyToggle={onReplyToggle}
              onCreateComment={onCreateComment}
            />
          ))}
        </Stack>
      ) : null}
    </Stack>
  );
}

async function handlePasteUpload(
  event: ClipboardEvent<HTMLTextAreaElement>,
  issueId: string,
  currentValue: string,
  setValue: (value: string) => void,
  uploadIssueAttachment: (issueId: string, file: File) => Promise<IssueUpload>,
  onUploaded: (upload: IssueUpload) => void,
  setUploadingAttachment: (value: boolean) => void,
) {
  const file = Array.from(event.clipboardData.items)
    .find((item) => item.kind === 'file')
    ?.getAsFile();

  if (!file) {
    return;
  }

  event.preventDefault();
  setUploadingAttachment(true);

  try {
    const upload = await uploadIssueAttachment(issueId, file);
    onUploaded(upload);
    const nextValue = insertAtCursor(event.currentTarget, currentValue, upload.markdown);
    setValue(nextValue);
  } finally {
    setUploadingAttachment(false);
  }
}

async function handleFileInputUpload(
  event: ChangeEvent<HTMLInputElement>,
  issueId: string,
  currentValue: string,
  setValue: (value: string) => void,
  uploadIssueAttachment: (issueId: string, file: File) => Promise<IssueUpload>,
  onUploaded: (upload: IssueUpload) => void,
  setUploadingAttachment: (value: boolean) => void,
) {
  const file = event.currentTarget.files?.[0];
  if (!file) {
    return;
  }

  setUploadingAttachment(true);
  try {
    const upload = await uploadIssueAttachment(issueId, file);
    onUploaded(upload);
    setValue(insertAtEnd(currentValue, upload.markdown));
  } finally {
    setUploadingAttachment(false);
    event.currentTarget.value = '';
  }
}

async function handleDropUpload(
  event: DragEvent<HTMLDivElement>,
  issueId: string,
  currentValue: string,
  setValue: (value: string) => void,
  uploadIssueAttachment: (issueId: string, file: File) => Promise<IssueUpload>,
  onUploaded: (upload: IssueUpload) => void,
  setUploadingAttachment: (value: boolean) => void,
  setDragActive: (value: boolean) => void,
) {
  event.preventDefault();
  setDragActive(false);
  const file = event.dataTransfer.files?.[0];
  if (!file) {
    return;
  }

  setUploadingAttachment(true);
  try {
    const upload = await uploadIssueAttachment(issueId, file);
    onUploaded(upload);
    setValue(insertAtEnd(currentValue, upload.markdown));
  } finally {
    setUploadingAttachment(false);
  }
}

function insertAtCursor(textarea: HTMLTextAreaElement, currentValue: string, insertValue: string) {
  const start = textarea.selectionStart ?? currentValue.length;
  const end = textarea.selectionEnd ?? currentValue.length;
  const prefix = currentValue.slice(0, start);
  const suffix = currentValue.slice(end);
  const separatorBefore = prefix && !prefix.endsWith('\n') ? '\n' : '';
  const separatorAfter = suffix && !suffix.startsWith('\n') ? '\n' : '';
  return `${prefix}${separatorBefore}${insertValue}${separatorAfter}${suffix}`;
}

function insertAtEnd(currentValue: string, insertValue: string) {
  const separatorBefore = currentValue && !currentValue.endsWith('\n') ? '\n' : '';
  return `${currentValue}${separatorBefore}${insertValue}`;
}

function removeUploadMarkdown(currentValue: string, markdown: string, proxyPath: string) {
  return currentValue
    .replace(`${markdown}\n`, '')
    .replace(`\n${markdown}`, '')
    .replace(markdown, '')
    .replace(`${proxyPath}\n`, '')
    .replace(`\n${proxyPath}`, '')
    .replace(proxyPath, '')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

function parseReplyTarget(body: string): number | null {
  const match = body.match(/<!--\s*issuehub-parent:(\d+)\s*-->/i);
  return match ? Number(match[1]) : null;
}

function stripReplyMarker(body: string): string {
  return body.replace(/<!--\s*issuehub-parent:\d+\s*-->\s*/i, '').trim();
}

function normalizeIssueState(state: string) {
  return state.toLowerCase() === 'closed' ? 'closed' : 'open';
}

function getVisibleAttachmentLinks(body: string, attachments: Attachment[]) {
  return attachments.filter((attachment) => !isAttachmentReferencedInBody(body, attachment));
}

function isAttachmentReferencedInBody(body: string, attachment: Attachment) {
  const normalizedBody = body.toLowerCase();
  const candidates = [attachment.proxy_path, attachment.external_url, attachment.filename]
    .filter(Boolean)
    .map((value) => value.toLowerCase());

  return candidates.some((candidate) => normalizedBody.includes(candidate));
}

function buildCommentThreads(comments: Comment[]): CommentThreadNode[] {
  const nodes = comments
    .slice()
    .sort((left, right) => new Date(left.created_at).getTime() - new Date(right.created_at).getTime())
    .map((comment) => ({
      comment,
      replyTo: comment.reply_to_gitlab_note_id ?? parseReplyTarget(comment.body_raw),
      discussionId: comment.discussion_id ?? null,
      children: [] as CommentThreadNode[],
    }));

  const byNoteId = new Map<number, CommentThreadNode>();
  for (const node of nodes) {
    byNoteId.set(node.comment.gitlab_note_id, node);
  }

  const discussionRoots = new Map<string, CommentThreadNode>();
  const roots: CommentThreadNode[] = [];
  for (const node of nodes) {
    if (node.replyTo && byNoteId.has(node.replyTo)) {
      byNoteId.get(node.replyTo)?.children.push(node);
    } else if (node.discussionId && discussionRoots.has(node.discussionId)) {
      discussionRoots.get(node.discussionId)?.children.push(node);
    } else {
      roots.push(node);
      if (node.discussionId) {
        discussionRoots.set(node.discussionId, node);
      }
    }
  }

  return roots;
}

function PendingUploadsList({
  uploads,
  onRemove,
}: {
  uploads: IssueUpload[];
  onRemove?: (upload: IssueUpload) => Promise<void>;
}) {
  const { t } = useAppContext();

  if (!uploads.length) {
    return null;
  }

  return (
    <Stack gap="xs">
      <Text size="sm">{t('issueDetail.pendingUploads')}</Text>
      {uploads.map((upload) => (
        <Group key={upload.upload_id} justify="space-between" wrap="nowrap">
          <Anchor href={upload.proxy_path} target="_blank" rel="noreferrer" size="sm">
            {upload.filename}
          </Anchor>
          {onRemove ? (
            <Button size="compact-xs" variant="subtle" color="red" onClick={() => void onRemove(upload)}>
              {t('issueDetail.removeUpload')}
            </Button>
          ) : null}
        </Group>
      ))}
    </Stack>
  );
}

function mapUploadsToAttachments(uploads: IssueUpload[]) {
  return uploads.map((upload) => ({
    id: upload.upload_id,
    filename: upload.filename,
    content_type: upload.content_type,
    byte_size: upload.byte_size,
    external_url: upload.proxy_path,
    proxy_path: upload.proxy_path,
    inline: upload.content_type.startsWith('image/'),
    sync_state: 'pending',
  }));
}
