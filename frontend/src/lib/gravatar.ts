import md5 from 'js-md5';

export function gravatarUrl(email: string, size = 80) {
  const normalizedEmail = email.trim().toLowerCase();
  const hash = md5(normalizedEmail);
  return `https://www.gravatar.com/avatar/${hash}?d=identicon&s=${size}`;
}
