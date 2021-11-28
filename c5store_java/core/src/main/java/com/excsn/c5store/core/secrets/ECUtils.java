package com.excsn.c5store.core.secrets;

import org.bouncycastle.asn1.pkcs.PrivateKeyInfo;
import org.bouncycastle.asn1.x509.SubjectPublicKeyInfo;
import org.bouncycastle.openssl.PEMParser;
import org.bouncycastle.openssl.jcajce.JcaPEMKeyConverter;

import java.io.IOException;
import java.io.Reader;
import java.security.PrivateKey;
import java.security.PublicKey;

public class ECUtils {
  public static PublicKey readPublicKey(Reader keyReader) throws IOException {
    PEMParser pemParser = new PEMParser(keyReader);
    JcaPEMKeyConverter converter = new JcaPEMKeyConverter();
    SubjectPublicKeyInfo publicKeyInfo = SubjectPublicKeyInfo.getInstance(pemParser.readObject());
    return converter.getPublicKey(publicKeyInfo);
  }

  public static PrivateKey readPrivateKey(Reader keyReader) throws IOException {
    PEMParser pemParser = new PEMParser(keyReader);
    JcaPEMKeyConverter converter = new JcaPEMKeyConverter();
    PrivateKeyInfo privateKeyInfo = PrivateKeyInfo.getInstance(pemParser.readObject());

    return converter.getPrivateKey(privateKeyInfo);
  }
}
