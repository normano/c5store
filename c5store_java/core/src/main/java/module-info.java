module com.excsn.c5store.core {
  requires com.excsn.security.crypto.ecies_25519;
  requires com.fasterxml.jackson.databind;
  requires com.google.common;
  requires alphanumeric.comparator;
  requires org.yaml.snakeyaml;
  requires org.bouncycastle.provider;
  requires org.bouncycastle.pkix;

  exports com.excsn.c5store.core;
  exports com.excsn.c5store.core.telemetry;
  exports com.excsn.c5store.core.serializers;
  exports com.excsn.c5store.core.secrets;
}