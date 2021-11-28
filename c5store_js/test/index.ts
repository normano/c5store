import test from "ava";
import path from "path";

import type { C5Store } from "../src/index";
import { createC5Store } from "../src/index";
import { EciesX25519SecretDecryptor } from "../src/secrets";

const configDirectory = path.resolve(__dirname, "resources");

test("C5Store can initialize and stop", async (t) => {

  let c5StoreTuple = await createC5Store(
    [path.join(configDirectory, "common.yaml")],
  );

  const c5StoreInst = c5StoreTuple[0];
  const c5StoreMgr = c5StoreTuple[1];

  
  t.true(c5StoreMgr.stop());
});

test("C5Store reads common.yaml values", async (t) => {

  let c5StoreTuple = await createC5Store(
    [
      path.join(configDirectory, "common.yaml"),
    ],
  );

  const c5StoreInst = c5StoreTuple[0];
  const c5StoreMgr = c5StoreTuple[1];

  c5StoreMgr.stop();
  
  t.is(c5StoreInst.get("nodejs"), "great");
  t.is(c5StoreInst.get("node.is"), "good");
});

test("C5Store reads secrets", async (t) => {

  let c5StoreTuple = await createC5Store(
    [
      path.join(configDirectory, "secret_config.yaml"),
    ],
    {
      secretOpts: {
        secretKeysPath: path.join(configDirectory, "secret_keys"),
        secretKeyStoreConfigureFn: (secretKeyStore) => {
          secretKeyStore.setDecryptor("base64", {"decrypt": (value, key) => {
            
            return Buffer.from(value.toString("binary"), "base64");
          }});

          secretKeyStore.setDecryptor("ecies_x25519", new EciesX25519SecretDecryptor());
        }
      }
    }
  );

  const c5StoreInst = c5StoreTuple[0];
  const c5StoreMgr = c5StoreTuple[1];

  c5StoreMgr.stop();
  
  t.deepEqual(c5StoreInst.get("a_secret"), Buffer.from("abcd"));
  t.deepEqual(c5StoreInst.get("hello_secret"), Buffer.from("Hello World"));
});

test("C5Store fails due to bad secret decryption", async (t) => {

  let c5StoreTuple = await createC5Store(
    [
      path.join(configDirectory, "secret_config.yaml"),
    ],
  );

  const c5StoreInst = c5StoreTuple[0];
  const c5StoreMgr = c5StoreTuple[1];
  
  c5StoreMgr.stop();
  
  t.is(c5StoreInst.get("a_secret"), undefined);
});