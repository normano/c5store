export interface Logger {
  
  debug(message: string): void;
  info(message: string): void;
  warn(message: string): void;
  error(message: string, error: Error): void;
}

export interface StatsRecorder {
  recordCounterIncrement(tags: {[key: string]: any}, name: string): void;
  recordTimer(tags: {[key: string]: any}, name: string, value: number): void;
  recordGauge(tags: {[key: string]: any}, name: string, value: number): void;
}