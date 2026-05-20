import {
  ActionIcon,
  Avatar,
  AppShell,
  Badge,
  Box,
  Burger,
  Group,
  Menu,
  NavLink,
  ScrollArea,
  SegmentedControl,
  Stack,
  Text,
  Tooltip,
  UnstyledButton,
  useComputedColorScheme,
  useMantineColorScheme,
  type MantineColorScheme,
} from '@mantine/core';
import { useDisclosure, useLocalStorage } from '@mantine/hooks';
import {
  IconActivityHeartbeat,
  IconChecklist,
  IconDeviceDesktop,
  IconFolders,
  IconLayoutDashboard,
  IconMoon,
  IconSun,
  IconUsersGroup,
} from '@tabler/icons-react';
import { Link, NavLink as RouterNavLink, Outlet, useLocation } from 'react-router-dom';

import { useAppContext } from '../context/AppContext';
import { gravatarUrl } from '../lib/gravatar';
import classes from './NavbarSimpleColored.module.css';

export function AppLayout() {
  const [mobileOpened, { toggle, close }] = useDisclosure(false);
  const [desktopCollapsed, setDesktopCollapsed] = useLocalStorage({
    key: 'issuehub-navbar-collapsed',
    defaultValue: false,
  });
  const { currentUser, projects, issues, logout, t } = useAppContext();
  const location = useLocation();
  const manageableProjects = projects.filter((project) => project.capabilities.can_manage);
	  const navItems = [
	    {
	      label: t('nav.overview'),
	      description: t('nav.overviewDesc'),
	      href: '/overview',
	      icon: <IconLayoutDashboard size={18} stroke={1.8} />,
	    },
	    {
	      label: t('nav.issues'),
	      description: t('nav.issuesDesc'),
	      href: '/issues',
	      icon: <IconChecklist size={18} stroke={1.8} />,
	    },
  ];
  if (currentUser?.is_admin || manageableProjects.length > 0) {
	    navItems.splice(1, 0, {
	      label: t('nav.projects'),
	      description: t('nav.projectsDesc'),
	      href: '/projects',
	      icon: <IconFolders size={18} stroke={1.8} />,
	    });
  }
  if (currentUser?.is_admin) {
	    navItems.splice(2, 0, {
	      label: t('nav.users'),
	      description: t('nav.usersDesc'),
	      href: '/users',
	      icon: <IconUsersGroup size={18} stroke={1.8} />,
	    });
	    navItems.splice(3, 0, {
	      label: t('nav.health'),
	      description: t('nav.healthDesc'),
	      href: '/admin/health',
	      icon: <IconActivityHeartbeat size={18} stroke={1.8} />,
	    });
  }

  return (
    <AppShell
      header={{ height: 76 }}
      navbar={{
        width: desktopCollapsed ? 88 : 320,
        breakpoint: 'md',
        collapsed: { mobile: !mobileOpened },
      }}
      padding="lg"
    >
      <AppShell.Header>
        <Group justify="space-between" h="100%" px="lg">
          <Group gap="md">
            <Burger opened={mobileOpened} onClick={toggle} hiddenFrom="md" size="sm" />
            <Burger
              opened={!desktopCollapsed}
              onClick={() => setDesktopCollapsed((current) => !current)}
              visibleFrom="md"
              size="sm"
            />
            <Box>
              <Text c="blue.7" fw={700} tt="uppercase" size="xs">
                {t('app.title')}
              </Text>
              <Text fw={700}>{t('app.subtitle')}</Text>
            </Box>
          </Group>

	          <Group gap="xs">
	            <Badge color="blue" variant="light">
	              {projects.length} {t('common.projects')}
	            </Badge>
	            <Badge color="blue" variant="light">
	              {issues.length} {t('common.issues')}
	            </Badge>
            <ThemeModeMenu />
            <Menu width={220} position="bottom-end" withinPortal>
              <Menu.Target>
                <UnstyledButton>
                  <Group gap="sm">
                    <Avatar
                      src={currentUser?.email ? gravatarUrl(currentUser.email, 80) : undefined}
                      radius="md"
                      size={36}
                    >
                      {currentUser?.full_name?.charAt(0) ?? '?'}
                    </Avatar>
                    <Box>
                      <Text size="sm" fw={600}>
                        {currentUser?.full_name}
                      </Text>
                      <Text size="xs" c="dimmed">
                        {currentUser?.email}
                      </Text>
                    </Box>
                  </Group>
                </UnstyledButton>
              </Menu.Target>

              <Menu.Dropdown>
                <Menu.Label>{t('common.account')}</Menu.Label>
                <Menu.Item component={Link} to="/profile">
                  {t('common.profile')}
                </Menu.Item>
	                {currentUser?.is_admin ? <Menu.Item disabled>{t('common.adminAccess')}</Menu.Item> : null}
                <Menu.Item onClick={() => void logout()} color="red">
                  {t('common.logout')}
                </Menu.Item>
              </Menu.Dropdown>
            </Menu>
          </Group>
        </Group>
      </AppShell.Header>

      <AppShell.Navbar p="md" className={classes.navbar}>
        <ScrollArea h="100%">
          <Stack gap="xs" className={classes.navbarMain}>
            {!desktopCollapsed ? (
              <Text size="xs" fw={700} c="var(--issuehub-nav-muted)" tt="uppercase" className={classes.header}>
                {t('app.navigation')}
              </Text>
            ) : null}
            {navItems.map((item) => (
              <Tooltip
                key={item.href}
                label={item.label}
                position="right"
                disabled={!desktopCollapsed}
              >
                <NavLink
                  className={`${classes.link} ${desktopCollapsed ? classes.linkCollapsed : ''} ${
                    location.pathname === item.href || location.pathname.startsWith(`${item.href}/`)
                      ? classes.linkActive
                      : ''
                  }`}
                  classNames={{
                    label: classes.linkLabel,
                    description: classes.linkDescription,
                  }}
                  component={RouterNavLink}
                  to={item.href}
                  label={desktopCollapsed ? undefined : item.label}
                  description={desktopCollapsed ? undefined : item.description}
                  leftSection={item.icon}
                  active={location.pathname === item.href || location.pathname.startsWith(`${item.href}/`)}
                  onClick={close}
                  variant="filled"
                />
              </Tooltip>
            ))}
          </Stack>
        </ScrollArea>
      </AppShell.Navbar>

      <AppShell.Main>
        <Stack gap="lg">
          <Outlet />
        </Stack>
      </AppShell.Main>
    </AppShell>
  );
}

function ThemeModeMenu() {
  const { t } = useAppContext();
  const { colorScheme, setColorScheme } = useMantineColorScheme();
  const computedColorScheme = useComputedColorScheme('light', { getInitialValueInEffect: true });
  const ThemeIcon =
    colorScheme === 'auto' ? IconDeviceDesktop : computedColorScheme === 'dark' ? IconMoon : IconSun;

  return (
    <Menu width={280} position="bottom-end" withinPortal>
      <Menu.Target>
        <Tooltip label={t('common.theme')}>
          <ActionIcon variant="default" size="lg" aria-label={t('common.theme')}>
            <ThemeIcon size={18} stroke={1.8} />
          </ActionIcon>
        </Tooltip>
      </Menu.Target>
      <Menu.Dropdown>
        <Menu.Label>{t('common.theme')}</Menu.Label>
        <Box px="sm" py={6}>
          <SegmentedControl
            fullWidth
            size="xs"
            value={colorScheme}
            onChange={(value) => setColorScheme(value as MantineColorScheme)}
            data={[
              {
                value: 'auto',
                label: (
                  <Group gap={4} justify="center" wrap="nowrap">
                    <IconDeviceDesktop size={14} stroke={1.8} />
                    <span>{t('theme.automatic')}</span>
                  </Group>
                ),
              },
              {
                value: 'light',
                label: (
                  <Group gap={4} justify="center" wrap="nowrap">
                    <IconSun size={14} stroke={1.8} />
                    <span>{t('theme.light')}</span>
                  </Group>
                ),
              },
              {
                value: 'dark',
                label: (
                  <Group gap={4} justify="center" wrap="nowrap">
                    <IconMoon size={14} stroke={1.8} />
                    <span>{t('theme.dark')}</span>
                  </Group>
                ),
              },
            ]}
          />
        </Box>
      </Menu.Dropdown>
    </Menu>
  );
}
