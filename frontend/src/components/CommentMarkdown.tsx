import { Anchor, Box, Code, Image, List, Text } from '@mantine/core';
import type { ReactNode } from 'react';

import { useAppContext } from '../context/AppContext';
import type { Attachment } from '../types';

type CommentMarkdownProps = {
  apiBaseUrl: string;
  body: string;
  attachments: Attachment[];
};

type InlineToken =
  | { type: 'text'; value: string }
  | { type: 'code'; value: string }
  | { type: 'strong'; value: string }
  | { type: 'em'; value: string }
  | { type: 'link'; label: string; target: string }
  | { type: 'image'; alt: string; target: string; width?: number; height?: number };

export function CommentMarkdown({ apiBaseUrl, body, attachments }: CommentMarkdownProps) {
  const { t } = useAppContext();
  const normalizedBody = stripReplyMarker(body);

  if (looksLikeHtmlFragment(normalizedBody)) {
    return <Box>{renderHtmlFragment(normalizedBody, attachments, apiBaseUrl, t)}</Box>;
  }

  const blocks = parseBlocks(normalizedBody);

  return (
    <Box>
      {blocks.map((block, index) => {
        if (block.type === 'code') {
          return (
            <Box
              key={`code-${index}`}
              component="pre"
              mb="sm"
              p="sm"
              style={{
                overflowX: 'auto',
                whiteSpace: 'pre-wrap',
                background: 'light-dark(var(--mantine-color-gray-0), var(--mantine-color-dark-6))',
                borderRadius: 'var(--mantine-radius-md)',
                fontSize: 'var(--mantine-font-size-sm)',
                lineHeight: 1.55,
              }}
            >
              {block.language ? (
                <Text component="span" size="xs" c="dimmed" style={{ display: 'block', marginBottom: 8 }}>
                  {block.language}
                </Text>
              ) : null}
              <Box component="code" style={{ fontFamily: 'monospace' }}>
                {block.code}
              </Box>
            </Box>
          );
        }

        if (block.type === 'list') {
          return (
            <List key={`list-${index}`} spacing={4} size="sm" mb="sm">
              {block.items.map((item, itemIndex) => (
                <List.Item key={`item-${itemIndex}`}>
                  {renderInline(parseInline(item), attachments, apiBaseUrl, t)}
                </List.Item>
              ))}
            </List>
          );
        }

        return (
          <Text key={`paragraph-${index}`} size="sm" mb="sm" style={{ whiteSpace: 'pre-wrap' }}>
            {renderInline(parseInline(block.text), attachments, apiBaseUrl, t)}
          </Text>
        );
      })}
    </Box>
  );
}

function renderInline(tokens: InlineToken[], attachments: Attachment[], apiBaseUrl: string, t: (key: string) => string): ReactNode[] {
  return tokens.map((token, index) => {
    if (token.type === 'text') {
      return <span key={index}>{renderPlainTextWithLinks(token.value)}</span>;
    }

    if (token.type === 'code') {
      return <Code key={index}>{token.value}</Code>;
    }

    if (token.type === 'strong') {
      return (
        <Text key={index} span fw={700}>
          {token.value}
        </Text>
      );
    }

    if (token.type === 'em') {
      return (
        <Text key={index} span fs="italic">
          {token.value}
        </Text>
      );
    }

    if (token.type === 'image') {
      const resolved = resolveTarget(token.target, attachments, apiBaseUrl);
      return (
        <Anchor key={index} href={resolved.href} target="_blank" rel="noreferrer" title={t('issueDetail.openFullImage')}>
          <Image
            src={resolved.href}
            alt={token.alt || 'attachment'}
            maw={520}
            w={token.width}
            h={token.height}
            style={{
              maxWidth: '100%',
              height: token.height ? `${token.height}px` : 'auto',
            }}
            radius="md"
            mt="xs"
            mb="xs"
          />
        </Anchor>
      );
    }

    const resolved = resolveTarget(token.target, attachments, apiBaseUrl);
    return (
      <Anchor key={index} href={resolved.href} target="_blank" rel="noreferrer">
        {token.label}
      </Anchor>
    );
  });
}

function resolveTarget(target: string, attachments: Attachment[], apiBaseUrl: string) {
  const normalized = normalizeTarget(target);
  const matchedAttachment = attachments.find((attachment) => {
    const external = normalizeTarget(attachment.external_url);
    return external === normalized || external.endsWith(normalized) || normalized.endsWith(external);
  });

  if (matchedAttachment) {
    return {
      href: `${apiBaseUrl}${matchedAttachment.proxy_path}`,
    };
  }

  return { href: target };
}

function normalizeTarget(target: string) {
  return target.trim().replace(/^https?:\/\/[^/]+/i, '');
}

function parseBlocks(body: string) {
  const blocks: Array<
    | { type: 'paragraph'; text: string }
    | { type: 'list'; items: string[] }
    | { type: 'code'; language: string | null; code: string }
  > = [];
  const lines = body.split('\n');
  let index = 0;

  while (index < lines.length) {
    const line = lines[index];
    const fenceMatch = line.match(/^```([^\s`]*)\s*$/);

    if (fenceMatch) {
      const language = fenceMatch[1] ? fenceMatch[1] : null;
      const codeLines: string[] = [];
      index += 1;

      while (index < lines.length && !/^```$/.test(lines[index])) {
        codeLines.push(lines[index]);
        index += 1;
      }

      if (index < lines.length && /^```$/.test(lines[index])) {
        index += 1;
      }

      blocks.push({
        type: 'code',
        language,
        code: codeLines.join('\n'),
      });
      continue;
    }

    const paragraphLines: string[] = [];
    while (index < lines.length && !/^```([^\s`]*)\s*$/.test(lines[index])) {
      paragraphLines.push(lines[index]);
      index += 1;
    }

    const sections = paragraphLines.join('\n').split(/\n{2,}/).map((value) => value.trim()).filter(Boolean);

    for (const section of sections) {
      const sectionLines = section.split('\n').map((value) => value.trimEnd());
      const listItems = sectionLines
        .filter((value) => /^[-*]\s+/.test(value))
        .map((value) => value.replace(/^[-*]\s+/, ''));

      if (listItems.length === sectionLines.length && listItems.length > 0) {
        blocks.push({ type: 'list', items: listItems });
      } else {
        blocks.push({ type: 'paragraph', text: section });
      }
    }
  }

  return blocks;
}

function parseInline(input: string): InlineToken[] {
  const tokens: InlineToken[] = [];
  let cursor = 0;
  const pattern =
    /!\[([^\]]*)\]\(([^)]+)\)(?:\s*\{([^}]*)\})?|\[([^\]]+)\]\(([^)]+)\)|`([^`]+)`|\*\*([^*]+)\*\*|\*([^*]+)\*/g;

  for (const match of input.matchAll(pattern)) {
    const index = match.index ?? 0;
    if (index > cursor) {
      tokens.push({ type: 'text', value: input.slice(cursor, index) });
    }

    if (match[1] !== undefined && match[2] !== undefined) {
      const attributes = parseImageAttributes(match[3]);
      tokens.push({
        type: 'image',
        alt: match[1],
        target: match[2],
        width: attributes.width,
        height: attributes.height,
      });
    } else if (match[4] !== undefined && match[5] !== undefined) {
      tokens.push({ type: 'link', label: match[4], target: match[5] });
    } else if (match[6] !== undefined) {
      tokens.push({ type: 'code', value: match[6] });
    } else if (match[7] !== undefined) {
      tokens.push({ type: 'strong', value: match[7] });
    } else if (match[8] !== undefined) {
      tokens.push({ type: 'em', value: match[8] });
    }

    cursor = index + match[0].length;
  }

  if (cursor < input.length) {
    tokens.push({ type: 'text', value: input.slice(cursor) });
  }

  if (tokens.length === 0) {
    tokens.push({ type: 'text', value: input });
  }

  return tokens;
}

function stripReplyMarker(body: string): string {
  return body.replace(/<!--\s*issuehub-parent:\d+\s*-->\s*/i, '').trim();
}

function renderPlainTextWithLinks(value: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  let cursor = 0;
  const urlPattern = /\bhttps?:\/\/[^\s<]+[^\s<.,!?;:)]/g;

  for (const match of value.matchAll(urlPattern)) {
    const index = match.index ?? 0;
    const url = match[0];

    if (index > cursor) {
      nodes.push(value.slice(cursor, index));
    }

    nodes.push(
      <Anchor key={`${index}-${url}`} href={url} target="_blank" rel="noreferrer">
        {url}
      </Anchor>,
    );
    cursor = index + url.length;
  }

  if (cursor < value.length) {
    nodes.push(value.slice(cursor));
  }

  return nodes.length > 0 ? nodes : [value];
}

function parseImageAttributes(rawAttributes?: string) {
  if (!rawAttributes) {
    return {};
  }

  const widthMatch = rawAttributes.match(/\bwidth=(\d+)\b/i);
  const heightMatch = rawAttributes.match(/\bheight=(\d+)\b/i);

  return {
    width: widthMatch ? Number(widthMatch[1]) : undefined,
    height: heightMatch ? Number(heightMatch[1]) : undefined,
  };
}

function looksLikeHtmlFragment(body: string) {
  const trimmed = body.trim();
  return /^<([a-z][a-z0-9-]*)(\s[^>]*)?>/i.test(trimmed);
}

function renderHtmlFragment(
  body: string,
  attachments: Attachment[],
  apiBaseUrl: string,
  t: (key: string) => string,
): ReactNode[] {
  if (typeof window === 'undefined' || typeof DOMParser === 'undefined') {
    return [body];
  }

  const parser = new DOMParser();
  const document = parser.parseFromString(`<div>${body}</div>`, 'text/html');
  const root = document.body.firstElementChild;

  if (!root) {
    return [body];
  }

  return Array.from(root.childNodes).map((node, index) =>
    renderHtmlNode(node, attachments, apiBaseUrl, `html-${index}`, t),
  );
}

function renderHtmlNode(
  node: ChildNode,
  attachments: Attachment[],
  apiBaseUrl: string,
  key: string,
  t: (key: string) => string,
): ReactNode {
  if (node.nodeType === Node.TEXT_NODE) {
    return node.textContent ?? '';
  }

  if (node.nodeType !== Node.ELEMENT_NODE) {
    return null;
  }

  const element = node as HTMLElement;
  const children = Array.from(element.childNodes).map((child, index) =>
    renderHtmlNode(child, attachments, apiBaseUrl, `${key}-${index}`, t),
  );
  const className = element.getAttribute('class') || undefined;
  const style = className && className.includes('idiff')
    ? {
        fontFamily: 'monospace',
      }
    : undefined;

  switch (element.tagName.toLowerCase()) {
    case 'p':
      return (
        <Text key={key} size="sm" mb="sm">
          {children}
        </Text>
      );
    case 'code':
      return (
        <Code key={key} style={style}>
          {children}
        </Code>
      );
    case 'pre':
      return (
        <Box
          key={key}
          component="pre"
          mb="sm"
          p="sm"
          style={{
            overflowX: 'auto',
            whiteSpace: 'pre-wrap',
            background: 'light-dark(var(--mantine-color-gray-0), var(--mantine-color-dark-6))',
            borderRadius: 'var(--mantine-radius-md)',
            fontSize: 'var(--mantine-font-size-sm)',
            lineHeight: 1.55,
          }}
        >
          {children}
        </Box>
      );
    case 'strong':
      return (
        <Text key={key} span fw={700}>
          {children}
        </Text>
      );
    case 'em':
      return (
        <Text key={key} span fs="italic">
          {children}
        </Text>
      );
    case 'br':
      return <br key={key} />;
    case 'span':
      return (
        <Text key={key} span className={className} style={style}>
          {children}
        </Text>
      );
    case 'a': {
      const href = element.getAttribute('href') || '#';
      const resolved = resolveTarget(href, attachments, apiBaseUrl);
      return (
        <Anchor key={key} href={resolved.href} target="_blank" rel="noreferrer">
          {children}
        </Anchor>
      );
    }
    case 'img': {
      const src = element.getAttribute('src') || '';
      const alt = element.getAttribute('alt') || 'attachment';
      const resolved = resolveTarget(src, attachments, apiBaseUrl);
      return (
        <Anchor key={key} href={resolved.href} target="_blank" rel="noreferrer" title={t('issueDetail.openFullImage')}>
          <Image
            src={resolved.href}
            alt={alt}
            maw={520}
            radius="md"
            mt="xs"
            mb="xs"
            style={{ maxWidth: '100%', height: 'auto' }}
          />
        </Anchor>
      );
    }
    case 'ul':
      return <List key={key} spacing={4} size="sm" mb="sm">{children}</List>;
    case 'ol':
      return <List key={key} type="ordered" spacing={4} size="sm" mb="sm">{children}</List>;
    case 'li':
      return <List.Item key={key}>{children}</List.Item>;
    default:
      return (
        <span key={key} className={className}>
          {children}
        </span>
      );
  }
}
