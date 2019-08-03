package com.excsn.c5store.core;

public class C5InitHolder {
  public final C5Store config;
  public final C5StoreMgr configMgr;
  public final Runnable stopFn;

  public C5InitHolder(C5Store config, C5StoreMgr configMgr, Runnable stopFn) {

    this.config = config;
    this.configMgr = configMgr;
    this.stopFn = stopFn;
  }
}
