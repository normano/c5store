module com.excsn.c5store.core {
  requires com.fasterxml.jackson.databind;
  requires com.google.common;
  requires alphanumeric.comparator;
  requires snakeyaml;

  exports com.excsn.c5store.core;
  exports com.excsn.c5store.core.telemetry;
  exports com.excsn.c5store.core.serializers;
}