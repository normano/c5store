import EciesX25519 from "@excsn/ecies_25519";

export interface SecretDecryptor {
  decrypt(encryptedValue: Buffer, key: Buffer): Buffer;
}

export class EciesX25519SecretDecryptor implements SecretDecryptor {

  private eciesX25519Inst = new EciesX25519();

  public decrypt(encryptedValue: Buffer, key: Buffer): Buffer {

    var decodedValue = Buffer.from(encryptedValue.toString(), "base64");
    return this.eciesX25519Inst.decrypt(key, decodedValue);
  }
}

/**
 * Stores decryptors and keys for decryption coordination.
 */
export class SecretKeyStore {

  private _secretDecryptors: Map<string, SecretDecryptor>;
  private _keys: Map<string, Buffer>;
  
  constructor() {
    this._secretDecryptors = new Map();
    this._keys = new Map();
  }

  public getDecryptor(name: string): SecretDecryptor | undefined {
    return this._secretDecryptors.get(name);
  }

  public setDecryptor(name: string, decryptor: SecretDecryptor): void {
    this._secretDecryptors.set(name, decryptor);
  }

  public getKey(name: string): Buffer | undefined {
    return this._keys.get(name);
  }

  public setKey(name: string, key: Buffer): void {
    this._keys.set(name, key);
  }
}