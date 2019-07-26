export interface Logger {
  
  debug(message: string);
  info(message: string);
  warn(message: string);
  error(message: string);
}

export interface StatsRecorder {
  recordCounterIncrement(tags: {}, name: string);
  recordTimer(tags: {}, name: string, value: number);
  recordGauge(tags: {}, name: string, value: number);
}