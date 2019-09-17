export interface Logger {
  
  debug(message: string);
  info(message: string);
  warn(message: string);
  error(message: string, error: Error);
}

export interface StatsRecorder {
  recordCounterIncrement(tags: {[key: string]: string}, name: string);
  recordTimer(tags: {[key: string]: string}, name: string, value: number);
  recordGauge(tags: {[key: string]: string}, name: string, value: number);
}