import { Anchor, Box, Button, Group, Input, Modal, SegmentedControl, Select, Stack, Text, Textarea, TextInput } from '@mantine/core';
import { useForm } from '@mantine/form';
import { useEffect, useRef, useState, type ChangeEvent, type ClipboardEvent, type DragEvent } from 'react';

import { useAppContext } from '../context/AppContext';
import type { CreateIssueValues, Project } from '../types';
import { CommentMarkdown } from './CommentMarkdown';

type CreateIssueModalProps = {
  opened: boolean;
  onClose: () => void;
  projects: Project[];
  initialProjectId: string | null;
  loading?: boolean;
  onSubmit: (values: CreateIssueValues) => Promise<void> | void;
};

export function CreateIssueModal({
  opened,
  onClose,
  projects,
  initialProjectId,
  loading,
  onSubmit,
}: CreateIssueModalProps) {
  const { t, uploadProjectIssueAttachment, deleteIssueUpload } = useAppContext();
  const form = useForm<CreateIssueValues>({
    initialValues: {
      project_id: initialProjectId ?? '',
      title: '',
      description: '',
    },
	    validate: {
	      project_id: (value) => (value ? null : t('issues.projectRequired')),
	      title: (value) => (value.trim().length > 2 ? null : t('issues.titleRequired')),
	    },
	  });
  const [pendingUploads, setPendingUploads] = useState<Array<{ upload_id: string; filename: string; markdown: string; proxy_path: string }>>([]);
  const [uploadingAttachment, setUploadingAttachment] = useState(false);
  const [dragActive, setDragActive] = useState(false);
	  const [editorMode, setEditorMode] = useState<'write' | 'preview'>('write');
	  const fileInputRef = useRef<HTMLInputElement | null>(null);
	  const canUpload = Boolean(form.values.project_id);

  useEffect(() => {
    if (opened) {
      form.setValues((current) => ({
        ...current,
        project_id: initialProjectId ?? current.project_id,
      }));
    }
  }, [opened, initialProjectId]);

  return (
	    <Modal opened={opened} onClose={onClose} title={t('issues.create')} centered>
      <form
        onSubmit={form.onSubmit(async (values) => {
          await onSubmit(values);
          form.reset();
          setPendingUploads([]);
          onClose();
        })}
      >
        <Stack gap="md">
	          <Select
	            label={t('issues.project')}
	            placeholder={t('issues.projectRequired')}
            data={projects.map((project) => ({ value: project.id, label: project.name }))}
            {...form.getInputProps('project_id')}
            searchable
            required
          />
	          <TextInput
	            label={t('issues.issueTitle')}
	            placeholder={t('issues.titlePlaceholder')}
            {...form.getInputProps('title')}
            required
          />
          <Stack gap="xs">
	            <Text size="sm" fw={500}>
	              {t('issues.description')}
            </Text>
            <SegmentedControl
              value={editorMode}
              onChange={(value) => setEditorMode(value as 'write' | 'preview')}
              data={[
                { label: t('common.write'), value: 'write' },
                { label: t('common.preview'), value: 'preview' },
              ]}
            />
            {editorMode === 'write' ? (
              <Textarea
	                placeholder={t('issues.descriptionPlaceholder')}
                minRows={8}
                autosize
                maxRows={18}
                onPaste={(event) =>
                  form.values.project_id
                    ? void handleProjectUploadPaste(
                        event,
                        form.values.project_id,
                        form.values.description,
                        (value) => form.setFieldValue('description', value),
                        uploadProjectIssueAttachment,
                        (upload) => setPendingUploads((current) => [...current, upload]),
                        setUploadingAttachment,
                      )
                    : undefined
                }
                {...form.getInputProps('description')}
              />
            ) : (
              <Box
                p="sm"
                mih={220}
                style={{
                  border: '1px solid var(--mantine-color-gray-3)',
                  borderRadius: 'var(--mantine-radius-md)',
                  background: 'var(--mantine-color-body)',
                }}
              >
                {form.values.description.trim() ? (
                  <CommentMarkdown
                    apiBaseUrl=""
                    body={form.values.description}
                    attachments={pendingUploads.map((upload) => ({
                      id: upload.upload_id,
                      filename: upload.filename,
                      content_type: upload.markdown.startsWith('![') ? 'image/*' : 'application/octet-stream',
                      byte_size: 0,
                      external_url: upload.proxy_path,
                      proxy_path: upload.proxy_path,
                      inline: upload.markdown.startsWith('!['),
                      sync_state: 'pending',
                    }))}
                  />
                ) : (
                  <Text size="sm" c="dimmed">
                    {t('common.previewEmpty')}
                  </Text>
                )}
              </Box>
            )}
          </Stack>
          <Box
            p="sm"
	            style={{
	              border: `1px dashed ${dragActive ? 'var(--mantine-color-blue-5)' : 'var(--mantine-color-gray-4)'}`,
	              borderRadius: 'var(--mantine-radius-md)',
	              background: dragActive
	                ? 'light-dark(var(--mantine-color-blue-0), rgba(25, 113, 194, 0.16))'
	                : undefined,
	              opacity: canUpload ? 1 : 0.72,
	            }}
	            onDragOver={(event) => {
	              event.preventDefault();
	              if (canUpload) {
	                setDragActive(true);
	              }
	            }}
            onDragLeave={() => setDragActive(false)}
            onDrop={(event) =>
	              canUpload
	                ? void handleProjectUploadDrop(
                    event,
                    form.values.project_id,
                    form.values.description,
                    (value) => form.setFieldValue('description', value),
                    uploadProjectIssueAttachment,
                    (upload) => setPendingUploads((current) => [...current, upload]),
                    setUploadingAttachment,
                    setDragActive,
                  )
                : undefined
            }
          >
            <Stack gap="sm">
              <Group justify="space-between" align="center">
	                <Text size="xs" c="dimmed">
	                  {canUpload ? t('issueDetail.dragAndDropHint') : t('issues.uploadNeedsProject')}
	                </Text>
                <Button
                  variant="default"
                  size="xs"
	                  disabled={!canUpload}
                  onClick={() => fileInputRef.current?.click()}
                  loading={uploadingAttachment}
                >
                  {t('issueDetail.attachFile')}
                </Button>
              </Group>
              <Input
                ref={fileInputRef}
                type="file"
                style={{ display: 'none' }}
                onChange={(event) =>
                  form.values.project_id
                    ? void handleProjectUploadInput(
                        event,
                        form.values.project_id,
                        form.values.description,
                        (value) => form.setFieldValue('description', value),
                        uploadProjectIssueAttachment,
                        (upload) => setPendingUploads((current) => [...current, upload]),
                        setUploadingAttachment,
                      )
                    : undefined
                }
              />
              {pendingUploads.length > 0 ? (
                <Stack gap="xs">
                  <Text size="sm">{t('issueDetail.pendingUploads')}</Text>
                  {pendingUploads.map((upload) => (
                    <Group key={upload.upload_id} justify="space-between" wrap="nowrap">
                      <Anchor href={upload.proxy_path} target="_blank" rel="noreferrer" size="sm">
                        {upload.filename}
                      </Anchor>
                      <Button
                        size="compact-xs"
                        variant="subtle"
                        color="red"
                        onClick={async () => {
                          await deleteIssueUpload(upload.upload_id);
                          setPendingUploads((current) =>
                            current.filter((candidate) => candidate.upload_id !== upload.upload_id),
                          );
                          form.setFieldValue(
                            'description',
                            removeUploadMarkdown(form.values.description, upload.markdown, upload.proxy_path),
                          );
                        }}
                      >
                        {t('issueDetail.removeUpload')}
                      </Button>
                    </Group>
                  ))}
                </Stack>
              ) : null}
            </Stack>
          </Box>
	          <Button type="submit" loading={loading || uploadingAttachment}>
	            {t('issues.create')}
	          </Button>
        </Stack>
      </form>
    </Modal>
  );
}

async function handleProjectUploadPaste(
  event: ClipboardEvent<HTMLTextAreaElement>,
  projectId: string,
  currentValue: string,
  setValue: (value: string) => void,
  uploadProjectIssueAttachment: (projectId: string, file: File) => Promise<{ markdown: string; upload_id: string; filename: string; proxy_path: string }>,
  onUploaded: (upload: { markdown: string; upload_id: string; filename: string; proxy_path: string }) => void,
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
    const upload = await uploadProjectIssueAttachment(projectId, file);
    onUploaded(upload);
    setValue(insertAtCursor(event.currentTarget, currentValue, upload.markdown));
  } finally {
    setUploadingAttachment(false);
  }
}

async function handleProjectUploadInput(
  event: ChangeEvent<HTMLInputElement>,
  projectId: string,
  currentValue: string,
  setValue: (value: string) => void,
  uploadProjectIssueAttachment: (projectId: string, file: File) => Promise<{ markdown: string; upload_id: string; filename: string; proxy_path: string }>,
  onUploaded: (upload: { markdown: string; upload_id: string; filename: string; proxy_path: string }) => void,
  setUploadingAttachment: (value: boolean) => void,
) {
  const file = event.currentTarget.files?.[0];
  if (!file) {
    return;
  }

  setUploadingAttachment(true);
  try {
    const upload = await uploadProjectIssueAttachment(projectId, file);
    onUploaded(upload);
    setValue(insertAtEnd(currentValue, upload.markdown));
  } finally {
    setUploadingAttachment(false);
    event.currentTarget.value = '';
  }
}

async function handleProjectUploadDrop(
  event: DragEvent<HTMLDivElement>,
  projectId: string,
  currentValue: string,
  setValue: (value: string) => void,
  uploadProjectIssueAttachment: (projectId: string, file: File) => Promise<{ markdown: string; upload_id: string; filename: string; proxy_path: string }>,
  onUploaded: (upload: { markdown: string; upload_id: string; filename: string; proxy_path: string }) => void,
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
    const upload = await uploadProjectIssueAttachment(projectId, file);
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
