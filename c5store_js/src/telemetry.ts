export interface Logger {
  
  debug(message: string);
  info(message: string);
  warn(message: string);
  error(message: string, error: Error);
}

export interface StatsRecorder {
  recordCounterIncrement(tags: {[key: string]: any}, name: string);
  recordTimer(tags: {[key: string]: any}, name: string, value: number);
  recordGauge(tags: {[key: string]: any}, name: string, value: number);
}