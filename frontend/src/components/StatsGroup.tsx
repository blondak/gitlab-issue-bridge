import { Text } from '@mantine/core';

import classes from './StatsGroup.module.css';

type StatItem = {
  title: string;
  value: number;
  description: string;
};

type StatsGroupProps = {
  data: StatItem[];
};

export function StatsGroup({ data }: StatsGroupProps) {
  const stats = data.map((stat) => (
    <div className={classes.stat} key={stat.title}>
      <Text className={classes.title}>{stat.title}</Text>
      <Text className={classes.count}>{stat.value}</Text>
      <Text className={classes.description}>{stat.description}</Text>
    </div>
  ));

  return <div className={classes.root}>{stats}</div>;
}
